use std::collections::HashMap;

use serde::{Deserialize, Serialize};

use super::BlameId;

#[derive(Default, Clone, Serialize, Deserialize)]
struct BlameSubtree {
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<(String, String)>,
    #[serde(default)]
    #[serde(skip_serializing_if = "HashMap::is_empty")]
    sub: HashMap<String, Box<BlameSubtree>>,
}

impl BlameSubtree {
    pub fn insert(&mut self, path: &str, commit: String, blob: String) {
        match path.split_once('/') {
            None => self.sub.entry(path.to_string()).or_default().id = Some((commit, blob)),
            Some((name, rest)) => self
                .sub
                .entry(name.to_string())
                .or_default()
                .insert(rest, commit, blob),
        }
    }

    pub fn into_blames(self, path: &str, blames: &mut Vec<BlameId>) {
        if let Some((commit, blob)) = self.id {
            blames.push(BlameId {
                commit,
                blob,
                path: path.to_string(),
            });
        }

        for (name, sub) in self.sub {
            let subpath = if path.is_empty() {
                name
            } else {
                format!("{path}/{name}")
            };

            sub.into_blames(&subpath, blames);
        }
    }
}

#[derive(Clone, Serialize, Deserialize)]
pub struct BlameTree {
    pub commit: String,
    blames: BlameSubtree,
}

impl BlameTree {
    pub fn new(commit: String, blames: Vec<BlameId>) -> Self {
        let mut tree = BlameSubtree::default();
        for blame in blames {
            tree.insert(&blame.path, blame.commit, blame.blob);
        }
        Self {
            commit,
            blames: tree,
        }
    }

    pub fn destruct(self) -> (String, Vec<BlameId>) {
        let mut blames = vec![];
        self.blames.into_blames("", &mut blames);
        (self.commit, blames)
    }
}
