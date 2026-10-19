//! Application facts restored only after the Editorial adapter checks stored integrity.
//! These values carry no HTTP representation and confer no current authorization.

/// Identity of an append; the admitted complete Script IR export is supplied separately.
#[derive(Clone, Copy)]
pub(crate) struct RevisionOperation<'a> {
    pub expected_revision: u64,
    pub operation_id: &'a str,
}

#[derive(Debug)]
pub(crate) struct RevisionFacts {
    pub script_id: String,
    pub revision: u64,
    pub accepted_by: String,
    pub accepted_at: String,
    pub content_digest: String,
    pub export_digest: String,
    pub canonical_export: String,
}

#[derive(Debug)]
pub(crate) struct ReviewFacts {
    pub id: String,
    pub script_id: String,
    pub revision: u64,
    pub operation_id: String,
    pub reviewed_by: String,
    pub reviewed_at: String,
    pub content_digest: String,
    pub export_digest: String,
}

pub(crate) struct RevisionHistory {
    pub revisions: Vec<RevisionFacts>,
    pub reviews: Vec<ReviewFacts>,
    pub next_after: Option<u64>,
}
