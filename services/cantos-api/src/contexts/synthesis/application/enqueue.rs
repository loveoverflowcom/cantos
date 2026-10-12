//! Enqueue one immutable, currently authorized snapshot. One transaction covers authority,
//! replay identity, budget creation and all dialogue steps; no provider or filesystem I/O.
use super::super::postgres::{encode, jobs};
use crate::{
    contexts::{
        production::{current_snapshot, production_actor as durable_actor},
        synthesis::{
            fingerprint::SpeechInput,
            jobs::{input_fingerprint, validate_execution_mode, ExecutionMode, RetryPolicy},
            DurableError, JobSummary,
        },
    },
    platform::postgres::{normalized_uuid, Store},
};
use uuid::Uuid;

impl Store {
    pub async fn enqueue_job(
        &self,
        token: &str,
        script: &str,
        snapshot: &str,
        operation: &str,
        execution: ExecutionMode,
        retry: RetryPolicy,
    ) -> Result<JobSummary, DurableError> {
        let script = normalized_uuid(script)?;
        let snapshot = normalized_uuid(snapshot)?;
        let operation = normalized_uuid(operation)?;
        let request = encode(&(
            snapshot.clone(),
            execution,
            retry.max_attempts(),
            retry.base_seconds(),
            retry.cap_seconds(),
        ))?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let actor = durable_actor(&tx, token).await?;
        let candidate = current_snapshot(&tx, &actor, &script, &snapshot).await?;
        if let Some(prior) = jobs::run_by_operation(&tx, &script, &actor, &operation).await? {
            if prior.request_bytes != request {
                return Err(DurableError::Conflict);
            }
            let result = jobs::summary(&tx, &prior.id).await?;
            durable_actor(&tx, token).await?;
            tx.commit().await?;
            return Ok(result);
        }
        let doc = &candidate.document;
        for line in &doc.resolved {
            validate_execution_mode(execution, line).map_err(|_| DurableError::ProviderDisabled)?;
        }
        if doc.resolved.is_empty() || doc.resolved.len() > 1000 {
            return Err(DurableError::InvalidRequest);
        }
        let run = Uuid::new_v4().to_string();
        let job = Uuid::new_v4().to_string();
        jobs::insert_run(
            &tx,
            jobs::NewRun {
                id: &run,
                job: &job,
                script: &script,
                snapshot: &snapshot,
                actor: &actor,
                operation: &operation,
                document: doc,
                execution,
                request: &request,
                retry,
            },
        )
        .await?;
        for line in &doc.resolved {
            let id = Uuid::new_v4().to_string();
            let fingerprint = input_fingerprint(line).map_err(|_| DurableError::Corrupt)?;
            // A line without a position-free identity (an unpinned model) simply never reuses.
            let speech = SpeechInput::from_resolved(line)
                .ok()
                .map(|input| input.fingerprint().to_string());
            jobs::insert_step(
                &tx,
                &id,
                &job,
                &line.dialogue_id,
                &fingerprint,
                speech.as_deref(),
            )
            .await?;
        }
        // Recheck time-sensitive eligibility immediately before commit under authority locks.
        current_snapshot(&tx, &actor, &script, &snapshot).await?;
        let result = jobs::summary(&tx, &run).await?;
        // Session expiry advances even while the transaction holds authority locks.
        durable_actor(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }
}
