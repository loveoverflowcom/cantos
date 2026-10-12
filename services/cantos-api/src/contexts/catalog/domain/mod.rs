//! Pure catalog values and transitions. IDs and authoritative time come from the shell;
//! ownership, link existence and transaction guards are checked by the application boundary.
mod error;
mod ids;
mod metadata;
mod patch;
mod reorder;
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
pub use unit::{ContentUnit, NewContentUnit, StoredContentUnit};
pub use work::{NewWork, StoredWork, Work};

pub mod content;
#[cfg(test)]
mod tests;
