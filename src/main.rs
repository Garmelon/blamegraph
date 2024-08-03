mod data;
mod gather;
mod graph;
mod progress;

use std::path::PathBuf;

use clap::{Parser, Subcommand, ValueEnum};
use data::Data;

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, ValueEnum)]
enum OutFormat {
    #[default]
    Html,
    Json,
}

#[derive(Debug, Subcommand)]
enum Command {
    Gather {
        repo: PathBuf,
    },
    Authors {
        hash: Option<String>,
        #[arg(long, short, default_value_t = false)]
        email: bool,
    },
    Years {
        hash: Option<String>,
    },
    GraphAuthors {
        outfile: PathBuf,
        #[arg(value_enum, default_value_t=Default::default())]
        format: OutFormat,
        #[arg(long, short, default_value_t = false)]
        email: bool,
    },
    GraphYears {
        outfile: PathBuf,
        #[arg(value_enum, default_value_t=Default::default())]
        format: OutFormat,
    },
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
        Command::Gather { repo } => gather::gather(&mut data, &repo)?,
        Command::Authors { hash, email } => graph::print_authors(&mut data, hash, email)?,
        Command::Years { hash } => graph::print_years(&mut data, hash)?,
        Command::GraphAuthors {
            outfile,
            format,
            email,
        } => graph::graph_authors(&mut data, &outfile, format, email)?,
        Command::GraphYears { outfile, format } => graph::graph_years(&mut data, &outfile, format)?,
    }
    Ok(())
}
