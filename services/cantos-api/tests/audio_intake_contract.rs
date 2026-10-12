use cantos_contracts::{
    ApiError, AudioIntentResponse, AudioIntentStatusResponse, AudioMediaMetadata,
    AudioReceiptResponse, CreateAudioIntentRequest,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

const SCHEMA: &str = include_str!("../../../packages/contracts/schema/audio-intake/v1.schema.json");
const REQUEST: &str =
    include_str!("../../../packages/contracts/fixtures/audio-intake/v1/intent-request.json");
const INTENT: &str =
    include_str!("../../../packages/contracts/fixtures/audio-intake/v1/intent-response.json");
const MEDIA: &str =
    include_str!("../../../packages/contracts/fixtures/audio-intake/v1/media-metadata.json");
const RECEIPT: &str =
    include_str!("../../../packages/contracts/fixtures/audio-intake/v1/receipt-response.json");
const EMPTY: &str =
    include_str!("../../../packages/contracts/fixtures/audio-intake/v1/status-empty.json");
const ACCEPTED: &str =
    include_str!("../../../packages/contracts/fixtures/audio-intake/v1/status-accepted.json");

fn schema(definition: &str) -> jsonschema::Validator {
    let root: Value = serde_json::from_str(SCHEMA).unwrap();
    assert!(jsonschema::draft202012::meta::is_valid(&root));
    jsonschema::validator_for(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": format!("#/$defs/{definition}"),
        "$defs": root["$defs"]
    }))
    .unwrap()
}

fn fixture_conforms<T: DeserializeOwned + Serialize>(definition: &str, fixture: &str) {
    let value: Value = serde_json::from_str(fixture).unwrap();
    schema(definition).validate(&value).unwrap();
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
}

#[test]
fn independent_safe_fixtures_conform_to_schema_and_shared_raw_transport() {
    fixture_conforms::<CreateAudioIntentRequest>("CreateAudioIntentRequest", REQUEST);
    fixture_conforms::<AudioIntentResponse>("AudioIntentResponse", INTENT);
    fixture_conforms::<AudioMediaMetadata>("AudioMediaMetadata", MEDIA);
    fixture_conforms::<AudioReceiptResponse>("AudioReceiptResponse", RECEIPT);
    fixture_conforms::<AudioIntentStatusResponse>("AudioIntentStatusResponse", EMPTY);
    fixture_conforms::<AudioIntentStatusResponse>("AudioIntentStatusResponse", ACCEPTED);
}

#[test]
fn schema_and_raw_consumer_close_each_object_against_forged_approval_or_delivery_facts() {
    let validator = schema("AudioIntentStatusResponse");
    for pointer in ["", "/intent", "/receipt", "/receipt/media"] {
        let mut value: Value = serde_json::from_str(ACCEPTED).unwrap();
        value.pointer_mut(pointer).unwrap()["publication_ready"] = json!(true);
        assert!(!validator.is_valid(&value), "pointer {pointer}");
        let error = serde_json::from_value::<AudioIntentStatusResponse>(value).unwrap_err();
        assert!(error
            .to_string()
            .contains("unknown field `publication_ready`"));
    }
    let mut value: Value = serde_json::from_str(REQUEST).unwrap();
    value["accepted_by"] = json!("another-owner");
    assert!(!schema("CreateAudioIntentRequest").is_valid(&value));
    let error = serde_json::from_value::<CreateAudioIntentRequest>(value).unwrap_err();
    assert!(error.to_string().contains("unknown field `accepted_by`"));
}

#[test]
fn profile_ids_are_closed_explicit_and_map_to_exact_codec_envelopes_without_a_lower_floor() {
    let request_validator = schema("CreateAudioIntentRequest");
    let media_validator = schema("AudioMediaMetadata");
    for (profile, container, codec, maximum) in [
        ("opus-mono-20-v1", "ogg", "opus", 24_000),
        ("opus-mono-24-v1", "ogg", "opus", 28_800),
        ("opus-mono-32-v1", "ogg", "opus", 38_400),
        ("opus-mono-40-v1", "ogg", "opus", 48_000),
    ] {
        let mut request: Value = serde_json::from_str(REQUEST).unwrap();
        request["profile_id"] = json!(profile);
        request_validator.validate(&request).unwrap();
        let mut media: Value = serde_json::from_str(MEDIA).unwrap();
        media["profile_id"] = json!(profile);
        media["container"] = json!(container);
        media["codec"] = json!(codec);
        for bitrate in [1, maximum] {
            media["audio_bitrate_bps"] = json!(bitrate);
            media_validator.validate(&media).unwrap();
            let decoded: AudioMediaMetadata = serde_json::from_value(media.clone()).unwrap();
            assert_eq!(decoded.profile_id, profile);
            assert_eq!(decoded.codec, codec);
        }
        media["audio_bitrate_bps"] = json!(maximum + 1);
        assert!(!media_validator.is_valid(&media), "profile {profile}");
        media["audio_bitrate_bps"] = json!(maximum);
        media["container"] = json!(if container == "ogg" { "mp4" } else { "ogg" });
        assert!(!media_validator.is_valid(&media), "profile {profile}");
        media["container"] = json!(container);
        media["codec"] = json!(if codec == "opus" { "aac_lc" } else { "opus" });
        assert!(!media_validator.is_valid(&media), "profile {profile}");
    }
    for invalid in [
        json!("default"),
        json!("automatic"),
        json!("opus-mono-20"),
        json!("aac-lc-mono-40-v1"),
        Value::Null,
    ] {
        let mut value: Value = serde_json::from_str(REQUEST).unwrap();
        value["profile_id"] = invalid;
        assert!(!request_validator.is_valid(&value));
    }
    let mut missing: Value = serde_json::from_str(REQUEST).unwrap();
    missing.as_object_mut().unwrap().remove("profile_id");
    assert!(!request_validator.is_valid(&missing));
}

#[test]
fn private_media_schema_bounds_duration_size_identity_and_observed_channel_rate() {
    let validator = schema("AudioMediaMetadata");
    for (field, maximum) in [("byte_length", 33_554_432_u64), ("duration_ms", 3_600_000)] {
        let mut value: Value = serde_json::from_str(MEDIA).unwrap();
        for allowed in [1, maximum] {
            value[field] = json!(allowed);
            validator.validate(&value).unwrap();
        }
        for invalid in [
            json!(0),
            json!(maximum + 1),
            json!(-1),
            json!(1.5),
            json!("1"),
            Value::Null,
        ] {
            value[field] = invalid;
            assert!(!validator.is_valid(&value), "field {field}");
        }
    }
    for (field, invalid) in [
        ("sha256", json!("a".repeat(63))),
        ("sha256", json!("A".repeat(64))),
        ("channels", json!(2)),
        ("sample_rate_hz", json!(44_100)),
        ("codec", json!("aac")),
        ("codec", json!("aac-lc")),
        ("codec", json!("aac_lc")),
        ("container", json!("mp4")),
        ("container", json!("wav")),
        ("audio_bitrate_bps", json!(0)),
        ("profile_id", json!("unadopted-profile")),
    ] {
        let mut value: Value = serde_json::from_str(MEDIA).unwrap();
        value[field] = invalid;
        assert!(!validator.is_valid(&value), "field {field}");
    }
}

#[test]
fn historical_aac_proposal_never_becomes_an_accepted_current_intake_profile() {
    let value: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/audio-intake/v1/rejected-aac-media.json"
    ))
    .unwrap();
    assert!(!schema("AudioMediaMetadata").is_valid(&value));
    // A raw DTO preserves the untrusted declaration; the service must admit it separately.
    let raw: AudioMediaMetadata = serde_json::from_value(value).unwrap();
    assert_eq!(raw.codec, "aac_lc");
    assert_eq!(raw.profile_id, "aac-lc-mono-40-v1");
}

#[test]
fn schema_refuses_zero_or_unpersistable_revisions_and_noncanonical_pins_or_public_urls() {
    let validator = schema("CreateAudioIntentRequest");
    for (field, invalid) in [
        ("expected_revision", json!(0)),
        ("expected_revision", json!(9_223_372_036_854_775_808_u64)),
        (
            "operation_id",
            json!("00000000-0000-4000-8000-00000000008A"),
        ),
        (
            "expected_content_digest",
            json!(format!("sir-c1:sha256:{}", "A".repeat(64))),
        ),
        (
            "expected_content_digest",
            json!(format!("sir-e1:sha256:{}", "1".repeat(64))),
        ),
    ] {
        let mut value: Value = serde_json::from_str(REQUEST).unwrap();
        value[field] = invalid;
        assert!(!validator.is_valid(&value), "field {field}");
    }
    let validator = schema("AudioIntentResponse");
    for field in ["upload_path", "status_path"] {
        for invalid in [
            "https://media.example/private-object",
            "/api/v1/theatre/private-object",
            "/api/v1/scripts/private-object",
            "/private-assets/object",
        ] {
            let mut value: Value = serde_json::from_str(INTENT).unwrap();
            value[field] = json!(invalid);
            assert!(!validator.is_valid(&value), "field {field}");
        }
    }
}

#[test]
fn omitted_or_null_receipt_means_only_no_accepted_receipt() {
    let validator = schema("AudioIntentStatusResponse");
    let mut value: Value = serde_json::from_str(EMPTY).unwrap();
    validator.validate(&value).unwrap();
    value.as_object_mut().unwrap().remove("receipt");
    validator.validate(&value).unwrap();
    let decoded: AudioIntentStatusResponse = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.receipt, None);
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["receipt"],
        Value::Null
    );
}

#[test]
fn audio_rejection_conforms_to_shared_studio_error_schema() {
    let root: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/studio/v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$ref": "#/$defs/ApiError",
        "$defs": root["$defs"]
    }))
    .unwrap();
    let value = json!({
        "code": "audio_rejected",
        "current_revision": null,
        "issues": [{"path": "audio", "rule": "malformed_audio"}]
    });
    validator.validate(&value).unwrap();
    let decoded: ApiError = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), value);
    let mut invented = value;
    invented["code"] = json!("audio_approved");
    assert!(!validator.is_valid(&invented));
}
