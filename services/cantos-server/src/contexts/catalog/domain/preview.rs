//! Admission of a Studio preview of one exact saved Work revision (ADR 0054 D03).
//! A pure decision over the Work the shell just read; it grants nothing and writes nothing.
use super::{CatalogError, Lifecycle, Work};

/// A preview shows the revision the creator named, never a different one. The saved head is the
/// only revision whose Units are recoverable (every child change advances the Work revision),
/// so a moved head is a stale request, not a fallback to the new head.
pub fn admit_preview(work: &Work, requested_revision: u64) -> Result<(), CatalogError> {
    if work.revision() != requested_revision {
        return Err(CatalogError::RevisionConflict {
            current_revision: work.revision(),
            expected_revision: requested_revision,
        });
    }
    match work.lifecycle() {
        Lifecycle::Draft => Ok(()),
        Lifecycle::Archived => Err(CatalogError::WorkArchived),
    }
}
