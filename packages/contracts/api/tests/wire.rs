use cantos_contracts::{ApiError, ErrorCode, RevisionResponse};

#[test]
fn studio_auth_configuration_is_closed_and_contains_no_credential_or_actor() {
    use cantos_contracts::{StudioAuthenticationConfiguration, StudioAuthenticationMode};
    for (mode, literal) in [
        (StudioAuthenticationMode::Development, "development"),
        (StudioAuthenticationMode::Oidc, "oidc"),
    ] {
        let value = serde_json::json!({"mode":literal});
        assert_eq!(
            serde_json::from_value::<StudioAuthenticationConfiguration>(value.clone())
                .unwrap()
                .mode,
            mode
        );
        assert_eq!(
            serde_json::to_value(StudioAuthenticationConfiguration { mode }).unwrap(),
            value
        );
    }
    for value in [
        serde_json::json!({"mode":"admin"}),
        serde_json::json!({"mode":"oidc","actor_id":"forged"}),
        serde_json::json!({"mode":"oidc","secret":"forged"}),
    ] {
        assert!(serde_json::from_value::<StudioAuthenticationConfiguration>(value).is_err());
    }
}

#[test]
fn shared_consumer_decodes_the_committed_v1_revision_and_error_fixtures() {
    let bytes = include_str!("../../fixtures/studio/v1/revision.json");
    let revision: RevisionResponse = serde_json::from_str(bytes).unwrap();
    assert_eq!(revision.revision, 1);
    assert!(revision.script_json.contains("Người dẫn chuyện"));
    let encoded = serde_json::to_value(revision).unwrap();
    assert_eq!(
        encoded,
        serde_json::from_str::<serde_json::Value>(bytes).unwrap()
    );
    let error: ApiError =
        serde_json::from_str(include_str!("../../fixtures/studio/v1/stale-revision.json")).unwrap();
    assert_eq!(error.code, ErrorCode::StaleRevision);
    assert_eq!(error.current_revision, Some(2));
}

#[test]
fn editorial_extensions_round_trip_without_changing_the_revision_contract() {
    let history: cantos_contracts::HistoryResponse =
        serde_json::from_str(include_str!("../../fixtures/studio/v1/history.json")).unwrap();
    let review: cantos_contracts::ReviewResponse =
        serde_json::from_str(include_str!("../../fixtures/studio/v1/review.json")).unwrap();
    assert_eq!(history.reviews, vec![review]);
    assert_eq!(history.revisions[0].revision, 1);
    assert_eq!(history.next_after, None);
    let request = serde_json::json!({"operation_id":"00000000-0000-4000-8000-000000000003", "revision":1, "reviewed_by":"forged"});
    assert!(serde_json::from_value::<cantos_contracts::ReviewRequest>(request).is_err());
}

#[test]
fn validation_preview_is_a_closed_contract_without_revision_or_permission_claims() {
    let bytes = include_str!("../../fixtures/studio/v1/validation.json");
    let validation: cantos_contracts::ScriptValidationResponse =
        serde_json::from_str(bytes).unwrap();
    assert!(validation.issues.is_empty());
    assert_eq!(
        serde_json::to_value(validation).unwrap(),
        serde_json::from_str::<serde_json::Value>(bytes).unwrap()
    );
    let request = cantos_contracts::ValidateScriptRequest {
        script_json: "{}".into(),
    };
    assert_eq!(
        serde_json::to_value(request).unwrap(),
        serde_json::json!({"script_json":"{}"})
    );
    for forged in [
        serde_json::json!({"script_json":"{}","actor_id":"owner"}),
        serde_json::json!({"script_json":"{}","reviewed_findings":true}),
        serde_json::json!({"script_json":"{}","script_id":"target"}),
    ] {
        assert!(serde_json::from_value::<cantos_contracts::ValidateScriptRequest>(forged).is_err());
    }
    assert!(
        serde_json::from_value::<cantos_contracts::ScriptValidationResponse>(
            serde_json::json!({"issues":[],"approved":true})
        )
        .is_err()
    );
}
