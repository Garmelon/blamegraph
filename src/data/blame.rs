use std::collections::HashMap;

use serde::{Deserialize, Serialize};

/// Map from file name to map from commit hash to amount of lines in said file originating from said commit.
#[derive(Serialize, Deserialize)]
pub struct Blame(pub HashMap<String, HashMap<String, u64>>);
