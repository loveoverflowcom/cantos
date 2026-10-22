//! Concrete application handle and aggregate use-case errors.
//! This is inward-consumed application glue, not the outer HTTP composition root.
use crate::contexts::media::{
    domain::object_storage::StorageError, storage::registry::StorageRegistry,
};
use crate::script_ir::ReadError;
use cantos_contracts::FieldIssue;
use std::sync::Arc;

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
    Storage(StorageError),
    AudioRejected(&'static str),
    AudioInspectionUnavailable(&'static str),
}

/// Concrete handle used by the owning application use cases. PostgreSQL owns its pool;
/// Media owns its concrete private-storage registry. Neither is part of a pure domain model.
#[derive(Clone)]
pub struct Store {
    pub(crate) pool: deadpool_postgres::Pool,
    pub(crate) media_storage: Option<Arc<StorageRegistry>>,
    pub(crate) admin: Option<Arc<crate::platform::admin::AdminService>>,
    pub(crate) identity_auth: Option<Arc<crate::platform::identity_auth::IdentityAuth>>,
    /// Development Studio session tokens (`auth_kind = development`). A production host clears
    /// it, so without Identity v1 the Studio surface has no credential at all.
    pub(crate) development_sessions: bool,
}
