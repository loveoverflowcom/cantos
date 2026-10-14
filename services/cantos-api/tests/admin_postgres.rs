//! Actual Admin HTTP/crypto/PG tests; provider is a named signed loopback mock, never Kanidm-live.
#[path = "support/oidc_provider.rs"]
mod mock;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_api::{
    interfaces::http::{router, AppState},
    platform::{
        admin::{AdminConfig, AdminError, AdminService, CLIENT_ID},
        postgres::{local_config, migrate, Store},
    },
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{
    env,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;
async fn connect(config: &Config) -> Client {
    let (c, io) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = io.await;
    });
    c
}
struct Harness {
    db: Client,
    store: Store,
    provider: mock::MockProvider,
    config: Config,
}
impl Harness {
    async fn new() -> Self {
        let mut config = local_config(
            &env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres"),
        )
        .unwrap();
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
        migrate(&config).await.unwrap();
        let db = connect(&config).await;
        db.batch_execute("GRANT EXECUTE ON FUNCTION admin_identity_facts(text,text,text),admin_append_audit(uuid,text,text,text,text,uuid),admin_operation(text,uuid),admin_revoke_access(text,uuid,text,uuid) TO cantos_app;").await.unwrap();
        let provider = mock::MockProvider::start_for(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            CLIENT_ID,
        )
        .await;
        db.execute(
            "INSERT INTO actors(id) VALUES('minh'),('other'),('third')",
            &[],
        )
        .await
        .unwrap();
        for actor in ["minh", "other", "third"] {
            db.execute(
                "INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES($1,$2,$2)",
                &[&provider.issuer(), &actor],
            )
            .await
            .unwrap();
        }
        let mut runtime = config.clone();
        runtime.user("cantos_app");
        let service =
            AdminService::new(AdminConfig::new(&provider.issuer(), "fixture".into()).unwrap())
                .unwrap();
        let store = Store::new(runtime).unwrap().with_admin(service);
        Self {
            db,
            store,
            provider,
            config,
        }
    }
    fn token(&self, actor: &str) -> String {
        self.provider
            .resource(actor, &["cantos_identity_operators"], None)
    }
    async fn count(&self, table: &str) -> i64 {
        self.db
            .query_one(&format!("SELECT count(*) FROM {table}"), &[])
            .await
            .unwrap()
            .get(0)
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        cookie: Option<&str>,
        body: Value,
    ) -> (StatusCode, Value) {
        let app = router(
            AppState {
                store: self.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "no-static-fixture",
        );
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header(header::ORIGIN, "http://127.0.0.1:8080")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(t) = token {
            r = r.header(header::AUTHORIZATION, format!("Bearer {t}"));
        }
        if let Some(c) = cookie {
            r = r.header(header::COOKIE, c);
        }
        let response = app
            .oneshot(r.body(Body::from(body.to_string())).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
}
const CAP: &str = "/api/v1/admin/capabilities";
fn revoke_path(actor: &str) -> String {
    format!("/api/v1/admin/actors/{actor}/revoke-access")
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn admin_capability_is_existing_registry_actor_with_explicit_source_and_audited_role() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let (status, body) = h.request("GET", CAP, Some(&token), None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let schema: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schemas/admin-v1.schema.json"
    ))
    .unwrap();
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(&body)
        .unwrap();
    assert_eq!(body["actor_id"], "minh");
    assert_eq!(body["role"], "identity_operator");
    assert_eq!(
        body["source"],
        "verified_issuer_group_and_current_introspection"
    );
    assert_eq!(h.count("admin_audit").await, 1);
    let row =
        h.db.query_one(
            "SELECT actor_id,surface,action,outcome,correlation_id::text FROM admin_audit",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, String>(1),
            row.get::<_, String>(2),
            row.get::<_, String>(3)
        ),
        (
            "minh".into(),
            "admin".into(),
            "capabilities".into(),
            "allowed".into()
        )
    );
    let dump: String =
        h.db.query_one("SELECT row_to_json(a)::text FROM admin_audit a", &[])
            .await
            .unwrap()
            .get(0);
    assert!(!dump.contains(&token));
    assert!(!dump.contains("fixture"));
    assert!(!dump.contains(&h.provider.issuer()));
    let unknown = h.token("not-registered");
    assert_eq!(
        h.request("GET", CAP, Some(&unknown), None, json!({}))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(h.count("actors").await, 3);
    assert_eq!(h.count("sessions").await, 0);
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn valid_admin_resource_credential_never_authorizes_private_studio_rendition_reads() {
    let h = Harness::new().await;
    // Allow Studio's real empty-session lookup. A missing test-role SQL privilege (503)
    // would not prove that relabeling this valid Admin JWT cannot authenticate to Studio.
    h.db.batch_execute("GRANT SELECT ON sessions TO cantos_app")
        .await
        .unwrap();
    let token = h.token("minh");
    let (status, capability) = h.request("GET", CAP, Some(&token), None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(capability["actor_id"], "minh");
    assert_eq!(capability["role"], "identity_operator");
    assert_eq!(h.count("admin_audit").await, 1);

    // No private media or storage adapter is configured: exact 401 must precede object
    // lookup, rather than passing because a rendition is missing or storage is unavailable.
    const AUTHORITY_AND_MEDIA: &str = "SELECT jsonb_build_object('actors',(SELECT count(*) FROM actors),'bindings',(SELECT count(*) FROM actor_external_identities),'studio_sessions',(SELECT count(*) FROM sessions),'theatre_sessions',(SELECT count(*) FROM theatre_identity_sessions),'native_sessions',(SELECT count(*) FROM theatre_native_sessions),'admin_audit',(SELECT count(*) FROM admin_audit),'admin_operations',(SELECT count(*) FROM admin_operations),'admin_revocations',(SELECT count(*) FROM admin_access_revocations),'script_members',(SELECT count(*) FROM script_members),'producer_grants',(SELECT count(*) FROM production_producer_grants),'intents',(SELECT count(*) FROM audio_upload_intents),'receipts',(SELECT count(*) FROM audio_intake_receipts),'submissions',(SELECT count(*) FROM audio_submissions),'renditions',(SELECT count(*) FROM audio_renditions))::text";
    let before: String =
        h.db.query_one(AUTHORITY_AND_MEDIA, &[])
            .await
            .unwrap()
            .get(0);
    let initial: Value = serde_json::from_str(&before).unwrap();
    for field in [
        "studio_sessions",
        "theatre_sessions",
        "native_sessions",
        "admin_operations",
        "admin_revocations",
        "script_members",
        "producer_grants",
        "intents",
        "receipts",
        "submissions",
        "renditions",
    ] {
        assert_eq!(initial[field], 0, "fixture unexpectedly has {field}");
    }
    let introspections_before = h.provider.data.introspection_calls.load(Ordering::SeqCst);
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "no-static-fixture",
    );
    let studio_cookie = format!("cantos_session={token}");
    let secure_studio_cookie = format!("__Secure-cantos_studio={token}");
    let theatre_cookie = format!("cantos_theatre={token}");
    let id = "00000000-0000-4000-8000-000000000001";
    for suffix in ["bytes", "read-access"] {
        let path = format!(
            "/api/v1/studio/content-units/{id}/audio-submissions/{id}/renditions/{id}/{suffix}"
        );
        for (case, cookie, bearer) in [
            ("Admin bearer", None, Some(token.as_str())),
            ("Studio cookie relabel", Some(studio_cookie.as_str()), None),
            (
                "secure Studio cookie relabel",
                Some(secure_studio_cookie.as_str()),
                None,
            ),
            (
                "Theatre cookie relabel",
                Some(theatre_cookie.as_str()),
                None,
            ),
            (
                "Studio cookie relabel and Admin bearer",
                Some(studio_cookie.as_str()),
                Some(token.as_str()),
            ),
        ] {
            let mut request = Request::builder().uri(&path);
            if let Some(cookie) = cookie {
                request = request.header(header::COOKIE, cookie);
            }
            if let Some(bearer) = bearer {
                request = request.header(header::AUTHORIZATION, format!("Bearer {bearer}"));
            }
            let response = app
                .clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{case} {suffix}"
            );
            assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
            assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
            assert!(!response.headers().contains_key(header::LOCATION));
            assert!(!response.headers().contains_key(header::SET_COOKIE));
            assert!(!response.headers().contains_key("x-cantos-read-expires-at"));
            let body: Value =
                serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                    .unwrap();
            assert_eq!(
                body,
                json!({"code":"unauthenticated","current_revision":null,"issues":[]}),
                "{case} {suffix}"
            );
        }
    }
    let after: String =
        h.db.query_one(AUTHORITY_AND_MEDIA, &[])
            .await
            .unwrap()
            .get(0);
    assert_eq!(
        after, before,
        "private reads changed authority or media state"
    );
    assert_eq!(
        h.provider.data.introspection_calls.load(Ordering::SeqCst),
        introspections_before,
        "Studio reads must not admit through the Admin provider verifier"
    );
    let (status, capability) = h.request("GET", CAP, Some(&token), None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(capability["actor_id"], "minh");
    assert_eq!(capability["role"], "identity_operator");
    assert_eq!(h.count("admin_audit").await, 2);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn credentials_roles_disable_revocation_and_other_surfaces_fail_closed_with_audit() {
    let h = Harness::new().await;
    for cookie in [
        "cantos_session=opaque",
        "cantos_theatre=opaque",
        "__Secure-cantos_studio=opaque",
    ] {
        assert_eq!(
            h.request("GET", CAP, None, Some(cookie), json!({})).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    for opaque in ["producer-scoped-grant", "theatre-session", "studio-session"] {
        assert_eq!(
            h.request("GET", CAP, Some(opaque), None, json!({})).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    for field in ["aud", "iss", "client_id"] {
        let bad = h.provider.resource(
            "minh",
            &["cantos_identity_operators"],
            Some((field, json!("wrong"))),
        );
        assert_eq!(
            h.request("GET", CAP, Some(&bad), None, json!({})).await.0,
            StatusCode::UNAUTHORIZED
        );
    }
    for groups in [&[][..], &["superadmin"][..]] {
        let bad = h.provider.resource("minh", groups, None);
        assert_eq!(
            h.request("GET", CAP, Some(&bad), None, json!({})).await.0,
            StatusCode::FORBIDDEN
        );
    }
    let auditor = h
        .provider
        .resource("minh", &["cantos_system_auditors"], None);
    assert_eq!(
        h.request("GET", CAP, Some(&auditor), None, json!({}))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        h.request(
            "POST",
            &revoke_path("other"),
            Some(&auditor),
            None,
            json!({"operation_id":uuid::Uuid::new_v4().to_string()})
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    let token = h.token("minh");
    h.provider.set_admin_groups(&[]);
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    h.provider.set_admin_groups(&["cantos_identity_operators"]);
    h.provider.data.active.store(false, Ordering::SeqCst);
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::UNAUTHORIZED
    );
    h.provider.data.active.store(true, Ordering::SeqCst);
    h.db.execute("UPDATE actors SET active=false WHERE id='minh'", &[])
        .await
        .unwrap();
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(h.count("admin_access_revocations").await, 0);
    // Dedicated token carries no Studio or Theatre permission, even with correct Admin groups.
    assert_eq!(
        h.request(
            "GET",
            "/api/v1/studio/scripts/00000000-0000-4000-8000-000000000001/head",
            Some(&token),
            None,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        h.request(
            "GET",
            "/api/v1/theatre/library/continue",
            Some(&token),
            None,
            json!({})
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn revocation_and_audit_are_atomic_replayable_and_append_only_under_runtime_role() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let operation = uuid::Uuid::new_v4().to_string();
    let body = json!({"operation_id":operation});
    let one = h
        .request(
            "POST",
            &revoke_path("other"),
            Some(&token),
            None,
            body.clone(),
        )
        .await;
    assert_eq!(one.0, StatusCode::OK);
    let two = h
        .request(
            "POST",
            &revoke_path("other"),
            Some(&token),
            None,
            body.clone(),
        )
        .await;
    assert_eq!(one, two);
    assert_eq!(h.count("admin_operations").await, 1);
    assert_eq!(h.count("admin_access_revocations").await, 1);
    assert_eq!(h.count("admin_audit").await, 2);
    assert_eq!(
        h.request("POST", &revoke_path("third"), Some(&token), None, body)
            .await
            .0,
        StatusCode::CONFLICT
    );
    let target = h.token("other");
    assert_eq!(
        h.request("GET", CAP, Some(&target), None, json!({}))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    // No backend resource right or existing session is modified by this Admin-only operation.
    assert!(h
        .db
        .query_one("SELECT active FROM actors WHERE id='other'", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
    let protected_queries = [
        (
            "admin_audit",
            "SELECT row_to_json(a)::text FROM admin_audit a ORDER BY id",
        ),
        (
            "admin_operations",
            "SELECT row_to_json(o)::text FROM admin_operations o ORDER BY actor_id,operation_id",
        ),
        (
            "admin_access_revocations",
            "SELECT row_to_json(r)::text FROM admin_access_revocations r ORDER BY actor_id",
        ),
    ];
    let mut protected_before = Vec::new();
    for &(_, query) in &protected_queries {
        protected_before.push(
            h.db.query(query, &[])
                .await
                .unwrap()
                .iter()
                .map(|row| row.get::<_, String>(0))
                .collect::<Vec<_>>(),
        );
    }
    // Include the receipt child of the audit FK: audit-only TRUNCATE refuses earlier with
    // 0A000 and never exercises admin_immutable's P0001 append-only trigger.
    for sql in [
        "UPDATE admin_audit SET outcome='denied'",
        "DELETE FROM admin_audit",
        "TRUNCATE admin_audit,admin_operations",
        "UPDATE admin_operations SET target_actor_id='third'",
        "DELETE FROM admin_operations",
        "TRUNCATE admin_operations",
        "UPDATE admin_access_revocations SET revoked_at=clock_timestamp()",
        "DELETE FROM admin_access_revocations",
        "TRUNCATE admin_access_revocations",
    ] {
        let error = h.db.batch_execute(sql).await.unwrap_err();
        assert_eq!(
            error.code().map(|code| code.code()),
            Some("P0001"),
            "admin_immutable must reject the statement itself: {sql}: {error}",
        );
        for ((table, query), expected) in protected_queries.iter().zip(&protected_before) {
            let after =
                h.db.query(*query, &[])
                    .await
                    .unwrap()
                    .iter()
                    .map(|row| row.get::<_, String>(0))
                    .collect::<Vec<_>>();
            assert_eq!(&after, expected, "{table} changed after refusing {sql}");
        }
    }
    let mut runtime = h.config.clone();
    runtime.user("cantos_app");
    let app = connect(&runtime).await;
    for sql in [
        "DELETE FROM admin_audit",
        "SELECT * FROM admin_audit",
        "INSERT INTO admin_access_revocations(actor_id) VALUES('third')",
    ] {
        assert_eq!(
            app.batch_execute(sql).await.unwrap_err().code(),
            Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE)
        );
    }
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn required_audit_failure_and_effect_failure_never_commit_partial_success() {
    let h = Harness::new().await;
    let token = h.token("minh");
    h.db.batch_execute("CREATE FUNCTION fixture_audit_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected'; END; $$; CREATE TRIGGER fixture_audit_fail BEFORE INSERT ON admin_audit FOR EACH STATEMENT EXECUTE FUNCTION fixture_audit_fail();").await.unwrap();
    assert_eq!(
        h.request(
            "POST",
            &revoke_path("other"),
            Some(&token),
            None,
            json!({"operation_id":uuid::Uuid::new_v4().to_string()})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        h.request("GET", CAP, None, None, json!({})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(h.count("admin_operations").await, 0);
    assert_eq!(h.count("admin_audit").await, 0);
    h.db.batch_execute("DROP TRIGGER fixture_audit_fail ON admin_audit; CREATE TRIGGER fixture_effect_fail BEFORE INSERT ON admin_operations FOR EACH STATEMENT EXECUTE FUNCTION fixture_audit_fail();").await.unwrap();
    assert_eq!(
        h.request(
            "POST",
            &revoke_path("other"),
            Some(&token),
            None,
            json!({"operation_id":uuid::Uuid::new_v4().to_string()})
        )
        .await
        .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(h.count("admin_operations").await, 0);
    assert_eq!(h.count("admin_audit").await, 1);
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn concurrent_duplicate_delivery_recovers_one_receipt_after_application_restart() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let op = uuid::Uuid::new_v4().to_string();
    let service = h.store.admin().unwrap();
    let (a, b) = tokio::join!(
        service.revoke_access(&h.store, Some(&token), "other", &op, uuid::Uuid::new_v4()),
        service.revoke_access(&h.store, Some(&token), "other", &op, uuid::Uuid::new_v4())
    );
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(h.count("admin_operations").await, 1);
    assert_eq!(h.count("admin_audit").await, 2);
    let mut config = h.config.clone();
    config.user("cantos_app");
    let restarted = Store::new(config).unwrap().with_admin(
        AdminService::new(AdminConfig::new(&h.provider.issuer(), "fixture".into()).unwrap())
            .unwrap(),
    );
    let receipt = restarted
        .admin()
        .unwrap()
        .revoke_access(&restarted, Some(&token), "other", &op, uuid::Uuid::new_v4())
        .await
        .unwrap();
    assert_eq!(receipt.operation_id, op);
    assert_eq!(h.count("admin_operations").await, 1);
    assert_eq!(h.count("admin_audit").await, 3);
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn local_admin_revocation_committed_while_waiting_for_lock_is_rechecked() {
    let mut h = Harness::new().await;
    let token = h.token("minh");
    let observer = connect(&h.config).await;
    let tx = h.db.transaction().await.unwrap();
    tx.execute("UPDATE actors SET active=active WHERE id='minh'", &[])
        .await
        .unwrap();
    let store = h.store.clone();
    let task = tokio::spawn(async move {
        store
            .admin()
            .unwrap()
            .capabilities(&store, Some(&token), uuid::Uuid::new_v4())
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(2),async {loop {let waiting:i64=observer.query_one("SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT * FROM admin_identity_facts%'",&[]).await.unwrap().get(0);if waiting==1 {break;} tokio::task::yield_now().await;}}).await.unwrap();
    tx.execute(
        "INSERT INTO admin_access_revocations(actor_id) VALUES('minh')",
        &[],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(task.await.unwrap().err(), Some(AdminError::Revoked));
    assert_eq!(h.count("admin_operations").await, 0);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn actor_disable_committed_while_waiting_for_lock_is_rechecked_and_audited() {
    let mut h = Harness::new().await;
    let token = h.token("minh");
    let observer = connect(&h.config).await;
    let tx = h.db.transaction().await.unwrap();
    tx.execute("UPDATE actors SET active=active WHERE id='minh'", &[])
        .await
        .unwrap();
    let store = h.store.clone();
    let operation = uuid::Uuid::new_v4().to_string();
    let task = tokio::spawn(async move {
        store
            .admin()
            .unwrap()
            .revoke_access(
                &store,
                Some(&token),
                "other",
                &operation,
                uuid::Uuid::new_v4(),
            )
            .await
    });
    // Observe the production admission query actually waiting before disabling the actor.
    // This checks fresh SQL facts after the wait, rather than only an already-disabled fixture.
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let waiting: i64 = observer
                .query_one(
                    "SELECT count(*) FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query LIKE 'SELECT * FROM admin_identity_facts%'",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            if waiting == 1 {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    tx.execute("UPDATE actors SET active=false WHERE id='minh'", &[])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(task.await.unwrap().err(), Some(AdminError::InactiveActor));
    assert_eq!(h.count("admin_operations").await, 0);
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(h.count("admin_audit").await, 1);
    let row =
        h.db.query_one(
            "SELECT actor_id,action,target_actor_id,outcome FROM admin_audit",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "minh");
    assert_eq!(row.get::<_, String>(1), "revoke_admin_access");
    assert_eq!(row.get::<_, String>(2), "other");
    assert_eq!(row.get::<_, String>(3), "denied");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn receipt_read_failure_after_verified_admission_is_audited_without_an_effect() {
    let h = Harness::new().await;
    let token = h.token("minh");
    h.db.batch_execute("REVOKE EXECUTE ON FUNCTION admin_operation(text,uuid) FROM cantos_app")
        .await
        .unwrap();
    let result = h
        .store
        .admin()
        .unwrap()
        .revoke_access(
            &h.store,
            Some(&token),
            "other",
            &uuid::Uuid::new_v4().to_string(),
            uuid::Uuid::new_v4(),
        )
        .await;
    assert_eq!(result.err(), Some(AdminError::Unavailable));
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(h.count("admin_operations").await, 0);
    let row =
        h.db.query_one("SELECT actor_id,outcome FROM admin_audit", &[])
            .await
            .unwrap();
    assert_eq!(row.get::<_, String>(0), "minh");
    assert_eq!(row.get::<_, String>(1), "unavailable");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn proof_expiring_during_required_audit_wait_rolls_back_effect_and_records_denial() {
    let mut h = Harness::new().await;
    let time: i64 =
        h.db.query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    let expires = time + 2;
    let token = h.provider.resource(
        "minh",
        &["cantos_identity_operators"],
        Some(("exp", json!(expires))),
    );
    let observer = connect(&h.config).await;
    let tx = h.db.transaction().await.unwrap();
    tx.batch_execute("LOCK TABLE admin_audit IN ACCESS EXCLUSIVE MODE")
        .await
        .unwrap();
    let store = h.store.clone();
    let op = uuid::Uuid::new_v4().to_string();
    let task = tokio::spawn(async move {
        store
            .admin()
            .unwrap()
            .revoke_access(&store, Some(&token), "other", &op, uuid::Uuid::new_v4())
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(3),async {loop {
        let row=observer.query_one("SELECT count(*) FILTER (WHERE wait_event_type='Lock' AND query LIKE 'SELECT admin_append_audit%'),floor(extract(epoch FROM clock_timestamp()))::bigint FROM pg_stat_activity WHERE datname=current_database()",&[]).await.unwrap();
        if row.get::<_,i64>(0)==1 && row.get::<_,i64>(1)>=expires {break;}
        tokio::task::yield_now().await;
    }}).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(task.await.unwrap().err(), Some(AdminError::StaleRole));
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(h.count("admin_operations").await, 0);
    let row =
        h.db.query_one("SELECT actor_id,outcome FROM admin_audit", &[])
            .await
            .unwrap();
    assert_eq!(row.get::<_, String>(0), "minh");
    assert_eq!(row.get::<_, String>(1), "denied");
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, synthetic point-lookup baseline"]
async fn bounded_admin_point_lookup_baseline_uses_current_runtime_role_and_primary_keys() {
    let h = Harness::new().await;
    h.db.execute("INSERT INTO actor_external_identities(issuer,subject,actor_id) SELECT $1,'synthetic-'||v,'minh' FROM generate_series(1,2000) v",&[&h.provider.issuer()]).await.unwrap();
    let mut config = h.config.clone();
    config.user("cantos_app");
    let mut runtime = connect(&config).await;
    let start = std::time::Instant::now();
    for _ in 0..20 {
        let tx = runtime.transaction().await.unwrap();
        assert_eq!(
            tx.query_one(
                "SELECT actor_id FROM admin_identity_facts($1,'synthetic-1900',NULL)",
                &[&h.provider.issuer()]
            )
            .await
            .unwrap()
            .get::<_, String>(0),
            "minh"
        );
        tx.commit().await.unwrap();
    }
    println!(
        "Admin baseline:2000 exact bindings,20 warm runtime-role point reads,total_ms={}",
        start.elapsed().as_millis()
    );
    let tx = runtime.transaction().await.unwrap();
    for row in tx
        .query(
            "EXPLAIN(ANALYZE,BUFFERS) SELECT * FROM admin_identity_facts($1,'synthetic-1900',NULL)",
            &[&h.provider.issuer()],
        )
        .await
        .unwrap()
    {
        println!("{}", row.get::<_, String>(0));
    }
    tx.commit().await.unwrap();
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn final_clock_read_failure_rolls_back_success_and_audits_unavailable_with_known_actor() {
    let h = Harness::new().await;
    let token = h.token("minh");
    h.db.batch_execute(
        "REVOKE EXECUTE ON FUNCTION pg_catalog.clock_timestamp() FROM PUBLIC,cantos_app",
    )
    .await
    .unwrap();
    assert_eq!(
        h.store
            .admin()
            .unwrap()
            .revoke_access(
                &h.store,
                Some(&token),
                "other",
                &uuid::Uuid::new_v4().to_string(),
                uuid::Uuid::new_v4()
            )
            .await
            .err(),
        Some(AdminError::Unavailable)
    );
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(h.count("admin_operations").await, 0);
    let row =
        h.db.query_one("SELECT actor_id,outcome FROM admin_audit", &[])
            .await
            .unwrap();
    assert_eq!(row.get::<_, String>(0), "minh");
    assert_eq!(row.get::<_, String>(1), "unavailable");
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, retained synthetic restart fixture"]
async fn admin_restart_fixture_preserves_block_receipt_and_audit_without_credentials() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let op = uuid::Uuid::new_v4().to_string();
    let receipt = h
        .store
        .admin()
        .unwrap()
        .revoke_access(&h.store, Some(&token), "other", &op, uuid::Uuid::new_v4())
        .await
        .unwrap();
    let directory =
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../target/revision-evidence");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("admin-restart.json"),
        serde_json::to_vec(&json!({"database":h.config.get_dbname().unwrap(),"receipt":receipt}))
            .unwrap(),
    )
    .unwrap();
}
#[tokio::test]
#[ignore = "xtask runs this only after actual disposable PostgreSQL process restart"]
async fn admin_recovery_after_database_restart() {
    let fixture: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../target/revision-evidence/admin-restart.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    assert_eq!(config.get_user(), Some("cantos_test_admin"));
    let database = fixture["database"].as_str().unwrap();
    assert!(
        database.starts_with("cantos_test_")
            && database
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b == b'_')
    );
    config.dbname(database);
    let db = connect(&config).await;
    migrate(&config).await.unwrap();
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM admin_access_revocations WHERE actor_id='other'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.query_one("SELECT count(*) FROM admin_operations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.query_one("SELECT count(*) FROM admin_audit", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let provider = mock::MockProvider::start_for(
        SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_secs(),
        CLIENT_ID,
    )
    .await;
    // Test-operator explicit binding to the same synthetic actor; no credential or provider
    // process is persisted across the gate's restart, and no auto-link is added to production.
    db.execute(
        "INSERT INTO actor_external_identities(issuer,subject,actor_id) VALUES($1,'minh','minh')",
        &[&provider.issuer()],
    )
    .await
    .unwrap();
    config.user("cantos_app");
    let store = Store::new(config).unwrap().with_admin(
        AdminService::new(AdminConfig::new(&provider.issuer(), "fixture".into()).unwrap()).unwrap(),
    );
    let token = provider.resource("minh", &["cantos_identity_operators"], None);
    let receipt = store
        .admin()
        .unwrap()
        .revoke_access(
            &store,
            Some(&token),
            "other",
            fixture["receipt"]["operation_id"].as_str().unwrap(),
            uuid::Uuid::new_v4(),
        )
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(receipt).unwrap(), fixture["receipt"]);
    assert_eq!(
        db.query_one("SELECT count(*) FROM admin_operations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        db.query_one(
            "SELECT count(*) FROM admin_audit WHERE outcome='replayed'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        1
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL, actual signed mock HTTP provider"]
async fn jwks_and_current_introspection_outages_fail_closed_with_unavailable_audit() {
    let mut h = Harness::new().await;
    let token = h.token("minh");
    h.provider.data.bad_jwks.store(true, Ordering::SeqCst);
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    h.provider.data.bad_jwks.store(false, Ordering::SeqCst);
    h.provider
        .data
        .introspection_fail
        .store(true, Ordering::SeqCst);
    // A fresh service avoids waiting for the intentional failed-metadata retry limiter.
    h.store = h.store.clone().with_admin(
        AdminService::new(AdminConfig::new(&h.provider.issuer(), "fixture".into()).unwrap())
            .unwrap(),
    );
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(h.count("admin_operations").await, 0);
    assert_eq!(h.count("admin_access_revocations").await, 0);
    assert_eq!(
        h.db.query_one(
            "SELECT count(*) FROM admin_audit WHERE actor_id IS NULL AND outcome='unavailable'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, i64>(0),
        2
    );
}
