//! Studio Web's own copy of the Studio preview representation (ADR 0054 D03, #650). The Server
//! owns `contracts/studio/preview/v1/`; the conformance test keeps this copy honest.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviewWork {
    pub id: String,
    pub kind: String,
    pub revision: u64,
    pub title: String,
    pub synopsis: String,
    pub language: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviewUnit {
    pub id: String,
    pub kind: String,
    pub revision: u64,
    pub ordinal: u32,
    pub title: String,
    pub description: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct StudioPreview {
    pub work: PreviewWork,
    pub units: Vec<PreviewUnit>,
    pub next_cursor: Option<String>,
}
