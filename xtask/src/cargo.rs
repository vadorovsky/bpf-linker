use std::{
    ffi::{OsStr, OsString},
    fs::{create_dir_all, remove_dir_all},
    os::unix::ffi::OsStringExt,
    path::Path,
    process::{Command, Stdio},
};

use clap::Parser;
use target_lexicon::Triple;
use thiserror::Error;

use crate::{
    containers::{ContainerEngine, ContainerError},
    target::{SupportedTriple, TripleExt},
};

#[derive(Debug, Error)]
pub enum CargoError {
    #[error(transparent)]
    Container(ContainerError),
    #[error("cargo build failed")]
    CargoBuild,
    #[error("could not find a git repository")]
    RepositoryNotFound,
}

#[derive(Parser)]
pub struct CargoArgs {
    /// Container engine (if not provided, is going to be autodetected).
    #[arg(long)]
    container_engine: Option<ContainerEngine>,

    /// Prefix in which LLVM libraries are going to be installed after build.
    #[arg(long)]
    llvm_install_dir: Option<OsString>,

    /// Target triple (optional).
    #[arg(short, long)]
    target: Option<SupportedTriple>,

    /// Build artifacts in release mode, with optimizations.
    #[arg(long)]
    release: bool,
}

pub fn run_cargo(args: CargoArgs, command: &OsStr) -> anyhow::Result<()> {
    let CargoArgs {
        container_engine,
        llvm_install_dir,
        target,
        release,
    } = args;

    let triple: Triple = match target {
        Some(target) => target.into(),
        None => target_lexicon::HOST,
    };

    let llvm_install_dir = match llvm_install_dir {
        Some(llvm_install_dir) => llvm_install_dir,
        None => Path::new("/tmp")
            .join(format!("aya-llvm-{triple}"))
            .into_os_string(),
    };
    if Path::new(&llvm_install_dir).exists() {
        remove_dir_all(&llvm_install_dir)?;
    }
    create_dir_all(&llvm_install_dir)?;

    let workdir = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output();
    let workdir = match workdir {
        Ok(output) if output.status.success() => {
            OsString::from_vec(
                // Remove the trailing `\n` character.
                output.stdout[..output.stdout.len() - 1].to_vec(),
            )
        }
        Ok(_) => {
            return Err(CargoError::RepositoryNotFound.into());
        }
        Err(_) => {
            return Err(CargoError::RepositoryNotFound.into());
        }
    };

    match triple.container_image() {
        Some((container_image, _)) => {
            println!("Using container image {container_image}");

            let container_engine =
                container_engine.unwrap_or(ContainerEngine::autodetect().ok_or(
                    CargoError::Container(ContainerError::ContainerEngineNotFound),
                )?);

            let mut workdir_arg = workdir;
            workdir_arg.push(":/usr/local/src/bpf-linker");

            let mut llvm_arg = llvm_install_dir.clone();
            llvm_arg.push(":");
            llvm_arg.push(&llvm_install_dir);

            let mut cmd = Command::new(container_engine.to_string());
            cmd.args([
                OsStr::new("run"),
                OsStr::new("-it"),
                OsStr::new("-w"),
                OsStr::new("/usr/local/src/bpf-linker"),
                OsStr::new("-v"),
                &workdir_arg,
                OsStr::new("-v"),
                &llvm_arg,
                OsStr::new(&container_image),
                OsStr::new("cargo"),
                command,
            ]);
            if release {
                cmd.arg("--release");
            }
            cmd.stdout(Stdio::inherit()).stderr(Stdio::inherit());
            println!("{cmd:?}");
            if !cmd.status()?.success() {
                return Err(CargoError::CargoBuild.into());
            }
        }
        None => {}
    }

    Ok(())
}
