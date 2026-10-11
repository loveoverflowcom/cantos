//! Local development worker shell for the existing PostgreSQL production authority.
//!
//! Only the explicit local eSpeak adapter and an opt-in synthetic test provider are reachable.
//! This module never imports a commercial or neural transport, publishes an asset or substitutes
//! current draft text for frozen inputs. Durable decisions belong to `postgres::durable`.

use std::{
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Component, Path, PathBuf},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::{
    jobs::{ExecutionMode, LeaseDuration},
    postgres::{
        durable::{AttemptWork, DurableError, JobLease, StepWork},
        Store, StoreError,
    },
    tts::{
        attempt::{DeliveryState, FailureClass},
        audio::MAX_OUTPUT_BYTES,
        audit_receipt,
        local::{run_local, LocalPhase, LocalRunError, LocalSynthesisRequest},
        ReceiptEvidence, ReceiptOutcome, TtsReceipt, UsageObservation,
    },
};

/// Explicit configuration is required; a default worker can never dispatch a provider.
#[derive(Clone, Debug)]
pub enum WorkerProvider {
    LocalEspeak { runtime: PathBuf },
    SyntheticFake,
}

impl WorkerProvider {
    pub fn mode(&self) -> ExecutionMode {
        match self {
            Self::LocalEspeak { .. } => ExecutionMode::LocalEspeak,
            Self::SyntheticFake => ExecutionMode::SyntheticFake,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failpoint {
    AfterClaim,
    AfterIntent,
    AfterProvider,
    AfterObservation,
    BeforeAccept,
    AfterAccept,
}

impl Failpoint {
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "after_claim" => Some(Self::AfterClaim),
            "after_intent" => Some(Self::AfterIntent),
            "after_provider" => Some(Self::AfterProvider),
            "after_observation" => Some(Self::AfterObservation),
            "before_accept" => Some(Self::BeforeAccept),
            "after_accept" => Some(Self::AfterAccept),
            _ => None,
        }
    }

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AfterClaim => "after_claim",
            Self::AfterIntent => "after_intent",
            Self::AfterProvider => "after_provider",
            Self::AfterObservation => "after_observation",
            Self::BeforeAccept => "before_accept",
            Self::AfterAccept => "after_accept",
        }
    }
}

pub struct WorkerConfig {
    pub provider: WorkerProvider,
    pub output_root: PathBuf,
    pub worker_id: String,
    pub lease_duration: LeaseDuration,
    pub once: bool,
    /// Only valid for the explicitly enabled synthetic provider.
    pub failpoint: Option<Failpoint>,
}

#[derive(Debug)]
pub enum WorkerError {
    InvalidConfiguration,
    UnsafeOutput,
    OutputIo,
    InvalidReceipt,
    FailpointInput,
    Durable(DurableError),
}

impl From<DurableError> for WorkerError {
    fn from(value: DurableError) -> Self {
        Self::Durable(value)
    }
}

impl std::fmt::Display for WorkerError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        // No provider error bodies, dialogue text or database URLs enter operational logs.
        write!(formatter, "local worker failed: {self:?}")
    }
}

impl std::error::Error for WorkerError {}

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

fn expected_job_boundary_error(error: &WorkerError) -> bool {
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

enum StepOutcome {
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
    let work = store.prepare_step(lease).await?;
    match work {
        StepWork::Blocked => emit_event("blocked", lease, None).map(|()| StepOutcome::Finished),
        StepWork::Recover(attempt, recorded) => {
            if attempt.mode != config.provider.mode() {
                return Err(WorkerError::InvalidConfiguration);
            }
            let bytes = match read_output(root, &recorded.output_path, &attempt.intent.attempt_id) {
                Ok(bytes) => bytes,
                Err(error) => {
                    store
                        .quarantine_recorded_output(
                            lease,
                            matches!(error, WorkerError::UnsafeOutput),
                        )
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
            emit_event("recovered", lease, Some(&attempt.intent.attempt_id))
                .map(|()| StepOutcome::Finished)
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

async fn dispatch(
    store: &Store,
    config: &WorkerConfig,
    root: &Path,
    attempt: &AttemptWork,
    heartbeat: &Heartbeat,
) -> Result<(), WorkerError> {
    // `prepare_step` has already committed the attempt and its reservation. Local journals
    // supplement that authority; they never prove non-execution or authorize a retry.
    let directory = create_attempt_directory(root, &attempt.intent.attempt_id)?;
    write_new_json(&directory.join("intent.json"), &attempt.intent)?;
    write_new_json(&directory.join("frozen-document.json"), &attempt.document)?;
    pause_at(
        config,
        Failpoint::AfterIntent,
        &attempt.lease,
        Some(&attempt.intent.attempt_id),
    )
    .await?;
    if let Err(error) = store.authorize_dispatch(attempt).await {
        let receipt = dispatch_rejection_receipt(&attempt.intent, &error);
        if receipt == attempt.intent {
            // Uncertain authorization is not a provider outcome. A duplicate caller may
            // still record its actual output; retain the durable intent without creating
            // an inferred final observation that could race and block that factual result.
            return Err(WorkerError::Durable(error));
        }
        let audit = audit_receipt(&attempt.document, &receipt, None)
            .map_err(|_| WorkerError::InvalidReceipt)?;
        // Database observation is authoritative even if a supplementary journal write fails.
        let _ = write_new_json(&directory.join("receipt.json"), &receipt);
        let _ = write_new_json(&directory.join("audit.json"), &audit);
        store.observe_attempt(attempt, &receipt, None, None).await?;
        return Err(WorkerError::Durable(error));
    }
    let provider = config.provider.clone();
    let line = attempt.line.clone();
    let provider_root = root.to_owned();
    let workspace = directory.clone();
    let intent = attempt.intent.clone();
    let fingerprint = attempt.input_fingerprint.clone();
    let outcome = tokio::task::spawn_blocking(move || {
        synthesize(
            &provider,
            &line,
            &intent,
            &fingerprint,
            &provider_root,
            &workspace,
        )
    })
    .await
    .unwrap_or(Err((
        FailureClass::Permanent,
        DeliveryState::MayHaveExecuted,
    )));
    let mut receipt = attempt.intent.clone();
    let bytes = match outcome {
        Ok(bytes) => {
            receipt.usage = UsageObservation::Reported {
                unit: "effective-unicode-scalars".into(),
                count: attempt.line.text.chars().count() as u64,
            };
            receipt.outcome = ReceiptOutcome::Output {
                audio_sha256: format!("{:x}", Sha256::digest(&bytes)),
            };
            Some(bytes)
        }
        Err((class, delivery)) => {
            receipt.outcome = ReceiptOutcome::Failure { class, delivery };
            None
        }
    };
    if matches!(config.provider, WorkerProvider::SyntheticFake) {
        receipt.evidence = ReceiptEvidence::SyntheticFixture;
    }
    let output_path = match persist_provider_output(&directory, attempt, &receipt, bytes.as_deref())
    {
        Ok(path) => path,
        Err(error) => {
            // A local output/journal fault cannot erase possible execution or become NotSent.
            // Keep definitive pre-send failures as recorded; otherwise pause for reconciliation.
            if matches!(receipt.outcome, ReceiptOutcome::Output { .. }) {
                receipt.outcome = ReceiptOutcome::Failure {
                    class: if matches!(error, WorkerError::InvalidReceipt) {
                        FailureClass::MalformedAudio
                    } else {
                        FailureClass::Permanent
                    },
                    delivery: DeliveryState::MayHaveExecuted,
                };
            }
            store.observe_attempt(attempt, &receipt, None, None).await?;
            return Err(error);
        }
    };
    pause_at(
        config,
        Failpoint::AfterProvider,
        &attempt.lease,
        Some(&attempt.intent.attempt_id),
    )
    .await?;
    // This write is deliberately unfenced: a stale/cancelled worker still records what ran.
    store
        .observe_attempt(attempt, &receipt, output_path.as_deref(), bytes.as_deref())
        .await?;
    pause_at(
        config,
        Failpoint::AfterObservation,
        &attempt.lease,
        Some(&attempt.intent.attempt_id),
    )
    .await?;
    if let Some(bytes) = bytes {
        pause_at(
            config,
            Failpoint::BeforeAccept,
            &attempt.lease,
            Some(&attempt.intent.attempt_id),
        )
        .await?;
        if heartbeat.lost() {
            return Err(WorkerError::Durable(DurableError::LeaseLost));
        }
        store
            .accept_step(&attempt.lease, &attempt.intent.attempt_id, &bytes)
            .await?;
        pause_at(
            config,
            Failpoint::AfterAccept,
            &attempt.lease,
            Some(&attempt.intent.attempt_id),
        )
        .await?;
        emit_event("accepted", &attempt.lease, Some(&attempt.intent.attempt_id))
    } else {
        emit_event(
            "failure_recorded",
            &attempt.lease,
            Some(&attempt.intent.attempt_id),
        )
    }
}

fn dispatch_rejection_receipt(intent: &TtsReceipt, error: &DurableError) -> TtsReceipt {
    let mut receipt = intent.clone();
    // The store checks a prior send marker before these authority/fence rejections. They
    // therefore prove this invocation was denied before a send boundary. Conflict, corrupt
    // state and unavailable commit results cannot prove non-execution by a cloned caller.
    let definitely_not_sent = matches!(
        error,
        DurableError::LeaseLost
            | DurableError::Store(
                StoreError::Unauthenticated
                    | StoreError::Forbidden
                    | StoreError::NotFound
                    | StoreError::StaleProductionInputs
                    | StoreError::StaleRevision(_)
                    | StoreError::ProductionBlocked(_)
            )
    );
    if definitely_not_sent {
        receipt.outcome = ReceiptOutcome::Failure {
            class: FailureClass::Cancelled,
            delivery: DeliveryState::NotSent,
        };
    }
    receipt
}

fn persist_provider_output(
    directory: &Path,
    attempt: &AttemptWork,
    receipt: &TtsReceipt,
    bytes: Option<&[u8]>,
) -> Result<Option<String>, WorkerError> {
    let audit = audit_receipt(&attempt.document, receipt, bytes)
        .map_err(|_| WorkerError::InvalidReceipt)?;
    let output_path = if let Some(bytes) = bytes {
        write_new(&directory.join("output.wav"), bytes)?;
        Some(
            directory
                .join("output.wav")
                .to_str()
                .ok_or(WorkerError::UnsafeOutput)?
                .to_owned(),
        )
    } else {
        None
    };
    write_new_json(&directory.join("receipt.json"), receipt)?;
    write_new_json(&directory.join("audit.json"), &audit)?;
    Ok(output_path)
}

type ProviderFailure = (FailureClass, DeliveryState);

fn synthesize(
    provider: &WorkerProvider,
    line: &cantos_contracts::ResolvedSynthesisInput,
    intent: &TtsReceipt,
    fingerprint: &str,
    root: &Path,
    workspace: &Path,
) -> Result<Vec<u8>, ProviderFailure> {
    let request = LocalSynthesisRequest::from_resolved(line)
        .map_err(|_| (FailureClass::UnsupportedControl, DeliveryState::NotSent))?;
    match provider {
        WorkerProvider::LocalEspeak { runtime } => {
            run_local(&request, runtime, workspace, Duration::from_secs(30))
                .map(|output| output.bytes().to_vec())
                .map_err(|error| local_failure(&error))
        }
        WorkerProvider::SyntheticFake => {
            let ledger = root.join(".fake-provider");
            ensure_private_directory(&ledger)
                .map_err(|_| (FailureClass::Permanent, DeliveryState::NotSent))?;
            // This independent create-only ledger is the fake provider's execution oracle.
            // A duplicate operation is refused; it is never counted as a fresh execution.
            write_new_json(
                &ledger.join(format!("{}.json", intent.operation_id)),
                &SyntheticExecution {
                    evidence: "synthetic-fixture-not-provider-live",
                    attempt_id: &intent.attempt_id,
                    operation_id: &intent.operation_id,
                    input_fingerprint: fingerprint,
                },
            )
            .map_err(|_| (FailureClass::Permanent, DeliveryState::MayHaveExecuted))?;
            Ok(synthetic_wav())
        }
    }
}

#[derive(Serialize)]
struct SyntheticExecution<'a> {
    evidence: &'static str,
    attempt_id: &'a str,
    operation_id: &'a str,
    input_fingerprint: &'a str,
}

fn synthetic_wav() -> Vec<u8> {
    let data_size = 440_u32;
    let mut bytes = Vec::with_capacity(44 + data_size as usize);
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&(36 + data_size).to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&22_050_u32.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&data_size.to_le_bytes());
    // Original synthetic PCM, not copied provider speech or an audible-quality assertion.
    for index in 0..220 {
        let sample: i16 = if index % 2 == 0 { 256 } else { -256 };
        bytes.extend_from_slice(&sample.to_le_bytes());
    }
    bytes
}

fn local_failure(error: &LocalRunError) -> ProviderFailure {
    match error {
        LocalRunError::TimedOut {
            phase: LocalPhase::Synthesize,
        } => (FailureClass::Timeout, DeliveryState::MayHaveExecuted),
        LocalRunError::ProcessFailed {
            phase: LocalPhase::Synthesize,
            ..
        }
        | LocalRunError::ProcessIoFailed {
            phase: LocalPhase::Synthesize,
        }
        | LocalRunError::ProcessOutputExceeded {
            phase: LocalPhase::Synthesize,
        }
        | LocalRunError::WorkspaceCleanupFailed => {
            (FailureClass::Permanent, DeliveryState::MayHaveExecuted)
        }
        LocalRunError::Audio(_)
        | LocalRunError::MissingOrUnsafeOutput
        | LocalRunError::OutputReadFailed => {
            (FailureClass::MalformedAudio, DeliveryState::MayHaveExecuted)
        }
        LocalRunError::RuntimeVoiceUnavailable => {
            (FailureClass::UnsupportedVoice, DeliveryState::NotSent)
        }
        LocalRunError::UnsupportedPlatform
        | LocalRunError::InvalidTimeout
        | LocalRunError::InvalidRuntimePath
        | LocalRunError::InvalidWorkspaceRoot
        | LocalRunError::WorkspaceCreationFailed
        | LocalRunError::ProcessStartFailed { .. }
        | LocalRunError::WrongRuntimeVersion
        | LocalRunError::TimedOut {
            phase: LocalPhase::Version | LocalPhase::Voices,
        }
        | LocalRunError::ProcessFailed {
            phase: LocalPhase::Version | LocalPhase::Voices,
            ..
        }
        | LocalRunError::ProcessIoFailed {
            phase: LocalPhase::Version | LocalPhase::Voices,
        }
        | LocalRunError::ProcessOutputExceeded {
            phase: LocalPhase::Version | LocalPhase::Voices,
        } => (FailureClass::Permanent, DeliveryState::NotSent),
    }
}

struct Heartbeat {
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

    fn lost(&self) -> bool {
        self.lost.load(Ordering::Acquire)
    }
}

impl Drop for Heartbeat {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn pause_at(
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

fn emit_event(event: &str, lease: &JobLease, attempt_id: Option<&str>) -> Result<(), WorkerError> {
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

fn emit(value: &impl Serialize) -> Result<(), WorkerError> {
    let mut stdout = std::io::stdout().lock();
    serde_json::to_writer(&mut stdout, value).map_err(|_| WorkerError::OutputIo)?;
    stdout
        .write_all(b"\n")
        .and_then(|()| stdout.flush())
        .map_err(|_| WorkerError::OutputIo)
}

fn prepare_root(path: &Path) -> Result<PathBuf, WorkerError> {
    let metadata = fs::symlink_metadata(path).map_err(|_| WorkerError::UnsafeOutput)?;
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(WorkerError::UnsafeOutput);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(WorkerError::UnsafeOutput);
        }
    }
    fs::canonicalize(path).map_err(|_| WorkerError::UnsafeOutput)
}

fn create_attempt_directory(root: &Path, attempt: &str) -> Result<PathBuf, WorkerError> {
    if uuid::Uuid::parse_str(attempt).is_err() {
        return Err(WorkerError::UnsafeOutput);
    }
    let directory = root.join(attempt);
    create_private_directory(&directory)?;
    Ok(directory)
}

fn create_private_directory(path: &Path) -> Result<(), WorkerError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(|_| WorkerError::OutputIo)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn ensure_private_directory(path: &Path) -> Result<(), WorkerError> {
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => {
            if let Some(parent) = path.parent() {
                sync_directory(parent)?;
            }
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(_) => return Err(WorkerError::OutputIo),
    }
    // Two independent workers may race to initialize this fixed provider ledger directory.
    prepare_root(path).map(|_| ())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), WorkerError> {
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options
            .mode(0o600)
            .custom_flags(libc::O_NOFOLLOW | libc::O_NONBLOCK);
    }
    let mut file = options.open(path).map_err(|_| WorkerError::OutputIo)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| WorkerError::OutputIo)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn write_new_json(path: &Path, value: &impl Serialize) -> Result<(), WorkerError> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|_| WorkerError::OutputIo)?;
    write_new(path, &bytes)
}

fn sync_directory(path: &Path) -> Result<(), WorkerError> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|_| WorkerError::OutputIo)
}

fn read_output(
    root: &Path,
    recorded: &str,
    expected_attempt: &str,
) -> Result<Vec<u8>, WorkerError> {
    let path = Path::new(recorded)
        .strip_prefix(root)
        .map_err(|_| WorkerError::UnsafeOutput)?;
    let components: Vec<_> = path.components().collect();
    if components.len() != 2
        || !matches!(components[0], Component::Normal(_))
        || !matches!(components[1], Component::Normal(name) if name == "output.wav")
    {
        return Err(WorkerError::UnsafeOutput);
    }
    let Some(attempt) = components[0].as_os_str().to_str() else {
        return Err(WorkerError::UnsafeOutput);
    };
    if uuid::Uuid::parse_str(attempt).is_err() || attempt != expected_attempt {
        return Err(WorkerError::UnsafeOutput);
    }
    let file = open_private_output(root, attempt)?;
    let metadata = file.metadata().map_err(|_| WorkerError::OutputIo)?;
    if !metadata.is_file() || metadata.len() > MAX_OUTPUT_BYTES as u64 {
        return Err(WorkerError::UnsafeOutput);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        if metadata.nlink() != 1 {
            return Err(WorkerError::UnsafeOutput);
        }
    }
    let mut bytes = Vec::new();
    file.take(MAX_OUTPUT_BYTES as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| WorkerError::OutputIo)?;
    if bytes.len() > MAX_OUTPUT_BYTES {
        return Err(WorkerError::UnsafeOutput);
    }
    Ok(bytes)
}

#[cfg(unix)]
fn open_private_output(root: &Path, attempt: &str) -> Result<File, WorkerError> {
    use std::{
        ffi::CString,
        os::fd::{AsRawFd, FromRawFd},
        os::unix::fs::OpenOptionsExt,
    };
    let root = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_DIRECTORY | libc::O_NOFOLLOW)
        .open(root)
        .map_err(private_open_error)?;
    let attempt = CString::new(attempt).map_err(|_| WorkerError::UnsafeOutput)?;
    // Directory descriptors prevent an ancestor replacement from redirecting the open.
    let descriptor = unsafe {
        libc::openat(
            root.as_raw_fd(),
            attempt.as_ptr(),
            libc::O_RDONLY | libc::O_DIRECTORY | libc::O_NOFOLLOW | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(private_open_error(std::io::Error::last_os_error()));
    }
    let directory = unsafe { File::from_raw_fd(descriptor) };
    let descriptor = unsafe {
        libc::openat(
            directory.as_raw_fd(),
            c"output.wav".as_ptr(),
            libc::O_RDONLY | libc::O_NOFOLLOW | libc::O_NONBLOCK | libc::O_CLOEXEC,
        )
    };
    if descriptor < 0 {
        return Err(private_open_error(std::io::Error::last_os_error()));
    }
    Ok(unsafe { File::from_raw_fd(descriptor) })
}

#[cfg(unix)]
fn private_open_error(error: std::io::Error) -> WorkerError {
    if matches!(error.raw_os_error(), Some(libc::ELOOP | libc::ENOTDIR)) {
        WorkerError::UnsafeOutput
    } else {
        // Missing bytes, permission denial and I/O faults are unavailable, not proof of
        // malformed audio. Never preserve the OS message, which can include private paths.
        WorkerError::OutputIo
    }
}

#[cfg(not(unix))]
fn open_private_output(_root: &Path, _attempt: &str) -> Result<File, WorkerError> {
    // The local adapter supports Unix only; a weaker pathname open cannot prove equivalence.
    Err(WorkerError::UnsafeOutput)
}

fn valid_identifier(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.' | b':'))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tts::audio::{inspect_output_for_profile, OutputProfile};

    #[test]
    fn recurring_worker_isolates_only_expected_job_boundary_errors() {
        for error in [
            DurableError::LeaseLost,
            DurableError::ReconciliationRequired,
            DurableError::Store(StoreError::Unauthenticated),
            DurableError::Store(StoreError::Forbidden),
            DurableError::Store(StoreError::NotFound),
            DurableError::Store(StoreError::StaleProductionInputs),
            DurableError::Store(StoreError::StaleRevision(2)),
            DurableError::Store(StoreError::ProductionBlocked(Vec::new())),
        ] {
            assert!(expected_job_boundary_error(&WorkerError::Durable(error)));
        }
        for error in [
            WorkerError::InvalidConfiguration,
            WorkerError::UnsafeOutput,
            WorkerError::OutputIo,
            WorkerError::InvalidReceipt,
            WorkerError::FailpointInput,
            WorkerError::Durable(DurableError::Corrupt),
            WorkerError::Durable(DurableError::Conflict),
            WorkerError::Durable(DurableError::InvalidRequest),
            WorkerError::Durable(DurableError::ProviderDisabled),
            WorkerError::Durable(DurableError::Store(StoreError::CorruptRevision)),
            WorkerError::Durable(DurableError::Store(StoreError::Unavailable)),
        ] {
            assert!(!expected_job_boundary_error(&error), "{error:?}");
        }
    }

    #[test]
    fn duplicate_or_uncertain_authorization_cannot_become_definitive_not_sent() {
        use crate::tts::{
            attempt::{ContractCapability, ReconciliationCapabilities},
            local_contract, ChargeObservation, TtsProviderIdentity, RECEIPT_VERSION,
        };
        let intent = TtsReceipt {
            version: RECEIPT_VERSION.into(),
            evidence: ReceiptEvidence::SyntheticFixture,
            attempt_id: uuid::Uuid::new_v4().to_string(),
            operation_id: uuid::Uuid::new_v4().to_string(),
            provider_request_id: None,
            idempotency_key: None,
            input_digest: "synthetic-input-digest".into(),
            dialogue_id: "dialogue-01".into(),
            provider: TtsProviderIdentity {
                provider_id: local_contract::PROVIDER_ID.into(),
                model_id: local_contract::MODEL_ID.into(),
                voice_id: "vi".into(),
                adapter_version: local_contract::ADAPTER_VERSION.into(),
            },
            reconciliation: ReconciliationCapabilities::new(
                ContractCapability::Unavailable,
                ContractCapability::Unavailable,
                ContractCapability::Unavailable,
            ),
            usage: UsageObservation::Unknown {},
            charge: ChargeObservation::Unknown {},
            outcome: ReceiptOutcome::Failure {
                class: FailureClass::Timeout,
                delivery: DeliveryState::MayHaveExecuted,
            },
        };
        for error in [
            DurableError::Conflict,
            DurableError::ReconciliationRequired,
            DurableError::Corrupt,
            DurableError::Store(StoreError::Unavailable),
            DurableError::Store(StoreError::CorruptRevision),
            DurableError::InvalidRequest,
            DurableError::ProviderDisabled,
            DurableError::BudgetExhausted,
        ] {
            assert_eq!(
                dispatch_rejection_receipt(&intent, &error),
                intent,
                "{error:?}"
            );
        }
        let mut denied = intent.clone();
        denied.outcome = ReceiptOutcome::Failure {
            class: FailureClass::Cancelled,
            delivery: DeliveryState::NotSent,
        };
        for error in [
            DurableError::LeaseLost,
            DurableError::Store(StoreError::Unauthenticated),
            DurableError::Store(StoreError::Forbidden),
            DurableError::Store(StoreError::NotFound),
            DurableError::Store(StoreError::StaleProductionInputs),
            DurableError::Store(StoreError::StaleRevision(2)),
            DurableError::Store(StoreError::ProductionBlocked(Vec::new())),
        ] {
            assert_eq!(
                dispatch_rejection_receipt(&intent, &error),
                denied,
                "{error:?}"
            );
        }
    }

    struct PrivateRoot(PathBuf);

    impl PrivateRoot {
        fn new() -> Self {
            let root = std::env::temp_dir().join(format!("cantos-worker-{}", uuid::Uuid::new_v4()));
            create_private_directory(&root).unwrap();
            Self(fs::canonicalize(root).unwrap())
        }

        fn output(&self, attempt: &str) -> PathBuf {
            let directory = create_attempt_directory(&self.0, attempt).unwrap();
            let output = directory.join("output.wav");
            write_new(&output, &synthetic_wav()).unwrap();
            output
        }
    }

    impl Drop for PrivateRoot {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn synthetic_output_is_original_bounded_native_pcm() {
        let bytes = synthetic_wav();
        let checksum = format!("{:x}", Sha256::digest(&bytes));
        let inspected =
            inspect_output_for_profile(&bytes, &checksum, OutputProfile::Espeak22Khz).unwrap();
        assert_eq!(inspected.frames(), 220);
        assert_eq!(inspected.sample_rate_hz(), 22_050);
        assert_eq!(inspected.channels(), 1);
        assert_eq!(inspected.bits_per_sample(), 16);
    }

    #[test]
    fn concurrent_synthetic_requests_initialize_one_private_ledger_and_both_execute() {
        use crate::tts::{
            attempt::{ContractCapability, ReconciliationCapabilities},
            local_contract, ChargeObservation, TtsProviderIdentity, RECEIPT_VERSION,
        };
        use cantos_contracts::{ResolvedSynthesisInput, SynthesisPerformance};
        let root = PrivateRoot::new();
        let line = ResolvedSynthesisInput {
            dialogue_id: "dialogue-01".into(),
            character_id: "narrator".into(),
            text: "Ánh đèn cuối cùng còn sáng trên sân khấu.".into(),
            language: "vi".into(),
            provider_id: local_contract::PROVIDER_ID.into(),
            model_id: local_contract::MODEL_ID.into(),
            voice_id: "vi".into(),
            voice_rights_record_id: "synthetic-voice-claim".into(),
            adapter_version: local_contract::ADAPTER_VERSION.into(),
            output_contract: local_contract::OUTPUT_CONTRACT.into(),
            performance: SynthesisPerformance {
                rate_permille: 1_000,
                pitch_semitones: 0,
                emotion: Some("neutral".into()),
                intensity_permille: Some(0),
            },
            pronunciation: Vec::new(),
        };
        let barrier = std::sync::Barrier::new(2);
        let operations = std::thread::scope(|scope| {
            let mut handles = Vec::new();
            for _ in 0..2 {
                handles.push(scope.spawn(|| {
                    let intent = TtsReceipt {
                        version: RECEIPT_VERSION.into(),
                        evidence: ReceiptEvidence::SyntheticFixture,
                        attempt_id: uuid::Uuid::new_v4().to_string(),
                        operation_id: uuid::Uuid::new_v4().to_string(),
                        provider_request_id: None,
                        idempotency_key: None,
                        input_digest: "synthetic-input-digest".into(),
                        dialogue_id: line.dialogue_id.clone(),
                        provider: TtsProviderIdentity {
                            provider_id: line.provider_id.clone(),
                            model_id: line.model_id.clone(),
                            voice_id: line.voice_id.clone(),
                            adapter_version: line.adapter_version.clone(),
                        },
                        reconciliation: ReconciliationCapabilities::new(
                            ContractCapability::Unavailable,
                            ContractCapability::Unavailable,
                            ContractCapability::Unavailable,
                        ),
                        usage: UsageObservation::Unknown {},
                        charge: ChargeObservation::Unknown {},
                        outcome: ReceiptOutcome::Failure {
                            class: FailureClass::Timeout,
                            delivery: DeliveryState::MayHaveExecuted,
                        },
                    };
                    barrier.wait();
                    assert_eq!(
                        synthesize(
                            &WorkerProvider::SyntheticFake,
                            &line,
                            &intent,
                            "synthetic-fingerprint",
                            &root.0,
                            &root.0
                        ),
                        Ok(synthetic_wav()),
                    );
                    intent.operation_id
                }));
            }
            handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect::<Vec<_>>()
        });
        let ledger = root.0.join(".fake-provider");
        assert_eq!(fs::read_dir(&ledger).unwrap().count(), 2);
        for operation in operations {
            let record: serde_json::Value = serde_json::from_slice(
                &fs::read(ledger.join(format!("{operation}.json"))).unwrap(),
            )
            .unwrap();
            assert_eq!(record["operation_id"], operation);
            assert_eq!(record["input_fingerprint"], "synthetic-fingerprint");
            assert_eq!(record["evidence"], "synthetic-fixture-not-provider-live");
        }
    }

    #[test]
    fn recovery_requires_exact_attempt_in_configured_private_root() {
        let root = PrivateRoot::new();
        let attempt = uuid::Uuid::new_v4().to_string();
        let output = root.output(&attempt);
        let output_path = output.to_str().unwrap();
        assert_eq!(
            read_output(&root.0, output_path, &attempt).unwrap(),
            synthetic_wav()
        );
        let another = uuid::Uuid::new_v4().to_string();
        assert!(matches!(
            read_output(&root.0, output_path, &another),
            Err(WorkerError::UnsafeOutput)
        ));
        assert!(matches!(
            read_output(&root.0, "/tmp/other/output.wav", &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
        assert!(matches!(
            read_output(
                &root.0,
                &format!("{}/../{attempt}/output.wav", root.0.display()),
                &attempt
            ),
            Err(WorkerError::UnsafeOutput)
        ));
        assert!(matches!(
            write_new(&output, b"overwrite"),
            Err(WorkerError::OutputIo)
        ));
        assert_eq!(fs::read(&output).unwrap(), synthetic_wav());
        fs::remove_file(&output).unwrap();
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::OutputIo)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn recovery_rejects_symlinks_hardlinks_and_nonregular_outputs() {
        use std::os::unix::fs::symlink;

        let root = PrivateRoot::new();
        let attempt = uuid::Uuid::new_v4().to_string();
        let directory = create_attempt_directory(&root.0, &attempt).unwrap();
        let original = root.0.join("original.wav");
        write_new(&original, &synthetic_wav()).unwrap();
        let output = directory.join("output.wav");
        let output_path = output.to_str().unwrap();
        symlink(&original, &output).unwrap();
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
        fs::remove_file(&output).unwrap();
        fs::hard_link(&original, &output).unwrap();
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
        fs::remove_file(&output).unwrap();
        fs::create_dir(&output).unwrap();
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
        fs::remove_dir(&output).unwrap();
        let fifo = std::ffi::CString::new(output_path).unwrap();
        assert_eq!(unsafe { libc::mkfifo(fifo.as_ptr(), 0o600) }, 0);
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
        fs::remove_file(&output).unwrap();
        let file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output)
            .unwrap();
        file.set_len(MAX_OUTPUT_BYTES as u64 + 1).unwrap();
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
        drop(file);
        fs::remove_file(&output).unwrap();
        fs::remove_dir(&directory).unwrap();
        symlink(&root.0, &directory).unwrap();
        assert!(matches!(
            read_output(&root.0, output_path, &attempt),
            Err(WorkerError::UnsafeOutput)
        ));
    }
}
