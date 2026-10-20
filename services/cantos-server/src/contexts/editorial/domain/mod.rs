//! Pure editorial rules. No HTTP, SQL, UI or clock.
pub mod access;
pub mod diagnostics;
pub mod evidence;
pub mod revisions;

pub use access::{permits, Access, Action};
pub use evidence::EvidenceKind;
pub use revisions::{
    decide_review, decide_save, export_digest, PriorOperation, ReviewDecision, SaveDecision,
};
