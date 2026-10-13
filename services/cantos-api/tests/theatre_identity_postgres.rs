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
        admin.batch_execute("GRANT SELECT ON actors,theatre_identity_sessions,listener_state,listener_saved,listener_history,listener_progress,listener_operations,listener_playback_sessions,listener_device_sequences TO cantos_app; GRANT UPDATE(revoked,revocation_pending) ON theatre_identity_sessions TO cantos_app; GRANT SELECT,INSERT,UPDATE ON theatre_identity_flows TO cantos_app; GRANT EXECUTE ON FUNCTION theatre_identity_finish(bytea,text,text,text,text,bytea,bytea,text,bigint),listener_lock_actor(text),listener_actor_active(text) TO cantos_app;").await.unwrap();
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
