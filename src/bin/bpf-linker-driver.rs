#![expect(unused_crate_dependencies, reason = "used in lib")]

mod cli;

use std::{env, fs, io};

#[cfg(any(
    feature = "rust-llvm-19",
    feature = "rust-llvm-20",
    feature = "rust-llvm-21"
))]
use aya_rustc_llvm_proxy as _;
use bpf_linker::{Linker, LinkerInput, LinkerOptions};
use clap::{Parser, error::ErrorKind};
use cli::{CliOptLevel, CliOutputType, CommandLine, normalized_args};
use tracing::info;
use tracing_subscriber::{EnvFilter, fmt::MakeWriter, prelude::*};
use tracing_tree::HierarchicalLayer;

/// Returns a [`HierarchicalLayer`](tracing_tree::HierarchicalLayer) for the
/// given `writer`.
fn tracing_layer<W>(writer: W) -> HierarchicalLayer<W>
where
    W: for<'writer> MakeWriter<'writer> + 'static,
{
    const TRACING_IDENT: usize = 2;
    HierarchicalLayer::new(TRACING_IDENT)
        .with_indent_lines(true)
        .with_writer(writer)
}
fn main() -> anyhow::Result<()> {
    let normalized_args = normalized_args(env::args_os());
    let CommandLine {
        target,
        cpu,
        cpu_features,
        output,
        emit,
        btf,
        allow_bpf_trap,
        optimize,
        export_symbols,
        log_file,
        log_level,
        unroll_loops,
        ignore_inline_never,
        dump_module,
        llvm_args,
        disable_expand_memcpy_in_order,
        disable_memory_builtins,
        inputs,
        export,
        fatal_errors,
        _debug,
        _libs,
    } = match CommandLine::try_parse_from(normalized_args) {
        Ok(command_line) => command_line,
        Err(err) => match err.kind() {
            ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => {
                print!("{err}");
                return Ok(());
            }
            _ => return Err(err.into()),
        },
    };

    // Configure tracing.
    let _guard = {
        let filter = EnvFilter::from_default_env();
        let filter = match log_level {
            None => filter,
            Some(log_level) => filter.add_directive(log_level.into()),
        };
        let subscriber_registry = tracing_subscriber::registry().with(filter);
        match log_file {
            Some((parent, file_name)) => {
                let file_appender = tracing_appender::rolling::never(parent, file_name);
                let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
                let subscriber = subscriber_registry
                    .with(tracing_layer(io::stdout))
                    .with(tracing_layer(non_blocking));
                tracing::subscriber::set_global_default(subscriber)?;
                Some(guard)
            }
            None => {
                let subscriber = subscriber_registry.with(tracing_layer(io::stderr));
                tracing::subscriber::set_global_default(subscriber)?;
                None
            }
        }
    };

    info!(
        "command line: {:?}",
        env::args().collect::<Vec<_>>().join(" ")
    );

    let export_symbols = export_symbols.map(fs::read_to_string).transpose()?;

    let export_symbols = export_symbols
        .as_deref()
        .into_iter()
        .flat_map(str::lines)
        .chain(export.iter().map(String::as_str));

    let output_type = match *emit.as_slice() {
        [] => unreachable!("emit has a default value"),
        [CliOutputType(output_type), ..] => output_type,
    };
    let optimize = match *optimize.as_slice() {
        [] => unreachable!("emit has a default value"),
        [.., CliOptLevel(optimize)] => optimize,
    };

    let mut linker = Linker::new(LinkerOptions {
        target,
        cpu,
        cpu_features,
        optimize,
        unroll_loops,
        ignore_inline_never,
        llvm_args,
        disable_expand_memcpy_in_order,
        disable_memory_builtins,
        btf,
        allow_bpf_trap,
    });

    if let Some(path) = dump_module {
        linker.set_dump_module_path(path);
    }

    let inputs = inputs
        .iter()
        .map(|p| LinkerInput::new_from_file(p.as_path()));

    linker.link_to_file(inputs, &output, output_type, export_symbols)?;

    if fatal_errors && linker.has_errors() {
        return Err(anyhow::anyhow!(
            "LLVM issued diagnostic with error severity"
        ));
    }

    Ok(())
}

#[cfg(test)]
mod test {
    use super::*;
    use std::path::PathBuf;

    // Test made to reproduce the following bug:
    // https://github.com/aya-rs/bpf-linker/issues/27
    // where --export argument followed by positional arguments resulted in
    // parsing the positional args as `export`, not as `inputs`.
    // There can be multiple exports, but they always have to be preceded by
    // `--export` flag.
    #[test]
    fn test_export_input_args() {
        let args = [
            "bpf-linker",
            "--export",
            "foo",
            "--export",
            "bar",
            "symbols.o", // this should be parsed as `input`, not `export`
            "rcgu.o",    // this should be parsed as `input`, not `export`
            "-L",
            "target/debug/deps",
            "-L",
            "target/debug",
            "-L",
            "/home/foo/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib",
            "-o",
            "/tmp/bin.s",
            "--target=bpf",
            "--emit=asm",
        ];
        let CommandLine { inputs, export, .. } = Parser::parse_from(args);
        assert_eq!(export, ["foo", "bar"]);
        assert_eq!(
            inputs,
            [PathBuf::from("symbols.o"), PathBuf::from("rcgu.o")]
        );
    }

    #[test]
    fn test_export_delimiter() {
        let args = [
            "bpf-linker",
            "--export",
            "foo,bar",
            "--export=ayy,lmao",
            "symbols.o", // this should be parsed as `input`, not `export`
            "--export=lol",
            "--export",
            "rotfl",
            "rcgu.o", // this should be parsed as `input`, not `export`
            "-L",
            "target/debug/deps",
            "-L",
            "target/debug",
            "-L",
            "/home/foo/.rustup/toolchains/nightly-x86_64-unknown-linux-gnu/lib",
            "-o",
            "/tmp/bin.s",
            "--target=bpf",
            "--emit=asm",
        ];
        let CommandLine { inputs, export, .. } = Parser::parse_from(args);
        assert_eq!(export, ["foo", "bar", "ayy", "lmao", "lol", "rotfl"]);
        assert_eq!(
            inputs,
            [PathBuf::from("symbols.o"), PathBuf::from("rcgu.o")]
        );
    }
}
