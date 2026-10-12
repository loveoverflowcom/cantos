use cantos_contracts::{
    AudioIntentResponse, AudioIntentStatusResponse, AudioMediaMetadata, AudioReceiptResponse,
    CreateAudioIntentRequest, ErrorCode,
};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::fmt::Debug;

const REQUEST: &str = include_str!("../../fixtures/audio-intake/v1/intent-request.json");
const INTENT: &str = include_str!("../../fixtures/audio-intake/v1/intent-response.json");
const MEDIA: &str = include_str!("../../fixtures/audio-intake/v1/media-metadata.json");
const RECEIPT: &str = include_str!("../../fixtures/audio-intake/v1/receipt-response.json");
const EMPTY: &str = include_str!("../../fixtures/audio-intake/v1/status-empty.json");
const ACCEPTED: &str = include_str!("../../fixtures/audio-intake/v1/status-accepted.json");

fn round_trip<T>(fixture: &str) -> T
where
    T: Clone + Debug + DeserializeOwned + Eq + Serialize,
{
    let value: Value = serde_json::from_str(fixture).unwrap();
    let decoded: T = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(decoded.clone(), decoded);
    assert_eq!(serde_json::to_value(&decoded).unwrap(), value);
    decoded
}

fn refuses_unknown_field<T: DeserializeOwned>(fixture: &str, pointer: &str) {
    let mut value: Value = serde_json::from_str(fixture).unwrap();
    value.pointer_mut(pointer).unwrap()["publication_ready"] = json!(true);
    let error = serde_json::from_value::<T>(value).err().unwrap();
    assert!(error
        .to_string()
        .contains("unknown field `publication_ready`"));
}

#[test]
fn independent_transport_fixtures_preserve_explicit_pins_and_private_observed_facts() {
    let request: CreateAudioIntentRequest = round_trip(REQUEST);
    let intent: AudioIntentResponse = round_trip(INTENT);
    let media: AudioMediaMetadata = round_trip(MEDIA);
    let receipt: AudioReceiptResponse = round_trip(RECEIPT);
    let empty: AudioIntentStatusResponse = round_trip(EMPTY);
    let accepted: AudioIntentStatusResponse = round_trip(ACCEPTED);
    assert_eq!(request.expected_revision, 7);
    assert_eq!(request.expected_revision, intent.script_revision);
    assert_eq!(request.expected_content_digest, intent.content_digest);
    assert_eq!(request.profile_id, "opus-mono-20-v1");
    assert_eq!(request.profile_id, intent.profile_id);
    assert_eq!(request.operation_id, intent.operation_id);
    assert_eq!(receipt.intent_id, intent.id);
    assert_eq!(receipt.script_id, intent.script_id);
    assert_eq!(receipt.script_revision, intent.script_revision);
    assert_eq!(receipt.content_digest, intent.content_digest);
    assert_eq!(receipt.profile_id, intent.profile_id);
    assert_eq!(receipt.operation_id, intent.operation_id);
    assert_eq!(receipt.media, media);
    assert_eq!(receipt.media.profile_id, receipt.profile_id);
    assert_eq!(media.container, "ogg");
    assert_eq!(media.codec, "opus");
    assert_eq!(media.channels, 1);
    assert_eq!(media.sample_rate_hz, 48_000);
    assert_eq!(empty.intent, intent);
    assert_eq!(empty.receipt, None);
    assert_eq!(accepted.receipt, Some(receipt));
    assert_eq!(
        intent.status_path,
        format!(
            "/api/v1/studio/scripts/{}/audio-intents/{}",
            intent.script_id, intent.id
        )
    );
    assert_eq!(intent.upload_path, format!("{}/bytes", intent.status_path));
}

#[test]
fn every_raw_object_boundary_refuses_fabricated_authority_or_publication_fields() {
    refuses_unknown_field::<CreateAudioIntentRequest>(REQUEST, "");
    refuses_unknown_field::<AudioIntentResponse>(INTENT, "");
    refuses_unknown_field::<AudioMediaMetadata>(MEDIA, "");
    for pointer in ["", "/media"] {
        refuses_unknown_field::<AudioReceiptResponse>(RECEIPT, pointer);
    }
    for pointer in ["", "/intent", "/receipt", "/receipt/media"] {
        refuses_unknown_field::<AudioIntentStatusResponse>(ACCEPTED, pointer);
    }
}

#[test]
fn intent_requires_every_pin_and_never_fills_a_default_profile_or_revision() {
    for field in [
        "operation_id",
        "expected_revision",
        "expected_content_digest",
        "profile_id",
    ] {
        let mut value: Value = serde_json::from_str(REQUEST).unwrap();
        value.as_object_mut().unwrap().remove(field);
        let error = serde_json::from_value::<CreateAudioIntentRequest>(value).unwrap_err();
        assert!(error
            .to_string()
            .contains(&format!("missing field `{field}`")));
    }
    for invalid in [json!(-1), json!(1.5), json!("7"), Value::Null] {
        let mut value: Value = serde_json::from_str(REQUEST).unwrap();
        value["expected_revision"] = invalid;
        let error = serde_json::from_value::<CreateAudioIntentRequest>(value).unwrap_err();
        assert!(error.to_string().contains("invalid"));
    }
}

#[test]
fn raw_observed_integer_fields_refuse_negative_fractional_string_null_and_u32_overflow() {
    for field in [
        "byte_length",
        "channels",
        "sample_rate_hz",
        "duration_ms",
        "audio_bitrate_bps",
    ] {
        for invalid in [json!(-1), json!(1.5), json!("1"), Value::Null] {
            let mut value: Value = serde_json::from_str(MEDIA).unwrap();
            value[field] = invalid;
            let error = serde_json::from_value::<AudioMediaMetadata>(value).unwrap_err();
            assert!(error.to_string().contains("invalid"), "field {field}");
        }
    }
    for field in ["channels", "sample_rate_hz"] {
        let mut value: Value = serde_json::from_str(MEDIA).unwrap();
        value[field] = json!(4_294_967_296_u64);
        let error = serde_json::from_value::<AudioMediaMetadata>(value).unwrap_err();
        assert!(error.to_string().contains("invalid value"));
    }
}

#[test]
fn nullable_receipt_absence_never_creates_acceptance_and_serializes_explicitly() {
    let mut value: Value = serde_json::from_str(EMPTY).unwrap();
    value.as_object_mut().unwrap().remove("receipt");
    let decoded: AudioIntentStatusResponse = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.receipt, None);
    assert_eq!(
        serde_json::to_value(decoded).unwrap()["receipt"],
        Value::Null
    );
}

#[test]
fn raw_string_fields_are_transport_not_profile_or_media_validation_witnesses() {
    let mut value: Value = serde_json::from_str(REQUEST).unwrap();
    value["profile_id"] = json!("unadopted-profile");
    let decoded: CreateAudioIntentRequest = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.profile_id, "unadopted-profile");
    let mut value: Value = serde_json::from_str(MEDIA).unwrap();
    value["codec"] = json!("producer-claim");
    let decoded: AudioMediaMetadata = serde_json::from_value(value).unwrap();
    assert_eq!(decoded.codec, "producer-claim");
}

#[test]
fn audio_rejection_code_has_one_stable_wire_spelling_without_success_semantics() {
    assert_eq!(
        serde_json::from_value::<ErrorCode>(json!("audio_rejected")).unwrap(),
        ErrorCode::AudioRejected
    );
    assert_eq!(
        serde_json::to_value(ErrorCode::AudioRejected).unwrap(),
        json!("audio_rejected")
    );
    let error = serde_json::from_value::<ErrorCode>(json!("audio_approved")).unwrap_err();
    assert!(error
        .to_string()
        .contains("unknown variant `audio_approved`"));
}
