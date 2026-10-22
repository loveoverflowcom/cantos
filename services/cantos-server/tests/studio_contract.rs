use cantos_contracts::RevisionResponse;
use cantos_server::script_ir::read_canonical_script;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

#[test]
fn studio_wire_fixtures_match_schema_and_independent_complete_export() {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/studio/v1.schema.json"
    ))
    .unwrap();
    for (name, bytes) in [
        (
            "Revision",
            include_str!("../../../packages/contracts/fixtures/studio/v1/revision.json"),
        ),
        (
            "ApiError",
            include_str!("../../../packages/contracts/fixtures/studio/v1/stale-revision.json"),
        ),
        (
            "History",
            include_str!("../../../packages/contracts/fixtures/studio/v1/history.json"),
        ),
        (
            "Review",
            include_str!("../../../packages/contracts/fixtures/studio/v1/review.json"),
        ),
        (
            "ScriptValidation",
            include_str!("../../../packages/contracts/fixtures/studio/v1/validation.json"),
        ),
        (
            "SaveRevision",
            include_str!("../../../packages/contracts/fixtures/studio/v1/save-revision.json"),
        ),
        (
            "ValidateScript",
            include_str!("../../../packages/contracts/fixtures/studio/v1/validate-script.json"),
        ),
    ] {
        let schema = json!({"$ref":format!("#/$defs/{name}"),"$defs":schema["$defs"]});
        jsonschema::validator_for(&schema)
            .unwrap()
            .validate(&serde_json::from_str::<Value>(bytes).unwrap())
            .unwrap();
    }
    let revision: RevisionResponse = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/studio/v1/revision.json"
    ))
    .unwrap();
    let content = read_canonical_script(revision.script_json.as_bytes()).unwrap();
    assert_eq!(
        content.content_digest().to_string(),
        revision.content_digest
    );
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(revision.script_json.as_bytes());
    assert_eq!(
        format!("sir-e1:sha256:{:x}", hash.finalize()),
        revision.export_digest
    );
}

#[test]
fn draft_validation_request_schema_refuses_extra_authority_fields_and_invalid_types() {
    let document: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/studio/v1.schema.json"
    ))
    .unwrap();
    let schema = json!({"$ref":"#/$defs/ValidateScript","$defs":document["$defs"]});
    let validator = jsonschema::validator_for(&schema).unwrap();
    assert!(validator.is_valid(&json!({"script_json":"{}"})));
    for invalid in [
        json!({"script_json":"{}","actor_id":"owner"}),
        json!({"script_json":"{}","reviewed_findings":true}),
        json!({"script_json":{}}),
        json!({"script_json":""}),
        json!({}),
    ] {
        assert!(!validator.is_valid(&invalid), "{invalid}");
    }
}

#[test]
fn catalog_literal_acceptance_and_rejection_fixtures_match_the_schema() {
    let document: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/catalog/v1.schema.json"
    ))
    .unwrap();
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/catalog/v1/cases.json"
    ))
    .unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/catalog/v1");
    for case in cases {
        let name = case["type"].as_str().unwrap();
        let schema = json!({"$ref":format!("#/$defs/{name}"),"$defs":document["$defs"]});
        let validator = jsonschema::validator_for(&schema).unwrap();
        let bytes = std::fs::read(directory.join(case["fixture"].as_str().unwrap())).unwrap();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            validator.is_valid(&value),
            case["valid"].as_bool().unwrap(),
            "{}",
            case["fixture"]
        );
    }
}
#[test]
fn content_identity_literal_contracts_and_independent_digests_match_admission() {
    use cantos_contracts::{AppendContentRequest, ContentRevisionResponse};
    use cantos_server::contexts::catalog::content::wire;
    let root: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/content/v1.schema.json"
    ))
    .unwrap();
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/content/v1/cases.json"
    ))
    .unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../packages/contracts/fixtures/content/v1");
    for case in cases {
        let value: Value = serde_json::from_slice(
            &std::fs::read(directory.join(case["file"].as_str().unwrap())).unwrap(),
        )
        .unwrap();
        let validator = jsonschema::validator_for(&json!({"$defs":root["$defs"],"$ref":format!("#/$defs/{}",case["type"].as_str().unwrap())})).unwrap();
        assert_eq!(
            validator.is_valid(&value),
            case["valid"].as_bool().unwrap(),
            "{case}"
        );
        if case["valid"] == false {
            if case["file"] == "reject-latest-script.json" {
                let request: AppendContentRequest = serde_json::from_value(value).unwrap();
                let Err(cantos_server::contexts::catalog::CatalogStoreError::Storage(
                    cantos_server::platform::store::StoreError::InvalidRequestFields(issues),
                )) = wire::admit_details(request.details)
                else {
                    panic!("expected exact revision issue");
                };
                assert_eq!(
                    issues,
                    vec![cantos_contracts::FieldIssue {
                        path: "details.body.script.revision".into(),
                        rule: "invalid_revision".into()
                    }]
                );
            } else {
                assert!(
                    serde_json::from_value::<AppendContentRequest>(value).is_err(),
                    "{case}"
                );
            }
        } else if case["type"] == "AppendContentRequest" {
            wire::admit_details(
                serde_json::from_value::<AppendContentRequest>(value)
                    .unwrap()
                    .details,
            )
            .unwrap();
        } else {
            let raw: ContentRevisionResponse = serde_json::from_value(value.clone()).unwrap();
            let mut effective = Sha256::new();
            effective.update(b"cantos/cc-c1\n");
            effective.update(
                serde_json::to_vec(
                    &json!({"body":raw.details.body,"language":raw.details.language}),
                )
                .unwrap(),
            );
            assert_eq!(
                raw.content_digest,
                format!("cc-c1:sha256:{:x}", effective.finalize())
            );
            let mut full = value;
            full.as_object_mut().unwrap().remove("revision_digest");
            let mut revision = Sha256::new();
            revision.update(b"cantos/cc-r1\n");
            revision.update(serde_json::to_vec(&full).unwrap());
            assert_eq!(
                raw.revision_digest,
                format!("cc-r1:sha256:{:x}", revision.finalize())
            );
        }
    }
}
