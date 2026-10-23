//! File-backed Turso import parity: preserved source bytes and immutable receipts through
//! the production Studio router. Ported assertions from legacy-pg-tests/tests/imports_postgres.rs.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_server::contract::studio::{
    ImportFormat, ImportMetadata, ImportOutcome, ImportRequest, ImportResponse,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::store::{Params, Row, StoreError},
    test_support::ServerFixture,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "http://127.0.0.1:8080";
const SCRIPT: &str =
    include_str!("../../../services/cantos-server/contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json");
const ORIGINAL: &str = "\u{feff}Chương một\r\n\r\nAn: Người bạn đã về chưa?\r\nMinh: Mình ở đây.\r\n\r\n— Một giọng chưa rõ tên.\r\n";

struct Harness {
    admin: ServerFixture,
}
// Fixture conveniences retain each ported test's exact SQL/response assertions. These are
// real statements through the same owner, never a persistence double.
trait FixtureQueries {
    async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError>;
    async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, StoreError>;
}
impl FixtureQueries for ServerFixture {
    async fn execute(&self, sql: &str, params: Params<'_>) -> Result<u64, StoreError> {
        self.exec(sql, params).await
    }
    async fn query_one(&self, sql: &str, params: Params<'_>) -> Result<Row, StoreError> {
        let rows = self.rows(sql, params).await?;
        assert_eq!(rows.len(), 1);
        Ok(rows.into_iter().next().unwrap())
    }
}
impl Harness {
    async fn new() -> Self {
        let admin = ServerFixture::new().await;
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.studio_caller(actor, token).await;
        }
        Self { admin }
    }
    fn app(&self) -> Router {
        router(
            AppState {
                store: self.admin.store().clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        )
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
            .app()
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
                "../../../services/cantos-server/contracts/studio/editorial/v1/editorial.schema.json"
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
        (
            row.try_get(0).unwrap(),
            row.try_get(1).unwrap(),
            row.try_get(2).unwrap(),
        )
    }
}

fn validate_import_contract(value: &Value, definition: &str) {
    let schema: Value = serde_json::from_str(include_str!(
        "../../../services/cantos-server/contracts/studio/manuscript/v1/manuscript.schema.json"
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
async fn originals_receipts_failed_extraction_and_registry_survive_reopen_exactly() {
    let mut h = Harness::new().await;
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
            h.admin
                .store()
                .import_manuscript(ALICE, request)
                .await
                .unwrap(),
            receipt
        );
        h.admin.restart().await;
        assert_eq!(
            h.admin
                .store()
                .load_import(ALICE, &receipt.id)
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(
            h.admin
                .store()
                .original_import(ALICE, &receipt.id)
                .await
                .unwrap(),
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
        .try_get(0)
        .unwrap();
    assert_eq!(rights, 0, "intake never grants publication rights");
}

#[tokio::test]
async fn denied_sessions_and_wrong_owner_never_read_or_write_sources() {
    let h = Harness::new().await;
    let receipt = h
        .admin
        .store()
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
            "UPDATE development_sessions SET revoked=1 WHERE actor_id='alice'",
            &[],
        )
        .await
        .unwrap();
    assert!(matches!(
        h.admin.store().load_import(ALICE, &receipt.id).await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(matches!(
        h.admin
            .store()
            .import_manuscript(ALICE, request(ImportFormat::Txt, b"changed"))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    const EXPIRED: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
    h.admin.seed_development_session("bob", EXPIRED, -1).await;
    assert!(matches!(
        h.admin
            .store()
            .import_manuscript(EXPIRED, request(ImportFormat::Txt, b"new"))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    const ACTIVE: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
    h.admin.seed_development_session("alice", ACTIVE, 1).await;
    h.admin
        .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
        .await
        .unwrap();
    assert!(matches!(
        h.admin.store().original_import(ACTIVE, &receipt.id).await,
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(h.counts().await, (1, 1, 0));
}

#[tokio::test]
async fn invalid_admission_and_injected_insert_failure_leave_no_partial_registry() {
    let h = Harness::new().await;
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
            h.admin.store().import_manuscript(ALICE, invalid).await,
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
    h.admin.exec("CREATE TRIGGER injected_import_failure BEFORE INSERT ON source_records BEGIN SELECT RAISE(ABORT,'cantos:test.import_failure'); END", &[]).await.unwrap();
    assert!(matches!(
        h.admin
            .store()
            .import_manuscript(ALICE, intent.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (0, 0, 0));
    h.admin
        .exec("DROP TRIGGER injected_import_failure", &[])
        .await
        .unwrap();
    h.admin.exec("CREATE TRIGGER fail_import_after_insert AFTER INSERT ON source_records BEGIN SELECT RAISE(ABORT,'cantos:test.after_insert'); END", &[]).await.unwrap();
    assert!(matches!(
        h.admin
            .store()
            .import_manuscript(ALICE, intent.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (0, 0, 0));
    h.admin
        .exec("DROP TRIGGER fail_import_after_insert", &[])
        .await
        .unwrap();
    let receipt = h
        .admin
        .store()
        .import_manuscript(ALICE, intent.clone())
        .await
        .unwrap();
    assert_eq!(
        h.admin
            .store()
            .import_manuscript(ALICE, intent)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(h.counts().await, (1, 1, 0));
}

#[tokio::test]
async fn script_links_share_the_preserved_original_but_never_grant_source_ownership() {
    let mut h = Harness::new().await;
    let text = h
        .admin
        .store()
        .import_manuscript(ALICE, request(ImportFormat::Txt, ORIGINAL.as_bytes()))
        .await
        .unwrap();
    let binary = h
        .admin
        .store()
        .import_manuscript(ALICE, request(ImportFormat::Docx, &[0xff, 0xfe]))
        .await
        .unwrap();
    let uncited = h
        .admin
        .store()
        .import_manuscript(ALICE, request(ImportFormat::Txt, b"private uncited"))
        .await
        .unwrap();
    for (kind, id) in cantos_server::script_ir::read_script(SCRIPT.as_bytes())
        .unwrap()
        .evidence_refs()
    {
        h.admin
            .exec(
                "INSERT INTO script_evidence VALUES('alice',?1,?2,'synthetic test reference')",
                &[&kind, &id],
            )
            .await
            .unwrap();
    }
    let mut value: Value = serde_json::from_str(SCRIPT).unwrap();
    value["provenance"][0]["source_record_id"] = json!(text.id);
    let script = Uuid::new_v4().to_string();
    h.admin
        .store()
        .save(
            ALICE,
            &script,
            cantos_server::contract::studio::SaveRevisionRequest {
                expected_revision: 0,
                operation_id: Uuid::new_v4().to_string(),
                script_json: value.to_string(),
            },
        )
        .await
        .unwrap();
    let path = format!("/scripts/{script}/sources/{}", text.id);
    let (status, body) = h.raw("GET", &path, Some(ALICE), vec![]).await;
    assert_eq!(status, StatusCode::OK);
    let source: cantos_server::contract::studio::SourceResponse =
        serde_json::from_slice(&body).unwrap();
    assert_eq!(source.original_text, ORIGINAL);
    assert_eq!(source.sha256, text.sha256);
    assert_eq!(source.recorded_at, text.recorded_at);
    assert_eq!(
        h.raw("GET", &path, Some(BOB), vec![]).await.0,
        StatusCode::NOT_FOUND
    );
    assert!(matches!(
        h.admin.store().source(ALICE, &script, &uncited.id).await,
        Err(StoreError::NotFound)
    ));
    h.admin
        .exec(
            "INSERT INTO script_members VALUES(?1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    assert_eq!(
        h.admin
            .store()
            .source(BOB, &script, &text.id)
            .await
            .unwrap(),
        source
    );
    assert!(matches!(
        h.admin.store().original_import(BOB, &text.id).await,
        Err(StoreError::NotFound)
    ));
    value["provenance"][0]["source_record_id"] = json!(binary.id);
    h.admin
        .store()
        .save(
            ALICE,
            &script,
            cantos_server::contract::studio::SaveRevisionRequest {
                expected_revision: 1,
                operation_id: Uuid::new_v4().to_string(),
                script_json: value.to_string(),
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        h.admin.store().source(ALICE, &script, &binary.id).await,
        Err(StoreError::InvalidRequest)
    ));
    assert_eq!(
        h.admin
            .store()
            .original_import(ALICE, &binary.id)
            .await
            .unwrap(),
        vec![0xff, 0xfe]
    );
    h.admin
        .exec("DELETE FROM script_members WHERE script_id=?1", &[&script])
        .await
        .unwrap();
    h.admin.restart().await;
    assert_eq!(
        h.admin
            .store()
            .source(ALICE, &script, &text.id)
            .await
            .unwrap(),
        source
    );
    assert!(matches!(
        h.admin.store().source(BOB, &script, &text.id).await,
        Err(StoreError::NotFound)
    ));
    assert_eq!(h.counts().await, (3, 4, 2));
}

#[tokio::test]
async fn legacy_text_only_sources_remain_readable_without_fabricating_an_import_receipt() {
    let mut h = Harness::new().await;
    let checksum = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
    for (kind, id) in cantos_server::script_ir::read_script(SCRIPT.as_bytes())
        .unwrap()
        .evidence_refs()
    {
        h.admin
            .exec(
                "INSERT INTO script_evidence VALUES('alice',?1,?2,'synthetic legacy reference')",
                &[&kind, &id],
            )
            .await
            .unwrap();
    }
    h.admin.exec("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','source-demo','legacy reference',?1,?2,0)", &[&ORIGINAL, &checksum]).await.unwrap();
    let script = Uuid::new_v4().to_string();
    h.admin
        .store()
        .save(
            ALICE,
            &script,
            cantos_server::contract::studio::SaveRevisionRequest {
                expected_revision: 0,
                operation_id: Uuid::new_v4().to_string(),
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    h.admin.restart().await;
    let source = h
        .admin
        .store()
        .source(ALICE, &script, "source-demo")
        .await
        .unwrap();
    assert_eq!(source.original_text, ORIGINAL);
    assert_eq!(source.sha256, checksum);
    assert_eq!(source.recorded_at, "1970-01-01T00:00:00.000000Z");
    assert!(matches!(
        h.admin.store().load_import(ALICE, "source-demo").await,
        Err(StoreError::NotFound)
    ));
}

#[tokio::test]
async fn bounded_native_import_point_reads_use_indexes_and_preserve_authorized_receipts() {
    let h = Harness::new().await;
    let mut bytes = "An: Ánh đèn cuối sân khấu. ".as_bytes().to_vec();
    bytes.resize(1024, b'x');
    let mut last = None;
    for _ in 0..1000 {
        last = Some(
            h.admin
                .store()
                .import_manuscript(ALICE, request(ImportFormat::Txt, &bytes))
                .await
                .unwrap(),
        );
    }
    let receipt = last.unwrap();
    let mut details = Vec::new();
    for (sql, params) in [
        ("EXPLAIN QUERY PLAN SELECT * FROM source_records WHERE owner_id=?1 AND id=?2 AND imported_by=?1", vec!["alice", receipt.id.as_str()]),
        ("EXPLAIN QUERY PLAN SELECT * FROM source_records WHERE imported_by=?1 AND import_operation_id=?2 AND import_operation_id IS NOT NULL", vec!["alice", receipt.metadata.operation_id.as_str()]),
    ] {
        let plan = h.admin.rows(sql, &[&params[0],&params[1]]).await.unwrap();
        let lines: Vec<String> = plan.iter().map(|row| row.try_get(3).unwrap()).collect();
        assert!(lines.iter().all(|line| !line.contains("SCAN ")), "{lines:?}");
        assert!(lines.iter().any(|line| line.contains("SEARCH source_records USING")), "{lines:?}");
        details.push(lines);
    }
    assert!(
        details[1]
            .iter()
            .any(|line| line.contains("source_import_operations")),
        "{details:?}"
    );
    let path = format!("/imports/{}", receipt.id);
    for _ in 0..5 {
        assert_eq!(
            h.json("GET", &path, Some(ALICE), None).await.0,
            StatusCode::OK
        );
    }
    let mut millis = Vec::new();
    for _ in 0..30 {
        let start = std::time::Instant::now();
        let (status, body) = h.raw("GET", &path, Some(ALICE), vec![]).await;
        millis.push(start.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_slice::<ImportResponse>(&body).unwrap(),
            receipt
        );
    }
    millis.sort_by(f64::total_cmp);
    println!(
        "{}",
        json!({"engine":"Turso =0.8.3","profile":"debug","synthetic_sources":1000,"bytes_per_source":1024,"warmups":5,"samples":30,"concurrency":1,"p50_ms":millis[14],"p95_ms":millis[28],"p99_ms":millis[29],"plan":details,"scope":"local authorized HTTP; no production or PG speedup claim"})
    );
    assert!(
        millis[28] <= 250.0,
        "bounded local warm-read p95 was {}ms",
        millis[28]
    );
    assert_eq!(h.counts().await, (1000, 1000, 0));
}
