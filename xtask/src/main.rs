use std::{env, ffi::OsString, os::unix::ffi::OsStringExt, path::PathBuf, process::Command};

use anyhow::{Context as _, Result, bail};
use clap::{Parser, Subcommand, ValueEnum};
use rustc_build_sysroot::{BuildMode, SysrootConfig, SysrootStatus};

#[derive(Clone, ValueEnum)]
enum Target {
    BpfebUnknownNone,
    BpfelUnknownNone,
}

impl ToString for Target {
    fn to_string(&self) -> String {
        match self {
            Self::BpfebUnknownNone => "bpfeb-unknown-none",
            Self::BpfelUnknownNone => "bpfel-unknown-none",
        }
        .to_owned()
    }
}

#[derive(Parser)]
struct BuildStd {
    #[arg(long, value_enum, default_value_t = Target::BpfelUnknownNone)]
    target: Target,
}

#[derive(Subcommand)]
enum XtaskSubcommand {
    /// Builds the Rust standard library for the given target in the current
    /// toolchain's sysroot.
    BuildStd(BuildStd),
}

/// Additional build commands for bpf-linker.
#[derive(Parser)]
struct CommandLine {
    #[command(subcommand)]
    subcommand: XtaskSubcommand,
}

fn sysroot_dir(rustc: &mut Command) -> Result<PathBuf> {
    let output = rustc
        .args(["--print", "sysroot"])
        .output()
        .context("failed to derermine rustc sysroot")?;
    if !output.status.success() {
        bail!("failed to determine rustc sysroot: {output:?}");
    }
    Ok(PathBuf::from(OsString::from_vec(
        output.stdout.trim_ascii().to_vec(),
    )))
}

fn build_bpf_sysroot(options: BuildStd) -> Result<()> {
    let BuildStd { target } = options;
    let mut rustc = Command::new(env::var_os("RUSTC").unwrap_or_else(|| OsString::from("rustc")));
    let sysroot_dir = sysroot_dir(&mut rustc)?;
    let source_dir = sysroot_dir
        .join("lib")
        .join("rustlib")
        .join("src")
        .join("rust")
        .join("library");
    let target = target.to_string();
    let rustlibdir = sysroot_dir.join("lib").join("rustlib").join(&target);
    match rustc_build_sysroot::SysrootBuilder::new(sysroot_dir.as_path(), &target)
        .build_mode(BuildMode::Build)
        .sysroot_config(SysrootConfig::NoStd)
        .rustflag("-Cdebuginfo=2")
        .build_from_source(&source_dir)?
    {
        SysrootStatus::AlreadyCached => println!(
            "Standard library for target {target} was already built: {}",
            rustlibdir.display()
        ),
        SysrootStatus::SysrootBuilt => println!(
            "Standard library for target {target} built successfully: {}",
            rustlibdir.display()
        ),
    }
    Ok(())
}

fn main() -> Result<()> {
    let CommandLine { subcommand } = Parser::parse();
    match subcommand {
        XtaskSubcommand::BuildStd(options) => build_bpf_sysroot(options),
    }
}
