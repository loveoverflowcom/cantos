use cantos_api::tts::attempt::{
    plan_recovery, AttemptFacts, AttemptFactsError, ContractCapability, ContractEvidence,
    DeliveryState, FailureClass, LookupReference, ProviderReferenceFacts,
    ReconciliationCapabilities, RecoveryPlan, MAX_CONTRACT_EVIDENCE_BYTES,
    MAX_PROVIDER_REFERENCE_BYTES,
};
use serde_json::{json, Value};

const FAILURES: [FailureClass; 7] = [
    FailureClass::Timeout,
    FailureClass::RateLimit,
    FailureClass::UnsupportedVoice,
    FailureClass::UnsupportedControl,
    FailureClass::Permanent,
    FailureClass::MalformedAudio,
    FailureClass::Cancelled,
];

fn evidence() -> ContractEvidence {
    ContractEvidence::new("synthetic-contract-case-v1".into()).unwrap()
}

fn declared() -> ContractCapability {
    ContractCapability::Declared {
        evidence: evidence(),
    }
}

fn verified() -> ContractCapability {
    ContractCapability::Verified {
        evidence: evidence(),
    }
}

fn references(operation: Option<&str>, provider_request: Option<&str>) -> ProviderReferenceFacts {
    ProviderReferenceFacts::new(
        operation.map(str::to_owned),
        provider_request.map(str::to_owned),
    )
    .unwrap()
}

fn no_lookup() -> ReconciliationCapabilities {
    ReconciliationCapabilities::new(
        ContractCapability::Unavailable,
        ContractCapability::Unavailable,
        ContractCapability::Unavailable,
    )
}

fn ambiguous(failure: FailureClass, references: ProviderReferenceFacts) -> AttemptFacts {
    AttemptFacts::new(failure, DeliveryState::MayHaveExecuted, references)
}

#[test]
fn exact_failure_matrix_requires_reconciliation_before_a_repeat() {
    let known_non_execution = [
        (
            FailureClass::Timeout,
            RecoveryPlan::BoundedRetryNeedsFreshAuthorization,
        ),
        (
            FailureClass::RateLimit,
            RecoveryPlan::BoundedRetryNeedsFreshAuthorization,
        ),
        (
            FailureClass::UnsupportedVoice,
            RecoveryPlan::SelectVoiceExplicitly,
        ),
        (FailureClass::UnsupportedControl, RecoveryPlan::FixInput),
        (FailureClass::Permanent, RecoveryPlan::FixInput),
        (FailureClass::MalformedAudio, RecoveryPlan::InspectOutput),
        (FailureClass::Cancelled, RecoveryPlan::Stop),
    ];
    for (failure, expected) in known_non_execution {
        for delivery in [DeliveryState::NotSent, DeliveryState::ConfirmedNotExecuted] {
            let attempt = AttemptFacts::new(
                failure,
                delivery,
                references(Some("operation-1"), Some("provider-request-1")),
            );
            assert_eq!(plan_recovery(&attempt, &no_lookup()), expected);
        }
        let attempt = ambiguous(
            failure,
            references(Some("operation-1"), Some("provider-request-1")),
        );
        assert_eq!(
            plan_recovery(&attempt, &no_lookup()),
            RecoveryPlan::ManualReconciliation
        );
    }
}

#[test]
fn idempotency_claim_alone_never_recommends_resubmitting_ambiguous_work() {
    for idempotency in [ContractCapability::Unavailable, declared(), verified()] {
        let capabilities = ReconciliationCapabilities::new(
            ContractCapability::Unavailable,
            ContractCapability::Unavailable,
            idempotency.clone(),
        );
        for failure in FAILURES {
            assert_eq!(
                plan_recovery(
                    &ambiguous(
                        failure,
                        references(Some("operation-1"), Some("provider-request-1"))
                    ),
                    &capabilities
                ),
                RecoveryPlan::ManualReconciliation
            );
        }
        assert_eq!(capabilities.same_key_idempotency(), &idempotency);
    }
}

#[test]
fn declared_lookup_is_insufficient_even_when_both_identifiers_are_recorded() {
    let capabilities = ReconciliationCapabilities::new(declared(), declared(), verified());
    for failure in FAILURES {
        assert_eq!(
            plan_recovery(
                &ambiguous(
                    failure,
                    references(Some("operation-1"), Some("provider-request-1"))
                ),
                &capabilities
            ),
            RecoveryPlan::ManualReconciliation
        );
    }
}

#[test]
fn verified_request_lookup_reconciles_every_ambiguous_failure_class() {
    let capabilities = ReconciliationCapabilities::new(
        ContractCapability::Unavailable,
        verified(),
        ContractCapability::Unavailable,
    );
    for failure in FAILURES {
        assert_eq!(
            plan_recovery(
                &ambiguous(failure, references(None, Some("provider-request-1"))),
                &capabilities
            ),
            RecoveryPlan::Lookup {
                reference: LookupReference::ProviderRequestId("provider-request-1".into()),
                contract_evidence: evidence(),
            }
        );
    }
}

#[test]
fn missing_request_identifier_requires_manual_reconciliation_without_operation_lookup() {
    let capabilities = ReconciliationCapabilities::new(declared(), verified(), verified());
    for failure in FAILURES {
        assert_eq!(
            plan_recovery(
                &ambiguous(failure, references(Some("operation-1"), None)),
                &capabilities
            ),
            RecoveryPlan::ManualReconciliation
        );
    }
}

#[test]
fn verified_operation_lookup_is_an_explicit_alternative_to_request_lookup() {
    let capabilities = ReconciliationCapabilities::new(verified(), declared(), verified());
    for failure in FAILURES {
        assert_eq!(
            plan_recovery(
                &ambiguous(
                    failure,
                    references(Some("operation-1"), Some("provider-request-1"))
                ),
                &capabilities
            ),
            RecoveryPlan::Lookup {
                reference: LookupReference::OperationId("operation-1".into()),
                contract_evidence: evidence(),
            }
        );
    }
}

#[test]
fn missing_both_identifiers_cannot_be_replaced_with_contract_evidence() {
    let capabilities = ReconciliationCapabilities::new(verified(), verified(), verified());
    for failure in FAILURES {
        assert_eq!(
            plan_recovery(&ambiguous(failure, references(None, None)), &capabilities),
            RecoveryPlan::ManualReconciliation
        );
    }
}

#[test]
fn provider_request_lookup_takes_precedence_when_both_lookup_paths_are_recorded() {
    let capabilities = ReconciliationCapabilities::new(verified(), verified(), verified());
    assert_eq!(
        plan_recovery(
            &ambiguous(
                FailureClass::Timeout,
                references(Some("operation-1"), Some("provider-request-1"))
            ),
            &capabilities
        ),
        RecoveryPlan::Lookup {
            reference: LookupReference::ProviderRequestId("provider-request-1".into()),
            contract_evidence: evidence(),
        }
    );
}

#[test]
fn lookup_capabilities_do_not_turn_known_non_execution_into_reconciliation() {
    let capabilities = ReconciliationCapabilities::new(verified(), verified(), verified());
    for delivery in [DeliveryState::NotSent, DeliveryState::ConfirmedNotExecuted] {
        let attempt = AttemptFacts::new(
            FailureClass::RateLimit,
            delivery,
            references(Some("operation-1"), Some("provider-request-1")),
        );
        assert_eq!(
            plan_recovery(&attempt, &capabilities),
            RecoveryPlan::BoundedRetryNeedsFreshAuthorization
        );
    }
}

#[test]
fn identifier_constructor_partitions_have_exact_errors() {
    for id in [
        "".to_owned(),
        " ".to_owned(),
        "request\n1".to_owned(),
        "yêu-cầu".to_owned(),
        "x".repeat(MAX_PROVIDER_REFERENCE_BYTES + 1),
    ] {
        assert_eq!(
            ProviderReferenceFacts::new(Some(id.clone()), None),
            Err(AttemptFactsError::InvalidOperationId)
        );
        assert_eq!(
            ProviderReferenceFacts::new(None, Some(id)),
            Err(AttemptFactsError::InvalidProviderRequestId)
        );
    }
    for length in [
        1,
        MAX_PROVIDER_REFERENCE_BYTES - 1,
        MAX_PROVIDER_REFERENCE_BYTES,
    ] {
        let id = "x".repeat(length);
        let reference = ProviderReferenceFacts::new(Some(id.clone()), Some(id.clone())).unwrap();
        assert_eq!(reference.operation_id(), Some(id.as_str()));
        assert_eq!(reference.provider_request_id(), Some(id.as_str()));
    }
}

#[test]
fn contract_evidence_constructor_partitions_have_exact_errors() {
    for value in [
        "".to_owned(),
        "   ".to_owned(),
        "contract\nreference".to_owned(),
        "x".repeat(MAX_CONTRACT_EVIDENCE_BYTES + 1),
    ] {
        assert_eq!(
            ContractEvidence::new(value),
            Err(AttemptFactsError::InvalidContractEvidence)
        );
    }
    for length in [
        1,
        MAX_CONTRACT_EVIDENCE_BYTES - 1,
        MAX_CONTRACT_EVIDENCE_BYTES,
    ] {
        let value = "x".repeat(length);
        assert_eq!(
            ContractEvidence::new(value.clone()).unwrap().as_str(),
            value
        );
    }
}

fn wire_attempt() -> Value {
    json!({
        "failure": "timeout",
        "delivery": "may_have_executed",
        "provider_reference": {
            "operation_id": "operation-1",
            "provider_request_id": "provider-request-1"
        }
    })
}

fn wire_capabilities() -> Value {
    json!({
        "lookup_by_operation_id": {"state": "unavailable"},
        "lookup_by_provider_request_id": {
            "state": "verified", "evidence": "synthetic-contract-case-v1"
        },
        "same_key_idempotency": {"state": "declared", "evidence": "synthetic-claim"}
    })
}

#[test]
fn serde_cannot_bypass_reference_or_evidence_validation() {
    let mut value = wire_attempt();
    value["provider_reference"]["operation_id"] = json!("");
    assert_eq!(
        serde_json::from_value::<AttemptFacts>(value)
            .unwrap_err()
            .to_string(),
        "invalid operation identifier"
    );
    let mut value = wire_attempt();
    value["provider_reference"]["provider_request_id"] = json!("invalid request");
    assert_eq!(
        serde_json::from_value::<AttemptFacts>(value)
            .unwrap_err()
            .to_string(),
        "invalid provider request identifier"
    );
    let mut value = wire_capabilities();
    value["lookup_by_provider_request_id"]["evidence"] = json!("  ");
    assert_eq!(
        serde_json::from_value::<ReconciliationCapabilities>(value)
            .unwrap_err()
            .to_string(),
        "invalid contract evidence reference"
    );
}

#[test]
fn serde_rejects_unknown_fields_at_each_fact_boundary() {
    for path in [vec![], vec!["provider_reference"]] {
        let mut value = wire_attempt();
        let mut target = &mut value;
        for segment in path {
            target = &mut target[segment];
        }
        target["undocumented_guarantee"] = json!(true);
        let error = serde_json::from_value::<AttemptFacts>(value).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown field `undocumented_guarantee`"),
            "{error}"
        );
    }
    for path in [
        vec![],
        vec!["lookup_by_operation_id"],
        vec!["lookup_by_provider_request_id"],
        vec!["same_key_idempotency"],
    ] {
        let mut value = wire_capabilities();
        let mut target = &mut value;
        for segment in path {
            target = &mut target[segment];
        }
        target["undocumented_guarantee"] = json!(true);
        let error = serde_json::from_value::<ReconciliationCapabilities>(value).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("unknown field `undocumented_guarantee`"),
            "{error}"
        );
    }
}

#[test]
fn serde_rejects_unknown_failure_delivery_and_capability_variants() {
    for (field, bad_variant) in [("failure", "retryable"), ("delivery", "charged_zero")] {
        let mut value = wire_attempt();
        value[field] = json!(bad_variant);
        let error = serde_json::from_value::<AttemptFacts>(value).unwrap_err();
        assert!(
            error
                .to_string()
                .contains(&format!("unknown variant `{bad_variant}`")),
            "{error}"
        );
    }
    let mut value = wire_capabilities();
    value["lookup_by_operation_id"]["state"] = json!("guaranteed");
    let error = serde_json::from_value::<ReconciliationCapabilities>(value).unwrap_err();
    assert!(error.to_string().contains("unknown variant `guaranteed`"));
}

#[test]
fn serde_round_trip_preserves_facts_and_lookup_advice() {
    let attempt: AttemptFacts = serde_json::from_value(wire_attempt()).unwrap();
    let capabilities: ReconciliationCapabilities =
        serde_json::from_value(wire_capabilities()).unwrap();
    assert_eq!(serde_json::to_value(&attempt).unwrap(), wire_attempt());
    assert_eq!(
        serde_json::to_value(&capabilities).unwrap(),
        wire_capabilities()
    );
    assert_eq!(attempt.failure(), FailureClass::Timeout);
    assert_eq!(attempt.delivery(), DeliveryState::MayHaveExecuted);
    assert_eq!(
        attempt.provider_reference().provider_request_id(),
        Some("provider-request-1")
    );
    assert_eq!(capabilities.lookup_by_provider_request_id(), &verified());
    assert_eq!(
        capabilities.lookup_by_operation_id(),
        &ContractCapability::Unavailable
    );
    assert_eq!(
        serde_json::to_value(plan_recovery(&attempt, &capabilities)).unwrap(),
        json!({
            "action": "lookup",
            "reference": { "kind": "provider_request_id", "id": "provider-request-1" },
            "contract_evidence": "synthetic-contract-case-v1"
        })
    );
}
