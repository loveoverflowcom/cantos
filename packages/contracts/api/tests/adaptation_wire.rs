use cantos_contracts::{AdaptationFindingCode, AdaptationReviewResponse, AdaptationStatus};
use serde_json::json;

#[test]
fn status_and_review_findings_are_closed_and_have_independent_wire_spellings() {
    for (value, expected) in [
        ("awaiting_proposal", AdaptationStatus::AwaitingProposal),
        ("succeeded", AdaptationStatus::Succeeded),
        ("cancelled", AdaptationStatus::Cancelled),
        ("accepted", AdaptationStatus::Accepted),
    ] {
        assert_eq!(
            serde_json::from_value::<AdaptationStatus>(json!(value)).unwrap(),
            expected
        );
        assert_eq!(serde_json::to_value(expected).unwrap(), json!(value));
    }
    assert_eq!(
        serde_json::from_value::<AdaptationFindingCode>(json!("unresolved_speaker")).unwrap(),
        AdaptationFindingCode::UnresolvedSpeaker
    );
    assert!(serde_json::from_value::<AdaptationStatus>(json!("publication_approved")).is_err());
    assert!(serde_json::from_value::<AdaptationFindingCode>(json!("rights_cleared")).is_err());
}

#[test]
fn caller_context_and_submission_wire_preserve_unknown_metadata_without_provider_fiction() {
    use cantos_contracts::{AdaptationContextRequest, SubmitAdaptationProposalRequest};
    let context = json!({
        "operation_id":"00000000-0000-4000-8000-000000000005",
        "source_id":"src_synthetic", "source_sha256":"0".repeat(64),
        "extractor_version":"cantos-text-1", "script_id":"00000000-0000-4000-8000-000000000004",
        "expected_revision":0, "rights_authorization":true
    });
    let request: AdaptationContextRequest = serde_json::from_value(context.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), context);
    let submission = json!({
        "operation_id":"00000000-0000-4000-8000-000000000006", "context_digest":"1".repeat(64),
        "proposal_json":"{}", "generation":{
            "host_tool":"Codex", "provider":null, "model":null, "configuration_json":null,
            "prompt_version":"cantos-radio-adapt-1", "usage":null, "cost":null
        }
    });
    let request: SubmitAdaptationProposalRequest =
        serde_json::from_value(submission.clone()).unwrap();
    assert!(request.generation.model.is_none());
    assert!(request.generation.usage.is_none());
    assert!(request.generation.cost.is_none());
    assert_eq!(serde_json::to_value(request).unwrap(), submission);
    let mut forged = submission;
    forged["generation"]["verified"] = json!(true);
    assert!(serde_json::from_value::<SubmitAdaptationProposalRequest>(forged).is_err());
}

#[test]
fn caller_submission_status_and_cost_basis_have_closed_distinct_spellings() {
    use cantos_contracts::{AdaptationCostBasis, AdaptationSubmissionStatus};
    assert_eq!(
        serde_json::to_value(AdaptationStatus::AwaitingProposal).unwrap(),
        json!("awaiting_proposal")
    );
    assert_eq!(
        serde_json::to_value(AdaptationSubmissionStatus::Invalid).unwrap(),
        json!("invalid")
    );
    assert_eq!(
        serde_json::to_value(AdaptationSubmissionStatus::Valid).unwrap(),
        json!("valid")
    );
    assert_eq!(
        serde_json::to_value(AdaptationCostBasis::CallerDeclared).unwrap(),
        json!("caller_declared")
    );
    assert_eq!(
        serde_json::to_value(AdaptationCostBasis::ProviderReported).unwrap(),
        json!("provider_reported")
    );
    assert!(serde_json::from_value::<AdaptationSubmissionStatus>(json!("accepted")).is_err());
}

#[test]
fn retired_provider_statuses_and_legacy_review_shapes_are_rejected() {
    for status in ["queued", "running", "invalid_output", "failed", "ambiguous"] {
        assert!(serde_json::from_value::<AdaptationStatus>(json!(status)).is_err());
    }
    let retired = json!({"workflow":"legacy","run":{},"source":{},"input_revision":null});
    let error = serde_json::from_value::<AdaptationReviewResponse>(retired).unwrap_err();
    assert!(error.to_string().contains("unknown variant `legacy`"));
}

#[test]
fn current_schema_accepts_caller_context_input_and_refuses_provider_execution_shapes() {
    let schema: serde_json::Value =
        serde_json::from_str(include_str!("../../schema/adaptation/v1.schema.json")).unwrap();
    for retired in [
        "Start",
        "Run",
        "ProviderMetadata",
        "ProviderResponse",
        "LegacyReview",
    ] {
        assert!(schema["$defs"].get(retired).is_none(), "{retired}");
    }
    let mut request_schema = schema.clone();
    request_schema["$ref"] = json!("#/$defs/ContextRequest");
    let request = json!({
        "operation_id":"00000000-0000-4000-8000-000000000005",
        "source_id":"src_synthetic", "source_sha256":"0".repeat(64),
        "extractor_version":"cantos-text-1", "script_id":"00000000-0000-4000-8000-000000000004",
        "expected_revision":0, "rights_authorization":true
    });
    let validator = jsonschema::validator_for(&request_schema).unwrap();
    assert!(validator.is_valid(&request));
    let mut provider_input = request;
    provider_input["expected_provider"] = json!({"provider":"retired-execution"});
    assert!(!validator.is_valid(&provider_input));
    let mut review_schema = schema;
    review_schema["$ref"] = json!("#/$defs/Review");
    let validator = jsonschema::validator_for(&review_schema).unwrap();
    assert!(!validator.is_valid(&json!({
        "workflow":"legacy", "run":{}, "source":{}, "input_revision":null
    })));
}
