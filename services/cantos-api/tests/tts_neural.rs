//! Pure neural admission and dispatch-refusal tests. Process doubles live beside the shell;
//! installed model evidence is a separate opt-in run, never claimed by this suite.
use std::{path::Path, time::Duration};

use cantos_api::{
    production::{authorize_inputs, prepare_inputs, production_input_digest},
    script_ir::read_script,
    tts::{
        neural::{run_neural, NeuralPurpose, NeuralRunError, NeuralSynthesisRequest},
        neural_contract::{
            valid_text, validate_resolved, NeuralRequestError, ADAPTER_VERSION, MAX_TEXT_BYTES,
            MODEL_ID, OUTPUT_CONTRACT, PROVIDER_ID, VOICE_IDS,
        },
    },
};
use cantos_contracts::{
    FrozenProductionDocument, ProductionApprovalResponse, ProductionRightsClaim,
    ProductionRightsClaimResponse, ProductionRightsStatus, ProductionRightsSubject,
    ProductionSettingsResponse, ResolvedSynthesisInput, RevisionResponse,
    SaveProductionRightsRequest, SaveProductionSettingsRequest, SynthesisPerformance,
};
use sha2::{Digest, Sha256};

fn resolved() -> ResolvedSynthesisInput {
    ResolvedSynthesisInput {
        dialogue_id: "dialogue-01".into(),
        character_id: "narrator".into(),
        text: "Đức Trí hỏi: Mỹ Duyên đã về chưa? Ánh đèn còn sáng!".into(),
        language: "vi-VN".into(),
        provider_id: PROVIDER_ID.into(),
        model_id: MODEL_ID.into(),
        voice_id: "duc-tri".into(),
        voice_rights_record_id: "fixture_voice".into(),
        adapter_version: ADAPTER_VERSION.into(),
        output_contract: OUTPUT_CONTRACT.into(),
        performance: SynthesisPerformance {
            rate_permille: 1_000,
            pitch_semitones: 0,
            emotion: Some("neutral".into()),
            intensity_permille: Some(0),
        },
        pronunciation: vec![],
    }
}

fn document(granted: bool) -> FrozenProductionDocument {
    // A new synthetic scope with plain punctuation; the historical eSpeak/reference
    // source, its ellipsis and saved snapshots are never modified.
    let original =
        include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
    let script = read_script(original.replace('…', "...").as_bytes()).unwrap();
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
        "../../../packages/contracts/fixtures/production/v1/settings-request.json"
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
        binding.provider_id = PROVIDER_ID.into();
        binding.model_id = MODEL_ID.into();
        binding.voice_id = if binding.character_id == "an" {
            "my-duyen"
        } else {
            "duc-tri"
        }
        .into();
        binding.voice_rights_record_id = format!("research_voice_{}", binding.character_id);
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    let mut rights = Vec::new();
    if granted {
        let base: SaveProductionRightsRequest = serde_json::from_slice(include_bytes!(
            "../../../packages/contracts/fixtures/production/v1/rights-request.json"
        ))
        .unwrap();
        let subjects = ["rights-demo", "rights-adaptation-demo", "rights-asset-demo"]
            .into_iter()
            .map(|id| {
                (
                    id.to_owned(),
                    ProductionRightsSubject::Evidence {
                        record_id: id.into(),
                    },
                )
            })
            .chain(settings.settings.bindings.iter().map(|binding| {
                (
                    binding.voice_rights_record_id.clone(),
                    ProductionRightsSubject::Voice {
                        provider_id: PROVIDER_ID.into(),
                        model_id: MODEL_ID.into(),
                        voice_id: binding.voice_id.clone(),
                    },
                )
            }));
        for (id, subject) in subjects {
            let mut claim = base.claim.clone();
            claim.record_id = id.clone();
            claim.subject = subject;
            claim.status = ProductionRightsStatus::Granted;
            rights.push(ProductionRightsClaimResponse {
                claim: ProductionRightsClaim {
                    version: 1,
                    declaration: claim,
                },
                operation_id: format!("claim-{id}"),
                recorded_by: "owner".into(),
                recorded_at: "2026-10-11T00:00:30Z".into(),
            });
        }
    }
    prepare_inputs("owner", &revision, &settings, &rights, true, 0)
        .unwrap()
        .document()
}

#[test]
fn only_exact_model_presets_language_and_explicit_default_controls_are_admitted() {
    for voice in VOICE_IDS {
        for language in ["vi", "vi-VN"] {
            let mut input = resolved();
            input.voice_id = voice.into();
            input.language = language.into();
            assert_eq!(validate_resolved(&input), Ok(()));
        }
    }
    assert!(MODEL_ID.len() <= 128);
    assert!(MODEL_ID
        .bytes()
        .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-'));
}

#[test]
fn every_mismatched_identity_and_unsupported_control_has_a_precise_error() {
    for (field, error) in [
        ("provider", NeuralRequestError::WrongProvider),
        ("model", NeuralRequestError::WrongModel),
        ("adapter", NeuralRequestError::WrongAdapter),
        ("output", NeuralRequestError::WrongOutputContract),
        ("voice", NeuralRequestError::UnsupportedVoice),
        ("language", NeuralRequestError::UnsupportedLanguage),
        ("rate", NeuralRequestError::UnsupportedRate),
        ("pitch", NeuralRequestError::UnsupportedPitch),
        ("emotion", NeuralRequestError::UnsupportedEmotion),
        ("intensity", NeuralRequestError::UnsupportedIntensity),
    ] {
        let mut input = resolved();
        match field {
            "provider" => input.provider_id = "espeak-ng-local".into(),
            "model" => input.model_id.push('0'),
            "adapter" => input.adapter_version.push('0'),
            "output" => input.output_contract.push('0'),
            "voice" => input.voice_id = "default".into(),
            "language" => input.language = "en".into(),
            "rate" => input.performance.rate_permille = 999,
            "pitch" => input.performance.pitch_semitones = 1,
            "emotion" => input.performance.emotion = Some("hopeful".into()),
            "intensity" => input.performance.intensity_permille = Some(1),
            _ => unreachable!(),
        }
        assert_eq!(validate_resolved(&input), Err(error));
    }
    for (emotion, intensity, error) in [
        (None, Some(0), NeuralRequestError::UnsupportedEmotion),
        (
            Some("neutral".into()),
            None,
            NeuralRequestError::UnsupportedIntensity,
        ),
    ] {
        let mut input = resolved();
        input.performance.emotion = emotion;
        input.performance.intensity_permille = intensity;
        assert_eq!(validate_resolved(&input), Err(error));
    }
}

#[test]
fn vietnamese_nfc_and_basic_punctuation_are_bounded_without_engine_markup() {
    assert!(valid_text(
        "Vọng Đài: Nguyễn Ái Hân hỏi, 'Trí đã về chưa?' (Phần 2)."
    ));
    assert!(valid_text(&"x".repeat(MAX_TEXT_BYTES)));
    assert!(valid_text(&"ơ".repeat(MAX_TEXT_BYTES / 2)));
    for text in [
        "",
        " ",
        "\n",
        "x\0y",
        "x\ty",
        "x\u{2028}y",
        "x\u{2029}y",
        "o\u{31b}",
        "[[a]]",
        "<break>",
        "{voice}",
        "日本語",
        "😀",
        "a\u{200d}b",
        "a/b",
        "…",
    ] {
        assert!(!valid_text(text), "text must be rejected: {text:?}");
    }
    assert!(!valid_text(&"x".repeat(MAX_TEXT_BYTES + 1)));
    assert!(!valid_text(&"ơ".repeat(MAX_TEXT_BYTES / 2 + 1)));
}

#[test]
fn research_scope_uses_complete_reconstruction_instead_of_trusting_a_rehashed_document() {
    let original = document(false);
    let request = NeuralSynthesisRequest::from_document(&original, "dialogue-01").unwrap();
    assert_eq!(request.input(), &original.resolved[0]);
    assert_eq!(request.input_digest(), original.input_digest);
    assert_eq!(request.dialogue_id(), "dialogue-01");
    assert!(!format!("{request:?}").contains(request.text()));
    assert_eq!(
        NeuralSynthesisRequest::from_document(&original, "other-dialogue"),
        Err(NeuralRequestError::DialogueMissing)
    );
    for field in ["digest", "text", "revision", "voice"] {
        let mut changed = original.clone();
        match field {
            "digest" => changed.input_digest.push('0'),
            "text" => changed.resolved[0].text.push('x'),
            "revision" => changed.revision.revision = 2,
            "voice" => changed.resolved[0].voice_id = "my-duyen".into(),
            _ => unreachable!(),
        }
        if field != "digest" {
            changed.input_digest = production_input_digest(&changed).unwrap();
        }
        assert_eq!(
            NeuralSynthesisRequest::from_document(&changed, "dialogue-01"),
            Err(NeuralRequestError::InvalidDocument)
        );
    }
}

#[test]
fn production_is_refused_before_io_even_when_planning_approval_and_claims_are_current() {
    let document = document(true);
    let approval = ProductionApprovalResponse {
        id: "approval-1".into(),
        snapshot_id: "snapshot-1".into(),
        approved_by: "owner".into(),
        approved_at: "2026-10-11T00:02:00Z".into(),
        operation_id: "approve-1".into(),
        input_digest: document.input_digest.clone(),
    };
    let witness =
        authorize_inputs(&document, 1, 1, &document.rights, true, Some(&approval), 0).unwrap();
    assert_eq!(witness.input_digest(), document.input_digest);
    let request = NeuralSynthesisRequest::from_document(&document, "dialogue-01").unwrap();
    assert_eq!(
        run_neural(
            &request,
            NeuralPurpose::Production,
            Path::new("/missing-runtime"),
            Path::new("/missing-model"),
            Duration::ZERO
        )
        .unwrap_err(),
        NeuralRunError::ProductionForbidden
    );
    assert_eq!(
        NeuralRunError::ProductionForbidden.failure_facts(),
        (
            cantos_api::tts::attempt::FailureClass::Permanent,
            cantos_api::tts::attempt::DeliveryState::NotSent
        )
    );
}

#[test]
fn research_sample_still_requires_explicit_timeout_and_real_local_paths() {
    let document = document(false);
    let request = NeuralSynthesisRequest::from_document(&document, "dialogue-01").unwrap();
    for duration in [Duration::ZERO, Duration::from_secs(121)] {
        assert_eq!(
            run_neural(
                &request,
                NeuralPurpose::ResearchSample,
                Path::new("python"),
                Path::new("models"),
                duration
            )
            .unwrap_err(),
            NeuralRunError::InvalidTimeout
        );
    }
    #[cfg(any(target_os = "macos", target_os = "linux"))]
    {
        assert_eq!(
            run_neural(
                &request,
                NeuralPurpose::ResearchSample,
                Path::new("python"),
                Path::new("models"),
                Duration::from_secs(2)
            )
            .unwrap_err(),
            NeuralRunError::InvalidRuntimePath
        );
        assert_eq!(
            run_neural(
                &request,
                NeuralPurpose::ResearchSample,
                Path::new("/cantos-missing-neural-python"),
                Path::new("/cantos-missing-neural-model"),
                Duration::from_secs(2)
            )
            .unwrap_err(),
            NeuralRunError::InvalidRuntimePath
        );
        assert_eq!(
            run_neural(
                &request,
                NeuralPurpose::ResearchSample,
                Path::new("/bin/sh"),
                Path::new("/cantos-missing-neural-model"),
                Duration::from_secs(2)
            )
            .unwrap_err(),
            NeuralRunError::InvalidModelDirectory
        );
        let root = std::env::temp_dir().join(format!(
            "cantos-neural-missing-model-{}",
            uuid::Uuid::new_v4()
        ));
        std::fs::create_dir(&root).unwrap();
        let root = std::fs::canonicalize(root).unwrap();
        assert_eq!(
            run_neural(
                &request,
                NeuralPurpose::ResearchSample,
                Path::new("/bin/sh"),
                &root,
                Duration::from_secs(2)
            )
            .unwrap_err(),
            NeuralRunError::ModelArtifactRejected {
                name: "codec_decoder.onnx"
            }
        );
        std::fs::remove_dir(root).unwrap();
    }
}
