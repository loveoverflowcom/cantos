//! Surface-policy regressions for #333.1, using the current origin-pinned HTTP host.
//!
//! Credentials are Identity v1 sessions (#335): a Studio Web cookie, a Theatre Web cookie and a
//! native Bearer (the three [`Purpose`]s), each issued through the production login route against a
//! scripted Identity that is then removed. Their rows are live, so a credential of the route's own
//! purpose reaches the verifier and fails closed as `unavailable` (Identity cannot be asked);
//! a wrong purpose, a mixed or duplicated credential or a foreign origin must be refused earlier.
//! No assertion here is a login proof; `identity_password_turso.rs` owns the real executable.
#[path = "support/fake_identity.rs"]
mod fake;
#[path = "support/identity_world.rs"]
mod world;

use axum::{
    body::Body,
    http::{header, HeaderMap, Method, Request, StatusCode},
    response::Response,
    Router,
};
use cantos_server::adapters::inbound::http::WebSurface;
use cantos_server::adapters::inbound::http::{api_router, with_browser_cors, AppState};
use cantos_server::platform::identity_auth::{
    config::IdentityAuthConfig, decision::Subject, IdentityAuth, PeerPolicy,
};
use http_body_util::BodyExt;
use serde::Deserialize;
use serde_json::{json, Value};
use std::path::PathBuf;
use tower::ServiceExt;

const STUDIO_ORIGIN: &str = "https://studio.cantos.example";
const THEATRE_ORIGIN: &str = "https://cantos.example";
const STUDIO_COOKIE: &str = "__Secure-cantos_id_studio";
const THEATRE_COOKIE: &str = "__Secure-cantos_id_theatre";
const PASSWORD: &str = "a correct horse battery";

/// What a session credential is for. The oracle names the contract's three purposes itself
/// rather than borrowing the production `Channel`, so a production rename cannot silently
/// rewrite what is being asserted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    Studio,
    Theatre,
    Native,
}

const PURPOSES: [Purpose; 3] = [Purpose::Studio, Purpose::Theatre, Purpose::Native];

/// What a request path accepts: a session of one purpose, the Admin surface (no session
/// credential satisfies it; only its availability is observable here) or a path no pinned
/// browser host serves.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Route {
    Session(Purpose),
    Admin,
    Unserved,
}

/// The one field of the native login reply these tests read. A renamed field fails the parse
/// by name instead of indexing an untyped value.
#[derive(Deserialize)]
struct NativeLogin {
    access_token: String,
}

fn api(origin: &str) -> Router {
    api_router(AppState {
        store: cantos_server::test_support::detached_store().without_development_sessions(),
        web_origin: origin.into(),
    })
}

fn browser(surface: WebSurface, origin: &str) -> Router {
    with_browser_cors(api(origin), origin, surface).unwrap()
}

/// One private server.db holding a live session per purpose, behind an Identity that no longer
/// answers. Routers built from it share that file; the directory outlives them.
struct Sessions {
    db: world::Database,
    dir: PathBuf,
    socket: PathBuf,
    uid: u32,
    studio: String,
    theatre: String,
    native: String,
}

impl Sessions {
    async fn issue() -> Self {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        // A short private directory: Unix socket paths are limited to about 100 bytes.
        let dir = PathBuf::from(format!(
            "/tmp/sp-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let uid = std::fs::metadata(&dir).unwrap().uid();
        let socket = dir.join("identity.sock");
        let identity = fake::Serving::start(&socket, fake::Script::healthy());
        let db = world::Database::new().await;
        let roster = [Subject::from_bytes(fake::Script::healthy().subject).unwrap()];
        let login = json!({"email": "owner@example.com", "password": PASSWORD});
        let studio_app = world::App::new(&db, STUDIO_ORIGIN, socket.clone(), uid, &roster);
        let studio = studio_app
            .post(
                "/api/v1/studio/identity/login",
                &[("origin", STUDIO_ORIGIN)],
                login.clone(),
            )
            .await;
        assert_eq!(studio.status, StatusCode::OK, "{:?}", studio.json);
        let theatre_app = world::App::new(&db, THEATRE_ORIGIN, socket.clone(), uid, &roster);
        let theatre = theatre_app
            .post(
                "/api/v1/theatre/identity/login",
                &[("origin", THEATRE_ORIGIN)],
                login.clone(),
            )
            .await;
        assert_eq!(theatre.status, StatusCode::OK, "{:?}", theatre.json);
        let native = theatre_app
            .post("/api/v1/theatre/native/identity/login", &[], login)
            .await;
        assert_eq!(native.status, StatusCode::OK, "{:?}", native.json);
        // From here on Identity cannot be asked: admission of a live session fails closed.
        drop(identity);
        std::fs::remove_file(&socket).unwrap();
        let cookie = |reply: &world::Reply, name: &str| {
            let (cookie, secret) = reply.cookie();
            assert_eq!(cookie, name);
            secret
        };
        Self {
            studio: cookie(&studio, STUDIO_COOKIE),
            theatre: cookie(&theatre, THEATRE_COOKIE),
            native: serde_json::from_value::<NativeLogin>(native.json)
                .unwrap()
                .access_token,
            db,
            dir,
            socket,
            uid,
        }
    }

    fn state(&self, web_origin: &str) -> AppState {
        let identity = IdentityAuth::new(
            IdentityAuthConfig {
                socket: self.socket.clone(),
                peer_uid: self.uid,
                peer_policy: PeerPolicy::DirectPeer,
                studio_subjects: vec![Subject::from_bytes(fake::Script::healthy().subject).unwrap()],
            },
            true,
        );
        AppState {
            store: self.db.store().with_identity_auth(identity),
            web_origin: web_origin.into(),
        }
    }

    fn browser(&self, surface: WebSurface, origin: &str) -> Router {
        with_browser_cors(api_router(self.state(origin)), origin, surface).unwrap()
    }

    /// The canonical assembly used by the production executable, over the same sessions.
    fn combined(&self, admin: Option<&str>) -> Router {
        let origins = cantos_server::platform::host::SurfaceOrigins::new(
            STUDIO_ORIGIN,
            THEATRE_ORIGIN,
            admin,
        )
        .unwrap();
        cantos_server::adapters::inbound::http::server_router(
            self.state(STUDIO_ORIGIN),
            &origins,
            None,
            None,
        )
    }

    fn secret(&self, purpose: Purpose) -> &str {
        match purpose {
            Purpose::Studio => &self.studio,
            Purpose::Theatre => &self.theatre,
            Purpose::Native => &self.native,
        }
    }

    /// The single header a client of `purpose` sends.
    fn credential(&self, purpose: Purpose) -> (&'static str, String) {
        let secret = self.secret(purpose);
        match purpose {
            Purpose::Studio => ("cookie", format!("{STUDIO_COOKIE}={secret}")),
            Purpose::Theatre => ("cookie", format!("{THEATRE_COOKIE}={secret}")),
            Purpose::Native => ("authorization", format!("Bearer {secret}")),
        }
    }
}

impl Drop for Sessions {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
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
    let sessions = Sessions::issue().await;
    for (surface, origin, purpose) in [
        (WebSurface::Studio, STUDIO_ORIGIN, Purpose::Studio),
        (WebSurface::Theatre, THEATRE_ORIGIN, Purpose::Theatre),
    ] {
        let app = sessions.browser(surface, origin);
        for (path, route) in [
            (
                "/api/v1/studio/identity/session",
                Route::Session(Purpose::Studio),
            ),
            (
                "/api/v1/theatre/identity/session",
                Route::Session(Purpose::Theatre),
            ),
            ("/api/v1/admin/capabilities", Route::Admin),
            ("/api/v1/production/catalog", Route::Unserved),
        ] {
            for sent_origin in [
                STUDIO_ORIGIN,
                THEATRE_ORIGIN,
                "https://preview.cantos.example",
                "https://attacker.invalid",
                "https://studio.cantos.example.attacker.invalid",
                "null",
            ] {
                for sent_purpose in PURPOSES {
                    let admitted = sent_origin == origin && route == Route::Session(purpose);
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
                                sessions.credential(sent_purpose),
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

/// Every way a request can carry an own-purpose secret that is not exactly one credential of
/// that purpose, each with a label for failure messages: the headers hold live secrets and
/// must never be printed. `own`/`other` are the route surface's cookie and the sibling
/// surface's cookie.
fn mixed_credentials(
    sessions: &Sessions,
    own: (&str, &str),
    other: (&str, &str),
) -> Vec<(&'static str, Vec<(&'static str, String)>)> {
    let (own_name, own_secret) = own;
    let (other_name, other_secret) = other;
    let plain = own_name.trim_start_matches("__Secure-");
    let own = format!("{own_name}={own_secret}");
    vec![
        ("no credential", vec![]),
        (
            "sibling cookie only",
            vec![("cookie", format!("{other_name}={other_secret}"))],
        ),
        // A sibling's or the native secret under this surface's cookie name.
        (
            "sibling secret under own cookie name",
            vec![("cookie", format!("{own_name}={other_secret}"))],
        ),
        (
            "native secret under own cookie name",
            vec![("cookie", format!("{own_name}={}", sessions.native))],
        ),
        (
            "own cookie twice in one header",
            vec![("cookie", format!("{own}; {own}"))],
        ),
        (
            "own cookie in two headers",
            vec![("cookie", own.clone()), ("cookie", own.clone())],
        ),
        // The insecure spelling beside the secure one, as a downgraded jar would send it.
        (
            "insecure spelling beside the secure cookie",
            vec![("cookie", format!("{own}; {plain}={own_secret}"))],
        ),
        // A Web cookie never travels with a Bearer, of any purpose.
        (
            "cookie beside the native bearer",
            vec![
                ("cookie", own.clone()),
                sessions.credential(Purpose::Native),
            ],
        ),
        (
            "cookie beside its own secret as a bearer",
            vec![
                ("cookie", own.clone()),
                ("authorization", format!("Bearer {own_secret}")),
            ],
        ),
        (
            "own secret as a bearer",
            vec![("authorization", format!("Bearer {own_secret}"))],
        ),
    ]
}

#[tokio::test]
async fn duplicate_and_mixed_credentials_cannot_reach_an_own_purpose_verifier() {
    let sessions = Sessions::issue().await;
    for (surface, origin, path, own, other) in [
        (
            WebSurface::Studio,
            STUDIO_ORIGIN,
            "/api/v1/studio/identity/session",
            (STUDIO_COOKIE, sessions.studio.as_str()),
            (THEATRE_COOKIE, sessions.theatre.as_str()),
        ),
        (
            WebSurface::Theatre,
            THEATRE_ORIGIN,
            "/api/v1/theatre/identity/session",
            (THEATRE_COOKIE, sessions.theatre.as_str()),
            (STUDIO_COOKIE, sessions.studio.as_str()),
        ),
    ] {
        let app = sessions.browser(surface, origin);
        for (label, credentials) in mixed_credentials(&sessions, own, other) {
            let mut headers = vec![("origin", origin.to_owned())];
            headers.extend(credentials);
            let response = call(&app, request(Method::GET, path, &headers)).await;
            cors_origin(response.headers(), Some(origin));
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{path} {label}"
            );
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
    let sessions = Sessions::issue().await;
    let app = sessions.combined(Some(ADMIN_ORIGIN));
    for (path, origin, route) in [
        (
            "/api/v1/studio/identity/session",
            STUDIO_ORIGIN,
            Route::Session(Purpose::Studio),
        ),
        (
            "/api/v1/theatre/identity/session",
            THEATRE_ORIGIN,
            Route::Session(Purpose::Theatre),
        ),
        ("/api/v1/admin/capabilities", ADMIN_ORIGIN, Route::Admin),
        (
            "/api/v1/production/catalog",
            STUDIO_ORIGIN,
            Route::Session(Purpose::Studio),
        ),
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
            for sent_purpose in PURPOSES {
                let admitted = sent_origin == origin;
                let (status, code) = if !admitted {
                    (StatusCode::FORBIDDEN, "forbidden")
                } else if route == Route::Admin || route == Route::Session(sent_purpose) {
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
                            sessions.credential(sent_purpose),
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
    let sessions = Sessions::issue().await;
    let app = sessions.combined(Some(ADMIN_ORIGIN));
    for (path, origin, own, other) in [
        (
            "/api/v1/studio/identity/session",
            STUDIO_ORIGIN,
            (STUDIO_COOKIE, sessions.studio.as_str()),
            (THEATRE_COOKIE, sessions.theatre.as_str()),
        ),
        (
            "/api/v1/theatre/identity/session",
            THEATRE_ORIGIN,
            (THEATRE_COOKIE, sessions.theatre.as_str()),
            (STUDIO_COOKIE, sessions.studio.as_str()),
        ),
    ] {
        for (label, credentials) in mixed_credentials(&sessions, own, other) {
            let mut headers = vec![("origin", origin.into())];
            headers.extend(credentials);
            let response = call(&app, request(Method::GET, path, &headers)).await;
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{path} {label}"
            );
            error(response, StatusCode::UNAUTHORIZED, "unauthenticated").await;
        }
    }
    for path in [
        "/api/v1/studio/session",
        "/api/v1/studio/identity/session",
        "/api/v1/theatre/session",
        "/api/v1/theatre/identity/session",
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
    let sessions = Sessions::issue().await;
    let app = sessions.combined(None);
    // Public discovery is a real successful handler without authentication, read from the
    // private file's (empty) catalog, with or without a browser Origin.
    for origin in [None, Some(THEATRE_ORIGIN)] {
        let headers = origin
            .map(|o| vec![("origin", o.into())])
            .unwrap_or_default();
        let response = call(
            &app,
            request(Method::GET, "/api/v1/theatre/works", &headers),
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
    // Native cookie-free Bearer has no synthetic Origin requirement and reaches its verifier;
    // a browser marker beside it (Origin, a cookie) is refused before the verifier.
    let native = "/api/v1/theatre/native/identity/session";
    error(
        call(
            &app,
            request(Method::GET, native, &[sessions.credential(Purpose::Native)]),
        )
        .await,
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
    )
    .await;
    for marker in [
        ("origin", THEATRE_ORIGIN.to_owned()),
        sessions.credential(Purpose::Theatre),
        ("sec-fetch-site", "same-site".to_owned()),
    ] {
        error(
            call(
                &app,
                request(
                    Method::GET,
                    native,
                    &[sessions.credential(Purpose::Native), marker],
                ),
            )
            .await,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        )
        .await;
    }
    // A Web secret is never a native Bearer.
    for purpose in [Purpose::Studio, Purpose::Theatre] {
        let secret = sessions.secret(purpose);
        error(
            call(
                &app,
                request(
                    Method::GET,
                    native,
                    &[("authorization", format!("Bearer {secret}"))],
                ),
            )
            .await,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        )
        .await;
    }
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
    let sessions = Sessions::issue().await;
    let app = sessions.combined(None);
    for (path, origin, expected) in [
        (
            "/api/v1/studio/identity/login",
            STUDIO_ORIGIN,
            json!({"code":"unavailable","current_revision":null,"issues":[]}),
        ),
        (
            "/api/v1/theatre/identity/login",
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
