//! Raw protocol and real Router rejection checks; no PostgreSQL or real delivery claim.
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use cantos_contracts as dto;
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::postgres::{local_config, Store},
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
fn schema(name: &str, value: &Value) {
    let root: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/publication/v1.schema.json"
    ))
    .unwrap();
    let selected =
        json!({"$schema":root["$schema"],"$defs":root["$defs"],"$ref":format!("#/$defs/{name}")});
    let validator = jsonschema::validator_for(&selected).unwrap();
    let errors: Vec<_> = validator
        .iter_errors(value)
        .map(|e| e.to_string())
        .collect();
    assert!(errors.is_empty(), "{errors:?}");
}
#[test]
fn strict_raw_request_fixtures_match_versioned_schema_and_reject_unknown_fields() {
    for (name, text) in [
        (
            "PublishAudioReleaseRequest",
            include_str!("../../../packages/contracts/fixtures/publication/v1/publish.json"),
        ),
        (
            "RecordDistributionRightsRequest",
            include_str!("../../../packages/contracts/fixtures/publication/v1/clearance.json"),
        ),
        (
            "RecordPublicationApprovalRequest",
            include_str!("../../../packages/contracts/fixtures/publication/v1/approval.json"),
        ),
        (
            "WithdrawAudioReleaseRequest",
            include_str!("../../../packages/contracts/fixtures/publication/v1/withdraw.json"),
        ),
    ] {
        let value: Value = serde_json::from_str(text).unwrap();
        schema(name, &value);
    }
    let mut value: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/publication/v1/publish.json"
    ))
    .unwrap();
    let request: dto::PublishAudioReleaseRequest = serde_json::from_value(value.clone()).unwrap();
    assert_eq!(serde_json::to_value(request).unwrap(), value);
    value["passed"] = json!(true);
    assert!(serde_json::from_value::<dto::PublishAudioReleaseRequest>(value).is_err());
}
#[tokio::test]
async fn real_publication_routes_require_studio_auth_and_have_no_legacy_or_listener_alias() {
    let app = router(
        AppState {
            store: Store::new(
                local_config("host=127.0.0.1 port=1 user=unused dbname=unused").unwrap(),
            )
            .unwrap(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "unused-public-root",
    );
    let unit = "20000000-0000-4000-8000-000000000002";
    let candidate = "50000000-0000-4000-8000-000000000005";
    for (method, suffix) in [
        ("GET", format!("release-candidates/{candidate}/readiness")),
        (
            "POST",
            format!("release-candidates/{candidate}/distribution-rights"),
        ),
        (
            "POST",
            format!("release-candidates/{candidate}/publication-approvals"),
        ),
        ("POST", format!("release-candidates/{candidate}/publish")),
        ("GET", "publication/active".into()),
        ("POST", "publication/withdraw".into()),
    ] {
        for (prefix, expected) in [
            ("/api/v1/studio", StatusCode::UNAUTHORIZED),
            ("/api/v1", StatusCode::NOT_FOUND),
            ("/api/v1/theatre", StatusCode::NOT_FOUND),
        ] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(format!("{prefix}/content-units/{unit}/{suffix}"))
                        .header("origin", "http://127.0.0.1:8080")
                        .header("content-type", "application/json")
                        .body(Body::from("{}"))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), expected, "{method} {prefix}/{suffix}");
            assert_eq!(response.headers()["cache-control"], "no-store");
            let body: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            if prefix == "/api/v1/theatre" {
                assert_eq!(body, json!({"code":"not_found","retryable":false}));
            } else {
                schema("ApiError", &body);
            }
        }
    }
}
