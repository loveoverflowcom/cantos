//! PostgreSQL plumbing shared by every context: the pool, local connection policy, transaction
//! locks, identifier canonicalization and the error vocabulary of the use cases built on it.
//!
//! Business tables, queries and decisions live in each context's own `postgres` and
//! `application` modules, never here.
use std::time::Duration;

use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::{Config, NoTls};

use crate::platform::ids::canonical_uuid;

mod migrations;
mod session;

pub use migrations::migrate;
pub use session::token_hash;
pub(crate) use session::{authenticate, session_token_registered};

/// A transaction borrowed by repository functions. Use cases open, pass and commit it but never
/// issue SQL through it themselves.
pub(crate) type Tx<'a> = deadpool_postgres::Transaction<'a>;

// Retained public compatibility path; the aggregate and its use-case error vocabulary belong
// to composition, while this module implements PostgreSQL connection/session plumbing only.
pub use crate::composition::{Store, StoreError};

impl From<tokio_postgres::Error> for StoreError {
    fn from(_: tokio_postgres::Error) -> Self {
        Self::Unavailable
    }
}

/// Row locks taken while reading facts that a decision depends on.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LockMode {
    /// `FOR SHARE`: concurrent readers, no concurrent writer.
    Share,
    /// `FOR UPDATE`: the caller writes under this row.
    Update,
}

impl LockMode {
    pub(crate) fn sql(self) -> &'static str {
        match self {
            LockMode::Share => "SHARE",
            LockMode::Update => "UPDATE",
        }
    }
}

/// Local/test only until TLS, deployment identity and secret delivery are reviewed.
pub fn local_config(url: &str) -> Result<Config, StoreError> {
    let mut config: Config = url.parse().map_err(|_| StoreError::InvalidRequest)?;
    if config.get_hosts().is_empty()
        || !config.get_hosts().iter().all(|host| match host {
            tokio_postgres::config::Host::Tcp(name) => {
                name == "127.0.0.1" || name == "localhost" || name == "::1"
            }
            tokio_postgres::config::Host::Unix(_) => true,
        })
    {
        return Err(StoreError::InvalidRequest);
    }
    config.connect_timeout(Duration::from_secs(5));
    config.options("-c statement_timeout=5000 -c lock_timeout=3000 -c idle_in_transaction_session_timeout=10000");
    Ok(config)
}

impl Store {
    pub fn new(config: Config) -> Result<Self, StoreError> {
        // Verified recycling detects a terminated socket before lending it again.
        let manager = Manager::from_config(
            config,
            NoTls,
            ManagerConfig {
                recycling_method: RecyclingMethod::Verified,
            },
        );
        let pool = Pool::builder(manager)
            .max_size(8)
            .wait_timeout(Some(Duration::from_secs(5)))
            .runtime(deadpool_postgres::Runtime::Tokio1)
            .build()
            .map_err(|_| StoreError::Unavailable)?;
        Ok(Self {
            pool,
            media_storage: None,
        })
    }

    pub(crate) async fn connection(&self) -> Result<deadpool_postgres::Object, StoreError> {
        self.pool.get().await.map_err(|_| StoreError::Unavailable)
    }
}

pub(crate) fn checked_number(number: u64) -> Result<i64, StoreError> {
    i64::try_from(number).map_err(|_| StoreError::InvalidRequest)
}

pub(crate) fn normalized_uuid(id: &str) -> Result<String, StoreError> {
    canonical_uuid(id).ok_or(StoreError::InvalidRequest)
}
