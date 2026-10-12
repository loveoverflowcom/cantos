//! Expiring claims and generation fencing.
use super::{status, DurableError, JobLease};
use crate::{
    contexts::synthesis::jobs::{evolve_run, ExecutionMode, LeaseDuration, RunEvent, RunStatus},
    platform::postgres::Store,
};
use tokio_postgres::Row;

fn valid_worker(worker: &str) -> bool {
    !worker.is_empty()
        && worker.len() <= 128
        && worker
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_.:".contains(&b))
}
pub(in crate::contexts::synthesis) async fn check_lease(
    tx: &deadpool_postgres::Transaction<'_>,
    lease: &JobLease,
) -> Result<Row, DurableError> {
    let job = tx
        .query_opt(
            "SELECT status FROM durable_jobs WHERE id=$1 AND run_id=$2 FOR UPDATE",
            &[&lease.job_id, &lease.run_id],
        )
        .await?
        .ok_or(DurableError::LeaseLost)?;
    if !matches!(status(&job)?, RunStatus::Queued | RunStatus::Running) {
        return Err(DurableError::LeaseLost);
    }
    tx.query_opt("SELECT * FROM durable_steps WHERE id=$1 AND job_id=$2 AND status='leased' AND lease_owner=$3 AND lease_token=$4 AND lease_expires_at>clock_timestamp() FOR UPDATE", &[&lease.step_id,&lease.job_id,&lease.worker_id,&lease.token]).await?.ok_or(DurableError::LeaseLost)
}

impl Store {
    pub async fn claim_step(
        &self,
        worker: &str,
        execution: ExecutionMode,
        ttl: LeaseDuration,
    ) -> Result<Option<JobLease>, DurableError> {
        if !valid_worker(worker) {
            return Err(DurableError::InvalidRequest);
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let row=tx.query_opt("SELECT j.id,j.run_id,j.status FROM durable_jobs j JOIN durable_runs r ON r.id=j.run_id WHERE j.status IN ('queued','running') AND r.mode=$1 AND EXISTS(SELECT 1 FROM durable_steps s WHERE s.job_id=j.id AND ((s.status='pending' AND s.not_before<=clock_timestamp()) OR (s.status='leased' AND s.lease_expires_at<=clock_timestamp()))) ORDER BY j.created_at,j.id LIMIT 1 FOR UPDATE OF j SKIP LOCKED", &[&execution.as_str()]).await?;
        let Some(row) = row else {
            tx.commit().await?;
            return Ok(None);
        };
        let job: String = row.get("id");
        let run: String = row.get("run_id");
        let step=tx.query_opt("SELECT id FROM durable_steps WHERE job_id=$1 AND ((status='pending' AND not_before<=clock_timestamp()) OR (status='leased' AND lease_expires_at<=clock_timestamp())) ORDER BY not_before,id LIMIT 1 FOR UPDATE SKIP LOCKED", &[&job]).await?;
        let Some(step) = step else {
            tx.commit().await?;
            return Ok(None);
        };
        let step: String = step.get("id");
        let leased=tx.query_one("UPDATE durable_steps SET status='leased',lease_owner=$2,lease_token=lease_token+1,lease_expires_at=clock_timestamp()+make_interval(secs=>$3::int) WHERE id=$1 RETURNING lease_token", &[&step,&worker,&(ttl.seconds() as i32)]).await?;
        let current = status(&row)?;
        let next = if current == RunStatus::Queued {
            evolve_run(current, RunEvent::Start).map_err(|_| DurableError::Conflict)?
        } else {
            current
        };
        tx.execute(
            "UPDATE durable_jobs SET status=$2,reason=NULL WHERE id=$1",
            &[&job, &next.as_str()],
        )
        .await?;
        let result = JobLease {
            run_id: run,
            job_id: job,
            step_id: step,
            worker_id: worker.into(),
            token: leased.get(0),
        };
        tx.commit().await?;
        Ok(Some(result))
    }
    pub async fn heartbeat_step(
        &self,
        lease: &JobLease,
        ttl: LeaseDuration,
    ) -> Result<(), DurableError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        check_lease(&tx, lease).await?;
        tx.execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()+make_interval(secs=>$3::int) WHERE id=$1 AND lease_token=$2", &[&lease.step_id,&lease.token,&(ttl.seconds() as i32)]).await?;
        tx.commit().await?;
        Ok(())
    }
}
