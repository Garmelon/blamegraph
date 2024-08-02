use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// A unique identifier for the blame of a single file. Can be converted to a
/// file name.Multiple commits may share a blame in certain situations.
#[derive(Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct BlameId {
    pub commit: String,
    pub blob: String,
    pub path: String,
}

impl BlameId {
    pub fn sha256(&self) -> String {
        let mut hasher = Sha256::new();

        // The commit and blob always have the same length, so I don't need any
        // sort of separator character between any of the fields to ensure that
        // different blame ids can't be confused.
        hasher.update(&self.commit);
        hasher.update(&self.blob);
        hasher.update(&self.path);

        let hash = hasher.finalize();
        format!("{hash:x}")
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct Blame {
    pub id: BlameId,
    pub lines_by_commit: HashMap<String, u64>,
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlameTree {
    pub commit: String,
    pub blames: Vec<BlameId>,
}
