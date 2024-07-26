use std::{
    path::Path,
    process::{Command, Output},
};

use anyhow::Context;

use crate::data::{CommitHash, Data};

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

    let lines = stdout(output)?
        .lines()
        .map(|s| s.to_string())
        .collect::<Vec<_>>();

    Ok(lines)
}

pub fn gather(datafile: &Path, repo: &Path) -> anyhow::Result<()> {
    let mut data = Data::load(datafile)?;

    data.log = git_rev_list(repo).context("failed to obtain rev-list")?;
    data.save(datafile)?;

    let n_commits = data.log.len();
    let n_commits_unknown = data
        .log
        .iter()
        .filter(|s| !data.blames.contains_key(*s))
        .count();
    println!("Found {n_commits} commits, {n_commits_unknown} without blame info");

    todo!()
}
