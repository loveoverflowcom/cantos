//! PostgreSQL plumbing shared by every context: the pool, local connection policy, transaction
//! locks, identifier canonicalization and the error vocabulary of the use cases built on it.
//!
//! Business tables, queries and decisions live in each context's own `postgres` and
//! `application` modules, never here.
use std::time::Duration;

use cantos_contracts::FieldIssue;
use deadpool_postgres::{Manager, ManagerConfig, Pool, RecyclingMethod};
use tokio_postgres::{Config, NoTls};
use uuid::Uuid;

use crate::script_ir::ReadError;

mod migrations;
mod session;

pub use migrations::migrate;
pub(crate) use session::authenticate;
pub use session::token_hash;

/// A transaction borrowed by repository functions. Use cases open, pass and commit it but never
/// issue SQL through it themselves.
pub(crate) type Tx<'a> = deadpool_postgres::Transaction<'a>;

/// Outcome vocabulary of every use case, mapped one-to-one onto the HTTP error contract.
///
/// Three origins share it: access refusals (`Unauthenticated`, `NotFound`, `Forbidden`),
/// rejections carried up from pure domain decisions (`InvalidRequest*`, `InvalidScript`,
/// `EvidenceUnavailable`, `Stale*`, `OperationReused`, `ProposalAlreadySubmitted`,
/// `ProductionBlocked`) and infrastructure failures (`Unavailable` for the database,
/// `CorruptRevision` for a stored row that fails its integrity checks). Domain functions never
/// construct it; application code maps their typed results into it.
#[derive(Debug)]
pub enum StoreError {
    Unauthenticated,
    NotFound,
    Forbidden,
    InvalidRequest,
    InvalidRequestFields(Vec<FieldIssue>),
    InvalidScript(ReadError),
    EvidenceUnavailable,
    StaleRevision(u64),
    OperationReused,
    ProposalAlreadySubmitted,
    StaleProductionInputs,
    ProductionBlocked(Vec<FieldIssue>),
    Unavailable,
    CorruptRevision,
}

impl From<tokio_postgres::Error> for StoreError {
    fn from(_: tokio_postgres::Error) -> Self {
        Self::Unavailable
    }
}

/// The application's handle on its PostgreSQL authority. Each context adds its use cases as
/// inherent methods from its own `application` module.
#[derive(Clone)]
pub struct Store {
    pool: Pool,
    /// Opt-in experimental read model for `job_status`; absent, every read is PostgreSQL's.
    /// The one place this module names a context: ADR 0017 attaches the view to the store.
    #[cfg(feature = "dbsp-experimental")]
    pub(crate) job_summaries:
        Option<std::sync::Arc<crate::contexts::synthesis::summary_view::AttachedView>>,
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
            #[cfg(feature = "dbsp-experimental")]
            job_summaries: None,
        })
    }

    /// Answers `job_status` through `view` under its read model. Authentication, authorization
    /// and every other read and write keep their PostgreSQL path.
    #[cfg(feature = "dbsp-experimental")]
    pub fn with_job_summary_view(
        mut self,
        view: std::sync::Arc<crate::contexts::synthesis::summary_view::AttachedView>,
    ) -> Self {
        self.job_summaries = Some(view);
        self
    }

    pub(crate) async fn connection(&self) -> Result<deadpool_postgres::Object, StoreError> {
        self.pool.get().await.map_err(|_| StoreError::Unavailable)
    }
}

pub(crate) fn checked_number(number: u64) -> Result<i64, StoreError> {
    i64::try_from(number).map_err(|_| StoreError::InvalidRequest)
}

pub(crate) fn normalized_uuid(id: &str) -> Result<String, StoreError> {
    let parsed = Uuid::parse_str(id).map_err(|_| StoreError::InvalidRequest)?;
    let canonical = parsed.to_string();
    if id != canonical {
        return Err(StoreError::InvalidRequest);
    }
    Ok(canonical)
}
