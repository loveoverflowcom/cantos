//! Editorial: accepted Script IR revisions, editorial reviews, sources linked to a script and
//! the per-script access table that every other context reuses.
pub mod domain;
mod postgres;

pub use domain::revisions::{
    decide_review, decide_save, permits, Access, Action, PriorOperation, ReviewDecision,
    SaveDecision,
};
