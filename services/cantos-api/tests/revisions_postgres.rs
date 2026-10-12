//! Real boundary tests. Only the disposable-cluster runner enables these ignored tests.
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use cantos_api::{
    http::{router, AppState},
    postgres::{local_config, migrate, token_hash, Store, StoreError},
    script_ir::read_script,
};
use cantos_contracts::{
    wire::{decode, encode, WireFormat},
    ReviewRequest, RevisionResponse, SaveRevisionRequest, ScriptValidationResponse,
    ValidateScriptRequest,
};
use http_body_util::BodyExt;
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};
use std::{
    env, fs,
    time::{Duration, Instant},
};
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "http://127.0.0.1:8080";
const FIXTURE: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");

struct Harness {
    admin: Client,
    admin_config: Config,
    store: Store,
    app: Router,
    database: String,
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        connection.await.unwrap_or_else(|error| {
            eprintln!(
                "test connection closed: {}",
                error.code().map(|code| code.code()).unwrap_or("transport")
            )
        });
    });
    client
}

impl Harness {
    async fn new(name: Option<&str>) -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("use scripts/test_postgres.py");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let cluster = connect(&config).await;
        let database = name
            .map(str::to_owned)
            .unwrap_or_else(|| format!("cantos_test_{}", Uuid::new_v4().simple()));
        assert!(
            database.starts_with("cantos_test_")
                && database
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
        );
        cluster
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .unwrap();
        config.dbname(&database);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin.batch_execute("GRANT CONNECT ON DATABASE postgres TO cantos_app; GRANT USAGE ON SCHEMA public TO cantos_app; GRANT SELECT ON actors,sessions,script_members,script_evidence,script_revisions,revision_evidence,scripts,source_records,script_reviews,script_review_operations TO cantos_app; GRANT INSERT ON scripts,script_revisions,revision_evidence,script_reviews,script_review_operations TO cantos_app; GRANT UPDATE(head_revision) ON scripts TO cantos_app; GRANT UPDATE(revoked) ON sessions TO cantos_app; INSERT INTO actors(id) VALUES('alice'),('bob');").await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,CURRENT_TIMESTAMP+interval '1 hour')", &[&token_hash(token),&actor]).await.unwrap();
            for (kind, id) in read_script(FIXTURE.as_bytes()).unwrap().evidence_refs() {
                admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,$2,$3,'synthetic fixture reference; no publication clearance')", &[&actor,&kind,&id]).await.unwrap();
            }
            admin.execute("INSERT INTO script_evidence VALUES($1,'rights','rights-new','synthetic changed rights reference')", &[&actor]).await.unwrap();
        }
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-test");
        let store = Store::new(app_config).unwrap();
        let app = router(
            AppState {
                store: store.clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        );
        Self {
            admin,
            admin_config: config,
            store,
            app,
            database,
        }
    }
    async fn count(&self, script: &str) -> (i64, i64) {
        let row = self.admin.query_one("SELECT head_revision,(SELECT count(*) FROM script_revisions WHERE script_id=$1) FROM scripts WHERE id=$1", &[&script]).await.unwrap();
        (row.get(0), row.get(1))
    }
    async fn request(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/v1/studio{path}"))
            .header("origin", ORIGIN)
            .header("content-type", "application/json")
            .header("x-actor-id", "alice");
        if let Some(token) = token {
            request = request.header("cookie", format!("cantos_session={token}"));
        }
        let response = self
            .app
            .clone()
            .oneshot(
                request
                    .body(Body::from(
                        body.map(|value| value.to_string()).unwrap_or_default(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let value: Value = serde_json::from_slice(&bytes).unwrap();
        let definition = if !status.is_success() {
            "ApiError"
        } else if path.contains("/history") {
            "History"
        } else if path.ends_with("/reviews") {
            "Review"
        } else if path.contains("/sources/") {
            "Source"
        } else {
            "Revision"
        };
        validate_response(&value, definition);
        (status, value)
    }
}

const MSGPACK: &str = "application/msgpack";

/// One exchange with explicit representation headers; every API response keeps its headers.
struct Exchange {
    status: StatusCode,
    headers: HeaderMap,
    body: Vec<u8>,
}

impl Exchange {
    /// A MessagePack success with the negotiated headers, decoded as `T`.
    fn packed<T: DeserializeOwned>(&self) -> T {
        assert_eq!(
            self.status,
            StatusCode::OK,
            "{:?}",
            String::from_utf8_lossy(&self.body)
        );
        assert_eq!(self.headers["content-type"], MSGPACK);
        assert_eq!(self.headers["vary"], "accept");
        decode(WireFormat::MessagePack, &self.body).unwrap()
    }

    /// Every failure stays a JSON ApiError, whatever the request asked for.
    fn error(&self, status: StatusCode) -> Value {
        assert_eq!(self.status, status);
        assert_eq!(self.headers["content-type"], "application/json");
        let value: Value = serde_json::from_slice(&self.body).unwrap();
        validate_response(&value, "ApiError");
        value
    }
}

fn packed<T: Serialize>(value: &T) -> Vec<u8> {
    encode(WireFormat::MessagePack, value).unwrap()
}

impl Harness {
    async fn exchange(
        &self,
        origin: &str,
        method: &str,
        path: &str,
        token: Option<&str>,
        (content_type, accept): (Option<&str>, Option<&str>),
        body: Vec<u8>,
    ) -> Exchange {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/v1/studio{path}"))
            .header("origin", origin);
        for (name, value) in [("content-type", content_type), ("accept", accept)] {
            if let Some(value) = value {
                request = request.header(name, value);
            }
        }
        if let Some(token) = token {
            request = request.header("cookie", format!("cantos_session={token}"));
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        Exchange {
            status: response.status(),
            headers: response.headers().clone(),
            body: response
                .into_body()
                .collect()
                .await
                .unwrap()
                .to_bytes()
                .to_vec(),
        }
    }

    async fn msgpack(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Vec<u8>,
    ) -> Exchange {
        let content_type = (!body.is_empty()).then_some(MSGPACK);
        self.exchange(
            ORIGIN,
            method,
            path,
            token,
            (content_type, Some(MSGPACK)),
            body,
        )
        .await
    }

    async fn reviews(&self, script: &str) -> i64 {
        self.admin
            .query_one(
                "SELECT count(*) FROM script_reviews WHERE script_id=$1",
                &[&script],
            )
            .await
            .unwrap()
            .get(0)
    }
}

fn validate_response(value: &Value, definition: &str) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/studio/v1.schema.json"
    ))
    .unwrap();
    let schema = json!({"$ref":format!("#/$defs/{definition}"),"$defs":schema["$defs"]});
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}

fn request(base: u64) -> SaveRevisionRequest {
    SaveRevisionRequest {
        expected_revision: base,
        operation_id: Uuid::new_v4().to_string(),
        script_json: FIXTURE.into(),
    }
}

fn changed_rights(mut request: SaveRevisionRequest) -> SaveRevisionRequest {
    let mut value: Value = serde_json::from_str(&request.script_json).unwrap();
    value["work"]["rights_record_id"] = json!("rights-new");
    request.script_json = value.to_string();
    request
}

fn review_request(revision: u64) -> Value {
    json!({"operation_id": Uuid::new_v4().to_string(), "revision": revision})
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn editorial_reviews_are_pinned_replayable_owner_only_and_history_is_bounded() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let path = format!("/scripts/{script}/reviews");
    let intent = review_request(1);
    let (status, reviewed) = h
        .request("POST", &path, Some(ALICE), Some(intent.clone()))
        .await;
    assert_eq!(status, StatusCode::OK);
    h.store.save(ALICE, &script, request(1)).await.unwrap();
    h.store.save(ALICE, &script, request(2)).await.unwrap();
    let (status, replay) = h
        .request("POST", &path, Some(ALICE), Some(intent.clone()))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(reviewed, replay);
    assert_eq!(
        h.request("POST", &path, Some(ALICE), Some(review_request(1)))
            .await
            .1,
        reviewed
    );
    let mut changed = intent;
    changed["revision"] = json!(3);
    let (status, error) = h.request("POST", &path, Some(ALICE), Some(changed)).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(error["code"], "operation_reused");
    let (status, stale) = h
        .request("POST", &path, Some(ALICE), Some(review_request(2)))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(stale["current_revision"], 3);
    let intent = review_request(3);
    let (first, second) = tokio::join!(
        h.request("POST", &path, Some(ALICE), Some(intent.clone())),
        h.request("POST", &path, Some(ALICE), Some(intent)),
    );
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(first, second);
    for token in [None, Some(BOB)] {
        let status = h
            .request("GET", &format!("/scripts/{script}/history"), token, None)
            .await
            .0;
        assert_eq!(
            status,
            if token.is_none() {
                StatusCode::UNAUTHORIZED
            } else {
                StatusCode::NOT_FOUND
            }
        );
    }
    h.admin
        .execute(
            "INSERT INTO script_members VALUES($1,'bob','editor')",
            &[&script],
        )
        .await
        .unwrap();
    assert_eq!(
        h.request("POST", &path, Some(BOB), Some(review_request(3)))
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, page) = h
        .request(
            "GET",
            &format!("/scripts/{script}/history?limit=1"),
            Some(BOB),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["next_after"], 1);
    assert_eq!(page["reviews"], json!([reviewed]));
    assert!(page["revisions"][0].get("script_json").is_none());
    let (_, page) = h
        .request(
            "GET",
            &format!("/scripts/{script}/history?after_revision=1&limit=2"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(page["revisions"].as_array().unwrap().len(), 2);
    assert_eq!(page["reviews"].as_array().unwrap().len(), 1);
    assert_eq!(page["next_after"], Value::Null);
    for query in [
        "limit=0",
        "limit=51",
        "limit=no",
        "after_revision=9223372036854775808",
        "extra=1",
    ] {
        assert_eq!(
            h.request(
                "GET",
                &format!("/scripts/{script}/history?{query}"),
                Some(ALICE),
                None
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    h.admin
        .execute("DELETE FROM script_members WHERE script_id=$1", &[&script])
        .await
        .unwrap();
    assert_eq!(
        h.request(
            "GET",
            &format!("/scripts/{script}/history"),
            Some(BOB),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    let rows = h.admin.query_one("SELECT (SELECT count(*) FROM script_reviews),(SELECT count(*) FROM script_review_operations)", &[]).await.unwrap();
    assert_eq!(rows.get::<_, i64>(0), 2);
    assert_eq!(rows.get::<_, i64>(1), 3);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn source_bytes_operator_credentials_and_review_rollback_cross_real_boundaries() {
    use cantos_api::operator;
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let original = "Người dẫn chuyện\r\n  Bản thảo gốc.\n";
    operator::record_source(
        &h.admin_config,
        "alice",
        "source-demo",
        "original synthetic source",
        original,
    )
    .await
    .unwrap();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let path = format!("/scripts/{script}/sources/source-demo");
    let (status, source) = h.request("GET", &path, Some(ALICE), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(source["original_text"], original);
    use sha2::{Digest, Sha256};
    assert_eq!(
        source["sha256"],
        format!("{:x}", Sha256::digest(original.as_bytes()))
    );
    assert_eq!(
        h.request("GET", &path, Some(BOB), None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        h.request("GET", &path, None, None).await.0,
        StatusCode::UNAUTHORIZED
    );
    operator::record_source(
        &h.admin_config,
        "alice",
        "unlinked",
        "not part of this script",
        "private other source",
    )
    .await
    .unwrap();
    assert_eq!(
        h.request(
            "GET",
            &format!("/scripts/{script}/sources/unlinked"),
            Some(ALICE),
            None
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    assert!(operator::record_source(
        &h.admin_config,
        "alice",
        "source-demo",
        "changed",
        "changed"
    )
    .await
    .is_err());
    assert_eq!(
        h.store
            .source(ALICE, &script, "source-demo")
            .await
            .unwrap()
            .original_text,
        original
    );

    let review_path = format!("/scripts/{script}/reviews");
    let intent = review_request(1);
    h.admin.batch_execute("CREATE TRIGGER injected_receipt_failure BEFORE INSERT ON script_review_operations FOR EACH ROW EXECUTE FUNCTION reject_settled_change()").await.unwrap();
    assert_eq!(
        h.request("POST", &review_path, Some(ALICE), Some(intent.clone()))
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM script_reviews", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    h.admin
        .batch_execute("DROP TRIGGER injected_receipt_failure ON script_review_operations")
        .await
        .unwrap();
    assert_eq!(
        h.request("POST", &review_path, Some(ALICE), Some(intent))
            .await
            .0,
        StatusCode::OK
    );
    for table in [
        "source_records",
        "script_reviews",
        "script_review_operations",
    ] {
        for sql in [
            format!("DELETE FROM {table}"),
            format!("TRUNCATE {table} CASCADE"),
        ] {
            assert_eq!(
                h.admin
                    .batch_execute(&sql)
                    .await
                    .unwrap_err()
                    .code()
                    .unwrap()
                    .code(),
                "55000"
            );
        }
    }
    for sql in [
        "UPDATE source_records SET original_text='changed'",
        "UPDATE script_reviews SET reviewed_by='bob'",
        "UPDATE script_review_operations SET actor_id='bob'",
    ] {
        assert_eq!(
            h.admin
                .batch_execute(sql)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "55000"
        );
    }
    h.admin.batch_execute("ALTER TABLE source_records DISABLE TRIGGER sources_immutable; UPDATE source_records SET original_text='corrupted' WHERE id='source-demo'; ALTER TABLE source_records ENABLE TRIGGER sources_immutable").await.unwrap();
    assert_eq!(
        h.request("GET", &path, Some(ALICE), None).await.0,
        StatusCode::INTERNAL_SERVER_ERROR
    );

    // Exercise the actual CLI without printing its generated credential to diagnostics.
    let url = env::var("CANTOS_TEST_CLUSTER_URL")
        .unwrap()
        .replace("/postgres", &format!("/{}", h.database));
    let cli = |args: &[&str]| {
        std::process::Command::new(env!("CARGO_BIN_EXE_cantos-operator"))
            .env("DATABASE_URL", &url)
            .args(args)
            .output()
            .unwrap()
    };
    let creator = cli(&["create-creator"]);
    assert!(creator.status.success());
    let creator = String::from_utf8(creator.stdout).unwrap();
    let token = cli(&["issue-token", creator.trim(), "1"]);
    assert!(token.status.success());
    let token = String::from_utf8(token.stdout).unwrap();
    let token = token.trim();
    assert_eq!(token.len(), 64);
    assert!(token
        .bytes()
        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)));
    assert_eq!(h.store.authenticate(token).await.unwrap(), creator.trim());
    assert!(cli(&["revoke-tokens", creator.trim()]).status.success());
    assert!(matches!(
        h.store.authenticate(token).await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(!cli(&["issue-token", creator.trim(), "0"]).status.success());
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn additive_migrations_upgrade_existing_revision_bytes_and_roll_back_failed_extension() {
    let config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let cluster = connect(&config).await;
    let name = format!("cantos_test_{}", Uuid::new_v4().simple());
    cluster
        .batch_execute(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    let mut config = config;
    config.dbname(&name);
    let admin = connect(&config).await;
    let migration = include_str!("../migrations/0001_script_revisions.sql");
    admin.batch_execute(migration).await.unwrap();
    admin.batch_execute("CREATE TABLE cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL); INSERT INTO actors VALUES('alice',true)").await.unwrap();
    admin
        .execute(
            "INSERT INTO cantos_migrations VALUES(1,$1)",
            &[&token_hash(migration)],
        )
        .await
        .unwrap();
    admin
        .execute(
            "INSERT INTO sessions VALUES($1,'alice',CURRENT_TIMESTAMP+interval '1 hour',false)",
            &[&token_hash(ALICE)],
        )
        .await
        .unwrap();
    for (kind, id) in read_script(FIXTURE.as_bytes()).unwrap().evidence_refs() {
        admin
            .execute(
                "INSERT INTO script_evidence VALUES('alice',$1,$2,'synthetic evidence')",
                &[&kind, &id],
            )
            .await
            .unwrap();
    }
    let store = Store::new(config.clone()).unwrap();
    let script = Uuid::new_v4().to_string();
    let saved = store.save(ALICE, &script, request(0)).await.unwrap();
    admin
        .batch_execute("CREATE TABLE source_records(collision text)")
        .await
        .unwrap();
    assert!(matches!(
        migrate(&config).await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(
        admin
            .query_one("SELECT count(*) FROM cantos_migrations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    assert_eq!(
        admin
            .query_one("SELECT to_regclass('script_reviews')::text", &[])
            .await
            .unwrap()
            .get::<_, Option<String>>(0),
        None
    );
    admin
        .batch_execute("DROP TABLE source_records")
        .await
        .unwrap();
    migrate(&config).await.unwrap();
    migrate(&config).await.unwrap();
    assert_eq!(store.load(ALICE, &script, Some(1)).await.unwrap(), saved);
    assert_eq!(
        admin
            .query_one("SELECT count(*) FROM cantos_migrations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        9
    );
    assert_eq!(
        admin
            .query_one("SELECT to_regclass('durable_runs')::text", &[])
            .await
            .unwrap()
            .get::<_, Option<String>>(0),
        Some("durable_runs".into())
    );
    assert_eq!(
        admin
            .query_one("SELECT to_regclass('speech_clips')::text", &[])
            .await
            .unwrap()
            .get::<_, Option<String>>(0),
        Some("speech_clips".into())
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn migration_replay_is_checked_and_failed_ddl_rolls_back() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let saved = h.store.save(ALICE, &script, request(0)).await.unwrap();
    migrate(&h.admin_config).await.unwrap();
    assert_eq!(h.store.load(ALICE, &script, Some(1)).await.unwrap(), saved);
    h.admin
        .execute(
            "UPDATE cantos_migrations SET checksum=$1",
            &[&vec![0u8; 32]],
        )
        .await
        .unwrap();
    assert!(matches!(
        migrate(&h.admin_config).await,
        Err(StoreError::CorruptRevision)
    ));
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let cluster = connect(&config).await;
    let name = format!("cantos_test_{}", Uuid::new_v4().simple());
    cluster
        .batch_execute(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    config.dbname(&name);
    let admin = connect(&config).await;
    admin
        .batch_execute("CREATE TABLE script_revisions(collision text)")
        .await
        .unwrap();
    assert!(matches!(
        migrate(&config).await,
        Err(StoreError::Unavailable)
    ));
    let row = admin
        .query_one(
            "SELECT to_regclass('actors')::text,to_regclass('cantos_migrations')::text",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, Option<String>>(0), None);
    assert_eq!(row.get::<_, Option<String>>(1), None);
    admin
        .batch_execute("DROP TABLE script_revisions")
        .await
        .unwrap();
    migrate(&config).await.unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn complete_exports_metadata_retries_and_pinned_history_survive_edits() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let first_request = request(0);
    let first = h
        .store
        .save(ALICE, &script, first_request.clone())
        .await
        .unwrap();
    let fixture: RevisionResponse = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/studio/v1/revision.json"
    ))
    .unwrap();
    assert_eq!(first.script_json, fixture.script_json);
    assert_eq!(first.export_digest, fixture.export_digest);
    assert_eq!(
        first.script_json.as_bytes(),
        read_script(FIXTURE.as_bytes()).unwrap().export_bytes()
    );
    let second = h
        .store
        .save(ALICE, &script, changed_rights(request(1)))
        .await
        .unwrap();
    assert_eq!(first.content_digest, second.content_digest);
    assert_ne!(first.export_digest, second.export_digest);
    assert_ne!(first.script_json, second.script_json);
    assert_eq!(
        h.store
            .save(ALICE, &script, first_request.clone())
            .await
            .unwrap(),
        first
    );
    assert_eq!(h.store.load(ALICE, &script, Some(1)).await.unwrap(), first);
    assert_eq!(h.store.load(ALICE, &script, None).await.unwrap(), second);
    assert!(matches!(
        h.store
            .save(ALICE, &script, changed_rights(first_request.clone()))
            .await,
        Err(StoreError::OperationReused)
    ));
    let mut wrong_base = first_request;
    wrong_base.expected_revision = 1;
    assert!(matches!(
        h.store.save(ALICE, &script, wrong_base).await,
        Err(StoreError::OperationReused)
    ));
    assert!(matches!(
        h.store.save(ALICE, &script, request(1)).await,
        Err(StoreError::StaleRevision(2))
    ));
    assert_eq!(h.count(&script).await, (2, 2));
    let evidence_count: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM revision_evidence WHERE script_id=$1",
            &[&script],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(evidence_count, 11);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn backend_denies_anonymous_wrong_owner_reader_revoked_and_expired_sessions() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let saved = h.store.save(ALICE, &script, request(0)).await.unwrap();
    let head = format!("/scripts/{script}/head");
    let writes = format!("/scripts/{script}/revisions");
    let (status, body) = h.request("GET", &head, None, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["code"], "unauthenticated");
    assert_eq!(
        h.request("POST", &writes, None, Some(json!({"invalid":"body"})))
            .await
            .0,
        StatusCode::UNAUTHORIZED
    );
    for path in [&head, &format!("/scripts/{script}/revisions/1")] {
        let (status, body) = h.request("GET", path, Some(BOB), None).await;
        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body["code"], "not_found");
        assert!(!body.to_string().contains("Ánh đèn"));
    }
    assert_eq!(
        h.request(
            "POST",
            &writes,
            Some(BOB),
            Some(serde_json::to_value(request(1)).unwrap())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
    h.admin
        .execute(
            "INSERT INTO script_members VALUES($1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    let (status, body) = h.request("GET", &head, Some(BOB), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(body).unwrap(),
        saved
    );
    assert_eq!(
        h.request(
            "POST",
            &writes,
            Some(BOB),
            Some(serde_json::to_value(request(1)).unwrap())
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    h.admin
        .execute(
            "UPDATE script_members SET role='editor' WHERE script_id=$1",
            &[&script],
        )
        .await
        .unwrap();
    let edited = h.store.save(BOB, &script, request(1)).await.unwrap();
    assert_eq!(edited.accepted_by, "bob");
    h.admin
        .execute("DELETE FROM script_members WHERE script_id=$1", &[&script])
        .await
        .unwrap();
    assert_eq!(
        h.request("GET", &head, Some(BOB), None).await.0,
        StatusCode::NOT_FOUND
    );
    h.admin.execute("UPDATE sessions SET expires_at=CURRENT_TIMESTAMP-interval '1 second' WHERE actor_id='bob'",&[]).await.unwrap();
    assert_eq!(
        h.request("GET", &head, Some(BOB), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    h.admin
        .execute(
            "UPDATE sessions SET revoked=true WHERE actor_id='alice'",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(
        h.request("GET", &head, Some(ALICE), None).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn invalid_exports_and_foreign_evidence_leave_no_partial_script() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let mut invalid = request(0);
    invalid.script_json = "{}".into();
    assert!(matches!(
        h.store.save(ALICE, &script, invalid).await,
        Err(StoreError::InvalidScript(_))
    ));
    // Every input field fits its scalar bound, but NFC doubles the whole export past 2 MiB.
    // This must be a typed admission failure, rather than a SQL CHECK failure mapped to 503.
    let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
    let lines = value["episode"]["acts"][0]["scenes"][0]["dialogues"]
        .as_array_mut()
        .unwrap();
    for number in 0..110 {
        lines.push(
            json!({"id":format!("expansion-{number}"),"speaker_id":"narrator",
            "text":"\u{0344}".repeat(5000),
            "delivery":{"emotion":"neutral","intensity_permille":300}}),
        );
    }
    let mut expanded = request(0);
    expanded.script_json = value.to_string();
    assert!(expanded.script_json.len() < 2 * 1024 * 1024);
    let (status, error) = h
        .request(
            "POST",
            &format!("/scripts/{script}/revisions"),
            Some(ALICE),
            Some(serde_json::to_value(expanded).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "invalid_script");
    assert_eq!(
        error["issues"],
        json!([{"path":"$","rule":"DocumentTooLarge"}])
    );
    h.admin.execute("INSERT INTO script_evidence VALUES('bob','rights','bob-private','synthetic foreign evidence')",&[]).await.unwrap();
    let mut value: Value = serde_json::from_str(FIXTURE).unwrap();
    value["work"]["rights_record_id"] = json!("bob-private");
    let mut foreign = request(0);
    foreign.script_json = value.to_string();
    assert!(matches!(
        h.store.save(ALICE, &script, foreign).await,
        Err(StoreError::EvidenceUnavailable)
    ));
    let count: i64 = h
        .admin
        .query_one("SELECT count(*) FROM scripts WHERE id=$1", &[&script])
        .await
        .unwrap()
        .get(0);
    assert_eq!(count, 0);
    assert!(matches!(
        h.store.save(ALICE, &script, request(1)).await,
        Err(StoreError::StaleRevision(0))
    ));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM scripts WHERE id=$1", &[&script])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

async fn wait_for_two_blocked_writers(admin: &Client) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let count: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='cantos-test' AND wait_event_type='Lock'",&[]).await.unwrap().get(0);
        if count >= 2 {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "writers did not reach the locked-head barrier"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_stale_saves_and_duplicate_delivery_are_atomic() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let mut blocker = connect(&h.admin_config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one("SELECT id FROM scripts WHERE id=$1 FOR UPDATE", &[&script])
        .await
        .unwrap();
    let a_store = h.store.clone();
    let a_script = script.clone();
    let a = tokio::spawn(async move { a_store.save(ALICE, &a_script, request(1)).await });
    let b_store = h.store.clone();
    let b_script = script.clone();
    let b = tokio::spawn(async move {
        b_store
            .save(ALICE, &b_script, changed_rights(request(1)))
            .await
    });
    wait_for_two_blocked_writers(&h.admin).await;
    tx.commit().await.unwrap();
    let a = a.await.unwrap();
    let b = b.await.unwrap();
    assert!(matches!(
        (&a, &b),
        (Ok(_), Err(StoreError::StaleRevision(2))) | (Err(StoreError::StaleRevision(2)), Ok(_))
    ));
    assert_eq!(h.count(&script).await, (2, 2));
    let duplicate = request(2);
    let tx = blocker.transaction().await.unwrap();
    tx.query_one("SELECT id FROM scripts WHERE id=$1 FOR UPDATE", &[&script])
        .await
        .unwrap();
    let mut tasks = vec![];
    for _ in 0..2 {
        let store = h.store.clone();
        let script = script.clone();
        let request = duplicate.clone();
        tasks.push(tokio::spawn(async move {
            store.save(ALICE, &script, request).await.unwrap()
        }));
    }
    wait_for_two_blocked_writers(&h.admin).await;
    tx.commit().await.unwrap();
    assert_eq!(
        tasks.remove(0).await.unwrap(),
        tasks.remove(0).await.unwrap()
    );
    assert_eq!(h.count(&script).await, (3, 3));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn killed_connection_before_commit_rolls_back_and_same_key_can_retry() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let first = h.store.save(ALICE, &script, request(0)).await.unwrap();
    h.admin.batch_execute("CREATE FUNCTION crash_save() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_terminate_backend(pg_backend_pid()); RETURN NEW; END $$; CREATE TRIGGER crash_before_commit AFTER INSERT ON script_revisions FOR EACH ROW EXECUTE FUNCTION crash_save();").await.unwrap();
    let retry = request(1);
    assert!(matches!(
        h.store.save(ALICE, &script, retry.clone()).await,
        Err(StoreError::Unavailable)
    ));
    h.admin
        .batch_execute(
            "DROP TRIGGER crash_before_commit ON script_revisions; DROP FUNCTION crash_save()",
        )
        .await
        .unwrap();
    assert_eq!(h.count(&script).await, (1, 1));
    assert_eq!(h.store.load(ALICE, &script, None).await.unwrap(), first);
    let second = h.store.save(ALICE, &script, retry.clone()).await.unwrap();
    assert_eq!(h.store.save(ALICE, &script, retry).await.unwrap(), second);
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn app_privileges_triggers_and_load_checks_guard_immutable_history() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let mut config = h.admin_config.clone();
    config.user("cantos_app");
    let app = connect(&config).await;
    for sql in [
        "UPDATE script_revisions SET canonical_export='x'",
        "DELETE FROM script_revisions",
        "TRUNCATE script_revisions CASCADE",
        "UPDATE actors SET active=false",
        "UPDATE script_members SET role='editor'",
        "ALTER TABLE script_revisions DISABLE TRIGGER ALL",
    ] {
        let error = app.batch_execute(sql).await.unwrap_err();
        assert_eq!(error.code().unwrap().code(), "42501", "{sql}");
    }
    let error = h
        .admin
        .execute(
            "UPDATE script_revisions SET canonical_export=canonical_export WHERE script_id=$1",
            &[&script],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "55000");
    let error = h
        .admin
        .execute(
            "DELETE FROM script_revisions WHERE script_id=$1",
            &[&script],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "55000");
    // Privileged test-only tamper bypasses the trigger to exercise the production load barrier.
    h.admin
        .batch_execute("ALTER TABLE script_revisions DISABLE TRIGGER revisions_immutable")
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE script_revisions SET export_digest=$1 WHERE script_id=$2",
            &[&format!("sir-e1:sha256:{}", "0".repeat(64)), &script],
        )
        .await
        .unwrap();
    h.admin
        .batch_execute("ALTER TABLE script_revisions ENABLE TRIGGER revisions_immutable")
        .await
        .unwrap();
    assert!(matches!(
        h.store.load(ALICE, &script, None).await,
        Err(StoreError::CorruptRevision)
    ));
    let (status, error) = h
        .request("GET", &format!("/scripts/{script}/head"), Some(ALICE), None)
        .await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(error["code"], "corrupt_revision");
    assert_eq!(
        h.request(
            "GET",
            &format!("/scripts/{script}/history"),
            Some(ALICE),
            None
        )
        .await
        .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        h.request(
            "POST",
            &format!("/scripts/{script}/reviews"),
            Some(ALICE),
            Some(review_request(1))
        )
        .await
        .0,
        StatusCode::INTERNAL_SERVER_ERROR
    );
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM script_reviews", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn restart_fixture_for_process_and_database_recovery() {
    let h = Harness::new(Some("cantos_test_restart")).await;
    let script = Uuid::new_v4().to_string();
    let request = request(0);
    let saved = h.store.save(ALICE, &script, request.clone()).await.unwrap();
    fs::create_dir_all("../../target/revision-evidence").unwrap();
    fs::write("../../target/revision-evidence/restart.json",json!({"database":h.database,"script":script,"token":ALICE,"request":request,"response":saved}).to_string()).unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn session_exchange_origin_guard_and_malformed_inputs_are_safe() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let login = Request::builder()
        .method("POST")
        .uri("/api/v1/studio/session")
        .header("origin", ORIGIN)
        .header("content-type", "application/json")
        .body(Body::from(json!({"token":ALICE}).to_string()))
        .unwrap();
    let response = h.app.clone().oneshot(login).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let cookie = response.headers()["set-cookie"].to_str().unwrap();
    assert!(
        cookie.contains("HttpOnly")
            && cookie.contains("SameSite=Strict")
            && cookie.contains("Path=/api/v1/studio")
    );
    let identity: cantos_contracts::SessionResponse =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(identity.actor_id, "alice");
    let request = Request::builder()
        .method("POST")
        .uri(format!("/api/v1/studio/scripts/{script}/revisions"))
        .header("origin", "https://attacker.invalid")
        .header("cookie", format!("cantos_session={ALICE}"))
        .header("content-type", "application/json")
        .body(Body::from(
            serde_json::to_string(&self::request(1)).unwrap(),
        ))
        .unwrap();
    assert_eq!(
        h.app.clone().oneshot(request).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let (status, error) = h
        .request(
            "GET",
            &format!("/scripts/{script}/revisions/not-a-number"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "invalid_request");
    let (status, error) = h
        .request(
            "POST",
            &format!("/scripts/{script}/revisions"),
            Some(ALICE),
            Some(json!({"unknown":"field"})),
        )
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(error["code"], "invalid_request");
    let mut invalid = self::request(1);
    invalid.script_json = include_str!(
        "../../../packages/contracts/fixtures/script-ir/0.1.0/reject/multiple-faults.json"
    )
    .into();
    let (status, error) = h
        .request(
            "POST",
            &format!("/scripts/{script}/revisions"),
            Some(ALICE),
            Some(serde_json::to_value(invalid).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "invalid_script");
    assert!(error["issues"].as_array().unwrap().len() > 1);
    assert_eq!(h.count(&script).await, (1, 1));
    let logout = Request::builder()
        .method("DELETE")
        .uri("/api/v1/studio/session")
        .header("origin", ORIGIN)
        .header("cookie", format!("cantos_session={ALICE}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        h.app.clone().oneshot(logout).await.unwrap().status(),
        StatusCode::NO_CONTENT
    );
    assert!(matches!(
        h.store.authenticate(ALICE).await,
        Err(StoreError::Unauthenticated)
    ));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn msgpack_and_json_carry_the_same_revision_facts_on_every_negotiated_route() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let first = h.store.save(ALICE, &script, request(0)).await.unwrap();
    let head = format!("/scripts/{script}/head");
    for path in [head.clone(), format!("/scripts/{script}/revisions/1")] {
        let (status, json) = h.request("GET", &path, Some(ALICE), None).await;
        assert_eq!(status, StatusCode::OK);
        let packed: RevisionResponse = h.msgpack("GET", &path, Some(ALICE), vec![]).await.packed();
        assert_eq!(
            packed,
            serde_json::from_value::<RevisionResponse>(json).unwrap()
        );
        assert_eq!(packed, first);
    }
    let writes = format!("/scripts/{script}/revisions");
    let second: RevisionResponse = h
        .msgpack("POST", &writes, Some(ALICE), packed(&request(1)))
        .await
        .packed();
    assert_eq!((second.revision, second.accepted_by.as_str()), (2, "alice"));
    let (_, json) = h.request("GET", &head, Some(ALICE), None).await;
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(json).unwrap(),
        second
    );

    let valid = ValidateScriptRequest {
        script_json: second.script_json.clone(),
    };
    let preview: ScriptValidationResponse = h
        .msgpack("POST", "/validation", Some(ALICE), packed(&valid))
        .await
        .packed();
    assert!(preview.issues.is_empty());
    let invalid = ValidateScriptRequest {
        script_json: include_str!(
            "../../../packages/contracts/fixtures/script-ir/0.1.0/reject/multiple-faults.json"
        )
        .into(),
    };
    let (status, json_error) = h
        .request(
            "POST",
            "/validation",
            Some(ALICE),
            Some(serde_json::to_value(&invalid).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    let packed_error = h
        .msgpack("POST", "/validation", Some(ALICE), packed(&invalid))
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(packed_error, json_error);
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn a_lost_msgpack_response_replays_in_either_representation_without_a_second_revision() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let writes = format!("/scripts/{script}/revisions");
    let intent = request(1);
    let committed: RevisionResponse = h
        .msgpack("POST", &writes, Some(ALICE), packed(&intent))
        .await
        .packed();
    // The client lost that response. The same key, base and snapshot replays the first result
    // whichever representation the retry uses; the codec never changes the idempotency key.
    let (status, replay) = h
        .request(
            "POST",
            &writes,
            Some(ALICE),
            Some(serde_json::to_value(&intent).unwrap()),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(replay).unwrap(),
        committed
    );
    let replay: RevisionResponse = h
        .msgpack("POST", &writes, Some(ALICE), packed(&intent))
        .await
        .packed();
    assert_eq!(replay, committed);
    assert_eq!(h.count(&script).await, (2, 2));

    let reused = h
        .msgpack(
            "POST",
            &writes,
            Some(ALICE),
            packed(&changed_rights(intent)),
        )
        .await
        .error(StatusCode::CONFLICT);
    assert_eq!(reused["code"], "operation_reused");
    let stale = h
        .msgpack("POST", &writes, Some(ALICE), packed(&request(1)))
        .await
        .error(StatusCode::CONFLICT);
    assert_eq!(stale["code"], "stale_revision");
    assert_eq!(stale["current_revision"], 2);
    assert_eq!(h.count(&script).await, (2, 2));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn refused_representations_write_nothing_and_other_routes_stay_json_only() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    h.store.save(ALICE, &script, request(0)).await.unwrap();
    let writes = format!("/scripts/{script}/revisions");
    let intent = request(1);
    let body = packed(&intent);
    let not_acceptable = h
        .exchange(
            ORIGIN,
            "POST",
            &writes,
            Some(ALICE),
            (Some(MSGPACK), Some("text/html")),
            body.clone(),
        )
        .await;
    assert_eq!(not_acceptable.headers["vary"], "accept");
    assert_eq!(
        not_acceptable.error(StatusCode::NOT_ACCEPTABLE),
        json!({"code":"invalid_request","current_revision":null,
               "issues":[{"path":"accept","rule":"not_acceptable"}]})
    );
    let unsupported = h
        .exchange(
            ORIGIN,
            "POST",
            &writes,
            Some(ALICE),
            (Some("text/plain"), None),
            serde_json::to_vec(&intent).unwrap(),
        )
        .await;
    assert_eq!(
        unsupported.error(StatusCode::UNSUPPORTED_MEDIA_TYPE)["issues"],
        json!([{"path":"content-type","rule":"unsupported_media_type"}])
    );
    let forged_field = packed(&json!({
        "expected_revision": 1, "operation_id": intent.operation_id,
        "script_json": intent.script_json, "accepted_by": "mallory"}));
    for malformed in [body[..body.len() - 1].to_vec(), forged_field] {
        let refused = h.msgpack("POST", &writes, Some(ALICE), malformed).await;
        assert_eq!(
            refused.error(StatusCode::BAD_REQUEST)["code"],
            "invalid_request"
        );
    }
    assert_eq!(h.count(&script).await, (1, 1));
    // Nothing was half-applied: the same intent still commits once.
    let saved: RevisionResponse = h.msgpack("POST", &writes, Some(ALICE), body).await.packed();
    assert_eq!(saved.revision, 2);

    let review = ReviewRequest {
        operation_id: Uuid::new_v4().to_string(),
        revision: 2,
    };
    let refused = h
        .msgpack(
            "POST",
            &format!("/scripts/{script}/reviews"),
            Some(ALICE),
            packed(&review),
        )
        .await;
    assert_eq!(
        refused.error(StatusCode::BAD_REQUEST)["code"],
        "invalid_request"
    );
    assert_eq!(h.reviews(&script).await, 0);
    let history = h
        .msgpack(
            "GET",
            &format!("/scripts/{script}/history"),
            Some(ALICE),
            vec![],
        )
        .await;
    assert_eq!(history.status, StatusCode::OK);
    assert_eq!(history.headers["content-type"], "application/json");
    assert!(!history.headers.contains_key("vary"));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn msgpack_requests_get_the_same_origin_session_and_permission_decisions() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let saved = h.store.save(ALICE, &script, request(0)).await.unwrap();
    let head = format!("/scripts/{script}/head");
    let writes = format!("/scripts/{script}/revisions");
    let body = packed(&request(1));
    for (method, path, body) in [("GET", &head, vec![]), ("POST", &writes, body.clone())] {
        let anonymous = h.msgpack(method, path, None, body.clone()).await;
        assert_eq!(
            anonymous.error(StatusCode::UNAUTHORIZED)["code"],
            "unauthenticated"
        );
        let stranger = h.msgpack(method, path, Some(BOB), body).await;
        let hidden = stranger.error(StatusCode::NOT_FOUND);
        assert_eq!(hidden["code"], "not_found");
        assert!(!hidden.to_string().contains("Ánh đèn"));
    }
    let forged = h
        .exchange(
            "https://attacker.invalid",
            "POST",
            &writes,
            Some(ALICE),
            (Some(MSGPACK), Some(MSGPACK)),
            body.clone(),
        )
        .await;
    assert_eq!(forged.error(StatusCode::FORBIDDEN)["code"], "forbidden");
    h.admin
        .execute(
            "INSERT INTO script_members VALUES($1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    let read: RevisionResponse = h.msgpack("GET", &head, Some(BOB), vec![]).await.packed();
    assert_eq!(read, saved);
    let denied = h.msgpack("POST", &writes, Some(BOB), body).await;
    assert_eq!(denied.error(StatusCode::FORBIDDEN)["code"], "forbidden");
    h.admin
        .execute("UPDATE sessions SET revoked=true WHERE actor_id='bob'", &[])
        .await
        .unwrap();
    let revoked = h.msgpack("GET", &head, Some(BOB), vec![]).await;
    assert_eq!(
        revoked.error(StatusCode::UNAUTHORIZED)["code"],
        "unauthenticated"
    );
    assert_eq!(h.count(&script).await, (1, 1));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn legacy_studio_paths_remain_compatible_and_sessions_stay_scoped_to_their_mount() {
    let h = Harness::new(None).await;
    let script = Uuid::new_v4().to_string();
    let send = |method: &'static str, uri: String, cookie: Option<String>, body: String| {
        let app = h.app.clone();
        async move {
            let mut request = Request::builder()
                .method(method)
                .uri(uri)
                .header("origin", ORIGIN)
                .header("content-type", "application/json");
            if let Some(cookie) = cookie {
                request = request.header("cookie", cookie);
            }
            app.oneshot(request.body(Body::from(body)).unwrap())
                .await
                .unwrap()
        }
    };
    // Legacy login keeps the pre-split cookie scope and is flagged deprecated.
    let login = send(
        "POST",
        "/api/v1/session".into(),
        None,
        json!({"token":ALICE}).to_string(),
    )
    .await;
    assert_eq!(login.status(), StatusCode::OK);
    assert!(login.headers()["set-cookie"]
        .to_str()
        .unwrap()
        .ends_with("Path=/api/v1"));
    assert_eq!(login.headers()["deprecation"], "true");
    // A write through the legacy path and a read through the current path see one revision.
    let session = Some(format!("cantos_session={ALICE}"));
    let body = serde_json::to_string(&request(0)).unwrap();
    let saved = send(
        "POST",
        format!("/api/v1/scripts/{script}/revisions"),
        session.clone(),
        body,
    )
    .await;
    assert_eq!(saved.status(), StatusCode::OK);
    let saved: RevisionResponse =
        serde_json::from_slice(&saved.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let (status, current) = h
        .request("GET", &format!("/scripts/{script}/head"), Some(ALICE), None)
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        serde_json::from_value::<RevisionResponse>(current).unwrap(),
        saved
    );
    assert_eq!(h.count(&script).await, (1, 1));
    // A valid Studio session grants nothing on the reserved surfaces.
    for base in ["/api/v1/theatre", "/api/v1/admin"] {
        for path in [format!("/scripts/{script}/head"), "/session".into()] {
            let response = send(
                "GET",
                format!("{base}{path}"),
                session.clone(),
                String::new(),
            )
            .await;
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{base}{path}");
        }
    }
}
