use std::{
    collections::HashMap,
    path::Path,
    process::{Command, Output},
    str::Lines,
};

use anyhow::Context;
use jiff::Timestamp;

use crate::data::Commit;

fn stdout(output: Output) -> anyhow::Result<String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!("command exited with {}", output.status)).context(stderr)?;
    }
    let stdout = String::from_utf8(output.stdout).context("failed to decode command output")?;
    Ok(stdout)
}

fn stdout_lossy(output: Output) -> anyhow::Result<String> {
    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr).to_string();
        Err(anyhow::anyhow!("command exited with {}", output.status)).context(stderr)?;
    }
    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    Ok(stdout)
}

fn parse_rev_list_entry(lines: &mut Lines) -> Option<Commit> {
    Some(Commit {
        hash: lines.next()?.to_string(),
        parents: lines
            .next()?
            .split(" ")
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string())
            .collect::<Vec<_>>(),
        author: lines.next()?.to_string(),
        author_mail: lines.next()?.to_string(),
        author_time: lines.next()?.parse::<Timestamp>().unwrap(),
        committer: lines.next()?.to_string(),
        committer_mail: lines.next()?.to_string(),
        committer_time: lines.next()?.parse::<Timestamp>().unwrap(),
        subject: lines.next()?.to_string(),
    })
}

pub fn git_rev_list(repo: &Path) -> anyhow::Result<Vec<Commit>> {
    // List commits in topological order, starting from the HEAD and proceeding
    // towards older and older commits.
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("rev-list")
        .arg("--topo-order")
        .arg("--no-commit-header")
        .arg("--format=tformat:%H%n%P%n%an%n%ae%n%aI%n%cn%n%ce%n%cI%n%s")
        .arg("HEAD")
        .output()?;

    let mut result = vec![];

    let stdout = stdout_lossy(output)?;
    let mut lines = stdout.lines();
    while let Some(info) = parse_rev_list_entry(&mut lines) {
        result.push(info);
    }

    Ok(result)
}

pub fn git_ls_tree(repo: &Path, hash: &str) -> anyhow::Result<HashMap<String, String>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("ls-tree")
        .arg("-r")
        .arg("--format=%(objectname) %(path)")
        .arg(hash)
        .output()?;

    let files = stdout(output)?
        .lines()
        .map(|s| {
            let (blob, path) = s.split_once(' ').unwrap();
            (path.to_string(), blob.to_string())
        })
        .collect::<HashMap<_, _>>();

    Ok(files)
}

fn parse_blame_entry(lines: &mut Lines) -> Option<String> {
    let first_line = lines.next()?;
    assert!(!first_line.starts_with('\t'));

    let hash = first_line.split(' ').next().unwrap().to_string();
    assert!(hash.len() == 40);

    // Skip remaining header lines and the line from the file
    for line in lines.by_ref() {
        if line.starts_with('\t') {
            break;
        }
    }

    Some(hash)
}

pub fn git_blame(repo: &Path, hash: &str, path: &str) -> anyhow::Result<HashMap<String, u64>> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repo)
        .arg("blame")
        .arg("--porcelain")
        .arg(hash)
        .arg("--")
        .arg(path)
        .output()?;

    let Ok(stdout) = stdout(output) else {
        // Very likely a binary file
        return Ok(HashMap::new());
    };

    let mut count: HashMap<String, u64> = HashMap::new();

    let mut lines = stdout.lines();
    while let Some(hash) = parse_blame_entry(&mut lines) {
        *count.entry(hash).or_default() += 1;
    }

    Ok(count)
}
