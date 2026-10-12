//! Attempt/reservation facts and writes. The application owns each encompassing transaction.
use super::{DurableError, JobLease};
use tokio_postgres::Row;

pub(in crate::contexts::synthesis) async fn latest_attempt(
    tx: &deadpool_postgres::Transaction<'_>,
    step: &str,
) -> Result<Option<Row>, DurableError> {
    Ok(tx.query_opt("SELECT a.*,o.receipt_bytes,o.output_path,EXISTS(SELECT 1 FROM durable_resolutions x WHERE x.attempt_id=a.id) AS resolved,EXISTS(SELECT 1 FROM durable_resolutions x WHERE x.attempt_id=a.id AND o.created_at>x.created_at) AS resolution_conflict FROM durable_attempts a LEFT JOIN durable_observations o ON o.attempt_id=a.id WHERE a.step_id=$1 ORDER BY attempt_number DESC LIMIT 1", &[&step]).await?)
}

pub(in crate::contexts::synthesis) async fn retry_waiting(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    delay: i32,
) -> Result<bool, DurableError> {
    Ok(tx.query_one("SELECT COALESCE((SELECT created_at FROM durable_resolutions WHERE attempt_id=$1),(SELECT created_at FROM durable_observations WHERE attempt_id=$1))+make_interval(secs=>$2::int)>clock_timestamp()", &[&attempt, &delay]).await?.get(0))
}

pub(in crate::contexts::synthesis) async fn defer_retry(
    tx: &deadpool_postgres::Transaction<'_>,
    step: &str,
    attempt: &str,
    delay: i32,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL,not_before=COALESCE((SELECT created_at FROM durable_resolutions WHERE attempt_id=$2),(SELECT created_at FROM durable_observations WHERE attempt_id=$2))+make_interval(secs=>$3::int) WHERE id=$1", &[&step, &attempt, &delay]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn reserve_budget(
    tx: &deadpool_postgres::Transaction<'_>,
    run: &str,
    amount: i64,
    currency: &str,
) -> Result<u64, DurableError> {
    Ok(tx.execute("UPDATE durable_budgets SET reserved_minor=reserved_minor+$2 WHERE run_id=$1 AND currency=$3 AND $2<=limit_minor-settled_minor-reserved_minor", &[&run, &amount, &currency]).await?)
}

pub(in crate::contexts::synthesis) async fn insert_reservation(
    tx: &deadpool_postgres::Transaction<'_>,
    reservation: &str,
    run: &str,
    step: &str,
    amount: i64,
    currency: &str,
) -> Result<(), DurableError> {
    tx.execute("INSERT INTO durable_reservations(id,run_id,step_id,amount_minor,currency) VALUES($1,$2,$3,$4,$5)", &[&reservation, &run, &step, &amount, &currency]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) struct NewAttempt<'a> {
    pub id: &'a str,
    pub lease: &'a JobLease,
    pub operation: &'a str,
    pub reservation: &'a str,
    pub intent_bytes: &'a [u8],
    pub number: i32,
}

pub(in crate::contexts::synthesis) async fn insert_attempt(
    tx: &deadpool_postgres::Transaction<'_>,
    new: NewAttempt<'_>,
) -> Result<(), DurableError> {
    let NewAttempt {
        id,
        lease,
        operation,
        reservation,
        intent_bytes,
        number,
    } = new;
    tx.execute("INSERT INTO durable_attempts(id,step_id,lease_token,operation_id,reservation_id,intent_bytes,attempt_number) VALUES($1,$2,$3,$4,$5,$6,$7)", &[&id, &lease.step_id, &lease.token, &operation, &reservation, &intent_bytes, &number]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn has_dispatch(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
) -> Result<bool, DurableError> {
    Ok(tx
        .query_one(
            "SELECT EXISTS(SELECT 1 FROM durable_dispatches WHERE attempt_id=$1)",
            &[&attempt],
        )
        .await?
        .get(0))
}

pub(in crate::contexts::synthesis) async fn intent_for_dispatch(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    step: &str,
    token: i64,
) -> Result<Vec<u8>, DurableError> {
    Ok(tx.query_opt("SELECT intent_bytes FROM durable_attempts WHERE id=$1 AND step_id=$2 AND lease_token=$3", &[&attempt, &step, &token]).await?.ok_or(DurableError::Conflict)?.get(0))
}

pub(in crate::contexts::synthesis) async fn has_observation_or_resolution(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
) -> Result<bool, DurableError> {
    Ok(tx.query_one("SELECT EXISTS(SELECT 1 FROM durable_observations WHERE attempt_id=$1) OR EXISTS(SELECT 1 FROM durable_resolutions WHERE attempt_id=$1)", &[&attempt]).await?.get(0))
}

pub(in crate::contexts::synthesis) async fn record_dispatch(
    tx: &deadpool_postgres::Transaction<'_>,
    attempt: &str,
    token: i64,
) -> Result<u64, DurableError> {
    Ok(tx.execute("INSERT INTO durable_dispatches(attempt_id,lease_token) VALUES($1,$2) ON CONFLICT DO NOTHING", &[&attempt, &token]).await?)
}
