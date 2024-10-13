use std::{
    env,
    ffi::{OsStr, OsString},
    fs::read_dir,
    os::unix::ffi::OsStringExt,
    path::Path,
    process::{Command, Stdio},
};

use anyhow::Context;
use clap::{ArgAction, Parser, ValueEnum};
use target_lexicon::{Environment, Triple};
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

#[derive(Clone, ValueEnum)]
pub enum LinkType {
    Dynamic,
    Static,
}

impl ToString for LinkType {
    fn to_string(&self) -> String {
        match self {
            Self::Dynamic => "dylib".to_owned(),
            Self::Static => "static".to_owned(),
        }
    }
}

impl LinkType {
    fn default(triple: &Triple) -> Self {
        // Link system libraries dynamically only on GNU/Linux or, as I've
        // recently taken to calling it, GNU plus Linux. The reason being -
        // Ubuntu doesn't ship static zlib and zstd.
        // Static linking works fine on other systems (BSDs, macOS,
        // musl/Linux).
        if triple.environment == Environment::Gnu {
            Self::Dynamic
        } else {
            Self::Static
        }
    }
}

#[derive(Parser)]
pub struct CargoArgs {
    /// Container engine (if not provided, is going to be autodetected).
    #[arg(long)]
    container_engine: Option<ContainerEngine>,

    /// Space or comma separated list of features to activate.
    #[arg(short, long)]
    features: Vec<OsString>,

    /// Activate all available features.
    #[arg(long)]
    all_features: bool,

    /// Do not activate the `default` feature.
    #[arg(long)]
    no_default_features: bool,

    #[arg(long)]
    link_type: Option<LinkType>,

    /// Prefix in which LLVM libraries are going to be installed after build.
    #[arg(long)]
    llvm_install_dir: Option<OsString>,

    /// Build artifacts in release mode, with optimizations.
    #[arg(long)]
    release: bool,

    /// Target triple (optional).
    #[arg(short, long)]
    target: Option<SupportedTriple>,

    /// Use verbose output (-vv very verbose/build.rs output).
    #[arg(short, long, action = ArgAction::Count)]
    verbose: u8,
}

pub fn run_cargo(args: CargoArgs, command: &OsStr) -> anyhow::Result<()> {
    let CargoArgs {
        container_engine,
        mut features,
        all_features,
        no_default_features,
        link_type,
        llvm_install_dir,
        release,
        target,
        verbose,
    } = args;

    // Disable the LLVM linking capabilities from llvm-sys, they don't support
    // cross-compilation. Instead, we are building our own linking flags, based
    // on the specified `llvm_install_dir`.
    features.push(OsString::from("llvm-sys/no-llvm-linking"));

    let triple: Triple = match target {
        Some(target) => target.into(),
        None => target_lexicon::HOST,
    };

    let link_type = link_type.unwrap_or(LinkType::default(&triple));

    let llvm_install_dir = match llvm_install_dir {
        Some(llvm_install_dir) => llvm_install_dir,
        None => Path::new("/tmp")
            .join(format!("aya-llvm-{triple}"))
            .into_os_string(),
    };

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

    let mut rustflags = OsString::from("RUSTFLAGS=-L native=");
    rustflags.push(Path::new(&llvm_install_dir).join("lib"));
    rustflags.push(" -L native=/lib -L native=/usr/lib");
    rustflags.push(format!(" -l {}=rt", link_type.to_string()));
    rustflags.push(format!(" -l {}=dl", link_type.to_string()));
    rustflags.push(format!(" -l {}=m", link_type.to_string()));
    rustflags.push(format!(" -l {}=z", link_type.to_string()));
    rustflags.push(format!(" -l {}=zstd", link_type.to_string()));
    if triple.environment == Environment::Gnu {
        rustflags.push(format!(" -l {}=stdc++", link_type.to_string()));
    } else {
        rustflags.push(format!(" -l {}=c++_static", link_type.to_string()));
        rustflags.push(format!(" -l {}=c++abi", link_type.to_string()));
    }

    for entry in read_dir(Path::new(&llvm_install_dir).join("lib"))
        .context("LLVM build directory not found")?
    {
        let entry = entry.context("failed to retrieve the file in the LLVM build directory")?;
        let path = entry.path();
        if path.is_file() && path.extension().and_then(|ext| ext.to_str()) == Some("a") {
            rustflags.push(" -l static=");
            rustflags.push(
                path.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .strip_prefix("lib")
                    .unwrap()
                    .strip_suffix(".a")
                    .unwrap(),
            );
        }
    }

    match triple.container_image() {
        Some((container_image, _)) => {
            println!("Using container image {container_image}");

            let container_engine =
                container_engine.unwrap_or(ContainerEngine::autodetect().ok_or(
                    CargoError::Container(ContainerError::ContainerEngineNotFound),
                )?);

            let mut llvm_prefix = OsString::from("LLVM_SYS_191_PREFIX=");
            llvm_prefix.push(&llvm_install_dir);

            let rustup_toolchain = env::var("RUSTUP_TOOLCHAIN").unwrap();
            let rustup_toolchain = rustup_toolchain.split('-').next().unwrap();
            let mut rustup_toolchain_triple = target_lexicon::HOST;
            rustup_toolchain_triple.environment = triple.environment;
            let rustup_toolchain =
                format!("{rustup_toolchain}-{}", rustup_toolchain_triple.to_string());
            let mut rustup_toolchain_arg = OsString::from("RUSTUP_TOOLCHAIN=");
            rustup_toolchain_arg.push(rustup_toolchain);

            let mut workdir_arg = workdir;
            workdir_arg.push(":/usr/local/src/bpf-linker");

            let mut llvm_arg = llvm_install_dir.clone();
            llvm_arg.push(":");
            llvm_arg.push(&llvm_install_dir);

            let mut cmd = Command::new(container_engine.to_string());
            cmd.args([
                OsStr::new("run"),
                OsStr::new("--rm"),
                OsStr::new("-e"),
                &llvm_prefix,
                OsStr::new("-e"),
                &rustflags,
                OsStr::new("-e"),
                &rustup_toolchain_arg,
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
                OsStr::new("--target"),
                OsStr::new(&triple.to_string()),
            ]);
            match verbose {
                0 => {}
                1 => {
                    cmd.arg("-v");
                }
                _ => {
                    cmd.arg("-vv");
                }
            }
            if release {
                cmd.arg("--release");
            }
            if !features.is_empty() {
                cmd.arg("--features");
                cmd.args(features);
            }
            if all_features {
                cmd.arg("--all-features");
            }
            if no_default_features {
                cmd.arg("--no-default-features");
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
