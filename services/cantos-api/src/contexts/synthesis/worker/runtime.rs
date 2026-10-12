//! Claim loop, lease heartbeat, explicit development failpoints and operational events.
use super::dispatch::dispatch;
use super::output::{prepare_root, valid_identifier};
use super::recovery::{recover_recorded, try_reuse};
use super::{Failpoint, WorkerConfig, WorkerError, WorkerProvider};
use crate::{
    contexts::synthesis::{jobs::LeaseDuration, DurableError, JobLease, StepWork},
    platform::postgres::{Store, StoreError},
};
use serde::Serialize;
use std::{
    io::Write,
    path::Path,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

/// Claims and performs one step per iteration; no transaction spans synthesis or file I/O.
pub async fn run_worker(store: Store, config: WorkerConfig) -> Result<(), WorkerError> {
    validate_config(&config)?;
    let root = prepare_root(&config.output_root)?;
    let shutdown = tokio::signal::ctrl_c();
    tokio::pin!(shutdown);
    loop {
        let claim = store.claim_step(
            &config.worker_id,
            config.provider.mode(),
            config.lease_duration,
        );
        let lease = tokio::select! {
            result = claim => result?,
            _ = &mut shutdown => return Ok(()),
        };
        if let Some(lease) = lease {
            match perform_step(&store, &config, &root, &lease).await {
                Ok(StepOutcome::Finished) => {}
                Ok(StepOutcome::Quarantined(error)) if config.once => return Err(error),
                Ok(StepOutcome::Quarantined(_)) => emit_event("quarantined", &lease, None)?,
                Err(error) if !config.once && expected_job_boundary_error(&error) => {
                    emit_event("job_boundary_rejected", &lease, None)?;
                }
                Err(error) => return Err(error),
            }
        }
        if config.once {
            return Ok(());
        }
        tokio::select! {
            _ = tokio::time::sleep(Duration::from_millis(500)) => {},
            _ = &mut shutdown => return Ok(()),
        }
    }
}

pub(super) fn expected_job_boundary_error(error: &WorkerError) -> bool {
    matches!(
        error,
        WorkerError::Durable(
            DurableError::LeaseLost
                | DurableError::ReconciliationRequired
                | DurableError::Store(
                    StoreError::Unauthenticated
                        | StoreError::Forbidden
                        | StoreError::NotFound
                        | StoreError::StaleProductionInputs
                        | StoreError::StaleRevision(_)
                        | StoreError::ProductionBlocked(_)
                )
        )
    )
}

pub(super) enum StepOutcome {
    Finished,
    /// The private result was quarantined in PostgreSQL; preserve its original shell error.
    Quarantined(WorkerError),
}

fn validate_config(config: &WorkerConfig) -> Result<(), WorkerError> {
    if !config.output_root.is_absolute()
        || !valid_identifier(&config.worker_id)
        || (config.failpoint.is_some() && !matches!(config.provider, WorkerProvider::SyntheticFake))
    {
        return Err(WorkerError::InvalidConfiguration);
    }
    if let WorkerProvider::LocalEspeak { runtime } = &config.provider {
        if !runtime.is_absolute() {
            return Err(WorkerError::InvalidConfiguration);
        }
    }
    Ok(())
}

async fn perform_step(
    store: &Store,
    config: &WorkerConfig,
    root: &Path,
    lease: &JobLease,
) -> Result<StepOutcome, WorkerError> {
    emit_event("claimed", lease, None)?;
    pause_at(config, Failpoint::AfterClaim, lease, None).await?;
    let heartbeat = Heartbeat::start(store.clone(), lease.clone(), config.lease_duration);
    // A verified clip, if one exists, satisfies the step with no attempt, reservation or send.
    // Anything short of a fully verified hit falls through to the ordinary dispatch path.
    if try_reuse(store, config, root, lease, &heartbeat).await? {
        return Ok(StepOutcome::Finished);
    }
    let work = store.prepare_step(lease).await?;
    match work {
        StepWork::Blocked => emit_event("blocked", lease, None).map(|()| StepOutcome::Finished),
        StepWork::Recover(attempt, recorded) => {
            recover_recorded(store, config, root, lease, &attempt, &recorded, &heartbeat).await
        }
        StepWork::Dispatch(attempt) => {
            if attempt.mode != config.provider.mode() {
                return Err(WorkerError::InvalidConfiguration);
            }
            dispatch(store, config, root, &attempt, &heartbeat)
                .await
                .map(|()| StepOutcome::Finished)
        }
    }
}

pub(super) struct Heartbeat {
    task: tokio::task::JoinHandle<()>,
    lost: Arc<AtomicBool>,
}

impl Heartbeat {
    fn start(store: Store, lease: JobLease, duration: LeaseDuration) -> Self {
        let lost = Arc::new(AtomicBool::new(false));
        let task_lost = Arc::clone(&lost);
        let task = tokio::spawn(async move {
            let interval = Duration::from_secs(u64::from(duration.seconds()) / 3)
                .max(Duration::from_millis(100));
            loop {
                tokio::time::sleep(interval).await;
                if store.heartbeat_step(&lease, duration).await.is_err() {
                    task_lost.store(true, Ordering::Release);
                    return;
                }
            }
        });
        Self { task, lost }
    }

    pub(super) fn lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.task.abort();
    }
}

pub(super) async fn pause_at(
    config: &WorkerConfig,
    point: Failpoint,
    lease: &JobLease,
    attempt_id: Option<&str>,
) -> Result<(), WorkerError> {
    if config.failpoint != Some(point) {
        return Ok(());
    }
    #[derive(Serialize)]
    struct Marker<'a> {
        event: &'static str,
        point: &'static str,
        job_id: &'a str,
        step_id: &'a str,
        attempt_id: Option<&'a str>,
    }
    emit(&Marker {
        event: "failpoint",
        point: point.as_str(),
        job_id: &lease.job_id,
        step_id: &lease.step_id,
        attempt_id,
    })?;
    tokio::task::spawn_blocking(|| {
        let mut line = String::new();
        std::io::stdin()
            .read_line(&mut line)
            .map_err(|_| WorkerError::FailpointInput)?;
        if line.trim() != "continue" {
            return Err(WorkerError::FailpointInput);
        }
        Ok(())
    })
    .await
    .map_err(|_| WorkerError::FailpointInput)?
}

pub(super) fn emit_event(
    event: &str,
    lease: &JobLease,
    attempt_id: Option<&str>,
) -> Result<(), WorkerError> {
    #[derive(Serialize)]
    struct Event<'a> {
        event: &'a str,
        job_id: &'a str,
        step_id: &'a str,
        attempt_id: Option<&'a str>,
    }
    emit(&Event {
        event,
        job_id: &lease.job_id,
        step_id: &lease.step_id,
        attempt_id,
    })
}

pub(super) fn emit_reuse_event(lease: &JobLease, clip_id: &str) -> Result<(), WorkerError> {
    #[derive(Serialize)]
    struct Event<'a> {
        event: &'static str,
        job_id: &'a str,
        step_id: &'a str,
        attempt_id: Option<&'a str>,
        clip_id: &'a str,
    }
    emit(&Event {
        event: "reused",
        job_id: &lease.job_id,
        step_id: &lease.step_id,
        attempt_id: None,
        clip_id,
    })
}

fn emit(value: &impl Serialize) -> Result<(), WorkerError> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).map_err(|_| WorkerError::OutputIo)?;
    stdout
        .write_all(b"\n")
        .and_then(|()| stdout.flush())
        .map_err(|_| WorkerError::OutputIo)
}
