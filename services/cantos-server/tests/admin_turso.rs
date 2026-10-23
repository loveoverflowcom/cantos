//! Actual Admin HTTP/PG tests. No Admin verifier is composed in production: the former
//! Kanidm/OIDC adapter is retired. These tests plug a synthetic in-process [`AdminVerifier`]
//! through the public port, so Admin policy, audit, revocation and PostgreSQL atomicity stay
//! covered while credential verification itself has no implementation to test.
use axum::{
    body::Body,
    http::{header, Request, StatusCode},
};
use cantos_server::{
    adapters::inbound::http::{router, server_router, AppState},
    platform::{
        admin::{map_groups, AdminError, AdminService, AdminVerifier, RoleEvidence, VerifyFuture},
        identity::domain::ExternalIdentityKey,
        store::Store,
    },
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::{SystemTime, UNIX_EPOCH},
};
#[path = "support/native_sql.rs"]
mod native_sql;
use native_sql::NativeSql;
use tower::ServiceExt;
const ISSUER: &str = "https://admin-fixture.example/oauth2/openid/cantos-admin-api";
fn unix_now() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs()
}

/// Synthetic verifier. A credential is `fixture|subject|group,group[|exp]`; the "current" role
/// observation is whatever groups were last issued or set, like a provider's live group state.
struct Fixture {
    groups: Mutex<Vec<String>>,
    active: AtomicBool,
    unavailable: AtomicBool,
    calls: AtomicUsize,
}
impl Fixture {
    fn new() -> Arc<Self> {
        Arc::new(Self {
            groups: Mutex::new(vec!["cantos_identity_operators".into()]),
            active: AtomicBool::new(true),
            unavailable: AtomicBool::new(false),
            calls: AtomicUsize::new(0),
        })
    }
    fn issuer(&self) -> String {
        ISSUER.into()
    }
    fn resource(&self, subject: &str, groups: &[&str], exp: Option<u64>) -> String {
        self.set_admin_groups(groups);
        let mut token = format!("fixture|{subject}|{}", groups.join(","));
        if let Some(exp) = exp {
            token.push_str(&format!("|{exp}"));
        }
        token
    }
    fn set_admin_groups(&self, groups: &[&str]) {
        *self.groups.lock().unwrap() = groups.iter().map(|g| (*g).to_owned()).collect();
    }
}
impl AdminVerifier for Fixture {
    fn verify<'a>(&'a self, token: &'a str) -> VerifyFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let parts: Vec<&str> = token.split('|').collect();
            if parts.len() < 3 || parts[0] != "fixture" {
                return Err(AdminError::Credential);
            }
            if self.unavailable.load(Ordering::SeqCst) {
                return Err(AdminError::ProviderUnavailable);
            }
            let signed: Vec<String> = parts[2]
                .split(',')
                .filter(|g| !g.is_empty())
                .map(str::to_owned)
                .collect();
            let signed = map_groups(&signed)?;
            if !self.active.load(Ordering::SeqCst) {
                return Err(AdminError::Credential);
            }
            let current = map_groups(&self.groups.lock().unwrap())?;
            if current != signed {
                return Err(AdminError::StaleRole);
            }
            let now = unix_now();
            let exp = parts
                .get(3)
                .map_or(Some(now + 200), |v| v.parse::<u64>().ok())
                .ok_or(AdminError::Credential)?;
            if now >= exp {
                return Err(AdminError::Credential);
            }
            let key =
                ExternalIdentityKey::new(ISSUER, parts[1]).map_err(|_| AdminError::Credential)?;
            Ok(RoleEvidence::verified(key, current, now, exp.min(now + 5)))
        })
    }
}

struct Harness {
    db: NativeSql,
    store: Store,
    provider: Arc<Fixture>,
}
impl Harness {
    async fn new() -> Self {
        let db = NativeSql::new().await;
        let provider = Fixture::new();
        db.execute(
            "INSERT INTO actors(id) VALUES('minh'),('other'),('third')",
            &[],
        )
        .await
        .unwrap();
        for actor in ["minh", "other", "third"] {
            db.execute(
                "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES(?1,?2,?2,0)",
                &[&provider.issuer(), &actor],
            )
            .await
            .unwrap();
        }
        let service = AdminService::new(provider.clone());
        let store = db.store().with_admin(service);
        Self {
            db,
            store,
            provider,
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
        let origins = cantos_server::platform::host::SurfaceOrigins::new(
            "http://127.0.0.1:8081",
            "http://127.0.0.1:8082",
            Some("http://127.0.0.1:8080"),
        )
        .unwrap();
        let app = server_router(
            AppState {
                store: self.store.clone(),
                web_origin: origins.studio().into(),
            },
            &origins,
            None,
            None,
        );
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header(
                header::ORIGIN,
                if path.starts_with("/api/v1/studio/") {
                    "http://127.0.0.1:8081"
                } else if path.starts_with("/api/v1/theatre/") {
                    "http://127.0.0.1:8082"
                } else {
                    "http://127.0.0.1:8080"
                },
            )
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
async fn admin_capability_is_existing_registry_actor_with_explicit_source_and_audited_role() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let (status, body) = h.request("GET", CAP, Some(&token), None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    let schema: Value = serde_json::from_str(include_str!(
        "../contracts/admin/access/v1/access.schema.json"
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
            "SELECT actor_id,surface,action,outcome,correlation_id FROM admin_audit",
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
        h.db.query_one("SELECT json_object('id',id,'actor_id',actor_id,'surface',surface,'action',action,'target_actor_id',target_actor_id,'outcome',outcome,'occurred_at',occurred_at,'correlation_id',correlation_id) FROM admin_audit", &[])
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
    assert_eq!(h.count("development_sessions").await, 0);
}
#[tokio::test]
async fn valid_admin_resource_credential_never_authorizes_private_studio_rendition_reads() {
    let h = Harness::new().await;
    // Allow Studio's real empty-session lookup. A missing test-role SQL privilege (503)
    // would not prove that relabeling this valid Admin JWT cannot authenticate to Studio.
    let token = h.token("minh");
    let (status, capability) = h.request("GET", CAP, Some(&token), None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(capability["actor_id"], "minh");
    assert_eq!(capability["role"], "identity_operator");
    assert_eq!(h.count("admin_audit").await, 1);

    // No private media or storage adapter is configured: exact 401 must precede object
    // lookup, rather than passing because a rendition is missing or storage is unavailable.
    const AUTHORITY_AND_MEDIA: &str = "SELECT json_object('actors',(SELECT count(*) FROM actors),'bindings',(SELECT count(*) FROM actor_external_identities),'studio_sessions',(SELECT count(*) FROM development_sessions),'theatre_sessions',(SELECT count(*) FROM identity_sessions),'native_sessions',(SELECT count(*) FROM identity_session_tokens),'admin_audit',(SELECT count(*) FROM admin_audit),'admin_operations',(SELECT count(*) FROM admin_operations),'admin_revocations',(SELECT count(*) FROM admin_access_revocations),'script_members',(SELECT count(*) FROM script_members),'producer_grants',(SELECT count(*) FROM production_producer_grants),'intents',(SELECT count(*) FROM audio_upload_intents),'receipts',(SELECT count(*) FROM audio_intake_receipts),'submissions',(SELECT count(*) FROM audio_submissions),'renditions',(SELECT count(*) FROM audio_renditions))";
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
    let verifications_before = h.provider.calls.load(Ordering::SeqCst);
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
        h.provider.calls.load(Ordering::SeqCst),
        verifications_before,
        "Studio reads must not admit through the Admin verifier"
    );
    let (status, capability) = h.request("GET", CAP, Some(&token), None, json!({})).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(capability["actor_id"], "minh");
    assert_eq!(capability["role"], "identity_operator");
    assert_eq!(h.count("admin_audit").await, 2);
}

#[tokio::test]
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
    h.provider.active.store(false, Ordering::SeqCst);
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::UNAUTHORIZED
    );
    h.provider.active.store(true, Ordering::SeqCst);
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
async fn required_audit_failure_and_effect_failure_never_commit_partial_success() {
    let h = Harness::new().await;
    let token = h.token("minh");
    h.db.execute("CREATE TRIGGER fixture_audit_fail BEFORE INSERT ON admin_audit BEGIN SELECT RAISE(ABORT,'synthetic fault'); END",&[]).await.unwrap();
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
    h.db.execute("DROP TRIGGER fixture_audit_fail", &[])
        .await
        .unwrap();
    h.db.execute("CREATE TRIGGER fixture_effect_fail BEFORE INSERT ON admin_operations BEGIN SELECT RAISE(ABORT,'synthetic fault'); END",&[]).await.unwrap();
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
async fn concurrent_duplicate_delivery_recovers_one_receipt_after_application_restart() {
    let mut h = Harness::new().await;
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

    let replacement = NativeSql::new().await;
    let old = std::mem::replace(&mut h.store, replacement.store());
    drop(old);
    h.db.0.restart().await;
    h.store =
        h.db.store()
            .with_admin(AdminService::new(h.provider.clone()));
    let restarted = h.store.clone();
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
async fn verifier_outage_fails_closed_with_unavailable_audit() {
    let h = Harness::new().await;
    let token = h.token("minh");
    h.provider.unavailable.store(true, Ordering::SeqCst);
    assert_eq!(
        h.request("GET", CAP, Some(&token), None, json!({})).await.0,
        StatusCode::SERVICE_UNAVAILABLE
    );
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

#[tokio::test]
async fn revocation_and_audit_are_atomic_replayable_and_append_only_under_runtime_role() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let op = uuid::Uuid::new_v4().to_string();
    let body = json!({"operation_id":op});
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
    assert_eq!(
        h.request(
            "POST",
            &revoke_path("other"),
            Some(&token),
            None,
            body.clone()
        )
        .await,
        one
    );
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
    assert!(h
        .db
        .query_one("SELECT active FROM actors WHERE id='other'", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
    assert_eq!(h.count("admin_access_revocations").await, 1);
    assert_eq!(h.count("admin_operations").await, 1);
    for table in [
        "admin_audit",
        "admin_operations",
        "admin_access_revocations",
    ] {
        let before = h.count(table).await;
        for sql in [
            format!("DELETE FROM {table}"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table}"),
        ] {
            assert_eq!(
                h.db.0.try_exec(&sql, &[]).await.unwrap_err().guard(),
                Some("immutable.admin_receipt")
            );
            assert_eq!(h.count(table).await, before);
        }
    }
    for sql in [
        "UPDATE admin_audit SET outcome='denied'",
        "UPDATE admin_operations SET target_actor_id='third'",
        "UPDATE admin_access_revocations SET revoked_at=revoked_at",
    ] {
        assert_eq!(
            h.db.0.try_exec(sql, &[]).await.unwrap_err().guard(),
            Some("immutable.admin_receipt")
        );
    }
    assert_eq!(h.count("development_sessions").await, 0);
    assert_eq!(h.count("identity_sessions").await, 0);
}
#[tokio::test]
async fn local_admin_revocation_committed_while_waiting_for_lock_is_rechecked() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let service = h.store.admin().unwrap();
    let blocker = h.db.0.writer().await;
    let mut pending = Box::pin(service.capabilities(&h.store, Some(&token), uuid::Uuid::new_v4()));
    assert!(futures_util::poll!(&mut pending).is_pending());
    blocker
        .execute("INSERT INTO admin_access_revocations VALUES('minh',0)", ())
        .await
        .unwrap();
    blocker.commit().await.unwrap();
    assert_eq!(pending.await.err(), Some(AdminError::Revoked));
    assert_eq!(h.count("admin_operations").await, 0);
    assert_eq!(h.count("admin_audit").await, 1);
}
#[tokio::test]
async fn actor_disable_committed_while_waiting_for_lock_is_rechecked_and_audited() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let service = h.store.admin().unwrap();
    let blocker = h.db.0.writer().await;
    let op = uuid::Uuid::new_v4().to_string();
    let mut pending =
        Box::pin(service.revoke_access(&h.store, Some(&token), "other", &op, uuid::Uuid::new_v4()));
    assert!(futures_util::poll!(&mut pending).is_pending());
    blocker
        .execute("UPDATE actors SET active=0 WHERE id='minh'", ())
        .await
        .unwrap();
    blocker.commit().await.unwrap();
    assert_eq!(pending.await.err(), Some(AdminError::InactiveActor));
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
    assert_eq!(
        (
            row.get::<_, String>(0),
            row.get::<_, String>(1),
            row.get::<_, String>(2),
            row.get::<_, String>(3)
        ),
        (
            "minh".into(),
            "revoke_admin_access".into(),
            "other".into(),
            "denied".into()
        )
    );
}
#[tokio::test]
async fn receipt_read_failure_after_verified_admission_is_audited_without_an_effect() {
    let h = Harness::new().await;
    let token = h.token("minh");
    h.db.execute(
        "ALTER TABLE admin_operations RENAME TO fixture_hidden_operations",
        &[],
    )
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
    h.db.execute(
        "ALTER TABLE fixture_hidden_operations RENAME TO admin_operations",
        &[],
    )
    .await
    .unwrap();
    assert_eq!(result.err(), Some(AdminError::Unavailable));
    assert_eq!(h.count("admin_operations").await, 0);
    assert_eq!(h.count("admin_access_revocations").await, 0);
    let row =
        h.db.query_one("SELECT actor_id,outcome FROM admin_audit", &[])
            .await
            .unwrap();
    assert_eq!(row.get::<_, String>(0), "minh");
    assert_eq!(row.get::<_, String>(1), "unavailable");
}
#[tokio::test]
async fn admin_recovery_after_database_restart() {
    let h = Harness::new().await;
    let token = h.token("minh");
    let op = uuid::Uuid::new_v4().to_string();
    let service = h.store.admin().unwrap();
    let receipt = service
        .revoke_access(&h.store, Some(&token), "other", &op, uuid::Uuid::new_v4())
        .await
        .unwrap();
    let raw="SELECT json_group_array(json_array(id,actor_id,surface,action,target_actor_id,outcome,occurred_at,correlation_id)) FROM (SELECT * FROM admin_audit ORDER BY id)";
    let before: String = h.db.query_one(raw, &[]).await.unwrap().get(0);
    assert!(!before.contains(&token));
    assert!(!before.contains(ISSUER));
    let Harness {
        mut db,
        store,
        provider,
    } = h;
    drop(store);
    db.0.restart().await;
    let h = Harness {
        store: db.store().with_admin(AdminService::new(provider.clone())),
        db,
        provider,
    };
    assert_eq!(
        h.db.query_one(raw, &[]).await.unwrap().get::<_, String>(0),
        before
    );
    assert_eq!(
        h.store
            .admin()
            .unwrap()
            .revoke_access(&h.store, Some(&token), "other", &op, uuid::Uuid::new_v4())
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(h.count("admin_operations").await, 1);
    assert_eq!(h.count("admin_access_revocations").await, 1);
    let target = h.token("other");
    assert_eq!(
        h.store
            .admin()
            .unwrap()
            .capabilities(&h.store, Some(&target), uuid::Uuid::new_v4())
            .await
            .err(),
        Some(AdminError::Revoked)
    );
}
#[tokio::test]
async fn bounded_admin_point_lookup_baseline_uses_current_runtime_role_and_primary_keys() {
    let h = Harness::new().await;
    for n in 0..2000 {
        h.db.execute("INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES(?1,?2,'minh',0)",&[&ISSUER,&format!("synthetic-{n}")]).await.unwrap();
    }
    let token = h.token("synthetic-1900");
    let mut samples = Vec::new();
    for i in 0..25 {
        let start = std::time::Instant::now();
        let reply = h
            .store
            .admin()
            .unwrap()
            .capabilities(&h.store, Some(&token), uuid::Uuid::new_v4())
            .await
            .unwrap();
        assert_eq!(reply.actor_id, "minh");
        if i >= 5 {
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    let plan:Vec<String>=h.db.query("EXPLAIN QUERY PLAN SELECT a.id,a.active,EXISTS(SELECT 1 FROM admin_access_revocations r WHERE r.actor_id=a.id) FROM actor_external_identities e JOIN actors a ON a.id=e.actor_id WHERE e.issuer=?1 AND e.subject=?2",&[&ISSUER,&"synthetic-1900"]).await.unwrap().iter().map(|r|r.get(3)).collect();
    assert!(
        plan.iter()
            .any(|p| p.contains("actor_external_identities") && p.contains("INDEX")),
        "{plan:?}"
    );
    eprintln!("Turso 0.8.3 Admin full verified-fixture service;2000 bindings;5 warmup/20 samples ms={samples:?};plan={plan:?};no production SLO claim");
}
