//! Raw Studio preview of one exact saved Work revision (ADR 0054 D03, #650).
//! Canonical schema and fixtures: `contracts/studio/preview/v1/`.
//!
//! An allowlist, not a Catalog snapshot: owner, visibility, rights references, links,
//! lifecycle, positions, digests and timestamps never appear, and v1 names no cover or audio.
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PreviewWork {
    pub id: String,
    pub kind: String,
    /// The exact saved Work revision this preview shows.
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
    /// The Unit revision saved at the previewed Work revision.
    pub revision: u64,
    /// One-based rank among the active Units, counted across pages.
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
