//! Raw Studio Content Identity v1 DTOs. These are not integrity or permission witnesses.
use serde::{Deserialize, Serialize};

use super::{CatalogScriptRef, CatalogSourceRef};

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
pub enum ContentBody {
    Recording {},
    Text {
        text: String,
        source: Option<CatalogSourceRef>,
    },
    Source {
        source: CatalogSourceRef,
    },
    Script {
        script: CatalogScriptRef,
        source: Option<CatalogSourceRef>,
    },
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentDetails {
    pub language: String,
    pub body: ContentBody,
    pub rights_refs: Vec<String>,
    /// Caller-declared authorship/import/adaptation/recording provenance; not verification.
    pub provenance_note: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppendContentRequest {
    pub operation_id: String,
    pub id: String,
    /// Content sequence, independent of the unit's metadata revision. Zero means no content.
    pub expected_revision: u64,
    pub expected_work_revision: u64,
    pub details: ContentDetails,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentRevisionResponse {
    pub id: String,
    pub work_id: String,
    pub unit_id: String,
    pub revision: u64,
    pub details: ContentDetails,
    pub content_digest: String,
    pub revision_digest: String,
    pub recorded_by: String,
    pub recorded_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AppendContentResponse {
    pub content: ContentRevisionResponse,
    pub work_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentRevisionSummary {
    pub id: String,
    pub revision: u64,
    pub content_digest: String,
    pub revision_digest: String,
    pub recorded_by: String,
    pub recorded_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContentHistoryResponse {
    pub revisions: Vec<ContentRevisionSummary>,
    pub next_after: Option<u64>,
}
