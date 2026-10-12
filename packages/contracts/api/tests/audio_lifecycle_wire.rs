use cantos_contracts::{
    ApiError, CreateAudioReviewRequest, CreateAudioSubmissionRequest, ErrorCode,
};
use serde_json::{json, Value};
fn validate(name: &str, value: &Value) -> bool {
    let root: Value =
        serde_json::from_str(include_str!("../../schema/audio-lifecycle/v1.schema.json")).unwrap();
    jsonschema::validator_for(&json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$defs":root["$defs"],"$ref":format!("#/$defs/{name}")})).unwrap().is_valid(value)
}
#[test]
fn audio_raw_roundtrip_requires_explicit_profile_and_rejects_caller_verified_metadata() {
    let value: Value = serde_json::from_str(include_str!(
        "../../fixtures/audio-lifecycle/v1/prepare-recording.json"
    ))
    .unwrap();
    let raw: CreateAudioSubmissionRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(raw).unwrap(), value);
    assert!(validate("CreateAudioSubmissionRequest", &value));
    for profile in ["", "aac-mono-40-v1", "opus-mono-24-v2"] {
        let mut bad = value.clone();
        bad["binding"]["profile_id"] = json!(profile);
        assert!(!validate("CreateAudioSubmissionRequest", &bad));
    }
    let mut bad = value;
    bad["verified_media"] = json!({"codec":"opus"});
    assert!(serde_json::from_value::<CreateAudioSubmissionRequest>(bad.clone()).is_err());
    assert!(!validate("CreateAudioSubmissionRequest", &bad));
}
#[test]
fn reviews_have_closed_decisions_and_api_errors_include_actual_audio_rejection() {
    let review = json!({"operation_id":"60000000-0000-4000-8000-000000000006","rendition_digest":format!("ar-r1:sha256:{}","a".repeat(64)),"sha256":"b".repeat(64),"kind":"qc","decision":"approved","notes":"Named synthetic evidence only","evidence_refs":["fixture"]});
    assert!(serde_json::from_value::<CreateAudioReviewRequest>(review.clone()).is_ok());
    assert!(validate("CreateAudioReviewRequest", &review));
    let mut bad = review;
    bad["decision"] = json!("maybe");
    assert!(serde_json::from_value::<CreateAudioReviewRequest>(bad).is_err());
    let error = serde_json::to_value(ApiError {
        code: ErrorCode::AudioRejected,
        current_revision: None,
        issues: vec![],
    })
    .unwrap();
    assert!(validate("ApiError", &error));
}
