//! Observation lookup and write-once selection primitives. The application owns the
//! transaction, receipt admission, current authority and the decision to complete the job.
use cantos_contracts::ResolvedSynthesisInput;
use tokio_postgres::Row;

use super::{clips, DurableError, JobLease};
use crate::contexts::synthesis::receipt::{decode_receipt, TtsReceipt};
use crate::platform::postgres::Tx;

pub(in crate::contexts::synthesis) async fn has_recorded_output(
    tx: &Tx<'_>,
    step: &str,
) -> Result<bool, DurableError> {
    Ok(tx.query_one(
        "SELECT EXISTS(SELECT 1 FROM durable_attempts a JOIN durable_observations o ON o.attempt_id=a.id WHERE a.step_id=$1 AND o.output_path IS NOT NULL)",
        &[&step],
    ).await?.get(0))
}

pub(in crate::contexts::synthesis) async fn resolved(
    tx: &Tx<'_>,
    attempt: &str,
) -> Result<bool, DurableError> {
    Ok(tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM durable_resolutions WHERE attempt_id=$1)",
            &[&attempt],
        )
        .await?
        .get(0))
}

pub(in crate::contexts::synthesis) struct Observation {
    pub intent: TtsReceipt,
    pub receipt: TtsReceipt,
    pub output_path: Option<String>,
}

pub(in crate::contexts::synthesis) async fn observed_attempt(
    tx: &Tx<'_>,
    attempt: &str,
    step: &str,
) -> Result<Observation, DurableError> {
    let row=tx.query_opt("SELECT a.intent_bytes,o.receipt_bytes,o.output_path FROM durable_attempts a JOIN durable_observations o ON o.attempt_id=a.id WHERE a.id=$1 AND a.step_id=$2", &[&attempt,&step]).await?.ok_or(DurableError::Conflict)?;
    Ok(Observation {
        intent: decode_receipt(&row.get::<_, Vec<u8>>(0)).map_err(|_| DurableError::Corrupt)?,
        receipt: decode_receipt(&row.get::<_, Vec<u8>>(1)).map_err(|_| DurableError::Corrupt)?,
        output_path: row.get(2),
    })
}

pub(in crate::contexts::synthesis) struct SelectedOutput<'a> {
    pub lease: &'a JobLease,
    pub attempt: &'a str,
    pub run: &'a Row,
    pub speech_fingerprint: Option<String>,
    pub owner: &'a str,
    pub line: &'a ResolvedSynthesisInput,
    pub receipt: &'a TtsReceipt,
    pub audio: &'a [u8],
    pub output_path: Option<String>,
}

/// Selection and registry insertion use the same transaction so either both persist or neither.
pub(in crate::contexts::synthesis) async fn select_output(
    tx: &Tx<'_>,
    selected: SelectedOutput<'_>,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_steps SET status='succeeded',accepted_attempt_id=$2,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1", &[&selected.lease.step_id,&selected.attempt]).await?;
    clips::register_accepted_clip(
        tx,
        selected.run,
        selected.speech_fingerprint.as_deref(),
        selected.owner,
        selected.line,
        selected.receipt,
        selected.audio,
        selected.output_path.as_deref(),
        selected.attempt,
    )
    .await
}

pub(in crate::contexts::synthesis) async fn pending_steps(
    tx: &Tx<'_>,
    job: &str,
) -> Result<bool, DurableError> {
    Ok(tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM durable_steps WHERE job_id=$1 AND status<>'succeeded')",
            &[&job],
        )
        .await?
        .get(0))
}

pub(in crate::contexts::synthesis) async fn complete_job(
    tx: &Tx<'_>,
    job: &str,
) -> Result<(), DurableError> {
    tx.execute(
        "UPDATE durable_jobs SET status='completed',reason=NULL WHERE id=$1",
        &[&job],
    )
    .await?;
    Ok(())
}
