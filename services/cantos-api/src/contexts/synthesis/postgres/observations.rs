//! Append-only observation and settlement SQL; the application chooses outcomes and owns tx.
use super::{DurableError, JobLease};
use tokio_postgres::Row;

pub(in crate::contexts::synthesis) async fn reservation(
    tx: &deadpool_postgres::Transaction<'_>,
    reservation: &str,
) -> Result<Row, DurableError> {
    Ok(tx
        .query_one(
            "SELECT run_id,amount_minor FROM durable_reservations WHERE id=$1",
            &[&reservation],
        )
        .await?)
}

pub(in crate::contexts::synthesis) async fn lock_budget(
    tx: &deadpool_postgres::Transaction<'_>,
    run: &str,
) -> Result<(), DurableError> {
    tx.query_one(
        "SELECT run_id FROM durable_budgets WHERE run_id=$1 FOR UPDATE",
        &[&run],
    )
    .await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn record_settlement(
    tx: &deadpool_postgres::Transaction<'_>,
    reservation: &str,
    kind: &str,
    amount: i64,
) -> Result<u64, DurableError> {
    Ok(tx.execute("INSERT INTO durable_settlements(reservation_id,kind,amount_minor) VALUES($1,$2,$3) ON CONFLICT DO NOTHING", &[&reservation, &kind, &amount]).await?)
}

pub(in crate::contexts::synthesis) async fn update_budget(
    tx: &deadpool_postgres::Transaction<'_>,
    run: &str,
    amount: i64,
    settled: i64,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_budgets SET reserved_minor=reserved_minor-$2,settled_minor=settled_minor+$3 WHERE run_id=$1", &[&run, &amount, &settled]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn attempt(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    lease: &JobLease,
) -> Result<Row, DurableError> {
    tx.query_opt("SELECT a.*,s.dialogue_id,s.input_fingerprint FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id JOIN durable_jobs j ON j.id=s.job_id WHERE a.id=$1 AND a.step_id=$2 AND s.job_id=$3 AND j.run_id=$4", &[&attempt, &lease.step_id, &lease.job_id, &lease.run_id]).await?.ok_or(DurableError::Conflict)
}

pub(in crate::contexts::synthesis) async fn lock_job(
    tx: &deadpool_postgres::Transaction<'_>,
    job: &str,
) -> Result<(), DurableError> {
    tx.query_one(
        "SELECT id FROM durable_jobs WHERE id=$1 FOR UPDATE",
        &[&job],
    )
    .await?;
    Ok(())
}

pub(in crate::contexts::synthesis) struct PriorObservation {
    pub receipt_bytes: Vec<u8>,
    pub output_path: Option<String>,
}

pub(in crate::contexts::synthesis) async fn prior_observation(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
) -> Result<Option<PriorObservation>, DurableError> {
    Ok(tx
        .query_opt(
            "SELECT receipt_bytes,output_path FROM durable_observations WHERE attempt_id=$1",
            &[&attempt],
        )
        .await?
        .map(|row| PriorObservation {
            receipt_bytes: row.get(0),
            output_path: row.get(1),
        }))
}

pub(in crate::contexts::synthesis) async fn insert_observation(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    bytes: &[u8],
    path: Option<&str>,
) -> Result<(), DurableError> {
    tx.execute(
        "INSERT INTO durable_observations(attempt_id,receipt_bytes,output_path) VALUES($1,$2,$3)",
        &[&attempt, &bytes, &path],
    )
    .await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn has_resolution(
    tx: &deadpool_postgres::Transaction<'_>,
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

pub(in crate::contexts::synthesis) async fn pause_resolution_conflict(
    tx: &deadpool_postgres::Transaction<'_>,
    job: &str,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_jobs SET status='paused',reason='resolution_contradiction' WHERE id=$1 AND status IN ('queued','running','paused')", &[&job]).await?;
    tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL WHERE job_id=$1 AND status='leased'", &[&job]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn defer_after_observation(
    tx: &deadpool_postgres::Transaction<'_>,
    step: &str,
    delay: i32,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL,not_before=clock_timestamp()+make_interval(secs=>$2::int) WHERE id=$1", &[&step, &delay]).await?;
    Ok(())
}
