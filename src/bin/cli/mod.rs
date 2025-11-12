use std::{
    ffi::{CString, OsString},
    path::{Component, Path, PathBuf},
    str::FromStr,
};

use anyhow::Result;
use bpf_linker::{Cpu, OptLevel, OutputType};
use clap::{
    Parser,
    builder::{PathBufValueParser, TypedValueParser as _},
};
use thiserror::Error;
use tracing::Level;

#[derive(Debug, Error)]
pub(crate) enum CliError {
    #[error("optimization level needs to be between 0-3, s or z (instead was `{0}`)")]
    InvalidOptimization(String),
    #[error("unknown emission type: `{0}` - expected one of: `llvm-bc`, `asm`, `llvm-ir`, `obj`")]
    InvalidOutputType(String),
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub(crate) struct CliOptLevel(pub(crate) OptLevel);

impl FromStr for CliOptLevel {
    type Err = CliError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(match s {
            "0" => OptLevel::No,
            "1" => OptLevel::Less,
            "2" => OptLevel::Default,
            "3" => OptLevel::Aggressive,
            "s" => OptLevel::Size,
            "z" => OptLevel::SizeMin,
            _ => return Err(CliError::InvalidOptimization(s.to_string())),
        }))
    }
}

#[allow(dead_code)]
#[derive(Copy, Clone, Debug)]
pub(crate) struct CliOutputType(pub(crate) OutputType);

impl FromStr for CliOutputType {
    type Err = CliError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(Self(match s {
            "llvm-bc" => OutputType::Bitcode,
            "asm" => OutputType::Assembly,
            "llvm-ir" => OutputType::LlvmAssembly,
            "obj" => OutputType::Object,
            _ => return Err(CliError::InvalidOutputType(s.to_string())),
        }))
    }
}

pub(crate) fn parent_and_file_name(p: PathBuf) -> Result<(PathBuf, PathBuf)> {
    let mut comps = p.components();
    let file_name = comps
        .next_back()
        .map(|p| match p {
            Component::Normal(p) => Ok(p),
            p => Err(anyhow::anyhow!("unexpected path component {:?}", p)),
        })
        .transpose()?
        .ok_or_else(|| anyhow::anyhow!("unexpected empty path"))?;
    let parent = comps.as_path();
    Ok((parent.to_path_buf(), Path::new(file_name).to_path_buf()))
}

#[derive(Debug, Parser)]
#[command(version)]
pub(crate) struct CommandLine {
    /// LLVM target triple. When not provided, the target is inferred from the inputs
    #[clap(long)]
    pub(crate) target: Option<CString>,

    /// Target BPF processor. Can be one of `generic`, `probe`, `v1`, `v2`, `v3`
    #[clap(long, default_value = "generic")]
    pub(crate) cpu: Cpu,

    /// Enable or disable CPU features. The available features are: alu32, dummy, dwarfris. Use
    /// +feature to enable a feature, or -feature to disable it.  For example
    /// --cpu-features=+alu32,-dwarfris
    #[clap(long, value_name = "features", default_value = "")]
    pub(crate) cpu_features: CString,

    /// Write output to <output>
    #[clap(short, long)]
    pub(crate) output: PathBuf,

    /// Output type. Can be one of `llvm-bc`, `asm`, `llvm-ir`, `obj`
    #[clap(long, default_value = "obj")]
    pub(crate) emit: Vec<CliOutputType>,

    /// Emit BTF information
    #[clap(long)]
    pub(crate) btf: bool,

    /// Permit automatic insertion of __bpf_trap calls.
    /// See: https://github.com/llvm/llvm-project/commit/ab391beb11f733b526b86f9df23734a34657d876
    #[clap(long)]
    pub(crate) allow_bpf_trap: bool,

    /// UNUSED: it only exists for compatibility with rustc
    #[clap(short = 'L', number_of_values = 1)]
    pub(crate) _libs: Vec<PathBuf>,

    /// Optimization level. 0-3, s, or z
    #[clap(short = 'O', default_value = "2")]
    pub(crate) optimize: Vec<CliOptLevel>,

    /// Export the symbols specified in the file `path`. The symbols must be separated by new lines
    #[clap(long, value_name = "path")]
    pub(crate) export_symbols: Option<PathBuf>,

    /// Output logs to the given `path`
    #[clap(
        long,
        value_name = "path",
        value_parser = PathBufValueParser::new().try_map(parent_and_file_name),
    )]
    pub(crate) log_file: Option<(PathBuf, PathBuf)>,

    /// Set the log level. If not specified, no logging is used. Can be one of
    /// `error`, `warn`, `info`, `debug`, `trace`.
    #[clap(long, value_name = "level")]
    pub(crate) log_level: Option<Level>,

    /// Try hard to unroll loops. Useful when targeting kernels that don't support loops
    #[clap(long)]
    pub(crate) unroll_loops: bool,

    /// Ignore `noinline`/`#[inline(never)]`. Useful when targeting kernels that don't support function calls
    #[clap(long)]
    pub(crate) ignore_inline_never: bool,

    /// Dump the final IR module to the given `path` before generating the code
    #[clap(long, value_name = "path")]
    pub(crate) dump_module: Option<PathBuf>,

    /// Extra command line arguments to pass to LLVM
    #[clap(long, value_name = "args", use_value_delimiter = true, action = clap::ArgAction::Append)]
    pub(crate) llvm_args: Vec<CString>,

    /// Disable passing --bpf-expand-memcpy-in-order to LLVM.
    #[clap(long)]
    pub(crate) disable_expand_memcpy_in_order: bool,

    /// Disable exporting memcpy, memmove, memset, memcmp and bcmp. Exporting
    /// those is commonly needed when LLVM does not manage to expand memory
    /// intrinsics to a sequence of loads and stores.
    #[clap(long)]
    pub(crate) disable_memory_builtins: bool,

    /// Input files. Can be object files or static libraries
    #[clap(required = true)]
    pub(crate) inputs: Vec<PathBuf>,

    /// Comma separated list of symbols to export. See also `--export-symbols`
    #[clap(long, value_name = "symbols", use_value_delimiter = true, action = clap::ArgAction::Append)]
    pub(crate) export: Vec<String>,

    /// Whether to treat LLVM errors as fatal.
    #[clap(long, action = clap::ArgAction::Set, default_value_t = true)]
    pub(crate) fatal_errors: bool,

    // The options below are for wasm-ld compatibility
    #[clap(long = "debug", hide = true)]
    pub(crate) _debug: bool,
}

pub(crate) fn normalized_args<I>(args: I) -> Vec<OsString>
where
    I: IntoIterator<Item = OsString>,
{
    args.into_iter()
        .map(|arg| {
            if arg == "-flavor" {
                OsString::from("--flavor")
            } else {
                arg
            }
        })
        .collect()
}
