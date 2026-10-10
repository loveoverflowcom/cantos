use std::fs;
use std::path::Path;

use cantos_server::editorial::{AcceptRequest, CreateRequest, SaveRequest};
use cantos_server::script_ir;
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

#[path = "support/contracts.rs"]
mod contract_schema;

#[derive(Deserialize)]
struct Fixture {
    file: String,
    definition: String,
}

fn fixture(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../contracts/http/fixtures")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("committed fixture exists"))
        .expect("valid fixture JSON")
}

#[test]
fn committed_http_fixtures_conform_to_each_route_definition() {
    let fixtures: Vec<Fixture> = serde_json::from_value(fixture("manifest.json")).unwrap();
    for entry in fixtures {
        let validator = contract_schema::validator(&entry.definition);
        let value = fixture(&entry.file);
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{}: {errors:?}", entry.file);
    }
}

#[test]
fn committed_request_fixtures_decode_through_the_production_dtos() {
    let create: CreateRequest = serde_json::from_value(fixture("create-request.json")).unwrap();
    let save: SaveRequest = serde_json::from_value(fixture("save-request.json")).unwrap();
    let accept: AcceptRequest = serde_json::from_value(fixture("accept-request.json")).unwrap();
    let content = script_ir::parse(create.content).unwrap();

    assert_eq!(content.source_ref(), create.source.id);
    assert_eq!(content.rights_ref(), create.rights.id);
    assert_eq!(save.base_version, 1);
    assert_eq!(accept.version, 2);
    assert_eq!(script_ir::parse(save.content).unwrap(), content);
    assert!(!create.operation_id.is_nil());
    assert!(!save.operation_id.is_nil());
    assert!(!accept.operation_id.is_nil());
}

#[test]
fn response_fixtures_retain_source_bytes_and_the_golden_content_digest() {
    let view = fixture("script-view.json");
    let source_text = view["source"]["original_text"].as_str().unwrap();
    assert_eq!(
        view["source"]["sha256"],
        format!("{:x}", Sha256::digest(source_text.as_bytes()))
    );
    let original = fixture("create-request.json");
    assert_eq!(original["source"]["original_text"], source_text);

    let history = fixture("history.json");
    for record in history["versions"]
        .as_array()
        .unwrap()
        .iter()
        .chain(history["revisions"].as_array().unwrap())
    {
        let content = script_ir::parse(record["content"].clone()).unwrap();
        assert_eq!(record["digest"], content.digest());
        assert_eq!(record["content"], content.as_json());
        assert_eq!(record["actor_id"], view["owner_id"]);
        assert_eq!(content.source_ref(), view["source"]["id"]);
        assert_eq!(content.rights_ref(), view["rights"]["id"]);
    }
    assert_eq!(history["versions"][0], view["head"]);
    assert_eq!(history["versions"][1], fixture("saved-version.json"));
    assert_eq!(history["revisions"][0], fixture("accepted-revision.json"));
}

#[test]
fn request_schema_and_production_dtos_both_reject_actor_spoofing() {
    let mut create = fixture("create-request.json");
    create["actor_id"] = json!("00000000-0000-4000-8000-000000000099");
    let errors: Vec<_> = contract_schema::validator("CreateRequest")
        .iter_errors(&create)
        .map(|error| error.instance_path.to_string())
        .collect();
    assert_eq!(errors, vec![""]);
    let error = match serde_json::from_value::<CreateRequest>(create) {
        Ok(_) => panic!("the request DTO must reject a client-supplied actor"),
        Err(error) => error,
    };
    assert_eq!(error.classify(), serde_json::error::Category::Data);
    assert!(error.to_string().contains("unknown field `actor_id`"));
}

#[test]
fn response_schema_rejects_missing_audit_and_incomplete_conflict_details() {
    let mut version = fixture("saved-version.json");
    version.as_object_mut().unwrap().remove("actor_id");
    let errors: Vec<_> = contract_schema::validator("SavedVersion")
        .iter_errors(&version)
        .map(|error| error.instance_path.to_string())
        .collect();
    assert_eq!(errors, vec![""]);

    let incomplete_conflict = json!({"error": {"code": "conflict"}});
    let errors: Vec<_> = contract_schema::validator("ErrorResponse")
        .iter_errors(&incomplete_conflict)
        .map(|error| error.instance_path.to_string())
        .collect();
    assert_eq!(errors, vec!["/error"]);
}
