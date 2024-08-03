mod git;

use std::{
    collections::{HashMap, HashSet},
    path::Path,
    sync::{Arc, Mutex},
};

use anyhow::Context;
use ignore::gitignore::Gitignore;
use indicatif::{MultiProgress, ProgressDrawTarget};
use rayon::iter::{ParallelBridge, ParallelIterator};

use crate::{
    data::{Blame, BlameId, BlameTree, Commit, Data},
    progress,
};

fn search_for_commits(repo: &Path) -> anyhow::Result<Vec<Commit>> {
    println!("Searching for commits");
    git::git_rev_list(repo).context("failed to obtain rev-list")
}

fn save_commits(data: &Data, commits: &[Commit]) -> anyhow::Result<()> {
    let pb = progress::counting_bar("Saving commits", commits.len());

    for commit in commits {
        data.save_commit(commit)?;
        pb.inc(1);
    }

    pb.finish();
    Ok(())
}

fn save_log(data: &Data, commits: &[Commit]) -> anyhow::Result<()> {
    println!("Saving log");
    let log = commits.iter().map(|c| c.hash.clone()).collect::<Vec<_>>();
    data.save_log(&log)?;
    Ok(())
}

/// Find the earliest commit that a blame for this file can be computed in.
///
/// Sharing blames across commits has a few subtle edge cases. Simplifying this
/// logic is probably not possible without a big performance hit.
fn find_blame_commit(
    parents: &[HashMap<(String, String), String>],
    key: &(String, String),
) -> Option<String> {
    let mut parents = parents.iter();
    let commit = parents.next()?.get(key)?;
    if parents.all(|p| p.get(key) == Some(commit)) {
        Some(commit.clone())
    } else {
        None
    }
}

fn compute_blametree(data: &mut Data, repo: &Path, commit: &Commit) -> anyhow::Result<BlameTree> {
    let mut parents = vec![];
    for hash in &commit.parents {
        let by_path_and_blob = data
            .load_blametree_cached(hash.clone())?
            .blames
            .into_iter()
            .map(|b| ((b.path, b.blob), b.commit))
            .collect::<HashMap<_, _>>();
        parents.push(by_path_and_blob);
    }

    let mut blames = vec![];

    let files = git::git_ls_tree(repo, &commit.hash)?;
    for (path, blob) in files {
        let key = (path, blob);
        let commit = find_blame_commit(&parents, &key).unwrap_or_else(|| commit.hash.clone());
        let (path, blob) = key;
        blames.push(BlameId { commit, blob, path });
    }

    Ok(BlameTree {
        commit: commit.hash.clone(),
        blames,
    })
}

fn compute_blametrees(data: &mut Data, repo: &Path, commits: &[Commit]) -> anyhow::Result<()> {
    let pb = progress::counting_bar("Computing blametrees", commits.len());

    // In topological order from parent to child, to ensure the blametrees of
    // all parents already exist when we get to a commit.
    for commit in commits.iter().rev() {
        if data.blametree_exists(commit.hash.clone()) {
            pb.inc(1);
            continue;
        }

        let blametree = compute_blametree(data, repo, commit)?;
        data.save_blametree(&blametree)?;
        pb.inc(1);
    }

    pb.finish();
    Ok(())
}

fn compute_blames_for_blametree(
    data: &Data,
    repo: &Path,
    ignore: &Gitignore,
    mp: MultiProgress,
    computed: Arc<Mutex<HashSet<BlameId>>>,
    blametree: BlameTree,
) -> anyhow::Result<()> {
    let pb = mp.add(progress::commit_blame_bar(
        &blametree.commit,
        blametree.blames.len(),
    ));

    for blame_id in blametree.blames {
        if ignore
            .matched_path_or_any_parents(&blame_id.path, false)
            .is_ignore()
        {
            continue;
        }

        if !computed.lock().unwrap().insert(blame_id.clone()) {
            pb.inc(1);
            continue;
        }

        if data.blame_exists(&blame_id) {
            pb.inc(1);
            continue;
        }

        let lines_by_commit = git::git_blame(repo, &blametree.commit, &blame_id.path)?;
        data.save_blame(&Blame {
            id: blame_id,
            lines_by_commit,
        })?;

        pb.inc(1);
    }

    pb.finish_and_clear();
    Ok(())
}

fn compute_blames(
    data: &mut Data,
    repo: &Path,
    ignore: &Gitignore,
    commits: &[Commit],
) -> anyhow::Result<()> {
    println!();
    let mp = MultiProgress::with_draw_target(ProgressDrawTarget::stdout_with_hz(5));
    mp.set_move_cursor(true);

    let pb = mp.add(progress::counting_bar("Computing blames", commits.len()));
    pb.tick();

    let computed = Arc::new(Mutex::new(HashSet::<BlameId>::new()));

    commits.iter().par_bridge().try_for_each(|commit| {
        let blametree = data.load_blametree_uncached(commit.hash.clone())?;
        compute_blames_for_blametree(data, repo, ignore, mp.clone(), computed.clone(), blametree)?;
        pb.inc(1);
        Ok::<_, anyhow::Error>(())
    })?;

    pb.finish();
    Ok(())
}

pub fn gather(data: &mut Data, repo: &Path) -> anyhow::Result<()> {
    let ignore = data.load_ignore_uncached()?;
    let commits = search_for_commits(repo)?;
    save_commits(data, &commits)?;
    save_log(data, &commits)?;
    compute_blametrees(data, repo, &commits)?;
    compute_blames(data, repo, &ignore, &commits)?;
    Ok(())
}
