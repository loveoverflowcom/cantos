use cantos_contracts::{AdaptationFindingCode, AdaptationStatus, StartAdaptationRequest};
use serde_json::{json, Value};

// Independently authored historical v1 destination assertion, retained as a compatibility oracle.
// It does not configure a current generation integration; no backend serializer produced it.
const START: &str = r#"{
  "operation_id":"00000000-0000-4000-8000-000000000003",
  "source_id":"src_synthetic", "script_id":"00000000-0000-4000-8000-000000000004",
  "expected_revision":0, "rights_authorization":true,
  "expected_provider":{
    "provider":"ollama-local", "endpoint":"http://127.0.0.1:11434", "model":"fixture:latest",
    "local_model_digest":null,
    "prompt_version":"cantos-radio-adapt-1", "contract_version":"cantos-adaptation-1",
    "config":{"temperature_milli":200,"seed":0,"num_context":8192,"num_predict":4096,"timeout_seconds":120}
  }
}"#;

#[test]
fn start_round_trip_retains_explicit_destination_model_and_scope_assertion() {
    let request: StartAdaptationRequest = serde_json::from_str(START).unwrap();
    assert_eq!(request.expected_provider.endpoint, "http://127.0.0.1:11434");
    assert_eq!(request.expected_provider.model, "fixture:latest");
    assert!(request.rights_authorization);
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::from_str::<Value>(START).unwrap()
    );
    let mut raw: Value = serde_json::from_str(START).unwrap();
    raw.as_object_mut().unwrap().remove("expected_provider");
    assert!(serde_json::from_value::<StartAdaptationRequest>(raw).is_err());
    let mut raw: Value = serde_json::from_str(START).unwrap();
    raw["approved"] = json!(true);
    assert!(serde_json::from_value::<StartAdaptationRequest>(raw).is_err());
}

#[test]
fn status_and_review_findings_are_closed_and_have_independent_wire_spellings() {
    for (value, expected) in [
        ("queued", AdaptationStatus::Queued),
        ("running", AdaptationStatus::Running),
        ("succeeded", AdaptationStatus::Succeeded),
        ("invalid_output", AdaptationStatus::InvalidOutput),
        ("failed", AdaptationStatus::Failed),
        ("ambiguous", AdaptationStatus::Ambiguous),
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
