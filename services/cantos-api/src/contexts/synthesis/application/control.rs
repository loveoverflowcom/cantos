//! Owner job controls replay idempotently and keep unresolved attempts from redispatching.
use super::super::postgres::{authority, jobs};
use crate::{
    contexts::{
        production::production_actor as durable_actor,
        synthesis::{
            jobs::{evolve_run, RunEvent, RunStatus},
            DurableError, JobSummary,
        },
    },
    platform::postgres::{normalized_uuid, Store},
};

impl Store {
    pub async fn control_job(
        &self,
        token: &str,
        id: &str,
        operation: &str,
        event: RunEvent,
    ) -> Result<JobSummary, DurableError> {
        let id = normalized_uuid(id)?;
        let operation = normalized_uuid(operation)?;
        let action = match event {
            RunEvent::Pause => "pause",
            RunEvent::Resume => "resume",
            RunEvent::Cancel => "cancel",
            RunEvent::Retry => "retry",
            RunEvent::Start | RunEvent::Complete | RunEvent::Fail => {
                return Err(DurableError::InvalidRequest)
            }
        };
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let r = jobs::run(&tx, &id).await?;
        crate::contexts::editorial::authorize(
            &tx,
            &actor,
            &r.get::<_, String>("script_id"),
            crate::contexts::editorial::Action::Review,
            crate::platform::postgres::LockMode::Update,
        )
        .await?;
        let job = jobs::job_for_control(&tx, &id).await?;
        let job_id = &job.id;
        if let Some(prior) = jobs::prior_control(&tx, job_id, &actor, &operation).await? {
            if prior != action {
                return Err(DurableError::Conflict);
            }
            let result = jobs::summary(&tx, &id).await?;
            durable_actor(&tx, token).await?;
            tx.commit().await?;
            return Ok(result);
        }
        if matches!(event, RunEvent::Resume | RunEvent::Retry) {
            authority(&tx, &r).await?;
            let unresolved = jobs::has_unresolved_attempts(&tx, job_id).await?;
            if unresolved {
                return Err(DurableError::ReconciliationRequired);
            }
        }
        let next = evolve_run(job.status()?, event).map_err(|_| DurableError::Conflict)?;
        let reason = if next == RunStatus::Paused {
            Some("creator_requested")
        } else {
            None
        };
        let step_status = if next == RunStatus::Cancelled {
            "cancelled"
        } else {
            "pending"
        };
        jobs::record_control(
            &tx,
            jobs::NewControl {
                job: job_id,
                actor: &actor,
                operation: &operation,
                action,
                next,
                reason,
                step_status,
            },
        )
        .await?;
        let result = jobs::summary(&tx, &id).await?;
        durable_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }
}
