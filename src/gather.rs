use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
    str::Lines,
};

use anyhow::Context;
use indicatif::{MultiProgress, ProgressBar, ProgressDrawTarget, ProgressStyle};
use jiff::Timestamp;
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::data::{Blame, Commit, Data};

fn stdout(output: Output) -> anyhow::Result<String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!("command exited with {}", output.status)).context(stderr)?;
    }
    let stdout = String::from_utf8(output.stdout).context("failed to decode command output")?;
    Ok(stdout)
}

fn parse_rev_list_entry(lines: &mut Lines) -> Option<Commit> {
    Some(Commit {
        hash: lines.next()?.to_string(),
        author: lines.next()?.to_string(),
        author_mail: lines.next()?.to_string(),
        author_time: lines.next()?.parse::<Timestamp>().unwrap(),
        committer: lines.next()?.to_string(),
        committer_mail: lines.next()?.to_string(),
        committer_time: lines.next()?.parse::<Timestamp>().unwrap(),
        subject: lines.next()?.to_string(),
    })
}

fn git_rev_list(repo: &Path) -> anyhow::Result<Vec<Commit>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-list")
        .arg("--no-commit-header")
        .arg("--format=tformat:%H%n%an%n%ae%n%aI%n%cn%n%ce%n%cI%n%s")
        .arg("HEAD")
        .output()?;

    let mut result = vec![];

    let stdout = stdout(output)?;
    let mut lines = stdout.lines();
    while let Some(info) = parse_rev_list_entry(&mut lines) {
        result.push(info);
    }

    Ok(result)
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

fn parse_author_info(
    name: Option<&str>,
    mail: Option<&str>,
    time: Option<&str>,
) -> Option<(String, String, Timestamp)> {
    let name = name?.to_string();

    let mut mail = mail?;
    if mail.starts_with('<') && mail.ends_with('>') {
        mail = mail.strip_prefix('<').unwrap().strip_suffix('>').unwrap();
    }
    let mail = mail.to_string();

    let time = Timestamp::from_second(time?.parse::<i64>().unwrap()).unwrap();

    Some((name, mail, time))
}

fn parse_blame_entry(lines: &mut Lines) -> Option<(String, Option<Commit>)> {
    let first_line = lines.next()?;
    assert!(!first_line.starts_with('\t'));
    let hash = first_line.split(' ').next().unwrap().to_string();

    let mut author = None;
    let mut author_mail = None;
    let mut author_time = None;
    let mut committer = None;
    let mut committer_mail = None;
    let mut committer_time = None;
    let mut summary = None;
    for line in lines.by_ref() {
        if line.starts_with("\t") {
            break;
        }
        match line.split_once(' ') {
            Some(("author", info)) => author = Some(info),
            Some(("author-mail", info)) => author_mail = Some(info),
            Some(("author-time", info)) => author_time = Some(info),
            Some(("committer", info)) => committer = Some(info),
            Some(("committer-mail", info)) => committer_mail = Some(info),
            Some(("committer-time", info)) => committer_time = Some(info),
            Some(("summary", info)) => summary = Some(info),
            _ => {} // We're on interested in this header element
        }
    }

    let author = parse_author_info(author, author_mail, author_time);
    let committer = parse_author_info(committer, committer_mail, committer_time);
    let commit = match (author, committer, summary) {
        (
            Some((author, author_mail, author_time)),
            Some((committer, committer_mail, committer_time)),
            Some(summary),
        ) => Some(Commit {
            hash: hash.clone(),
            author,
            author_mail,
            author_time,
            committer,
            committer_mail,
            committer_time,
            subject: summary.to_string(),
        }),
        _ => None,
    };

    Some((hash, commit))
}

fn git_blame_file(
    data: &Data,
    repo: &Path,
    hash: &str,
    file: &str,
) -> anyhow::Result<Option<HashMap<String, u64>>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("blame")
        .arg("--porcelain")
        .arg(hash)
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
            data.save_commit(&hash, &commit)?;
        }
        *count.entry(hash).or_default() += 1;
    }

    Ok(Some(count))
}

fn git_blame_commit(
    data: &Data,
    pb: &ProgressBar,
    repo: &Path,
    hash: &str,
) -> anyhow::Result<Blame> {
    let mut blames = HashMap::new();

    let files = git_ls_tree(repo, hash)?;
    pb.set_length(files.len().try_into().unwrap());

    for file in files {
        pb.inc(1);
        if let Some(blame) = git_blame_file(data, repo, hash, &file)? {
            blames.insert(file, blame);
        }
    }

    Ok(Blame(blames))
}

pub fn gather(data: &Data, repo: &Path) -> anyhow::Result<()> {
    println!("Searching for commits to blame");
    let log = git_rev_list(repo).context("failed to obtain rev-list")?;
    data.save_log(&log)?;

    let known_blames = data.load_known_blames()?;
    let unblamed = log
        .iter()
        .map(|c| &c.hash)
        .filter(|h| !known_blames.contains(*h))
        .cloned()
        .collect::<Vec<_>>();

    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::stdout_with_hz(5));
    mp.set_move_cursor(true);
    let pb = ProgressBar::new(log.len().try_into().unwrap())
        .with_position((log.len() - unblamed.len()).try_into().unwrap())
        .with_style(ProgressStyle::with_template("Blaming commits: {pos}/{len}").unwrap());
    let pb = mp.add(pb);
    pb.tick();

    unblamed.iter().par_bridge().try_for_each(|hash| {
        let bpb = ProgressBar::new(0)
            .with_style(ProgressStyle::with_template("{msg:40} {bar:36} {percent:>3}%").unwrap())
            .with_message(hash.to_string());
        let bpb = mp.add(bpb);

        let result: Result<(), anyhow::Error> = match git_blame_commit(data, &bpb, repo, hash) {
            Ok(blame) => data.save_blame(hash, &blame),
            Err(e) => Err(e),
        };

        pb.inc(1);
        bpb.finish_and_clear();

        result
    })?;

    pb.finish();

    Ok(())
}
