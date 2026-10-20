use cantos_contracts::admin::{CapabilityResponse, RevokeAccessRequest, RevokeAccessResponse};
#[test]
fn admin_wire_fixtures_match_schema_and_shared_dtos() {
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schemas/admin-v1.schema.json"
    ))
    .unwrap();
    let validator = jsonschema::validator_for(&schema).unwrap();
    let capability: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/admin/v1/capability.json"
    ))
    .unwrap();
    let revocation: serde_json::Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/admin/v1/revocation.json"
    ))
    .unwrap();
    validator.validate(&capability).unwrap();
    validator.validate(&revocation).unwrap();
    let c: CapabilityResponse = serde_json::from_value(capability).unwrap();
    assert_eq!(c.role, "identity_operator");
    let r: RevokeAccessResponse = serde_json::from_value(revocation).unwrap();
    assert!(r.revoked);
    assert!(serde_json::from_str::<RevokeAccessRequest>(
        r#"{"operation_id":"x","actor_id":"forged"}"#
    )
    .is_err());
}
