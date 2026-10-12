use cantos_api::script_ir::read_canonical_script;
use cantos_contracts::RevisionResponse;
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
