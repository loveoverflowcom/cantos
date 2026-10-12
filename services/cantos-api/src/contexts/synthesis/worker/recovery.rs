//! Fresh object verification for reuse and conservative recovery of recorded provider output.
use super::output::read_output;
use super::runtime::{emit_event, emit_reuse_event, pause_at, Heartbeat, StepOutcome};
use super::{Failpoint, WorkerConfig, WorkerError};
use crate::{
    contexts::synthesis::{
        media::clip_probe::ClipProber,
        receipt::audit_receipt,
        reuse::{resolve_reuse, ClipCandidate, ReuseDecision, RightsStanding},
        AttemptWork, DurableError, JobLease, RecordedOutput,
    },
    platform::postgres::Store,
};
use std::path::Path;

pub(super) async fn recover_recorded(
    store: &Store,
    config: &WorkerConfig,
    root: &Path,
    lease: &JobLease,
    attempt: &AttemptWork,
    recorded: &RecordedOutput,
    heartbeat: &Heartbeat,
) -> Result<StepOutcome, WorkerError> {
    if attempt.mode != config.provider.mode() {
        return Err(WorkerError::InvalidConfiguration);
    }
    let bytes = match read_output(root, &recorded.output_path, &attempt.intent.attempt_id) {
        Ok(bytes) => bytes,
        Err(error) => {
            store
                .quarantine_recorded_output(lease, matches!(error, WorkerError::UnsafeOutput))
                .await?;
            return Ok(StepOutcome::Quarantined(error));
        }
    };
    if audit_receipt(&attempt.document, &recorded.receipt, Some(&bytes)).is_err() {
        store.quarantine_recorded_output(lease, true).await?;
        return Ok(StepOutcome::Quarantined(WorkerError::InvalidReceipt));
    }
    pause_at(
        config,
        Failpoint::BeforeAccept,
        lease,
        Some(&attempt.intent.attempt_id),
    )
    .await?;
    if heartbeat.lost() {
        return Err(WorkerError::Durable(DurableError::LeaseLost));
    }
    store
        .accept_step(lease, &attempt.intent.attempt_id, &bytes)
        .await?;
    pause_at(
        config,
        Failpoint::AfterAccept,
        lease,
        Some(&attempt.intent.attempt_id),
    )
    .await?;
    emit_event("recovered", lease, Some(&attempt.intent.attempt_id)).map(|()| StepOutcome::Finished)
}

/// Oldest-first probe of the registry candidates, stopping at the first object that verifies.
/// No transaction is open and nothing is written until the fenced selection below.
pub(super) async fn try_reuse(
    store: &Store,
    config: &WorkerConfig,
    root: &Path,
    lease: &JobLease,
    heartbeat: &Heartbeat,
) -> Result<bool, WorkerError> {
    let Some(prepared) = store.reuse_candidates(lease).await? else {
        return Ok(false);
    };
    if prepared.candidates.is_empty() {
        return Ok(false);
    }
    let prober = ClipProber::open(root).map_err(|_| WorkerError::UnsafeOutput)?;
    let stored = prepared.candidates.clone();
    // Candidates are already scoped to this request, so the first one whose object verifies is the
    // oldest valid clip; newer ones cannot win and are not read.
    let (candidates, witnesses) = tokio::task::spawn_blocking(move || {
        let mut probed = Vec::with_capacity(stored.len());
        let mut witnesses = Vec::new();
        for stored in stored {
            let (probe, witness) = prober.inspect(&stored).into_parts();
            probed.push(ClipCandidate {
                clip: stored.clip,
                probe,
            });
            if let Some(witness) = witness {
                witnesses.push(witness);
                break;
            }
        }
        (probed, witnesses)
    })
    .await
    .map_err(|_| WorkerError::OutputIo)?;
    let ReuseDecision::Reuse { clip_id, .. } =
        resolve_reuse(&prepared.request, RightsStanding::Current, &candidates)
    else {
        return Ok(false);
    };
    let Some(verified) = witnesses
        .into_iter()
        .find(|witness| witness.clip_id() == clip_id)
    else {
        return Ok(false);
    };
    pause_at(config, Failpoint::BeforeReuse, lease, None).await?;
    if heartbeat.lost() {
        return Err(WorkerError::Durable(DurableError::LeaseLost));
    }
    match store.select_reused_clip(lease, &verified).await {
        Ok(_) => {
            emit_reuse_event(lease, &clip_id)?;
            Ok(true)
        }
        // Authority, the registry or the step changed since the lookup. The dispatch path
        // re-reads every fact itself, so falling back is safe and never reuses on a guess.
        Err(DurableError::Conflict) => Ok(false),
        Err(error) => Err(error.into()),
    }
}
