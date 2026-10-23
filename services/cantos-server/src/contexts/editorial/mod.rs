//! Editorial: accepted Script IR revisions, editorial reviews, the evidence records revisions
//! cite and the per-script access table that every other context reuses.
//!
//! Upstream of every other context. Other contexts reach its tables only through the
//! `pub(crate)` use-case helpers and queries re-exported here.
mod application;
pub(crate) mod document;
pub mod domain;
mod facts;
mod turso;

pub(crate) use application::{append_revision, authorize};
pub use domain::{
    decide_review, decide_save, export_digest, permits, Access, Action, EvidenceKind,
    PriorOperation, ReviewDecision, SaveDecision,
};
pub(crate) use facts::{ReviewFacts, RevisionFacts, RevisionHistory, RevisionOperation};
pub(crate) use turso::{
    ensure_evidence, evidence_exists, find_revision, load_revision, owned_revision,
    register_evidence, reviewed_by_active_owner, rights_cited_by_script, script_exists,
    source_cited_by_script,
};
