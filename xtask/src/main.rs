use clap::Parser;

mod build;

#[derive(Debug, Parser)]
pub struct Options {
    #[clap(subcommand)]
    command: Command,
}

#[derive(Debug, Parser)]
enum Command {
    Build(build::Options),
}

fn main() -> anyhow::Result<()> {
    let opts = Options::parse();

    match opts.command {
        Command::Build(opts) => build::build(opts)?,
    }

    Ok(())
}
