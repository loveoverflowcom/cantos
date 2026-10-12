use cantos_api::contexts::production::{
    admit_frozen_document, prepare_inputs, production_catalog, production_input_digest,
    validate_rights_declaration, validate_settings,
};
use cantos_api::script_ir::read_script;
use cantos_contracts::{
    ProductionSettingsResponse, RevisionResponse, SaveProductionRightsRequest,
    SaveProductionSettingsRequest,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[test]
fn production_requests_match_schema_raw_dtos_and_pure_admission() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/production/v1.schema.json"
    ))
    .unwrap();
    assert!(jsonschema::draft202012::meta::is_valid(&schema));
    for (definition, bytes) in [
        (
            "SaveProductionSettingsRequest",
            include_str!(
                "../../../packages/contracts/fixtures/production/v1/settings-request.json"
            ),
        ),
        (
            "SaveProductionRightsRequest",
            include_str!("../../../packages/contracts/fixtures/production/v1/rights-request.json"),
        ),
    ] {
        let selected = json!({"$ref":format!("#/$defs/{definition}"),"$defs":schema["$defs"]});
        let validator = jsonschema::validator_for(&selected).unwrap();
        let value: Value = serde_json::from_str(bytes).unwrap();
        validator.validate(&value).unwrap();
        let mut fabricated = value.clone();
        fabricated["legally_verified"] = json!(true);
        assert!(!validator.is_valid(&fabricated));
    }
    let settings: SaveProductionSettingsRequest = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/production/v1/settings-request.json"
    ))
    .unwrap();
    validate_settings(&settings.settings).unwrap();
    let rights: SaveProductionRightsRequest = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/production/v1/rights-request.json"
    ))
    .unwrap();
    validate_rights_declaration(&rights.claim).unwrap();
}

#[test]
fn prepared_responses_preserve_large_admitted_script_ir_pronunciation_with_separate_bounds() {
    let mut raw: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    ))
    .unwrap();
    raw["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["pronunciation_overrides"] = json!([
        {"surface":"Ánh","replacement":"a".repeat(600)}
    ]);
    let admitted = read_script(&serde_json::to_vec(&raw).unwrap()).unwrap();
    let script_json = String::from_utf8(admitted.export_bytes()).unwrap();
    let mut export_hash = Sha256::new();
    export_hash.update(b"cantos/script-export/e1\n");
    export_hash.update(script_json.as_bytes());
    let revision = RevisionResponse {
        script_id: "00000000-0000-4000-8000-000000000053".into(),
        revision: 1,
        accepted_by: "fixture-owner".into(),
        accepted_at: "2026-10-10T00:00:00.000000Z".into(),
        content_digest: admitted.content_digest().to_string(),
        export_digest: format!("sir-e1:sha256:{:x}", export_hash.finalize()),
        script_json,
    };
    let request: SaveProductionSettingsRequest = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/production/v1/settings-request.json"
    ))
    .unwrap();
    let settings = ProductionSettingsResponse {
        script_id: revision.script_id.clone(),
        version: 1,
        script_revision: 1,
        operation_id: request.operation_id,
        recorded_by: "fixture-owner".into(),
        recorded_at: "2026-10-10T00:00:01.000000Z".into(),
        settings: request.settings,
    };
    let candidate = prepare_inputs("fixture-owner", &revision, &settings, &[], true, 100).unwrap();
    let preview = candidate.preview();
    assert_eq!(
        preview.resolved[0].pronunciation[0]
            .replacement
            .chars()
            .count(),
        600
    );
    assert!(preview.resolved[0].text.starts_with(&"a".repeat(600)));

    let document: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/production/v1.schema.json"
    ))
    .unwrap();
    for (definition, response) in [
        (
            "ProductionPreviewResponse",
            serde_json::to_value(preview).unwrap(),
        ),
        (
            "FrozenProductionDocument",
            serde_json::to_value(candidate.document()).unwrap(),
        ),
    ] {
        let schema = json!({"$ref":format!("#/$defs/{definition}"),"$defs":document["$defs"]});
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&response)
            .unwrap();
    }
    // Character presets keep their smaller independent request limit.
    let mut invalid_preset = settings.settings;
    invalid_preset.bindings[0]
        .pronunciation
        .push(cantos_contracts::ProductionPronunciation {
            surface: "Ánh".into(),
            replacement: "a".repeat(600),
        });
    assert_eq!(
        validate_settings(&invalid_preset),
        Err(vec![cantos_contracts::FieldIssue {
            path: "bindings[0].pronunciation[0].replacement".into(),
            rule: "text_length_or_normalization".into(),
        }])
    );
}

fn schema_validator(definition: &str) -> jsonschema::Validator {
    let document: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/production/v1.schema.json"
    ))
    .unwrap();
    let schema = json!({"$ref":format!("#/$defs/{definition}"),"$defs":document["$defs"]});
    jsonschema::validator_for(&schema).unwrap()
}

#[test]
fn every_implemented_catalog_identity_round_trips_through_the_published_schema() {
    let catalog = serde_json::to_value(production_catalog()).unwrap();
    schema_validator("ProductionCatalogResponse")
        .validate(&catalog)
        .unwrap();
    let mut request: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/production/v1/settings-request.json"
    ))
    .unwrap();
    let bindings = request["settings"]["bindings"].as_array_mut().unwrap();
    for (binding, (model, voice)) in bindings.iter_mut().zip([
        ("google-neural2", "vi-VN-Neural2-A"),
        ("google-chirp3-hd", "vi-VN-Chirp3-HD-Aoede"),
        ("google-wavenet", "vi-VN-Wavenet-D"),
    ]) {
        binding["provider_id"] = json!("google-cloud-tts");
        binding["model_id"] = json!(model);
        binding["voice_id"] = json!(voice);
        binding["voice_rights_record_id"] = json!(format!(
            "rights-voice-{}",
            binding["character_id"].as_str().unwrap()
        ));
        binding["performance"] = json!({
            "rate_permille": 1000,
            "pitch_semitones": 0,
            "emotion": "neutral",
            "intensity_permille": 0,
        });
    }
    schema_validator("SaveProductionSettingsRequest")
        .validate(&request)
        .unwrap();
    let request: SaveProductionSettingsRequest = serde_json::from_value(request).unwrap();
    validate_settings(&request.settings).unwrap();
    let script = read_script(include_bytes!(
        "../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    ))
    .unwrap();
    let script_json = String::from_utf8(script.export_bytes()).unwrap();
    let mut export_hash = Sha256::new();
    export_hash.update(b"cantos/script-export/e1\n");
    export_hash.update(script_json.as_bytes());
    let revision = RevisionResponse {
        script_id: "00000000-0000-4000-8000-000000000054".into(),
        revision: 1,
        accepted_by: "fixture-owner".into(),
        accepted_at: "2026-10-11T00:00:00.000000Z".into(),
        content_digest: script.content_digest().to_string(),
        export_digest: format!("sir-e1:sha256:{:x}", export_hash.finalize()),
        script_json,
    };
    let settings = ProductionSettingsResponse {
        script_id: revision.script_id.clone(),
        version: 1,
        script_revision: 1,
        operation_id: request.operation_id,
        recorded_by: "fixture-owner".into(),
        recorded_at: "2026-10-11T00:00:01.000000Z".into(),
        settings: request.settings,
    };
    let candidate = prepare_inputs("fixture-owner", &revision, &settings, &[], true, 100).unwrap();
    assert!(candidate
        .document()
        .resolved
        .iter()
        .all(|line| line.adapter_version == "google-cloud-tts-synthesize-3"));
    schema_validator("ProductionPreviewResponse")
        .validate(&serde_json::to_value(candidate.preview()).unwrap())
        .unwrap();
    schema_validator("FrozenProductionDocument")
        .validate(&serde_json::to_value(candidate.document()).unwrap())
        .unwrap();
    // Previously frozen v2 is still a valid history wire value, while new candidates emit v3.
    let mut historical = candidate.document();
    for line in &mut historical.resolved {
        line.adapter_version = "google-cloud-tts-synthesize-2".into();
    }
    historical.input_digest = production_input_digest(&historical).unwrap();
    schema_validator("FrozenProductionDocument")
        .validate(&serde_json::to_value(&historical).unwrap())
        .unwrap();
    assert_eq!(
        admit_frozen_document(
            &historical,
            &historical.revision,
            &historical.settings,
            &historical.rights
        ),
        Ok(())
    );
    let mut unsupported = serde_json::to_value(historical).unwrap();
    unsupported["resolved"][0]["adapter_version"] = json!("google-cloud-tts-synthesize-unknown");
    assert!(!schema_validator("FrozenProductionDocument").is_valid(&unsupported));
}

/// Studio casting tests consume this fixture; it must stay the exact served catalog.
#[test]
fn published_catalog_fixture_is_the_served_catalog() {
    let fixture: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/production/v1/catalog-response.json"
    ))
    .unwrap();
    assert_eq!(serde_json::to_value(production_catalog()).unwrap(), fixture);
}
