//! The Server's fact boundary against a **scripted** Identity: stale, misbound, raced, restored
//! and slow answers that the real executable never produces. This is `mock-integration-tested`
//! evidence about what the Server refuses; it is not a login proof (see
//! `identity_password_postgres.rs` for the real executable). Real PostgreSQL, production router.
#[path = "support/fake_identity.rs"]
mod fake;
#[path = "support/identity_world.rs"]
mod world;

use axum::http::StatusCode;
use cantos_server::platform::identity_auth::decision::Subject;
use fake::{Script, Serving};
use serde_json::json;
use world::{App, Database, Reply};

const ORIGIN: &str = "https://theatre.example";
const THEATRE: &str = "/api/v1/theatre";

struct Harness {
    db: Database,
    app: App,
    fake: Serving,
    _dir: std::path::PathBuf,
}

impl Harness {
    async fn start() -> Self {
        Self::build(None, &[]).await
    }

    async fn with_peer_uid(peer_uid: Option<u32>) -> Self {
        Self::build(peer_uid, &[]).await
    }

    /// The scripted subject is on the Studio roster.
    async fn with_studio() -> Self {
        Self::build(None, &[Subject::from_bytes([0x11; 16]).unwrap()]).await
    }

    async fn build(peer_uid: Option<u32>, studio: &[Subject]) -> Self {
        use std::os::unix::fs::{DirBuilderExt, MetadataExt};
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let dir = std::path::PathBuf::from(format!(
            "/tmp/cf-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::DirBuilder::new().mode(0o700).create(&dir).unwrap();
        let uid = std::fs::metadata(&dir).unwrap().uid();
        let socket = dir.join("identity.sock");
        let fake = Serving::start(&socket, Script::healthy());
        let db = Database::new().await;
        let app = App::new(&db, ORIGIN, socket, peer_uid.unwrap_or(uid), studio);
        Self {
            db,
            app,
            fake,
            _dir: dir,
        }
    }

    async fn login(&self) -> Reply {
        self.app
            .post(
                &format!("{THEATRE}/identity/login"),
                &[("origin", ORIGIN)],
                json!({"email": "owner@example.com", "password": "a correct horse battery"}),
            )
            .await
    }

    async fn session_count(&self) -> i64 {
        self.db
            .admin
            .query_one("SELECT count(*) FROM identity_sessions", &[])
            .await
            .unwrap()
            .get(0)
    }

    async fn revoked_reason(&self) -> Vec<Option<String>> {
        self.db
            .admin
            .query(
                "SELECT revoked_reason FROM identity_sessions ORDER BY authenticated_at,id",
                &[],
            )
            .await
            .unwrap()
            .iter()
            .map(|row| row.get(0))
            .collect()
    }

    async fn protected(&self, cookie: &str) -> StatusCode {
        self.app
            .get(
                &format!("{THEATRE}/library/continue"),
                &[("cookie", cookie)],
            )
            .await
            .status
    }
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self._dir);
    }
}

fn cookie_of(reply: &Reply) -> String {
    let (name, value) = reply.cookie();
    format!("{name}={value}")
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn a_stale_future_or_misbound_fact_never_becomes_a_session() {
    let h = Harness::start().await;
    // Edge of the window: 29 s old is admitted, which also shows the happy path is reachable.
    h.fake.set(|s| s.fact_age_micros = 29_000_000);
    assert_eq!(h.login().await.status, StatusCode::OK);
    assert_eq!(h.session_count().await, 1);

    type Mutation = fn(&mut Script);
    let refusals: [(&str, Mutation); 7] = [
        ("31 s old", |s| s.fact_age_micros = 31_000_000),
        ("6 s in the future", |s| s.fact_age_micros = -6_000_000),
        ("another operation", |s| {
            s.echo_operation = Some(vec![9; 16])
        }),
        ("zero operation", |s| s.echo_operation = Some(vec![0; 16])),
        ("another purpose", |s| s.fact_purpose = Some(2)),
        ("no assurance", |s| s.fact_assurance = 0),
        ("zero subject", |s| s.subject = [0; 16]),
    ];
    for (label, mutate) in refusals {
        let fresh = Harness::start().await;
        fresh.fake.set(mutate);
        let reply = fresh.login().await;
        assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE, "{label}");
        assert!(reply.set_cookies().is_empty(), "{label}");
        assert_eq!(fresh.session_count().await, 0, "{label}");
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn a_security_change_between_the_fact_and_the_issue_is_refused_and_cuts_no_session() {
    type Mutation = fn(&mut Script);
    let races: [(&str, Mutation, StatusCode); 6] = [
        (
            "version advanced",
            |s| s.version = 4,
            StatusCode::UNAUTHORIZED,
        ),
        (
            "version regressed",
            |s| s.version = 2,
            StatusCode::UNAUTHORIZED,
        ),
        ("disabled", |s| s.active = false, StatusCode::UNAUTHORIZED),
        (
            "restored epoch",
            |s| s.epoch = [0x33; 16],
            StatusCode::UNAUTHORIZED,
        ),
        // Not even a well-formed answer: fail closed as unavailable, not as a denial.
        (
            "another subject",
            |s| s.state_subject = Some(vec![0x44; 16]),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (
            "unknown state",
            |s| s.state_value = Some(0),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    ];
    for (label, mutate, expected) in races {
        let h = Harness::start().await;
        h.fake.set(mutate);
        let reply = h.login().await;
        assert_eq!(reply.status, expected, "{label}");
        assert!(reply.set_cookies().is_empty(), "{label}");
        assert_eq!(h.session_count().await, 0, "{label}");
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn every_security_change_revokes_durably_and_a_matching_snapshot_never_resurrects() {
    type Mutation = fn(&mut Script);
    let changes: [(&str, Mutation, &str); 4] = [
        ("version advanced", |s| s.version = 4, "version-advanced"),
        (
            "version regressed (restored older file)",
            |s| s.version = 2,
            "version-regressed",
        ),
        ("disabled", |s| s.active = false, "disabled"),
        (
            "restored instance",
            |s| s.epoch = [0x33; 16],
            "epoch-changed",
        ),
    ];
    for (label, mutate, reason) in changes {
        let h = Harness::start().await;
        let cookie = cookie_of(&h.login().await);
        assert_eq!(h.protected(&cookie).await, StatusCode::OK, "{label}");
        let original = h.fake.script.lock().unwrap().clone();
        h.fake.set(mutate);
        assert_eq!(
            h.protected(&cookie).await,
            StatusCode::UNAUTHORIZED,
            "{label}"
        );
        assert_eq!(
            h.revoked_reason().await,
            [Some(reason.to_owned())],
            "{label}"
        );
        // Identity looks exactly as it did at issue again (re-enabled, restored back): the family
        // stays dead, because revocation is a durable fact and not a comparison.
        h.fake.set(|s| *s = original);
        assert_eq!(
            h.protected(&cookie).await,
            StatusCode::UNAUTHORIZED,
            "{label}: resurrected"
        );
        assert_eq!(
            h.revoked_reason().await,
            [Some(reason.to_owned())],
            "{label}"
        );
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn a_slow_or_confused_identity_fails_closed_without_revoking_anything() {
    let h = Harness::start().await;
    let cookie = cookie_of(&h.login().await);
    assert_eq!(h.protected(&cookie).await, StatusCode::OK);
    let calls = h.fake.state_calls();
    // A reply later than the 3 s state deadline is an outage, not a security change.
    h.fake
        .set(|s| s.state_delay = std::time::Duration::from_secs(5));
    assert_eq!(h.protected(&cookie).await, StatusCode::SERVICE_UNAVAILABLE);
    h.fake.set(|s| {
        s.state_delay = std::time::Duration::ZERO;
        s.state_value = Some(0);
    });
    assert_eq!(h.protected(&cookie).await, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        h.revoked_reason().await,
        [None],
        "an outage revokes nothing"
    );
    h.fake.set(|s| s.state_value = None);
    assert_eq!(h.protected(&cookie).await, StatusCode::OK);
    // Every admission asked Identity: no verdict is cached beyond a single request.
    assert!(h.fake.state_calls() >= calls + 3);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn a_login_that_outlives_its_deadline_is_an_unknown_effect_never_a_session_or_a_retry() {
    let h = Harness::start().await;
    h.fake
        .set(|s| s.auth_delay = std::time::Duration::from_secs(14));
    let reply = h.login().await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(reply.set_cookies().is_empty());
    assert_eq!(h.session_count().await, 0);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn only_the_configured_peer_uid_is_trusted_as_identity() {
    let h = Harness::with_peer_uid(Some(0xfff0_0000)).await;
    h.fake.set(|_| {});
    let reply = h.login().await;
    assert_eq!(reply.status, StatusCode::SERVICE_UNAVAILABLE);
    assert!(reply.set_cookies().is_empty());
    assert_eq!(h.session_count().await, 0);
    assert_eq!(h.fake.state_calls(), 0);
    let _ = Subject::from_bytes([1; 16]);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn an_operator_bound_actor_is_reused_and_an_inactive_one_is_never_logged_into() {
    use cantos_server::platform::{
        identity::domain::{ActorId, ExternalIdentityKey},
        operator,
    };
    let h = Harness::start().await;
    let subject = Subject::from_bytes([0x11; 16]).unwrap();
    // The operator linked this exact Identity subject to an existing actor beforehand.
    let actor = operator::create_creator(&h.db.operator).await.unwrap();
    let key = ExternalIdentityKey::new("cantos-identity", subject.hex()).unwrap();
    operator::link_identity(&h.db.operator, &ActorId::new(actor.clone()).unwrap(), &key)
        .await
        .unwrap();
    let login = h.login().await;
    assert_eq!(login.status, StatusCode::OK, "{:?}", login.json);
    assert_eq!(
        login.json["actor_id"],
        json!(actor),
        "no second actor is minted"
    );
    let cookie = cookie_of(&login);
    assert_eq!(h.protected(&cookie).await, StatusCode::OK);
    let actors: i64 =
        h.db.admin
            .query_one("SELECT count(*) FROM actors", &[])
            .await
            .unwrap()
            .get(0);
    assert_eq!(actors, 1);

    // Deactivating the actor ends admission at once and refuses the next login, with no session.
    h.db.admin
        .execute("UPDATE actors SET active=false WHERE id=$1", &[&actor])
        .await
        .unwrap();
    assert_eq!(h.protected(&cookie).await, StatusCode::UNAUTHORIZED);
    let refused = h.login().await;
    assert_eq!(refused.status, StatusCode::UNAUTHORIZED);
    assert!(refused.set_cookies().is_empty());
    assert_eq!(h.session_count().await, 1);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn concurrent_first_logins_of_one_subject_bind_exactly_one_actor() {
    let h = Harness::start().await;
    let (a, b, c) = tokio::join!(h.login(), h.login(), h.login());
    for reply in [&a, &b, &c] {
        assert_eq!(reply.status, StatusCode::OK, "{:?}", reply.json);
    }
    assert_eq!(a.json["actor_id"], b.json["actor_id"]);
    assert_eq!(b.json["actor_id"], c.json["actor_id"]);
    let actors: i64 =
        h.db.admin
            .query_one("SELECT count(*) FROM actors", &[])
            .await
            .unwrap()
            .get(0);
    assert_eq!(actors, 1, "a losing candidate actor is never left behind");
    assert_eq!(h.session_count().await, 3);
}

impl Harness {
    async fn login_native(&self) -> Reply {
        self.app
            .post(
                &format!("{THEATRE}/native/identity/login"),
                &[],
                json!({"email": "owner@example.com", "password": "a correct horse battery"}),
            )
            .await
    }

    async fn login_studio(&self) -> Reply {
        self.app
            .post(
                "/api/v1/studio/identity/login",
                &[("origin", ORIGIN)],
                json!({"email": "owner@example.com", "password": "a correct horse battery"}),
            )
            .await
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn a_studio_secret_is_accepted_only_from_its_own_secure_cookie() {
    let h = Harness::with_studio().await;
    let login = h.login_studio().await;
    assert_eq!(login.status, StatusCode::OK, "{:?}", login.json);
    let (name, secret) = login.cookie();
    let head = "/api/v1/studio/scripts/00000000-0000-4000-8000-000000000001/head";
    assert_eq!(
        h.app
            .get(head, &[("cookie", &format!("{name}={secret}"))])
            .await
            .status,
        StatusCode::NOT_FOUND,
        "the genuine cookie authenticates (the script is simply absent)"
    );
    // The same secret under the legacy plain name, or the plain Identity spelling, never does.
    for forged in [
        format!("cantos_session={secret}"),
        format!("cantos_id_studio={secret}"),
        format!("__Secure-cantos_studio={secret}"),
    ] {
        assert_eq!(
            h.app.get(head, &[("cookie", &forged)]).await.status,
            StatusCode::UNAUTHORIZED,
            "{forged}"
        );
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn legacy_studio_session_logout_really_ends_an_identity_session() {
    let h = Harness::with_studio().await;
    let login = h.login_studio().await;
    let (name, secret) = login.cookie();
    let cookie = format!("{name}={secret}");
    let session = login.json["session_id"].as_str().unwrap().to_owned();
    let out = h
        .app
        .send(
            axum::http::Method::DELETE,
            "/api/v1/studio/session",
            &[("cookie", &cookie), ("origin", ORIGIN)],
            None,
        )
        .await;
    assert!(out.status.is_success(), "{:?}", out.status);
    assert_eq!(
        h.revoked_reason().await,
        [Some("logout".to_owned())],
        "{session}"
    );
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn an_unknown_subject_at_admission_is_a_durable_security_fact_not_an_outage() {
    let h = Harness::start().await;
    let cookie = cookie_of(&h.login().await);
    assert_eq!(h.protected(&cookie).await, StatusCode::OK);
    h.fake.set(|s| s.state_not_found = true);
    assert_eq!(h.protected(&cookie).await, StatusCode::UNAUTHORIZED);
    assert_eq!(
        h.revoked_reason().await,
        [Some("subject-missing".to_owned())]
    );
    h.fake.set(|s| s.state_not_found = false);
    assert_eq!(
        h.protected(&cookie).await,
        StatusCode::UNAUTHORIZED,
        "never resurrected"
    );
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn concurrent_refresh_of_one_secret_lets_one_win_and_revokes_the_family() {
    let h = Harness::start().await;
    let login = h.login_native().await;
    let body = json!({
        "refresh_token": login.json["refresh_token"],
        "session_id": login.json["session_id"],
    });
    let path = format!("{THEATRE}/native/identity/refresh");
    let (a, b) = tokio::join!(
        h.app.post(&path, &[], body.clone()),
        h.app.post(&path, &[], body)
    );
    let mut statuses = [a.status, b.status];
    statuses.sort();
    assert_eq!(statuses, [StatusCode::OK, StatusCode::UNAUTHORIZED]);
    // Replay containment: the winner's new pair is dead too, and the family says why.
    let winner = if a.status == StatusCode::OK { &a } else { &b };
    let auth = format!("Bearer {}", winner.json["access_token"].as_str().unwrap());
    let reply = h
        .app
        .get(
            &format!("{THEATRE}/library/continue"),
            &[("authorization", &auth)],
        )
        .await;
    assert_eq!(reply.status, StatusCode::UNAUTHORIZED);
    assert_eq!(h.revoked_reason().await, [Some("replay".to_owned())]);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn renew_racing_logout_always_ends_revoked_and_never_fails_with_a_server_error() {
    for _ in 0..4 {
        let h = Harness::start().await;
        let login = h.login().await;
        let cookie = cookie_of(&login);
        let headers = [("cookie", cookie.as_str()), ("origin", ORIGIN)];
        let renew_path = format!("{THEATRE}/identity/session/renew");
        let logout_path = format!("{THEATRE}/identity/session");
        let renew = h.app.post(&renew_path, &headers, json!(null));
        let logout = h
            .app
            .send(axum::http::Method::DELETE, &logout_path, &headers, None);
        let (renewed, out) = tokio::join!(renew, logout);
        assert!(
            [StatusCode::OK, StatusCode::UNAUTHORIZED].contains(&renewed.status),
            "{:?}",
            renewed.status
        );
        assert_eq!(out.status, StatusCode::NO_CONTENT);
        assert_eq!(h.protected(&cookie).await, StatusCode::UNAUTHORIZED);
        assert_eq!(h.revoked_reason().await, [Some("logout".to_owned())]);
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres"]
async fn the_runtime_role_cannot_weaken_or_rewrite_a_session_whatever_its_privileges() {
    let h = Harness::start().await;
    let native = h.login_native().await;
    let web = h.login().await;
    let runtime = world::connect(&h.db.runtime).await;
    let code = |result: Result<u64, tokio_postgres::Error>| {
        result.unwrap_err().code().unwrap().code().to_owned()
    };
    // No DELETE or TRUNCATE; no column outside the narrow UPDATE grants.
    for sql in [
        "DELETE FROM identity_sessions",
        "DELETE FROM identity_session_tokens",
        "UPDATE identity_sessions SET actor_id='someone-else'",
        "UPDATE identity_sessions SET absolute_expires=absolute_expires+1",
        "UPDATE identity_sessions SET observed_security_version=1",
        "UPDATE identity_session_tokens SET access_hash=refresh_hash",
        "UPDATE identity_session_tokens SET session_id=gen_random_uuid()",
    ] {
        assert_eq!(code(runtime.execute(sql, &[]).await), "42501", "{sql}");
    }
    // Granted columns are still guarded: the idle deadline never shrinks, a Web cookie has no
    // refresh secret to spend, and a spent refresh secret never becomes unspent.
    assert_eq!(
        code(
            runtime
                .execute(
                    "UPDATE identity_sessions SET idle_expires=idle_expires-1",
                    &[]
                )
                .await
        ),
        "23514"
    );
    assert_eq!(
        code(
            runtime
                .execute(
                    "UPDATE identity_session_tokens SET used=true WHERE refresh_hash IS NULL",
                    &[]
                )
                .await
        ),
        "23514"
    );
    runtime
        .execute(
            "UPDATE identity_session_tokens SET used=true WHERE refresh_hash IS NOT NULL",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        code(
            runtime
                .execute(
                    "UPDATE identity_session_tokens SET used=false WHERE refresh_hash IS NOT NULL",
                    &[]
                )
                .await
        ),
        "23514"
    );
    // Revocation is permanent and cannot be rewritten or extended.
    let session = native.json["session_id"].as_str().unwrap();
    runtime
        .execute(
            "UPDATE identity_sessions SET revoked=true,revoked_reason='logout' WHERE id=$1::text::uuid",
            &[&session],
        )
        .await
        .unwrap();
    for sql in [
        "UPDATE identity_sessions SET revoked=false,revoked_reason=NULL WHERE revoked",
        "UPDATE identity_sessions SET revoked_reason='replay' WHERE revoked",
        "UPDATE identity_sessions SET idle_expires=idle_expires+1 WHERE revoked",
    ] {
        assert_eq!(code(runtime.execute(sql, &[]).await), "23514", "{sql}");
    }
    // The registry function binds only the fixed issuer and only a well-formed subject.
    let bound: Option<String> = runtime
        .query_one(
            "SELECT identity_session_register('not-a-subject','candidate-x')",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(bound, None);
    let _ = web;
}
