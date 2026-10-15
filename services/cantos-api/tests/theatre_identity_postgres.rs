//! Actual disposable PostgreSQL + HTTP handlers + signed mock provider, through xtask postgres.
#[path = "support/oidc_provider.rs"]
mod mock;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_api::{
    interfaces::http::{router, AppState},
    platform::{
        postgres::{local_config, migrate, Store},
        theatre_identity::{AuthError, TheatreIdentity},
    },
};
use std::{
    env,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}
struct Harness {
    admin: Client,
    store: Store,
    mock: mock::MockProvider,
}
impl Harness {
    async fn new() -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let db = format!("cantos_test_{}", uuid::Uuid::new_v4().simple());
        connect(&config)
            .await
            .batch_execute(&format!("CREATE DATABASE {db}"))
            .await
            .unwrap();
        config.dbname(&db);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin.batch_execute("GRANT SELECT ON actors,theatre_identity_sessions,listener_state,listener_saved,listener_history,listener_progress,listener_operations,listener_playback_sessions,listener_device_sequences TO cantos_app; GRANT UPDATE(revoked,revocation_pending,revoked_by_token_hash) ON theatre_identity_sessions TO cantos_app; GRANT SELECT,INSERT,UPDATE ON theatre_identity_flows TO cantos_app; GRANT EXECUTE ON FUNCTION theatre_identity_finish(bytea,text,text,text,text,bytea,bytea,text,bigint),listener_lock_actor(text),listener_actor_active(text) TO cantos_app;").await.unwrap();
        config.user("cantos_app");
        let mock = mock::MockProvider::start_for(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            "cantos-theatre-web-bff",
        )
        .await;
        let identity =
            TheatreIdentity::new(&mock.issuer(), "http://127.0.0.1:8080", "fixture".into())
                .unwrap();
        let store = Store::new(config).unwrap().with_theatre_identity(identity);
        Self { admin, store, mock }
    }
    fn oidc(&self) -> &TheatreIdentity {
        self.store.theatre_identity().unwrap()
    }
    async fn start(&self, previous: Option<&str>) -> (String, String, String) {
        let start = self.oidc().begin(&self.store, previous).await.unwrap();
        let browser = start
            .cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_string();
        let (state, code) = self.mock.issue(&start.url, "minh", None);
        (state, browser, code)
    }
    async fn login(&self, previous: Option<&str>) -> (String, String) {
        let (s, b, c) = self.start(previous).await;
        let complete = self.oidc().callback(&self.store, &s, &b, &c).await.unwrap();
        let token = complete
            .cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_string();
        (token, complete.actor_id)
    }
    #[allow(dead_code)]
    async fn count(&self, table: &str) -> i64 {
        self.admin
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .unwrap()
            .get(0)
    }
}

use cantos_api::contexts::listening::auth::{
    AuthenticationError, ListenerAuthenticator, ListenerCredentials,
};
use http_body_util::BodyExt;
async fn request(
    h: &Harness,
    method: &str,
    path: &str,
    cookie: Option<&str>,
    origin: Option<&str>,
    bearer: Option<&str>,
) -> axum::response::Response {
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    );
    let mut request = Request::builder().method(method).uri(path);
    if let Some(cookie) = cookie {
        request = request.header(header::COOKIE, cookie);
        if method == "DELETE" {
            if let Some(token) = cookie
                .split(';')
                .find_map(|part| part.trim().strip_prefix("cantos_theatre="))
            {
                if let Some(row) = h.admin.query_opt("SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1", &[&cantos_api::platform::postgres::token_hash(token)]).await.unwrap() {
                    request = request.header("x-cantos-theatre-session", row.get::<_,String>(0));
                }
            }
        }
    }
    if let Some(origin) = origin {
        request = request.header(header::ORIGIN, origin);
    }
    if let Some(bearer) = bearer {
        request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
    }
    app.oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap()
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; actual crypto/HTTP mock, not Kanidm-live"]
async fn verified_theatre_registry_binding_mints_actual_sealed_listener_and_bff_session() {
    let h = Harness::new().await;
    let (token, actor) = h.login(None).await;
    let adapter = TheatreListenerAuthenticator::new(h.store.clone());
    let principal = adapter
        .authenticate(ListenerCredentials::bearer(&token).unwrap())
        .await
        .unwrap();
    assert_eq!(principal.listener().as_str(), actor);
    let response = request(
        &h,
        "GET",
        "/api/v1/theatre/session",
        Some(&format!("cantos_theatre={token}")),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let body: cantos_contracts::theatre_identity::Session =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(body.actor_id, actor);
    assert!(uuid::Uuid::parse_str(&body.session_id).is_ok());
    assert_ne!(body.session_id, token);
    // Actual Library handler receives the sealed principal. No Library domain/store is recreated.
    let response = request(
        &h,
        "GET",
        "/api/v1/theatre/progress/00000000-0000-4000-8000-000000000001",
        Some(&format!("cantos_theatre={token}")),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(
        value,
        serde_json::json!({"resource":"progress","current":null})
    );
    assert_eq!(h.count("sessions").await, 0); // Theatre never issues a Studio session.
    let (_, same) = h.login(None).await;
    assert_eq!(same, actor);
    assert_eq!(h.count("actor_external_identities").await, 1);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn issued_theatre_web_cookie_never_authorizes_private_studio_rendition_reads() {
    let h = Harness::new().await;
    // Exercise Studio's real empty-session lookup, rather than accepting a missing SQL grant
    // as proof that a Theatre token cannot become a Studio credential by cookie relabeling.
    h.admin
        .batch_execute("GRANT SELECT ON sessions TO cantos_app")
        .await
        .unwrap();
    let (token, actor) = h.login(None).await;
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), 1);
    let theatre_cookie = format!("cantos_theatre={token}");
    let response = request(
        &h,
        "GET",
        "/api/v1/theatre/session",
        Some(&theatre_cookie),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::OK);
    let session: cantos_contracts::theatre_identity::Session =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(session.actor_id, actor);

    // No media/storage fixture is needed: the exact 401 must precede object lookup, rather
    // than treating a missing rendition or disabled storage adapter as an authorization proof.
    const AUTHORITY_AND_MEDIA: &str = "SELECT jsonb_build_object('actors',(SELECT count(*) FROM actors),'bindings',(SELECT count(*) FROM actor_external_identities),'theatre_sessions',(SELECT count(*) FROM theatre_identity_sessions),'theatre_revoked',(SELECT count(*) FROM theatre_identity_sessions WHERE revoked),'studio_sessions',(SELECT count(*) FROM sessions),'script_members',(SELECT count(*) FROM script_members),'producer_grants',(SELECT count(*) FROM production_producer_grants),'intents',(SELECT count(*) FROM audio_upload_intents),'receipts',(SELECT count(*) FROM audio_intake_receipts),'submissions',(SELECT count(*) FROM audio_submissions),'renditions',(SELECT count(*) FROM audio_renditions))::text";
    let before: String = h
        .admin
        .query_one(AUTHORITY_AND_MEDIA, &[])
        .await
        .unwrap()
        .get(0);
    let initial: serde_json::Value = serde_json::from_str(&before).unwrap();
    let introspections_before = h.mock.data.introspection_calls.load(Ordering::SeqCst);
    assert_eq!(initial["theatre_sessions"], 1);
    assert_eq!(initial["theatre_revoked"], 0);
    for field in [
        "studio_sessions",
        "script_members",
        "producer_grants",
        "intents",
        "receipts",
        "submissions",
        "renditions",
    ] {
        assert_eq!(initial[field], 0, "fixture unexpectedly has {field}");
    }
    let studio_cookie = format!("cantos_session={token}");
    let secure_studio_cookie = format!("__Secure-cantos_studio={token}");
    let id = "00000000-0000-4000-8000-000000000001";
    for suffix in ["bytes", "read-access"] {
        let path = format!(
            "/api/v1/studio/content-units/{id}/audio-submissions/{id}/renditions/{id}/{suffix}"
        );
        for (case, cookie, bearer) in [
            ("Theatre cookie", Some(theatre_cookie.as_str()), None),
            ("Studio cookie relabel", Some(studio_cookie.as_str()), None),
            (
                "secure Studio cookie relabel",
                Some(secure_studio_cookie.as_str()),
                None,
            ),
            ("Theatre token as bearer", None, Some(token.as_str())),
            (
                "Theatre cookie and bearer",
                Some(theatre_cookie.as_str()),
                Some(token.as_str()),
            ),
        ] {
            let response = request(&h, "GET", &path, cookie, None, bearer).await;
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{case} {suffix}"
            );
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert!(!response.headers().contains_key(header::LOCATION));
            assert!(!response.headers().contains_key("x-cantos-read-expires-at"));
            let body: serde_json::Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(
                body,
                serde_json::json!({"code":"unauthenticated","current_revision":null,"issues":[]}),
                "{case} {suffix}"
            );
        }
    }
    let after: String = h
        .admin
        .query_one(AUTHORITY_AND_MEDIA, &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        after, before,
        "private reads changed authority or media state"
    );
    assert_eq!(
        h.mock.data.introspection_calls.load(Ordering::SeqCst),
        introspections_before,
        "Studio private reads must not admit through Theatre provider authentication"
    );
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&theatre_cookie),
            None,
            None,
        )
        .await
        .status(),
        StatusCode::OK,
        "the actual Theatre cookie remains valid for its own surface"
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn cookies_bearers_csrf_and_surface_purposes_are_separate_and_default_deny() {
    let h = Harness::new().await;
    let (token, _) = h.login(None).await;
    for cookie in [
        format!("cantos_session={token}"),
        format!("__Secure-cantos_theatre={token}"),
        format!("cantos_theatre={token}; cantos_theatre={token}"),
    ] {
        assert_eq!(
            request(
                &h,
                "GET",
                "/api/v1/theatre/session",
                Some(&cookie),
                None,
                None
            )
            .await
            .status(),
            StatusCode::UNAUTHORIZED
        );
    }
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            None,
            None,
            Some(&token)
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/studio/session",
            Some(&format!("cantos_theatre={token}")),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/admin/session",
            Some(&format!("cantos_theatre={token}")),
            None,
            None
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        request(
            &h,
            "DELETE",
            "/api/v1/theatre/session",
            Some(&format!("cantos_theatre={token}")),
            Some("https://evil.example"),
            None
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
    let adapter = TheatreListenerAuthenticator::new(h.store.clone());
    for raw in ["minh", "fake.oidc.token", "cantos_session=opaque"] {
        assert_eq!(
            adapter
                .authenticate(ListenerCredentials::bearer(raw).unwrap())
                .await
                .err(),
            Some(AuthenticationError::Unauthenticated)
        );
    }
    h.admin.batch_execute("SET ROLE cantos_app").await.unwrap();
    assert_eq!(
        h.admin
            .query("SELECT * FROM actor_external_identities", &[])
            .await
            .unwrap_err()
            .code()
            .unwrap()
            .code(),
        "42501"
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn callback_race_is_single_exchange_and_local_logout_survives_remote_outage() {
    let h = Harness::new().await;
    let (state, browser, code) = h.start(None).await;
    let (a, b) = tokio::join!(
        h.oidc().callback(&h.store, &state, &browser, &code),
        h.oidc().callback(&h.store, &state, &browser, &code)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), 1);
    assert_eq!(a.err().or(b.err()), Some(AuthError::State));
    let (token, actor) = h.login(None).await;
    let adapter = TheatreListenerAuthenticator::new(h.store.clone());
    h.mock.data.active.store(false, Ordering::SeqCst);
    assert_eq!(
        adapter
            .authenticate(ListenerCredentials::bearer(&token).unwrap())
            .await
            .err(),
        Some(AuthenticationError::Unauthenticated)
    );
    h.mock.data.active.store(true, Ordering::SeqCst);
    h.admin
        .execute("UPDATE actors SET active=false WHERE id=$1", &[&actor])
        .await
        .unwrap();
    assert_eq!(
        adapter
            .authenticate(ListenerCredentials::bearer(&token).unwrap())
            .await
            .err(),
        Some(AuthenticationError::Unauthenticated)
    );
    h.admin
        .execute("UPDATE actors SET active=true WHERE id=$1", &[&actor])
        .await
        .unwrap();
    h.mock.data.revoke_fail.store(true, Ordering::SeqCst);
    let result = h.oidc().logout(&h.store, Some(&token), None).await.unwrap();
    assert!(!result.provider_revoked);
    assert_eq!(
        adapter
            .authenticate(ListenerCredentials::bearer(&token).unwrap())
            .await
            .err(),
        Some(AuthenticationError::Unauthenticated)
    );
    let row=h.admin.query_one("SELECT revoked,revocation_pending FROM theatre_identity_sessions WHERE token_hash=sha256(convert_to($1,'UTF8'))",&[&token]).await.unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(row.get::<_, bool>(1));
    h.mock.data.revoke_fail.store(false, Ordering::SeqCst);
    assert!(
        h.oidc()
            .logout(&h.store, Some(&token), None)
            .await
            .unwrap()
            .provider_revoked
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn actual_http_login_callback_cookie_logout_and_expired_code_recovery() {
    let h = Harness::new().await;
    let response = request(&h, "GET", "/api/v1/theatre/auth/login", None, None, None).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let url = response.headers()[header::LOCATION].to_str().unwrap();
    let flow = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let (state, code) = h.mock.issue(url, "đặng-minh", None);
    let callback = format!("/api/v1/theatre/auth/callback?state={state}&code={code}");
    let response = request(&h, "GET", &callback, Some(&flow), None, None).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/#auth-return");
    let cookie = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|h| h.to_str().unwrap())
        .find(|v| v.starts_with("cantos_theatre="))
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    // A completion hash alone grants nothing; the app must confirm the actual cookie session.
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&h, "GET", &callback, Some(&flow), None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let response = request(
        &h,
        "DELETE",
        "/api/v1/theatre/session",
        Some(&cookie),
        Some("http://127.0.0.1:8080"),
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        response.headers()["cantos-provider-revocation"],
        "completed"
    );
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let (s, b, c) = h.start(None).await;
    h.mock.expire_code(&c);
    let count = h.mock.data.token_calls.load(Ordering::SeqCst);
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::ProviderUnavailable)
    );
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), count + 1);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; signed-provider HTTP boundary"]
async fn bound_provider_cancellation_returns_to_login_without_exchanging_or_issuing_a_session() {
    let h = Harness::new().await;
    let response = request(&h, "GET", "/api/v1/theatre/auth/login", None, None, None).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let url = response.headers()[header::LOCATION].to_str().unwrap();
    let (state, code) = h.mock.issue(url, "cancelled-listener", None);
    let flow = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let token_calls = h.mock.data.token_calls.load(Ordering::SeqCst);
    let callback = format!(
        "/api/v1/theatre/auth/callback?state={state}&error=access_denied&error_description=ignored&return_to=https://example.com"
    );
    let response = request(&h, "GET", &callback, Some(&flow), None, None).await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(response.headers()[header::LOCATION], "/#auth-cancelled");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert_eq!(h.count("theatre_identity_sessions").await, 0);
    assert_eq!(h.count("actors").await, 0);
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), token_calls);
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&flow),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    for (path, cookie) in [
        (callback.clone(), None),
        (
            format!(
                "/api/v1/theatre/auth/callback?state={}&error=access_denied",
                "f".repeat(64)
            ),
            Some(flow.as_str()),
        ),
        (
            format!("/api/v1/theatre/auth/callback?state={state}&code=code&error=access_denied"),
            Some(flow.as_str()),
        ),
        (
            format!("/api/v1/theatre/auth/callback?state={state}&error="),
            Some(flow.as_str()),
        ),
    ] {
        let response = request(&h, "GET", &path, cookie, None, None).await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::LOCATION));
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    assert_eq!(h.count("theatre_identity_sessions").await, 0);
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), token_calls);
    let forged_binder = format!("cantos_theatre_flow_{state}={}", "f".repeat(64));
    let forged = request(&h, "GET", &callback, Some(&forged_binder), None, None).await;
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);
    assert!(!forged.headers().contains_key(header::LOCATION));
    assert!(!forged.headers().contains_key(header::SET_COOKIE));
    let late_code = format!("/api/v1/theatre/auth/callback?state={state}&code={code}");
    let late = request(&h, "GET", &late_code, Some(&flow), None, None).await;
    assert_eq!(late.status(), StatusCode::UNAUTHORIZED);
    assert!(!late.headers().contains_key(header::SET_COOKIE));
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), token_calls);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; per-flow UI nonce and exact cancellation"]
async fn old_callback_keeps_its_ui_intent_and_cancellation_never_revokes_another_flow() {
    let h = Harness::new().await;
    let before = h.count("theatre_identity_flows").await;
    for intent in [
        "bad",
        "",
        &"a".repeat(33),
        &"A".repeat(32),
        "first&intent=second",
    ] {
        let response = request(
            &h,
            "GET",
            &format!("/api/v1/theatre/auth/login?intent={intent}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    assert_eq!(h.count("theatre_identity_flows").await, before);
    let mut starts = Vec::new();
    for (intent, subject) in [("1".repeat(32), "old-a"), ("2".repeat(32), "current-b")] {
        let response = request(
            &h,
            "GET",
            &format!("/api/v1/theatre/auth/login?intent={intent}"),
            None,
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let (state, code) = h.mock.issue(
            response.headers()[header::LOCATION].to_str().unwrap(),
            subject,
            None,
        );
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| {
                value
                    .to_str()
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap()
                    .to_owned()
            })
            .collect::<Vec<_>>();
        assert_eq!(cookies.len(), 2);
        assert!(cookies
            .iter()
            .any(|value| value == &format!("cantos_theatre_return_{state}={intent}")));
        starts.push((state, code, intent, cookies.join("; ")));
    }
    let inventory = starts
        .iter()
        .map(|(_, _, _, cookie)| cookie.as_str())
        .collect::<Vec<_>>()
        .join("; ");
    let mut issued = Vec::new();
    for (state, code, intent, _) in &starts {
        let response = request(
            &h,
            "GET",
            &format!("/api/v1/theatre/auth/callback?state={state}&code={code}"),
            Some(&inventory),
            None,
            None,
        )
        .await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        assert_eq!(
            response.headers()[header::LOCATION],
            format!("/#auth-return?intent={intent}")
        );
        let cookies = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .map(|value| value.to_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(cookies.len(), 2);
        let cleared = cookies
            .iter()
            .find(|value| value.contains("Max-Age=0"))
            .unwrap();
        assert!(cleared.starts_with(&format!("cantos_theatre_return_{state}=")));
        let cookie = cookies
            .iter()
            .find(|value| value.starts_with("cantos_theatre="))
            .unwrap()
            .split(';')
            .next()
            .unwrap()
            .to_owned();
        issued.push(cookie);
    }
    let (old_state, _, old_intent, _) = &starts[0];
    let response = request(
        &h,
        "GET",
        &format!("/api/v1/theatre/auth/callback?state={old_state}&error=access_denied"),
        Some(&format!("{inventory}; {}", issued[1])),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    assert_eq!(
        response.headers()[header::LOCATION],
        format!("/#auth-cancelled?intent={old_intent}")
    );
    assert!(response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .all(|value| value
            .to_str()
            .unwrap()
            .starts_with(&format!("cantos_theatre_return_{old_state}="))));
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&issued[0]),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&issued[1]),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; cancellation fences a paused code exchange"]
async fn provider_cancellation_during_exchange_prevents_late_session_issuance() {
    let h = Harness::new().await;
    let start = h.oidc().begin(&h.store, None).await.unwrap();
    let (state, code) = h.mock.issue(&start.url, "late-cancelled-listener", None);
    let cookie = start.cookie.split(';').next().unwrap().to_owned();
    let pause = h.mock.pause_code(&code);
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    );
    let path = format!("/api/v1/theatre/auth/callback?state={state}&code={code}");
    let captured = cookie.clone();
    let late = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .uri(path)
                .header(header::COOKIE, captured)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), pause.entered.notified())
        .await
        .unwrap();
    let cancelled = request(
        &h,
        "GET",
        &format!("/api/v1/theatre/auth/callback?state={state}&error=access_denied"),
        Some(&cookie),
        None,
        None,
    )
    .await;
    assert_eq!(cancelled.status(), StatusCode::SEE_OTHER);
    assert_eq!(cancelled.headers()[header::LOCATION], "/#auth-cancelled");
    pause.release.notify_one();
    let late = tokio::time::timeout(std::time::Duration::from_secs(2), late)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(late.status(), StatusCode::UNAUTHORIZED);
    assert!(!late.headers().contains_key(header::SET_COOKIE));
    assert_eq!(h.count("theatre_identity_sessions").await, 0);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; deterministic actor-lock interleaving"]
async fn theatre_disable_at_registry_lock_and_two_rotations_refuse_stale_authority() {
    let mut h = Harness::new().await;
    let (old, actor) = h.login(None).await;
    let (s1, b1, c1) = h.start(Some(&old)).await;
    let (s2, b2, c2) = h.start(Some(&old)).await;
    let (one, two) = tokio::join!(
        h.oidc().callback(&h.store, &s1, &b1, &c1),
        h.oidc().callback(&h.store, &s2, &b2, &c2)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(one.err().or(two.err()), Some(AuthError::Inactive));
    assert_eq!(h.count("theatre_identity_sessions").await, 2);
    let (s, b, c) = h.start(None).await;
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    config.dbname(&db);
    let observer = connect(&config).await;
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one("SELECT id FROM actors WHERE id=$1 FOR UPDATE", &[&actor])
        .await
        .unwrap();
    let store = h.store.clone();
    let callback = tokio::spawn(async move {
        store
            .theatre_identity()
            .unwrap()
            .callback(&store, &s, &b, &c)
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND query LIKE 'SELECT theatre_identity_finish(%' AND wait_event_type='Lock')",&[]).await.unwrap().get(0);
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("actual issuance must wait on actor");
    tx.execute("UPDATE actors SET active=false WHERE id=$1", &[&actor])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(callback.await.unwrap().err(), Some(AuthError::Inactive));
    assert_eq!(h.count("theatre_identity_sessions").await, 2);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; synthetic immutable receipt, not physical playback"]
async fn actual_listener_identity_keeps_a_receipt_private_after_logout_and_account_b_login() {
    let h = Harness::new().await;
    let (a_token, a_actor) = h.login(None).await;
    let start = h.oidc().begin(&h.store, None).await.unwrap();
    let browser = start
        .cookie
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1;
    let (state, code) = h.mock.issue(&start.url, "nguyễn-khánh", None);
    let b = h
        .oidc()
        .callback(&h.store, &state, browser, &code)
        .await
        .unwrap();
    assert_ne!(a_actor, b.actor_id);
    let b_cookie = b.cookie.split(';').next().unwrap();
    let operation = uuid::Uuid::new_v4().to_string();
    let receipt = cantos_contracts::listener_library::Receipt {
        operation_id: operation.clone(),
        outcome: cantos_contracts::listener_library::WriteOutcome::Accepted,
        resolution: None,
        state: cantos_contracts::listener_library::MutationState::History {
            current: cantos_contracts::listener_library::HistoryState {
                revision: 1,
                epoch: 1,
            },
        },
    };
    let bytes = serde_json::to_vec(&receipt).unwrap();
    h.admin
        .execute(
            "INSERT INTO listener_state(listener_id) VALUES($1)",
            &[&a_actor],
        )
        .await
        .unwrap();
    h.admin.execute("INSERT INTO listener_operations(listener_id,operation_id,request_digest,response_bytes,response_digest) VALUES($1,$2,decode(repeat('00',32),'hex'),$3,sha256($3))",&[&a_actor,&operation,&bytes]).await.unwrap();
    let path = format!("/api/v1/theatre/operations/{operation}");
    assert_eq!(
        request(
            &h,
            "GET",
            &path,
            Some(&format!("cantos_theatre={a_token}")),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    assert_eq!(
        request(&h, "GET", &path, Some(b_cookie), None, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    h.oidc()
        .logout(&h.store, Some(&a_token), None)
        .await
        .unwrap();
    assert_eq!(
        request(
            &h,
            "GET",
            &path,
            Some(&format!("cantos_theatre={a_token}")),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        request(&h, "GET", &path, Some(b_cookie), None, None)
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
}

use cantos_api::contexts::listening::auth::TheatreListenerAuthenticator;

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; expiry during actual actor admission wait"]
async fn bff_admission_refreshes_session_expiry_after_actor_lock_wait() {
    let mut h = Harness::new().await;
    let (token, actor) = h.login(None).await;
    h.admin.execute("UPDATE theatre_identity_sessions SET expires_at=clock_timestamp()+interval '2 seconds' WHERE token_hash=sha256(convert_to($1,'UTF8'))",&[&token]).await.unwrap();
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    config.dbname(&db);
    let observer = connect(&config).await;
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one("SELECT id FROM actors WHERE id=$1 FOR UPDATE", &[&actor])
        .await
        .unwrap();
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    );
    let pending = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .uri("/api/v1/theatre/session")
                .header(header::COOKIE, format!("cantos_theatre={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        loop {
            let blocked:bool=observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND query LIKE 'SELECT listener_lock_actor(%' AND wait_event_type='Lock')",&[]).await.unwrap().get(0);
            if blocked { break; }
            tokio::task::yield_now().await;
        }
    }).await.expect("BFF request must reach actor admission lock before expiry");
    tx.query_one("SELECT pg_sleep(2.1)", &[]).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(pending.await.unwrap().status(), StatusCode::UNAUTHORIZED);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; expiry during actual issuance trigger wait"]
async fn expired_theatre_flow_rolls_back_registration_after_a_session_insert_wait() {
    let h = Harness::new().await;
    let (state, browser, code) = h.start(None).await;
    h.admin.batch_execute("CREATE FUNCTION fixture_pause_theatre_issue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(1.1); RETURN NEW; END $$; CREATE TRIGGER fixture_pause_theatre_issue AFTER INSERT ON theatre_identity_sessions FOR EACH ROW EXECUTE FUNCTION fixture_pause_theatre_issue(); UPDATE theatre_identity_flows SET expires_at=clock_timestamp()+interval '1 second';").await.unwrap();
    assert_eq!(
        h.oidc()
            .callback(&h.store, &state, &browser, &code)
            .await
            .err(),
        Some(AuthError::Expired)
    );
    assert_eq!(h.count("actors").await, 0);
    assert_eq!(h.count("actor_external_identities").await, 0);
    assert_eq!(h.count("theatre_identity_sessions").await, 0);
    assert_eq!(
        h.oidc()
            .callback(&h.store, &state, &browser, &code)
            .await
            .err(),
        Some(AuthError::State)
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; stale tab cleanup/session intent fence"]
async fn old_account_requests_and_cleanup_cannot_mutate_or_revoke_the_new_cookie_session() {
    let h = Harness::new().await;
    let (a, _) = h.login(None).await;
    let (b, _) = h.login(None).await;
    let id = |token: &str| cantos_api::platform::postgres::token_hash(token);
    let a_session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&id(&a)],
        )
        .await
        .unwrap()
        .get(0);
    let b_session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&id(&b)],
        )
        .await
        .unwrap()
        .get(0);
    for (stale, expected) in [(false, Some(a_session.as_str())), (true, None)] {
        let mut req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/theatre/session")
            .header(header::COOKIE, format!("cantos_theatre={b}"))
            .header(header::ORIGIN, "http://127.0.0.1:8080");
        if stale {
            req = req.header("x-cantos-theatre-stale-only", "1");
        }
        if let Some(expected) = expected {
            req = req.header("x-cantos-theatre-session", expected);
        }
        let response = router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "no-static-fixtures",
        )
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        assert_eq!(response.headers()["x-cantos-theatre-session"], b_session);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        assert_eq!(
            request(
                &h,
                "GET",
                "/api/v1/theatre/session",
                Some(&format!("cantos_theatre={b}")),
                None,
                None
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
    h.mock.data.introspection_fail.store(true, Ordering::SeqCst);
    let flows_before = h.count("theatre_identity_flows").await;
    assert_eq!(
        h.oidc().begin(&h.store, Some(&b)).await.err(),
        Some(AuthError::ProviderUnavailable)
    );
    assert_eq!(h.count("theatre_identity_flows").await, flows_before);
    let response = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    )
    .oneshot(
        Request::builder()
            .method("DELETE")
            .uri("/api/v1/theatre/session")
            .header(header::COOKIE, format!("cantos_theatre={b}"))
            .header(header::ORIGIN, "http://127.0.0.1:8080")
            .header("x-cantos-theatre-stale-only", "1")
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let revoked: bool = h
        .admin
        .query_one(
            "SELECT revoked FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&id(&b)],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!revoked);
    h.mock
        .data
        .introspection_fail
        .store(false, Ordering::SeqCst);
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&format!("cantos_theatre={b}")),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    let response=router(AppState{store:h.store.clone(),web_origin:"http://127.0.0.1:8080".into()},"no-static-fixtures").oneshot(Request::builder().method("POST").uri("/api/v1/theatre/library/history/clear")
        .header(header::COOKIE,format!("cantos_theatre={b}"))
        .header(header::ORIGIN,"http://127.0.0.1:8080")
        .header("x-cantos-theatre-session",&a_session).header(header::CONTENT_TYPE,"application/json")
        .body(Body::from(serde_json::to_vec(&serde_json::json!({"operation_id":uuid::Uuid::new_v4().to_string(),"history_epoch":0})).unwrap())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(h.count("listener_state").await, 0);
    assert_eq!(h.count("listener_operations").await, 0);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; delayed logout response/shared cookie order"]
async fn delayed_guarded_logout_and_completed_callbacks_never_expire_new_shared_cookies() {
    let h = Harness::new().await;
    let (old, _) = h.login(None).await;
    let old_session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(&old)],
        )
        .await
        .unwrap()
        .get(0);
    // Both requests began while anonymous. Their immutable request cookie snapshots must not
    // emit flow-expiry headers that erase the other in-flight binder when responses reorder.
    let first = h.oidc().begin(&h.store, None).await.unwrap();
    let second = h.oidc().begin(&h.store, None).await.unwrap();
    let (first_state, first_code) = h.mock.issue(&first.url, "linh", None);
    let (second_state, second_code) = h.mock.issue(&second.url, "an", None);
    let first_browser = first.cookie.split(';').next().unwrap().to_owned();
    let second_browser = second.cookie.split(';').next().unwrap().to_owned();

    h.mock.data.revoke_pause.store(true, Ordering::SeqCst);
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    );
    let old_delete_token = old.clone();
    let old_delete = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/theatre/session")
                .header(header::COOKIE, format!("cantos_theatre={old_delete_token}"))
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header("x-cantos-theatre-session", old_session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(2),
        h.mock.data.revoke_started.notified(),
    )
    .await
    .expect("local revoke reached the bounded provider effect");
    let first_path = format!("/api/v1/theatre/auth/callback?state={first_state}&code={first_code}");
    let second_path =
        format!("/api/v1/theatre/auth/callback?state={second_state}&code={second_code}");
    let (first_response, second_response) = tokio::join!(
        request(&h, "GET", &first_path, Some(&first_browser), None, None),
        request(&h, "GET", &second_path, Some(&second_browser), None, None),
    );
    h.mock.data.revoke_release.notify_one();
    let old_response = old_delete.await.unwrap();
    assert_eq!(old_response.status(), StatusCode::NO_CONTENT);
    assert!(
        !old_response.headers().contains_key(header::SET_COOKIE),
        "a late old-account response must not expire any shared cookie"
    );
    for response in [&first_response, &second_response] {
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let values: Vec<_> = response
            .headers()
            .get_all(header::SET_COOKIE)
            .iter()
            .collect();
        assert_eq!(
            values.len(),
            1,
            "completed SQL flow stays one-time without global flow expiry"
        );
        assert!(values[0].to_str().unwrap().starts_with("cantos_theatre="));
        assert!(!values[0].to_str().unwrap().contains("Max-Age=0"));
    }
    let new_cookie = second_response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(&new_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    // An inert, positively locally revoked cookie may be replaced by a new login; an arbitrary
    // unrecognized credential never receives that exception.
    assert!(h.oidc().begin(&h.store, Some(&old)).await.is_ok());
    let unknown = "e".repeat(64);
    assert_eq!(
        h.oidc().begin(&h.store, Some(&unknown)).await.err(),
        Some(AuthError::Inactive)
    );
    let new_token = new_cookie.split_once('=').unwrap().1;
    h.admin.execute("UPDATE theatre_identity_sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash=$1", &[&cantos_api::platform::postgres::token_hash(new_token)]).await.unwrap();
    assert!(h.oidc().begin(&h.store, Some(new_token)).await.is_ok());
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; stale cleanup cannot cancel another tab's guest flow"]
async fn guarded_inert_a_cookie_does_not_cancel_new_anonymous_b_flow() {
    let h = Harness::new().await;
    let (a, _) = h.login(None).await;
    let a_session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(&a)],
        )
        .await
        .unwrap()
        .get(0);
    h.oidc().logout(&h.store, Some(&a), None).await.unwrap();
    for stale_only in [false, true] {
        let start = h.oidc().begin(&h.store, Some(&a)).await.unwrap();
        let (state, code) = h.mock.issue(&start.url, "b-pending", None);
        let browser_cookie = start.cookie.split(';').next().unwrap();
        let mut req = Request::builder()
            .method("DELETE")
            .uri("/api/v1/theatre/session")
            .header(
                header::COOKIE,
                format!("cantos_theatre={a}; {browser_cookie}"),
            )
            .header(header::ORIGIN, "http://127.0.0.1:8080");
        req = if stale_only {
            req.header("x-cantos-theatre-stale-only", "1")
        } else {
            req.header("x-cantos-theatre-session", &a_session)
        };
        let response = router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "no-static-fixtures",
        )
        .oneshot(req.body(Body::empty()).unwrap())
        .await
        .unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        let cancelled: bool = h
            .admin
            .query_one(
                "SELECT cancelled FROM theatre_identity_flows WHERE state_hash=$1",
                &[&cantos_api::platform::postgres::token_hash(&state)],
            )
            .await
            .unwrap()
            .get(0);
        assert!(
            !cancelled,
            "current browser binder is not proof that B's anonymous flow belongs to old A"
        );
        let path = format!("/api/v1/theatre/auth/callback?state={state}&code={code}");
        let response = request(&h, "GET", &path, Some(browser_cookie), None, None).await;
        assert_eq!(response.status(), StatusCode::SEE_OTHER);
        let cookie = response.headers()[header::SET_COOKIE]
            .to_str()
            .unwrap()
            .split(';')
            .next()
            .unwrap();
        assert_eq!(
            request(
                &h,
                "GET",
                "/api/v1/theatre/session",
                Some(cookie),
                None,
                None
            )
            .await
            .status(),
            StatusCode::OK
        );
    }
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; two guest callbacks plus third binder cannot resurrect after logout"]
async fn immutable_guest_inventory_logout_cancels_paused_callback_and_permits_fresh_login() {
    let h = Harness::new().await;
    let first = h.oidc().begin(&h.store, None).await.unwrap();
    let second = h.oidc().begin(&h.store, None).await.unwrap();
    let (s1, c1) = h.mock.issue(&first.url, "first-guest", None);
    let (s2, c2) = h.mock.issue(&second.url, "late-guest", None);
    let b1 = first.cookie.split(';').next().unwrap().to_owned();
    let b2 = second.cookie.split(';').next().unwrap().to_owned();
    let pause = h.mock.pause_code(&c2);
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    );
    let second_path = format!("/api/v1/theatre/auth/callback?state={s2}&code={c2}");
    let second_cookie = b2.clone();
    let late = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .uri(second_path)
                .header(header::COOKIE, second_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(2), pause.entered.notified())
        .await
        .unwrap();
    let first_path = format!("/api/v1/theatre/auth/callback?state={s1}&code={c1}");
    let first_response = request(&h, "GET", &first_path, Some(&b1), None, None).await;
    assert_eq!(first_response.status(), StatusCode::SEE_OTHER);
    let session_cookie = first_response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let first_token = session_cookie.split_once('=').unwrap().1;
    let session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(first_token)],
        )
        .await
        .unwrap()
        .get(0);
    let third = h.oidc().begin(&h.store, None).await.unwrap();
    let b3 = third.cookie.split(';').next().unwrap();
    assert_ne!(b1.split_once('=').unwrap().0, b2.split_once('=').unwrap().0);
    let inventory_cookie = format!("{session_cookie}; {b1}; {b2}; {b3}");
    let response = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixtures",
    )
    .oneshot(
        Request::builder()
            .method("DELETE")
            .uri("/api/v1/theatre/session")
            .header(header::ORIGIN, "http://127.0.0.1:8080")
            .header(header::COOKIE, &inventory_cookie)
            .header("x-cantos-theatre-session", session)
            .body(Body::empty())
            .unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    pause.release.notify_one();
    let late_response = late.await.unwrap();
    assert_eq!(late_response.status(), StatusCode::UNAUTHORIZED);
    assert!(!late_response.headers().contains_key(header::SET_COOKIE));
    assert_eq!(h.count("theatre_identity_sessions").await, 1);
    let cancelled: bool = h
        .admin
        .query_one(
            "SELECT cancelled FROM theatre_identity_flows WHERE state_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(&s2)],
        )
        .await
        .unwrap()
        .get(0);
    assert!(cancelled);
    let fresh = h.oidc().begin(&h.store, Some(first_token)).await.unwrap();
    let (state, code) = h.mock.issue(&fresh.url, "fresh-after-logout", None);
    let fresh_path = format!("/api/v1/theatre/auth/callback?state={state}&code={code}");
    let response = request(
        &h,
        "GET",
        &fresh_path,
        Some(fresh.cookie.split(';').next().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    let fresh_cookie = response.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(fresh_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; held successful callback receipt is revoked with its browser inventory"]
async fn immutable_inventory_revokes_already_issued_sibling_even_when_its_cookie_response_is_held()
{
    let h = Harness::new().await;
    let a = h.oidc().begin(&h.store, None).await.unwrap();
    let b = h.oidc().begin(&h.store, None).await.unwrap();
    let (sa, ca) = h.mock.issue(&a.url, "visible-a", None);
    let (sb, cb) = h.mock.issue(&b.url, "held-b", None);
    let ba = a.cookie.split(';').next().unwrap();
    let bb = b.cookie.split(';').next().unwrap();
    let pa = format!("/api/v1/theatre/auth/callback?state={sa}&code={ca}");
    let pb = format!("/api/v1/theatre/auth/callback?state={sb}&code={cb}");
    let ar = request(&h, "GET", &pa, Some(ba), None, None).await;
    let br = request(&h, "GET", &pb, Some(bb), None, None).await;
    assert_eq!(ar.status(), StatusCode::SEE_OTHER);
    assert_eq!(br.status(), StatusCode::SEE_OTHER);
    let ac = ar.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let bc = br.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let at = ac.split_once('=').unwrap().1;
    let session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(at)],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(h.count("theatre_identity_sessions").await, 2);
    // No guard is a retired protocol, with no authority-changing effect.
    let app = || {
        router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "no-static-fixtures",
        )
    };
    let unguarded = app()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/theatre/session")
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header(header::COOKIE, ac)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unguarded.status(), StatusCode::UNAUTHORIZED);
    assert!(!unguarded.headers().contains_key(header::SET_COOKIE));
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", Some(ac), None, None)
            .await
            .status(),
        StatusCode::OK
    );
    // B's successful response is withheld while A's current cookie dispatches logout.
    // A's own source is included even without its completed binder in the request.
    let response = app()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/theatre/session")
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header(header::COOKIE, format!("{ac}; {bb}"))
                .header("x-cantos-theatre-session", session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let revoked: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM theatre_identity_sessions WHERE revoked",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(revoked, 2);
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", Some(bc), None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; attributed sibling retry preserves fresh B"]
async fn inert_a_retry_finishes_captured_sibling_revoke_without_cancelling_fresh_b() {
    let h = Harness::new().await;
    let a = h.oidc().begin(&h.store, None).await.unwrap();
    let sibling = h.oidc().begin(&h.store, None).await.unwrap();
    let (sa, ca) = h.mock.issue(&a.url, "retry-a", None);
    let (ss, cs) = h.mock.issue(&sibling.url, "retry-sibling", None);
    let ba = a.cookie.split(';').next().unwrap();
    let bs = sibling.cookie.split(';').next().unwrap();
    let ar = request(
        &h,
        "GET",
        &format!("/api/v1/theatre/auth/callback?state={sa}&code={ca}"),
        Some(ba),
        None,
        None,
    )
    .await;
    let sr = request(
        &h,
        "GET",
        &format!("/api/v1/theatre/auth/callback?state={ss}&code={cs}"),
        Some(bs),
        None,
        None,
    )
    .await;
    assert_eq!(sr.status(), StatusCode::SEE_OTHER);
    let ac = ar.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let at = ac.split_once('=').unwrap().1;
    let session: String = h
        .admin
        .query_one(
            "SELECT session_id::text FROM theatre_identity_sessions WHERE token_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(at)],
        )
        .await
        .unwrap()
        .get(0);
    let delete = |cookie: String| {
        Request::builder()
            .method("DELETE")
            .uri("/api/v1/theatre/session")
            .header(header::ORIGIN, "http://127.0.0.1:8080")
            .header(header::COOKIE, cookie)
            .header("x-cantos-theatre-session", &session)
            .body(Body::empty())
            .unwrap()
    };
    let app = || {
        router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "no-static-fixtures",
        )
    };
    h.mock.data.revoke_fail.store(true, Ordering::SeqCst);
    let first = app()
        .oneshot(delete(format!("{ac}; {ba}; {bs}")))
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::NO_CONTENT);
    assert_eq!(first.headers()["cantos-provider-revocation"], "pending");
    let pending: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM theatre_identity_sessions WHERE revoked AND revocation_pending",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(pending, 2);
    let b = h.oidc().begin(&h.store, Some(at)).await.unwrap();
    let (sb, cb) = h.mock.issue(&b.url, "fresh-b-during-retry", None);
    let bb = b.cookie.split(';').next().unwrap();
    let br = request(
        &h,
        "GET",
        &format!("/api/v1/theatre/auth/callback?state={sb}&code={cb}"),
        Some(bb),
        None,
        None,
    )
    .await;
    assert_eq!(br.status(), StatusCode::SEE_OTHER);
    let bc = br.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    h.mock.data.revoke_fail.store(false, Ordering::SeqCst);
    // Old inventory is gone; durable attribution recovers captured pending siblings.
    let retry = app().oneshot(delete(format!("{ac}; {bb}"))).await.unwrap();
    assert_eq!(retry.status(), StatusCode::NO_CONTENT);
    assert_eq!(retry.headers()["cantos-provider-revocation"], "completed");
    let pending: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM theatre_identity_sessions WHERE revoked AND revocation_pending",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(pending, 0);
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", Some(bc), None, None)
            .await
            .status(),
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; rotation lost ack needs captured unexpired binder, never root authority alone"]
async fn retired_a_cancels_only_captured_unexpired_rotation_receipts_and_preserves_observed_b() {
    let h = Harness::new().await;
    let (a, _) = h.login(None).await;
    let ac = format!("cantos_theatre={a}");
    let rotation = h.oidc().begin(&h.store, Some(&a)).await.unwrap();
    let (sb, cb) = h.mock.issue(&rotation.url, "observed-rotation-b", None);
    let bb = rotation.cookie.split(';').next().unwrap();
    let path = format!("/api/v1/theatre/auth/callback?state={sb}&code={cb}");
    let br = request(&h, "GET", &path, Some(bb), None, None).await;
    assert_eq!(br.status(), StatusCode::SEE_OTHER);
    let bc = br.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let bt = bc.split_once('=').unwrap().1;
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", Some(bc), None, None)
            .await
            .status(),
        StatusCode::OK
    );
    // A's retired credential/UUID alone cannot cancel B after a successful account switch.
    let old = request(
        &h,
        "DELETE",
        "/api/v1/theatre/session",
        Some(&ac),
        Some("http://127.0.0.1:8080"),
        None,
    )
    .await;
    assert_eq!(old.status(), StatusCode::NO_CONTENT);
    assert!(!old.headers().contains_key(header::SET_COOKIE));
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", Some(bc), None, None)
            .await
            .status(),
        StatusCode::OK
    );
    let cancelled: bool = h
        .admin
        .query_one(
            "SELECT cancelled FROM theatre_identity_flows WHERE state_hash=$1",
            &[&cantos_api::platform::postgres::token_hash(&sb)],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!cancelled);
    // Raw replay of an expired binder is not a standing cancellation capability.
    h.admin.execute("UPDATE theatre_identity_flows SET expires_at=clock_timestamp()-interval '1 second' WHERE state_hash=$1", &[&cantos_api::platform::postgres::token_hash(&sb)]).await.unwrap();
    let expired = request(
        &h,
        "DELETE",
        "/api/v1/theatre/session",
        Some(&format!("{ac}; {bb}")),
        Some("http://127.0.0.1:8080"),
        None,
    )
    .await;
    assert_eq!(expired.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        request(&h, "GET", "/api/v1/theatre/session", Some(bc), None, None)
            .await
            .status(),
        StatusCode::OK
    );
    // A→B→C propagates the serialization root; only C's captured fresh binder/source receipt
    // authorizes cancelling the successful but withheld C callback response.
    let next = h.oidc().begin(&h.store, Some(bt)).await.unwrap();
    let (sc, cc) = h.mock.issue(&next.url, "held-rotation-c", None);
    let cbinder = next.cookie.split(';').next().unwrap();
    let path = format!("/api/v1/theatre/auth/callback?state={sc}&code={cc}");
    let cr = request(&h, "GET", &path, Some(cbinder), None, None).await;
    assert_eq!(cr.status(), StatusCode::SEE_OTHER);
    let c_cookie = cr.headers()[header::SET_COOKIE]
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap();
    let old = request(
        &h,
        "DELETE",
        "/api/v1/theatre/session",
        Some(&ac),
        Some("http://127.0.0.1:8080"),
        None,
    )
    .await;
    assert_eq!(old.status(), StatusCode::NO_CONTENT);
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(c_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::OK
    );
    let with_receipt = request(
        &h,
        "DELETE",
        "/api/v1/theatre/session",
        Some(&format!("{ac}; {cbinder}")),
        Some("http://127.0.0.1:8080"),
        None,
    )
    .await;
    assert_eq!(with_receipt.status(), StatusCode::NO_CONTENT);
    assert!(!with_receipt.headers().contains_key(header::SET_COOKIE));
    assert_eq!(
        request(
            &h,
            "GET",
            "/api/v1/theatre/session",
            Some(c_cookie),
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
}
