//! Work and content-unit identity, metadata and stable ordering for Cantos Studio.
pub mod domain;

mod application;
mod postgres;

pub use application::content;
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

pub(crate) use postgres::{
    listener_metadata, ListenerMetadata, ListenerMetadataRequest, LISTENER_CURRENT_FILTER,
};

pub(crate) use postgres::lock_listener_catalog;
