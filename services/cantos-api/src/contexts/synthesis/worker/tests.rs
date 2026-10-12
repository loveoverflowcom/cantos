use super::dispatch::{dispatch_rejection_receipt, synthesize, synthetic_wav};
use super::output::{create_attempt_directory, create_private_directory, read_output, write_new};
use super::runtime::expected_job_boundary_error;
use super::{Failpoint, WorkerError, WorkerProvider};
use crate::contexts::synthesis::media::audio::{inspect_output_for_profile, OutputProfile};
use crate::{
    contexts::synthesis::{
        attempt::{DeliveryState, FailureClass},
        media::audio::MAX_OUTPUT_BYTES,
        receipt::{ReceiptEvidence, ReceiptOutcome, TtsReceipt, UsageObservation},
        DurableError,
    },
    platform::postgres::StoreError,
};
use sha2::{Digest, Sha256};
use std::{
    fs::{self, OpenOptions},
    path::PathBuf,
};

#[test]
fn every_failpoint_round_trips_through_its_cli_spelling() {
    let all = [
        Failpoint::AfterClaim,
        Failpoint::BeforeReuse,
        Failpoint::AfterIntent,
        Failpoint::AfterProvider,
        Failpoint::AfterObservation,
        Failpoint::BeforeAccept,
        Failpoint::AfterAccept,
    ];
    for point in all {
        assert_eq!(Failpoint::parse(point.as_str()), Some(point));
    }
    assert_eq!(Failpoint::parse("before_reuse "), None);
    assert_eq!(Failpoint::parse("after_reuse"), None);
}

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
    use crate::contexts::production::catalog::local as local_contract;
    use crate::contexts::synthesis::{
        attempt::{ContractCapability, ReconciliationCapabilities},
        receipt::{ChargeObservation, TtsProviderIdentity, RECEIPT_VERSION},
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
    use crate::contexts::production::catalog::local as local_contract;
    use crate::contexts::synthesis::{
        attempt::{ContractCapability, ReconciliationCapabilities},
        receipt::{ChargeObservation, TtsProviderIdentity, RECEIPT_VERSION},
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
        let record: serde_json::Value =
            serde_json::from_slice(&fs::read(ledger.join(format!("{operation}.json"))).unwrap())
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
