use std::collections::BTreeMap;

use jiff::{civil::DateTime, tz::TimeZone, Timestamp, ToSpan, Unit};

use crate::{
    data::{Commit, Data},
    progress,
};

pub fn first_hash(log: &[String], hash: Option<String>) -> anyhow::Result<String> {
    if let Some(hash) = hash {
        return Ok(hash);
    }

    if let Some(hash) = log.first() {
        return Ok(hash.to_string());
    }

    anyhow::bail!("found no viable hash");
}

pub fn load_commits(data: &mut Data, log: Vec<String>) -> anyhow::Result<Vec<Commit>> {
    let pb = progress::counting_bar("Loading commits", log.len());

    let mut commits = vec![];
    for hash in log {
        commits.push(data.load_commit(hash)?);
        pb.inc(1);
    }

    pb.finish();
    Ok(commits)
}

fn key(tz: &TimeZone, ts: Timestamp) -> (i16, i8) {
    let dt = tz.to_datetime(ts);
    (dt.year(), dt.month())
}

pub fn order_for_equidistance(tz: &TimeZone, commits: &mut [Commit]) {
    commits.reverse();
    commits.sort_by_cached_key(|c| key(tz, c.committer_time));
    commits.reverse();
}

pub fn make_equidistant(tz: &TimeZone, times: &mut Vec<i64>) {
    let mut time_by_key = BTreeMap::<(i16, i8), u32>::new();
    for time in times.iter() {
        let ts = Timestamp::from_second(*time).unwrap();
        *time_by_key.entry(key(tz, ts)).or_default() += 1;
    }

    times.clear();
    for ((year, month), amount) in time_by_key {
        let amount: i64 = amount.into();

        let start = DateTime::new(year, month, 1, 0, 0, 0, 0).unwrap();
        let start = tz.to_zoned(start).unwrap();
        let end = start.checked_add(1.month()).unwrap();

        let seconds_interval = start.until((Unit::Second, &end)).unwrap().get_seconds();
        let seconds_per_commit = seconds_interval / amount;
        for n in 0..amount {
            let seconds_since_start = seconds_per_commit * n + seconds_per_commit / 2;
            let time = start
                .checked_add(seconds_since_start.seconds())
                .unwrap()
                .timestamp()
                .as_second();
            times.push(time)
        }
    }
}
