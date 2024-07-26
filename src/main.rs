mod data;
mod gather;

use std::path::PathBuf;

use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
enum Command {
    Gather { repo: PathBuf },
}

#[derive(Debug, Parser)]
struct Args {
    datafile: PathBuf,
    #[command(subcommand)]
    cmd: Command,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    match args.cmd {
        Command::Gather { repo } => gather::gather(&args.datafile, &repo)?,
    }
    Ok(())
}
