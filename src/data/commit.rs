use jiff::Zoned;
use serde::{Deserialize, Serialize};

#[derive(Clone, Default, Serialize, Deserialize)]
pub struct Commit {
    pub author: String,
    pub author_mail: String,
    pub author_time: Zoned,
    pub committer: String,
    pub committer_mail: String,
    pub committer_time: Zoned,
}
