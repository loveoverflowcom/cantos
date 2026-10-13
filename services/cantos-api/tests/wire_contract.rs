//! The negotiated revision routes behind the real router and middleware, without PostgreSQL.
//! Origin and session refusals happen before any store access, so both representations are
//! checked here; database-backed parity lives in `revisions_postgres.rs`.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cantos_api::{
    interfaces::http::{router, AppState},
    platform::postgres::{local_config, Store},
    script_ir::{read_canonical_script, read_script},
};
use cantos_contracts::{
    wire::{decode, decode_pilot, encode_pilot, WireFormat},
    RevisionResponse, SaveRevisionRequest, ValidateScriptRequest,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:8080";
const SCRIPT: &str = "ad04abb5-bba0-4240-b211-3ed2aa179999";
const PROTOBUF_GOLDENS: &str =
    include_str!("../../../packages/contracts/fixtures/protobuf/v1/goldens.json");
const GOLDENS: &str =
    include_str!("../../../packages/contracts/fixtures/studio/v1/msgpack-goldens.json");

fn app() -> Router {
    // The pool is lazy and port 9 is never dialled: every request here is refused first.
    let config = local_config("postgresql://cantos_app@127.0.0.1:9/cantos").unwrap();
    let state = AppState {
        store: Store::new(config).unwrap(),
        web_origin: ORIGIN.into(),
    };
    router(state, "apps/cantos-studio/web/dist")
}

fn save_body(format: WireFormat) -> Vec<u8> {
    let request: SaveRevisionRequest = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/studio/v1/save-revision.json"
    ))
    .unwrap();
    encode_pilot(format, &request).unwrap()
}

fn validate_body(format: WireFormat) -> Vec<u8> {
    let request: ValidateScriptRequest = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/studio/v1/validate-script.json"
    ))
    .unwrap();
    encode_pilot(format, &request).unwrap()
}

struct Exchange {
    status: StatusCode,
    content_type: String,
    cache_control: String,
    nosniff: String,
    body: Vec<u8>,
}

async fn call(
    mount: &str,
    method: &str,
    path: &str,
    format: WireFormat,
    origin: &str,
    cookie: bool,
    body: Vec<u8>,
) -> Exchange {
    let mut request = Request::builder()
        .method(method)
        .uri(format!("{mount}{path}"))
        .header(header::ORIGIN, origin)
        .header(header::ACCEPT, format.media_type());
    if method == "POST" {
        request = request.header(header::CONTENT_TYPE, format.media_type());
    }
    if cookie {
        request = request.header(header::COOKIE, "cantos_session=x; cantos_session=y");
    }
    let response = app()
        .oneshot(request.body(Body::from(body)).unwrap())
        .await
        .unwrap();
    let header = |name| {
        response
            .headers()
            .get(name)
            .map(|value: &axum::http::HeaderValue| value.to_str().unwrap().to_owned())
            .unwrap_or_default()
    };
    Exchange {
        status: response.status(),
        content_type: header(header::CONTENT_TYPE),
        cache_control: header(header::CACHE_CONTROL),
        nosniff: header(header::X_CONTENT_TYPE_OPTIONS),
        body: response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    }
}

fn assert_api_error(exchange: &Exchange, status: StatusCode, code: &str) {
    assert_eq!(exchange.status, status);
    assert_eq!(exchange.content_type, "application/json");
    assert_eq!(exchange.cache_control, "no-store");
    assert_eq!(exchange.nosniff, "nosniff");
    let value: Value = serde_json::from_slice(&exchange.body).unwrap();
    assert_eq!(
        value,
        json!({"code": code, "current_revision": null, "issues": []})
    );
    let schema: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/studio/v1.schema.json"
    ))
    .unwrap();
    let schema = json!({"$ref": "#/$defs/ApiError", "$defs": schema["$defs"]});
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&value)
        .unwrap();
}

/// (method, path, body) for each negotiated route.
fn negotiated_routes(format: WireFormat) -> [(&'static str, String, Vec<u8>); 4] {
    [
        ("GET", format!("/scripts/{SCRIPT}/head"), vec![]),
        ("GET", format!("/scripts/{SCRIPT}/revisions/1"), vec![]),
        (
            "POST",
            format!("/scripts/{SCRIPT}/revisions"),
            save_body(format),
        ),
        ("POST", "/validation".to_owned(), validate_body(format)),
    ]
}

#[tokio::test]
async fn origin_and_session_refusals_are_the_same_json_errors_for_all_representations() {
    for mount in ["/api/v1/studio", "/api/v1"] {
        for format in [
            WireFormat::Json,
            WireFormat::MessagePack,
            WireFormat::Protobuf,
        ] {
            for (method, path, body) in negotiated_routes(format) {
                if method == "POST" {
                    let forged = call(
                        mount,
                        method,
                        &path,
                        format,
                        "https://attacker.invalid",
                        true,
                        body.clone(),
                    )
                    .await;
                    assert_api_error(&forged, StatusCode::FORBIDDEN, "forbidden");
                }
                let anonymous =
                    call(mount, method, &path, format, ORIGIN, false, body.clone()).await;
                assert_api_error(&anonymous, StatusCode::UNAUTHORIZED, "unauthenticated");
                // A duplicated session cookie is refused before any database lookup.
                let ambiguous = call(mount, method, &path, format, ORIGIN, true, body).await;
                assert_api_error(&ambiguous, StatusCode::UNAUTHORIZED, "unauthenticated");
            }
        }
    }
}

fn golden(definition: &str) -> Vec<u8> {
    let goldens: Value = serde_json::from_str(GOLDENS).unwrap();
    let hex = goldens["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["definition"] == definition)
        .unwrap()["msgpack_hex"]
        .as_str()
        .unwrap()
        .to_owned();
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

fn protobuf_golden(name: &str) -> Vec<u8> {
    let goldens: Value = serde_json::from_str(PROTOBUF_GOLDENS).unwrap();
    let hex = goldens["cases"]
        .as_array()
        .unwrap()
        .iter()
        .find(|case| case["name"] == name)
        .unwrap()["protobuf_hex"]
        .as_str()
        .unwrap();
    (0..hex.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(&hex[index..index + 2], 16).unwrap())
        .collect()
}

#[test]
fn canonical_export_bytes_and_both_digests_survive_the_msgpack_representation() {
    let from_msgpack: RevisionResponse =
        decode(WireFormat::MessagePack, &golden("Revision")).unwrap();
    let from_json: RevisionResponse = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/studio/v1/revision.json"
    ))
    .unwrap();
    assert_eq!(
        from_msgpack.script_json.as_bytes(),
        from_json.script_json.as_bytes()
    );
    let content = read_canonical_script(from_msgpack.script_json.as_bytes()).unwrap();
    assert_eq!(
        content.content_digest().to_string(),
        from_msgpack.content_digest
    );
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(from_msgpack.script_json.as_bytes());
    assert_eq!(
        format!("sir-e1:sha256:{:x}", hash.finalize()),
        from_msgpack.export_digest
    );
}

#[test]
fn a_draft_with_vietnamese_and_special_characters_admits_identically_from_either_representation() {
    let from_msgpack: ValidateScriptRequest =
        decode(WireFormat::MessagePack, &golden("ValidateScript")).unwrap();
    let from_json: ValidateScriptRequest =
        decode(WireFormat::Json, &validate_body(WireFormat::Json)).unwrap();
    let from_protobuf: ValidateScriptRequest =
        decode_pilot(WireFormat::Protobuf, &protobuf_golden("validate-script")).unwrap();
    assert_eq!(from_msgpack, from_json);
    assert_eq!(from_protobuf, from_json);
    assert_eq!(
        from_protobuf.script_json.as_bytes(),
        from_json.script_json.as_bytes()
    );
    let text = &from_msgpack.script_json;
    for fragment in [
        "“Đi thôi!”",
        "\\\"khẽ\\\"",
        "C:\\\\kịch",
        "🎭",
        "Nga\u{300}y",
    ] {
        assert!(text.contains(fragment), "{fragment}");
    }
    let admitted = read_script(text.as_bytes()).unwrap();
    assert_eq!(
        admitted.content_digest().to_string(),
        read_script(from_json.script_json.as_bytes())
            .unwrap()
            .content_digest()
            .to_string()
    );
}

#[test]
fn protobuf_retains_exact_canonical_export_and_digest_facts() {
    let json: RevisionResponse = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/studio/v1/revision.json"
    ))
    .unwrap();
    let bytes = protobuf_golden("revision");
    assert_eq!(encode_pilot(WireFormat::Protobuf, &json).unwrap(), bytes);
    let decoded: RevisionResponse = decode_pilot(WireFormat::Protobuf, &bytes).unwrap();
    assert_eq!(decoded, json);
    assert_eq!(decoded.script_json.as_bytes(), json.script_json.as_bytes());
    let canonical = read_canonical_script(decoded.script_json.as_bytes()).unwrap();
    assert_eq!(
        canonical.content_digest().to_string(),
        decoded.content_digest
    );
    let mut hash = Sha256::new();
    hash.update(b"cantos/script-export/e1\n");
    hash.update(decoded.script_json.as_bytes());
    assert_eq!(
        format!("sir-e1:sha256:{:x}", hash.finalize()),
        decoded.export_digest
    );
}
