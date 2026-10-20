//! Real disposable PostgreSQL + production API + ephemeral signed OIDC HTTP mock.
#[path = "support/oidc_provider.rs"]
mod mock;
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_contracts::bearer::{
    CancelRequest, ExchangeRequest, Flow, FlowRequest, RefreshRequest, TokenPair,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::{
        bearer::core::Client,
        postgres::{local_config, migrate, Store},
        studio_oidc::{OidcConfig, StudioOidc},
        theatre_identity::{NativeIdentity, TheatreIdentity},
    },
};
use http_body_util::BodyExt;
use std::{
    env,
    sync::atomic::Ordering,
    time::{SystemTime, UNIX_EPOCH},
};
use tokio_postgres::{Config, NoTls};
use tower::ServiceExt;
const ORIGIN: &str = "http://127.0.0.1:8082";
async fn connect(config: &Config) -> tokio_postgres::Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}
struct Harness {
    admin: tokio_postgres::Client,
    store: Store,
    provider: mock::MockProvider,
    client: Client,
}
impl Harness {
    async fn new(client: Client) -> Self {
        Self::new_with(client, false).await
    }
    async fn new_with(client: Client, development_callback: bool) -> Self {
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
        admin
            .batch_execute(include_str!(
                "../../../infra/vps/postgres/runtime-grants.sql"
            ))
            .await
            .unwrap();
        config.user("cantos_app");
        let provider = mock::MockProvider::start_for(
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .unwrap()
                .as_secs(),
            client.name(),
        )
        .await;
        let store = Store::new(config).unwrap().without_development_sessions();
        let store = match client {
            Client::Studio => store.with_studio_oidc(
                StudioOidc::new(
                    OidcConfig::new(&provider.issuer(), ORIGIN, "fixture".into()).unwrap(),
                )
                .unwrap()
                .with_admin_subjects(vec!["alice".into(), "bob".into()])
                .unwrap(),
            ),
            Client::TheatreWeb => store.with_theatre_identity(
                TheatreIdentity::new(&provider.issuer(), ORIGIN, "fixture".into()).unwrap(),
            ),
            Client::TheatreNative => store.with_theatre_native(
                NativeIdentity::with_callbacks(&provider.issuer(), development_callback).unwrap(),
            ),
        };
        Self {
            admin,
            store,
            provider,
            client,
        }
    }
    fn root(&self) -> String {
        format!(
            "/api/v1/{}{}",
            self.client.surface(),
            if self.client == Client::TheatreNative {
                "/native/auth/bearer"
            } else {
                "/auth/bearer"
            }
        )
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        body: serde_json::Value,
        token: Option<&str>,
    ) -> (StatusCode, axum::http::HeaderMap, serde_json::Value) {
        let mut req = Request::builder()
            .method(method)
            .uri(path)
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(token) = token {
            req = req.header(header::AUTHORIZATION, format!("Bearer {token}"));
        }
        let app = router(
            AppState {
                store: self.store.clone(),
                web_origin: ORIGIN.into(),
            },
            "/nonexistent",
        );
        let response = app
            .oneshot(
                req.body(Body::from(serde_json::to_vec(&body).unwrap()))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value = if bytes.is_empty() {
            serde_json::Value::Null
        } else {
            serde_json::from_slice(&bytes).unwrap()
        };
        assert_eq!(headers[header::CACHE_CONTROL], "no-store");
        (status, headers, value)
    }
    async fn flow(&self) -> (Flow, ExchangeRequest) {
        self.flow_with_profile(None).await
    }
    async fn flow_with_profile(
        &self,
        callback_profile: Option<cantos_contracts::bearer::CallbackProfile>,
    ) -> (Flow, ExchangeRequest) {
        use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
        use sha2::{Digest, Sha256};
        let verifier = "v".repeat(64);
        let state = uuid::Uuid::new_v4().simple().to_string().repeat(2);
        let (status, _, value) = self
            .request(
                "POST",
                &format!("{}/flows", self.root()),
                serde_json::to_value(FlowRequest {
                    state: state.clone(),
                    nonce: "n".repeat(64),
                    code_challenge: URL_SAFE_NO_PAD.encode(Sha256::digest(verifier.as_bytes())),
                    reauthenticate: false,
                    callback_profile,
                })
                .unwrap(),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        let flow: Flow = serde_json::from_value(value).unwrap();
        (
            flow,
            ExchangeRequest {
                state,
                code: String::new(),
                code_verifier: verifier,
            },
        )
    }
    async fn ready(&self, subject: &str) -> (Flow, ExchangeRequest) {
        let (flow, mut request) = self.flow().await;
        let (state, code) = self.provider.issue(&flow.authorization_url, subject, None);
        if self.client == Client::TheatreNative {
            request.code = code;
        } else {
            let url = url::Url::parse(&flow.authorization_url).unwrap();
            let redirect = url
                .query_pairs()
                .find(|(k, _)| k == "redirect_uri")
                .unwrap()
                .1
                .into_owned();
            let path = url::Url::parse(&redirect).unwrap().path().to_owned();
            let (status, headers, _) = self
                .request(
                    "GET",
                    &format!("{path}?state={state}&code={code}"),
                    serde_json::Value::Null,
                    None,
                )
                .await;
            assert_eq!(status, StatusCode::SEE_OTHER);
            let returned = url::Url::parse(headers[header::LOCATION].to_str().unwrap()).unwrap();
            assert_eq!(returned.origin().ascii_serialization(), ORIGIN);
            let pairs: std::collections::HashMap<_, _> =
                returned.query_pairs().into_owned().collect();
            assert_eq!(pairs["state"], request.state);
            assert_eq!(pairs["flow_id"], flow.flow_id);
            request.code = pairs["code"].clone();
            assert!(!headers.contains_key(header::SET_COOKIE));
        }
        (flow, request)
    }
    async fn login(&self, subject: &str) -> TokenPair {
        let (flow, request) = self.ready(subject).await;
        let (status, _, value) = self
            .request(
                "POST",
                &format!("{}/flows/{}/exchange", self.root(), flow.flow_id),
                serde_json::to_value(request).unwrap(),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK, "{value}");
        serde_json::from_value(value).unwrap()
    }
    async fn refresh(&self, pair: &TokenPair) -> (StatusCode, serde_json::Value) {
        let (status, _, value) = self
            .request(
                "POST",
                &format!("{}/refresh", self.root()),
                serde_json::to_value(RefreshRequest {
                    refresh_token: pair.refresh_token.clone(),
                    session_id: pair.session_id.clone(),
                })
                .unwrap(),
                None,
            )
            .await;
        (status, value)
    }
    async fn session(&self, token: &str) -> StatusCode {
        self.request(
            "GET",
            &format!("/api/v1/{}/session", self.client.surface()),
            serde_json::Value::Null,
            Some(token),
        )
        .await
        .0
    }
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn all_surfaces_issue_rotate_revoke_with_exact_scope_and_stable_session() {
    for client in [Client::Studio, Client::TheatreWeb, Client::TheatreNative] {
        let h = Harness::new(client).await;
        let first = h.login("alice").await;
        assert!(first.expires_at < first.absolute_expires_at);
        assert_eq!(h.session(&first.access_token).await, StatusCode::OK);
        let (status, value) = h.refresh(&first).await;
        assert_eq!(status, StatusCode::OK, "{value}");
        let next: TokenPair = serde_json::from_value(value).unwrap();
        assert!(first.access_token != next.access_token);
        assert!(first.refresh_token != next.refresh_token);
        assert_eq!(first.session_id, next.session_id);
        assert_eq!(first.absolute_expires_at, next.absolute_expires_at);
        assert_eq!(first.authenticated_at, next.authenticated_at);
        assert_eq!(
            h.session(&first.access_token).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(h.session(&next.access_token).await, StatusCode::OK);
        let foreign = if client == Client::Studio {
            "theatre"
        } else {
            "studio"
        };
        assert_eq!(
            h.request(
                "GET",
                &format!("/api/v1/{foreign}/session"),
                serde_json::Value::Null,
                Some(&next.access_token)
            )
            .await
            .0,
            StatusCode::UNAUTHORIZED
        );
        let (status, _, _) = h
            .request(
                "DELETE",
                &format!("{}/session", h.root()),
                serde_json::Value::Null,
                Some(&next.access_token),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(
            h.session(&next.access_token).await,
            StatusCode::UNAUTHORIZED
        );
        assert_eq!(h.refresh(&next).await.0, StatusCode::UNAUTHORIZED);
    }
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn lost_response_and_simultaneous_refresh_revoke_every_generation() {
    let h = Harness::new(Client::TheatreWeb).await;
    let first = h.login("alice").await;
    let (a, b) = tokio::join!(h.refresh(&first), h.refresh(&first));
    assert_eq!(
        [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count(),
        1
    );
    assert_eq!(
        [a.0, b.0]
            .iter()
            .filter(|s| **s == StatusCode::UNAUTHORIZED)
            .count(),
        1
    );
    let value = if a.0 == StatusCode::OK { a.1 } else { b.1 };
    let next: TokenPair = serde_json::from_value(value).unwrap();
    assert_eq!(
        h.session(&next.access_token).await,
        StatusCode::UNAUTHORIZED
    );
    let other = h.login("bob").await;
    assert_eq!(h.session(&other.access_token).await, StatusCode::OK);
    h.provider
        .data
        .introspection_fail
        .store(true, Ordering::SeqCst);
    assert_eq!(h.refresh(&first).await.0, StatusCode::UNAUTHORIZED);
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM bearer_families WHERE revoked", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn cancelled_or_lost_exchange_cannot_restore_and_does_not_cancel_another_account() {
    let h = Harness::new(Client::TheatreWeb).await;
    let (flow, request) = h.ready("alice").await;
    let (status, _, value) = h
        .request(
            "POST",
            &format!("{}/flows/{}/exchange", h.root(), flow.flow_id),
            serde_json::to_value(&request).unwrap(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let pair: TokenPair = serde_json::from_value(value).unwrap();
    let b = h.login("bob").await;
    let (status, _, _) = h
        .request(
            "POST",
            &format!("{}/flows/{}/cancel", h.root(), flow.flow_id),
            serde_json::to_value(CancelRequest {
                state: request.state.clone(),
                code_verifier: request.code_verifier.clone(),
            })
            .unwrap(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 1);
    assert!(!h
        .admin
        .query_one(
            "SELECT revocation_pending FROM bearer_families WHERE id=$1::text::uuid",
            &[&pair.session_id],
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    assert_eq!(
        h.session(&pair.access_token).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(h.session(&b.access_token).await, StatusCode::OK);
    assert_eq!(
        h.request(
            "POST",
            &format!("{}/flows/{}/exchange", h.root(), flow.flow_id),
            serde_json::to_value(request).unwrap(),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let (flow, request) = h.flow().await;
    let (state, code) = h.provider.issue(&flow.authorization_url, "alice", None);
    h.store
        .bearer(h.client)
        .unwrap()
        .cancel(
            &h.store,
            &flow.flow_id,
            CancelRequest {
                state: request.state,
                code_verifier: request.code_verifier,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .bearer(h.client)
            .unwrap()
            .callback(&h.store, &state, &code)
            .await,
        Err(cantos_server::platform::studio_oidc::AuthError::State)
    ));
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn cancelled_callback_receipt_retries_provider_cleanup_without_issuing_authority() {
    let h = Harness::new(Client::TheatreWeb).await;
    let (flow, request) = h.ready("alice").await;
    h.provider.data.revoke_fail.store(true, Ordering::SeqCst);
    let cancel_path = format!("{}/flows/{}/cancel", h.root(), flow.flow_id);
    let cancel = || {
        h.request(
            "POST",
            &cancel_path,
            serde_json::to_value(CancelRequest {
                state: request.state.clone(),
                code_verifier: request.code_verifier.clone(),
            })
            .unwrap(),
            None,
        )
    };
    let (status, headers, _) = cancel().await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(headers["cantos-provider-revocation"], "pending");
    assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 1);
    assert!(h
        .admin
        .query_one(
            "SELECT cancelled AND provider_access IS NOT NULL FROM bearer_flows WHERE id=$1::text::uuid",
            &[&flow.flow_id],
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    h.provider.data.revoke_fail.store(false, Ordering::SeqCst);
    let (status, headers, _) = cancel().await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(headers["cantos-provider-revocation"], "completed");
    assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 2);
    assert!(h
        .admin
        .query_one(
            "SELECT cancelled AND provider_access IS NULL FROM bearer_flows WHERE id=$1::text::uuid",
            &[&flow.flow_id],
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    // Clearing the artifact deliberately retains no durable upstream completion proof. A
    // later retry is conservative rather than claiming an acknowledgement it cannot prove.
    let (status, headers, _) = cancel().await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(headers["cantos-provider-revocation"], "pending");
    assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM bearer_families", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn cancel_during_provider_exchange_cleans_late_web_and_native_artifacts_durably() {
    use cantos_contracts::bearer::CallbackProfile;
    for (client, profile) in [
        (Client::TheatreWeb, None),
        (Client::TheatreNative, None),
        (Client::TheatreNative, Some(CallbackProfile::Development)),
    ] {
        let h = Harness::new_with(client, profile.is_some()).await;
        let (flow, mut request) = h.flow_with_profile(profile).await;
        let (state, code) = h.provider.issue(&flow.authorization_url, "alice", None);
        let pause = h.provider.pause_code(&code);
        request.code = code.clone();
        let service = h.store.bearer(client).unwrap();
        let callback = async {
            if client == Client::TheatreNative {
                service
                    .exchange(
                        &h.store,
                        &flow.flow_id,
                        ExchangeRequest {
                            state: request.state.clone(),
                            code: request.code.clone(),
                            code_verifier: request.code_verifier.clone(),
                        },
                    )
                    .await
                    .map(|_| ())
            } else {
                service.callback(&h.store, &state, &code).await.map(|_| ())
            }
        };
        let cancel = async {
            tokio::time::timeout(std::time::Duration::from_secs(5), pause.entered.notified())
                .await
                .unwrap();
            let completed = service
                .cancel(
                    &h.store,
                    &flow.flow_id,
                    CancelRequest {
                        state: request.state.clone(),
                        code_verifier: request.code_verifier.clone(),
                    },
                )
                .await
                .unwrap();
            assert!(
                !completed,
                "an in-flight provider exchange cannot acknowledge revocation"
            );
            h.provider.data.revoke_fail.store(true, Ordering::SeqCst);
            pause.release.notify_one();
        };
        let (result, ()) = tokio::join!(callback, cancel);
        assert!(matches!(
            result,
            Err(cantos_server::platform::studio_oidc::AuthError::State)
        ));
        assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 1);
        assert!(h.admin.query_one(
            "SELECT cancelled AND provider_access IS NOT NULL FROM bearer_flows WHERE id=$1::text::uuid",
            &[&flow.flow_id],
        ).await.unwrap().get::<_, bool>(0));
        h.provider.data.revoke_fail.store(false, Ordering::SeqCst);
        service
            .cancel(
                &h.store,
                &flow.flow_id,
                CancelRequest {
                    state: request.state,
                    code_verifier: request.code_verifier,
                },
            )
            .await
            .unwrap();
        assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 2);
        assert!(h.admin.query_one(
            "SELECT cancelled AND provider_access IS NULL FROM bearer_flows WHERE id=$1::text::uuid",
            &[&flow.flow_id],
        ).await.unwrap().get::<_, bool>(0));
        assert_eq!(
            h.admin
                .query_one("SELECT count(*) FROM bearer_families", &[])
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
    }
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn denied_studio_subject_and_failed_registry_issue_revoke_verified_provider_artifacts() {
    let h = Harness::new(Client::Studio).await;
    let (flow, _) = h.flow().await;
    let (state, code) = h
        .provider
        .issue(&flow.authorization_url, "outside-roster", None);
    assert!(matches!(
        h.store
            .bearer(h.client)
            .unwrap()
            .callback(&h.store, &state, &code)
            .await,
        Err(cantos_server::platform::studio_oidc::AuthError::Inactive)
    ));
    assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 1);
    assert!(h.admin.query_one(
        "SELECT cancelled AND provider_access IS NULL FROM bearer_flows WHERE id=$1::text::uuid",
        &[&flow.flow_id],
    ).await.unwrap().get::<_, bool>(0));
    let first = h.login("alice").await;
    let (flow, request) = h.ready("alice").await;
    h.admin
        .execute(
            "UPDATE actors SET active=false WHERE id=$1",
            &[&first.actor_id],
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .bearer(h.client)
            .unwrap()
            .exchange(&h.store, &flow.flow_id, request)
            .await,
        Err(cantos_server::platform::studio_oidc::AuthError::Inactive)
    ));
    assert_eq!(h.provider.data.revoke_calls.load(Ordering::SeqCst), 2);
    assert!(h.admin.query_one(
        "SELECT cancelled AND provider_access IS NULL FROM bearer_flows WHERE id=$1::text::uuid",
        &[&flow.flow_id],
    ).await.unwrap().get::<_, bool>(0));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM bearer_families", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn expiry_disable_wrong_binding_provider_outage_and_recent_auth_fail_closed() {
    let h = Harness::new(Client::Studio).await;
    let first = h.login("alice").await;
    let mut wrong = RefreshRequest {
        refresh_token: first.refresh_token.clone(),
        session_id: uuid::Uuid::new_v4().to_string(),
    };
    assert_eq!(
        h.request(
            "POST",
            &format!("{}/refresh", h.root()),
            serde_json::to_value(&wrong).unwrap(),
            None
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    wrong.session_id = first.session_id.clone();
    assert_eq!(
        h.request(
            "POST",
            "/api/v1/studio/works/work/delegations",
            serde_json::json!({}),
            Some(&first.access_token)
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    h.provider
        .data
        .introspection_fail
        .store(true, Ordering::SeqCst);
    assert_eq!(h.refresh(&first).await.0, StatusCode::SERVICE_UNAVAILABLE);
    h.provider
        .data
        .introspection_fail
        .store(false, Ordering::SeqCst);
    assert_eq!(h.session(&first.access_token).await, StatusCode::OK);
    h.admin
        .execute(
            "UPDATE actors SET active=false WHERE id=$1",
            &[&first.actor_id],
        )
        .await
        .unwrap();
    assert_eq!(h.refresh(&first).await.0, StatusCode::UNAUTHORIZED);
    h.admin
        .execute(
            "UPDATE actors SET active=true WHERE id=$1",
            &[&first.actor_id],
        )
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE bearer_families SET idle_expires=0 WHERE id=$1::text::uuid",
            &[&first.session_id],
        )
        .await
        .unwrap();
    assert_eq!(h.refresh(&first).await.0, StatusCode::UNAUTHORIZED);
    h.admin
        .execute(
            "UPDATE bearer_tokens SET expires_at=0 WHERE family_id=$1::text::uuid",
            &[&first.session_id],
        )
        .await
        .unwrap();
    assert_eq!(
        h.session(&first.access_token).await,
        StatusCode::UNAUTHORIZED
    );
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn logout_wins_while_refresh_is_in_provider_io_even_if_remote_revoke_fails() {
    let h = Harness::new(Client::TheatreWeb).await;
    let first = h.login("alice").await;
    h.provider
        .data
        .introspection_pause
        .store(true, Ordering::SeqCst);
    h.provider.data.revoke_fail.store(true, Ordering::SeqCst);
    let racing_refresh = h.refresh(&first);
    let logout = async {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            h.provider.data.introspection_started.notified(),
        )
        .await
        .unwrap();
        let (status, headers, _) = h
            .request(
                "DELETE",
                &format!("{}/session", h.root()),
                serde_json::Value::Null,
                Some(&first.access_token),
            )
            .await;
        assert_eq!(status, StatusCode::NO_CONTENT);
        assert_eq!(headers["cantos-provider-revocation"], "pending");
        h.provider.data.introspection_release.notify_one();
    };
    let ((status, _), ()) = tokio::join!(racing_refresh, logout);
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(
        h.session(&first.access_token).await,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM bearer_tokens", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert!(h
        .admin
        .query_one(
            "SELECT revoked AND revocation_pending FROM bearer_families",
            &[]
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn lost_refresh_response_is_reauthenticated_and_consumption_is_monotonic() {
    let h = Harness::new(Client::TheatreWeb).await;
    let first = h.login("alice").await;
    let (status, _discarded_response) = h.refresh(&first).await;
    assert_eq!(status, StatusCode::OK);
    // Drop a committed response at the client boundary; never retry it as an idempotent request.
    assert_eq!(h.refresh(&first).await.0, StatusCode::UNAUTHORIZED);
    let token_hash = cantos_server::platform::postgres::token_hash(&first.access_token);
    assert_eq!(
        h.admin
            .execute(
                "UPDATE bearer_tokens SET used=false WHERE access_hash=$1",
                &[&token_hash]
            )
            .await
            .unwrap_err()
            .code()
            .unwrap()
            .code(),
        "23514"
    );
    assert_eq!(
        h.admin
            .execute("UPDATE bearer_families SET revoked=false", &[])
            .await
            .unwrap_err()
            .code()
            .unwrap()
            .code(),
        "23514"
    );
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn signed_auth_time_is_preserved_and_current_roster_reduction_refuses_access() {
    let h = Harness::new(Client::Studio).await;
    let (flow, mut request) = h.flow().await;
    let time = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs();
    let (state, code) = h.provider.issue(
        &flow.authorization_url,
        "alice",
        Some(("auth_time".into(), serde_json::json!(time))),
    );
    let redirect = h
        .store
        .bearer(h.client)
        .unwrap()
        .callback(&h.store, &state, &code)
        .await
        .unwrap();
    let url = url::Url::parse(&redirect).unwrap();
    request.code = url
        .query_pairs()
        .find(|(k, _)| k == "code")
        .unwrap()
        .1
        .into_owned();
    let first = h
        .store
        .bearer(h.client)
        .unwrap()
        .exchange(&h.store, &flow.flow_id, request)
        .await
        .unwrap();
    assert_eq!(first.authenticated_at, Some(time));
    let (status, value) = h.refresh(&first).await;
    assert_eq!(status, StatusCode::OK);
    let next: TokenPair = serde_json::from_value(value).unwrap();
    assert_eq!(next.authenticated_at, Some(time));
    let reduced = h.store.clone().with_studio_oidc(
        StudioOidc::new(OidcConfig::new(&h.provider.issuer(), ORIGIN, "fixture".into()).unwrap())
            .unwrap()
            .with_admin_subjects(vec!["bob".into()])
            .unwrap(),
    );
    assert!(matches!(
        reduced
            .bearer(Client::Studio)
            .unwrap()
            .authenticate(&reduced, &next.access_token)
            .await,
        Err(cantos_server::platform::studio_oidc::AuthError::Inactive)
    ));
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn bearer_authorizes_the_existing_json_and_protobuf_validation_equally() {
    use cantos_contracts::{
        wire::{self, WireFormat},
        ScriptValidationResponse, ValidateScriptRequest,
    };
    let h = Harness::new(Client::Studio).await;
    let pair = h.login("alice").await;
    let script =
        include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
    let request = ValidateScriptRequest {
        script_json: script.into(),
    };
    for format in [WireFormat::Json, WireFormat::Protobuf] {
        let body = wire::encode_pilot(format, &request).unwrap();
        let app = router(
            AppState {
                store: h.store.clone(),
                web_origin: ORIGIN.into(),
            },
            "/nonexistent",
        );
        let response = app
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/v1/studio/validation")
                    .header(header::CONTENT_TYPE, format.media_type())
                    .header(header::ACCEPT, format.media_type())
                    .header(
                        header::AUTHORIZATION,
                        format!("Bearer {}", pair.access_token),
                    )
                    .header("x-cantos-studio-session", &pair.session_id)
                    .body(Body::from(body))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        assert_eq!(
            response.headers()["x-cantos-studio-session"],
            pair.session_id
        );
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let result: ScriptValidationResponse = wire::decode_pilot(format, &bytes).unwrap();
        assert!(result.issues.is_empty());
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn bearer_hash_lookup_uses_bounded_postgresql_index_without_cached_authority() {
    let h = Harness::new(Client::TheatreWeb).await;
    let first = h.login("alice").await;
    h.admin.execute("INSERT INTO bearer_tokens(access_hash,refresh_hash,family_id,expires_at) SELECT sha256(convert_to('synthetic-access-'||v,'UTF8')),sha256(convert_to('synthetic-refresh-'||v,'UTF8')),$1::text::uuid,$2 FROM generate_series(1,2000) v",&[&first.session_id,&(first.expires_at as i64)]).await.unwrap();
    h.admin
        .batch_execute("ANALYZE bearer_tokens; ANALYZE bearer_families; SET ROLE cantos_app")
        .await
        .unwrap();
    let sql = include_str!("../src/platform/bearer/credential.sql");
    let hash = cantos_server::platform::postgres::token_hash(&first.access_token);
    let client = h.client.name();
    let issuer = h.provider.issuer();
    let plan: Vec<String> = h
        .admin
        .query(
            &format!("EXPLAIN (ANALYZE,BUFFERS) {sql}"),
            &[&hash, &client, &issuer],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    assert!(plan.join("\n").contains("bearer_tokens_pkey"));
    let started = std::time::Instant::now();
    for _ in 0..20 {
        assert_eq!(
            h.admin
                .query(sql, &[&hash, &client, &issuer])
                .await
                .unwrap()
                .len(),
            1
        );
    }
    let evidence = serde_json::json!({"mode":"disposable-postgresql/app-role/warm-point-lookup","synthetic_retained_generations":2001,"samples":20,"elapsed_microseconds":started.elapsed().as_micros(),"plan":plan,"production_slo":false});
    h.admin.batch_execute("RESET ROLE").await.unwrap();
    let directory = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../artifacts/agent-runs/issue253-phase1");
    std::fs::create_dir_all(&directory).unwrap();
    std::fs::write(
        directory.join("lookup-postgres.json"),
        serde_json::to_vec_pretty(&evidence).unwrap(),
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn account_disable_during_provider_io_prevents_rotation_without_consuming_refresh() {
    let h = Harness::new(Client::TheatreWeb).await;
    let first = h.login("alice").await;
    h.provider
        .data
        .introspection_pause
        .store(true, Ordering::SeqCst);
    let pending = h.refresh(&first);
    let disable = async {
        tokio::time::timeout(
            std::time::Duration::from_secs(5),
            h.provider.data.introspection_started.notified(),
        )
        .await
        .unwrap();
        h.admin
            .execute(
                "UPDATE actors SET active=false WHERE id=$1",
                &[&first.actor_id],
            )
            .await
            .unwrap();
        h.provider.data.introspection_release.notify_one();
    };
    let ((status, _), ()) = tokio::join!(pending, disable);
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!h
        .admin
        .query_one("SELECT used FROM bearer_tokens", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM bearer_tokens", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn configuration_change_cannot_retarget_an_inflight_web_flow() {
    let h = Harness::new(Client::TheatreWeb).await;
    let (flow, _) = h.flow().await;
    let (state, code) = h.provider.issue(&flow.authorization_url, "alice", None);
    let moved = h.store.clone().with_theatre_identity(
        TheatreIdentity::new(
            &h.provider.issuer(),
            "http://127.0.0.1:8099",
            "fixture".into(),
        )
        .unwrap(),
    );
    assert!(matches!(
        moved
            .bearer(h.client)
            .unwrap()
            .callback(&moved, &state, &code)
            .await,
        Err(cantos_server::platform::studio_oidc::AuthError::State)
    ));
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn provider_cancellation_returns_only_a_bound_generic_error_and_cannot_issue_late() {
    let h = Harness::new(Client::TheatreWeb).await;
    let (flow, _) = h.flow().await;
    let (state, code) = h.provider.issue(&flow.authorization_url, "alice", None);
    let (status, headers, _) = h
        .request(
            "GET",
            &format!(
                "{}/callback?state={state}&error=access_denied&error_description=cancelled",
                h.root()
            ),
            serde_json::Value::Null,
            None,
        )
        .await;
    assert_eq!(status, StatusCode::SEE_OTHER);
    let url = url::Url::parse(headers[header::LOCATION].to_str().unwrap()).unwrap();
    assert_eq!(url.origin().ascii_serialization(), ORIGIN);
    let pairs: std::collections::HashMap<_, _> = url.query_pairs().collect();
    assert_eq!(pairs["error"], "authorization_failed");
    assert!(!pairs.contains_key("code"));
    assert!(matches!(
        h.store
            .bearer(h.client)
            .unwrap()
            .callback(&h.store, &state, &code)
            .await,
        Err(cantos_server::platform::studio_oidc::AuthError::State)
    ));
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 0);
}

fn begin_body(profile: Option<&str>) -> serde_json::Value {
    let mut body = serde_json::json!({
        "state": uuid::Uuid::new_v4().simple().to_string().repeat(2),
        "nonce": "n".repeat(64),
        "code_challenge": "c".repeat(43),
    });
    if let Some(profile) = profile {
        body["callback_profile"] = profile.into();
    }
    body
}
fn redirect_of(flow: &serde_json::Value) -> String {
    url::Url::parse(flow["authorization_url"].as_str().unwrap())
        .unwrap()
        .query_pairs()
        .find(|(k, _)| k == "redirect_uri")
        .unwrap()
        .1
        .into_owned()
}
async fn stored_redirects(h: &Harness) -> Vec<String> {
    h.admin
        .query(
            "SELECT redirect_uri FROM bearer_flows ORDER BY redirect_uri",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect()
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn native_callback_profile_is_closed_admitted_by_operator_and_pinned_per_flow() {
    let h = Harness::new_with(Client::TheatreNative, true).await;
    let flows = format!("{}/flows", h.root());
    let (status, _, production) = h.request("POST", &flows, begin_body(None), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        redirect_of(&production),
        "com.cantos.theatre:/oauth2/callback"
    );
    let (status, _, explicit) = h
        .request("POST", &flows, begin_body(Some("production")), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        redirect_of(&explicit),
        "com.cantos.theatre:/oauth2/callback"
    );
    let (status, _, development) = h
        .request("POST", &flows, begin_body(Some("development")), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        redirect_of(&development),
        "com.cantos.theatre.dev:/oauth2/callback"
    );
    assert_eq!(
        stored_redirects(&h).await,
        [
            "com.cantos.theatre.dev:/oauth2/callback",
            "com.cantos.theatre:/oauth2/callback",
            "com.cantos.theatre:/oauth2/callback"
        ]
    );
    // Arbitrary URIs, other spellings and unknown fields are never a profile.
    for bad in [
        "com.cantos.theatre.dev:/oauth2/callback",
        "com.cantos.evil",
        "Development",
        "",
    ] {
        let (status, _, _) = h.request("POST", &flows, begin_body(Some(bad)), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{bad}");
    }
    let mut extra = begin_body(None);
    extra["redirect_uri"] = "com.cantos.theatre.dev:/oauth2/callback".into();
    assert_eq!(
        h.request("POST", &flows, extra, None).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(stored_redirects(&h).await.len(), 3);

    let (bound_flow, mut exchange) = h
        .flow_with_profile(Some(cantos_contracts::bearer::CallbackProfile::Development))
        .await;
    let (_, code) = h
        .provider
        .issue(&bound_flow.authorization_url, "alice", None);
    exchange.code = code;
    let flow = bound_flow.flow_id.as_str();
    // A stored callback no longer admitted (or tampered) can never be consumed.
    h.admin
        .execute(
            "UPDATE bearer_flows SET redirect_uri='com.cantos.evil:/oauth2/callback' WHERE id=$1::text::uuid",
            &[&flow],
        )
        .await
        .unwrap();
    let (status, _, _) = h
        .request(
            "POST",
            &format!("{}/flows/{flow}/exchange", h.root()),
            serde_json::to_value(&exchange).unwrap(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 0);
    assert!(!h
        .admin
        .query_one(
            "SELECT consumed FROM bearer_flows WHERE id=$1::text::uuid",
            &[&flow],
        )
        .await
        .unwrap()
        .get::<_, bool>(0));

    // The identical state, PKCE verifier and provider code must work once the sole
    // invalid fact is restored. The mock provider checks its originally bound redirect,
    // so defaulting development exchange to the production URI also fails this control.
    h.admin
        .execute(
            "UPDATE bearer_flows SET redirect_uri='com.cantos.theatre.dev:/oauth2/callback' WHERE id=$1::text::uuid",
            &[&flow],
        )
        .await
        .unwrap();
    let (status, _, value) = h
        .request(
            "POST",
            &format!("{}/flows/{flow}/exchange", h.root()),
            serde_json::to_value(&exchange).unwrap(),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    let pair: TokenPair = serde_json::from_value(value).unwrap();
    assert_eq!(h.session(&pair.access_token).await, StatusCode::OK);
    assert_eq!(h.provider.data.token_calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn unadmitted_development_profile_is_refused_without_fallback_or_flow_and_other_clients_reject_it(
) {
    let native = Harness::new(Client::TheatreNative).await;
    let (status, _, _) = native
        .request(
            "POST",
            &format!("{}/flows", native.root()),
            begin_body(Some("development")),
            None,
        )
        .await;
    assert!(status.is_server_error(), "{status}");
    assert!(stored_redirects(&native).await.is_empty());
    let (status, _, _) = native
        .request(
            "POST",
            &format!("{}/flows", native.root()),
            begin_body(None),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    for client in [Client::Studio, Client::TheatreWeb] {
        let h = Harness::new(client).await;
        let (status, _, _) = h
            .request(
                "POST",
                &format!("{}/flows", h.root()),
                begin_body(Some("development")),
                None,
            )
            .await;
        assert!(status.is_client_error(), "{status}");
        assert!(stored_redirects(&h).await.is_empty());
    }
}
