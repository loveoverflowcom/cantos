//! Explicit local integration lane: mounted HTTP handlers, real enrollment/bootstrap/action
//! adapters, disposable PostgreSQL with deployed grants, and the owned Auth Emulator.
//! Only OAuth access-token acquisition and Kanidm are mocks. This is not browser/OIDC,
//! real-mail, IAM, real-project association, provider expiry or production acceptance.

use std::{
    collections::BTreeSet,
    sync::{Arc, Mutex},
    time::Duration,
};

use reqwest::{header, Client, Method, StatusCode};
use serde_json::{json, Value};
use tokio_postgres::{Config, NoTls};
use url::Url;
use uuid::Uuid;

use super::{
    application::FirebaseEnrollmentService, postgres::PostgresFirebaseEnrollmentRepository,
    runtime::FirebaseEnrollmentRuntime,
};
use crate::{
    interfaces::http::{router, AppState},
    platform::{
        postgres::{local_config, migrate, Store},
        theatre_identity::TheatreIdentity,
        theatre_registration::{
            application::test_support::FakeIssuer,
            domain::RegistrationPolicy,
            firebase_action::{emulator_tests::Emulator, FirebaseActionClient},
            firebase_bootstrap::{
                tests::service_account, FirebaseBootstrapClient, FirebaseBootstrapConfig,
                FirebaseMailboxProvider,
            },
            runtime::AttemptPolicy,
        },
    },
};

const ISSUER: &str = "https://issuer.example/oauth2/openid/cantos-theatre-web-bff";
const START: &str = "/api/v1/theatre/auth/registration";
const CONFIRM: &str = "/api/v1/theatre/auth/registration/firebase/confirm";
const CANCEL: &str = "/api/v1/theatre/auth/registration/firebase/cancel";
const GRANTS: &str =
    include_str!("../../../../../../infra/vps/postgres/theatre-registration-grants.sql");

/// Private recipient memory; neither Debug nor serialized output exposes the proof URL.
struct RecipientAction {
    operation: Uuid,
    code: String,
    cantos_url: Url,
}

impl std::fmt::Debug for RecipientAction {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("RecipientAction([REDACTED])")
    }
}

impl RecipientAction {
    /// Exact inventory delta + recipient/purpose/continuation match. Never dereference the
    /// emulator default handler: doing so can consume the action before Cantos confirms it.
    fn select(
        codes: &[Value],
        previous: &BTreeSet<String>,
        recipient: &str,
        web_origin: &str,
        emulator_address: std::net::SocketAddr,
        operation: Uuid,
    ) -> Option<Self> {
        if !recipient.ends_with(".test") || operation.is_nil() {
            return None;
        }
        let expected = format!("{web_origin}/?firebaseEnrollment={operation}");
        let mut matched = codes.iter().filter_map(|entry| {
            let code = entry["oobCode"].as_str()?;
            if previous.contains(code)
                || entry["email"].as_str()? != recipient
                || entry["requestType"].as_str()? != "VERIFY_EMAIL"
            {
                return None;
            }
            let issued = Url::parse(entry["oobLink"].as_str()?).ok()?;
            if issued.scheme() != "http"
                || issued.host_str() != Some(emulator_address.ip().to_string().as_str())
                || issued.port_or_known_default() != Some(emulator_address.port())
                || issued.path() != "/emulator/action"
                || !issued.username().is_empty()
                || issued.password().is_some()
                || issued.fragment().is_some()
            {
                return None;
            }
            let pairs: Vec<_> = issued.query_pairs().collect();
            let unique = |key: &str| {
                let mut values = pairs.iter().filter(|(name, _)| name == key);
                let value = values.next()?.1.as_ref();
                values.next().is_none().then_some(value)
            };
            if unique("mode")? != "verifyEmail"
                || unique("oobCode")? != code
                || unique("continueUrl")? != expected
            {
                return None;
            }
            let mut cantos_url = Url::parse(&format!("{web_origin}/")).ok()?;
            cantos_url
                .query_pairs_mut()
                .append_pair("mode", "verifyEmail")
                .append_pair("oobCode", code)
                .append_pair("continueUrl", &expected);
            Some(Self {
                operation,
                code: code.to_owned(),
                cantos_url,
            })
        });
        let action = matched.next()?;
        matched.next().is_none().then_some(action)
    }

    fn body(&self) -> Value {
        json!({"operation_id":self.operation.to_string(),"oob_code":self.code})
    }
}

fn inventory_ids(codes: &[Value]) -> BTreeSet<String> {
    codes
        .iter()
        .filter_map(|entry| entry["oobCode"].as_str().map(str::to_owned))
        .collect()
}

/// Fixture transport accepts an empty body only for the router's method refusal.
fn response_json(status: StatusCode, bytes: &[u8]) -> Result<Value, &'static str> {
    if status == StatusCode::METHOD_NOT_ALLOWED && bytes.is_empty() {
        Ok(Value::Null)
    } else {
        serde_json::from_slice(bytes).map_err(|_| "required JSON response")
    }
}

async fn connect(config: &Config) -> tokio_postgres::Client {
    let (client, connection) = config
        .connect(NoTls)
        .await
        .expect("owned PostgreSQL connection");
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

struct Harness {
    // Caller owns the entire disposable cluster; cluster teardown also covers test panic.
    admin: tokio_postgres::Client,
    emulator: Emulator,
    origin: String,
    http: Client,
    server: tokio::task::JoinHandle<()>,
    events: Arc<Mutex<Vec<&'static str>>>,
}

impl Drop for Harness {
    fn drop(&mut self) {
        self.server.abort();
    }
}

impl Harness {
    async fn new() -> Self {
        let project = format!("demo-cantos-{}", &Uuid::new_v4().simple().to_string()[..12]);
        let emulator = Emulator::start_project(&project);
        let cluster = std::env::var("CANTOS_TEST_CLUSTER_URL")
            .expect("xtask supplies a new disposable PostgreSQL cluster");
        let mut config = local_config(&cluster).expect("local fixture PostgreSQL target");
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let database = format!("cantos_test_{}", Uuid::new_v4().simple());
        connect(&config)
            .await
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .expect("create run-owned database");
        config.dbname(&database);
        migrate(&config).await.expect("real current migrations");
        let admin = connect(&config).await;
        admin
            .batch_execute(GRANTS)
            .await
            .expect("deployed registration grants");
        config.user("cantos_app");
        let store = Store::new(config).expect("app-role pool");
        let repository = Arc::new(
            PostgresFirebaseEnrollmentRepository::new(store.clone(), ISSUER, &project)
                .expect("real PostgreSQL enrollment repository"),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
            .await
            .expect("run-owned API listener");
        let address = listener.local_addr().expect("API socket address");
        let origin = format!("http://{address}");
        let mut account = service_account();
        account["project_id"] = json!(project);
        account["client_email"] = json!(format!("synthetic@{project}.iam.gserviceaccount.com"));
        let config = FirebaseBootstrapConfig::parse(
            &project,
            "synthetic-fixture-key",
            &serde_json::to_vec(&account).expect("public synthetic signing fixture"),
        )
        .expect("synthetic service-account parse");
        let mailbox = FirebaseMailboxProvider::new(
            FirebaseBootstrapClient::auth_emulator_fixture(
                config,
                emulator.address,
                Duration::from_secs(3),
            )
            .expect("cfg(test) loopback bootstrap transport"),
            &origin,
        )
        .expect("production mailbox adapter");
        let events = Arc::new(Mutex::new(Vec::new()));
        let service = Arc::new(
            FirebaseEnrollmentService::new(
                repository,
                Arc::new(mailbox),
                Arc::new(
                    FirebaseActionClient::owned_auth_emulator_fixture(
                        emulator.address,
                        &project,
                        Duration::from_secs(3),
                    )
                    .expect("cfg(test) loopback action transport"),
                ),
                Arc::new(FakeIssuer {
                    create_error: None,
                    setup_error: None,
                    create_waits: false,
                    mismatch: false,
                    events: events.clone(),
                }),
                &project,
                RegistrationPolicy::new(600, 60, 3600, 100, 100).expect("fixture policy"),
                ISSUER,
            )
            .expect("production enrollment service"),
        );
        let store = store
            .with_theatre_identity(
                TheatreIdentity::new(ISSUER, &origin, "isolated-integration-fixture".into())
                    .expect("same production session boundary; no fabricated sessions"),
            )
            .with_theatre_firebase_enrollment(
                FirebaseEnrollmentRuntime::new(
                    service,
                    &origin,
                    [7; 32],
                    8,
                    AttemptPolicy::new(60, 100, 100).expect("fixture admission policy"),
                )
                .expect("production admission boundary"),
            );
        let static_root = emulator.directory().join("web");
        std::fs::create_dir(&static_root).expect("run-owned empty static directory");
        let app = router(
            AppState {
                store,
                web_origin: origin.clone(),
            },
            static_root.to_str().expect("owned fixture directory path"),
        );
        let server = tokio::spawn(async move {
            let _ = axum::serve(
                listener,
                app.into_make_service_with_connect_info::<std::net::SocketAddr>(),
            )
            .await;
        });
        Self {
            admin,
            emulator,
            origin,
            http: Client::builder()
                .no_proxy()
                .redirect(reqwest::redirect::Policy::none())
                .timeout(Duration::from_secs(5))
                .build()
                .expect("bounded owned-loopback HTTP client"),
            server,
            events,
        }
    }

    async fn request(&self, method: Method, path: &str, body: Value) -> (StatusCode, Value) {
        // Only fixed server-relative paths enter this helper; never an arbitrary remote URL.
        assert!(path.starts_with('/') && !path.starts_with("//"));
        let request = self
            .http
            .request(method, format!("{}{path}", self.origin))
            .header(header::ORIGIN, &self.origin)
            .header(header::CONTENT_TYPE, "application/json")
            .body(serde_json::to_vec(&body).expect("fixture request JSON"));
        let response = request.send().await.expect("owned API request");
        assert!(
            !response.headers().contains_key(header::SET_COOKIE),
            "no session from registration"
        );
        assert!(
            !response.headers().contains_key(header::LOCATION),
            "no redirect handoff"
        );
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .expect("bounded registration response");
        assert!(bytes.len() <= 16_384, "bounded response");
        (
            status,
            response_json(status, &bytes).expect("registration response contract"),
        )
    }

    async fn phase(&self, operation: Uuid) -> String {
        self.admin.query_one(
            "SELECT phase FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
            &[&ISSUER, &operation.to_string()],
        ).await.expect("durable phase query").get(0)
    }

    async fn count_effect(&self, effect: &str) -> usize {
        self.events
            .lock()
            .expect("mock issuer events")
            .iter()
            .filter(|e| **e == effect)
            .count()
    }

    async fn start(&self, label: &str) -> RecipientAction {
        let recipient = format!("{label}-{}@example.test", Uuid::new_v4().simple());
        let previous = inventory_ids(&self.emulator.pending_codes().await);
        let (status, body) = self
            .request(Method::POST, START, json!({"email":recipient}))
            .await;
        assert_eq!(status, StatusCode::ACCEPTED);
        assert!(
            body == json!({"status":"accepted"}),
            "generic initiation receipt"
        );
        tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let row = self.admin.query_opt(
                    "SELECT id::text,phase FROM theatre_firebase_enrollments WHERE namespace=$1 AND email=$2",
                    &[&ISSUER, &recipient],
                ).await.expect("owned enrollment query");
                if let Some(row) = row {
                    let phase: String = row.get(1);
                    assert!(phase != "blocked", "bootstrap/send failed; provider body remains private");
                    if phase == "awaiting_action" {
                        let id: String = row.get(0);
                        let operation = Uuid::parse_str(&id).expect("durable operation identity");
                        if let Some(action) = RecipientAction::select(
                            &self.emulator.pending_codes().await,
                            &previous,
                            &recipient,
                            &self.origin,
                            self.emulator.address,
                            operation,
                        ) {
                            return action;
                        }
                    }
                }
                tokio::time::sleep(Duration::from_millis(20)).await;
            }
        }).await.expect("bounded emitted action and durable send deadline")
    }

    async fn anonymous(&self) {
        let (status, body) = self
            .request(Method::GET, "/api/v1/theatre/session", Value::Null)
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(body["code"], "unauthenticated");
    }
}

#[tokio::test]
#[ignore = "explicit owned Auth Emulator + disposable PostgreSQL, mock Kanidm; not Web/OIDC"]
async fn owned_auth_emulator_registration_http_and_postgres_fences() {
    let fixture = Harness::new().await;
    let action = fixture.start("recipient-b").await;
    assert_eq!(fixture.count_effect("create").await, 0);
    assert_eq!(fixture.count_effect("setup").await, 0);
    fixture.anonymous().await;
    let binding = fixture.admin.query_one(
        "SELECT email,uid,expires_at::text FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
        &[&ISSUER, &action.operation.to_string()],
    ).await.expect("immutable enrollment binding");
    let recipient: String = binding.get(0);
    let uid: String = binding.get(1);
    let expiry: String = binding.get(2);
    let inventory_before = inventory_ids(&fixture.emulator.pending_codes().await);
    let (status, receipt) = fixture
        .request(Method::POST, START, json!({"email":recipient}))
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(
        receipt == json!({"status":"accepted"}),
        "cooldown initiation stays generic"
    );
    let stable = fixture.admin.query_one(
        "SELECT uid=$3,expires_at::text=$4,(SELECT count(*) FROM theatre_firebase_send_starts WHERE namespace=$1 AND enrollment_id=$2::text::uuid) FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
        &[&ISSUER, &action.operation.to_string(), &uid, &expiry],
    ).await.expect("cooldown preserves original UID, deadline and durable send budget");
    assert!(stable.get::<_, bool>(0));
    assert!(stable.get::<_, bool>(1));
    assert_eq!(stable.get::<_, i64>(2), 1);
    assert!(
        inventory_before == inventory_ids(&fixture.emulator.pending_codes().await),
        "cooldown never sends another action"
    );

    // The recipient's action page GET, scanner/reload and guessed/empty action requests
    // have no authority. A's initiating receipt contains no operation or polling secret.
    let before = fixture.emulator.pending_codes().await;
    for _ in 0..2 {
        let response = fixture
            .http
            .get(action.cantos_url.clone())
            .send()
            .await
            .unwrap_or_else(|_| panic!("private action-page request failed"));
        assert!(matches!(
            response.status(),
            StatusCode::NOT_FOUND | StatusCode::OK
        ));
    }
    for path in [CONFIRM, CANCEL] {
        let (status, _) = fixture.request(Method::GET, path, Value::Null).await;
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
    }
    for body in [
        json!({"operation_id":Uuid::new_v4().to_string(),"oob_code":action.code}),
        json!({"operation_id":action.operation.to_string(),"oob_code":"wrong-code"}),
    ] {
        let (status, result) = fixture.request(Method::POST, CONFIRM, body).await;
        assert_eq!(status, StatusCode::OK);
        assert!(
            result == json!({"status":"unavailable"}),
            "invalid proof has no handoff"
        );
    }
    assert_eq!(fixture.count_effect("create").await, 0);
    assert_eq!(fixture.phase(action.operation).await, "awaiting_action");
    assert!(
        inventory_ids(&before) == inventory_ids(&fixture.emulator.pending_codes().await),
        "scanner requests cannot alter private inventory"
    );

    let (left, right) = tokio::join!(
        fixture.request(Method::POST, CONFIRM, action.body()),
        fixture.request(Method::POST, CONFIRM, action.body()),
    );
    assert_eq!(left.0, StatusCode::OK);
    assert_eq!(right.0, StatusCode::OK);
    let outcomes = [left.1, right.1];
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| v["status"] == "setup_ready")
            .count(),
        1
    );
    assert_eq!(
        outcomes
            .iter()
            .filter(|v| v["status"] == "unavailable")
            .count(),
        1
    );
    assert_eq!(fixture.count_effect("create").await, 1);
    assert_eq!(fixture.count_effect("setup").await, 1);
    assert_eq!(fixture.phase(action.operation).await, "handed_off");
    fixture.anonymous().await;
    let (_, replay) = fixture.request(Method::POST, CONFIRM, action.body()).await;
    assert!(
        replay == json!({"status":"unavailable"}),
        "replay has no handoff"
    );

    // A different/default action handler already consumed this proof. Historical
    // emailVerified=true never substitutes for possession of an unconsumed action.
    let consumed = fixture.start("preconsumed").await;
    fixture
        .emulator
        .action("update", json!({"oobCode":consumed.code}))
        .await;
    let (_, rejected) = fixture
        .request(Method::POST, CONFIRM, consumed.body())
        .await;
    assert!(
        rejected == json!({"status":"unavailable"}),
        "unavailable proof has no handoff"
    );
    assert_eq!(fixture.phase(consumed.operation).await, "awaiting_action");
    assert_eq!(fixture.count_effect("create").await, 1);

    let cancelled = fixture.start("cancelled").await;
    let (status, receipt) = fixture
        .request(Method::POST, CANCEL, cancelled.body())
        .await;
    assert_eq!(status, StatusCode::ACCEPTED);
    assert!(
        receipt == json!({"status":"accepted"}),
        "generic cancellation receipt"
    );
    assert_eq!(fixture.phase(cancelled.operation).await, "cancelled");
    let (_, rejected) = fixture
        .request(Method::POST, CONFIRM, cancelled.body())
        .await;
    assert!(
        rejected == json!({"status":"unavailable"}),
        "unavailable proof has no handoff"
    );
    assert_eq!(fixture.count_effect("create").await, 1);
    fixture.anonymous().await;
}

#[test]
fn recipient_inventory_requires_unique_new_exactly_bound_verification_action() {
    let origin = "http://127.0.0.1:9099";
    let operation = Uuid::from_u128(1);
    let recipient = "recipient@example.test";
    let continuation = format!("{origin}/?firebaseEnrollment={operation}");
    let mut issued = Url::parse("http://127.0.0.1:9199/emulator/action").unwrap();
    issued
        .query_pairs_mut()
        .append_pair("mode", "verifyEmail")
        .append_pair("oobCode", "private-code")
        .append_pair("continueUrl", &continuation);
    let entry = json!({"email":recipient,"requestType":"VERIFY_EMAIL","oobCode":"private-code","oobLink":issued.as_str()});
    let select = |entries: &[Value], previous: &BTreeSet<String>| {
        RecipientAction::select(
            entries,
            previous,
            recipient,
            origin,
            "127.0.0.1:9199".parse().unwrap(),
            operation,
        )
    };
    let action = select(std::slice::from_ref(&entry), &BTreeSet::new()).unwrap();
    assert_eq!(format!("{action:?}"), "RecipientAction([REDACTED])");
    assert_eq!(action.cantos_url.origin().ascii_serialization(), origin);
    assert!(select(
        std::slice::from_ref(&entry),
        &BTreeSet::from(["private-code".into()])
    )
    .is_none());
    assert!(select(&[entry.clone(), entry.clone()], &BTreeSet::new()).is_none());
    for (field, value) in [
        ("email", "other@example.test"),
        ("requestType", "PASSWORD_RESET"),
        ("oobCode", "mismatched-code"),
        (
            "oobLink",
            "http://127.0.0.1:9199/?mode=verifyEmail&oobCode=private-code&continueUrl=wrong",
        ),
        ("oobLink", "https://127.0.0.1:9199/emulator/action"),
        ("oobLink", "http://127.0.0.2:9199/emulator/action"),
        ("oobLink", "http://127.0.0.1:9299/emulator/action"),
        ("oobLink", "http://user@127.0.0.1:9199/emulator/action"),
    ] {
        let mut wrong = entry.clone();
        wrong[field] = json!(value);
        assert!(
            select(&[wrong], &BTreeSet::new()).is_none(),
            "reject exact binding mismatch"
        );
    }
}

#[tokio::test]
async fn real_router_scanner_refusals_and_anonymous_session_match_fixture_response_contract() {
    use axum::{body::Body, http::Request};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    // A lazy, unreachable pool makes any accidental database dependency fail this check.
    let store = Store::new(Config::new()).unwrap().with_theatre_identity(
        TheatreIdentity::new(ISSUER, "http://127.0.0.1:9099", "contract-fixture".into()).unwrap(),
    );
    let app = router(
        AppState {
            store,
            web_origin: "http://127.0.0.1:9099".into(),
        },
        "target/cantos-no-static-contract-fixture",
    );
    for path in [CONFIRM, CANCEL] {
        let response = app
            .clone()
            .oneshot(Request::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        assert_eq!(status, StatusCode::METHOD_NOT_ALLOWED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
        assert!(!response.headers().contains_key(header::SET_COOKIE));
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        assert!(bytes.is_empty(), "Axum's method refusal has an empty body");
        assert_eq!(response_json(status, &bytes), Ok(Value::Null));
    }
    let response = app
        .oneshot(
            Request::get("/api/v1/theatre/session")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    assert!(
        !bytes.is_empty(),
        "anonymous sessions retain the JSON error contract"
    );
    let body = response_json(status, &bytes).unwrap();
    assert_eq!(body["code"], "unauthenticated");
    for status in [StatusCode::OK, StatusCode::UNAUTHORIZED] {
        assert_eq!(response_json(status, b""), Err("required JSON response"));
    }
}
