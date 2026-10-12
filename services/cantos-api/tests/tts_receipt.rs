use std::{fs, process::Command};

use cantos_api::contexts::production::prepare_inputs;
use cantos_api::contexts::synthesis::attempt::{
    ContractCapability, DeliveryState, FailureClass, ReconciliationCapabilities, RecoveryPlan,
};
use cantos_api::contexts::synthesis::receipt::{
    audit_receipt, decode_document, decode_receipt, ChargeObservation, ReceiptError,
    ReceiptEvidence, ReceiptOutcome, TtsProviderIdentity, TtsReceipt, UsageObservation,
    MAX_RECEIPT_BYTES, RECEIPT_VERSION,
};
use cantos_api::script_ir::read_script;
use cantos_contracts::{
    FrozenProductionDocument, ProductionSettingsResponse, RevisionResponse,
    SaveProductionSettingsRequest,
};
use serde_json::json;
use sha2::{Digest, Sha256};

fn document() -> FrozenProductionDocument {
    let script = read_script(include_bytes!(
        "../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    ))
    .unwrap();
    let script_json = String::from_utf8(script.export_bytes()).unwrap();
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(script_json.as_bytes());
    let revision = RevisionResponse {
        script_id: "00000000-0000-4000-8000-000000000005".into(),
        revision: 1,
        accepted_by: "owner".into(),
        accepted_at: "2026-10-10T00:00:00Z".into(),
        content_digest: script.content_digest().to_string(),
        export_digest: format!("sir-e1:sha256:{:x}", hash.finalize()),
        script_json,
    };
    let request: SaveProductionSettingsRequest = serde_json::from_slice(include_bytes!(
        "../../../packages/contracts/fixtures/production/v1/settings-request.json"
    ))
    .unwrap();
    let settings = ProductionSettingsResponse {
        script_id: revision.script_id.clone(),
        version: 1,
        script_revision: 1,
        operation_id: request.operation_id,
        recorded_by: "owner".into(),
        recorded_at: "2026-10-10T00:01:00Z".into(),
        settings: request.settings,
    };
    // Missing rights deliberately leave this planning document ineligible for authorization.
    prepare_inputs("owner", &revision, &settings, &[], true, 0)
        .unwrap()
        .document()
}

#[cfg(unix)]
fn local_document() -> FrozenProductionDocument {
    let base = document();
    let mut settings = base.settings.clone();
    for binding in &mut settings.settings.bindings {
        binding.provider_id = "espeak-ng-local".into();
        binding.model_id = "espeak-ng-1.52.0".into();
        binding.voice_rights_record_id = format!("local_voice_{}", binding.character_id);
        binding.voice_id = match binding.character_id.as_str() {
            "narrator" => "vi",
            "an" => "vi-vn-x-central",
            "minh" => "vi-vn-x-south",
            _ => panic!("unexpected fixture role"),
        }
        .into();
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    prepare_inputs(&base.owner_id, &base.revision, &settings, &[], true, 0)
        .unwrap()
        .document()
}

fn receipt(document: &FrozenProductionDocument) -> TtsReceipt {
    let line = &document.resolved[0];
    TtsReceipt {
        version: RECEIPT_VERSION.into(),
        evidence: ReceiptEvidence::SyntheticFixture,
        attempt_id: "synthetic-attempt-1".into(),
        operation_id: "synthetic-operation-1".into(),
        provider_request_id: None,
        idempotency_key: Some("synthetic-key-1".into()),
        input_digest: document.input_digest.clone(),
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
    }
}

fn wav() -> Vec<u8> {
    let mut bytes = b"RIFF".to_vec();
    bytes.extend(38_u32.to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(24_000_u32.to_le_bytes());
    bytes.extend(48_000_u32.to_le_bytes());
    bytes.extend(2_u16.to_le_bytes());
    bytes.extend(16_u16.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend(2_u32.to_le_bytes());
    bytes.extend(12_i16.to_le_bytes());
    bytes
}

#[test]
fn unknown_charges_and_attempt_identifiers_survive_audit_and_restart_serialization() {
    let document = document();
    let receipt = receipt(&document);
    let reopened = decode_receipt(&serde_json::to_vec(&receipt).unwrap()).unwrap();
    let audit = audit_receipt(&document, &reopened, None).unwrap();
    assert_eq!(audit.receipt(), &receipt);
    assert_eq!(audit.recovery(), Some(&RecoveryPlan::ManualReconciliation));
    let report = serde_json::to_value(audit).unwrap();
    assert_eq!(report["receipt"]["charge"], json!({"status":"unknown"}));
    assert_eq!(
        report["assessment"],
        "offline-inspection-only-no-dispatch-or-acceptance"
    );
    assert!(!report.to_string().contains("script_json"));
}

#[test]
fn receipt_cannot_attach_to_another_snapshot_dialogue_or_voice() {
    let document = document();
    let original = receipt(&document);
    let mut changed = original.clone();
    changed.input_digest.push('0');
    assert_eq!(
        audit_receipt(&document, &changed, None).unwrap_err(),
        ReceiptError::InputDigestMismatch
    );
    changed = original.clone();
    changed.dialogue_id = "invented-line".into();
    assert_eq!(
        audit_receipt(&document, &changed, None).unwrap_err(),
        ReceiptError::DialogueMissing
    );
    for field in ["provider", "model", "voice", "adapter"] {
        changed = original.clone();
        match field {
            "provider" => changed.provider.provider_id = "other".into(),
            "model" => changed.provider.model_id = "other".into(),
            "voice" => changed.provider.voice_id = "other".into(),
            "adapter" => changed.provider.adapter_version = "other".into(),
            _ => unreachable!(),
        }
        assert_eq!(
            audit_receipt(&document, &changed, None).unwrap_err(),
            ReceiptError::ProviderIdentityMismatch
        );
    }
}

#[test]
fn forged_export_and_resolved_controls_fail_even_when_receipt_digest_matches() {
    let original = document();
    let receipt = receipt(&original);
    let mut changed = original.clone();
    changed.resolved[0].text.push('x');
    assert_eq!(
        audit_receipt(&changed, &receipt, None).unwrap_err(),
        ReceiptError::InvalidDocument
    );
    changed = original;
    changed.revision.export_digest.push('0');
    assert_eq!(
        audit_receipt(&changed, &receipt, None).unwrap_err(),
        ReceiptError::InvalidDocument
    );
}

#[test]
fn synthetic_output_is_hash_bound_but_never_promoted_to_real_synthesis() {
    let document = document();
    let mut receipt = receipt(&document);
    let bytes = wav();
    receipt.outcome = ReceiptOutcome::Output {
        audio_sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    let audit = audit_receipt(&document, &receipt, Some(&bytes)).unwrap();
    assert_eq!(audit.audio().unwrap().frames(), 1);
    assert_eq!(
        audit.audio().unwrap().file_sha256(),
        format!("{:x}", Sha256::digest(&bytes))
    );
    assert_eq!(audit.recovery(), None);
    assert_eq!(
        audit_receipt(&document, &receipt, None).unwrap_err(),
        ReceiptError::AudioRequired
    );
    receipt.evidence = ReceiptEvidence::CallerReported;
    assert_eq!(
        audit_receipt(&document, &receipt, Some(&bytes)).unwrap_err(),
        ReceiptError::ReferenceOnlyOutputClaim
    );
    receipt.outcome = ReceiptOutcome::Failure {
        class: FailureClass::Permanent,
        delivery: DeliveryState::ConfirmedNotExecuted,
    };
    assert_eq!(
        audit_receipt(&document, &receipt, Some(&bytes)).unwrap_err(),
        ReceiptError::AudioUnexpected
    );
}

#[test]
fn strict_wire_bounds_versions_and_money_do_not_default_to_free() {
    let document = document();
    let mut receipt = receipt(&document);
    assert_eq!(
        decode_receipt(&vec![b' '; MAX_RECEIPT_BYTES + 1]).unwrap_err(),
        ReceiptError::ReceiptTooLarge
    );
    assert_eq!(
        decode_document(b"{}").unwrap_err(),
        ReceiptError::InvalidDocumentJson
    );
    let mut value = serde_json::to_value(&receipt).unwrap();
    value["extra"] = json!(1);
    assert_eq!(
        decode_receipt(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
        ReceiptError::InvalidReceiptJson
    );
    for field in ["usage", "charge", "provider", "outcome"] {
        let mut value = serde_json::to_value(&receipt).unwrap();
        value[field]["invented"] = json!(1);
        assert_eq!(
            decode_receipt(&serde_json::to_vec(&value).unwrap()).unwrap_err(),
            ReceiptError::InvalidReceiptJson,
            "unknown field at {field}"
        );
    }
    receipt.version = "future-version".into();
    assert_eq!(
        audit_receipt(&document, &receipt, None).unwrap_err(),
        ReceiptError::UnsupportedVersion
    );
    receipt.version = RECEIPT_VERSION.into();
    receipt.attempt_id = "\nprivate".into();
    assert_eq!(
        audit_receipt(&document, &receipt, None).unwrap_err(),
        ReceiptError::InvalidField {
            field: "attempt_id"
        }
    );
    receipt.attempt_id = "attempt-1".into();
    receipt.charge = ChargeObservation::Reported {
        currency: cantos_contracts::ProductionCurrency::USD,
        amount_minor: u64::MAX,
        reference: "claim-1".into(),
    };
    assert_eq!(
        audit_receipt(&document, &receipt, None).unwrap_err(),
        ReceiptError::InvalidField {
            field: "charge.amount_minor"
        }
    );
    receipt.charge = ChargeObservation::Unknown {};
    receipt.usage = UsageObservation::Reported {
        unit: "unicode-scalars".into(),
        count: 4,
    };
    assert_eq!(
        audit_receipt(&document, &receipt, None)
            .unwrap()
            .receipt()
            .charge,
        ChargeObservation::Unknown {}
    );
}

#[test]
fn cli_inspects_local_failure_and_synthetic_output_with_structured_exit_codes() {
    let directory = std::env::temp_dir().join(format!("cantos-tts-audit-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let document = document();
    let mut receipt = receipt(&document);
    let document_path = directory.join("document.json");
    let receipt_path = directory.join("receipt.json");
    let wav_path = directory.join("output.wav");
    fs::write(&document_path, serde_json::to_vec(&document).unwrap()).unwrap();
    fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let binary = env!("CARGO_BIN_EXE_inspect-tts-receipt");
    let output = Command::new(binary)
        .args([&document_path, &receipt_path])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["recovery"]["action"], "manual_reconciliation");
    let bytes = wav();
    fs::write(&wav_path, &bytes).unwrap();
    receipt.outcome = ReceiptOutcome::Output {
        audio_sha256: format!("{:x}", Sha256::digest(&bytes)),
    };
    fs::write(&receipt_path, serde_json::to_vec(&receipt).unwrap()).unwrap();
    let output = Command::new(binary)
        .args([&document_path, &receipt_path, &wav_path])
        .output()
        .unwrap();
    assert!(output.status.success());
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["audio"]["frames"], 1);
    fs::write(&wav_path, b"damaged").unwrap();
    let output = Command::new(binary)
        .args([&document_path, &receipt_path, &wav_path])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    let report: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(report["code"], "invalid_audio");
    fs::write(&receipt_path, b"private-invalid-json").unwrap();
    let output = Command::new(binary)
        .args([&document_path, &receipt_path])
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(1));
    assert!(!String::from_utf8_lossy(&output.stdout).contains("private-invalid"));
    assert_eq!(
        Command::new(binary)
            .arg("missing-input")
            .output()
            .unwrap()
            .status
            .code(),
        Some(2)
    );
    fs::remove_dir_all(directory).unwrap();
}

#[cfg(unix)]
#[test]
fn cli_rejects_fifo_without_waiting_for_a_writer() {
    use std::{
        thread,
        time::{Duration, Instant},
    };
    let directory = std::env::temp_dir().join(format!("cantos-tts-fifo-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    let fifo = directory.join("input.fifo");
    assert!(Command::new("mkfifo")
        .arg(&fifo)
        .status()
        .unwrap()
        .success());
    let mut child = Command::new(env!("CARGO_BIN_EXE_inspect-tts-receipt"))
        .arg(&fifo)
        .arg("unused-receipt")
        .spawn()
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let status = loop {
        if let Some(status) = child.try_wait().unwrap() {
            break Some(status);
        }
        if Instant::now() >= deadline {
            child.kill().unwrap();
            child.wait().unwrap();
            break None;
        }
        thread::sleep(Duration::from_millis(10));
    };
    fs::remove_dir_all(directory).unwrap();
    assert_eq!(
        status
            .expect("FIFO must be rejected without blocking")
            .code(),
        Some(2)
    );
}

#[cfg(unix)]
#[test]
fn local_probe_persists_conservative_intent_before_dispatch_and_observed_failures() {
    use std::os::unix::fs::{DirBuilderExt, PermissionsExt};
    let directory =
        std::env::temp_dir().join(format!("cantos-probe-failures-{}", uuid::Uuid::new_v4()));
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let directory = fs::canonicalize(directory).unwrap();
    let document = local_document();
    let input = directory.join("document.json");
    fs::write(&input, serde_json::to_vec(&document).unwrap()).unwrap();
    for (case, version, expected_delivery) in [
        ("version", "1.51.0", DeliveryState::NotSent),
        ("synthesis", "1.52.0", DeliveryState::MayHaveExecuted),
    ] {
        let output_directory = directory.join(case);
        let runtime = directory.join(format!("fake-{case}"));
        // The fake verifies intent exists on its first invocation, before even preflight.
        // It fails if the real CLI moves the durable write after child dispatch.
        let script = format!(
            "#!/bin/sh\n[ -f '{}/intent.json' ] || exit 97\ncase \"$1\" in\n--version) printf 'eSpeak NG text-to-speech: {version} Data at: fixture\\n'; exit 0;;\n--voices=vi) printf 'Pty Language Age/Gender VoiceName File\\n5 vi --/M Synthetic aav/vi\\n'; exit 0;;\nesac\nprintf 'observed' > '{}/synthesis-called'\n/bin/cat > /dev/null\nexit 7\n",
            output_directory.display(), directory.display()
        );
        fs::write(&runtime, script).unwrap();
        fs::set_permissions(&runtime, fs::Permissions::from_mode(0o700)).unwrap();
        let output = Command::new(env!("CARGO_BIN_EXE_probe-local-tts"))
            .arg(&input)
            .arg("dialogue-01")
            .arg(&runtime)
            .arg(&output_directory)
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(1));
        let intent_bytes = fs::read(output_directory.join("intent.json")).unwrap();
        let intent = decode_receipt(&intent_bytes).unwrap();
        assert_eq!(
            intent.outcome,
            ReceiptOutcome::Failure {
                class: FailureClass::Timeout,
                delivery: DeliveryState::MayHaveExecuted
            }
        );
        assert_eq!(intent.charge, ChargeObservation::Unknown {});
        let observation_bytes = fs::read(output_directory.join("receipt.json")).unwrap();
        let observed = decode_receipt(&observation_bytes).unwrap();
        assert_eq!(
            observed.outcome,
            ReceiptOutcome::Failure {
                class: FailureClass::Permanent,
                delivery: expected_delivery
            }
        );
        assert_eq!(observed.attempt_id, intent.attempt_id);
        assert_eq!(observed.operation_id, intent.operation_id);
        assert_eq!(observed.charge, ChargeObservation::Unknown {});
        assert!(audit_receipt(&document, &observed, None).is_ok());
        assert!(!output_directory.join("output.wav").exists());
        assert_eq!(
            directory.join("synthesis-called").exists(),
            case == "synthesis"
        );
        assert!(!Command::new(env!("CARGO_BIN_EXE_probe-local-tts"))
            .arg(&input)
            .arg("dialogue-01")
            .arg(&runtime)
            .arg(&output_directory)
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(
            fs::read(output_directory.join("intent.json")).unwrap(),
            intent_bytes
        );
        assert_eq!(
            fs::read(output_directory.join("receipt.json")).unwrap(),
            observation_bytes
        );
    }
    fs::remove_dir_all(directory).unwrap();
}

#[test]
#[ignore = "explicit installed local eSpeak NG runtime and synthetic sample authorization required"]
#[cfg(unix)]
fn installed_local_runtime_generates_three_snapshot_bound_vietnamese_samples() {
    use std::os::unix::fs::DirBuilderExt;
    let runtime =
        std::env::var("CANTOS_LOCAL_TTS_RUNTIME").expect("explicit local runtime path required");
    let document = local_document();
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/tts-evidence");
    fs::create_dir_all(&root).unwrap();
    let directory = root.join(uuid::Uuid::new_v4().to_string());
    fs::DirBuilder::new()
        .mode(0o700)
        .create(&directory)
        .unwrap();
    let document_path = directory.join("frozen-document.json");
    fs::write(
        &document_path,
        serde_json::to_vec_pretty(&document).unwrap(),
    )
    .unwrap();
    let mut samples = Vec::new();
    for line in document.resolved.iter().take(3) {
        let output_directory = directory.join(&line.dialogue_id);
        let output = Command::new(env!("CARGO_BIN_EXE_probe-local-tts"))
            .arg(&document_path)
            .arg(&line.dialogue_id)
            .arg(&runtime)
            .arg(&output_directory)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        let bytes = fs::read(output_directory.join("output.wav")).unwrap();
        let receipt =
            decode_receipt(&fs::read(output_directory.join("receipt.json")).unwrap()).unwrap();
        let audit = audit_receipt(&document, &receipt, Some(&bytes)).unwrap();
        assert_eq!(receipt.evidence, ReceiptEvidence::CallerReported);
        assert_eq!(receipt.provider.voice_id, line.voice_id);
        let audio = audit.audio().unwrap();
        assert!(audio.frames() > 0);
        assert_eq!(audio.sample_rate_hz(), 22_050);
        assert_eq!(audio.channels(), 1);
        assert_eq!(audio.bits_per_sample(), 16);
        assert_eq!(audio.file_sha256(), format!("{:x}", Sha256::digest(&bytes)));
        // Repeating the exact destination must not rerender or overwrite its intent/result.
        assert!(!Command::new(env!("CARGO_BIN_EXE_probe-local-tts"))
            .arg(&document_path)
            .arg(&line.dialogue_id)
            .arg(&runtime)
            .arg(&output_directory)
            .output()
            .unwrap()
            .status
            .success());
        assert_eq!(
            fs::read(output_directory.join("output.wav")).unwrap(),
            bytes
        );
        samples.push(json!({"dialogue_id":line.dialogue_id,"character_id":line.character_id,"voice_id":line.voice_id,"audio":audio,"bytes":bytes.len(),"path":output_directory.join("output.wav")}));
    }
    let evidence = json!({"level":"provider-live-tested","provider":"espeak-ng-local","model":"espeak-ng-1.52.0","adapter":"espeak-ng-local-v1","input_digest":document.input_digest,"script_export_digest":document.revision.export_digest,"samples":samples,"audio_listened":"NOT_RUN","rights":"original nonprivate repository sample; no publication approval","commercial_calls":0,"resource_cost":"unmeasured"});
    fs::write(
        directory.join("evidence.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
    println!(
        "local TTS evidence: {}",
        directory.join("evidence.json").display()
    );
}
