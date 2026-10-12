//! Durable PostgreSQL primitives, grouped by job lifecycle and attempt boundary.
//! Application use cases retain transaction ownership; SQL never decides provider execution.
pub(super) mod acceptance;
pub(super) mod attempts;
mod clips;
pub(super) mod jobs;
mod leases;
pub(super) mod observations;
pub(super) mod reconciliation;
#[cfg(feature = "dbsp-experimental")]
pub mod summary_view;

pub use clips::{ObjectProbes, PlanFacts, PlanReport, ReusePrepared, ReuseSelected};
#[cfg(feature = "dbsp-experimental")]
use jobs::summary;
pub(super) use jobs::{run, stop_job};
pub(super) use leases::check_lease;

use crate::{
    contexts::{
        production::current_snapshot,
        synthesis::{
            attempt::{DeliveryState, FailureClass},
            jobs::{AttemptOutcome, ExecutionMode, RetryPolicy, RunStatus},
            receipt::{decode_document, ReceiptOutcome, TtsReceipt},
        },
    },
    platform::postgres::StoreError,
};
use cantos_contracts::{FrozenProductionDocument, ProductionCurrency, ResolvedSynthesisInput};
use serde::{Deserialize, Serialize};
use tokio_postgres::Row;

#[derive(Debug)]
pub enum DurableError {
    Store(StoreError),
    InvalidRequest,
    Conflict,
    LeaseLost,
    BudgetExhausted,
    ReconciliationRequired,
    ProviderDisabled,
    Corrupt,
}
impl From<StoreError> for DurableError {
    fn from(value: StoreError) -> Self {
        Self::Store(value)
    }
}
impl From<tokio_postgres::Error> for DurableError {
    fn from(_: tokio_postgres::Error) -> Self {
        Self::Store(StoreError::Unavailable)
    }
}

#[derive(Debug, Clone)]
pub struct JobLease {
    pub run_id: String,
    pub job_id: String,
    pub step_id: String,
    pub worker_id: String,
    pub token: i64,
}
#[derive(Debug, Clone)]
pub struct AttemptWork {
    pub lease: JobLease,
    pub document: FrozenProductionDocument,
    pub line: ResolvedSynthesisInput,
    pub intent: TtsReceipt,
    pub mode: ExecutionMode,
    pub input_fingerprint: String,
}
#[derive(Debug, Clone)]
pub struct RecordedOutput {
    pub receipt: TtsReceipt,
    pub output_path: String,
}
#[derive(Debug)]
pub enum StepWork {
    Dispatch(Box<AttemptWork>),
    Recover(Box<AttemptWork>, Box<RecordedOutput>),
    Blocked,
}
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct JobSummary {
    pub run_id: String,
    pub job_id: String,
    pub status: RunStatus,
    pub reason: Option<String>,
    /// Steps that succeeded, however they were satisfied (generated or reused).
    pub completed_steps: u64,
    /// Of the completed steps, those satisfied by a verified clip with no provider attempt.
    /// Absent in reports written before the clip registry; it then reads as zero.
    #[serde(default)]
    pub reused_steps: u64,
    pub total_steps: u64,
}

pub(super) fn encode(value: &impl Serialize) -> Result<Vec<u8>, DurableError> {
    let bytes = serde_json::to_vec(value).map_err(|_| DurableError::InvalidRequest)?;
    if bytes.len() > 65536 {
        return Err(DurableError::InvalidRequest);
    }
    Ok(bytes)
}
pub(super) fn status(row: &Row) -> Result<RunStatus, DurableError> {
    row.get::<_, String>("status")
        .parse()
        .map_err(|_| DurableError::Corrupt)
}
pub(super) fn mode(row: &Row) -> Result<ExecutionMode, DurableError> {
    row.get::<_, String>("mode")
        .parse()
        .map_err(|_| DurableError::Corrupt)
}
/// Authority refusals that pause a job rather than fail a command. One definition serves the
/// dispatch preparation and the reuse lookup so they can never disagree about when to stop.
pub(super) fn is_authority_boundary(error: &StoreError) -> bool {
    matches!(
        error,
        StoreError::ProductionBlocked(_)
            | StoreError::Unauthenticated
            | StoreError::Forbidden
            | StoreError::NotFound
            | StoreError::StaleProductionInputs
            | StoreError::StaleRevision(_)
    )
}
pub(super) fn currency(value: ProductionCurrency) -> &'static str {
    match value {
        ProductionCurrency::USD => "USD",
        ProductionCurrency::VND => "VND",
    }
}
pub(super) fn outcome(receipt: Option<&TtsReceipt>, resolved: bool) -> AttemptOutcome {
    if resolved
        && receipt.is_some_and(|receipt| matches!(receipt.outcome, ReceiptOutcome::Output { .. }))
    {
        return AttemptOutcome::Unresolved;
    }
    if resolved {
        return AttemptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: DeliveryState::ConfirmedNotExecuted,
        };
    }
    match receipt.map(|r| &r.outcome) {
        None => AttemptOutcome::Unresolved,
        Some(ReceiptOutcome::Output { .. }) => AttemptOutcome::Output,
        Some(ReceiptOutcome::Failure { class, delivery }) => AttemptOutcome::Failure {
            class: *class,
            delivery: *delivery,
        },
    }
}
pub(super) fn policy(row: &Row) -> Result<RetryPolicy, DurableError> {
    RetryPolicy::new(
        row.get::<_, i32>("max_attempts") as u32,
        row.get::<_, i64>("base_seconds") as u32,
        row.get::<_, i64>("cap_seconds") as u32,
    )
    .map_err(|_| DurableError::Corrupt)
}

pub(super) async fn document(
    tx: &deadpool_postgres::Transaction<'_>,
    r: &Row,
) -> Result<FrozenProductionDocument, DurableError> {
    let snapshot: String = r.get("snapshot_id");
    let bytes = crate::contexts::production::stored_document(
        tx,
        &r.get::<_, String>("script_id"),
        &snapshot,
    )
    .await?;
    let doc = decode_document(&bytes).map_err(|_| DurableError::Corrupt)?;
    if doc.input_digest != r.get::<_, String>("input_digest")
        || doc.revision.script_id != r.get::<_, String>("script_id")
        || doc.owner_id != r.get::<_, String>("created_by")
    {
        return Err(DurableError::Corrupt);
    }
    Ok(doc)
}
pub(super) async fn authority(
    tx: &deadpool_postgres::Transaction<'_>,
    r: &Row,
) -> Result<FrozenProductionDocument, DurableError> {
    let candidate = current_snapshot(
        tx,
        &r.get::<_, String>("created_by"),
        &r.get::<_, String>("script_id"),
        &r.get::<_, String>("snapshot_id"),
    )
    .await?;
    if candidate.document.input_digest != r.get::<_, String>("input_digest") {
        return Err(DurableError::Corrupt);
    }
    Ok(candidate.document)
}
