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
        Self::with_peer_uid(None).await
    }

    async fn with_peer_uid(peer_uid: Option<u32>) -> Self {
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
        let app = App::new(&db, ORIGIN, socket, peer_uid.unwrap_or(uid), &[]);
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
