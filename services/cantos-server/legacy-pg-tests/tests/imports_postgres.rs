//! Intake boundary evidence; run only inside scripts/test_postgres.py's disposable cluster.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_contracts::{
    ImportFormat, ImportMetadata, ImportOutcome, ImportRequest, ImportResponse,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::postgres::{local_config, migrate, token_hash, Store, StoreError},
    script_ir::read_script,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const ORIGINAL: &str = "\u{feff}Chương một\r\n\r\nAn: Người bạn đã về chưa?\r\nMinh: Mình ở đây.\r\n\r\n— Một giọng chưa rõ tên.\r\n";

struct Harness {
    admin: Client,
    config: Config,
    store: Store,
    app: Router,
    database: String,
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        if let Err(error) = connection.await {
            eprintln!(
                "test connection closed: {}",
                error.code().map(|code| code.code()).unwrap_or("transport")
            );
        }
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
        admin.batch_execute("GRANT USAGE ON SCHEMA public TO cantos_app; GRANT SELECT ON actors,sessions,script_members,script_evidence,script_revisions,revision_evidence,scripts,source_records,script_reviews,script_review_operations TO cantos_app; GRANT INSERT ON scripts,script_revisions,revision_evidence,script_evidence,source_records TO cantos_app; GRANT UPDATE(head_revision) ON scripts TO cantos_app; GRANT UPDATE(revoked) ON sessions TO cantos_app; INSERT INTO actors(id) VALUES('alice'),('bob');").await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,CURRENT_TIMESTAMP+interval '1 hour')", &[&token_hash(token), &actor]).await.unwrap();
        }
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-import-test");
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
            config,
            store,
            app,
            database,
        }
    }

    async fn raw(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Vec<u8>,
    ) -> (StatusCode, Vec<u8>) {
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
            .oneshot(request.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        if path.ends_with("/original") && response.status().is_success() {
            assert_eq!(
                response.headers()["content-type"],
                "application/octet-stream"
            );
            assert_eq!(
                response.headers()["content-disposition"],
                "attachment; filename=\"manuscript.bin\""
            );
        }
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, bytes.to_vec())
    }

    async fn json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        let (status, bytes) = self
            .raw(
                method,
                path,
                token,
                body.map(|body| body.to_string().into_bytes())
                    .unwrap_or_default(),
            )
            .await;
        let value = serde_json::from_slice(&bytes).unwrap();
        if status.is_success() {
            validate_import_contract(&value, "ImportResponse");
        } else {
            let schema: Value = serde_json::from_str(include_str!(
                "../../../packages/contracts/schema/studio/v1.schema.json"
            ))
            .unwrap();
            let schema = json!({"$ref":"#/$defs/ApiError","$defs":schema["$defs"]});
            jsonschema::validator_for(&schema)
                .unwrap()
                .validate(&value)
                .unwrap();
        }
        (status, value)
    }

    async fn counts(&self) -> (i64, i64, i64) {
        let row = self.admin.query_one("SELECT (SELECT count(*) FROM source_records),(SELECT count(*) FROM script_evidence WHERE kind='source'),(SELECT count(*) FROM script_revisions)", &[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2))
    }
}

fn validate_import_contract(value: &Value, definition: &str) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/schema/manuscript/v1.schema.json"
    ))
    .unwrap();
    let schema = json!({"$ref":format!("#/$defs/{definition}"),"$defs":schema["$defs"]});
    jsonschema::validator_for(&schema)
        .unwrap()
        .validate(value)
        .unwrap();
}

fn request(format: ImportFormat, bytes: &[u8]) -> ImportRequest {
    ImportRequest {
        metadata: ImportMetadata {
            operation_id: Uuid::new_v4().to_string(),
            file_name: "../Bản thảo gốc.txt".into(),
            format,
            reference: "Synthetic Vietnamese multi-character fixture".into(),
            rights_holder: None,
            permission_evidence: None,
            usage_scope: None,
        },
        original_bytes: bytes.into(),
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn originals_receipts_failed_extraction_and_registry_survive_reopen_exactly() {
    let h = Harness::new(None).await;
    let fixtures = [
        (ImportFormat::Txt, ORIGINAL.as_bytes()),
        (
            ImportFormat::Markdown,
            "# Cảnh một\n\nAn: Chào Minh!\n".as_bytes(),
        ),
        (ImportFormat::ScriptIr, SCRIPT.as_bytes()),
        (ImportFormat::Docx, &[0x50, 0x4b, 0x01][..]),
        (ImportFormat::Txt, &[0xff, 0xfe][..]),
        (ImportFormat::Txt, b"An:\0xin chao".as_slice()),
        (ImportFormat::Txt, b"".as_slice()),
    ];
    for (index, (format, bytes)) in fixtures.into_iter().enumerate() {
        let mut request = request(format, bytes);
        if index == 0 {
            request.metadata.rights_holder = Some("Tác giả hư cấu".into());
            request.metadata.permission_evidence =
                Some("Synthetic local-test permission note".into());
            request.metadata.usage_scope =
                Some("personal adaptation; publication unconfirmed".into());
        }
        validate_import_contract(&json!(request), "ImportRequest");
        let (status, value) = h
            .json("POST", "/imports", Some(ALICE), Some(json!(request)))
            .await;
        assert_eq!(status, StatusCode::OK);
        let receipt: ImportResponse = serde_json::from_value(value).unwrap();
        assert_eq!(receipt.imported_by, "alice");
        assert_eq!(receipt.metadata, request.metadata);
        assert_eq!(receipt.sha256, format!("{:x}", Sha256::digest(bytes)));
        assert_eq!(receipt.byte_len, bytes.len() as u64);
        assert_eq!(
            h.store.import_manuscript(ALICE, request).await.unwrap(),
            receipt
        );
        let mut config = h.config.clone();
        config.user("cantos_app");
        let reopened = Store::new(config).unwrap();
        assert_eq!(
            reopened.load_import(ALICE, &receipt.id).await.unwrap(),
            receipt
        );
        assert_eq!(
            reopened.original_import(ALICE, &receipt.id).await.unwrap(),
            bytes
        );
        assert_eq!(
            h.raw(
                "GET",
                &format!("/imports/{}/original", receipt.id),
                Some(ALICE),
                vec![],
            )
            .await,
            (StatusCode::OK, bytes.to_vec())
        );
        if index == 0 {
            assert_eq!(receipt.original_text.as_deref(), Some(ORIGINAL));
        }
        if index >= 3 {
            assert!(matches!(receipt.outcome, ImportOutcome::Failed { .. }));
        } else {
            assert!(matches!(receipt.outcome, ImportOutcome::Parsed { .. }));
        }
        if index >= 4 {
            assert_eq!(receipt.original_text, None);
        }
    }
    assert_eq!(h.counts().await, (7, 7, 0));
    let rights: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM script_evidence WHERE kind='rights'",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(rights, 0, "intake never grants publication rights");
}

async fn wait_for_blocked_imports(admin: &Client, expected: i64) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let waiting: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='cantos-import-test' AND wait_event_type='Lock'", &[]).await.unwrap().get(0);
        if waiting >= expected {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "writers did not reach actor lock barrier"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_duplicates_replay_once_and_changed_operation_is_rejected_atomically() {
    let h = Harness::new(None).await;
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended('manuscript-import:alice', 0))",
        &[],
    )
    .await
    .unwrap();
    let intent = request(ImportFormat::Txt, ORIGINAL.as_bytes());
    let mut tasks = vec![];
    for _ in 0..2 {
        let store = h.store.clone();
        let request = intent.clone();
        tasks.push(tokio::spawn(async move {
            store.import_manuscript(ALICE, request).await.unwrap()
        }));
    }
    wait_for_blocked_imports(&h.admin, 2).await;
    tx.commit().await.unwrap();
    let first = tasks.remove(0).await.unwrap();
    assert_eq!(first, tasks.remove(0).await.unwrap());
    assert_eq!(h.counts().await, (1, 1, 0));
    for metadata_change in [false, true] {
        let mut changed = intent.clone();
        if metadata_change {
            changed.metadata.usage_scope = Some("personal adaptation only".into());
        } else {
            changed.original_bytes.push(b' ');
        }
        assert!(matches!(
            h.store.import_manuscript(ALICE, changed).await,
            Err(StoreError::OperationReused)
        ));
    }
    assert_eq!(h.store.load_import(ALICE, &first.id).await.unwrap(), first);
    assert_eq!(h.counts().await, (1, 1, 0));

    let tx = blocker.transaction().await.unwrap();
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended('manuscript-import:alice', 0))",
        &[],
    )
    .await
    .unwrap();
    let competing = request(ImportFormat::Txt, b"An: Another immutable source.");
    let mut changed = competing.clone();
    changed.original_bytes.push(b'!');
    let first_store = h.store.clone();
    let first_task =
        tokio::spawn(async move { first_store.import_manuscript(ALICE, competing).await });
    let second_store = h.store.clone();
    let second_task =
        tokio::spawn(async move { second_store.import_manuscript(ALICE, changed).await });
    wait_for_blocked_imports(&h.admin, 2).await;
    tx.commit().await.unwrap();
    let results = (first_task.await.unwrap(), second_task.await.unwrap());
    assert!(matches!(
        results,
        (Ok(_), Err(StoreError::OperationReused)) | (Err(StoreError::OperationReused), Ok(_))
    ));
    assert_eq!(h.counts().await, (2, 2, 0));
    let bob = h.store.import_manuscript(BOB, intent).await.unwrap();
    assert_ne!(first.id, bob.id, "operation IDs are actor scoped");
    assert_eq!(h.counts().await, (3, 3, 0));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn actor_deactivation_while_waiting_for_import_lock_is_rechecked_before_insert() {
    let h = Harness::new(None).await;
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one(
        "SELECT pg_advisory_xact_lock(hashtextextended('manuscript-import:alice', 0))",
        &[],
    )
    .await
    .unwrap();
    let store = h.store.clone();
    let task = tokio::spawn(async move {
        store
            .import_manuscript(ALICE, request(ImportFormat::Txt, ORIGINAL.as_bytes()))
            .await
    });
    wait_for_blocked_imports(&h.admin, 1).await;
    h.admin
        .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        task.await.unwrap(),
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(h.counts().await, (0, 0, 0));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn denied_sessions_and_wrong_owner_never_read_or_write_sources() {
    let h = Harness::new(None).await;
    let receipt = h
        .store
        .import_manuscript(ALICE, request(ImportFormat::Txt, ORIGINAL.as_bytes()))
        .await
        .unwrap();
    for suffix in ["", "/original"] {
        let path = format!("/imports/{}{suffix}", receipt.id);
        for (token, status) in [
            (None, StatusCode::UNAUTHORIZED),
            (Some(BOB), StatusCode::NOT_FOUND),
        ] {
            let (actual, body) = h.raw("GET", &path, token, vec![]).await;
            assert_eq!(actual, status);
            assert!(!String::from_utf8_lossy(&body).contains("Chương"));
        }
    }
    assert_eq!(
        h.json(
            "POST",
            "/imports",
            None,
            Some(json!(request(ImportFormat::Txt, ORIGINAL.as_bytes())))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    h.admin
        .execute(
            "UPDATE sessions SET revoked=true WHERE actor_id='alice'",
            &[],
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_import(ALICE, &receipt.id).await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(matches!(
        h.store
            .import_manuscript(ALICE, request(ImportFormat::Txt, b"changed"))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    h.admin.execute("UPDATE sessions SET expires_at=CURRENT_TIMESTAMP-interval '1 second' WHERE actor_id='bob'", &[]).await.unwrap();
    assert!(matches!(
        h.store
            .import_manuscript(BOB, request(ImportFormat::Txt, b"new"))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    h.admin
        .execute(
            "UPDATE sessions SET revoked=false WHERE actor_id='alice';",
            &[],
        )
        .await
        .unwrap();
    h.admin
        .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
        .await
        .unwrap();
    assert!(matches!(
        h.store.original_import(ALICE, &receipt.id).await,
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(h.counts().await, (1, 1, 0));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn invalid_admission_and_injected_insert_or_connection_failure_leave_no_partial_registry() {
    let h = Harness::new(None).await;
    for variant in 0..6 {
        let mut invalid = request(ImportFormat::Txt, ORIGINAL.as_bytes());
        match variant {
            0 => invalid.original_bytes = vec![b'x'; 1_048_577],
            1 => invalid.metadata.operation_id = "not-a-uuid".into(),
            2 => invalid.metadata.file_name = "\r\nInjected".into(),
            3 => invalid.metadata.reference.clear(),
            4 => invalid.metadata.rights_holder = Some(" ".into()),
            5 => invalid.metadata.permission_evidence = Some("x".repeat(2049)),
            _ => unreachable!(),
        }
        assert!(matches!(
            h.store.import_manuscript(ALICE, invalid).await,
            Err(StoreError::InvalidRequestFields(_))
        ));
    }
    assert_eq!(h.counts().await, (0, 0, 0));
    let mut multiple = request(ImportFormat::Txt, b"x");
    multiple.metadata.file_name = "Đ".repeat(128);
    multiple.metadata.reference = " ".into();
    let (status, error) = h
        .json("POST", "/imports", Some(ALICE), Some(json!(multiple)))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        error,
        json!({
            "code": "invalid_request",
            "current_revision": null,
            "issues": [
                {"path": "/metadata/file_name", "rule": "byte_length"},
                {"path": "/metadata/reference", "rule": "nonblank"}
            ]
        })
    );
    assert!(!error.to_string().contains('Đ'));
    assert_eq!(h.counts().await, (0, 0, 0));
    assert_eq!(
        h.json(
            "POST",
            "/imports",
            Some(ALICE),
            Some(json!({"metadata":"malformed"}))
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.raw(
            "POST",
            "/imports",
            Some(ALICE),
            vec![b'x'; 5 * 1024 * 1024 + 1]
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    let intent = request(ImportFormat::Txt, ORIGINAL.as_bytes());
    h.admin.batch_execute("CREATE TRIGGER injected_import_failure BEFORE INSERT ON source_records FOR EACH ROW EXECUTE FUNCTION reject_settled_change()").await.unwrap();
    assert!(matches!(
        h.store.import_manuscript(ALICE, intent.clone()).await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (0, 0, 0));
    h.admin.batch_execute("DROP TRIGGER injected_import_failure ON source_records; CREATE FUNCTION crash_import() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_terminate_backend(pg_backend_pid()); RETURN NEW; END $$; CREATE TRIGGER crash_import_before_commit AFTER INSERT ON source_records FOR EACH ROW EXECUTE FUNCTION crash_import()").await.unwrap();
    assert!(matches!(
        h.store.import_manuscript(ALICE, intent.clone()).await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (0, 0, 0));
    h.admin.batch_execute("DROP TRIGGER crash_import_before_commit ON source_records; DROP FUNCTION crash_import()").await.unwrap();
    let receipt = h
        .store
        .import_manuscript(ALICE, intent.clone())
        .await
        .unwrap();
    assert_eq!(
        h.store.import_manuscript(ALICE, intent).await.unwrap(),
        receipt
    );
    assert_eq!(h.counts().await, (1, 1, 0));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn imported_sources_are_immutable_tamper_detected_and_legacy_links_reuse_revision_store() {
    let h = Harness::new(None).await;
    let text = h
        .store
        .import_manuscript(ALICE, request(ImportFormat::Txt, ORIGINAL.as_bytes()))
        .await
        .unwrap();
    let binary = h
        .store
        .import_manuscript(ALICE, request(ImportFormat::Docx, &[0xff, 0xfe]))
        .await
        .unwrap();
    for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
        h.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice',$1,$2,'synthetic test reference; no clearance')", &[&kind, &id]).await.unwrap();
    }
    let mut value: Value = serde_json::from_str(SCRIPT).unwrap();
    value["provenance"][0]["source_record_id"] = json!(text.id);
    let script = Uuid::new_v4().to_string();
    h.store
        .save(
            ALICE,
            &script,
            cantos_contracts::SaveRevisionRequest {
                expected_revision: 0,
                operation_id: Uuid::new_v4().to_string(),
                script_json: value.to_string(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        h.store
            .source(ALICE, &script, &text.id)
            .await
            .unwrap()
            .original_text,
        ORIGINAL
    );
    value["provenance"][0]["source_record_id"] = json!(binary.id);
    h.store
        .save(
            ALICE,
            &script,
            cantos_contracts::SaveRevisionRequest {
                expected_revision: 1,
                operation_id: Uuid::new_v4().to_string(),
                script_json: value.to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store.source(ALICE, &script, &binary.id).await,
        Err(StoreError::InvalidRequest)
    ));
    assert_eq!(
        h.store.original_import(ALICE, &binary.id).await.unwrap(),
        vec![0xff, 0xfe]
    );
    let mut app_config = h.config.clone();
    app_config.user("cantos_app");
    let app = connect(&app_config).await;
    for sql in [
        "UPDATE source_records SET original_bytes='x'",
        "DELETE FROM source_records",
        "TRUNCATE source_records CASCADE",
        "UPDATE actors SET active=false",
        "ALTER TABLE source_records DISABLE TRIGGER ALL",
    ] {
        assert_eq!(
            app.batch_execute(sql)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "42501"
        );
    }
    for sql in [
        "UPDATE source_records SET import_metadata=import_metadata",
        "DELETE FROM source_records",
        "TRUNCATE source_records CASCADE",
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
    h.admin
        .batch_execute("ALTER TABLE source_records DISABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE source_records SET original_bytes=$1 WHERE id=$2",
            &[&b"tampered".to_vec(), &text.id],
        )
        .await
        .unwrap();
    h.admin
        .batch_execute("ALTER TABLE source_records ENABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_import(ALICE, &text.id).await,
        Err(StoreError::CorruptRevision)
    ));
    assert!(matches!(
        h.store.original_import(ALICE, &text.id).await,
        Err(StoreError::CorruptRevision)
    ));
    h.admin
        .batch_execute("ALTER TABLE source_records DISABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE source_records SET original_bytes=$1,original_text=NULL WHERE id=$2",
            &[&ORIGINAL.as_bytes().to_vec(), &text.id],
        )
        .await
        .unwrap();
    h.admin
        .batch_execute("ALTER TABLE source_records ENABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_import(ALICE, &text.id).await,
        Err(StoreError::CorruptRevision)
    ));
    h.admin
        .batch_execute("ALTER TABLE source_records DISABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE source_records SET original_text=$1,reference='forged reference' WHERE id=$2",
            &[&ORIGINAL, &text.id],
        )
        .await
        .unwrap();
    h.admin
        .batch_execute("ALTER TABLE source_records ENABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_import(ALICE, &text.id).await,
        Err(StoreError::CorruptRevision)
    ));
    h.admin
        .batch_execute("ALTER TABLE source_records DISABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    h.admin.execute("UPDATE source_records SET import_outcome=jsonb_set(import_outcome,'{error,code}', '\"forged_fact\"'::jsonb) WHERE id=$1", &[&binary.id]).await.unwrap();
    h.admin
        .batch_execute("ALTER TABLE source_records ENABLE TRIGGER sources_immutable")
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_import(ALICE, &binary.id).await,
        Err(StoreError::CorruptRevision)
    ));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn baseline_replay_preserves_source_records_and_revisions_without_backfill() {
    let cluster_config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    let cluster = connect(&cluster_config).await;
    let name = format!("cantos_test_{}", Uuid::new_v4().simple());
    cluster
        .batch_execute(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    let mut config = cluster_config;
    config.dbname(&name);
    let admin = connect(&config).await;
    migrate(&config).await.unwrap();
    admin.batch_execute("INSERT INTO actors(id) VALUES('alice'); INSERT INTO script_evidence VALUES('alice','source','old-source','pre-import synthetic original')").await.unwrap();
    admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,'alice',CURRENT_TIMESTAMP+interval '1 hour')", &[&token_hash(ALICE)]).await.unwrap();
    for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
        admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice',$1,$2,'pre-import synthetic reference')", &[&kind, &id]).await.unwrap();
    }
    let store = Store::new(config.clone()).unwrap();
    let script = Uuid::new_v4().to_string();
    let saved = store
        .save(
            ALICE,
            &script,
            cantos_contracts::SaveRevisionRequest {
                expected_revision: 0,
                operation_id: Uuid::new_v4().to_string(),
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    let old_hash = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
    admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256) VALUES('alice','old-source','old reference',$1,$2)", &[&ORIGINAL, &old_hash]).await.unwrap();
    let before = admin
        .query_one("SELECT row_to_json(s)::text FROM source_records s", &[])
        .await
        .unwrap()
        .get::<_, String>(0);
    migrate(&config).await.unwrap();
    migrate(&config).await.unwrap();
    assert_eq!(store.load(ALICE, &script, Some(1)).await.unwrap(), saved);
    let after: Value = serde_json::from_str(
        &admin
            .query_one("SELECT row_to_json(s)::text FROM source_records s", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
    )
    .unwrap();
    let before: Value = serde_json::from_str(&before).unwrap();
    for (key, value) in before.as_object().unwrap() {
        assert_eq!(&after[key], value);
    }
    for key in [
        "original_bytes",
        "import_metadata",
        "import_outcome",
        "import_digest",
        "imported_by",
        "import_operation_id",
    ] {
        assert_eq!(after[key], Value::Null);
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn import_restart_fixture_preserves_exact_bytes_receipt_and_same_operation() {
    let h = Harness::new(Some("cantos_test_import_restart")).await;
    let request = request(ImportFormat::Txt, ORIGINAL.as_bytes());
    let receipt = h
        .store
        .import_manuscript(ALICE, request.clone())
        .await
        .unwrap();
    fs::create_dir_all("../../target/import-evidence").unwrap();
    fs::write(
        "../../target/import-evidence/restart.json",
        json!({"database": h.database, "token": ALICE, "request": request, "response": receipt})
            .to_string(),
    )
    .unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn bounded_import_point_reads_keep_authorized_postgresql_index_baseline() {
    // Exploratory budget declared before observations: warm p95 <=250ms, concurrency one,
    // 1,000 immutable sources of 1KiB. No before/after speedup or production load claim.
    let h = Harness::new(None).await;
    let mut bytes = "An: Ánh đèn cuối sân khấu. ".as_bytes().to_vec();
    bytes.resize(1024, b'x');
    let seeded_at = Instant::now();
    let mut last = None;
    for _ in 0..1000 {
        last = Some(
            h.store
                .import_manuscript(ALICE, request(ImportFormat::Txt, &bytes))
                .await
                .unwrap(),
        );
    }
    let seed_ms = seeded_at.elapsed().as_secs_f64() * 1000.0;
    let receipt = last.unwrap();
    h.admin
        .batch_execute("ANALYZE source_records; ANALYZE sessions; ANALYZE actors")
        .await
        .unwrap();
    let mut config = h.config.clone();
    config.user("cantos_app");
    let app = connect(&config).await;
    let columns = "id,reference,imported_by,import_operation_id,import_digest,original_bytes,original_text,sha256,import_metadata::text AS import_metadata,import_outcome::text AS import_outcome,to_char(recorded_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS recorded_at";
    let point = format!("EXPLAIN (ANALYZE, BUFFERS) SELECT {columns} FROM source_records WHERE owner_id=$1 AND id=$2 AND imported_by=$1");
    let replay = format!("EXPLAIN (ANALYZE, BUFFERS) SELECT {columns} FROM source_records WHERE imported_by=$1 AND import_operation_id=$2");
    let point_plan: Vec<String> = app
        .query(&point, &[&"alice", &receipt.id])
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    let replay_plan: Vec<String> = app
        .query(&replay, &[&"alice", &receipt.metadata.operation_id])
        .await
        .unwrap()
        .into_iter()
        .map(|row| row.get(0))
        .collect();
    let authorization_plan: Vec<String> = app.query("EXPLAIN (ANALYZE, BUFFERS) SELECT a.id FROM sessions s JOIN actors a ON a.id=s.actor_id WHERE s.token_hash=$1 AND NOT s.revoked AND s.expires_at>CURRENT_TIMESTAMP AND a.active FOR SHARE OF s", &[&token_hash(ALICE)]).await.unwrap().into_iter().map(|row| row.get(0)).collect();
    assert!(point_plan
        .iter()
        .any(|line| line.contains("source_records_pkey")));
    assert!(replay_plan
        .iter()
        .any(|line| line.contains("source_import_operations")));
    let path = format!("/imports/{}", receipt.id);
    for _ in 0..5 {
        assert_eq!(
            h.json("GET", &path, Some(ALICE), None).await.0,
            StatusCode::OK
        );
    }
    let mut timings_ms = vec![];
    for _ in 0..30 {
        let start = Instant::now();
        let (status, response) = h.raw("GET", &path, Some(ALICE), vec![]).await;
        timings_ms.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<ImportResponse>(&response).unwrap(),
            receipt
        );
    }
    timings_ms.sort_by(f64::total_cmp);
    let p50_ms = timings_ms[14];
    let p95_ms = timings_ms[28];
    let p99_ms = timings_ms[29];
    let pg_version: String = app.query_one("SELECT version()", &[]).await.unwrap().get(0);
    fs::create_dir_all("../../target/import-evidence").unwrap();
    fs::write(
        "../../target/import-evidence/point-lookups.json",
        serde_json::to_string_pretty(&json!({
            "postgresql": pg_version,
            "database": h.database,
            "source_sha256": receipt.sha256,
            "fixture_kind": "synthetic Vietnamese source plus ASCII padding",
            "source_count": 1000,
            "source_bytes": 1024,
            "seed_ms": seed_ms,
            "concurrency": 1,
            "warm_up_reads": 5,
            "samples": 30,
            "target_warm_p95_ms": 250,
            "p50_ms": p50_ms,
            "p95_ms": p95_ms,
            "p99_ms": p99_ms,
            "timings_ms": timings_ms,
            "point_plan": point_plan,
            "replay_plan": replay_plan,
            "authorization_plan": authorization_plan,
            "dbsp": "defer: authorized immutable indexed point lookups have no repeated aggregate; no engine experiment run",
            "limits": "warm in-process HTTP Router with real PostgreSQL; no socket transport, cold cache, high concurrency or production workload"
        })).unwrap(),
    ).unwrap();
    assert!(
        p95_ms <= 250.0,
        "declared warm point-read p95 exceeded 250ms: {p95_ms}"
    );
}
