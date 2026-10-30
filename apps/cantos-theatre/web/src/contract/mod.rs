//! Theatre Web's own Rust representation of the Server contracts it consumes: Theatre
//! Discovery/Playback and Library (`/api/v1/theatre`), Identity auth v1 signup and the shared
//! JSON error envelope. The Server owns the canonical schemas and fixtures under
//! `services/cantos-server/contracts/`; this client keeps only what it uses and checks it
//! against those fixtures in its own tests. Raw DTOs, never authority or validation witnesses.
use serde::{Deserialize, Serialize};

pub mod account_presentation;
pub mod discovery;
pub mod identity_auth;
pub mod library;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    Unauthenticated,
    NotFound,
    Forbidden,
    InvalidRequest,
    InvalidScript,
    EvidenceUnavailable,
    StaleRevision,
    OperationReused,
    ProposalAlreadySubmitted,
    Unavailable,
    CorruptRevision,
    StaleProductionInputs,
    ProductionBlocked,
    InvalidCatalog,
    CatalogConflict,
    StaleCursor,
    AudioRejected,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct FieldIssue {
    pub path: String,
    pub rule: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ApiError {
    pub code: ErrorCode,
    pub current_revision: Option<u64>,
    pub issues: Vec<FieldIssue>,
}

#[cfg(test)]
mod conformance_tests;
