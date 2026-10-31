//! Independent schema fixtures and closed native representations; neither supplies authority.
use cantos_server::contract::{studio::*, theatre::viewer_access::*};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
const STUDIO: &str =
    include_str!("../../contracts/studio/viewer-access/v1/viewer-access.schema.json");
const THEATRE: &str =
    include_str!("../../contracts/theatre/viewer-access/v1/viewer-access.schema.json");
fn accepts(schema: &str, name: &str, value: &Value) -> bool {
    let root: Value = serde_json::from_str(schema).unwrap();
    jsonschema::validator_for(
        &json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")}),
    )
    .unwrap()
    .is_valid(value)
}
fn round<T: Serialize + DeserializeOwned>(v: Value) {
    assert_eq!(
        serde_json::to_value(serde_json::from_value::<T>(v.clone()).unwrap()).unwrap(),
        v
    );
}
#[test]
fn viewer_access_fixtures_are_closed_and_round_trip() {
    let cases: Vec<Value> = serde_json::from_str(include_str!(
        "../../contracts/studio/viewer-access/v1/fixtures/corpus.json"
    ))
    .unwrap();
    for case in cases {
        let name = case["type"].as_str().unwrap();
        let v = case["value"].clone();
        assert!(accepts(STUDIO, name, &v), "{name}");
        match name {
            "ViewerAccessRequest" => round::<ViewerAccessRequest>(v),
            "ViewerGroupRequest" => round::<ViewerGroupRequest>(v),
            "ViewerAccessReceipt" => round::<ViewerAccessReceipt>(v),
            "ViewerAccessState" => round::<ViewerAccessState>(v),
            "ViewerGroups" => round::<ViewerGroups>(v),
            "ChapterMatrixPage" => round::<ChapterMatrixPage>(v),
            "UnitViewerAccessState" => round::<UnitViewerAccessState>(v),
            _ => panic!("unknown fixture"),
        }
    }
    let claim: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/viewer-access/v1/fixtures/claim.json"
    ))
    .unwrap();
    assert!(accepts(THEATRE, "ClaimViewerSharesRequest", &claim));
    round::<ClaimViewerSharesRequest>(claim.clone());
    let claimed: Value = serde_json::from_str(include_str!(
        "../../contracts/theatre/viewer-access/v1/fixtures/claimed.json"
    ))
    .unwrap();
    assert!(accepts(THEATRE, "ClaimViewerSharesReceipt", &claimed));
    round::<ClaimViewerSharesReceipt>(claimed);
    for field in ["email", "actor_id", "subject_id", "groups"] {
        let mut wrong = claim.clone();
        wrong[field] = json!("forged");
        assert!(!accepts(THEATRE, "ClaimViewerSharesRequest", &wrong));
        assert!(serde_json::from_value::<ClaimViewerSharesRequest>(wrong).is_err());
    }
    for kind in ["email", "tier", "studio_role", "unknown"] {
        let wrong = json!({"kind":kind,"email":"forged@example.com"});
        assert!(!accepts(STUDIO, "ViewerPrincipal", &wrong));
        assert!(serde_json::from_value::<ViewerPrincipal>(wrong).is_err());
    }
}

#[test]
fn viewer_nullable_expiry_is_required_on_the_wire() {
    let request = json!({"operation_id":"06520000-0000-4000-8000-000000000001","expected_version":0,"change":{"action":"grant_actor","grant_id":"06520000-0000-4000-8000-000000000002","actor_id":"reader"}});
    assert!(!accepts(STUDIO, "ViewerAccessRequest", &request));
    assert!(serde_json::from_value::<ViewerAccessRequest>(request).is_err());
}
