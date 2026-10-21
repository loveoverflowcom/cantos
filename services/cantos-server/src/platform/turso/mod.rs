//! Candidate `server.db` for #333: the one Server schema registry over the shared
//! `cantos-store-turso` owner. Only the actor registry slice exists, so this is not Server
//! schema v1, and nothing composes it: PostgreSQL stays the serving authority until #333.9.
//!
//! Context adapters receive a [`ServerDatabase`] and share its serialized writer and readers.
//! SQL stays in each owner's persistence adapter; this module holds no business query.
use std::{fmt, path::PathBuf, sync::Arc};

use cantos_store_turso::{
    clock::Clock,
    error::OpenError,
    registry::{DatabaseKind, Intent, Migration, Registry, RegistryError},
    OpenRequest, OwnedDatabase,
};

/// The `server.db` schema generation. PostgreSQL's epoch 2 and its checksums are never replayed.
pub const SERVER_EPOCH: u32 = 1;

/// Each reviewed slice appends a version; an applied version is never edited, so a candidate
/// file built by an older head is refused as diverged instead of adopted.
pub(crate) const MIGRATIONS: [Migration; 1] = [Migration {
    version: 1,
    name: "server_actor_registry",
    sql: include_str!("actor_registry.sql"),
}];

/// The one `server.db` registry. Migration and serving compositions must share it.
pub fn registry() -> Result<Registry, RegistryError> {
    Registry::new(DatabaseKind::Server, SERVER_EPOCH, &MIGRATIONS)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ServerDatabaseError {
    InvalidRegistry,
    Open(OpenError),
}

impl fmt::Display for ServerDatabaseError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::InvalidRegistry => f.write_str("invalid server schema registry"),
            Self::Open(error) => write!(f, "server database open did not complete: {error}"),
        }
    }
}

impl std::error::Error for ServerDatabaseError {}

/// A `server.db` opened through [`registry`]. Adapters accept only this handle, so a file opened
/// with any other registry (Identity's, a fixture) cannot reach them.
#[derive(Clone)]
pub struct ServerDatabase {
    owner: Arc<OwnedDatabase>,
}

impl ServerDatabase {
    /// `Intent::Migrate` may create or upgrade the file; `Intent::Serve` requires it current.
    pub async fn open(
        path: impl Into<PathBuf>,
        intent: Intent,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, ServerDatabaseError> {
        let schema = registry().map_err(|_| ServerDatabaseError::InvalidRegistry)?;
        let owner = OwnedDatabase::open(OpenRequest::new(path, schema, intent, clock))
            .await
            .map_err(ServerDatabaseError::Open)?;
        Ok(Self {
            owner: Arc::new(owner),
        })
    }

    pub(crate) fn owner(&self) -> &OwnedDatabase {
        &self.owner
    }
}

#[cfg(test)]
mod tests;
