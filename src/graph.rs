mod common;
#[allow(clippy::module_inception)]
mod graph;
mod series;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
};

use graph::Graph;
use ignore::gitignore::Gitignore;
use jiff::tz::TimeZone;
use lru::LruCache;
use series::Series;
use unicode_width::UnicodeWidthStr;

use crate::{
    data::{Authors, BlameId, BlameTree, Data},
    progress, OutFormat,
};

///////////////
// By author //
///////////////

fn count_authors(
    data: &mut Data,
    count_cache: &mut LruCache<BlameId, HashMap<String, u64>>,
    ignore: &Gitignore,
    authors: &Authors,
    blametree: BlameTree,
    use_email: bool,
) -> anyhow::Result<HashMap<String, u64>> {
    let mut count = HashMap::<String, u64>::new();
    for blame_id in blametree.blames {
        if ignore
            .matched_path_or_any_parents(&blame_id.path, false)
            .is_ignore()
        {
            continue;
        }

        if let Some(cached_count) = count_cache.get(&blame_id) {
            for (author, amount) in cached_count {
                *count.entry(author.clone()).or_default() += amount;
            }
            continue;
        }

        let mut cached_count = HashMap::new();
        let blame = data.load_blame_cached(&blame_id)?;
        for (hash, amount) in blame.lines_by_commit {
            let info = data.load_commit_cached(hash.clone())?;
            let author = if use_email {
                &info.author_mail
            } else {
                &info.author
            };
            let author = authors.get(author);
            *count.entry(author.clone()).or_default() += amount;
            *cached_count.entry(author).or_default() += amount;
        }
        count_cache.put(blame_id, cached_count);
    }
    Ok(count)
}

pub fn print_authors(data: &mut Data, hash: Option<String>, use_email: bool) -> anyhow::Result<()> {
    let log = data.load_log_uncached()?;
    let hash = common::first_hash(&log, hash)?;
    let blametree = data.load_blametree_cached(hash)?;
    let ignore = data.load_ignore_uncached()?;
    let authors = data.load_authors_uncached()?;

    let mut cache = LruCache::new(10000.try_into().unwrap());
    let count = count_authors(data, &mut cache, &ignore, &authors, blametree, use_email)?;
    let mut count = count.into_iter().map(|(a, n)| (n, a)).collect::<Vec<_>>();
    count.sort_unstable();

    for (n, a) in count {
        let n = format!("{n}");
        let space = (78 - a.width() - n.width()).max(1);
        println!("{a} {} {n}", ".".repeat(space));
    }

    Ok(())
}

pub fn graph_authors(
    data: &mut Data,
    outfile: &Path,
    format: OutFormat,
    use_email: bool,
) -> anyhow::Result<()> {
    println!("Loading basic info");
    let log = data.load_log_uncached()?;
    let ignore = data.load_ignore_uncached()?;
    let authors = data.load_authors_uncached()?;
    let tz = TimeZone::system();

    let mut commits = common::load_commits(data, log)?;
    common::order_for_equidistance(&tz, &mut commits);

    let pb = progress::counting_bar("Loading blames", commits.len());
    let mut cache = LruCache::new(10000.try_into().unwrap());
    let mut counts = vec![];
    for commit in commits {
        let blametree = data.load_blametree_cached(commit.hash.clone())?;
        let Ok(count) = count_authors(data, &mut cache, &ignore, &authors, blametree, use_email)
        else {
            break;
        };
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

    if series.len() > 50 {
        let mut misc_authors = series.pop().unwrap();
        misc_authors.name = "Misc authors".to_string();
        while series.len() > 49 {
            misc_authors.add(&series.pop().unwrap());
        }
        series.push(misc_authors);
    }

    println!("Saving data");
    let mut graph = Graph::new("Lines per author", commits, time, series);
    graph.make_equidistant(tz);
    match format {
        OutFormat::Html => graph.save_html(outfile)?,
        OutFormat::Json => graph.save_json(outfile)?,
    }
    Ok(())
}

/////////////
// By year //
/////////////

fn count_years(
    data: &mut Data,
    count_cache: &mut LruCache<BlameId, HashMap<i16, u64>>,
    ignore: &Gitignore,
    tz: &TimeZone,
    blametree: BlameTree,
) -> anyhow::Result<HashMap<i16, u64>> {
    let mut count = HashMap::<i16, u64>::new();
    for blame_id in blametree.blames {
        if ignore
            .matched_path_or_any_parents(&blame_id.path, false)
            .is_ignore()
        {
            continue;
        }

        if let Some(cached_count) = count_cache.get(&blame_id) {
            for (year, amount) in cached_count {
                *count.entry(*year).or_default() += amount;
            }
            continue;
        }

        let mut cached_count = HashMap::new();
        let blame = data.load_blame_cached(&blame_id)?;
        for (hash, amount) in blame.lines_by_commit {
            let info = data.load_commit_cached(hash.clone())?;
            let year = tz.to_datetime(info.author_time).year();
            *count.entry(year).or_default() += amount;
            *cached_count.entry(year).or_default() += amount;
        }
        count_cache.put(blame_id, cached_count);
    }
    Ok(count)
}

pub fn print_years(data: &mut Data, hash: Option<String>) -> anyhow::Result<()> {
    let log = data.load_log_uncached()?;
    let hash = common::first_hash(&log, hash)?;
    let blametree = data.load_blametree_cached(hash)?;
    let ignore = data.load_ignore_uncached()?;
    let tz = TimeZone::system();

    let mut cache = LruCache::new(10000.try_into().unwrap());
    let count = count_years(data, &mut cache, &ignore, &tz, blametree)?;
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
    println!("Loading basic info");
    let log = data.load_log_uncached()?;
    let ignore = data.load_ignore_uncached()?;
    let tz = TimeZone::system();

    let mut commits = common::load_commits(data, log)?;
    common::order_for_equidistance(&tz, &mut commits);

    let pb = progress::counting_bar("Loading blames", commits.len());
    let mut cache = LruCache::new(10000.try_into().unwrap());
    let mut counts = vec![];
    for commit in commits {
        let blametree = data.load_blametree_cached(commit.hash.clone())?;
        let Ok(count) = count_years(data, &mut cache, &ignore, &tz, blametree) else {
            break;
        };
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
    graph.make_equidistant(tz);
    match format {
        OutFormat::Html => graph.save_html(outfile)?,
        OutFormat::Json => graph.save_json(outfile)?,
    }
    Ok(())
}
