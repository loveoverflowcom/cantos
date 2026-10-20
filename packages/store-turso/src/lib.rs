//! Single-owner embedded Turso foundation shared by Cantos Server (`server.db`) and Cantos
//! Identity (`identity.db`).
//!
//! One process owns each file. The crate supplies what both services would otherwise build
//! twice: an exclusive owner lock, a versioned and checksummed schema registry with a pure
//! startup decision, a serialized `BEGIN IMMEDIATE` writer with typed commit outcomes, an
//! injected trusted clock, read-only readers and a verified `VACUUM INTO` backup. It holds no
//! business table, query or rule; those stay in each service's contexts.
//!
//! The engine is a pre-1.0 library pinned exactly. Every capability the schema relies on is
//! asserted against the real engine by [`capability`], so an upgrade that changes behavior
//! fails a test instead of a production invariant.

#![forbid(unsafe_code)]

pub mod capability;
pub mod clock;
pub mod error;
pub mod registry;
pub mod value;

mod database;
mod owner;
mod sql;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use database::{
    BackupError, BackupReceipt, OpenRequest, OwnedDatabase, ReadConnection, WriteTransaction,
    DEFAULT_BUSY_TIMEOUT,
};
// Consumers name engine value types through this crate so the exact pin stays in one place.
pub use turso::{params_from_iter, IntoParams, Row, Rows, Value};
