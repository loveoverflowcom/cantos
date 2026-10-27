//! Reusable single-owner embedded Turso infrastructure for internal consumers.
//!
//! One process owns each file. The crate supplies an exclusive owner lock, a versioned and
//! checksummed schema registry with a pure
//! startup decision, a serialized `BEGIN IMMEDIATE` writer with typed commit outcomes, an
//! injected trusted clock, read-only readers and a verified `VACUUM INTO` backup. It holds no
//! application identity, business table, query or rule. Consumers choose their header marker,
//! schema epoch and migrations, and translate store faults into their own business errors.
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
mod integrity;
mod owner;
mod sql;

#[cfg(any(test, feature = "testing"))]
pub mod testing;

pub use database::{
    BackupError, BackupReceipt, OpenRequest, OwnedDatabase, ReadConnection, Rows, WriteTransaction,
    DEFAULT_BUSY_TIMEOUT,
};
// Consumers name engine value types through this crate so the exact pin stays in one place.
pub use turso::{params_from_iter, IntoParams, Row, Value};
