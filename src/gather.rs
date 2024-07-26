use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
    str::Lines,
};

use anyhow::Context;
use jiff::{
    tz::{Offset, TimeZone},
    Timestamp, Zoned,
};

use crate::data::{Blame, Commit, CommitHash, Data};

fn stdout(output: Output) -> anyhow::Result<String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!("command exited with {}", output.status)).context(stderr)?;
    }
    let stdout = String::from_utf8(output.stdout).context("failed to decode command output")?;
    Ok(stdout)
}

fn git_rev_list(repo: &Path) -> anyhow::Result<Vec<CommitHash>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-list")
        .arg("HEAD")
        .output()?;

    let revs = stdout(output)?
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    Ok(revs)
}

fn git_ls_tree(repo: &Path, rev: &str) -> anyhow::Result<Vec<String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("ls-tree")
        .arg("-r")
        .arg("--name-only")
        .arg(rev)
        .output()?;

    let files = stdout(output)?
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    Ok(files)
}

fn parse_tz(tz: &str) -> TimeZone {
    assert!(tz.len() == 5);
    assert!(tz.starts_with('+') || tz.starts_with('-'));
    assert!(tz.chars().skip(1).all(|c| c.is_ascii_digit()));

    let sign = match &tz[..1] {
        "+" => 1,
        "-" => -1,
        _ => unreachable!(),
    };

    let hours = &tz[1..=2].parse::<i32>().unwrap();
    let mins = &tz[3..=4].parse::<i32>().unwrap();

    let seconds = sign * (hours * 60 * 60 + mins * 60);
    let offset = Offset::from_seconds(seconds).unwrap();
    TimeZone::fixed(offset)
}

fn parse_author_info(
    name: Option<&str>,
    mail: Option<&str>,
    time: Option<&str>,
    tz: Option<&str>,
) -> Option<(String, String, Zoned)> {
    let name = name?.to_string();

    let mut mail = mail?;
    if mail.starts_with('<') && mail.ends_with('>') {
        mail = mail.strip_prefix('<').unwrap().strip_suffix('>').unwrap();
    }
    let mail = mail.to_string();

    let timestamp = Timestamp::from_second(time?.parse::<i64>().unwrap()).unwrap();
    let tz = parse_tz(tz?);
    let time = Zoned::new(timestamp, tz);

    Some((name, mail, time))
}

fn parse_blame_entry(lines: &mut Lines) -> Option<(CommitHash, Option<Commit>)> {
    let first_line = lines.next()?;
    assert!(!first_line.starts_with('\t'));
    let hash = first_line.split(' ').next().unwrap().to_string();

    let mut author = None;
    let mut author_mail = None;
    let mut author_time = None;
    let mut author_tz = None;
    let mut committer = None;
    let mut committer_mail = None;
    let mut committer_time = None;
    let mut committer_tz = None;
    for line in lines.by_ref() {
        if line.starts_with("\t") {
            break;
        }
        match line.split_once(' ') {
            Some(("author", info)) => author = Some(info),
            Some(("author-mail", info)) => author_mail = Some(info),
            Some(("author-time", info)) => author_time = Some(info),
            Some(("author-tz", info)) => author_tz = Some(info),
            Some(("committer", info)) => committer = Some(info),
            Some(("committer-mail", info)) => committer_mail = Some(info),
            Some(("committer-time", info)) => committer_time = Some(info),
            Some(("committer-tz", info)) => committer_tz = Some(info),
            _ => {} // We're on interested in this header element
        }
    }

    let author = parse_author_info(author, author_mail, author_time, author_tz);
    let committer = parse_author_info(committer, committer_mail, committer_time, committer_tz);
    let commit = match (author, committer) {
        (
            Some((author, author_mail, author_time)),
            Some((committer, committer_mail, committer_time)),
        ) => Some(Commit {
            author,
            author_mail,
            author_time,
            committer,
            committer_mail,
            committer_time,
        }),
        _ => None,
    };

    Some((hash, commit))
}

fn git_blame_file(
    repo: &Path,
    rev: &str,
    file: &str,
    commits: &mut HashMap<CommitHash, Commit>,
) -> anyhow::Result<Option<HashMap<CommitHash, u64>>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("blame")
        .arg("--porcelain")
        .arg(rev)
        .arg("--")
        .arg(file)
        .output()?;

    let Ok(stdout) = stdout(output) else {
        // Very likely a binary file, just ignore it
        return Ok(None);
    };

    let mut count = HashMap::new();

    let mut lines = stdout.lines();
    while let Some((hash, commit)) = parse_blame_entry(&mut lines) {
        if let Some(commit) = commit {
            commits.entry(hash.clone()).or_insert(commit);
        }
        *count.entry(hash).or_default() += 1;
    }

    Ok(Some(count))
}

fn git_blame_commit(
    repo: &Path,
    rev: &str,
    commits: &mut HashMap<CommitHash, Commit>,
) -> anyhow::Result<Blame> {
    let mut blames = HashMap::new();

    let files = git_ls_tree(repo, rev)?;
    let total = format!("{}", files.len());
    for (i, file) in files.into_iter().enumerate() {
        println!(
            "  - ({n:width$}/{total}) {file}",
            n = i + 1,
            width = total.len()
        );

        if let Some(blame) = git_blame_file(repo, rev, &file, commits)? {
            blames.insert(file, blame);
        }
    }
    Ok(Blame(blames))
}

pub fn gather(datafile: &Path, repo: &Path) -> anyhow::Result<()> {
    let mut data = Data::load(datafile)?;

    data.log = git_rev_list(repo).context("failed to obtain rev-list")?;
    data.save(datafile)?;

    let unblamed = data
        .log
        .iter()
        .filter(|s| !data.blames.contains_key(*s))
        .cloned()
        .collect::<Vec<_>>();

    let total = format!("{}", unblamed.len());
    for (i, rev) in unblamed.into_iter().enumerate() {
        println!("({n:width$}/{total}) {rev}", n = i + 1, width = total.len());
        let blame = git_blame_commit(repo, &rev, &mut data.commits)?;
        data.blames.insert(rev, blame);
        data.save(datafile)?;
    }

    Ok(())
}
