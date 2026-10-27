//! Pure catalog values and transitions. IDs and authoritative time come from the shell;
//! ownership, link existence and transaction guards are checked by the application boundary.
pub mod content;
pub mod delegation;
mod error;
mod ids;
mod metadata;
pub mod metadata_disclosure;
mod patch;
mod reorder;
mod revision;
mod unit;
mod work;

pub use error::{CatalogError, IdKind};
pub use ids::{ContentUnitId, OwnerId, WorkId};
pub use metadata::{
    ContentUnitKind, Lifecycle, ScriptRef, SourceRef, UnitDetails, Visibility, WorkDetails,
    WorkKind, DESCRIPTION_MAX_BYTES, LANGUAGE_MAX_BYTES, MAX_RIGHTS_REFS, TITLE_MAX_BYTES,
};
pub use patch::{LinkPatch, UnitPatch, WorkPatch};
pub use reorder::{plan_reorder, OrderAssignment, ReorderPlan};
pub use revision::Revision;
pub use unit::{ContentUnit, NewContentUnit, StoredContentUnit};
pub use work::{NewWork, StoredWork, Work};

#[cfg(test)]
mod tests;
