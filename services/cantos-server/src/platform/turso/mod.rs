//! The one Server schema registry over the shared `cantos-store-turso` owner. Foundation,
//! native sessions, the CMP business API closure and Editorial/Ingestion are implemented;
//! the remaining context write paths still need their #333 schema and adapter ports.
//!
//! Context adapters receive a [`ServerDatabase`] and share its serialized writer and readers.
//! SQL stays in each owner's persistence adapter; this module holds no business query.
use std::{fmt, path::PathBuf, sync::Arc};

use cantos_store_turso::{
    clock::{Clock, ClockFault},
    error::{CommitFault, OpenError},
    registry::{DatabaseKind, Intent, Migration, Registry, RegistryError},
    value::UnixMicros,
    OpenRequest, OwnedDatabase, WriteTransaction,
};

/// The `server.db` schema generation. PostgreSQL's epoch 2 and its checksums are never replayed.
pub const SERVER_EPOCH: u32 = 1;

/// Each reviewed slice appends a version; an applied version is never edited, so a candidate
/// file built by an older head is refused as diverged instead of adopted.
pub(crate) const MIGRATIONS: [Migration; 16] = [
    Migration {
        version: 1,
        name: "server_actor_registry",
        sql: include_str!("actor_registry.sql"),
    },
    Migration {
        version: 2,
        name: "server_identity_sessions",
        sql: include_str!("identity_sessions.sql"),
    },
    Migration {
        version: 3,
        name: "server_development_sessions",
        sql: include_str!("development_sessions.sql"),
    },
    Migration {
        version: 4,
        name: "server_editorial",
        sql: include_str!("editorial.sql"),
    },
    Migration {
        version: 5,
        name: "server_catalog",
        sql: include_str!("catalog.sql"),
    },
    Migration {
        version: 6,
        name: "server_admin",
        sql: include_str!("admin.sql"),
    },
    Migration {
        version: 7,
        name: "server_adaptation",
        sql: include_str!("adaptation.sql"),
    },
    Migration {
        version: 8,
        name: "server_production",
        sql: include_str!("production.sql"),
    },
    Migration {
        version: 9,
        name: "server_media_artwork",
        sql: include_str!("media_artwork.sql"),
    },
    Migration {
        version: 10,
        name: "server_media_audio",
        sql: include_str!("media_audio.sql"),
    },
    Migration {
        version: 11,
        name: "server_listening",
        sql: include_str!("listening.sql"),
    },
    // Versions 4-11 are reserved placeholders whose bytes are already registered and never
    // change. The CMP business API slice (#333) appends its dependency closure after them, in
    // foreign-key order, instead of editing those versions.
    Migration {
        version: 12,
        name: "server_catalog_publication_read",
        sql: include_str!("catalog_publication_read.sql"),
    },
    Migration {
        version: 13,
        name: "server_production_briefs",
        sql: include_str!("production_briefs.sql"),
    },
    Migration {
        version: 14,
        name: "server_audio_release_chain",
        sql: include_str!("audio_release_chain.sql"),
    },
    Migration {
        version: 15,
        name: "server_listener_library",
        sql: include_str!("listener_library.sql"),
    },
    Migration {
        version: 16,
        name: "server_editorial_ingestion",
        sql: include_str!("editorial_ingestion.sql"),
    },
];

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
    clock: Arc<dyn Clock>,
}

impl ServerDatabase {
    /// `Intent::Migrate` may create or upgrade the file; `Intent::Serve` requires it current.
    pub async fn open(
        path: impl Into<PathBuf>,
        intent: Intent,
        clock: Arc<dyn Clock>,
    ) -> Result<Self, ServerDatabaseError> {
        let schema = registry().map_err(|_| ServerDatabaseError::InvalidRegistry)?;
        let owner = OwnedDatabase::open(OpenRequest::new(path, schema, intent, clock.clone()))
            .await
            .map_err(ServerDatabaseError::Open)?;
        Ok(Self {
            owner: Arc::new(owner),
            clock,
        })
    }

    pub(crate) fn owner(&self) -> &OwnedDatabase {
        &self.owner
    }

    /// The trusted clock the owner stamps write transactions with, for a decision taken on a
    /// reader. A write reads `WriteTransaction::now()` instead, which follows commit order.
    pub(crate) fn now(&self) -> Result<UnixMicros, ClockFault> {
        self.clock.now()
    }
}

/// Where a COMMIT acknowledgement comes from. Production always takes the engine's answer;
/// tests lose it after a real commit or a real rollback to model an outcome nobody observed.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Acknowledgement {
    Engine,
    #[cfg(test)]
    LostAfterCommit,
    #[cfg(test)]
    LostAfterRollback,
}

impl Acknowledgement {
    pub(crate) async fn commit(self, tx: WriteTransaction<'_>) -> Result<(), CommitFault> {
        #[cfg(test)]
        let lost = || {
            CommitFault::Unknown(cantos_store_turso::error::StoreFault::Io(
                std::io::ErrorKind::TimedOut,
            ))
        };
        match self {
            Self::Engine => tx.commit().await,
            #[cfg(test)]
            Self::LostAfterCommit => {
                tx.commit().await?;
                Err(lost())
            }
            #[cfg(test)]
            Self::LostAfterRollback => {
                tx.rollback().await.map_err(CommitFault::Rejected)?;
                Err(lost())
            }
        }
    }
}

#[cfg(test)]
mod tests;
