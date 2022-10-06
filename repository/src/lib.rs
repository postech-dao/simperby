pub mod raw;

use serde::{Deserialize, Serialize};

pub type Branch = String;
pub type Tag = String;

#[derive(Debug, PartialEq, Eq, PartialOrd, Ord, Clone, Serialize, Deserialize, Hash)]
pub struct CommitHash {
    pub hash: [u8; 32],
}
