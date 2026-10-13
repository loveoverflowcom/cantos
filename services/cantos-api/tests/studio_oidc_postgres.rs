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
        studio_oidc::{AuthError, OidcConfig, StudioOidc},
    },
};
use http_body_util::BodyExt;
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
        admin.batch_execute("GRANT SELECT ON actors,sessions,studio_oidc_sessions TO cantos_app; GRANT UPDATE ON sessions TO cantos_app; GRANT SELECT,INSERT,UPDATE ON studio_oidc_flows TO cantos_app; GRANT UPDATE(revocation_pending,revoked_by_token_hash) ON studio_oidc_sessions TO cantos_app; GRANT EXECUTE ON FUNCTION studio_oidc_finish(bytea,text,text,text,text,bytea,bytea,text,bigint),production_lock_actor(text) TO cantos_app;").await.unwrap();
        config.user("cantos_app");
        let mock = mock::MockProvider::start(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
        )
        .await;
        let oidc_config =
            OidcConfig::new(&mock.issuer(), "http://127.0.0.1:8080", "fixture".into()).unwrap();
        let store = Store::new(config)
            .unwrap()
            .with_studio_oidc(StudioOidc::new(oidc_config.clone()).unwrap());
        Self { admin, store, mock }
    }
    fn oidc(&self) -> &StudioOidc {
        self.store.studio_oidc().unwrap()
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
    async fn login_subject(&self, subject: &str) -> (String, String) {
        let start = self.oidc().begin(&self.store, None).await.unwrap();
        let browser = start
            .cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1;
        let (state, code) = self.mock.issue(&start.url, subject, None);
        let complete = self
            .oidc()
            .callback(&self.store, &state, browser, &code)
            .await
            .unwrap();
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
    async fn count(&self, table: &str) -> i64 {
        self.admin
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .unwrap()
            .get(0)
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn callback_is_single_use_browser_bound_and_failed_exchange_is_not_retried() {
    let h = Harness::new().await;
    let (s, b, c) = h.start(None).await;
    assert_eq!(
        h.oidc()
            .callback(&h.store, &s, &"a".repeat(64), &c)
            .await
            .err(),
        Some(AuthError::State)
    );
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), 0);
    let (one, two) = tokio::join!(
        h.oidc().callback(&h.store, &s, &b, &c),
        h.oidc().callback(&h.store, &s, &b, &c)
    );
    assert_eq!(usize::from(one.is_ok()) + usize::from(two.is_ok()), 1);
    assert_eq!(one.err().or(two.err()), Some(AuthError::State));
    assert_eq!(h.count("sessions").await, 1);
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), 1);
    let (s, b, c) = h.start(None).await;
    h.mock.data.token_fail.store(true, Ordering::SeqCst);
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::ProviderUnavailable)
    );
    let attempts = h.mock.data.token_calls.load(Ordering::SeqCst);
    h.mock.data.token_fail.store(false, Ordering::SeqCst);
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), attempts);
    assert_eq!(h.count("sessions").await, 1);
    // An issued code expires at the provider while the local browser flow is still valid.
    // invalid_grant must consume the callback without minting authority or retrying exchange.
    let (s, b, c) = h.start(None).await;
    h.mock.expire_code(&c);
    let attempts = h.mock.data.token_calls.load(Ordering::SeqCst);
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::ProviderUnavailable)
    );
    assert_eq!(h.count("sessions").await, 1);
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), attempts + 1);
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.mock.data.token_calls.load(Ordering::SeqCst), attempts + 1);
    // A lost successful callback acknowledgement cannot replay the code or expose a second cookie.
    let (s, b, c) = h.start(None).await;
    drop(h.oidc().callback(&h.store, &s, &b, &c).await.unwrap());
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.count("sessions").await, 2);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn mapping_is_idempotent_isolated_and_invalid_or_expired_proofs_create_no_actor() {
    let h = Harness::new().await;
    let (token, actor) = h.login(None).await;
    assert_eq!(h.store.authenticate(&token).await.unwrap(), actor);
    let old_token = h.mock.data.last_id_token.lock().unwrap().clone().unwrap();
    let (s, b, c) = h.start(None).await;
    *h.mock.data.id_override.lock().unwrap() = Some(old_token);
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::Nonce)
    );
    *h.mock.data.id_override.lock().unwrap() = None;
    assert_eq!(h.count("sessions").await, 1);
    let (_, same) = h.login(None).await;
    assert_eq!(same, actor);
    assert_eq!(h.count("actors").await, 1);
    let start = h.oidc().begin(&h.store, None).await.unwrap();
    let b = start
        .cookie
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1;
    let (s, c) = h.mock.issue(&start.url, "another", None);
    let different = h.oidc().callback(&h.store, &s, b, &c).await.unwrap();
    assert_ne!(different.actor_id, actor);
    assert_eq!(h.count("actor_external_identities").await, 2);
    for (field, value, expected) in [
        ("iss", serde_json::json!("https://other"), AuthError::Issuer),
        (
            "aud",
            serde_json::json!("cantos-theatre"),
            AuthError::Audience,
        ),
        ("nonce", serde_json::json!("wrong"), AuthError::Nonce),
        ("exp", serde_json::json!(1), AuthError::Expired),
    ] {
        let start = h.oidc().begin(&h.store, None).await.unwrap();
        let b = start
            .cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1;
        let (s, c) = h
            .mock
            .issue(&start.url, "rejected", Some((field.into(), value)));
        assert_eq!(
            h.oidc().callback(&h.store, &s, b, &c).await.err(),
            Some(expected)
        );
    }
    assert_eq!(h.count("actors").await, 2);
    let (s, b, c) = h.start(None).await;
    h.admin.batch_execute("UPDATE studio_oidc_flows SET expires_at=clock_timestamp()-interval '1 second' WHERE NOT consumed").await.unwrap();
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::State)
    );
    h.admin
        .execute("UPDATE actors SET active=false WHERE id=$1", &[&actor])
        .await
        .unwrap();
    assert!(matches!(
        h.store.authenticate(&token).await,
        Err(cantos_api::platform::postgres::StoreError::Unauthenticated)
    ));
    let (s, b, c) = h.start(None).await;
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::Inactive)
    );
    assert_eq!(h.count("actors").await, 2);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn rotation_and_logout_races_preserve_revocation_and_provider_outage_is_durable() {
    let h = Harness::new().await;
    let (old, actor) = h.login(None).await;
    let (new, same) = h.login(Some(&old)).await;
    assert_ne!(old, new);
    assert_eq!(actor, same);
    assert!(h.store.authenticate(&old).await.is_err());
    assert_eq!(h.store.authenticate(&new).await.unwrap(), actor);
    let (s, b, c) = h.start(Some(&new)).await;
    h.mock.data.revoke_fail.store(true, Ordering::SeqCst);
    let logout = h.oidc().logout(&h.store, Some(&new), None).await.unwrap();
    assert!(!logout.provider_revoked);
    assert!(logout
        .cookies
        .iter()
        .all(|cookie| cookie.contains("Max-Age=0")));
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::Inactive)
    );
    assert!(h.store.authenticate(&new).await.is_err());
    assert!(h
        .admin
        .query_one(
            "SELECT bool_or(revocation_pending) FROM studio_oidc_sessions",
            &[]
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    h.mock.data.revoke_fail.store(false, Ordering::SeqCst);
    assert!(
        h.oidc()
            .logout(&h.store, Some(&new), None)
            .await
            .unwrap()
            .provider_revoked
    );
    let (s, b, c) = h.start(None).await;
    h.oidc().logout(&h.store, None, Some(&b)).await.unwrap();
    assert_eq!(
        h.oidc().callback(&h.store, &s, &b, &c).await.err(),
        Some(AuthError::State)
    );
    let (token, _) = h.login(None).await;
    h.mock.data.active.store(false, Ordering::SeqCst);
    assert!(matches!(
        h.store.authenticate(&token).await,
        Err(cantos_api::platform::postgres::StoreError::Unauthenticated)
    ));
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn studio_http_session_paths_csrf_cookie_and_development_credentials_are_isolated() {
    let h = Harness::new().await;
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let configuration = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/configuration")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(configuration.status(), StatusCode::OK);
    assert_eq!(configuration.headers()[header::CACHE_CONTROL], "no-store");
    assert!(!configuration.headers().contains_key(header::SET_COOKIE));
    let bytes = configuration
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        serde_json::json!({"mode":"oidc"})
    );
    for path in [
        "/api/v1/theatre/auth/configuration",
        "/api/v1/admin/auth/configuration",
        "/api/v1/auth/configuration",
    ] {
        let response = app
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let start = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/login")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(start.status(), StatusCode::SEE_OTHER);
    assert_eq!(start.headers()["cache-control"], "no-store");
    assert_eq!(start.headers()["referrer-policy"], "no-referrer");
    let cookie = start.headers()[header::SET_COOKIE].to_str().unwrap();
    assert!(cookie.contains("HttpOnly; SameSite=Lax; Path=/api/v1/studio; Max-Age=300"));
    let browser = cookie.split(';').next().unwrap();
    let (s, c) = h.mock.issue(
        start.headers()[header::LOCATION].to_str().unwrap(),
        "minh",
        None,
    );
    let callback = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/v1/studio/auth/callback?state={s}&code={c}"))
                .header(header::COOKIE, browser)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
    let cookies: Vec<_> = callback
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect();
    assert!(cookies[0].contains("HttpOnly; SameSite=Strict; Path=/api/v1/studio"));
    assert_eq!(
        cookies.len(),
        1,
        "callback must not expire a newer shared browser flow"
    );
    let session = cookies[0].split(';').next().unwrap();
    for (path, status) in [
        ("/api/v1/studio/session", StatusCode::OK),
        ("/api/v1/theatre/session", StatusCode::NOT_FOUND),
        ("/api/v1/admin/session", StatusCode::NOT_FOUND),
        ("/api/v1/auth/login", StatusCode::NOT_FOUND),
    ] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header(header::COOKIE, session)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status);
    }
    let cross = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/login")
                .header("sec-fetch-site", "cross-site")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cross.status(), StatusCode::FORBIDDEN);
    let login = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/v1/studio/session")
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{\"token\":\"development\"}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(login.status(), StatusCode::FORBIDDEN);
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let logout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, session)
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header(
                    "x-cantos-studio-session",
                    h.store
                        .studio_session_id(session.split_once('=').unwrap().1)
                        .await
                        .unwrap()
                        .unwrap(),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    assert_eq!(logout.headers()["cantos-provider-revocation"], "completed");
    let denied = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    // HTTP DB role still cannot select/insert registry bindings or arbitrary actors/sessions.
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    config.dbname(&db).user("cantos_app");
    let client = connect(&config).await;
    for sql in ["SELECT * FROM actor_external_identities","INSERT INTO actors(id) VALUES('spoof')","INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES(decode(repeat('00',32),'hex'),'spoof',now())"] {assert_eq!(client.batch_execute(sql).await.unwrap_err().code().unwrap().code(),"42501");}
}

async fn wait_for_finish_lock(admin: &Client) {
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let blocked:bool=admin.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT studio_oidc_finish%')",&[]).await.unwrap().get(0);
            if blocked {break;}
            tokio::task::yield_now().await;
        }
    }).await.expect("callback must reach the actual SQL lock");
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn frozen_a_requests_and_stale_cleanup_cannot_read_or_revoke_cookie_b() {
    const FENCE: &str = "x-cantos-studio-session";
    let h = Harness::new().await;
    let (a, actor_a) = h.login_subject("account-a").await;
    let (b, actor_b) = h.login_subject("account-b").await;
    assert_ne!(actor_a, actor_b);
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let mut ids = Vec::new();
    for (token, actor) in [(&a, &actor_a), (&b, &actor_b)] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, format!("cantos_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let id = response.headers()[FENCE].to_str().unwrap().to_owned();
        assert!(uuid::Uuid::parse_str(&id).is_ok());
        assert_ne!(id, *token, "the correlation must never be the credential");
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<cantos_contracts::SessionResponse>(&body)
                .unwrap()
                .actor_id,
            *actor
        );
        ids.push(id);
    }
    assert_ne!(ids[0], ids[1]);
    // Each case models an A intent queued before the shared browser cookie jar became B.
    for (method, path, body) in [
        ("GET", "/api/v1/studio/production/catalog", None),
        ("POST", "/api/v1/studio/imports", Some("{}")),
        ("DELETE", "/api/v1/studio/session", None),
        ("GET", "/api/v1/studio/auth/login", None),
    ] {
        let mut request = Request::builder()
            .method(method)
            .uri(path)
            .header(header::COOKIE, format!("cantos_session={b}"))
            .header(header::ORIGIN, "http://127.0.0.1:8080")
            .header(FENCE, &ids[0]);
        if body.is_some() {
            request = request.header(header::CONTENT_TYPE, "application/json");
        }
        let response = app
            .clone()
            .oneshot(request.body(Body::from(body.unwrap_or(""))).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::CONFLICT, "{method} {path}");
        assert_eq!(response.headers()[FENCE], ids[1]);
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        let body = response.into_body().collect().await.unwrap().to_bytes();
        assert_eq!(
            serde_json::from_slice::<cantos_contracts::ApiError>(&body)
                .unwrap()
                .code,
            cantos_contracts::ErrorCode::Unauthenticated
        );
        assert_eq!(h.store.authenticate(&b).await.unwrap(), actor_b);
    }
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/production/catalog")
                .header(header::COOKIE, format!("cantos_session={b}"))
                .header(FENCE, &ids[1])
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[FENCE], ids[1]);
    // Navigation downloads carry the same noncredential UUID as a query fence.
    let response=app.clone().oneshot(Request::builder().uri(format!("/api/v1/studio/imports/00000000-0000-4000-8000-000000000001/original?studio_session_id={}",ids[0])).header(header::COOKIE,format!("cantos_session={b}")).body(Body::empty()).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    for outage in [false, true] {
        if outage {
            *h.mock.data.expected_secret.lock().unwrap() = "wrong-client-secret".into();
        }
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, format!("cantos_session={b}"))
                    .header(header::ORIGIN, "http://127.0.0.1:8080")
                    .header("x-cantos-studio-stale-only", "1")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if outage {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::CONFLICT
            }
        );
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        *h.mock.data.expected_secret.lock().unwrap() = "fixture".into();
        assert_eq!(h.store.authenticate(&b).await.unwrap(), actor_b);
    }
    assert_eq!(h.mock.data.revoke_calls.load(Ordering::SeqCst), 0);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, format!("cantos_session={b}"))
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header(FENCE, &ids[1])
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(
        !response.headers().contains_key(header::SET_COOKIE),
        "a delayed A-style response must never expire a newer B cookie or flow"
    );
    assert!(h.store.authenticate(&b).await.is_err());
    assert_eq!(h.store.authenticate(&a).await.unwrap(), actor_a);
    assert_eq!(h.mock.data.revoke_calls.load(Ordering::SeqCst), 1);
    // The guarded UI left a positively known inert cookie. It must be able to start another
    // login without giving an unknown cookie or a provider outage this anonymous exemption.
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/studio/auth/login?studio_session_id={}",
                    ids[1]
                ))
                .header(header::COOKIE, format!("cantos_session={b}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::SEE_OTHER);
    for token in ["f".repeat(64), a.clone()] {
        *h.mock.data.expected_secret.lock().unwrap() = "wrong-client-secret".into();
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/auth/login")
                    .header(header::COOKIE, format!("cantos_session={token}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if token == a {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::UNAUTHORIZED
            }
        );
        assert!(!response.headers().contains_key(header::SET_COOKIE));
    }
    *h.mock.data.expected_secret.lock().unwrap() = "fixture".into();
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn configured_new_issuer_never_receives_the_old_issuer_revocation_bearer() {
    let h = Harness::new().await;
    let (token, _) = h.login(None).await;
    let replacement = mock::MockProvider::start(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
    )
    .await;
    let config = OidcConfig::new(
        &replacement.issuer(),
        "http://127.0.0.1:8080",
        "fixture".into(),
    )
    .unwrap();
    let store = h
        .store
        .clone()
        .with_studio_oidc(StudioOidc::new(config).unwrap());
    let result = store
        .studio_oidc()
        .unwrap()
        .logout(&store, Some(&token), None)
        .await
        .unwrap();
    assert!(!result.provider_revoked);
    assert_eq!(replacement.data.revoke_calls.load(Ordering::SeqCst), 0);
    assert_eq!(h.mock.data.revoke_calls.load(Ordering::SeqCst), 0);
    let row=h.admin.query_one("SELECT s.revoked,o.revocation_pending FROM sessions s JOIN studio_oidc_sessions o USING(token_hash) WHERE token_hash=$1",&[&cantos_api::platform::postgres::token_hash(&token)]).await.unwrap();
    assert!(row.get::<_, bool>(0));
    assert!(row.get::<_, bool>(1));
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn acknowledged_logout_cancels_consumed_guest_callbacks_without_the_latest_shared_binder() {
    for third_flow in [false, true] {
        let h = Harness::new().await;
        let app = router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "/nonexistent",
        );
        let mut jar = std::collections::BTreeMap::<String, String>::new();
        let mut attempts = Vec::new();
        // Use actual Set-Cookie name semantics: the old mutable implementation loses A1/A2
        // when a newer begin overwrites it; the repair keeps bounded independent binders.
        for subject in ["guest-a1", "guest-a2"] {
            let cookie = jar
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            let begin = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/studio/auth/login")
                        .header(header::COOKIE, &cookie)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(begin.status(), StatusCode::SEE_OTHER);
            for value in begin.headers().get_all(header::SET_COOKIE) {
                let (name, value) = value
                    .to_str()
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap()
                    .split_once('=')
                    .unwrap();
                jar.insert(name.into(), value.into());
            }
            let (state, code) = h.mock.issue(
                begin.headers()[header::LOCATION].to_str().unwrap(),
                subject,
                None,
            );
            let pause = h.mock.pause_code(&code);
            let cookie = jar
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("; ");
            let callback_app = app.clone();
            let callback = tokio::spawn(async move {
                callback_app
                    .oneshot(
                        Request::builder()
                            .uri(format!(
                                "/api/v1/studio/auth/callback?state={state}&code={code}"
                            ))
                            .header(header::COOKIE, cookie)
                            .body(Body::empty())
                            .unwrap(),
                    )
                    .await
                    .unwrap()
            });
            tokio::time::timeout(std::time::Duration::from_secs(5), pause.entered.notified())
                .await
                .unwrap();
            attempts.push((callback, pause));
        }
        let (first, first_pause) = attempts.remove(0);
        first_pause.release.notify_one();
        let first = first.await.unwrap();
        assert_eq!(first.status(), StatusCode::SEE_OTHER);
        for value in first.headers().get_all(header::SET_COOKIE) {
            let (name, value) = value
                .to_str()
                .unwrap()
                .split(';')
                .next()
                .unwrap()
                .split_once('=')
                .unwrap();
            if value.is_empty() {
                jar.remove(name);
            } else {
                jar.insert(name.into(), value.into());
            }
        }
        if third_flow {
            // A third begin changes the old shared binder while A2 is already exchanging.
            let begin = app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri("/api/v1/studio/auth/login")
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(begin.status(), StatusCode::SEE_OTHER);
            for value in begin.headers().get_all(header::SET_COOKIE) {
                let (name, value) = value
                    .to_str()
                    .unwrap()
                    .split(';')
                    .next()
                    .unwrap()
                    .split_once('=')
                    .unwrap();
                jar.insert(name.into(), value.into());
            }
        } else {
            jar.remove(h.oidc().flow_name()); // no shared binder; independent names survive
        }
        let cookie = jar
            .iter()
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        let observed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(observed.status(), StatusCode::OK);
        let expected = observed.headers()["x-cantos-studio-session"].clone();
        let logout = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, &cookie)
                    .header(header::ORIGIN, "http://127.0.0.1:8080")
                    .header("x-cantos-studio-session", expected)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(logout.status(), StatusCode::NO_CONTENT);
        let (late, pause) = attempts.remove(0);
        pause.release.notify_one();
        let late = late.await.unwrap();
        assert_eq!(late.status(),StatusCode::UNAUTHORIZED,"acknowledged logout must fence an already consumed guest callback (third={third_flow})");
        assert!(!late.headers().contains_key(header::SET_COOKIE));
        assert_eq!(h.count("sessions").await, 1);
        let active: i64 = h
            .admin
            .query_one("SELECT count(*) FROM sessions WHERE NOT revoked", &[])
            .await
            .unwrap()
            .get(0);
        assert_eq!(active, 0);
        let fresh = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/auth/login")
                    .header(header::COOKIE, cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(fresh.status(), StatusCode::SEE_OTHER);
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn guarded_legacy_recovery_retires_only_obsolete_paths_and_preserves_the_current_session() {
    let mut h = Harness::new().await;
    let (token, actor) = h.login(None).await;
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let duplicate = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header("x-cantos-studio-stale-only", "1")
                .header(
                    header::COOKIE,
                    format!("cantos_session={token}; cantos_session=legacy-alias"),
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate.status(), StatusCode::CONFLICT);
    let expired: Vec<_> = duplicate
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect();
    assert_eq!(
        expired,
        vec!["cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1; Max-Age=0"]
    );
    assert_eq!(h.store.authenticate(&token).await.unwrap(), actor);
    let development = "d".repeat(64);
    h.admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at,auth_kind) VALUES($1,$2,clock_timestamp()+interval '1 hour','development')",&[&cantos_api::platform::postgres::token_hash(&development),&actor]).await.unwrap();
    assert_eq!(
        h.oidc().begin(&h.store, Some(&development)).await.err(),
        Some(AuthError::Inactive)
    );
    let http = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let retired = http
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header("x-cantos-studio-stale-only", "1")
                .header(header::COOKIE, format!("cantos_session={development}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(retired.status(), StatusCode::NO_CONTENT);
    assert!(
        !retired.headers().contains_key(header::SET_COOKIE),
        "current HTTP cookie path must not expire a newer session"
    );
    let begin = http
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/login")
                .header(header::COOKIE, format!("cantos_session={development}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        begin.status(),
        StatusCode::SEE_OTHER,
        "positivelyknown inert developmentcookie must recover into real OIDC, never into authority"
    );
    h.config_for_secure_test();
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "https://studio.example".into(),
        },
        "/nonexistent",
    );
    let mixed=format!("__Secure-cantos_studio={token}; cantos_session=legacy-studio; cantos_session=legacy-alias; cantos_oidc=legacy-flow");
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, &mixed)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::UNAUTHORIZED);
    let recover = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::ORIGIN, "https://studio.example")
                .header("x-cantos-studio-stale-only", "1")
                .header(header::COOKIE, mixed)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        recover.status(),
        StatusCode::CONFLICT,
        "current valid Secure session must be re-observed, never revoked by stale cleanup"
    );
    let expired: Vec<_> = recover
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect();
    assert_eq!(expired.len(), 3);
    assert!(expired.iter().all(|value| !value.starts_with("__Secure-")));
    assert_eq!(h.store.authenticate(&token).await.unwrap(), actor);
    let current = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, format!("__Secure-cantos_studio={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(current.status(), StatusCode::OK);
    assert_eq!(h.mock.data.revoke_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn inert_a_without_a_fresh_private_binder_cannot_revoke_observed_b_but_lost_ack_is_cancelled()
{
    for proof in ["absent", "expired", "fresh"] {
        let h = Harness::new().await;
        let (a, _) = h.login(None).await;
        let expected = h.store.studio_session_id(&a).await.unwrap().unwrap();
        let (state, binder, code) = h.start(Some(&a)).await;
        let issued = h
            .oidc()
            .callback(&h.store, &state, &binder, &code)
            .await
            .unwrap();
        let b = issued
            .cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_owned();
        let app = router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "/nonexistent",
        );
        let observed = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, format!("cantos_session={b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(observed.status(), StatusCode::OK);
        let mut cookie = format!("cantos_session={a}");
        if proof != "absent" {
            cookie.push_str(&format!(
                "; {}={binder}",
                h.oidc().flow_cookie_name(&state).unwrap()
            ));
            if proof == "expired" {
                h.admin.execute("UPDATE studio_oidc_flows SET expires_at=clock_timestamp()-interval '1 second' WHERE state_hash=$1",&[&cantos_api::platform::postgres::token_hash(&state)]).await.unwrap();
            }
        }
        let logout = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("DELETE")
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, cookie)
                    .header(header::ORIGIN, "http://127.0.0.1:8080")
                    .header("x-cantos-studio-session", expected)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            logout.status(),
            if proof == "expired" {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::NO_CONTENT
            }
        );
        assert!(!logout.headers().contains_key(header::SET_COOKIE));
        let b_after = app
            .oneshot(
                Request::builder()
                    .uri("/api/v1/studio/session")
                    .header(header::COOKIE, format!("cantos_session={b}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            b_after.status(),
            if proof == "fresh" {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::OK
            },
            "{proof}"
        );
        let cancelled: bool = h
            .admin
            .query_one(
                "SELECT cancelled FROM studio_oidc_flows WHERE state_hash=$1",
                &[&cantos_api::platform::postgres::token_hash(&state)],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(cancelled, proof == "fresh");
        if proof == "fresh" {
            assert_eq!(logout.headers()["cantos-provider-revocation"], "pending");
        }
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn inert_a_cleanup_with_new_guest_b_inventory_keeps_b_login_usable() {
    let h = Harness::new().await;
    let (a, _) = h.login(None).await;
    let expected = h.store.studio_session_id(&a).await.unwrap().unwrap();
    h.oidc().logout(&h.store, Some(&a), None).await.unwrap();
    let (state, binder, code) = h.start(None).await;
    let cookie = format!(
        "cantos_session={a}; {}={binder}",
        h.oidc().flow_cookie_name(&state).unwrap()
    );
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let cleanup = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, &cookie)
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header("x-cantos-studio-session", expected)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(cleanup.status(), StatusCode::NO_CONTENT);
    assert!(!cleanup.headers().contains_key(header::SET_COOKIE));
    let callback = app
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/v1/studio/auth/callback?state={state}&code={code}"
                ))
                .header(header::COOKIE, cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(callback.status(), StatusCode::SEE_OTHER);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn browser_inventory_limit_refuses_begin_and_overflow_logout_without_partial_effects() {
    let h = Harness::new().await;
    let (a, _) = h.login(None).await;
    let expected = h.store.studio_session_id(&a).await.unwrap().unwrap();
    let mut cookies = Vec::new();
    for _ in 0..cantos_api::platform::studio_oidc::MAX_BROWSER_FLOWS {
        cookies.push(
            h.oidc()
                .begin(&h.store, None)
                .await
                .unwrap()
                .cookie
                .split(';')
                .next()
                .unwrap()
                .to_owned(),
        );
    }
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let begin = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/login")
                .header(header::COOKIE, cookies.join("; "))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(begin.status(), StatusCode::UNAUTHORIZED);
    assert!(!begin.headers().contains_key(header::SET_COOKIE));
    assert_eq!(h.count("studio_oidc_flows").await, 9);
    // Concurrent initial begin responses may overshoot the bound. No operation truncates the
    // resulting inventory and acknowledges only a subset of a user's logout intent.
    cookies.push(
        h.oidc()
            .begin(&h.store, None)
            .await
            .unwrap()
            .cookie
            .split(';')
            .next()
            .unwrap()
            .to_owned(),
    );
    let cookie = format!("cantos_session={a}; {}", cookies.join("; "));
    let logout = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header("x-cantos-studio-session", expected)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::UNAUTHORIZED);
    assert!(!logout.headers().contains_key(header::SET_COOKIE));
    assert!(h.store.authenticate(&a).await.is_ok());
    let cancelled: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM studio_oidc_flows WHERE cancelled",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(cancelled, 0);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn private_binder_expiry_while_receipt_lock_waits_cannot_revoke_observed_b() {
    let mut h = Harness::new().await;
    let (a, _) = h.login(None).await;
    let expected = h.store.studio_session_id(&a).await.unwrap().unwrap();
    let (state, binder, code) = h.start(Some(&a)).await;
    let issued = h
        .oidc()
        .callback(&h.store, &state, &binder, &code)
        .await
        .unwrap();
    let b = issued
        .cookie
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned();
    let source = cantos_api::platform::postgres::token_hash(&state);
    h.admin.execute("UPDATE studio_oidc_flows SET expires_at=clock_timestamp()+interval '3 seconds' WHERE state_hash=$1",&[&source]).await.unwrap();
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "/nonexistent",
    );
    let cookie = format!(
        "cantos_session={a}; {}={binder}",
        h.oidc().flow_cookie_name(&state).unwrap()
    );
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one(
        "SELECT token_hash FROM studio_oidc_sessions WHERE token_hash=$1 FOR UPDATE",
        &[&cantos_api::platform::postgres::token_hash(&b)],
    )
    .await
    .unwrap();
    let logout = tokio::spawn(async move {
        app.oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, cookie)
                .header(header::ORIGIN, "http://127.0.0.1:8080")
                .header("x-cantos-studio-session", expected)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap()
    });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let blocked:bool=tx.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT token_hash FROM studio_oidc_sessions WHERE token_hash=%')",&[]).await.unwrap().get(0);
            if blocked {break;}
            tokio::task::yield_now().await;
        }
    }).await.expect("logout must wait at the actual receipt lock, after collecting its private binder proof");
    tokio::time::timeout(std::time::Duration::from_secs(4),async {
        loop {
            let expired:bool=tx.query_one("SELECT expires_at<=clock_timestamp() FROM studio_oidc_flows WHERE state_hash=$1",&[&source]).await.unwrap().get(0);
            if expired {break;}
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    tx.commit().await.unwrap();
    let response = logout.await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(h.store.authenticate(&b).await.is_ok());
    assert_eq!(h.mock.data.revoke_calls.load(Ordering::SeqCst), 0);
    let cancelled: bool = h
        .admin
        .query_one(
            "SELECT cancelled FROM studio_oidc_flows WHERE state_hash=$1",
            &[&source],
        )
        .await
        .unwrap()
        .get(0);
    assert!(!cancelled);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn disable_at_final_sql_lock_and_competing_rotation_never_mint_stale_authority() {
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
    assert_eq!(h.count("sessions").await, 2);
    let (s, b, c) = h.start(None).await;
    // Hold the actor row before dispatch. Wait for the production finish statement to block,
    // disable under that lock, then release: the callback must recheck the winning current fact.
    let observer_config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let mut observer_config = observer_config;
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    observer_config.dbname(&db);
    let observer = connect(&observer_config).await;
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one("SELECT id FROM actors WHERE id=$1 FOR UPDATE", &[&actor])
        .await
        .unwrap();
    let store = h.store.clone();
    let callback = tokio::spawn(async move {
        store
            .studio_oidc()
            .unwrap()
            .callback(&store, &s, &b, &c)
            .await
    });
    wait_for_finish_lock(&observer).await;
    tx.execute("UPDATE actors SET active=false WHERE id=$1", &[&actor])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(callback.await.unwrap().err(), Some(AuthError::Inactive));
    assert_eq!(h.count("sessions").await, 2);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn proof_expiry_at_final_actor_lock_refuses_session_and_rotation() {
    let mut h = Harness::new().await;
    let (old, actor) = h.login(None).await;
    let start = h.oidc().begin(&h.store, Some(&old)).await.unwrap();
    let browser = start
        .cookie
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned();
    let expiry = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
        + 2;
    let (state, code) = h.mock.issue(
        &start.url,
        "minh",
        Some(("exp".into(), serde_json::json!(expiry))),
    );
    let mut observer_config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    observer_config.dbname(&db);
    let observer = connect(&observer_config).await;
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one("SELECT id FROM actors WHERE id=$1 FOR UPDATE", &[&actor])
        .await
        .unwrap();
    let store = h.store.clone();
    let callback = tokio::spawn(async move {
        store
            .studio_oidc()
            .unwrap()
            .callback(&store, &state, &browser, &code)
            .await
    });
    wait_for_finish_lock(&observer).await;
    tokio::time::timeout(std::time::Duration::from_secs(4), async {
        loop {
            let expired: bool = observer
                .query_one(
                    "SELECT extract(epoch FROM clock_timestamp()) >= $1::bigint",
                    &[&(expiry as i64)],
                )
                .await
                .unwrap()
                .get(0);
            if expired {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("database clock must pass the signed proof expiry");
    tx.commit().await.unwrap();
    assert_eq!(callback.await.unwrap().err(), Some(AuthError::Inactive));
    assert_eq!(h.count("sessions").await, 1);
    assert_eq!(h.count("actors").await, 1);
    assert_eq!(h.count("studio_oidc_sessions").await, 1);
    assert_eq!(h.store.authenticate(&old).await.unwrap(), actor);
    let completed: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM studio_oidc_flows WHERE completed",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(completed, 1);
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn expiry_while_receipt_insert_waits_rolls_back_every_issuance_effect() {
    for expiry_kind in ["proof", "flow", "previous"] {
        let mut h = Harness::new().await;
        let (old, actor) = h.login(None).await;
        let expiry = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs()
            + 3;
        if expiry_kind == "previous" {
            h.admin
                .execute(
                    "UPDATE sessions SET expires_at=to_timestamp($1::bigint) WHERE token_hash=$2",
                    &[
                        &(expiry as i64),
                        &cantos_api::platform::postgres::token_hash(&old),
                    ],
                )
                .await
                .unwrap();
        }
        let start = h.oidc().begin(&h.store, Some(&old)).await.unwrap();
        let browser = start
            .cookie
            .split(';')
            .next()
            .unwrap()
            .split_once('=')
            .unwrap()
            .1
            .to_owned();
        let changed_claim = if expiry_kind == "proof" {
            Some(("exp".into(), serde_json::json!(expiry)))
        } else {
            None
        };
        let (state, code) = h.mock.issue(&start.url, "minh", changed_claim);
        if expiry_kind == "flow" {
            h.admin.execute("UPDATE studio_oidc_flows SET expires_at=to_timestamp($1::bigint) WHERE state_hash=$2", &[&(expiry as i64), &cantos_api::platform::postgres::token_hash(&state)]).await.unwrap();
        }
        let mut observer_config =
            local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
        let db: String = h
            .admin
            .query_one("SELECT current_database()", &[])
            .await
            .unwrap()
            .get(0);
        observer_config.dbname(&db);
        let observer = connect(&observer_config).await;
        let tx = h.admin.transaction().await.unwrap();
        // This wait occurs AFTER the actor/identity checks and the speculative sessions INSERT.
        // A sessions table lock would block rotation's preliminary previous-session read instead.
        tx.batch_execute("LOCK TABLE studio_oidc_sessions IN ACCESS EXCLUSIVE MODE")
            .await
            .unwrap();
        let store = h.store.clone();
        let callback = tokio::spawn(async move {
            store
                .studio_oidc()
                .unwrap()
                .callback(&store, &state, &browser, &code)
                .await
        });
        wait_for_finish_lock(&observer).await;
        let waiting_on_receipt: bool = observer.query_one("SELECT EXISTS(SELECT 1 FROM pg_locks l JOIN pg_stat_activity a USING(pid) WHERE l.relation='studio_oidc_sessions'::regclass AND NOT l.granted AND a.query LIKE 'SELECT studio_oidc_finish%')", &[]).await.unwrap().get(0);
        assert!(
            waiting_on_receipt,
            "{expiry_kind} must wait at the receipt INSERT, after the first session effect"
        );
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let expired: bool = observer
                    .query_one(
                        "SELECT extract(epoch FROM clock_timestamp()) >= $1::bigint",
                        &[&(expiry as i64)],
                    )
                    .await
                    .unwrap()
                    .get(0);
                if expired {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("database clock must pass the selected expiry while receipt INSERT waits");
        tx.commit().await.unwrap();
        assert_eq!(
            callback.await.unwrap().err(),
            Some(AuthError::Inactive),
            "{expiry_kind} expiry must refuse at the final effect fence"
        );
        assert_eq!(h.count("actors").await, 1, "{expiry_kind}");
        assert_eq!(h.count("sessions").await, 1, "{expiry_kind}");
        assert_eq!(h.count("studio_oidc_sessions").await, 1, "{expiry_kind}");
        let old_facts = h.admin.query_one("SELECT s.revoked,o.revocation_pending FROM sessions s JOIN studio_oidc_sessions o USING(token_hash) WHERE s.token_hash=$1", &[&cantos_api::platform::postgres::token_hash(&old)]).await.unwrap();
        assert!(
            !old_facts.get::<_, bool>(0),
            "{expiry_kind} must not revoke the previous session"
        );
        assert!(
            !old_facts.get::<_, bool>(1),
            "{expiry_kind} must not schedule previous bearer revocation"
        );
        let completed: i64 = h
            .admin
            .query_one(
                "SELECT count(*) FROM studio_oidc_flows WHERE completed",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(
            completed, 1,
            "{expiry_kind} must retain only the original completed flow"
        );
        if expiry_kind != "previous" {
            assert_eq!(h.store.authenticate(&old).await.unwrap(), actor);
        }
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn secure_cookie_namespace_is_required_and_disabled_configuration_rejects_oidc_sessions() {
    let mut h = Harness::new().await;
    let (token, _) = h.login(None).await;
    h.config_for_secure_test();
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "https://studio.example".into(),
        },
        "/nonexistent",
    );
    let wrong = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, format!("cantos_session={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong.status(), StatusCode::UNAUTHORIZED);
    let logout = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/v1/studio/session")
                .header(header::ORIGIN, "https://studio.example")
                .header("x-cantos-studio-stale-only", "1")
                .header(
                    header::COOKIE,
                    "cantos_session=legacy-studio; cantos_session=legacy-alias",
                )
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(logout.status(), StatusCode::NO_CONTENT);
    let retired: Vec<_> = logout
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .map(|v| v.to_str().unwrap())
        .collect();
    assert!(retired
        .contains(&"cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1/studio; Max-Age=0"));
    assert!(
        retired.contains(&"cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1; Max-Age=0")
    );
    assert!(
        retired
            .iter()
            .all(|value| !value.starts_with("__Secure-cantos_")),
        "legacy recovery must never expire a concurrent current Secure cookie"
    );
    // Model the browser applying those deletions, then sending only the current Secure cookie.
    let right = app
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/session")
                .header(header::COOKIE, format!("__Secure-cantos_studio={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(right.status(), StatusCode::OK);
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    config.dbname(&db).user("cantos_app");
    let no_oidc = Store::new(config).unwrap();
    let no_oidc_app = router(
        AppState {
            store: no_oidc.clone(),
            web_origin: "https://studio.example".into(),
        },
        "/nonexistent",
    );
    let configuration = no_oidc_app
        .oneshot(
            Request::builder()
                .uri("/api/v1/studio/auth/configuration")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(configuration.status(), StatusCode::OK);
    let bytes = configuration
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes();
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
        serde_json::json!({"mode":"development"})
    );
    assert!(matches!(
        no_oidc.authenticate(&token).await,
        Err(cantos_api::platform::postgres::StoreError::Unauthenticated)
    ));
    h.admin.execute("UPDATE sessions SET expires_at=clock_timestamp()-interval '1 second' WHERE token_hash=$1",&[&cantos_api::platform::postgres::token_hash(&token)]).await.unwrap();
    assert!(h.store.authenticate(&token).await.is_err());
}

impl Harness {
    fn config_for_secure_test(&mut self) {
        // Same issuer/DB session, different reviewed cookie host policy; no provider mutation.
        let oidc = StudioOidc::new(
            OidcConfig::new(
                &self.mock.issuer(),
                "https://studio.example",
                "fixture".into(),
            )
            .unwrap(),
        )
        .unwrap();
        self.store = self.store.clone().with_studio_oidc(oidc);
    }
}

#[tokio::test]
#[ignore = "requires xtask disposable PostgreSQL"]
async fn independent_logins_and_operator_registration_share_one_atomic_identity() {
    use cantos_api::platform::identity::{
        domain::{ActorId, ExternalIdentityKey},
        IdentityService, PostgresIdentityRegistry,
    };
    use sha2::{Digest, Sha256};
    let mut h = Harness::new().await;
    let (s1, b1, c1) = h.start(None).await;
    let (s2, b2, c2) = h.start(None).await;
    let (one, two) = tokio::join!(
        h.oidc().callback(&h.store, &s1, &b1, &c1),
        h.oidc().callback(&h.store, &s2, &b2, &c2)
    );
    assert_eq!(one.unwrap().actor_id, two.unwrap().actor_id);
    assert_eq!(
        (
            h.count("actors").await,
            h.count("actor_external_identities").await,
            h.count("sessions").await
        ),
        (1, 1, 2)
    );

    // Independent lock oracle for the stated PR1 framing. Hold it until BOTH adapters wait;
    // a mismatched SQL/Rust digest cannot pass by registering before the other call reaches SQL.
    let subject = "operator-race";
    let issuer = h.mock.issuer();
    let mut digest = Sha256::new();
    digest.update(b"cantos-identity-registry-v1");
    for part in [issuer.as_str(), subject] {
        digest.update((part.len() as u64).to_be_bytes());
        digest.update(part.as_bytes());
    }
    let lock = i64::from_be_bytes(digest.finalize()[..8].try_into().unwrap());
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let db: String = h
        .admin
        .query_one("SELECT current_database()", &[])
        .await
        .unwrap()
        .get(0);
    config.dbname(&db);
    let observer = connect(&config).await;
    let service = IdentityService::new(PostgresIdentityRegistry::new(Store::new(config).unwrap()));
    let start = h.oidc().begin(&h.store, None).await.unwrap();
    let browser = start
        .cookie
        .split(';')
        .next()
        .unwrap()
        .split_once('=')
        .unwrap()
        .1
        .to_owned();
    let (state, code) = h.mock.issue(&start.url, subject, None);
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one("SELECT pg_advisory_xact_lock($1)", &[&lock])
        .await
        .unwrap();
    let store = h.store.clone();
    let callback = tokio::spawn(async move {
        store
            .studio_oidc()
            .unwrap()
            .callback(&store, &state, &browser, &code)
            .await
    });
    let key = ExternalIdentityKey::new(&issuer, subject).unwrap();
    let candidate = ActorId::new(uuid::Uuid::new_v4().to_string()).unwrap();
    let operator = tokio::spawn(async move { service.register(&key, &candidate).await });
    tokio::time::timeout(std::time::Duration::from_secs(2),async {
        loop {
            let waiting:i64=observer.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND (query LIKE 'SELECT studio_oidc_finish%' OR query='SELECT pg_advisory_xact_lock($1)')",&[]).await.unwrap().get(0);
            if waiting==2 {break;}
            tokio::task::yield_now().await;
        }
    }).await.expect("both registry adapters must block on the same framed identity lock");
    tx.commit().await.unwrap();
    let session = callback.await.unwrap().unwrap();
    let registration = operator.await.unwrap().unwrap();
    assert_eq!(session.actor_id, registration.actor_id().as_str());
    assert_eq!(
        (
            h.count("actors").await,
            h.count("actor_external_identities").await,
            h.count("sessions").await
        ),
        (2, 2, 3)
    );
}
