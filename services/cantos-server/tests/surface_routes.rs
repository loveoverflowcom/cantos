//! Route isolation of the three API surfaces behind the real router, without PostgreSQL.
//! Every request here is decided before any store access, so the lazy pool is never dialled.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
    Router,
};
use cantos_server::adapters::inbound::http::{router, AppState};
use cantos_server::contract::routes::{ADMIN_BASE, API_BASE, STUDIO_BASE, THEATRE_BASE};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const ORIGIN: &str = "http://127.0.0.1:8080";
const SCRIPT: &str = "ad04abb5-bba0-4240-b211-3ed2aa179999";

fn app() -> Router {
    let state = AppState {
        store: cantos_server::test_support::detached_store(),
        web_origin: ORIGIN.into(),
    };
    router(state, "apps/cantos-studio/web/dist")
}

struct Reply {
    status: StatusCode,
    deprecation: Option<String>,
    body: Value,
}

async fn call(method: &str, path: &str, cookie: bool, body: &str) -> Reply {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json");
    if cookie {
        request = request.header(header::COOKIE, "cantos_session=x; cantos_session=y");
    }
    let response = app()
        .oneshot(request.body(Body::from(body.to_owned())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let deprecation = response
        .headers()
        .get("deprecation")
        .map(|value| value.to_str().unwrap().to_owned());
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    Reply {
        status,
        deprecation,
        body: serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    }
}

#[test]
fn surface_bases_are_distinct_children_of_the_api_root() {
    assert_eq!(STUDIO_BASE, format!("{API_BASE}/studio"));
    assert_eq!(THEATRE_BASE, format!("{API_BASE}/theatre"));
    assert_eq!(ADMIN_BASE, format!("{API_BASE}/admin"));
}

#[tokio::test]
async fn unimplemented_theatre_and_admin_paths_serve_nothing_even_with_studio_session() {
    for base in [THEATRE_BASE, ADMIN_BASE] {
        for path in [
            "".to_owned(),
            "/session".to_owned(),
            "/catalog".to_owned(),
            format!("/scripts/{SCRIPT}/head"),
            "/production/catalog".to_owned(),
            "/imports".to_owned(),
        ] {
            for cookie in [false, true] {
                for method in ["GET", "POST", "DELETE"] {
                    let reply = call(method, &format!("{base}{path}"), cookie, "{}").await;
                    assert_eq!(reply.status, StatusCode::NOT_FOUND, "{method} {base}{path}");
                    assert_eq!(reply.body["code"], "not_found");
                }
            }
        }
    }
}

#[tokio::test]
async fn surfaces_do_not_leak_routes_into_each_other() {
    for path in [
        format!("{THEATRE_BASE}/scripts/{SCRIPT}/head"),
        format!("{ADMIN_BASE}/scripts/{SCRIPT}/head"),
        format!("{STUDIO_BASE}/theatre/scripts/{SCRIPT}/head"),
        format!("{STUDIO_BASE}/admin/session"),
        format!("{STUDIO_BASE}/studio/scripts/{SCRIPT}/head"),
        format!("{API_BASE}/studio/studio/session"),
        format!("{API_BASE}/theatre/studio/session"),
        format!("{API_BASE}/unknown/session"),
    ] {
        let reply = call("GET", &path, true, "").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{path}");
        assert_eq!(reply.body["code"], "not_found");
    }
}

#[tokio::test]
async fn ordinary_host_never_mounts_contract_fixture_or_publication_bytes() {
    for path in [
        "/fixture-bytes".to_owned(),
        format!("{THEATRE_BASE}/fixture-bytes"),
        format!("{THEATRE_BASE}/publications/{SCRIPT}/bytes"),
    ] {
        for studio_cookie in [false, true] {
            assert_eq!(
                call("GET", &path, studio_cookie, "").await.status,
                StatusCode::NOT_FOUND
            );
        }
    }
}

#[tokio::test]
async fn studio_routes_require_authentication_at_the_current_and_legacy_paths() {
    for suffix in [
        format!("/scripts/{SCRIPT}/head"),
        format!("/scripts/{SCRIPT}/history"),
        "/production/catalog".to_owned(),
        format!("/adaptations/{SCRIPT}/context"),
    ] {
        let current = call("GET", &format!("{STUDIO_BASE}{suffix}"), false, "").await;
        let legacy = call("GET", &format!("{API_BASE}{suffix}"), false, "").await;
        assert_eq!(current.status, StatusCode::UNAUTHORIZED, "{suffix}");
        assert_eq!(current.body["code"], "unauthenticated");
        assert_eq!(
            (legacy.status, &legacy.body),
            (current.status, &current.body)
        );
        // A session cookie sent twice is ambiguous and is refused the same way on both paths.
        let ambiguous = call("GET", &format!("{STUDIO_BASE}{suffix}"), true, "").await;
        assert_eq!(ambiguous.status, StatusCode::UNAUTHORIZED, "{suffix}");
    }
}

#[tokio::test]
async fn legacy_studio_paths_are_flagged_deprecated_and_current_ones_are_not() {
    let current = call(
        "GET",
        &format!("{STUDIO_BASE}/production/catalog"),
        false,
        "",
    )
    .await;
    let legacy = call("GET", &format!("{API_BASE}/production/catalog"), false, "").await;
    assert_eq!(current.deprecation, None);
    assert_eq!(legacy.deprecation.as_deref(), Some("true"));
}

#[tokio::test]
async fn session_endpoint_exists_for_studio_only_and_shares_validation_with_the_legacy_alias() {
    let malformed = json!({"unexpected": true}).to_string();
    for path in [
        format!("{STUDIO_BASE}/session"),
        format!("{API_BASE}/session"),
    ] {
        let reply = call("POST", &path, false, &malformed).await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST, "{path}");
        assert_eq!(reply.body["code"], "invalid_request");
    }
    for path in [
        format!("{THEATRE_BASE}/session"),
        format!("{ADMIN_BASE}/session"),
    ] {
        let reply = call("POST", &path, false, &malformed).await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn origin_guard_covers_every_surface_for_mutations() {
    for base in [STUDIO_BASE, THEATRE_BASE, ADMIN_BASE, API_BASE] {
        let request = Request::builder()
            .method("POST")
            .uri(format!("{base}/session"))
            .header(header::ORIGIN, "https://attacker.invalid")
            .header(header::CONTENT_TYPE, "application/json")
            .body(Body::from("{}"))
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{base}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}

#[tokio::test]
async fn content_identity_routes_exist_only_in_current_studio_and_never_share_its_session() {
    for (method, suffix) in [
        ("GET", format!("/content-units/{SCRIPT}/content/head")),
        ("GET", format!("/content-units/{SCRIPT}/content/history")),
        (
            "GET",
            format!("/content-units/{SCRIPT}/content/revisions/{SCRIPT}"),
        ),
        ("POST", format!("/content-units/{SCRIPT}/content/revisions")),
        ("POST", format!("/content-units/{SCRIPT}/production-briefs")),
        (
            "GET",
            format!("/content-units/{SCRIPT}/production-briefs/1"),
        ),
        (
            "POST",
            format!("/content-units/{SCRIPT}/production-briefs/1/producer-grants"),
        ),
        (
            "GET",
            format!("/producer/content-units/{SCRIPT}/production-briefs/1"),
        ),
    ] {
        let current = call(method, &format!("{STUDIO_BASE}{suffix}"), false, "{}").await;
        assert_eq!(current.status, StatusCode::UNAUTHORIZED);
        assert_eq!(current.body["code"], "unauthenticated");
        for base in [API_BASE, THEATRE_BASE, ADMIN_BASE] {
            for cookie in [false, true] {
                let reply = call(method, &format!("{base}{suffix}"), cookie, "{}").await;
                assert_eq!(
                    reply.status,
                    StatusCode::NOT_FOUND,
                    "{method} {base}{suffix}"
                );
                assert_eq!(reply.body["code"], "not_found");
            }
        }
    }
}

#[tokio::test]
async fn admin_configuration_is_unavailable_without_admin_specific_verifier() {
    let reply = call("GET", &format!("{ADMIN_BASE}/capabilities"), true, "").await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reply.body["code"], "unavailable");
    for base in [STUDIO_BASE, THEATRE_BASE, API_BASE] {
        let reply = call("GET", &format!("{base}/capabilities"), true, "").await;
        assert_eq!(reply.status, StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn admin_raw_transport_errors_keep_safe_json_contract_without_database_access() {
    for body in [
        "{",
        r#"{}"#,
        r#"{"operation_id":3}"#,
        r#"{"operation_id":"x","actor_id":"forged"}"#,
    ] {
        let reply = call(
            "POST",
            &format!("{ADMIN_BASE}/actors/minh/revoke-access"),
            false,
            body,
        )
        .await;
        assert_eq!(reply.status, StatusCode::BAD_REQUEST);
        assert_eq!(reply.body["code"], "invalid_request");
    }
    let reply = call(
        "POST",
        &format!("{ADMIN_BASE}/actors/%FF/revoke-access"),
        false,
        r#"{"operation_id":"x"}"#,
    )
    .await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_request");
    let reply = call("POST", &format!("{ADMIN_BASE}/capabilities"), false, "{}").await;
    assert_eq!(reply.status, StatusCode::BAD_REQUEST);
    assert_eq!(reply.body["code"], "invalid_request");
}

#[tokio::test]
async fn private_audio_reads_require_studio_authentication_and_have_no_other_surface_alias() {
    for suffix in [
        format!("/scripts/{SCRIPT}/audio-intents/{SCRIPT}/bytes"),
        format!("/scripts/{SCRIPT}/audio-intents/{SCRIPT}/read-access"),
        format!("/content-units/{SCRIPT}/audio-submissions/{SCRIPT}/renditions/{SCRIPT}/bytes"),
        format!(
            "/content-units/{SCRIPT}/audio-submissions/{SCRIPT}/renditions/{SCRIPT}/read-access"
        ),
    ] {
        for method in ["GET", "HEAD"] {
            for cookie in [false, true] {
                let current = call(method, &format!("{STUDIO_BASE}{suffix}"), cookie, "").await;
                assert_eq!(
                    current.status,
                    StatusCode::UNAUTHORIZED,
                    "{method} {STUDIO_BASE}{suffix}"
                );
                if method == "GET" {
                    assert_eq!(current.body["code"], "unauthenticated");
                }
                for base in [API_BASE, THEATRE_BASE, ADMIN_BASE] {
                    let reply = call(method, &format!("{base}{suffix}"), cookie, "").await;
                    assert_eq!(
                        reply.status,
                        StatusCode::NOT_FOUND,
                        "{method} {base}{suffix}"
                    );
                }
            }
        }
        let request = Request::builder()
            .uri(format!("{STUDIO_BASE}{suffix}"))
            .header(header::AUTHORIZATION, "Bearer producer-upload-capability")
            .body(Body::empty())
            .unwrap();
        let response = app().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED, "{suffix}");
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
}

#[tokio::test]
async fn retired_provider_adaptation_routes_are_not_mounted_on_any_surface() {
    for base in [STUDIO_BASE, API_BASE, THEATRE_BASE, ADMIN_BASE] {
        for (method, suffix) in [
            ("GET", "/adaptations/provider".to_owned()),
            ("POST", "/adaptations".to_owned()),
            ("GET", format!("/adaptations/{SCRIPT}")),
            ("POST", format!("/adaptations/{SCRIPT}/cancel")),
        ] {
            for cookie in [false, true] {
                let path = format!("{base}{suffix}");
                let reply = call(method, &path, cookie, "{}").await;
                assert_eq!(reply.status, StatusCode::NOT_FOUND, "{method} {path}");
                assert_eq!(reply.body["code"], "not_found", "{method} {path}");
            }
        }
    }
}
