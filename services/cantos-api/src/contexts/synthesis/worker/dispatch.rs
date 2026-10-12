//! Durable dispatch intent, provider execution and append-only observation before acceptance.
use super::output::{
    create_attempt_directory, ensure_private_directory, write_new, write_new_json,
};
use super::runtime::{emit_event, pause_at, Heartbeat};
use super::{Failpoint, WorkerConfig, WorkerError, WorkerProvider};
use crate::{
    contexts::synthesis::{
        attempt::{DeliveryState, FailureClass},
        providers::local::{run_local, LocalPhase, LocalRunError, LocalSynthesisRequest},
        receipt::{audit_receipt, ReceiptEvidence, ReceiptOutcome, TtsReceipt, UsageObservation},
        AttemptWork, DurableError,
    },
    platform::postgres::{Store, StoreError},
};
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::{path::Path, time::Duration};

pub(super) async fn dispatch(
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

pub(super) fn dispatch_rejection_receipt(intent: &TtsReceipt, error: &DurableError) -> TtsReceipt {
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

pub(super) fn synthesize(
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

pub(super) fn synthetic_wav() -> Vec<u8> {
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
