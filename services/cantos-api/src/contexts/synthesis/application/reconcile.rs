//! Explicit owner attestation settles non-execution; unknown outcomes remain paused.
use super::super::postgres::{authority, jobs, outcome, reconciliation};
use super::settle::settle;
use crate::{
    contexts::{
        production::production_actor as durable_actor,
        synthesis::{
            attempt::DeliveryState,
            jobs::{AttemptOutcome, RunStatus},
            receipt::decode_receipt,
            DurableError, JobSummary,
        },
    },
    platform::postgres::{normalized_uuid, Store, StoreError},
};

impl Store {
    /// Resolve only attested non-execution; no unknown result automatically dispatches again.
    pub async fn resolve_not_executed(
        &self,
        token: &str,
        id: &str,
        attempt: &str,
        evidence: &str,
    ) -> Result<JobSummary, DurableError> {
        if evidence.trim().is_empty()
            || evidence.len() > 2048
            || evidence.chars().any(char::is_control)
        {
            return Err(DurableError::InvalidRequest);
        }
        let id = normalized_uuid(id)?;
        let attempt = normalized_uuid(attempt)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let r = jobs::run(&tx, &id).await?;
        authority(&tx, &r).await?;
        if actor != r.get::<_, String>("created_by") {
            return Err(DurableError::Store(StoreError::Forbidden));
        }
        let job = reconciliation::job_for_resolution(&tx, &id).await?;
        let job_id = &job.id;
        if job.status != RunStatus::Paused {
            return Err(DurableError::Conflict);
        }
        let prior_attempt = reconciliation::resolution_attempt(&tx, &attempt, job_id).await?;
        let receipt = prior_attempt
            .receipt_bytes
            .as_deref()
            .map(decode_receipt)
            .transpose()
            .map_err(|_| DurableError::Corrupt)?;
        if !matches!(
            outcome(receipt.as_ref(), false),
            AttemptOutcome::Unresolved
                | AttemptOutcome::Failure {
                    delivery: DeliveryState::MayHaveExecuted,
                    ..
                }
        ) {
            return Err(DurableError::Conflict);
        }
        if let Some(prior) = reconciliation::resolution_evidence(&tx, &attempt).await? {
            if prior != evidence {
                return Err(DurableError::Conflict);
            }
        } else {
            reconciliation::record_resolution(&tx, &attempt, &actor, evidence).await?;
        }
        settle(&tx, &prior_attempt.reservation_id, "released", None).await?;
        let result = jobs::summary(&tx, &id).await?;
        durable_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }
}
