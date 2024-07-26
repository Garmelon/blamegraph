mod data;

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

fn main() {
    let args = Args::parse();
    println!("{args:#?}");
}
