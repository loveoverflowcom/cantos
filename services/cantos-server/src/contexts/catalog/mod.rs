//! Work and content-unit identity, metadata and stable ordering for Cantos Studio.
pub mod domain;

mod application;
mod postgres;
mod storage;
mod turso;

pub use application::content;
pub use application::delegation;
pub(crate) use application::owner_work::{
    advance_owner_work, owner_work, owner_work_now, owner_work_revision, recheck_owner_work,
    OwnerWork,
};
pub(crate) use application::production_content::{
    pinned_content, production_content, restore_content, ProductionContent,
};
pub use application::wire;
pub use application::{
    CatalogCommand, CatalogCursor, CatalogOutcome, CatalogPage, CatalogStoreError, MemberRole,
};

pub(crate) use turso::{
    listener_metadata, ListenerMetadata, ListenerMetadataRequest, LISTENER_CURRENT_FILTER,
};

/// Test-only reads of a stored snapshot through the production decoders, so a fixture that edits a
/// Work or Unit starts from exactly the values a reader would admit.
#[cfg(any(test, feature = "test-support"))]
pub(crate) fn decode_work_snapshot(
    bytes: &[u8],
) -> Result<domain::Work, crate::platform::store::StoreError> {
    storage::decode_work(bytes)
}

#[cfg(any(test, feature = "test-support"))]
pub(crate) fn decode_unit_snapshot(
    bytes: &[u8],
    parent: domain::WorkKind,
) -> Result<domain::ContentUnit, crate::platform::store::StoreError> {
    storage::decode_unit(bytes, parent)
}
