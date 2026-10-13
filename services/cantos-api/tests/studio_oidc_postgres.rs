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
        admin.batch_execute("GRANT SELECT ON actors,sessions,studio_oidc_sessions TO cantos_app; GRANT UPDATE ON sessions TO cantos_app; GRANT SELECT,INSERT,UPDATE ON studio_oidc_flows TO cantos_app; GRANT UPDATE(revocation_pending) ON studio_oidc_sessions TO cantos_app; GRANT EXECUTE ON FUNCTION studio_oidc_finish(bytea,text,text,text,text,bytea,bytea,text,bigint),production_lock_actor(text) TO cantos_app;").await.unwrap();
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
    assert!(cookies[1].contains("Max-Age=0"));
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
