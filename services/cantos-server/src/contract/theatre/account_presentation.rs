//! Server-owned, session-bound presentation. Contains no credentials or raw actor identifier.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AccountPresentation {
    pub version: String,
    pub alias: String,
    pub avatar_seed: String,
}
