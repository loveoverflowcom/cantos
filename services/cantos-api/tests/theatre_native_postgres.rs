//! Disposable PostgreSQL + actual HTTP handlers + ephemeral signed public-client mock.
#[path = "support/oidc_provider.rs"]
mod mock;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_api::{
    adapters::inbound::http::{router, AppState},
    platform::{
        postgres::{local_config, migrate, Store},
        theatre_identity::{AuthError, NativeIdentity},
    },
};
use cantos_contracts::theatre_identity::{
    NativeExchange, NativeExchangeRequest, NativeFlowRequest, NativeSession,
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
    provider: mock::MockProvider,
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
        let admin = connect(&config).await;
        admin.batch_execute("GRANT SELECT ON actors,theatre_native_sessions,listener_state,listener_saved,listener_history,listener_progress,listener_operations,listener_playback_sessions,listener_device_sequences TO cantos_app; GRANT UPDATE(revoked,revocation_pending) ON theatre_native_sessions TO cantos_app; GRANT SELECT,INSERT,UPDATE ON theatre_native_flows TO cantos_app; GRANT EXECUTE ON FUNCTION theatre_native_finish(uuid,text,text,text,bytea,bytea,text,bigint,uuid),listener_lock_actor(text),listener_actor_active(text) TO cantos_app;").await.unwrap();
        config.user("cantos_app");
        let provider = mock::MockProvider::start_for(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            "cantos-theatre-native",
        )
        .await;
        let native = NativeIdentity::new(&provider.issuer()).unwrap();
        let store = Store::new(config).unwrap().with_theatre_native(native);
        Self {
            admin,
            store,
            provider,
        }
    }
    fn native(&self) -> &NativeIdentity {
        self.store.theatre_native().unwrap()
    }
    async fn flow(&self, subject: &str) -> (String, NativeExchangeRequest) {
        let state = uuid::Uuid::new_v4().simple().to_string().repeat(2);
        let nonce = "n".repeat(43);
        let verifier = "v".repeat(43);
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        use sha2::{Digest, Sha256};
        let flow = self
            .native()
            .begin(
                &self.store,
                NativeFlowRequest {
                    state: state.clone(),
                    nonce,
                    code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
                },
            )
            .await
            .unwrap();
        let (actual, code) = self.provider.issue(&flow.authorization_url, subject, None);
        assert_eq!(actual, state);
        (
            flow.flow_id,
            NativeExchangeRequest {
                state,
                code,
                code_verifier: verifier,
            },
        )
    }
    async fn login(&self, subject: &str) -> NativeExchange {
        let (flow, body) = self.flow(subject).await;
        self.native()
            .exchange(&self.store, &flow, body)
            .await
            .unwrap()
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        cookie: Option<&str>,
        expected: Option<&str>,
        body: Option<serde_json::Value>,
    ) -> axum::response::Response {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(v) = token {
            request = request.header(header::AUTHORIZATION, format!("Bearer {v}"));
        }
        if let Some(v) = cookie {
            request = request.header(header::COOKIE, v);
        }
        if let Some(v) = expected {
            request = request.header("x-cantos-theatre-session", v);
        }
        let body = if let Some(body) = body {
            request = request.header(header::CONTENT_TYPE, "application/json");
            Body::from(serde_json::to_vec(&body).unwrap())
        } else {
            Body::empty()
        };
        router(
            AppState {
                store: self.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "/nonexistent",
        )
        .oneshot(request.body(body).unwrap())
        .await
        .unwrap()
    }
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn native_public_exchange_and_resource_seam_verify_real_crypto_without_a_client_secret() {
    let h = Harness::new().await;
    use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
    use sha2::{Digest, Sha256};
    let verifier = "v".repeat(43);
    let state = "s".repeat(43);
    let response=h.request("POST","/api/v1/theatre/native/auth/flows",None,None,None,Some(serde_json::json!({"state":state,"nonce":"n".repeat(43),"code_challenge":URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes()))}))).await;
    assert_eq!(response.status(), StatusCode::OK);
    let flow: cantos_contracts::theatre_identity::NativeFlow =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let (actual, code) = h
        .provider
        .issue(&flow.authorization_url, "nguyễn-minh", None);
    assert_eq!(actual, state);
    let response = h
        .request(
            "POST",
            &format!(
                "/api/v1/theatre/native/auth/flows/{}/exchange",
                flow.flow_id
            ),
            None,
            None,
            None,
            Some(serde_json::json!({"state":state,"code":code,"code_verifier":verifier})),
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let login: NativeExchange =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(login.access_token.len(), 64);
    assert!(!format!("{login:?}").contains(&login.access_token));
    let response = h
        .request(
            "GET",
            "/api/v1/theatre/native/session",
            Some(&login.access_token),
            None,
            None,
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let view: NativeSession =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(view.actor_id, login.actor_id);
    assert_eq!(view.session_id, login.session_id);
    let path = "/api/v1/theatre/progress/00000000-0000-4000-8000-000000000001";
    let response = h
        .request(
            "GET",
            path,
            Some(&login.access_token),
            None,
            Some(&login.session_id),
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()["x-cantos-theatre-session"],
        login.session_id
    );
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM theatre_native_sessions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM sessions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    for (path, cookie, token, status) in [
        (
            "/api/v1/theatre/native/session",
            Some(format!("cantos_theatre={}", login.access_token)),
            None,
            StatusCode::UNAUTHORIZED,
        ),
        (
            "/api/v1/theatre/session",
            None,
            Some(login.access_token.clone()),
            StatusCode::UNAUTHORIZED,
        ),
        (
            "/api/v1/studio/session",
            None,
            Some(login.access_token.clone()),
            StatusCode::UNAUTHORIZED,
        ),
    ] {
        assert_eq!(
            h.request("GET", path, token.as_deref(), cookie.as_deref(), None, None)
                .await
                .status(),
            status
        );
    }
    h.provider.data.active.store(false, Ordering::SeqCst);
    assert_eq!(
        h.request("GET", path, Some(&login.access_token), None, None, None)
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn issued_native_opaque_credential_never_authorizes_private_studio_rendition_reads() {
    let h = Harness::new().await;
    // Enable the ordinary Studio session lookup without issuing any Studio session: a denied
    // relabeled cookie must be 401, not a missing SQL privilege reported as unavailable.
    h.admin
        .batch_execute("GRANT SELECT ON sessions TO cantos_app")
        .await
        .unwrap();
    let login = h.login("nguyễn-minh-native-private-denial").await;
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 1);
    let response = h
        .request(
            "GET",
            "/api/v1/theatre/native/session",
            Some(&login.access_token),
            None,
            None,
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::OK);
    let session: NativeSession =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(session.actor_id, login.actor_id);
    assert_eq!(session.session_id, login.session_id);

    // These empty media tables make admission itself the oracle: no missing object or storage
    // configuration may mask an incorrectly admitted cross-surface credential as a 404/503.
    const AUTHORITY_AND_MEDIA: &str = "SELECT jsonb_build_object('actors',(SELECT count(*) FROM actors),'bindings',(SELECT count(*) FROM actor_external_identities),'native_sessions',(SELECT count(*) FROM theatre_native_sessions),'native_revoked',(SELECT count(*) FROM theatre_native_sessions WHERE revoked),'studio_sessions',(SELECT count(*) FROM sessions),'script_members',(SELECT count(*) FROM script_members),'producer_grants',(SELECT count(*) FROM production_producer_grants),'intents',(SELECT count(*) FROM audio_upload_intents),'receipts',(SELECT count(*) FROM audio_intake_receipts),'submissions',(SELECT count(*) FROM audio_submissions),'renditions',(SELECT count(*) FROM audio_renditions))::text";
    let before: String = h
        .admin
        .query_one(AUTHORITY_AND_MEDIA, &[])
        .await
        .unwrap()
        .get(0);
    let initial: serde_json::Value = serde_json::from_str(&before).unwrap();
    let introspections_before = h.provider.data.introspection_calls.load(Ordering::SeqCst);
    assert_eq!(initial["native_sessions"], 1);
    assert_eq!(initial["native_revoked"], 0);
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
    let studio_cookie = format!("cantos_session={}", login.access_token);
    let secure_studio_cookie = format!("__Secure-cantos_studio={}", login.access_token);
    let theatre_cookie = format!("cantos_theatre={}", login.access_token);
    let id = "00000000-0000-4000-8000-000000000001";
    for suffix in ["bytes", "read-access"] {
        let path = format!(
            "/api/v1/studio/content-units/{id}/audio-submissions/{id}/renditions/{id}/{suffix}"
        );
        for (case, bearer, cookie) in [
            ("native bearer", Some(login.access_token.as_str()), None),
            ("Studio cookie relabel", None, Some(studio_cookie.as_str())),
            (
                "secure Studio cookie relabel",
                None,
                Some(secure_studio_cookie.as_str()),
            ),
            (
                "Theatre cookie relabel",
                None,
                Some(theatre_cookie.as_str()),
            ),
            (
                "bearer and Studio cookie relabel",
                Some(login.access_token.as_str()),
                Some(studio_cookie.as_str()),
            ),
        ] {
            let response = h.request("GET", &path, bearer, cookie, None, None).await;
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
        h.provider.data.introspection_calls.load(Ordering::SeqCst),
        introspections_before,
        "Studio private reads must not admit through native provider authentication"
    );
    assert_eq!(
        h.request(
            "GET",
            "/api/v1/theatre/native/session",
            Some(&login.access_token),
            None,
            None,
            None,
        )
        .await
        .status(),
        StatusCode::OK,
        "the actual native credential remains valid for its own surface"
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn native_pkce_state_replay_and_remote_unknown_outcomes_never_redispatch_a_code() {
    let h = Harness::new().await;
    let (flow, body) = h.flow("A").await;
    let wrong = NativeExchangeRequest {
        state: body.state.clone(),
        code: body.code.clone(),
        code_verifier: "w".repeat(43),
    };
    assert_eq!(
        h.native().exchange(&h.store, &flow, wrong).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 0);
    let duplicate = NativeExchangeRequest {
        state: body.state.clone(),
        code: body.code.clone(),
        code_verifier: body.code_verifier.clone(),
    };
    let (a, b) = tokio::join!(
        h.native().exchange(&h.store, &flow, body),
        h.native().exchange(&h.store, &flow, duplicate)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 1);
    h.admin
        .batch_execute("DELETE FROM theatre_native_flows")
        .await
        .unwrap();
    let (flow, body) = h.flow("B").await;
    h.provider.data.token_fail.store(true, Ordering::SeqCst);
    let replay = NativeExchangeRequest {
        state: body.state.clone(),
        code: body.code.clone(),
        code_verifier: body.code_verifier.clone(),
    };
    assert_eq!(
        h.native().exchange(&h.store, &flow, body).await.err(),
        Some(AuthError::ProviderUnavailable)
    );
    assert_eq!(
        h.native().exchange(&h.store, &flow, replay).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn native_local_logout_survives_provider_outage_and_actor_disable_refuses_a_fresh_resource() {
    let h = Harness::new().await;
    let login = h.login("A").await;
    h.provider.data.revoke_fail.store(true, Ordering::SeqCst);
    let response = h
        .request(
            "DELETE",
            "/api/v1/theatre/native/session",
            Some(&login.access_token),
            None,
            None,
            None,
        )
        .await;
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(response.headers()["cantos-provider-revocation"], "pending");
    assert_eq!(
        h.request(
            "GET",
            "/api/v1/theatre/native/session",
            Some(&login.access_token),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    h.admin
        .batch_execute("DELETE FROM theatre_native_flows")
        .await
        .unwrap();
    let login = h.login("B").await;
    h.admin
        .execute(
            "UPDATE actors SET active=false WHERE id=$1",
            &[&login.actor_id],
        )
        .await
        .unwrap();
    assert_eq!(
        h.request(
            "GET",
            "/api/v1/theatre/native/session",
            Some(&login.access_token),
            None,
            None,
            None
        )
        .await
        .status(),
        StatusCode::UNAUTHORIZED
    );
    let pending: bool = h
        .admin
        .query_one(
            "SELECT revocation_pending FROM theatre_native_sessions WHERE actor_id<>$1",
            &[&login.actor_id],
        )
        .await
        .unwrap()
        .get(0);
    assert!(pending);
}
#[tokio::test]
#[ignore = "xtask disposable PostgreSQL"]
async fn stale_native_session_fence_refuses_mutation_before_any_library_state_is_written() {
    let h = Harness::new().await;
    let login = h.login("B").await;
    let body =
        serde_json::json!({"operation_id":uuid::Uuid::new_v4().to_string(),"history_epoch":0});
    let response = h
        .request(
            "POST",
            "/api/v1/theatre/library/history/clear",
            Some(&login.access_token),
            None,
            Some("00000000-0000-4000-8000-000000000001"),
            Some(body),
        )
        .await;
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(
        response.headers()["x-cantos-theatre-session"],
        login.session_id
    );
    for table in ["listener_state", "listener_operations"] {
        assert_eq!(
            h.admin
                .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
    }
    let calls = h.provider.data.introspection_calls.load(Ordering::SeqCst);
    for (token, cookie) in [
        (None, None),
        (None, Some("")),
        (None, Some("cantos_session=studio-is-not-native")),
        (None, Some("cantos_theatre=not-a-web-credential")),
        (Some(""), None),
        (Some("malformed-native-credential"), None),
    ] {
        let response = h
            .request(
                "GET",
                "/api/v1/theatre/progress/00000000-0000-4000-8000-000000000001",
                token,
                cookie,
                Some(&login.session_id),
                None,
            )
            .await;
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let value: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(value["code"], "unauthenticated");
    }
    assert_eq!(
        h.provider.data.introspection_calls.load(Ordering::SeqCst),
        calls
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; native admission actor-lock waits"]
async fn native_admission_rechecks_disable_and_expiry_after_the_actor_lock_wait() {
    for disable in [true, false] {
        let h = Harness::new().await;
        let login = h.login("native-lock-wait").await;
        if !disable {
            h.admin
                .execute(
                    "UPDATE theatre_native_sessions SET expires_at=clock_timestamp()+interval '1 second' WHERE token_hash=$1",
                    &[&cantos_api::platform::postgres::token_hash(&login.access_token)],
                )
                .await
                .unwrap();
        }
        h.admin.batch_execute("BEGIN").await.unwrap();
        if disable {
            h.admin
                .execute(
                    "UPDATE actors SET active=false WHERE id=$1",
                    &[&login.actor_id],
                )
                .await
                .unwrap();
        } else {
            h.admin
                .query_one(
                    "SELECT id FROM actors WHERE id=$1 FOR UPDATE",
                    &[&login.actor_id],
                )
                .await
                .unwrap();
        }

        let app = router(
            AppState {
                store: h.store.clone(),
                web_origin: "http://127.0.0.1:8080".into(),
            },
            "/nonexistent",
        );
        let body =
            serde_json::json!({"operation_id":uuid::Uuid::new_v4().to_string(),"history_epoch":0});
        let request = Request::builder()
            .method("POST")
            .uri("/api/v1/theatre/library/history/clear")
            .header(
                header::AUTHORIZATION,
                format!("Bearer {}", login.access_token),
            )
            .header(header::CONTENT_TYPE, "application/json")
            .header("x-cantos-theatre-session", login.session_id.as_str())
            .body(Body::from(serde_json::to_vec(&body).unwrap()))
            .unwrap();
        let operation = tokio::spawn(async move { app.oneshot(request).await.unwrap() });
        tokio::time::timeout(std::time::Duration::from_secs(3), async {
            loop {
                // The fixture holds one transaction; do not reuse its first activity snapshot.
                h.admin
                    .query_one("SELECT pg_stat_clear_snapshot()", &[])
                    .await
                    .unwrap();
                let waiting: bool = h
                    .admin
                    .query_one(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event_type='Lock' AND query='SELECT listener_lock_actor($1)')",
                        &[],
                    )
                    .await
                    .unwrap()
                    .get(0);
                if waiting {
                    break;
                }
                tokio::task::yield_now().await;
            }
        })
        .await
        .expect("the actual native HTTP admission must reach its actor lock");
        if !disable {
            // PostgreSQL, rather than an approximate client sleep, advances the lock wait
            // beyond the exact local session deadline observed before authentication.
            h.admin
                .query_one("SELECT pg_sleep(1.1)", &[])
                .await
                .unwrap();
        }
        h.admin.batch_execute("COMMIT").await.unwrap();
        let response = tokio::time::timeout(std::time::Duration::from_secs(3), operation)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        let error: serde_json::Value =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(error["code"], "unauthenticated");
        for table in ["listener_state", "listener_operations"] {
            assert_eq!(
                h.admin
                    .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                    .await
                    .unwrap()
                    .get::<_, i64>(0),
                0,
                "refused native admission must not dispatch {table} writes"
            );
        }
    }
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; native issuance expires during a session insert wait"]
async fn native_issuance_expiry_rolls_back_registry_and_session_after_an_insert_trigger_wait() {
    let h = Harness::new().await;
    let (flow, body) = h.flow("native-expired-new-actor").await;
    let replay = NativeExchangeRequest {
        state: body.state.clone(),
        code: body.code.clone(),
        code_verifier: body.code_verifier.clone(),
    };
    h.admin
        .batch_execute(
            "CREATE FUNCTION fixture_pause_native_issue() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_sleep(1.1); RETURN NEW; END $$; CREATE TRIGGER fixture_pause_native_issue AFTER INSERT ON theatre_native_sessions FOR EACH ROW EXECUTE FUNCTION fixture_pause_native_issue();",
        )
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE theatre_native_flows SET expires_at=clock_timestamp()+interval '1 second' WHERE flow_id=$1::text::uuid",
            &[&flow],
        )
        .await
        .unwrap();
    assert_eq!(
        h.native().exchange(&h.store, &flow, body).await.err(),
        Some(AuthError::Expired)
    );
    for table in [
        "actors",
        "actor_external_identities",
        "theatre_native_sessions",
    ] {
        assert_eq!(
            h.admin
                .query_one(&format!("SELECT count(*) FROM {table}"), &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0,
            "late expiry must roll back every issuance write to {table}"
        );
    }
    let durable = h
        .admin
        .query_one(
            "SELECT consumed,completed FROM theatre_native_flows WHERE flow_id=$1::text::uuid",
            &[&flow],
        )
        .await
        .unwrap();
    assert!(durable.get::<_, bool>(0));
    assert!(!durable.get::<_, bool>(1));
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        h.native().exchange(&h.store, &flow, replay).await.err(),
        Some(AuthError::State)
    );
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 1);
}
