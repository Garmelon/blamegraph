use std::collections::{HashMap, HashSet};

use serde::{Deserialize, Serialize};

#[derive(Default, Serialize, Deserialize)]
pub struct Authors(HashMap<String, String>);

impl Authors {
    pub fn new(rename: HashMap<String, String>) -> Self {
        Self(rename)
    }

    pub fn check_for_cycles(&self) -> anyhow::Result<()> {
        for start in self.0.keys() {
            let mut cur = start;
            let mut seen = HashSet::new();
            seen.insert(cur);

            while let Some(next) = self.0.get(cur) {
                if seen.contains(next) {
                    anyhow::bail!("author loop detected containing {next}");
                }

                seen.insert(next);
                cur = next;
            }
        }
        Ok(())
    }

    pub fn get(&self, name: &str) -> String {
        let mut name = name;
        while let Some(next_name) = self.0.get(name) {
            name = next_name;
        }
        name.to_string()
    }
}
