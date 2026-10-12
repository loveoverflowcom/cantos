//! Job identity, status and lifecycle SQL under application-owned transactions.
use super::{currency, status, DurableError, JobSummary};
use crate::{
    contexts::synthesis::jobs::{ExecutionMode, RetryPolicy, RunStatus},
    platform::postgres::StoreError,
};
use cantos_contracts::FrozenProductionDocument;
use tokio_postgres::Row;
use uuid::Uuid;

pub(in crate::contexts::synthesis) async fn summary(
    tx: &deadpool_postgres::Transaction<'_>,
    run: &str,
) -> Result<JobSummary, DurableError> {
    let row=tx.query_opt("SELECT j.id,j.status,j.reason,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id) AS total,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id AND s.status='succeeded') AS done,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id AND s.reused_clip_id IS NOT NULL) AS reused FROM durable_jobs j WHERE j.run_id=$1", &[&run]).await?.ok_or(DurableError::Store(StoreError::NotFound))?;
    Ok(JobSummary {
        run_id: run.into(),
        job_id: row.get("id"),
        status: status(&row)?,
        reason: row.get("reason"),
        total_steps: row.get::<_, i64>("total") as u64,
        completed_steps: row.get::<_, i64>("done") as u64,
        reused_steps: row.get::<_, i64>("reused") as u64,
    })
}
pub(in crate::contexts::synthesis) async fn run(
    tx: &deadpool_postgres::Transaction<'_>,
    id: &str,
) -> Result<Row, DurableError> {
    tx.query_opt("SELECT * FROM durable_runs WHERE id=$1", &[&id])
        .await?
        .ok_or(DurableError::Store(StoreError::NotFound))
}
pub(in crate::contexts::synthesis) async fn stop_job(
    tx: &deadpool_postgres::Transaction<'_>,
    job: &str,
    new_status: &str,
    reason: &str,
) -> Result<(), DurableError> {
    tx.execute("UPDATE durable_jobs SET status=$2,reason=$3 WHERE id=$1 AND status IN ('queued','running')", &[&job,&new_status,&reason]).await?;
    tx.execute("UPDATE durable_steps SET status='pending',lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL WHERE job_id=$1 AND status='leased'", &[&job]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) struct PriorRun {
    pub id: String,
    pub request_bytes: Vec<u8>,
}

pub(in crate::contexts::synthesis) async fn run_by_operation(
    tx: &deadpool_postgres::Transaction<'_>,
    script: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<PriorRun>, DurableError> {
    Ok(tx.query_opt(
        "SELECT id,request_bytes FROM durable_runs WHERE script_id=$1 AND created_by=$2 AND operation_id=$3",
        &[&script, &actor, &operation],
    ).await?.map(|row| PriorRun { id: row.get("id"), request_bytes: row.get("request_bytes") }))
}

pub(in crate::contexts::synthesis) struct NewRun<'a> {
    pub id: &'a str,
    pub job: &'a str,
    pub script: &'a str,
    pub snapshot: &'a str,
    pub actor: &'a str,
    pub operation: &'a str,
    pub document: &'a FrozenProductionDocument,
    pub execution: ExecutionMode,
    pub request: &'a [u8],
    pub retry: RetryPolicy,
}

pub(in crate::contexts::synthesis) async fn insert_run(
    tx: &deadpool_postgres::Transaction<'_>,
    new: NewRun<'_>,
) -> Result<(), DurableError> {
    let NewRun {
        id: run,
        job,
        script,
        snapshot,
        actor,
        operation,
        document: doc,
        execution,
        request,
        retry,
    } = new;
    let budget = &doc.settings.settings.budget;
    tx.execute("INSERT INTO durable_runs(id,script_id,snapshot_id,created_by,operation_id,input_digest,mode,request_bytes,max_attempts,base_seconds,cap_seconds) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)", &[&run,&script,&snapshot,&actor,&operation,&doc.input_digest,&execution.as_str(),&request,&(retry.max_attempts() as i32),&(retry.base_seconds() as i64),&(retry.cap_seconds() as i64)]).await?;
    tx.execute(
        "INSERT INTO durable_jobs(id,run_id,status) VALUES($1,$2,'queued')",
        &[&job, &run],
    )
    .await?;
    tx.execute("INSERT INTO durable_budgets(run_id,currency,limit_minor,reserved_minor,settled_minor) VALUES($1,$2,$3,0,0)", &[&run,&currency(budget.currency),&(budget.limit_minor as i64)]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) async fn insert_step(
    tx: &deadpool_postgres::Transaction<'_>,
    id: &str,
    job: &str,
    dialogue: &str,
    fingerprint: &str,
    speech: Option<&str>,
) -> Result<(), DurableError> {
    tx.execute("INSERT INTO durable_steps(id,job_id,dialogue_id,input_fingerprint,speech_fingerprint,status) VALUES($1,$2,$3,$4,$5,'pending')", &[&id,&job,&dialogue,&fingerprint,&speech]).await?;
    Ok(())
}

pub(in crate::contexts::synthesis) struct ControlJob {
    pub id: String,
    status: String,
}

impl ControlJob {
    pub(in crate::contexts::synthesis) fn status(&self) -> Result<RunStatus, DurableError> {
        self.status.parse().map_err(|_| DurableError::Corrupt)
    }
}

pub(in crate::contexts::synthesis) async fn job_for_control(
    tx: &deadpool_postgres::Transaction<'_>,
    id: &str,
) -> Result<ControlJob, DurableError> {
    let job = tx
        .query_one(
            "SELECT * FROM durable_jobs WHERE run_id=$1 FOR UPDATE",
            &[&id],
        )
        .await?;
    Ok(ControlJob {
        id: job.get("id"),
        status: job.get("status"),
    })
}

pub(in crate::contexts::synthesis) async fn prior_control(
    tx: &deadpool_postgres::Transaction<'_>,
    job_id: &str,
    actor: &str,
    operation: &str,
) -> Result<Option<String>, DurableError> {
    Ok(tx.query_opt(
        "SELECT action FROM durable_controls WHERE job_id=$1 AND actor_id=$2 AND operation_id=$3",
        &[&job_id, &actor, &operation],
    ).await?.map(|row| row.get(0)))
}

pub(in crate::contexts::synthesis) async fn has_unresolved_attempts(
    tx: &deadpool_postgres::Transaction<'_>,
    job_id: &str,
) -> Result<bool, DurableError> {
    let unresolved:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id LEFT JOIN durable_observations o ON o.attempt_id=a.id LEFT JOIN durable_resolutions x ON x.attempt_id=a.id WHERE s.job_id=$1 AND ((x.attempt_id IS NOT NULL AND o.created_at>x.created_at) OR (x.attempt_id IS NULL AND (o.attempt_id IS NULL OR convert_from(o.receipt_bytes,'UTF8')::jsonb#>>'{outcome,delivery}'='may_have_executed'))))", &[&job_id]).await?.get(0);
    Ok(unresolved)
}

pub(in crate::contexts::synthesis) struct NewControl<'a> {
    pub job: &'a str,
    pub actor: &'a str,
    pub operation: &'a str,
    pub action: &'a str,
    pub next: RunStatus,
    pub reason: Option<&'a str>,
    pub step_status: &'a str,
}

pub(in crate::contexts::synthesis) async fn record_control(
    tx: &deadpool_postgres::Transaction<'_>,
    control: NewControl<'_>,
) -> Result<(), DurableError> {
    let NewControl {
        job: job_id,
        actor,
        operation,
        action,
        next,
        reason,
        step_status,
    } = control;
    tx.execute("INSERT INTO durable_controls(id,job_id,actor_id,operation_id,action) VALUES($1,$2,$3,$4,$5)", &[&Uuid::new_v4().to_string(),&job_id,&actor,&operation,&action]).await?;
    tx.execute(
        "UPDATE durable_jobs SET status=$2,reason=$3 WHERE id=$1",
        &[&job_id, &next.as_str(), &reason],
    )
    .await?;
    tx.execute("UPDATE durable_steps SET status=$2,lease_token=lease_token+1,lease_owner=NULL,lease_expires_at=NULL WHERE job_id=$1 AND status IN ('pending','leased','failed')", &[&job_id,&step_status]).await?;
    Ok(())
}
