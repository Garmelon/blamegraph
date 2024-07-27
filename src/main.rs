mod data;
mod gather;
mod graph;
mod years;

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
    },
    Years {
        hash: Option<String>,
    },
    GraphAuthors {
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
        Command::Gather { repo } => gather::gather(&data, &repo)?,
        Command::Authors { hash } => graph::print_authors(&mut data, hash)?,
        Command::Years { hash } => years::years(&mut data, hash)?,
        Command::GraphAuthors { outfile, format } => {
            graph::graph_authors(&mut data, &outfile, format)?
        }
    }
    Ok(())
}
