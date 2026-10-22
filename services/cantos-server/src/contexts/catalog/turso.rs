//! Catalog's Turso persistence adapters. Only the public-listener metadata reader is ported so
//! far; the Studio write path still carries PostgreSQL SQL in `postgres` until its own version.
mod listener;
pub(crate) use listener::{
    listener_metadata, ListenerMetadata, ListenerMetadataRequest, LISTENER_CURRENT_FILTER,
};
