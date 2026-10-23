//! Persistence plumbing shared by every context: the one embedded Turso `server.db`, the
//! statement facade, identifier canonicalization and the error vocabulary of the use cases.
//!
//! Business tables, queries and decisions live in each context's own persistence and
//! `application` modules, never here. The schema registry is [`crate::platform::turso`]; this
//! module opens it and hands each use case a [`Conn`].
use std::{path::PathBuf, sync::Arc};

use cantos_store_turso::{clock::SystemClock, registry::Intent};

use crate::platform::{
    ids::canonical_uuid,
    turso::{Acknowledgement, ServerDatabase, ServerDatabaseError},
};

mod session;
mod sql;
#[cfg(test)]
mod tests;

pub use session::token_hash;
pub(crate) use session::{actor_active, authenticate, session_token_registered};
pub(crate) use sql::{Conn, Tx};
pub use sql::{Error, FromColumn, Param, Params, Row, RowIndex};

// Retained public compatibility path; the aggregate and its use-case error vocabulary belong
// to application, while this module implements persistence plumbing only.
pub use crate::application::{Store, StoreError};

impl From<Error> for StoreError {
    fn from(error: Error) -> Self {
        match error {
            // A stored value of the wrong type is a row written outside the typed path.
            Error::Column => Self::CorruptRevision,
            // Every other failure is infrastructure. An unknown commit outcome is reported as
            // unavailable, never as a rejection: callers reconcile by their operation receipt.
            Error::Fault(_) | Error::Commit(_) | Error::ReadOnly | Error::Cardinality => {
                Self::Unavailable
            }
        }
    }
}

impl From<ServerDatabaseError> for StoreError {
    fn from(_: ServerDatabaseError) -> Self {
        Self::Unavailable
    }
}

impl Store {
    /// Open `server.db` at `path`. [`Intent::Serve`] never changes the file and refuses one that
    /// is missing, foreign, diverged, newer or pending migrations; [`Intent::Migrate`] is the
    /// explicit operator action that creates or upgrades it. A second process opening the same
    /// file is refused: one process owns it.
    pub async fn open(
        path: impl Into<PathBuf>,
        intent: Intent,
    ) -> Result<Self, ServerDatabaseError> {
        let database = ServerDatabase::open(path, intent, Arc::new(SystemClock)).await?;
        Ok(Self::from_database(database))
    }

    pub(crate) fn database(&self) -> &ServerDatabase {
        &self.database
    }

    /// The provider-free Identity session adapter over this store's owner.
    pub(crate) fn sessions(&self) -> crate::platform::identity_auth::turso::TursoSessions {
        crate::platform::identity_auth::turso::TursoSessions::new(self.database.clone())
    }

    pub(crate) fn from_database(database: ServerDatabase) -> Self {
        Self {
            database,
            acknowledgement: Acknowledgement::Engine,
            admin: None,
            media_storage: None,
            identity_auth: None,
            development_sessions: true,
        }
    }

    /// The per-operation handle. Infallible today; asynchronous so a future connection source
    /// does not change every call site.
    pub(crate) async fn connection(&self) -> Result<Conn, StoreError> {
        Ok(Conn::new(self.database.clone(), self.acknowledgement))
    }
}

pub(crate) fn checked_number(number: u64) -> Result<i64, StoreError> {
    i64::try_from(number).map_err(|_| StoreError::InvalidRequest)
}

pub(crate) fn normalized_uuid(id: &str) -> Result<String, StoreError> {
    canonical_uuid(id).ok_or(StoreError::InvalidRequest)
}

/// The existing wire timestamp: UTC with exactly six fractional digits. Persisted time is
/// INTEGER microseconds; neither an engine clock nor a text SQL conversion is authoritative.
pub(crate) fn utc_timestamp(micros: i64) -> Result<String, StoreError> {
    use chrono::Datelike;

    let at = chrono::DateTime::from_timestamp_micros(micros)
        .filter(|at| micros >= 0 && at.year() <= 9999)
        .ok_or(StoreError::CorruptRevision)?;
    Ok(at.format("%Y-%m-%dT%H:%M:%S%.6fZ").to_string())
}

/// Batched current actor facts; no Studio session is consulted by the listener consumer.
pub(crate) async fn active_actors(
    tx: &Tx<'_>,
    ids: &[String],
) -> Result<std::collections::BTreeSet<String>, StoreError> {
    if ids.len() > 51 {
        return Err(StoreError::InvalidRequest);
    }
    tx.query(
        "SELECT id FROM actors WHERE active=1 AND id IN (SELECT value FROM json_each(?1))",
        &[&ids],
    )
    .await?
    .iter()
    .map(|r| r.try_get(0).map_err(|_| StoreError::CorruptRevision))
    .collect()
}

/// Discovery visibility requires the release owner's (creator's) actor to be active. It checks
/// any `actors.id`, and here it is applied to the owner (alias `rel`, the release row), never to
/// the listener.
pub(crate) const RELEASE_OWNER_ACTIVE_FILTER: &str =
    "EXISTS(SELECT 1 FROM actors owner_actor WHERE owner_actor.id=rel.owner_id AND owner_actor.active=1)";

/// Row locks taken by PostgreSQL-era SQL. The serialized writer orders every unit now, so a
/// context's port removes its `FOR {SHARE|UPDATE}` clauses and this type with them.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LockMode {
    Share,
    Update,
}
