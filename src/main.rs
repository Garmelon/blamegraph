mod authors;
mod data;
mod gather;
mod years;

use std::path::PathBuf;

use clap::{Parser, Subcommand};
use data::Data;

#[derive(Debug, Subcommand)]
enum Command {
    Gather { repo: PathBuf },
    Authors { hash: Option<String> },
    Years { hash: Option<String> },
}

#[derive(Debug, Parser)]
struct Args {
    datadir: PathBuf,

    #[command(subcommand)]
    cmd: Command,
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let mut data = Data::new(args.datadir);

    match args.cmd {
        Command::Gather { repo } => gather::gather(&data, &repo)?,
        Command::Authors { hash } => authors::authors(&mut data, hash)?,
        Command::Years { hash } => years::years(&mut data, hash)?,
    }
    Ok(())
}
