use cantos_api::contexts::production::{
    admit_frozen_document, authorize_inputs, estimate_cost, evaluate_for_owner, evaluate_snapshot,
    prepare_inputs, production_catalog, production_input_digest, validate_rights_declaration,
    validate_settings, MAX_MONEY_MINOR,
};
use cantos_api::script_ir::read_script;
use cantos_contracts::{
    CharacterCasting, FieldIssue, FrozenProductionDocument, ProductionApprovalResponse,
    ProductionBudget, ProductionCurrency, ProductionEstimate, ProductionFindingCode,
    ProductionPronunciation, ProductionRate, ProductionRightsClaim, ProductionRightsClaimResponse,
    ProductionRightsDeclaration, ProductionRightsScope, ProductionRightsStatus,
    ProductionRightsSubject, ProductionSettings, ProductionSettingsResponse, RevisionResponse,
    SynthesisPerformance,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

const SCRIPT: &[u8] =
    include_bytes!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const NOW: i64 = 100;

fn revision_from_json(value: &Value) -> RevisionResponse {
    let script = read_script(&serde_json::to_vec(value).unwrap()).unwrap();
    let bytes = script.export_bytes();
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(&bytes);
    RevisionResponse {
        script_id: "00000000-0000-4000-8000-000000000005".into(),
        revision: 1,
        accepted_by: "editor".into(),
        accepted_at: "2026-10-10T00:00:00Z".into(),
        content_digest: script.content_digest().to_string(),
        export_digest: format!("sir-e1:sha256:{:x}", hash.finalize()),
        script_json: String::from_utf8(bytes).unwrap(),
    }
}

fn revision() -> RevisionResponse {
    revision_from_json(&serde_json::from_slice(SCRIPT).unwrap())
}

fn settings() -> ProductionSettingsResponse {
    ProductionSettingsResponse {
        script_id: revision().script_id,
        version: 1,
        script_revision: 1,
        operation_id: "00000000-0000-4000-8000-000000000010".into(),
        recorded_by: "editor".into(),
        recorded_at: "2026-10-10T00:01:00Z".into(),
        settings: ProductionSettings {
            bindings: ["narrator", "an", "minh"]
                .into_iter()
                .map(|character| {
                    let narrator = character == "narrator";
                    CharacterCasting {
                        character_id: character.into(),
                        provider_id: "reference-only".into(),
                        model_id: "reference-v1".into(),
                        voice_id: if narrator {
                            "synthetic-narrator"
                        } else {
                            "synthetic-character"
                        }
                        .into(),
                        language: "vi-VN".into(),
                        voice_rights_record_id: if narrator {
                            "rights-voice-narrator"
                        } else {
                            "rights-voice-character"
                        }
                        .into(),
                        performance: SynthesisPerformance {
                            rate_permille: 1_000,
                            pitch_semitones: 0,
                            emotion: None,
                            intensity_permille: None,
                        },
                        pronunciation: vec![],
                    }
                })
                .collect(),
            budget: ProductionBudget {
                currency: ProductionCurrency::VND,
                limit_minor: 4,
                scope: "fixture-private-scene".into(),
                territory: "private-planning".into(),
                rate: Some(ProductionRate {
                    reference: "self-declared synthetic tariff, not provider price".into(),
                    version: "fixture-rate-v1".into(),
                    units_per_charge: 100,
                    amount_minor: 1,
                }),
            },
        },
    }
}

fn right(record_id: &str, subject: ProductionRightsSubject) -> ProductionRightsClaimResponse {
    ProductionRightsClaimResponse {
        claim: ProductionRightsClaim {
            version: 1,
            declaration: ProductionRightsDeclaration {
                record_id: record_id.into(),
                subject,
                scope: ProductionRightsScope::ProductionSynthesis,
                rights_holder: "Reference-only fixture author".into(),
                languages: vec!["vi-VN".into()],
                territory: "private-planning".into(),
                attribution: "not-applicable-private-planning".into(),
                restrictions: "none-declared".into(),
                permitted_scope: "fixture-private-scene".into(),
                status: ProductionRightsStatus::Granted,
                reference: "synthetic test declaration, not legal verification".into(),
                valid_from_unix: 0,
                valid_until_unix: Some(200),
            },
        },
        operation_id: format!("claim-{record_id}"),
        recorded_by: "owner".into(),
        recorded_at: "2026-10-10T00:00:30Z".into(),
    }
}

fn rights() -> Vec<ProductionRightsClaimResponse> {
    let mut rights: Vec<_> = ["rights-demo", "rights-adaptation-demo", "rights-asset-demo"]
        .into_iter()
        .map(|id| {
            right(
                id,
                ProductionRightsSubject::Evidence {
                    record_id: id.into(),
                },
            )
        })
        .collect();
    for (id, voice) in [
        ("rights-voice-narrator", "synthetic-narrator"),
        ("rights-voice-character", "synthetic-character"),
    ] {
        rights.push(right(
            id,
            ProductionRightsSubject::Voice {
                provider_id: "reference-only".into(),
                model_id: "reference-v1".into(),
                voice_id: voice.into(),
            },
        ));
    }
    rights
}

fn document() -> FrozenProductionDocument {
    prepare_inputs("owner", &revision(), &settings(), &rights(), true, NOW)
        .unwrap()
        .document()
}

fn google_document(mixed_local: bool) -> FrozenProductionDocument {
    let mut settings = settings();
    for binding in &mut settings.settings.bindings {
        let local = mixed_local && binding.character_id == "narrator";
        binding.provider_id = if local {
            "espeak-ng-local"
        } else {
            "google-cloud-tts"
        }
        .into();
        binding.model_id = if local {
            "espeak-ng-1.52.0"
        } else {
            "google-neural2"
        }
        .into();
        binding.voice_id = if local { "vi" } else { "vi-VN-Neural2-A" }.into();
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    let mut rights = rights();
    for right in &mut rights {
        if let ProductionRightsSubject::Voice {
            provider_id,
            model_id,
            voice_id,
        } = &mut right.claim.declaration.subject
        {
            let local = mixed_local && right.claim.declaration.record_id == "rights-voice-narrator";
            *provider_id = if local {
                "espeak-ng-local"
            } else {
                "google-cloud-tts"
            }
            .into();
            *model_id = if local {
                "espeak-ng-1.52.0"
            } else {
                "google-neural2"
            }
            .into();
            *voice_id = if local { "vi" } else { "vi-VN-Neural2-A" }.into();
        }
    }
    prepare_inputs("owner", &revision(), &settings, &rights, true, NOW)
        .unwrap()
        .document()
}

fn historical_google_document(mixed_local: bool) -> FrozenProductionDocument {
    let mut document = google_document(mixed_local);
    for line in &mut document.resolved {
        if line.provider_id == "google-cloud-tts" {
            line.adapter_version = "google-cloud-tts-synthesize-2".into();
        }
    }
    document.input_digest = production_input_digest(&document).unwrap();
    document
}

fn approval(document: &FrozenProductionDocument) -> ProductionApprovalResponse {
    ProductionApprovalResponse {
        id: "approval-1".into(),
        snapshot_id: "snapshot-1".into(),
        approved_by: "owner".into(),
        approved_at: "2026-10-10T00:02:00Z".into(),
        operation_id: "approve-operation-1".into(),
        input_digest: document.input_digest.clone(),
    }
}

fn codes(
    settings: &ProductionSettingsResponse,
    rights: &[ProductionRightsClaimResponse],
    reviewed: bool,
    now: i64,
) -> Vec<ProductionFindingCode> {
    prepare_inputs("owner", &revision(), settings, rights, reviewed, now)
        .unwrap()
        .preview()
        .findings
        .into_iter()
        .map(|finding| finding.code)
        .collect()
}

#[test]
fn reference_catalog_and_resolved_inputs_make_no_running_voice_or_billing_claim() {
    let catalog = production_catalog();
    assert_eq!(catalog.capabilities.len(), 8);
    assert!(catalog
        .capabilities
        .iter()
        .take(2)
        .all(|capability| capability.reference_only && !capability.billable_dispatch_available));
    let local = &catalog.capabilities[2];
    assert_eq!(local.provider_id, "espeak-ng-local");
    assert!(!local.reference_only && !local.billable_dispatch_available);
    let neural = &catalog.capabilities[3];
    assert_eq!(
        neural,
        &cantos_api::contexts::production::catalog::neural::capability()
    );
    assert_eq!(neural.voice_ids, ["duc-tri", "my-duyen"]);
    assert!(!neural.billable_dispatch_available);
    let google: Vec<_> = catalog.capabilities[4..]
        .iter()
        .map(|c| {
            (
                c.provider_id.as_str(),
                c.model_id.as_str(),
                c.voice_ids.len(),
            )
        })
        .collect();
    assert_eq!(
        google,
        [
            ("google-cloud-tts", "google-chirp3-hd", 30),
            ("google-cloud-tts", "google-neural2", 2),
            ("google-cloud-tts", "google-wavenet", 4),
            ("google-cloud-tts", "google-standard", 4),
        ]
    );
    assert!(catalog.capabilities[4..].iter().all(|c| !c.reference_only
        && !c.billable_dispatch_available
        && !c.intensity_supported
        && c.emotions == ["neutral"]));
    let prepared = prepare_inputs("owner", &revision(), &settings(), &rights(), true, NOW).unwrap();
    let preview = prepared.preview();
    assert!(preview.inputs_eligible);
    assert!(!preview.billable_dispatch_available);
    assert_eq!(preview.resolved.len(), 4);
    assert_eq!(
        preview.resolved[1].text,
        "Ngày mai, mình có diễn tiếp không?"
    );
    assert_eq!(
        preview.resolved[1].performance.emotion.as_deref(),
        Some("hopeful")
    );
    assert_eq!(
        preview.resolved[1].performance.intensity_permille,
        Some(600)
    );
    assert_eq!(
        preview.resolved[1].output_contract,
        "reference-only/no-audio"
    );
    assert_eq!(
        preview.estimate,
        ProductionEstimate::Known {
            currency: ProductionCurrency::VND,
            amount_minor: 4,
            units: 135,
            rate_reference: "self-declared synthetic tariff, not provider price".into(),
            rate_version: "fixture-rate-v1".into(),
        }
    );
}

#[test]
fn local_casting_preserves_requested_identity_and_rejects_inherited_unsupported_delivery() {
    let mut local_settings = settings();
    for binding in &mut local_settings.settings.bindings {
        binding.provider_id = "espeak-ng-local".into();
        binding.model_id = "espeak-ng-1.52.0".into();
        binding.voice_id = "vi".into();
    }
    let errors = prepare_inputs("owner", &revision(), &local_settings, &[], true, NOW)
        .err()
        .unwrap();
    assert!(errors
        .iter()
        .any(|e| e.rule == "unsupported_control" && e.path.contains("performance")));
    for binding in &mut local_settings.settings.bindings {
        // This is an explicit reviewed override, never an adapter fallback.
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    let document = prepare_inputs("owner", &revision(), &local_settings, &[], true, NOW)
        .unwrap()
        .document();
    assert_eq!(document.resolved[0].provider_id, "espeak-ng-local");
    assert_eq!(document.resolved[0].adapter_version, "espeak-ng-local-v1");
    assert_eq!(
        document.resolved[0].output_contract,
        "wav-pcm-s16le-mono-22050-v1"
    );
    assert_ne!(document.input_digest, crate::document().input_digest);
    local_settings.settings.bindings[0]
        .performance
        .pitch_semitones = 1;
    assert_eq!(
        validate_settings(&local_settings.settings).unwrap_err(),
        vec![FieldIssue {
            path: "bindings[0].performance.pitch_semitones".into(),
            rule: "unsupported_control".into()
        }]
    );
    local_settings.settings.bindings[0]
        .performance
        .pitch_semitones = 0;
    local_settings.settings.bindings[0].voice_id = "invented-voice".into();
    assert_eq!(
        validate_settings(&local_settings.settings).unwrap_err(),
        vec![FieldIssue {
            path: "bindings[0].voice_id".into(),
            rule: "unsupported_control".into()
        }]
    );
}

#[test]
fn google_casting_pins_its_adapter_and_requires_an_explicit_neutral_delivery() {
    let mut google = settings();
    for (binding, (model, voice)) in google.settings.bindings.iter_mut().zip([
        ("google-neural2", "vi-VN-Neural2-A"),
        ("google-chirp3-hd", "vi-VN-Chirp3-HD-Aoede"),
        ("google-wavenet", "vi-VN-Wavenet-D"),
    ]) {
        binding.provider_id = "google-cloud-tts".into();
        binding.model_id = model.into();
        binding.voice_id = voice.into();
        // One voice subject per rights record; two voices cannot share a declaration.
        binding.voice_rights_record_id = format!("rights-voice-{}", binding.character_id);
    }
    assert_eq!(validate_settings(&google.settings), Ok(()));
    // Script IR delivery (calm, hopeful, warm) is inherited and refused, never dropped.
    let inherited: Vec<_> = prepare_inputs("owner", &revision(), &google, &[], true, NOW)
        .err()
        .unwrap()
        .into_iter()
        .map(|issue| (issue.path, issue.rule))
        .collect();
    assert_eq!(
        inherited,
        ["01", "02", "03", "04"].map(|n| (
            format!("dialogues.dialogue-{n}.performance"),
            "unsupported_control".to_owned()
        ))
    );
    for binding in &mut google.settings.bindings {
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    let document = prepare_inputs("owner", &revision(), &google, &[], true, NOW)
        .unwrap()
        .document();
    assert!(document.resolved.iter().all(|line| {
        line.provider_id == "google-cloud-tts"
            && line.adapter_version == "google-cloud-tts-synthesize-3"
            && line.output_contract == "wav-pcm-s16le-mono-24000-v1"
            && line.performance.emotion.as_deref() == Some("neutral")
            && line.performance.intensity_permille == Some(0)
    }));
    assert_ne!(document.input_digest, crate::document().input_digest);
    // Chirp 3: HD has no rate or pitch parameter; other families accept the documented range.
    for (index, rate, pitch, field) in [
        (1, 1_100, 0, "rate_permille"),
        (1, 1_000, 1, "pitch_semitones"),
        (0, 2_001, 0, "rate_permille"),
        (0, 1_000, -21, "pitch_semitones"),
    ] {
        let mut changed = google.clone();
        changed.settings.bindings[index].performance.rate_permille = rate;
        changed.settings.bindings[index].performance.pitch_semitones = pitch;
        assert_eq!(
            validate_settings(&changed.settings).unwrap_err(),
            vec![FieldIssue {
                path: format!("bindings[{index}].performance.{field}"),
                rule: "unsupported_control".into()
            }]
        );
    }
    let mut accepted = google.clone();
    accepted.settings.bindings[0].performance.rate_permille = 2_000;
    accepted.settings.bindings[0].performance.pitch_semitones = -20;
    assert_eq!(validate_settings(&accepted.settings), Ok(()));
    // A voice outside the chosen model's family is reported on the voice it does not offer.
    for (field, change, reported) in [
        ("voice_id", "vi-VN-Neural2-Z", "voice_id"),
        ("model_id", "google-wavenet", "voice_id"),
        ("language", "en-US", "language"),
    ] {
        let mut changed = google.clone();
        let binding = &mut changed.settings.bindings[0];
        match field {
            "voice_id" => binding.voice_id = change.into(),
            "model_id" => binding.model_id = change.into(),
            _ => binding.language = change.into(),
        }
        assert_eq!(
            validate_settings(&changed.settings).unwrap_err(),
            vec![FieldIssue {
                path: format!("bindings[0].{reported}"),
                rule: "unsupported_control".into()
            }],
            "{field}"
        );
    }
    // Google's 5,000-byte request limit is checked on the resolved text before freezing.
    let mut raw: Value = serde_json::from_slice(SCRIPT).unwrap();
    raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!("ấ".repeat(1_667));
    assert_eq!(
        prepare_inputs("owner", &revision_from_json(&raw), &google, &[], true, NOW)
            .err()
            .unwrap(),
        vec![FieldIssue {
            path: "dialogues.dialogue-01.text".into(),
            rule: "google_text_too_large".into(),
        }]
    );
}

#[test]
fn historical_google_inputs_remain_exact_inspectable_history_but_approval_is_stale() {
    for mixed_local in [false, true] {
        let current = google_document(mixed_local);
        let historical = historical_google_document(mixed_local);
        let before = serde_json::to_vec(&historical).unwrap();
        assert_ne!(historical.input_digest, current.input_digest);
        assert_eq!(
            admit_frozen_document(
                &historical,
                &historical.revision,
                &historical.settings,
                &historical.rights
            ),
            Ok(())
        );
        assert_eq!(serde_json::to_vec(&historical).unwrap(), before);
        let approved = approval(&historical);
        let eligibility = evaluate_snapshot(
            &historical,
            1,
            1,
            &historical.rights,
            true,
            Some(&approved),
            NOW,
        );
        assert!(!eligibility.inputs_eligible);
        assert!(!eligibility.approval_current);
        assert!(!eligibility.billable_dispatch_available);
        assert_eq!(eligibility.findings.len(), 1);
        assert_eq!(
            eligibility.findings[0].code,
            ProductionFindingCode::ApprovalScopeMismatch
        );
        assert_eq!(eligibility.findings[0].path, "resolved.adapter_version");
        assert!(authorize_inputs(
            &historical,
            "owner",
            1,
            1,
            &historical.rights,
            true,
            Some(&approved),
            NOW
        )
        .is_err());
        assert!(
            evaluate_snapshot(
                &current,
                1,
                1,
                &current.rights,
                true,
                Some(&approval(&current)),
                NOW
            )
            .approval_current
        );
        if mixed_local {
            assert!(historical
                .resolved
                .iter()
                .any(|line| line.adapter_version == "espeak-ng-local-v1"));
        }
    }
}

#[test]
fn historical_admission_never_normalizes_forged_effective_inputs_even_with_a_new_digest() {
    type Mutation = fn(&mut FrozenProductionDocument);
    let original = historical_google_document(false);
    let mutations: [Mutation; 12] = [
        |d| d.catalog_version = "production-reference-v2".into(),
        |d| d.resolved[0].text.push('!'),
        |d| d.resolved[0].provider_id = "reference-only".into(),
        |d| d.resolved[0].model_id = "google-wavenet".into(),
        |d| d.resolved[0].voice_id = "vi-VN-Neural2-Z".into(),
        |d| d.resolved[0].voice_rights_record_id = "other-rights".into(),
        |d| d.resolved[0].output_contract = "wav-pcm-s16le-mono-22050-v1".into(),
        |d| d.resolved[0].performance.rate_permille += 1,
        |d| d.resolved[0].performance.emotion = None,
        |d| d.resolved[0].adapter_version = "google-cloud-tts-synthesize-unknown".into(),
        |d| d.resolved[0].adapter_version = "google-cloud-tts-synthesize-3".into(),
        |d| d.resolved.swap(0, 1),
    ];
    for (index, mutation) in mutations.into_iter().enumerate() {
        let mut forged = original.clone();
        mutation(&mut forged);
        forged.input_digest = production_input_digest(&forged).unwrap();
        assert_eq!(
            admit_frozen_document(&forged, &forged.revision, &forged.settings, &forged.rights),
            Err(vec![FieldIssue {
                path: "document.input_digest".into(),
                rule: "frozen_document_identity_mismatch".into(),
            }]),
            "historical mutation {index}"
        );
    }
    let mut digest_only = original.clone();
    digest_only.input_digest = google_document(false).input_digest;
    assert_eq!(
        admit_frozen_document(
            &digest_only,
            &digest_only.revision,
            &digest_only.settings,
            &digest_only.rights
        ),
        Err(vec![FieldIssue {
            path: "document.input_digest".into(),
            rule: "frozen_document_identity_mismatch".into(),
        }])
    );
    // Settled shell facts must also match, even if embedded receipts are self-consistent.
    let mut other_settings = original.settings.clone();
    other_settings.recorded_by = "other-editor".into();
    assert_eq!(
        admit_frozen_document(
            &original,
            &original.revision,
            &other_settings,
            &original.rights
        ),
        Err(vec![FieldIssue {
            path: "document.input_digest".into(),
            rule: "frozen_document_identity_mismatch".into(),
        }])
    );
}

fn local_settings() -> ProductionSettingsResponse {
    let mut settings = settings();
    for binding in &mut settings.settings.bindings {
        binding.provider_id = "espeak-ng-local".into();
        binding.model_id = "espeak-ng-1.52.0".into();
        binding.voice_id = "vi".into();
        binding.performance.emotion = Some("neutral".into());
        binding.performance.intensity_permille = Some(0);
    }
    settings
}

#[test]
fn local_freeze_rejects_line_separators_and_phoneme_syntax_after_pronunciation_resolution() {
    for replacement in ["Ánh\u{2028}đèn", "Ánh\u{2029}đèn", "[[a]]", "[["] {
        let mut settings = local_settings();
        settings.settings.bindings[0].pronunciation = vec![ProductionPronunciation {
            surface: "Ánh đèn".into(),
            replacement: replacement.into(),
        }];
        // The generic declaration remains valid; the local effective-text gate
        // must reject the resulting bytes before any candidate can be frozen.
        assert_eq!(validate_settings(&settings.settings), Ok(()));
        assert_eq!(
            prepare_inputs("owner", &revision(), &settings, &[], true, NOW)
                .err()
                .unwrap(),
            vec![FieldIssue {
                path: "dialogues.dialogue-01.text".into(),
                rule: "invalid_local_text".into(),
            }]
        );
    }
}

#[test]
fn local_freeze_rejects_phoneme_syntax_created_across_pronunciation_boundaries() {
    let mut raw: Value = serde_json::from_slice(SCRIPT).unwrap();
    raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!("[Ánh");
    let revision = revision_from_json(&raw);
    let mut settings = local_settings();
    settings.settings.bindings[0].pronunciation = vec![ProductionPronunciation {
        surface: "Ánh".into(),
        replacement: "[a]]".into(),
    }];
    assert_eq!(
        prepare_inputs("owner", &revision, &settings, &[], true, NOW)
            .err()
            .unwrap(),
        vec![FieldIssue {
            path: "dialogues.dialogue-01.text".into(),
            rule: "invalid_local_text".into(),
        }]
    );
}

#[test]
fn reference_candidates_preserve_preexisting_text_admission_and_exact_replacements() {
    for replacement in ["Ánh\u{2028}đèn", "Ánh\u{2029}đèn", "[[a]]", "[["] {
        let mut settings = settings();
        settings.settings.bindings[0].pronunciation = vec![ProductionPronunciation {
            surface: "Ánh đèn".into(),
            replacement: replacement.into(),
        }];
        let document = prepare_inputs("owner", &revision(), &settings, &rights(), true, NOW)
            .unwrap()
            .document();
        assert_eq!(
            document.resolved[0].text,
            format!("{replacement} cuối cùng còn sáng trên sân khấu.")
        );
        assert_eq!(document.resolved[0].provider_id, "reference-only");
        assert_eq!(
            document.resolved[0].adapter_version,
            "reference-only-no-dispatch-v1"
        );
        assert_eq!(
            document.resolved[0].output_contract,
            "reference-only/no-audio"
        );
    }
}

#[test]
fn unsupported_control_and_invalid_registry_ids_are_rejected_without_substitution() {
    type SettingsMutation = fn(&mut ProductionSettings);
    for (mutate, path) in [
        (
            (|s| s.bindings[0].provider_id = "unknown".into()) as SettingsMutation,
            "bindings[0].provider_id",
        ),
        (
            |s| s.bindings[0].model_id = "unknown".into(),
            "bindings[0].model_id",
        ),
        (
            |s| s.bindings[0].voice_id = "real-person-voice".into(),
            "bindings[0].voice_id",
        ),
        (
            |s| s.bindings[0].language = "fr".into(),
            "bindings[0].language",
        ),
        (
            |s| s.bindings[0].performance.rate_permille = 499,
            "bindings[0].performance.rate_permille",
        ),
        (
            |s| s.bindings[0].performance.pitch_semitones = 13,
            "bindings[0].performance.pitch_semitones",
        ),
        (
            |s| s.bindings[0].performance.emotion = Some("joyful".into()),
            "bindings[0].performance.emotion",
        ),
        (
            |s| s.bindings[0].performance.intensity_permille = Some(1_001),
            "bindings[0].performance.intensity_permille",
        ),
    ] {
        let mut settings = settings().settings;
        mutate(&mut settings);
        assert_eq!(
            validate_settings(&settings),
            Err(vec![FieldIssue {
                path: path.into(),
                rule: "unsupported_control".into()
            }])
        );
    }
    for id in [
        "Rights",
        "rights.example",
        "rights:example",
        &"a".repeat(65),
    ] {
        let mut settings = settings().settings;
        settings.bindings[0].voice_rights_record_id = id.into();
        assert_eq!(
            validate_settings(&settings),
            Err(vec![FieldIssue {
                path: "bindings[0].voice_rights_record_id".into(),
                rule: "id_format".into()
            }])
        );
        let mut claim = rights()[0].claim.declaration.clone();
        claim.record_id = id.into();
        claim.subject = ProductionRightsSubject::Evidence {
            record_id: id.into(),
        };
        assert_eq!(
            validate_rights_declaration(&claim),
            Err(vec![FieldIssue {
                path: "claim.record_id".into(),
                rule: "id_format".into()
            }])
        );
    }
}

#[test]
fn missing_binding_unknown_character_and_language_translation_fail_with_exact_issues() {
    let mut settings = settings();
    settings.settings.bindings.remove(2);
    let issues = prepare_inputs("owner", &revision(), &settings, &rights(), true, NOW)
        .err()
        .unwrap();
    assert_eq!(
        issues,
        vec![FieldIssue {
            path: "dialogues.dialogue-03.speaker_id".into(),
            rule: "casting_missing".into()
        }]
    );
    let mut settings = crate::settings();
    settings.settings.bindings[2].character_id = "unknown".into();
    assert_eq!(
        prepare_inputs("owner", &revision(), &settings, &rights(), true, NOW)
            .err()
            .unwrap(),
        vec![
            FieldIssue {
                path: "bindings.unknown".into(),
                rule: "unknown_character".into()
            },
            FieldIssue {
                path: "dialogues.dialogue-03.speaker_id".into(),
                rule: "casting_missing".into()
            },
        ]
    );
    let mut settings = crate::settings();
    settings.settings.bindings[0].language = "en".into();
    assert_eq!(
        prepare_inputs("owner", &revision(), &settings, &rights(), true, NOW)
            .err()
            .unwrap(),
        vec![FieldIssue {
            path: "dialogues.dialogue-01.language".into(),
            rule: "language_mismatch_no_translation".into()
        }]
    );
}

#[test]
fn every_rights_status_term_subject_language_scope_and_restriction_blocks_fail_closed() {
    type RightMutation = fn(&mut ProductionRightsDeclaration);
    for (mutate, expected) in [
        (
            (|r| r.status = ProductionRightsStatus::Pending) as RightMutation,
            ProductionFindingCode::RightsPending,
        ),
        (
            |r| r.status = ProductionRightsStatus::Revoked,
            ProductionFindingCode::RightsRevoked,
        ),
        (
            |r| r.valid_from_unix = NOW + 1,
            ProductionFindingCode::RightsNotYetValid,
        ),
        (
            |r| r.valid_until_unix = Some(NOW),
            ProductionFindingCode::RightsExpired,
        ),
        (
            |r| {
                r.subject = ProductionRightsSubject::Voice {
                    provider_id: "reference-only".into(),
                    model_id: "reference-v1".into(),
                    voice_id: "synthetic-character".into(),
                }
            },
            ProductionFindingCode::RightsSubjectMismatch,
        ),
        (
            |r| r.languages = vec!["en".into()],
            ProductionFindingCode::RightsLanguageMismatch,
        ),
        (
            |r| r.permitted_scope = "different-private-scene".into(),
            ProductionFindingCode::RightsScopeMismatch,
        ),
        (
            |r| r.restrictions = "unresolved restriction requires manual review".into(),
            ProductionFindingCode::RightsRestrictionsRequireReview,
        ),
    ] {
        let mut rights = rights();
        mutate(&mut rights[0].claim.declaration);
        assert_eq!(codes(&settings(), &rights, true, NOW), vec![expected]);
    }
    let mut rights = rights();
    rights.remove(0);
    assert_eq!(
        codes(&settings(), &rights, true, NOW),
        vec![ProductionFindingCode::RightsMissing]
    );
    assert_eq!(
        codes(&settings(), &crate::rights(), false, NOW),
        vec![ProductionFindingCode::ScriptUnreviewed]
    );
    let mut right = crate::rights()[0].claim.declaration.clone();
    right.territory = "worldwide".into();
    assert_eq!(
        validate_rights_declaration(&right),
        Err(vec![FieldIssue {
            path: "claim.territory".into(),
            rule: "unsupported_private_planning_territory".into()
        }])
    );
}

#[test]
fn a_frozen_blocked_candidate_is_inspectable_but_never_authorized() {
    let mut rights = rights();
    rights[0].claim.declaration.status = ProductionRightsStatus::Pending;
    rights[1].claim.declaration.status = ProductionRightsStatus::Revoked;
    rights[2].claim.declaration.valid_until_unix = Some(NOW);
    let mut settings = settings();
    settings.settings.budget.rate = None;
    let candidate = prepare_inputs("owner", &revision(), &settings, &rights, false, NOW).unwrap();
    assert_eq!(
        candidate
            .preview()
            .findings
            .iter()
            .map(|f| f.code)
            .collect::<Vec<_>>(),
        vec![
            ProductionFindingCode::RightsRevoked,
            ProductionFindingCode::RightsExpired,
            ProductionFindingCode::RightsPending,
            ProductionFindingCode::ScriptUnreviewed,
            ProductionFindingCode::EstimateUnavailable,
        ]
    );
    let document = candidate.document();
    let approval = approval(&document);
    let gate = evaluate_snapshot(&document, 1, 1, &rights, false, Some(&approval), NOW);
    assert!(!gate.inputs_eligible);
    assert!(!gate.approval_current);
    assert!(!gate.billable_dispatch_available);
    assert!(authorize_inputs(
        &document,
        "owner",
        1,
        1,
        &rights,
        false,
        Some(&approval),
        NOW
    )
    .is_err());
}

#[test]
fn approval_missing_is_separate_from_input_eligibility_and_foreign_owner_fails() {
    let document = document();
    let eligible = evaluate_snapshot(&document, 1, 1, &rights(), true, None, NOW);
    assert!(eligible.inputs_eligible);
    assert!(!eligible.approval_current);
    assert_eq!(
        eligible.findings[0].code,
        ProductionFindingCode::ApprovalMissing
    );
    let mut approval = approval(&document);
    let authorized = authorize_inputs(
        &document,
        "owner",
        1,
        1,
        &rights(),
        true,
        Some(&approval),
        NOW,
    )
    .unwrap();
    assert_eq!(authorized.input_digest(), document.input_digest);
    approval.approved_by = "other-owner".into();
    let eligibility = evaluate_snapshot(&document, 1, 1, &rights(), true, Some(&approval), NOW);
    assert_eq!(
        eligibility.findings[0].code,
        ProductionFindingCode::ApprovalScopeMismatch
    );
    assert!(!eligibility.approval_current);
}

#[test]
fn frozen_approval_cannot_follow_a_transfer_to_a_new_owner() {
    use ProductionFindingCode::{ApprovalMissing, ApprovalScopeMismatch};

    let document = document();
    let original = document.clone();
    for (owner, approver, expected) in [
        ("owner", "owner", vec![]),
        (
            "owner",
            "other-owner",
            vec![ApprovalMissing, ApprovalScopeMismatch],
        ),
        (
            "other-owner",
            "owner",
            vec![ApprovalMissing, ApprovalScopeMismatch],
        ),
        (
            "other-owner",
            "other-owner",
            vec![ApprovalScopeMismatch, ApprovalScopeMismatch],
        ),
    ] {
        let mut approval = approval(&document);
        approval.approved_by = approver.into();
        let eligibility = evaluate_for_owner(
            &document,
            owner,
            1,
            1,
            &rights(),
            true,
            Some(&approval),
            NOW,
        );
        assert_eq!(eligibility.inputs_eligible, expected.is_empty());
        assert_eq!(eligibility.approval_current, expected.is_empty());
        assert_eq!(
            eligibility
                .findings
                .iter()
                .map(|finding| finding.code)
                .collect::<Vec<_>>(),
            expected,
            "current owner {owner}, recorded approver {approver}",
        );
        if !expected.is_empty() {
            assert_eq!(eligibility.findings.last().unwrap().path, "/approval");
        }
        assert_eq!(document, original);
    }
}

#[test]
fn immutable_snapshot_does_not_follow_changed_draft_rights_versions_or_expiry() {
    let document = document();
    let original = document.clone();
    let approval = approval(&document);
    let mut current_rights = rights();
    current_rights[0].claim.version = 2;
    current_rights[0].claim.declaration.reference = "new evidence claim".into();
    let eligibility =
        evaluate_snapshot(&document, 2, 2, &current_rights, true, Some(&approval), NOW);
    assert_eq!(
        eligibility
            .findings
            .iter()
            .map(|f| f.code)
            .collect::<Vec<_>>(),
        vec![
            ProductionFindingCode::RevisionChanged,
            ProductionFindingCode::SettingsChanged,
            ProductionFindingCode::RightsChanged
        ]
    );
    assert!(!eligibility.approval_current);
    let expired = evaluate_snapshot(&document, 1, 1, &rights(), true, Some(&approval), 200);
    assert_eq!(
        expired
            .findings
            .iter()
            .filter(|finding| finding.code == ProductionFindingCode::RightsExpired)
            .count(),
        5
    );
    assert!(!expired.approval_current);
    assert_eq!(document, original);
}

#[test]
fn digest_binds_every_scoped_input_including_rights_excluded_from_content_digest() {
    type Mutation = fn(&mut FrozenProductionDocument);
    let document = document();
    let digest = document.input_digest.clone();
    let mutations: Vec<Mutation> = vec![
        |d| d.owner_id = "other-owner".into(),
        |d| d.catalog_version = "production-reference-v2".into(),
        |d| d.revision.script_id = "00000000-0000-4000-8000-000000000006".into(),
        |d| d.revision.revision += 1,
        |d| {
            d.revision.script_json = d
                .revision
                .script_json
                .replace("rights-demo", "rights-changed")
        },
        |d| d.revision.script_json = d.revision.script_json.replace("Ngày mai", "Ngày kia"),
        |d| d.settings.version += 1,
        |d| d.settings.settings.bindings[0].voice_id = "synthetic-narrator".into(),
        |d| d.settings.settings.bindings[0].model_id = "reference-v2".into(),
        |d| d.settings.settings.bindings[0].performance.rate_permille += 1,
        |d| d.settings.settings.bindings[0].performance.pitch_semitones += 1,
        |d| d.settings.settings.bindings[0].performance.emotion = Some("angry".into()),
        |d| {
            d.settings.settings.bindings[0]
                .performance
                .intensity_permille = Some(1)
        },
        |d| {
            d.settings.settings.bindings[0]
                .pronunciation
                .push(ProductionPronunciation {
                    surface: "Ngày".into(),
                    replacement: "Mai".into(),
                })
        },
        |d| d.settings.settings.budget.currency = ProductionCurrency::USD,
        |d| d.settings.settings.budget.limit_minor += 1,
        |d| d.settings.settings.budget.scope = "new-private-scope".into(),
        |d| d.settings.settings.budget.territory = "worldwide".into(),
        |d| d.settings.settings.budget.rate.as_mut().unwrap().version = "fixture-rate-v2".into(),
        |d| {
            d.settings
                .settings
                .budget
                .rate
                .as_mut()
                .unwrap()
                .amount_minor += 1
        },
        |d| {
            d.settings
                .settings
                .budget
                .rate
                .as_mut()
                .unwrap()
                .units_per_charge += 1
        },
        |d| d.rights[0].claim.version += 1,
        |d| d.rights[0].claim.declaration.status = ProductionRightsStatus::Revoked,
        |d| d.rights[0].claim.declaration.rights_holder = "another declared holder".into(),
        |d| d.rights[0].claim.declaration.languages = vec!["all".into()],
        |d| d.rights[0].claim.declaration.reference = "another recorded evidence".into(),
        |d| d.rights[0].claim.declaration.valid_until_unix = Some(300),
        |d| d.rights[0].claim.declaration.attribution = "private attribution due".into(),
        |d| d.rights[0].claim.declaration.restrictions = "private-planning-only".into(),
        |d| d.resolved[0].adapter_version = "another-adapter-v1".into(),
        |d| d.resolved[0].output_contract = "different-output-contract".into(),
        |d| d.resolved[0].text = "new effective text".into(),
    ];
    for (index, mutate) in mutations.into_iter().enumerate() {
        let mut changed = document.clone();
        mutate(&mut changed);
        assert_ne!(
            production_input_digest(&changed).unwrap(),
            digest,
            "input mutation {index}"
        );
        let eligibility = evaluate_snapshot(
            &changed,
            1,
            1,
            &rights(),
            true,
            Some(&approval(&document)),
            NOW,
        );
        assert!(!eligibility.approval_current, "input mutation {index}");
    }
    let mut raw: Value = serde_json::from_slice(SCRIPT).unwrap();
    raw["work"]["rights_record_id"] = json!("rights-changed");
    let changed = revision_from_json(&raw);
    assert_eq!(changed.content_digest, document.revision.content_digest);
    assert_ne!(changed.export_digest, document.revision.export_digest);
}

#[test]
fn canonical_order_and_unrelated_claims_do_not_change_the_candidate_identity() {
    let baseline = document();
    let mut settings = settings();
    settings.settings.bindings.reverse();
    let mut rights = rights();
    rights.reverse();
    rights.push(right(
        "rights-unrelated",
        ProductionRightsSubject::Evidence {
            record_id: "rights-unrelated".into(),
        },
    ));
    let candidate = prepare_inputs("owner", &revision(), &settings, &rights, true, NOW)
        .unwrap()
        .document();
    assert_eq!(candidate, baseline);
}

#[test]
fn cost_rounding_is_independent_per_dialogue_exact_and_integer_bounded() {
    for units in 0..=32_u64 {
        for amount in 0..=12_u64 {
            for denominator in 1..=12_u64 {
                let mut budget = settings().settings.budget;
                let rate = budget.rate.as_mut().unwrap();
                rate.amount_minor = amount;
                rate.units_per_charge = denominator;
                // Integer arithmetic oracle authored separately from div_ceil implementation.
                let product = units * amount;
                let expected = product / denominator + u64::from(product % denominator != 0);
                match estimate_cost(ProductionCurrency::VND, units, &budget) {
                    ProductionEstimate::Known { amount_minor, .. } => {
                        assert_eq!(amount_minor, expected)
                    }
                    other => panic!("unexpected estimate {other:?}"),
                }
            }
        }
    }
    let mut budget = settings().settings.budget;
    budget.rate.as_mut().unwrap().amount_minor = MAX_MONEY_MINOR;
    budget.rate.as_mut().unwrap().units_per_charge = 1;
    assert!(matches!(
        estimate_cost(ProductionCurrency::VND, 1, &budget),
        ProductionEstimate::Known {
            amount_minor: MAX_MONEY_MINOR,
            ..
        }
    ));
    assert_eq!(
        estimate_cost(ProductionCurrency::VND, 2, &budget),
        ProductionEstimate::Unavailable {
            reason: "estimate_overflow".into()
        }
    );
    assert_eq!(
        estimate_cost(ProductionCurrency::USD, 1, &budget),
        ProductionEstimate::Unavailable {
            reason: "invalid_or_out_of_range_rate".into()
        }
    );
    budget.rate = None;
    assert_eq!(
        estimate_cost(ProductionCurrency::VND, 0, &budget),
        ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into()
        }
    );
    let mut settings = settings();
    settings.settings.budget.limit_minor = 3;
    assert_eq!(
        codes(&settings, &rights(), true, NOW),
        vec![ProductionFindingCode::BudgetExceeded]
    );
}

#[test]
fn pronunciation_is_longest_first_nonrecursive_nfc_and_costs_effective_text() {
    let mut raw: Value = serde_json::from_slice(SCRIPT).unwrap();
    raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!("ab");
    let revision = revision_from_json(&raw);
    let mut settings = settings();
    settings.settings.bindings[0].pronunciation = vec![
        ProductionPronunciation {
            surface: "a".into(),
            replacement: "b".into(),
        },
        ProductionPronunciation {
            surface: "b".into(),
            replacement: "c".into(),
        },
    ];
    let candidate = prepare_inputs("owner", &revision, &settings, &rights(), true, NOW)
        .unwrap()
        .document();
    assert_eq!(candidate.resolved[0].text, "bc");
    settings.settings.bindings[0]
        .pronunciation
        .push(ProductionPronunciation {
            surface: "ab".into(),
            replacement: "y".into(),
        });
    let candidate = prepare_inputs("owner", &revision, &settings, &rights(), true, NOW)
        .unwrap()
        .document();
    assert_eq!(candidate.resolved[0].text, "y");
    raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!("x\u{301}");
    let revision = revision_from_json(&raw);
    settings.settings.bindings[0].pronunciation = vec![ProductionPronunciation {
        surface: "x".into(),
        replacement: "e".into(),
    }];
    let candidate = prepare_inputs("owner", &revision, &settings, &rights(), true, NOW)
        .unwrap()
        .document();
    assert_eq!(candidate.resolved[0].text, "é");
    assert!(matches!(
        candidate.estimate,
        ProductionEstimate::Known { units: 95, .. }
    ));
    assert!(candidate.revision.script_json.contains("x\u{301}"));
}

#[test]
fn pronunciation_amplification_is_rejected_before_allocating_unbounded_output() {
    let mut raw: Value = serde_json::from_slice(SCRIPT).unwrap();
    raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] = json!("a".repeat(200));
    let revision = revision_from_json(&raw);
    let mut settings = settings();
    settings.settings.bindings[0].pronunciation = vec![ProductionPronunciation {
        surface: "a".into(),
        replacement: "b".repeat(512),
    }];
    assert_eq!(
        prepare_inputs("owner", &revision, &settings, &rights(), true, NOW)
            .err()
            .unwrap(),
        vec![FieldIssue {
            path: "dialogues.dialogue-01.text".into(),
            rule: "effective_text_too_large".into()
        }]
    );
}

#[test]
fn conflicting_pronunciation_and_forged_canonical_revision_fail_admission() {
    let mut settings = settings();
    settings.settings.bindings[1].pronunciation = vec![ProductionPronunciation {
        surface: "Vọng Đài".into(),
        replacement: "Vong Dai".into(),
    }];
    assert_eq!(
        prepare_inputs("owner", &revision(), &settings, &rights(), true, NOW)
            .err()
            .unwrap(),
        vec![FieldIssue {
            path: "dialogues.dialogue-04.pronunciation".into(),
            rule: "conflicting_pronunciation".into()
        }]
    );
    let mut revision = revision();
    revision.export_digest = "sir-e1:sha256:forged".into();
    assert_eq!(
        prepare_inputs("owner", &revision, &crate::settings(), &rights(), true, NOW)
            .err()
            .unwrap(),
        vec![FieldIssue {
            path: "revision".into(),
            rule: "digest_mismatch".into()
        }]
    );
}

#[test]
fn authorization_witness_agrees_with_the_current_owner_gate() {
    let document = document();
    let approval = approval(&document);
    for owner in ["owner", "new-owner"] {
        let current = evaluate_for_owner(
            &document,
            owner,
            1,
            1,
            &rights(),
            true,
            Some(&approval),
            NOW,
        );
        let witness = authorize_inputs(
            &document,
            owner,
            1,
            1,
            &rights(),
            true,
            Some(&approval),
            NOW,
        );
        assert_eq!(witness.is_ok(), current.approval_current, "{owner}");
        if let Err(findings) = witness {
            assert_eq!(findings, current.findings, "{owner}");
        }
    }
}
