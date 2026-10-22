//! Pre-composition regressions for #333.1, using the current origin-pinned HTTP host.
//! No issuer or database is configured: prefix-shaped sentinels are never real sessions.
//! An own-purpose sentinel reaches the unavailable verifier; wrong purposes must fail earlier.
use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::Response,
    Router,
};
use cantos_contracts::web_api::WebSurface;
use cantos_server::adapters::inbound::http::{api_router, with_browser_cors, AppState};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

const STUDIO_ORIGIN: &str = "https://studio.cantos.example";
const THEATRE_ORIGIN: &str = "https://cantos.example";
const SECRET: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

fn api(origin: &str) -> Router {
    api_router(AppState {
        store: cantos_server::test_support::detached_store().without_development_sessions(),
        web_origin: origin.into(),
    })
}

fn browser(surface: WebSurface, origin: &str) -> Router {
    with_browser_cors(api(origin), origin, surface).unwrap()
}

fn bearer(purpose: &str) -> String {
    format!("Bearer ca1_{purpose}_{SECRET}")
}

fn request(method: Method, path: &str, headers: &[(&str, String)]) -> Request<Body> {
    let mut request = Request::builder().method(method).uri(path);
    for (name, value) in headers {
        request = request.header(*name, value);
    }
    request.body(Body::empty()).unwrap()
}

async fn call(app: &Router, request: Request<Body>) -> Response {
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        app.clone().oneshot(request),
    )
    .await
    .expect("the HTTP refusal must precede issuer/database I/O")
    .unwrap()
}

async fn error(response: Response, status: StatusCode, code: &str) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let body: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        body,
        json!({"code": code, "current_revision": null, "issues": []})
    );
}

fn cors_origin(headers: &HeaderMap, origin: Option<&str>) {
    assert_eq!(
        headers
            .get(header::ACCESS_CONTROL_ALLOW_ORIGIN)
            .map(|v| v.to_str().unwrap()),
        origin
    );
    assert_eq!(
        headers
            .get(header::ACCESS_CONTROL_ALLOW_CREDENTIALS)
            .map(|v| v.to_str().unwrap()),
        origin.map(|_| "true")
    );
    assert!(headers.get_all(header::VARY).iter().any(|v| v == "Origin"));
}

#[tokio::test]
async fn pinned_hosts_keep_route_origin_and_credential_purpose_independent() {
    for (surface, origin, purpose) in [
        (WebSurface::Studio, STUDIO_ORIGIN, "s"),
        (WebSurface::Theatre, THEATRE_ORIGIN, "t"),
    ] {
        let app = browser(surface, origin);
        for (path, route_purpose) in [
            ("/api/v1/studio/auth/bearer/session", "s"),
            ("/api/v1/theatre/auth/bearer/session", "t"),
            ("/api/v1/admin/capabilities", "admin"),
            ("/api/v1/production/catalog", "legacy"),
        ] {
            for sent_origin in [
                STUDIO_ORIGIN,
                THEATRE_ORIGIN,
                "https://preview.cantos.example",
                "https://attacker.invalid",
                "https://studio.cantos.example.attacker.invalid",
                "null",
            ] {
                for sent_purpose in ["s", "t", "n"] {
                    let admitted = sent_origin == origin && route_purpose == purpose;
                    let (status, code) = if !admitted {
                        (StatusCode::FORBIDDEN, "forbidden")
                    } else if sent_purpose == purpose {
                        // Positive control for parsing/routing only, never authenticated access.
                        (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
                    } else {
                        (StatusCode::UNAUTHORIZED, "unauthenticated")
                    };
                    let response = call(
                        &app,
                        request(
                            Method::GET,
                            path,
                            &[
                                ("origin", sent_origin.into()),
                                ("authorization", bearer(sent_purpose)),
                            ],
                        ),
                    )
                    .await;
                    cors_origin(response.headers(), admitted.then_some(origin));
                    error(response, status, code).await;
                }
            }
        }
    }
}

#[tokio::test]
async fn duplicate_and_mixed_credentials_cannot_reach_an_own_purpose_verifier() {
    for (surface, origin, path, purpose) in [
        (
            WebSurface::Studio,
            STUDIO_ORIGIN,
            "/api/v1/studio/auth/bearer/session",
            "s",
        ),
        (
            WebSurface::Theatre,
            THEATRE_ORIGIN,
            "/api/v1/theatre/auth/bearer/session",
            "t",
        ),
    ] {
        let app = browser(surface, origin);
        for credentials in [
            vec![],
            vec![("cookie", format!("__Secure-cantos_studio={SECRET}"))],
            vec![("cookie", format!("__Secure-cantos_theatre={SECRET}"))],
            vec![
                ("authorization", bearer(purpose)),
                ("authorization", bearer(purpose)),
            ],
            vec![
                ("authorization", bearer(purpose)),
                ("authorization", bearer("n")),
            ],
            vec![
                ("authorization", bearer(purpose)),
                ("cookie", format!("__Secure-cantos_studio={SECRET}")),
            ],
            vec![
                ("authorization", bearer(purpose)),
                ("cookie", format!("__Secure-cantos_theatre={SECRET}")),
            ],
            vec![(
                "cookie",
                format!("cantos_session={SECRET}; cantos_session={SECRET}"),
            )],
            vec![
                ("cookie", format!("cantos_session={SECRET}")),
                ("cookie", format!("cantos_session={SECRET}")),
            ],
        ] {
            let mut headers = vec![("origin", origin.to_owned())];
            headers.extend(credentials);
            let response = call(&app, request(Method::GET, path, &headers)).await;
            cors_origin(response.headers(), Some(origin));
            error(response, StatusCode::UNAUTHORIZED, "unauthenticated").await;
        }
    }
}

#[tokio::test]
async fn preflight_preserves_each_hosts_origin_method_and_session_header_fence() {
    for (surface, origin, path, own, other) in [
        (
            WebSurface::Studio,
            STUDIO_ORIGIN,
            "/api/v1/studio/session",
            "x-cantos-studio-session",
            "x-cantos-theatre-session",
        ),
        (
            WebSurface::Theatre,
            THEATRE_ORIGIN,
            "/api/v1/theatre/session",
            "x-cantos-theatre-session",
            "x-cantos-studio-session",
        ),
    ] {
        let app = browser(surface, origin);
        for method in ["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH"] {
            for requested in [own, other, "authorization, content-type"] {
                let allowed = method != "PATCH" && requested != other;
                let response = call(
                    &app,
                    request(
                        Method::OPTIONS,
                        path,
                        &[
                            ("origin", origin.into()),
                            ("access-control-request-method", method.into()),
                            ("access-control-request-headers", requested.into()),
                        ],
                    ),
                )
                .await;
                cors_origin(response.headers(), allowed.then_some(origin));
                if allowed {
                    assert_eq!(response.status(), StatusCode::NO_CONTENT);
                    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                    assert_eq!(
                        response.headers()[header::ACCESS_CONTROL_ALLOW_METHODS],
                        method
                    );
                    let headers = response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
                        .to_str()
                        .unwrap();
                    assert!(headers.split(", ").any(|header| header == own));
                    assert!(!headers.split(", ").any(|header| header == other));
                    assert!(response
                        .into_body()
                        .collect()
                        .await
                        .unwrap()
                        .to_bytes()
                        .is_empty());
                } else {
                    error(response, StatusCode::FORBIDDEN, "forbidden").await;
                }
            }
        }
        for headers in [
            vec![("origin", origin.into()), ("origin", origin.into())],
            vec![("origin", "https://preview.cantos.example".into())],
            vec![
                ("origin", origin.into()),
                ("access-control-request-method", "GET".into()),
            ],
        ] {
            let mut headers = headers;
            headers.push(("access-control-request-method", "DELETE".into()));
            let response = call(&app, request(Method::OPTIONS, path, &headers)).await;
            cors_origin(response.headers(), None);
            error(response, StatusCode::FORBIDDEN, "forbidden").await;
        }
    }
}

#[tokio::test]
async fn caller_supplied_host_and_forwarding_cannot_replace_the_pinned_origin() {
    for (surface, origin, path) in [
        (WebSurface::Studio, STUDIO_ORIGIN, "/api/v1/studio/session"),
        (
            WebSurface::Theatre,
            THEATRE_ORIGIN,
            "/api/v1/theatre/session",
        ),
    ] {
        let app = browser(surface, origin);
        for method in [Method::GET, Method::POST, Method::OPTIONS] {
            let response = call(
                &app,
                request(
                    method,
                    path,
                    &[
                        ("origin", "https://preview.cantos.example".into()),
                        ("host", origin.strip_prefix("https://").unwrap().into()),
                        (
                            "forwarded",
                            format!(
                                "host={};proto=https",
                                origin.strip_prefix("https://").unwrap()
                            ),
                        ),
                        (
                            "x-forwarded-host",
                            origin.strip_prefix("https://").unwrap().into(),
                        ),
                        ("x-forwarded-proto", "https".into()),
                        ("access-control-request-method", "POST".into()),
                    ],
                ),
            )
            .await;
            cors_origin(response.headers(), None);
            error(response, StatusCode::FORBIDDEN, "forbidden").await;
        }
    }
}

#[tokio::test]
async fn all_surfaces_retain_their_own_non_browser_error_and_no_store_contract() {
    let app = api(STUDIO_ORIGIN);
    for path in ["/api/v1/studio/works", "/api/v1/production/catalog"] {
        let response = call(&app, request(Method::GET, path, &[])).await;
        assert!(!response
            .headers()
            .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
        error(response, StatusCode::UNAUTHORIZED, "unauthenticated").await;
    }
    let response = call(
        &app,
        request(Method::GET, "/api/v1/theatre/library/continue", &[]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).unwrap(),
        json!({"code":"unauthenticated", "retryable":false})
    );
    let response = call(
        &app,
        request(Method::GET, "/api/v1/admin/capabilities", &[]),
    )
    .await;
    assert!(!response
        .headers()
        .contains_key(header::ACCESS_CONTROL_ALLOW_ORIGIN));
    error(response, StatusCode::SERVICE_UNAVAILABLE, "unavailable").await;
}

const ADMIN_ORIGIN: &str = "https://admin.cantos.example";

/// The same canonical assembly used by the production executable. One shared application
/// handle; the unreachable DB/absent issuers only support negative controls, never login.
fn combined(admin: Option<&str>) -> Router {
    let state = AppState {
        store: cantos_server::test_support::detached_store().without_development_sessions(),
        web_origin: STUDIO_ORIGIN.into(),
    };
    let origins =
        cantos_server::platform::host::SurfaceOrigins::new(STUDIO_ORIGIN, THEATRE_ORIGIN, admin)
            .unwrap();
    cantos_server::adapters::inbound::http::server_router(state, &origins, None, None)
}

#[tokio::test]
async fn combined_routes_select_only_their_configured_origin_and_credential_purpose() {
    let app = combined(Some(ADMIN_ORIGIN));
    for (path, origin, purpose) in [
        ("/api/v1/studio/auth/bearer/session", STUDIO_ORIGIN, "s"),
        ("/api/v1/theatre/auth/bearer/session", THEATRE_ORIGIN, "t"),
        ("/api/v1/admin/capabilities", ADMIN_ORIGIN, "admin"),
        ("/api/v1/production/catalog", STUDIO_ORIGIN, "s"),
    ] {
        for sent_origin in [
            STUDIO_ORIGIN,
            THEATRE_ORIGIN,
            ADMIN_ORIGIN,
            "https://preview.cantos.example",
            "https://attacker.invalid",
            "null",
            "https://studio.cantos.example.attacker.invalid",
        ] {
            for sent_purpose in ["s", "t", "n"] {
                let admitted = sent_origin == origin;
                let (status, code) = if !admitted {
                    (StatusCode::FORBIDDEN, "forbidden")
                } else if purpose == "admin" || purpose == sent_purpose {
                    // Absent verifier/disabled Admin, not successful authentication.
                    (StatusCode::SERVICE_UNAVAILABLE, "unavailable")
                } else {
                    (StatusCode::UNAUTHORIZED, "unauthenticated")
                };
                let response = call(
                    &app,
                    request(
                        Method::GET,
                        path,
                        &[
                            ("origin", sent_origin.into()),
                            ("authorization", bearer(sent_purpose)),
                        ],
                    ),
                )
                .await;
                cors_origin(response.headers(), admitted.then_some(origin));
                error(response, status, code).await;
            }
        }
    }
}

#[tokio::test]
async fn combined_preflight_runs_before_auth_and_keeps_surface_methods_and_headers() {
    let app = combined(Some(ADMIN_ORIGIN));
    for (path, origin, own, sibling) in [
        (
            "/api/v1/studio/works",
            STUDIO_ORIGIN,
            "x-cantos-studio-session",
            "x-cantos-theatre-session",
        ),
        (
            "/api/v1/production/catalog",
            STUDIO_ORIGIN,
            "x-cantos-studio-session",
            "x-cantos-theatre-session",
        ),
        (
            "/api/v1/theatre/library/saved",
            THEATRE_ORIGIN,
            "x-cantos-theatre-session",
            "x-cantos-studio-session",
        ),
        (
            "/api/v1/admin/capabilities",
            ADMIN_ORIGIN,
            "authorization",
            "x-cantos-studio-session",
        ),
    ] {
        for method in ["GET", "HEAD", "POST", "PUT", "DELETE", "PATCH"] {
            for name in [own, sibling, "authorization, content-type", "range"] {
                let admin = origin == ADMIN_ORIGIN;
                let admitted = method != "PATCH"
                    && name != sibling
                    && (!admin || !matches!(method, "PUT" | "DELETE") && name != "range");
                let response = call(
                    &app,
                    request(
                        Method::OPTIONS,
                        path,
                        &[
                            ("origin", origin.into()),
                            ("access-control-request-method", method.into()),
                            ("access-control-request-headers", name.into()),
                        ],
                    ),
                )
                .await;
                cors_origin(response.headers(), admitted.then_some(origin));
                if admitted {
                    assert_eq!(response.status(), StatusCode::NO_CONTENT);
                    assert_eq!(
                        response.headers()[header::ACCESS_CONTROL_ALLOW_METHODS],
                        method
                    );
                    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
                    assert!(!response.headers()[header::ACCESS_CONTROL_ALLOW_HEADERS]
                        .to_str()
                        .unwrap()
                        .contains(sibling));
                } else {
                    error(response, StatusCode::FORBIDDEN, "forbidden").await;
                }
            }
        }
        for headers in [
            vec![("origin", origin.into()), ("origin", origin.into())],
            vec![
                ("origin", origin.into()),
                ("access-control-request-method", "POST".into()),
            ],
        ] {
            let mut headers = headers;
            headers.push(("access-control-request-method", "GET".into()));
            error(
                call(&app, request(Method::OPTIONS, path, &headers)).await,
                StatusCode::FORBIDDEN,
                "forbidden",
            )
            .await;
        }
    }
}

#[tokio::test]
async fn combined_credentials_and_forwarding_headers_cannot_cross_a_route_boundary() {
    let app = combined(Some(ADMIN_ORIGIN));
    for (path, origin, purpose) in [
        ("/api/v1/studio/auth/bearer/session", STUDIO_ORIGIN, "s"),
        ("/api/v1/theatre/auth/bearer/session", THEATRE_ORIGIN, "t"),
    ] {
        for credentials in [
            vec![
                ("authorization", bearer(purpose)),
                ("authorization", bearer(purpose)),
            ],
            vec![
                ("authorization", bearer(purpose)),
                ("cookie", "cantos_session=fixture".into()),
            ],
            vec![(
                "cookie",
                "cantos_session=fixture; cantos_session=fixture".into(),
            )],
            vec![
                ("cookie", "cantos_session=fixture".into()),
                ("cookie", "cantos_session=fixture".into()),
            ],
        ] {
            let mut headers = vec![("origin", origin.into())];
            headers.extend(credentials);
            error(
                call(&app, request(Method::GET, path, &headers)).await,
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
            )
            .await;
        }
    }
    for path in [
        "/api/v1/studio/session",
        "/api/v1/theatre/session",
        "/api/v1/admin/capabilities",
        "/api/v1/production/catalog",
    ] {
        for method in [Method::GET, Method::POST, Method::OPTIONS] {
            error(
                call(
                    &app,
                    request(
                        method,
                        path,
                        &[
                            ("origin", "https://preview.cantos.example".into()),
                            ("host", "studio.cantos.example".into()),
                            ("forwarded", "host=studio.cantos.example;proto=https".into()),
                            ("x-forwarded-host", "cantos.example".into()),
                            ("x-forwarded-proto", "https".into()),
                            ("access-control-request-method", "POST".into()),
                        ],
                    ),
                )
                .await,
                StatusCode::FORBIDDEN,
                "forbidden",
            )
            .await;
        }
    }
}

#[tokio::test]
async fn combined_public_native_and_protected_routes_keep_their_contracts() {
    let app = combined(None);
    // Public configuration is a real successful handler without authentication or DB I/O.
    for origin in [None, Some(THEATRE_ORIGIN)] {
        let headers = origin
            .map(|o| vec![("origin", o.into())])
            .unwrap_or_default();
        let response = call(
            &app,
            request(Method::GET, "/api/v1/theatre/auth/configuration", &headers),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    for path in ["/api/v1/studio/works", "/api/v1/production/catalog"] {
        error(
            call(&app, request(Method::GET, path, &[])).await,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        )
        .await;
    }
    let response = call(
        &app,
        request(Method::GET, "/api/v1/theatre/library/continue", &[]),
    )
    .await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        serde_json::from_slice::<Value>(&response.into_body().collect().await.unwrap().to_bytes())
            .unwrap(),
        json!({"code":"unauthenticated", "retryable":false})
    );
    // Native cookie-free Bearer has no synthetic Origin requirement.
    error(
        call(
            &app,
            request(
                Method::GET,
                "/api/v1/theatre/native/auth/bearer/session",
                &[("authorization", bearer("n"))],
            ),
        )
        .await,
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
    )
    .await;
    // Admin browser admission is independently opt-in; Studio cannot enable it.
    error(
        call(
            &app,
            request(
                Method::GET,
                "/api/v1/admin/capabilities",
                &[("origin", STUDIO_ORIGIN.into())],
            ),
        )
        .await,
        StatusCode::FORBIDDEN,
        "forbidden",
    )
    .await;
    error(
        call(
            &app,
            request(Method::GET, "/api/v1/admin/capabilities", &[]),
        )
        .await,
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
    )
    .await;
}

#[tokio::test]
async fn combined_slow_request_body_expires_with_each_surfaces_error_and_cache_policy() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let app = combined(None);
    for (path, origin, expected) in [
        (
            "/api/v1/studio/auth/bearer/flows",
            STUDIO_ORIGIN,
            json!({"code":"unavailable","current_revision":null,"issues":[]}),
        ),
        (
            "/api/v1/theatre/auth/bearer/flows",
            THEATRE_ORIGIN,
            json!({"code":"unavailable","retryable":true}),
        ),
    ] {
        let polled = Arc::new(AtomicBool::new(false));
        let observed = Arc::clone(&polled);
        // A slow caller's body reaches the real production JSON extractor; no handler,
        // verifier or deadline is replaced. The production boundary must stop the read.
        let body = Body::from_stream(futures_util::stream::poll_fn(move |_| {
            observed.store(true, Ordering::SeqCst);
            std::task::Poll::Pending::<Option<Result<axum::body::Bytes, std::io::Error>>>
        }));
        let started = std::time::Instant::now();
        let response = tokio::time::timeout(
            std::time::Duration::from_secs(20),
            app.clone().oneshot(
                Request::builder()
                    .method(Method::POST)
                    .uri(path)
                    .header(header::ORIGIN, origin)
                    .header(header::CONTENT_TYPE, "application/json")
                    .body(body)
                    .unwrap(),
            ),
        )
        .await
        .unwrap()
        .unwrap();
        assert!(
            polled.load(Ordering::SeqCst),
            "the actual handler must poll the slow body"
        );
        assert!(started.elapsed() >= std::time::Duration::from_secs(14));
        assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        cors_origin(response.headers(), Some(origin));
        assert_eq!(
            serde_json::from_slice::<Value>(
                &response.into_body().collect().await.unwrap().to_bytes()
            )
            .unwrap(),
            expected
        );
    }
}
