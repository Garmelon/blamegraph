use std::{
    collections::{HashMap, HashSet},
    fmt, fs,
    path::Path,
};

use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::data::{Authors, Blame, Commit, Data};

#[derive(Serialize)]
struct Series {
    name: String,
    values: Vec<i64>,
}

impl Series {
    fn new(name: impl ToString) -> Self {
        Self {
            name: name.to_string(),
            values: vec![],
        }
    }

    fn push<N>(&mut self, n: N)
    where
        N: TryInto<i64>,
        N::Error: fmt::Debug,
    {
        self.values.push(n.try_into().unwrap())
    }
}

#[derive(Serialize)]
struct Graph {
    title: String,
    series: Vec<Series>,
}

impl Graph {
    fn new(title: &str, series: Vec<Series>) -> Self {
        Self {
            title: title.to_string(),
            series,
        }
    }

    fn save(&self, path: &Path) -> anyhow::Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, serde_json::to_vec(self)?)?;
        Ok(())
    }
}

fn first_hash(log: &[Commit], hash: Option<String>) -> anyhow::Result<String> {
    if let Some(hash) = hash {
        return Ok(hash);
    }

    if let Some(commit) = log.first() {
        return Ok(commit.hash.to_string());
    }

    anyhow::bail!("found no viable hash");
}

fn count_authors(
    data: &mut Data,
    authors: &Authors,
    blame: &Blame,
) -> anyhow::Result<HashMap<String, u64>> {
    let mut count = HashMap::<String, u64>::new();
    for file in blame.0.values() {
        for (hash, amount) in file {
            let info = data.load_commit(hash.clone())?;
            let author = authors.get(&info.author_mail);
            *count.entry(author).or_default() += amount;
        }
    }
    Ok(count)
}

pub fn print_authors(data: &mut Data, hash: Option<String>) -> anyhow::Result<()> {
    let log = data.load_log()?;
    let hash = first_hash(&log, hash)?;

    let blame = data
        .load_blame(&hash)
        .context(format!("found no blame for {hash}"))?;

    let authors = data.load_authors()?;

    let count = count_authors(data, &authors, &blame)?;
    let mut count = count.into_iter().map(|(a, n)| (n, a)).collect::<Vec<_>>();
    count.sort_unstable();

    for (n, a) in count {
        let n = format!("{n}");
        let space = (78 - a.width() - n.width()).max(1);
        println!("{a} {} {n}", ".".repeat(space));
    }

    Ok(())
}

pub fn graph_authors(data: &mut Data, outfile: &Path) -> anyhow::Result<()> {
    println!("Loading log and authors");
    let mut log = data.load_log()?;
    log.reverse(); // Now in chronological order
    let authors = data.load_authors()?;

    let pb = ProgressBar::new(log.len().try_into().unwrap())
        .with_style(ProgressStyle::with_template("Loading blames: {pos}/{len}").unwrap());
    let mut counts = vec![];
    for commit in log {
        let blame = data.load_blame(&commit.hash)?;
        let count = count_authors(data, &authors, &blame)?;
        counts.push((commit, count));
        pb.inc(1);
    }
    pb.finish();

    println!("Crunching numbers");
    let all_authors = counts
        .iter()
        .flat_map(|(_, count)| count.keys())
        .collect::<HashSet<_>>();

    let mut time = Series::new("Time");
    let mut by_author = all_authors
        .iter()
        .map(|author| (*author, Series::new(author)))
        .collect::<HashMap<_, _>>();

    for (commit, count) in &counts {
        time.push(commit.committer_time.as_second());
        for author in &all_authors {
            let amount = count.get(*author).copied().unwrap_or(0);
            by_author.get_mut(author).unwrap().push(amount);
        }
    }

    let total_by_author = by_author
        .iter()
        .map(|(author, series)| (*author, series.values.iter().sum::<i64>()))
        .collect::<HashMap<_, _>>();

    let mut series = by_author.into_values().collect::<Vec<_>>();
    series.sort_unstable_by_key(|s| total_by_author.get(&s.name).unwrap());
    series.reverse();
    series.insert(0, time);

    println!("Saving data");
    Graph::new("Authors over time", series).save(outfile)?;
    Ok(())
}
