use std::{fs, process::Command, time::Instant};

use cantos_api::{
    FieldIssue, FrozenProductionDocument, ProductionPronunciation, ProductionSettingsResponse,
    RevisionResponse, SaveProductionSettingsRequest,
};
use cantos_server::production::prepare_inputs;
use cantos_server::script_ir::read_script;
use cantos_server::tts::attempt::{ContractCapability, ReconciliationCapabilities};
use cantos_server::tts::neural_contract;
use cantos_server::tts::{
    audit_receipt, ChargeObservation, ReceiptError, ReceiptEvidence, ReceiptOutcome,
    TtsProviderIdentity, TtsReceipt, UsageObservation, RECEIPT_VERSION,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

fn samples() -> Value {
    serde_json::from_slice(include_bytes!(
        "../../../scripts/fixtures/neural-vietnamese.json"
    ))
    .unwrap()
}

fn document(text: &str, voice: &str) -> FrozenProductionDocument {
    let mut export: Value = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    ))
    .unwrap();
    export["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!(text);
    // The research adapter admits basic punctuation. Keep this fixture's
    // separate line explicit rather than normalizing an existing snapshot.
    export["episode"]["acts"][0]["scenes"][1]["dialogues"][0]["text"] =
        json!("Vọng Đài vẫn sáng... Hẹn gặp lại!");
    let script = read_script(&serde_json::to_vec(&export).unwrap()).unwrap();
    let script_json = String::from_utf8(script.export_bytes()).unwrap();
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(script_json.as_bytes());
    let revision = RevisionResponse {
        script_id: "00000000-0000-4000-8000-000000000006".into(),
        revision: 1,
        accepted_by: "owner".into(),
        accepted_at: "2026-10-11T00:00:00Z".into(),
        content_digest: script.content_digest().to_string(),
        export_digest: format!("sir-e1:sha256:{:x}", hash.finalize()),
        script_json,
    };
    let request: SaveProductionSettingsRequest = serde_json::from_slice(include_bytes!(
        "../../../contracts/fixtures/production/v1/settings-request.json"
    ))
    .unwrap();
    let mut settings = ProductionSettingsResponse {
        script_id: revision.script_id.clone(),
        version: 1,
        script_revision: 1,
        operation_id: request.operation_id,
        recorded_by: "owner".into(),
        recorded_at: "2026-10-11T00:01:00Z".into(),
        settings: request.settings,
    };
    for binding in &mut settings.settings.bindings {
        binding.provider_id = neural_contract::PROVIDER_ID.into();
        binding.model_id = neural_contract::MODEL_ID.into();
        binding.voice_id = voice.into();
        binding.voice_rights_record_id = "research-neural-voice".into();
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    // Preserve missing production rights. A research sample is never an eligible asset.
    prepare_inputs("owner", &revision, &settings, &[], true, 0)
        .unwrap()
        .document()
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

fn receipt(document: &FrozenProductionDocument, bytes: &[u8]) -> TtsReceipt {
    let line = &document.resolved[0];
    TtsReceipt {
        version: RECEIPT_VERSION.into(),
        evidence: ReceiptEvidence::CallerReported,
        attempt_id: "research-attempt".into(),
        operation_id: "research-operation".into(),
        provider_request_id: None,
        idempotency_key: None,
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
        outcome: ReceiptOutcome::Output {
            audio_sha256: format!("{:x}", Sha256::digest(bytes)),
        },
    }
}

#[test]
fn neural_receipt_inspects_research_pcm_without_acceptance_or_production_rights() {
    let document = document("Ánh đèn còn sáng.", "duc-tri");
    let bytes = wav();
    let receipt = receipt(&document, &bytes);
    let audit = audit_receipt(&document, &receipt, Some(&bytes)).unwrap();
    assert_eq!(
        audit.audio().unwrap().output_profile_version(),
        "cantos-vieneu-nano-tts-pcm-1"
    );
    let report = serde_json::to_value(&audit).unwrap();
    assert_eq!(
        report["assessment"],
        "offline-inspection-only-no-dispatch-or-acceptance"
    );
    assert_eq!(report["receipt"]["charge"], json!({"status":"unknown"}));
    assert!(document.rights.is_empty());
}

#[test]
fn neural_receipt_refuses_changed_scope_voice_revision_and_checksum() {
    let document = document("Ánh đèn còn sáng.", "duc-tri");
    let bytes = wav();
    let original = receipt(&document, &bytes);
    let mut changed = original.clone();
    changed.input_digest.push('0');
    assert_eq!(
        audit_receipt(&document, &changed, Some(&bytes)).unwrap_err(),
        ReceiptError::InputDigestMismatch
    );
    changed = original.clone();
    changed.dialogue_id = "absent".into();
    assert_eq!(
        audit_receipt(&document, &changed, Some(&bytes)).unwrap_err(),
        ReceiptError::DialogueMissing
    );
    changed = original.clone();
    changed.provider.voice_id = "my-duyen".into();
    assert_eq!(
        audit_receipt(&document, &changed, Some(&bytes)).unwrap_err(),
        ReceiptError::ProviderIdentityMismatch
    );
    let mut revised = document.clone();
    revised.revision.revision += 1;
    assert_eq!(
        audit_receipt(&revised, &original, Some(&bytes)).unwrap_err(),
        ReceiptError::InvalidDocument
    );
    let mut corrupt = bytes;
    *corrupt.last_mut().unwrap() ^= 1;
    assert!(matches!(
        audit_receipt(&document, &original, Some(&corrupt)),
        Err(ReceiptError::InvalidAudio { .. })
    ));
}

#[test]
fn neural_freeze_rejects_unsupported_effective_text_after_pronunciation() {
    let document = document("Ánh đèn còn sáng.", "duc-tri");
    for replacement in ["[[a]]", "Ánh\u{2028}đèn", "😀", "…", "x/y"] {
        let mut settings = document.settings.clone();
        settings
            .settings
            .bindings
            .iter_mut()
            .find(|binding| binding.character_id == "narrator")
            .unwrap()
            .pronunciation = vec![ProductionPronunciation {
            surface: "Ánh đèn".into(),
            replacement: replacement.into(),
        }];
        assert_eq!(
            prepare_inputs("owner", &document.revision, &settings, &[], true, 0)
                .err()
                .unwrap(),
            vec![FieldIssue {
                path: "dialogues.dialogue-01.text".into(),
                rule: "invalid_neural_text".into(),
            }]
        );
    }
}

#[test]
fn cli_preserves_conservative_intent_and_permanent_not_sent_preflight_failure() {
    let document = document("Ánh đèn còn sáng.", "duc-tri");
    let root = std::env::temp_dir().join(format!("cantos-neural-cli-{}", uuid::Uuid::new_v4()));
    fs::create_dir(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let input = root.join("document.json");
    fs::write(&input, serde_json::to_vec(&document).unwrap()).unwrap();
    let destination = root.join("new-attempt");
    let invoke = || {
        Command::new(env!("CARGO_BIN_EXE_probe-neural-tts"))
            .arg("--research-only")
            .arg(&input)
            .arg("dialogue-01")
            .arg(root.join("missing-python"))
            .arg(root.join("missing-model"))
            .arg(&destination)
            .output()
            .unwrap()
    };
    let first = invoke();
    assert!(!first.status.success());
    let intent: TtsReceipt =
        serde_json::from_slice(&fs::read(destination.join("intent.json")).unwrap()).unwrap();
    assert!(matches!(
        intent.outcome,
        ReceiptOutcome::Failure {
            class: cantos_server::tts::attempt::FailureClass::Timeout,
            delivery: cantos_server::tts::attempt::DeliveryState::MayHaveExecuted,
        }
    ));
    let receipt_path = destination.join("receipt.json");
    let original_receipt = fs::read(&receipt_path).unwrap();
    let receipt: TtsReceipt = serde_json::from_slice(&original_receipt).unwrap();
    assert!(matches!(
        receipt.outcome,
        ReceiptOutcome::Failure {
            class: cantos_server::tts::attempt::FailureClass::Permanent,
            delivery: cantos_server::tts::attempt::DeliveryState::NotSent,
        }
    ));
    assert_eq!(receipt.attempt_id, intent.attempt_id);
    assert_eq!(receipt.input_digest, document.input_digest);
    assert!(audit_receipt(&document, &receipt, None).is_ok());
    assert!(!destination.join("output.wav").exists());
    assert!(!invoke().status.success());
    assert_eq!(fs::read(&receipt_path).unwrap(), original_receipt);
    fs::remove_dir_all(&root).unwrap();
}

#[test]
#[ignore = "requires an explicitly installed pinned local model/runtime and coordinated resource window"]
fn installed_neural_runtime_generates_private_original_vietnamese_samples() {
    let python = std::env::var_os("CANTOS_NEURAL_PYTHON").expect("explicit Python path required");
    let model = std::env::var_os("CANTOS_NEURAL_MODEL").expect("explicit model directory required");
    let root = std::env::current_dir()
        .unwrap()
        .join("../../target/neural-tts-evidence")
        .join(uuid::Uuid::new_v4().to_string());
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&root).unwrap();
    let root = fs::canonicalize(root).unwrap();
    let mut records = Vec::new();
    for sample in samples()["samples"].as_array().unwrap() {
        let id = sample["id"].as_str().unwrap();
        let document = document(
            sample["text"].as_str().unwrap(),
            sample["voice_id"].as_str().unwrap(),
        );
        let path = root.join(format!("{id}-input.json"));
        fs::write(&path, serde_json::to_vec_pretty(&document).unwrap()).unwrap();
        let destination = root.join(id);
        let start = Instant::now();
        let result = Command::new("/usr/bin/time")
            .args([
                "-l",
                env!("CARGO_BIN_EXE_probe-neural-tts"),
                "--research-only",
            ])
            .arg(&path)
            .arg("dialogue-01")
            .arg(&python)
            .arg(&model)
            .arg(&destination)
            .output()
            .unwrap();
        let elapsed = start.elapsed();
        fs::write(root.join(format!("{id}-time.txt")), &result.stderr).unwrap();
        assert!(
            result.status.success(),
            "{id}: {}",
            String::from_utf8_lossy(&result.stderr)
        );
        let output = fs::read(destination.join("output.wav")).unwrap();
        let receipt: TtsReceipt =
            serde_json::from_slice(&fs::read(destination.join("receipt.json")).unwrap()).unwrap();
        let audit = audit_receipt(&document, &receipt, Some(&output)).unwrap();
        let audio = audit.audio().unwrap();
        let duration = audio.frames() as f64 / f64::from(audio.sample_rate_hz());
        records.push(json!({"sample":id,"voice_id":sample["voice_id"],"input_digest":document.input_digest,"wav_sha256":audio.file_sha256(),"bytes":output.len(),"frames":audio.frames(),"sample_rate_hz":audio.sample_rate_hz(),"wall_seconds":elapsed.as_secs_f64(),"real_time_factor":elapsed.as_secs_f64()/duration,"timing_scope":"cold CLI with model hash verification","listening":"NOT_RUN"}));
    }
    fs::write(
        root.join("evidence.json"),
        serde_json::to_vec_pretty(&records).unwrap(),
    )
    .unwrap();
    println!("private neural evidence: {}", root.display());
}
