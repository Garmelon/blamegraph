mod authors;
mod data;
mod gather;

use std::{collections::HashMap, path::PathBuf};

use clap::{Parser, Subcommand};

#[derive(Debug, Subcommand)]
enum Command {
    Gather { repo: PathBuf },
    Authors { hash: Option<String> },
}

#[derive(Debug, Parser)]
struct Args {
    datafile: PathBuf,

    #[arg(long = "rename", short = 'r', num_args = 2)]
    rename: Vec<Vec<String>>,

    #[command(subcommand)]
    cmd: Command,
}

fn parse_renames(renames: Vec<Vec<String>>) -> HashMap<String, String> {
    let mut result = HashMap::new();
    for mut rename in renames {
        let to = rename.pop().unwrap();
        let from = rename.pop().unwrap();
        assert!(rename.is_empty());
        result.insert(from, to);
    }
    result
}

fn main() -> anyhow::Result<()> {
    let args = Args::parse();
    let renames = parse_renames(args.rename);

    match args.cmd {
        Command::Gather { repo } => gather::gather(&args.datafile, &repo)?,
        Command::Authors { hash } => authors::authors(&args.datafile, &renames, hash)?,
    }
    Ok(())
}
