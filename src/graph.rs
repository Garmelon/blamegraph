use std::{
    collections::{BTreeMap, HashMap, HashSet},
    fmt, fs,
    path::Path,
};

use anyhow::Context;
use indicatif::{ProgressBar, ProgressStyle};
use jiff::{
    civil::{Date, Time},
    tz::TimeZone,
    Timestamp, ToSpan,
};
use serde::Serialize;
use unicode_width::UnicodeWidthStr;

use crate::{
    data::{Authors, Blame, Commit, Data},
    OutFormat,
};

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
    commits: Vec<Commit>,
    time: Vec<i64>,
    series: Vec<Series>,
}

impl Graph {
    fn new(
        title: &str,
        mut commits: Vec<Commit>,
        mut time: Vec<i64>,
        mut series: Vec<Series>,
    ) -> Self {
        commits.reverse();
        time.reverse();
        for series in &mut series {
            series.values.reverse();
        }

        Self {
            title: title.to_string(),
            commits,
            time,
            series,
        }
    }

    fn make_day_equidistant(&mut self, tz: TimeZone) {
        let seconds_per_day = 24 * 60 * 60;

        let is_sorted = self
            .time
            .iter()
            .zip(self.time.iter().skip(1))
            .all(|(a, b)| a <= b);
        assert!(is_sorted, "time must be monotonically increasing");

        let mut commits_by_date = BTreeMap::<Date, u32>::new();
        for time in &self.time {
            let time = Timestamp::from_second(*time).unwrap();
            let date = tz.to_datetime(time).date();
            *commits_by_date.entry(date).or_default() += 1;
        }

        self.time.clear();
        for (date, amount) in commits_by_date {
            let amount: i64 = amount.into();
            let seconds_per_commit = seconds_per_day / amount;
            for n in 0..amount {
                let seconds = seconds_per_commit * n + seconds_per_commit / 2;
                let time = Time::midnight() + seconds.seconds();
                let time = tz.to_timestamp(date.to_datetime(time)).unwrap().as_second();
                self.time.push(time)
            }
        }
    }

    fn save_json(&self, path: &Path) -> anyhow::Result<()> {
        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, serde_json::to_vec(self)?)?;
        Ok(())
    }

    fn save_html(&self, path: &Path) -> anyhow::Result<()> {
        const UPLOT_CSS: &str = include_str!("../static/uPlot.css");
        const UPLOT_JS: &str = include_str!("../static/uPlot.js");
        const UPLOT_STACK_JS: &str = include_str!("../static/uPlot_stack.js");
        const GRAPH_TEMPLATE: &str = include_str!("../static/graph_template.html");

        let data = serde_json::to_string(self)?;
        let html = GRAPH_TEMPLATE
            .replace("/* replace with uplot css */", UPLOT_CSS)
            .replace("/* replace with uplot js */", UPLOT_JS)
            .replace("/* replace with uplot stack js */", UPLOT_STACK_JS)
            .replace("$replace_with_data$", &data);

        fs::create_dir_all(path.parent().unwrap())?;
        fs::write(path, html)?;
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

///////////////
// By author //
///////////////

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

pub fn graph_authors(data: &mut Data, outfile: &Path, format: OutFormat) -> anyhow::Result<()> {
    println!("Loading log and authors");
    let log = data.load_log()?;
    let tz = TimeZone::system();
    let authors = data.load_authors()?;

    let pb = ProgressBar::new(log.len().try_into().unwrap())
        .with_style(ProgressStyle::with_template("Loading blames: {pos}/{len}").unwrap());
    let mut counts = vec![];
    for commit in log {
        let Ok(blame) = data.load_blame(&commit.hash) else {
            break;
        };
        let count = count_authors(data, &authors, &blame)?;
        counts.push((commit, count));
        pb.inc(1);
    }
    pb.set_length(pb.position());
    pb.finish();

    println!("Crunching numbers");
    let all_authors = counts
        .iter()
        .flat_map(|(_, count)| count.keys().cloned())
        .collect::<HashSet<_>>();

    let mut commits = vec![];
    let mut time = vec![];
    let mut by_author = all_authors
        .iter()
        .map(|author| (author, Series::new(author)))
        .collect::<HashMap<_, _>>();

    for (commit, count) in counts {
        for author in &all_authors {
            let amount = count.get(author).copied().unwrap_or(0);
            by_author.get_mut(author).unwrap().push(amount);
        }
        time.push(commit.committer_time.as_second());
        commits.push(commit);
    }

    let total_by_author = by_author
        .iter()
        .map(|(author, series)| (*author, series.values.iter().sum::<i64>()))
        .collect::<HashMap<_, _>>();

    let mut series = by_author.into_values().collect::<Vec<_>>();
    series.sort_unstable_by_key(|s| total_by_author.get(&s.name).unwrap());
    series.reverse();

    println!("Saving data");
    let mut graph = Graph::new("Lines per author", commits, time, series);
    graph.make_day_equidistant(tz);
    match format {
        OutFormat::Html => graph.save_html(outfile)?,
        OutFormat::Json => graph.save_json(outfile)?,
    }
    Ok(())
}

/////////////
// By year //
/////////////

fn count_years(data: &mut Data, tz: &TimeZone, blame: &Blame) -> anyhow::Result<HashMap<i16, u64>> {
    let mut count = HashMap::<i16, u64>::new();
    for file in blame.0.values() {
        for (hash, amount) in file {
            let info = data.load_commit(hash.clone())?;
            let year = tz.to_datetime(info.author_time).year();
            *count.entry(year).or_default() += amount;
        }
    }
    Ok(count)
}

pub fn print_years(data: &mut Data, hash: Option<String>) -> anyhow::Result<()> {
    let log = data.load_log()?;
    let hash = first_hash(&log, hash)?;

    let blame = data
        .load_blame(&hash)
        .context(format!("found no blame for {hash}"))?;

    let tz = TimeZone::system();

    let count = count_years(data, &tz, &blame)?;
    let mut count = count.into_iter().collect::<Vec<_>>();
    count.sort_unstable();

    for (y, n) in count {
        let n = format!("{n}");
        let y = format!("{y:4}");
        let space = (18 - y.width() - n.width()).max(1);
        println!("{y} {} {n}", ".".repeat(space));
    }

    Ok(())
}

pub fn graph_years(data: &mut Data, outfile: &Path, format: OutFormat) -> anyhow::Result<()> {
    println!("Loading log and authors");
    let log = data.load_log()?;
    let tz = TimeZone::system();

    let pb = ProgressBar::new(log.len().try_into().unwrap())
        .with_style(ProgressStyle::with_template("Loading blames: {pos}/{len}").unwrap());
    let mut counts = vec![];
    for commit in log {
        let Ok(blame) = data.load_blame(&commit.hash) else {
            break;
        };
        let count = count_years(data, &tz, &blame)?;
        counts.push((commit, count));
        pb.inc(1);
    }
    pb.set_length(pb.position());
    pb.finish();

    println!("Crunching numbers");
    let all_years = counts
        .iter()
        .flat_map(|(_, count)| count.keys().copied())
        .collect::<HashSet<_>>();

    let min_year = *all_years.iter().min().unwrap();
    let max_year = *all_years.iter().max().unwrap();

    let mut commits = vec![];
    let mut time = vec![];
    let mut by_year = (min_year..=max_year)
        .map(|year| (year, Series::new(year)))
        .collect::<HashMap<_, _>>();

    for (commit, count) in counts {
        for year in min_year..=max_year {
            let amount = count.get(&year).copied().unwrap_or(0);
            by_year.get_mut(&year).unwrap().push(amount);
        }
        time.push(commit.committer_time.as_second());
        commits.push(commit)
    }

    let mut series = by_year.into_iter().collect::<Vec<_>>();
    series.sort_unstable_by_key(|(year, _)| *year);
    let series = series
        .into_iter()
        .map(|(_, series)| series)
        .collect::<Vec<_>>();

    println!("Saving data");
    let mut graph = Graph::new("Lines per year", commits, time, series);
    graph.make_day_equidistant(tz);
    match format {
        OutFormat::Html => graph.save_html(outfile)?,
        OutFormat::Json => graph.save_json(outfile)?,
    }
    Ok(())
}
