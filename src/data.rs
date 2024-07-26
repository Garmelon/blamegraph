use std::{collections::HashMap, fs, io::ErrorKind, path::Path};

use jiff::Zoned;
use serde::{Deserialize, Serialize};
use tempfile::NamedTempFile;

pub type CommitHash = String;

#[derive(Default, Serialize, Deserialize)]
pub struct Commit {
    pub author: String,
    pub author_email: String,
    pub author_date: Zoned,
    pub committer: String,
    pub committer_email: String,
    pub committer_date: Zoned,
}

/// Lines of code per commit for each file.
#[derive(Serialize, Deserialize)]
pub struct Blame(pub HashMap<String, HashMap<CommitHash, u64>>);

#[derive(Default, Serialize, Deserialize)]
pub struct Data {
    /// Commits in chronological order, as reported by `git rev-list HEAD`.
    pub log: Vec<CommitHash>,

    /// Commit info, parsed from `git blame --porcelain` commands.
    pub commits: HashMap<CommitHash, Commit>,

    /// Blame info, parsed from `git blame --porcelain` commands.
    pub blames: HashMap<CommitHash, Blame>,
}

impl Data {
    pub fn load(path: &Path) -> anyhow::Result<Data> {
        Ok(match fs::read_to_string(path) {
            Ok(str) => serde_json::from_str(&str)?,
            Err(e) if e.kind() == ErrorKind::NotFound => Data::default(),
            Err(e) => Err(e)?,
        })
    }

    pub fn save(&self, path: &Path) -> anyhow::Result<()> {
        let tmp_file = NamedTempFile::new_in(path.parent().unwrap())?;
        serde_json::to_writer(&tmp_file, self)?;
        tmp_file.persist(path)?;
        Ok(())
    }
}
