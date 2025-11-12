#![expect(unused_crate_dependencies, reason = "used in lib")]

mod cli;

use std::{
    env,
    ffi::OsString,
    fs,
    path::PathBuf,
    process::{Command, ExitStatus},
};

use anyhow::{Context, Result};
use bpf_linker::{LlvmVersionDetectionError, bitcode_llvm_version};
use clap::{Parser, error::ErrorKind};
use cli::{CommandLine, normalized_args};

const DEFAULT_LLVM_MAJOR: u32 = 21;
const VERSIONED_BINARIES: &[(u32, &str)] = &[
    (19, "bpf-linker-19"),
    (20, "bpf-linker-20"),
    (21, "bpf-linker-21"),
];

fn main() -> Result<()> {
    match proxy_main()? {
        Some(status) if status.success() => Ok(()),
        Some(status) => {
            std::process::exit(status.code().unwrap_or(1));
        }
        None => Ok(()),
    }
}

fn proxy_main() -> Result<Option<ExitStatus>> {
    let raw_args: Vec<OsString> = env::args_os().collect();
    let normalized_args = normalized_args(raw_args.iter().cloned());

    let CommandLine { inputs, .. } = match CommandLine::try_parse_from(normalized_args) {
        Ok(command_line) => command_line,
        Err(err) => match err.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                print!("{err}");
                return Ok(None);
            }
            _ => return Err(err.into()),
        },
    };

    let selected_major = match detect_llvm_major(&inputs)? {
        Some(major) => major,
        None => DEFAULT_LLVM_MAJOR,
    };

    let binary = binary_for_major(selected_major)
        .with_context(|| format!("unsupported LLVM major version: {selected_major}"))?;

    let status = Command::new(binary)
        .args(raw_args.iter().skip(1))
        .status()
        .with_context(|| format!("failed to invoke {binary}"))?;

    Ok(Some(status))
}

fn detect_llvm_major(inputs: &[PathBuf]) -> Result<Option<u32>> {
    for path in inputs {
        let data =
            fs::read(path).with_context(|| format!("failed to read input `{}`", path.display()))?;

        match bitcode_llvm_version(&data) {
            Ok((major, _)) => return Ok(Some(major)),
            Err(LlvmVersionDetectionError::Bitcode(_)) => continue,
            Err(other) => {
                return Err(anyhow::Error::new(other).context(format!(
                    "failed to parse LLVM bitcode version from `{}`",
                    path.display()
                )));
            }
        }
    }

    Ok(None)
}

fn binary_for_major(major: u32) -> Option<&'static str> {
    VERSIONED_BINARIES
        .iter()
        .find_map(|(supported, name)| (*supported == major).then_some(*name))
}
