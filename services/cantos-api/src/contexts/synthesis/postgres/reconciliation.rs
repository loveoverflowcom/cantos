//! Resolution lookup and append-only owner attestations under the use case transaction.
use super::{status, DurableError};
use crate::contexts::synthesis::jobs::RunStatus;

pub(in crate::contexts::synthesis) struct ResolutionJob {
    pub id: String,
    pub status: RunStatus,
}

pub(in crate::contexts::synthesis) async fn job_for_resolution(
    tx: &deadpool_postgres::Transaction<'_>,
    id: &str,
) -> Result<ResolutionJob, DurableError> {
    let job = tx
        .query_one(
            "SELECT id,status FROM durable_jobs WHERE run_id=$1 FOR UPDATE",
            &[&id],
        )
        .await?;
    Ok(ResolutionJob {
        id: job.get("id"),
        status: status(&job)?,
    })
}

pub(in crate::contexts::synthesis) struct ResolutionAttempt {
    pub reservation_id: String,
    pub receipt_bytes: Option<Vec<u8>>,
}

pub(in crate::contexts::synthesis) async fn resolution_attempt(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    job_id: &str,
) -> Result<ResolutionAttempt, DurableError> {
    let row=tx.query_opt("SELECT a.reservation_id,o.receipt_bytes FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id LEFT JOIN durable_observations o ON o.attempt_id=a.id WHERE a.id=$1 AND s.job_id=$2", &[&attempt,&job_id]).await?.ok_or(DurableError::Conflict)?;
    Ok(ResolutionAttempt {
        reservation_id: row.get("reservation_id"),
        receipt_bytes: row.get("receipt_bytes"),
    })
}

pub(in crate::contexts::synthesis) async fn resolution_evidence(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
) -> Result<Option<String>, DurableError> {
    Ok(tx
        .query_opt(
            "SELECT evidence FROM durable_resolutions WHERE attempt_id=$1",
            &[&attempt],
        )
        .await?
        .map(|row| row.get(0)))
}

pub(in crate::contexts::synthesis) async fn record_resolution(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    actor: &str,
    evidence: &str,
) -> Result<(), DurableError> {
    tx.execute("INSERT INTO durable_resolutions(attempt_id,resolved_by,evidence,kind) VALUES($1,$2,$3,'not_executed')", &[&attempt,&actor,&evidence]).await?;
    Ok(())
}
