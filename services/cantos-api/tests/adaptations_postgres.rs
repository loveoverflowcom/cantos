//! Independent production-store/HTTP oracles on a fresh disposable PostgreSQL cluster.
//! All manuscripts and proposal documents are original synthetic Vietnamese fixtures.
//! No model or provider is called: external callers submit bounded, untrusted proposal bytes.
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use cantos_api::{
    adaptation::{admit_output, TrustedBinding},
    http::{router, AppState},
    postgres::{local_config, migrate, token_hash, Store, StoreError},
    script_ir::read_script,
};
use cantos_contracts::{
    AcceptAdaptationRequest, AdaptationConfig, AdaptationContextRequest, AdaptationContextResponse,
    AdaptationCost, AdaptationCostBasis, AdaptationCurrency, AdaptationProviderMetadata,
    AdaptationRunResponse, AdaptationStatus, AdaptationSubmissionReceipt,
    AdaptationSubmissionStatus, AdaptationUsage, CallerGenerationMetadata, CancelAdaptationRequest,
    HistoryResponse, ImportFormat, ImportMetadata, ImportOutcome, ImportRequest, ImportResponse,
    ReviewRequest, ReviewResponse, RevisionResponse, SaveRevisionRequest, ScriptValidationResponse,
    SourceResponse, StartAdaptationRequest, SubmitAdaptationProposalRequest, ValidateScriptRequest,
};
use http_body_util::BodyExt;
use serde::Serialize;
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
const ORIGINAL: &str = "Đêm xuống, đèn Vọng Đài vẫn sáng.\r\n\r\nMai: Chúng mình sẽ chờ ở đây.\r\n\r\n— Ai đã mở cửa?\r\n";

fn document() -> String {
    json!({
        "contract_version":"cantos-adaptation-1","title":"Ngọn đèn Vọng Đài",
        "characters":[{"name":"Mai","personality":"Chưa xác định"}],
        "scenes":[{
            "title":"Bên cánh cửa","lines":[
                {"speaker":"Người dẫn chuyện","text":"Đêm xuống, đèn Vọng Đài vẫn sáng.","source_blocks":[0],"delivery":{"emotion":"calm","intensity_permille":300}},
                {"speaker":"Mai","text":"Chúng mình sẽ chờ ở đây.","source_blocks":[1],"delivery":{"emotion":"neutral","intensity_permille":300}},
                {"speaker":null,"text":"Ai đã mở cửa?","source_blocks":[2],"delivery":{"emotion":"neutral","intensity_permille":450}}
            ],"cues":[
                {"kind":"ambience","description":"Không khí đêm yên tĩnh","line_index":0,"edge":"start","source_blocks":[0]},
                {"kind":"music","description":"Nhạc nền cần tác giả duyệt","line_index":1,"edge":"start","source_blocks":[1]},
                {"kind":"sfx","description":"Tiếng cửa cần xác minh","line_index":2,"edge":"end","source_blocks":[2]}
            ],"pacing_note":"Nhịp chậm; khoảng dừng cần biên tập viên quyết định"
        }],"omitted_blocks":[],"review_notes":["Chưa xác định người đặt câu hỏi; không suy đoán danh tính"]
    }).to_string()
}

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
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        );
        cluster
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .unwrap();
        config.dbname(&database);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin.batch_execute("GRANT USAGE ON SCHEMA public TO cantos_app; GRANT SELECT ON actors,sessions,script_members,script_evidence,script_revisions,revision_evidence,scripts,source_records,script_reviews,script_review_operations TO cantos_app; GRANT INSERT ON scripts,script_revisions,revision_evidence,script_evidence,source_records TO cantos_app; GRANT UPDATE(head_revision) ON scripts TO cantos_app; GRANT UPDATE(revoked) ON sessions TO cantos_app; INSERT INTO actors(id) VALUES('alice'),('bob');").await.unwrap();
        admin.batch_execute("GRANT SELECT,INSERT ON adaptation_runs,adaptation_attempts,adaptation_observations,adaptation_proposals,adaptation_cancellations,adaptation_acceptances,adaptation_submissions TO cantos_app; GRANT UPDATE(status,problem,dispatch_deadline,updated_at) ON adaptation_runs TO cantos_app;").await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,CURRENT_TIMESTAMP+interval '1 hour')",&[&token_hash(token),&actor]).await.unwrap();
        }
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-adaptation-test");
        let store = Store::new(app_config).unwrap();
        let app = make_router(store.clone());
        Self {
            admin,
            config,
            store,
            app,
            database,
        }
    }

    async fn import(&self, token: &str) -> ImportResponse {
        self.store
            .import_manuscript(
                token,
                ImportRequest {
                    metadata: ImportMetadata {
                        operation_id: operation(),
                        file_name: "Vọng Đài — synthetic.txt".into(),
                        format: ImportFormat::Txt,
                        reference: "Original synthetic adaptation integration fixture".into(),
                        rights_holder: Some("Synthetic test author".into()),
                        permission_evidence: Some(
                            "Local tests only; no publication clearance".into(),
                        ),
                        usage_scope: Some("private local adaptation".into()),
                    },
                    original_bytes: ORIGINAL.as_bytes().to_vec(),
                },
            )
            .await
            .unwrap()
    }

    async fn counts(&self) -> (i64, i64, i64, i64, i64) {
        let row=self.admin.query_one("SELECT (SELECT count(*) FROM adaptation_runs),(SELECT count(*) FROM adaptation_attempts),(SELECT count(*) FROM adaptation_proposals),(SELECT count(*) FROM adaptation_acceptances),(SELECT count(*) FROM script_revisions)",&[]).await.unwrap();
        (row.get(0), row.get(1), row.get(2), row.get(3), row.get(4))
    }
    async fn submissions(&self) -> i64 {
        self.admin
            .query_one("SELECT count(*) FROM adaptation_submissions", &[])
            .await
            .unwrap()
            .get(0)
    }
    async fn context(
        &self,
        source: &ImportResponse,
        script: &str,
        revision: u64,
    ) -> AdaptationContextResponse {
        self.store
            .create_adaptation_context(ALICE, context_request(source, script, revision))
            .await
            .unwrap()
    }
    async fn proposed(
        &self,
        source: &ImportResponse,
        script: &str,
        revision: u64,
    ) -> AdaptationContextResponse {
        let context = self.context(source, script, revision).await;
        let receipt = self
            .store
            .submit_adaptation_proposal(ALICE, &context.id, submission(&context, document()))
            .await
            .unwrap();
        assert_eq!(receipt.status, AdaptationSubmissionStatus::Valid);
        self.store
            .load_adaptation_context(ALICE, &context.id)
            .await
            .unwrap()
    }
    async fn http(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.raw_http(
            method,
            path,
            token,
            body.map(|value| value.to_string().into_bytes())
                .unwrap_or_default(),
            ORIGIN,
        )
        .await
    }
    async fn raw_http(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        bytes: Vec<u8>,
        origin: &str,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder()
            .method(method)
            .uri(format!("/api/v1{path}"))
            .header("origin", origin)
            .header("content-type", "application/json")
            .header("x-actor-id", "alice");
        if let Some(token) = token {
            request = request.header("cookie", format!("cantos_session={token}"));
        }
        let response = self
            .app
            .clone()
            .oneshot(request.body(Body::from(bytes)).unwrap())
            .await
            .unwrap();
        assert_eq!(response.headers()["cache-control"], "no-store");
        assert_eq!(response.headers()["x-content-type-options"], "nosniff");
        let status = response.status();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        (status, serde_json::from_slice(&bytes).unwrap())
    }
    async fn originals_unchanged(&self, source: &ImportResponse) {
        assert_eq!(
            self.store.original_import(ALICE, &source.id).await.unwrap(),
            ORIGINAL.as_bytes()
        );
        assert_eq!(
            self.store.load_import(ALICE, &source.id).await.unwrap(),
            *source
        );
        assert_eq!(source.sha256, sha(ORIGINAL.as_bytes()));
    }
    fn reopened(&self) -> Store {
        let mut config = self.config.clone();
        config.user("cantos_app");
        Store::new(config).unwrap()
    }
}

fn make_router(store: Store) -> Router {
    router(
        AppState {
            store,
            web_origin: ORIGIN.into(),
        },
        "apps/cantos-studio/web/dist",
    )
}
fn operation() -> String {
    Uuid::new_v4().to_string()
}
fn sha(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}
fn context_request(
    source: &ImportResponse,
    script: &str,
    revision: u64,
) -> AdaptationContextRequest {
    let ImportOutcome::Parsed { extraction } = &source.outcome else {
        panic!("parsed fixture required")
    };
    AdaptationContextRequest {
        operation_id: operation(),
        source_id: source.id.clone(),
        source_sha256: source.sha256.clone(),
        extractor_version: extraction.extractor_version.clone(),
        script_id: script.into(),
        expected_revision: revision,
        rights_authorization: true,
    }
}
fn submission(
    context: &AdaptationContextResponse,
    proposal_json: String,
) -> SubmitAdaptationProposalRequest {
    SubmitAdaptationProposalRequest {
        operation_id: operation(),
        context_digest: context.context_digest.clone(),
        proposal_json,
        generation: CallerGenerationMetadata {
            host_tool: "synthetic-integration-caller".into(),
            provider: None,
            model: None,
            configuration_json: None,
            prompt_version: context.prompt_version.clone(),
            usage: None,
            cost: None,
        },
    }
}
fn accept(context: &AdaptationContextResponse) -> AcceptAdaptationRequest {
    AcceptAdaptationRequest {
        operation_id: operation(),
        expected_revision: context.request.expected_revision,
        script_json: context.proposal.as_ref().unwrap().script_json.clone(),
        reviewed_findings: true,
    }
}
fn edit(saved: &RevisionResponse, revision: u64) -> SaveRevisionRequest {
    let mut value: Value = serde_json::from_str(&saved.script_json).unwrap();
    value["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"] =
        json!("Chúng mình sẽ chờ đến sáng.");
    SaveRevisionRequest {
        operation_id: operation(),
        expected_revision: revision,
        script_json: value.to_string(),
    }
}
async fn bounded<T>(future: impl std::future::Future<Output = T>) -> T {
    tokio::time::timeout(Duration::from_secs(10), future)
        .await
        .expect("adaptation integration barrier timed out")
}
async fn blocked_connections(admin: &Client, expected: i64) {
    bounded(async {
        loop {
            let waiting:i64=admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='cantos-adaptation-test' AND wait_event_type='Lock'",&[]).await.unwrap().get(0);
            if waiting>=expected {break}
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    }).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn context_exports_frozen_source_and_valid_submission_requires_explicit_reviewed_acceptance()
{
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let script = operation();
    let request = context_request(&source, &script, 0);
    let (status, value) = h
        .http(
            "POST",
            "/adaptations/contexts",
            Some(ALICE),
            Some(json!(request)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let context: AdaptationContextResponse = serde_json::from_value(value).unwrap();
    assert_eq!(context.request, request);
    assert_eq!(context.created_by, "alice");
    assert_eq!(context.source, source);
    assert_eq!(context.prompt_version, "cantos-radio-adapt-1");
    assert_eq!(context.contract_version, "cantos-adaptation-1");
    assert_eq!(context.status, AdaptationStatus::AwaitingProposal);
    assert_eq!(context.input_revision, None);
    assert_eq!(context.proposal, None);
    assert_eq!(context.accepted_revision, None);
    assert_eq!(context.latest_submission, None);
    assert_eq!(context.context_digest.len(), 64);
    let prompt: Value = serde_json::from_str(&context.user_prompt).unwrap();
    assert_eq!(prompt["source"]["id"], source.id);
    assert_eq!(prompt["source"]["sha256"], sha(ORIGINAL.as_bytes()));
    assert_eq!(prompt["source"]["extractor_version"], "cantos-import-1");
    assert_eq!(prompt["source"]["blocks"][2]["text"], "— Ai đã mở cửa?");
    assert!(context.system_prompt.contains("inert source data"));
    assert_eq!(
        serde_json::from_str::<Value>(&context.proposal_schema_json).unwrap()["type"],
        "object"
    );
    assert_eq!(h.counts().await, (1, 0, 0, 0, 0));

    let intent = submission(&context, document());
    let (status, value) = h
        .http(
            "POST",
            &format!("/adaptations/{}/proposals", context.id),
            Some(ALICE),
            Some(json!(intent)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let receipt: AdaptationSubmissionReceipt = serde_json::from_value(value).unwrap();
    assert_eq!(receipt.status, AdaptationSubmissionStatus::Valid);
    assert_eq!(receipt.output_sha256, sha(intent.proposal_json.as_bytes()));
    assert_eq!(receipt.generation, intent.generation);
    assert_eq!(receipt.generation.usage, None);
    assert_eq!(receipt.generation.cost, None);
    assert_eq!(receipt.submitted_by, "alice");
    let proposed = h
        .store
        .load_adaptation_context(ALICE, &context.id)
        .await
        .unwrap();
    assert_eq!(proposed.status, AdaptationStatus::Succeeded);
    assert_eq!(proposed.latest_submission, Some(receipt.clone()));
    assert_eq!(proposed.proposal, receipt.proposal);
    assert_eq!(proposed.context_digest, context.context_digest);
    let proposal = proposed.proposal.as_ref().unwrap();
    assert_eq!(
        read_script(proposal.script_json.as_bytes())
            .unwrap()
            .export_bytes(),
        proposal.script_json.as_bytes()
    );
    let script_value: Value = serde_json::from_str(&proposal.script_json).unwrap();
    assert_eq!(script_value["provenance"][0]["source_record_id"], source.id);
    assert!(script_value["provenance"]
        .as_array()
        .unwrap()
        .iter()
        .any(|p| p["generation_record_id"] == context.generation_record_id));
    assert_eq!(
        proposal
            .coverage
            .iter()
            .map(|coverage| coverage.block)
            .collect::<Vec<_>>(),
        [0, 1, 2]
    );
    for code in [
        "unresolved_speaker",
        "source_warning",
        "unsupported_performance_control",
    ] {
        assert!(
            proposal
                .findings
                .iter()
                .any(|finding| json!(finding)["code"] == code),
            "{code}"
        );
    }
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    let reviews: i64 = h
        .admin
        .query_one("SELECT count(*) FROM script_reviews", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(reviews, 0);
    let mut unreviewed = accept(&proposed);
    unreviewed.reviewed_findings = false;
    assert!(matches!(
        h.store
            .accept_adaptation(ALICE, &proposed.id, unreviewed)
            .await,
        Err(StoreError::InvalidRequestFields(_))
    ));
    let acceptance = accept(&proposed);
    let saved = h
        .store
        .accept_adaptation(ALICE, &proposed.id, acceptance.clone())
        .await
        .unwrap();
    assert_eq!(saved.script_id, script);
    assert_eq!(saved.revision, 1);
    assert_eq!(saved.accepted_by, "alice");
    assert_eq!(saved.script_json, proposal.script_json);
    assert_eq!(
        h.store
            .accept_adaptation(ALICE, &proposed.id, acceptance)
            .await
            .unwrap(),
        saved
    );
    let accepted = h
        .store
        .load_adaptation_context(ALICE, &proposed.id)
        .await
        .unwrap();
    assert_eq!(accepted.status, AdaptationStatus::Accepted);
    assert_eq!(accepted.accepted_revision, Some(saved.clone()));
    assert_eq!(accepted.proposal, proposed.proposal);
    assert_eq!(h.store.load(ALICE, &script, Some(1)).await.unwrap(), saved);
    assert_eq!(h.counts().await, (1, 0, 1, 1, 1));
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_context_and_submission_retries_are_exact_and_changed_operations_conflict() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let intent = context_request(&source, &operation(), 0);
    let (first, second) = tokio::join!(
        h.store.create_adaptation_context(ALICE, intent.clone()),
        h.store.create_adaptation_context(ALICE, intent.clone())
    );
    let context = first.unwrap();
    assert_eq!(second.unwrap(), context);
    let mut changed = intent;
    changed.script_id = operation();
    assert!(matches!(
        h.store.create_adaptation_context(ALICE, changed).await,
        Err(StoreError::OperationReused)
    ));
    let intent = submission(&context, document());
    let (first, second) = tokio::join!(
        h.store
            .submit_adaptation_proposal(ALICE, &context.id, intent.clone()),
        h.store
            .submit_adaptation_proposal(ALICE, &context.id, intent.clone())
    );
    let receipt = first.unwrap();
    assert_eq!(second.unwrap(), receipt);
    for variant in 0..5 {
        let mut changed = intent.clone();
        match variant {
            0 => changed.proposal_json.push(' '),
            1 => changed.generation.host_tool.push_str("-changed"),
            2 => changed.generation.model = Some("declared-other-model".into()),
            3 => changed.generation.configuration_json = Some("{\"temperature\":0}".into()),
            4 => {
                changed.generation.usage = Some(AdaptationUsage {
                    input_tokens: Some(1),
                    output_tokens: None,
                })
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            h.store
                .submit_adaptation_proposal(ALICE, &context.id, changed)
                .await,
            Err(StoreError::OperationReused)
        ));
    }
    assert_eq!(h.submissions().await, 1);
    assert!(matches!(
        h.store
            .submit_adaptation_proposal(ALICE, &context.id, submission(&context, document()))
            .await,
        Err(StoreError::ProposalAlreadySubmitted)
    ));
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    assert_eq!(
        h.store
            .submit_adaptation_proposal(ALICE, &context.id, intent)
            .await
            .unwrap(),
        receipt
    );
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn invalid_model_bytes_record_immutable_caller_receipts_and_new_operation_can_correct_them() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.context(&source, &operation(), 0).await;
    let mut semantic: Value = serde_json::from_str(&document()).unwrap();
    semantic["scenes"][0]["lines"][0]["source_blocks"] = json!([999]);
    for raw in [
        "not JSON".to_string(),
        "{}".to_string(),
        semantic.to_string(),
        "x".repeat(256 * 1024 + 1),
    ] {
        let mut intent = submission(&context, raw);
        intent.generation.usage = Some(AdaptationUsage {
            input_tokens: Some(87),
            output_tokens: Some(123),
        });
        let receipt = h
            .store
            .submit_adaptation_proposal(ALICE, &context.id, intent.clone())
            .await
            .unwrap();
        assert_eq!(receipt.status, AdaptationSubmissionStatus::Invalid);
        assert_eq!(receipt.proposal, None);
        assert!(receipt.problem.is_some());
        assert_eq!(receipt.output_sha256, sha(intent.proposal_json.as_bytes()));
        assert_eq!(receipt.generation.usage, intent.generation.usage);
        assert_eq!(
            receipt.generation.cost, None,
            "unknown caller cost is not zero"
        );
        assert_eq!(
            h.store
                .submit_adaptation_proposal(ALICE, &context.id, intent)
                .await
                .unwrap(),
            receipt
        );
        let reloaded = h
            .store
            .load_adaptation_context(ALICE, &context.id)
            .await
            .unwrap();
        assert_eq!(reloaded.status, AdaptationStatus::AwaitingProposal);
        assert_eq!(reloaded.proposal, None);
        assert_eq!(reloaded.latest_submission, Some(receipt));
        assert_eq!(h.counts().await, (1, 0, 0, 0, 0));
    }
    let intent = submission(&context, document());
    let receipt = h
        .store
        .submit_adaptation_proposal(ALICE, &context.id, intent)
        .await
        .unwrap();
    assert_eq!(receipt.status, AdaptationSubmissionStatus::Valid);
    assert_eq!(h.submissions().await, 5);
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn source_checksum_extractor_rights_and_bounds_are_checked_before_context_export() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    for variant in 0..3 {
        let mut request = context_request(&source, &operation(), 0);
        match variant {
            0 => request.source_sha256 = "0".repeat(64),
            1 => request.extractor_version = "unreviewed-extractor-2".into(),
            2 => request.rights_authorization = false,
            _ => unreachable!(),
        }
        assert!(matches!(
            h.store.create_adaptation_context(ALICE, request).await,
            Err(StoreError::InvalidRequestFields(_))
        ));
        assert_eq!(h.counts().await, (0, 0, 0, 0, 0));
    }
    for (bytes, absent_claim) in [
        (ORIGINAL.as_bytes().to_vec(), true),
        (vec![0xff], false),
        (vec![b'x'; 24_577], false),
    ] {
        let mut metadata = source.metadata.clone();
        metadata.operation_id = operation();
        if absent_claim {
            metadata.permission_evidence = None;
        }
        let imported = h
            .store
            .import_manuscript(
                ALICE,
                ImportRequest {
                    metadata,
                    original_bytes: bytes,
                },
            )
            .await
            .unwrap();
        let extractor_version = match &imported.outcome {
            ImportOutcome::Parsed { extraction } => extraction.extractor_version.clone(),
            _ => "cantos-import-1".into(),
        };
        let failed = matches!(imported.outcome, ImportOutcome::Failed { .. });
        let request = AdaptationContextRequest {
            operation_id: operation(),
            source_id: imported.id,
            source_sha256: imported.sha256,
            extractor_version,
            script_id: operation(),
            expected_revision: 0,
            rights_authorization: true,
        };
        let result = h.store.create_adaptation_context(ALICE, request).await;
        if failed {
            assert!(matches!(result, Err(StoreError::InvalidRequest)));
        } else {
            assert!(matches!(result, Err(StoreError::InvalidRequestFields(_))));
        }
    }
    assert_eq!(h.counts().await, (0, 0, 0, 0, 0));
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn context_digest_prompt_version_and_caller_metadata_cannot_be_forged() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.context(&source, &operation(), 0).await;
    for variant in 0..5 {
        let mut intent = submission(&context, document());
        match variant {
            0 => intent.context_digest = "0".repeat(64),
            1 => intent.generation.prompt_version = "unreviewed-prompt-2".into(),
            2 => intent.generation.host_tool = " ".into(),
            3 => intent.generation.configuration_json = Some("not JSON".into()),
            4 => {
                intent.generation.cost = Some(AdaptationCost {
                    currency: AdaptationCurrency::USD,
                    amount_minor: 5,
                    basis: AdaptationCostBasis::ProviderReported,
                })
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            h.store
                .submit_adaptation_proposal(ALICE, &context.id, intent)
                .await,
            Err(StoreError::InvalidRequestFields(_))
        ));
        assert_eq!(h.submissions().await, 0);
    }
    let other = h.context(&source, &operation(), 0).await;
    assert!(matches!(
        h.store
            .submit_adaptation_proposal(ALICE, &other.id, submission(&context, document()))
            .await,
        Err(StoreError::InvalidRequestFields(_))
    ));
    let mut intent = submission(&context, document());
    intent.generation.cost = Some(AdaptationCost {
        currency: AdaptationCurrency::VND,
        amount_minor: 100,
        basis: AdaptationCostBasis::CallerDeclared,
    });
    let receipt = h
        .store
        .submit_adaptation_proposal(ALICE, &context.id, intent.clone())
        .await
        .unwrap();
    assert_eq!(receipt.generation.cost, intent.generation.cost);
    assert_eq!(receipt.generation.provider, None);
    assert_eq!(receipt.generation.model, None);
    assert_eq!(h.counts().await, (2, 0, 1, 0, 0));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn hostile_http_unknown_fields_malformed_json_bad_origin_and_oversized_bodies_write_nothing()
{
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let request = context_request(&source, &operation(), 0);
    let mut forged = json!(request);
    forged["host_command"] = json!("inert data must not invoke host tools");
    assert_eq!(
        h.http("POST", "/adaptations/contexts", Some(ALICE), Some(forged))
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.raw_http(
            "POST",
            "/adaptations/contexts",
            Some(ALICE),
            b"{bad".to_vec(),
            ORIGIN
        )
        .await
        .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.raw_http(
            "POST",
            "/adaptations/contexts",
            Some(ALICE),
            json!(request).to_string().into_bytes(),
            "https://example.invalid"
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        h.raw_http(
            "POST",
            "/adaptations/contexts",
            Some(ALICE),
            vec![b'x'; 64 * 1024 + 1],
            ORIGIN
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert_eq!(h.counts().await, (0, 0, 0, 0, 0));
    let context = h.context(&source, &operation(), 0).await;
    let path = format!("/adaptations/{}/proposals", context.id);
    let mut forged = json!(submission(&context, document()));
    forged["generation"]["host_command"] = json!("untrusted");
    assert_eq!(
        h.http("POST", &path, Some(ALICE), Some(forged)).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.raw_http("POST", &path, Some(ALICE), b"{bad".to_vec(), ORIGIN)
            .await
            .0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        h.raw_http(
            "POST",
            &path,
            Some(ALICE),
            vec![b'x'; 1024 * 1024 + 1],
            ORIGIN
        )
        .await
        .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(matches!(
        h.store
            .submit_adaptation_proposal(
                ALICE,
                &context.id,
                submission(&context, "x".repeat(1024 * 1024 + 1))
            )
            .await,
        Err(StoreError::InvalidRequestFields(_))
    ));
    assert_eq!(h.counts().await, (1, 0, 0, 0, 0));
    assert_eq!(h.submissions().await, 0);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn anonymous_foreign_revoked_and_expired_sessions_cannot_export_submit_review_or_accept() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.context(&source, &operation(), 0).await;
    assert!(matches!(
        h.store
            .create_adaptation_context(BOB, context_request(&source, &operation(), 0))
            .await,
        Err(StoreError::NotFound)
    ));
    for token in [None, Some(BOB)] {
        for path in [
            format!("/adaptations/{}/context", context.id),
            format!("/adaptations/{}/review", context.id),
        ] {
            let status = h.http("GET", &path, token, None).await.0;
            assert_eq!(
                status,
                if token.is_none() {
                    StatusCode::UNAUTHORIZED
                } else {
                    StatusCode::NOT_FOUND
                }
            );
        }
        let status = h
            .http(
                "POST",
                &format!("/adaptations/{}/proposals", context.id),
                token,
                Some(json!(submission(&context, document()))),
            )
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
    let receipt = h
        .store
        .submit_adaptation_proposal(ALICE, &context.id, submission(&context, document()))
        .await
        .unwrap();
    assert_eq!(receipt.status, AdaptationSubmissionStatus::Valid);
    let proposed = h
        .store
        .load_adaptation_context(ALICE, &context.id)
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .accept_adaptation(BOB, &context.id, accept(&proposed))
            .await,
        Err(StoreError::NotFound)
    ));
    h.admin
        .execute(
            "UPDATE sessions SET revoked=true WHERE actor_id='alice'",
            &[],
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_adaptation_context(ALICE, &context.id).await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(matches!(
        h.store
            .submit_adaptation_proposal(ALICE, &context.id, submission(&context, document()))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    assert!(matches!(
        h.store
            .accept_adaptation(ALICE, &context.id, accept(&proposed))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    h.admin.execute("UPDATE sessions SET revoked=false,expires_at=clock_timestamp()-interval '1 second' WHERE actor_id='alice'",&[]).await.unwrap();
    assert!(matches!(
        h.store
            .create_adaptation_context(ALICE, context_request(&source, &operation(), 0))
            .await,
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    assert_eq!(h.submissions().await, 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn frozen_base_stale_acceptance_and_concurrent_exact_acceptance_preserve_all_revisions() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let script = operation();
    let first = h.proposed(&source, &script, 0).await;
    let base = h
        .store
        .accept_adaptation(ALICE, &first.id, accept(&first))
        .await
        .unwrap();
    let next = h.proposed(&source, &script, 1).await;
    assert_eq!(next.input_revision, Some(base.clone()));
    assert!(matches!(
        h.store
            .create_adaptation_context(ALICE, context_request(&source, &script, 0))
            .await,
        Err(StoreError::StaleRevision(1))
    ));
    let waiting = h.context(&source, &script, 1).await;
    let edited = h.store.save(ALICE, &script, edit(&base, 1)).await.unwrap();
    assert!(matches!(
        h.store
            .submit_adaptation_proposal(ALICE, &waiting.id, submission(&waiting, document()))
            .await,
        Err(StoreError::StaleRevision(2))
    ));
    assert_eq!(
        h.store
            .load_adaptation_context(ALICE, &waiting.id)
            .await
            .unwrap(),
        waiting
    );
    assert!(matches!(
        h.store
            .accept_adaptation(ALICE, &next.id, accept(&next))
            .await,
        Err(StoreError::StaleRevision(2))
    ));
    assert_eq!(h.store.load(ALICE, &script, Some(1)).await.unwrap(), base);
    assert_eq!(h.store.load(ALICE, &script, None).await.unwrap(), edited);
    assert_eq!(
        h.store
            .load_adaptation_context(ALICE, &next.id)
            .await
            .unwrap(),
        next
    );
    let last = h.proposed(&source, &script, 2).await;
    let intent = accept(&last);
    let (first, second) = tokio::join!(
        h.store.accept_adaptation(ALICE, &last.id, intent.clone()),
        h.store.accept_adaptation(ALICE, &last.id, intent.clone())
    );
    let saved = first.unwrap();
    assert_eq!(second.unwrap(), saved);
    assert_eq!(saved.revision, 3);
    let mut changed = intent;
    changed.script_json.push(' ');
    assert!(matches!(
        h.store.accept_adaptation(ALICE, &last.id, changed).await,
        Err(StoreError::OperationReused)
    ));
    assert_eq!(h.store.load(ALICE, &script, Some(1)).await.unwrap(), base);
    assert_eq!(h.store.load(ALICE, &script, Some(2)).await.unwrap(), edited);
    assert_eq!(h.counts().await, (4, 0, 3, 2, 3));
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn failed_context_creation_rolls_back_run_and_registered_evidence_then_exact_retry_recovers()
{
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let evidence: i64 = h
        .admin
        .query_one("SELECT count(*) FROM script_evidence", &[])
        .await
        .unwrap()
        .get(0);
    h.admin.batch_execute("CREATE FUNCTION test_context_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic context insert failure'; END $$; CREATE TRIGGER test_context_fail BEFORE INSERT ON adaptation_runs FOR EACH ROW EXECUTE FUNCTION test_context_fail();").await.unwrap();
    let intent = context_request(&source, &operation(), 0);
    assert!(matches!(
        h.store
            .create_adaptation_context(ALICE, intent.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (0, 0, 0, 0, 0));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM script_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        evidence
    );
    h.admin
        .batch_execute(
            "DROP TRIGGER test_context_fail ON adaptation_runs; DROP FUNCTION test_context_fail();",
        )
        .await
        .unwrap();
    let context = h
        .store
        .create_adaptation_context(ALICE, intent.clone())
        .await
        .unwrap();
    assert_eq!(
        h.store
            .create_adaptation_context(ALICE, intent)
            .await
            .unwrap(),
        context
    );
    assert_eq!(h.counts().await, (1, 0, 0, 0, 0));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn failed_submission_receipt_or_proposal_insert_rolls_back_whole_transition_and_exact_retry_recovers(
) {
    for table in ["adaptation_submissions", "adaptation_proposals"] {
        let h = Harness::new(None).await;
        let source = h.import(ALICE).await;
        let context = h.context(&source, &operation(), 0).await;
        let intent = submission(&context, document());
        h.admin.batch_execute(&format!("CREATE FUNCTION test_submission_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic submission insert failure'; END $$; CREATE TRIGGER test_submission_fail BEFORE INSERT ON {table} FOR EACH ROW EXECUTE FUNCTION test_submission_fail();")).await.unwrap();
        assert!(matches!(
            h.store
                .submit_adaptation_proposal(ALICE, &context.id, intent.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        assert_eq!(h.submissions().await, 0);
        assert_eq!(h.counts().await, (1, 0, 0, 0, 0));
        assert_eq!(
            h.store
                .load_adaptation_context(ALICE, &context.id)
                .await
                .unwrap(),
            context
        );
        h.admin.batch_execute(&format!("DROP TRIGGER test_submission_fail ON {table}; DROP FUNCTION test_submission_fail();")).await.unwrap();
        let receipt = h
            .store
            .submit_adaptation_proposal(ALICE, &context.id, intent.clone())
            .await
            .unwrap();
        assert_eq!(
            h.store
                .submit_adaptation_proposal(ALICE, &context.id, intent)
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(h.submissions().await, 1);
        assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn failed_acceptance_insert_rolls_back_revision_head_and_receipt_then_exact_retry_recovers() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.proposed(&source, &operation(), 0).await;
    let intent = accept(&context);
    h.admin.batch_execute("CREATE FUNCTION test_acceptance_fail() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'synthetic acceptance insert failure'; END $$; CREATE TRIGGER test_acceptance_fail BEFORE INSERT ON adaptation_acceptances FOR EACH ROW EXECUTE FUNCTION test_acceptance_fail();").await.unwrap();
    assert!(matches!(
        h.store
            .accept_adaptation(ALICE, &context.id, intent.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM scripts", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    assert_eq!(
        h.store
            .load_adaptation_context(ALICE, &context.id)
            .await
            .unwrap(),
        context
    );
    h.admin.batch_execute("DROP TRIGGER test_acceptance_fail ON adaptation_acceptances; DROP FUNCTION test_acceptance_fail();").await.unwrap();
    let saved = h
        .store
        .accept_adaptation(ALICE, &context.id, intent.clone())
        .await
        .unwrap();
    assert_eq!(
        h.store
            .accept_adaptation(ALICE, &context.id, intent)
            .await
            .unwrap(),
        saved
    );
    assert_eq!(h.counts().await, (1, 0, 1, 1, 1));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn context_and_submission_replay_after_reopened_store_never_dispatch_or_replace_history() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context_intent = context_request(&source, &operation(), 0);
    let context = h
        .store
        .create_adaptation_context(ALICE, context_intent.clone())
        .await
        .unwrap();
    let invalid = submission(&context, "not JSON".into());
    let invalid_receipt = h
        .store
        .submit_adaptation_proposal(ALICE, &context.id, invalid.clone())
        .await
        .unwrap();
    let reopened = h.reopened();
    assert_eq!(
        reopened
            .submit_adaptation_proposal(ALICE, &context.id, invalid)
            .await
            .unwrap(),
        invalid_receipt
    );
    let valid = submission(&context, document());
    let receipt = reopened
        .submit_adaptation_proposal(ALICE, &context.id, valid.clone())
        .await
        .unwrap();
    let proposed = reopened
        .load_adaptation_context(ALICE, &context.id)
        .await
        .unwrap();
    let intent = accept(&proposed);
    let saved = reopened
        .accept_adaptation(ALICE, &context.id, intent.clone())
        .await
        .unwrap();
    let reopened = h.reopened();
    assert_eq!(
        reopened
            .submit_adaptation_proposal(ALICE, &context.id, valid)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(
        reopened
            .accept_adaptation(ALICE, &context.id, intent)
            .await
            .unwrap(),
        saved
    );
    let accepted = reopened
        .load_adaptation_context(ALICE, &context.id)
        .await
        .unwrap();
    assert_eq!(
        reopened
            .create_adaptation_context(ALICE, context_intent)
            .await
            .unwrap(),
        accepted
    );
    assert_eq!(accepted.accepted_revision, Some(saved));
    assert_eq!(h.submissions().await, 2);
    assert_eq!(h.counts().await, (1, 0, 1, 1, 1));
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn cancellation_fences_late_external_submission_and_exact_cancel_receipts_replay() {
    for proposed in [false, true] {
        let h = Harness::new(None).await;
        let source = h.import(ALICE).await;
        let context = if proposed {
            h.proposed(&source, &operation(), 0).await
        } else {
            h.context(&source, &operation(), 0).await
        };
        let intent = CancelAdaptationRequest {
            operation_id: operation(),
        };
        let cancelled = h
            .store
            .cancel_adaptation_context(ALICE, &context.id, intent.clone())
            .await
            .unwrap();
        assert_eq!(cancelled.status, AdaptationStatus::Cancelled);
        assert_eq!(cancelled.proposal, context.proposal);
        assert_eq!(
            h.store
                .cancel_adaptation_context(ALICE, &context.id, intent.clone())
                .await
                .unwrap(),
            cancelled
        );
        assert_eq!(
            h.reopened()
                .cancel_adaptation_context(ALICE, &context.id, intent)
                .await
                .unwrap(),
            cancelled
        );
        assert!(matches!(
            h.store
                .submit_adaptation_proposal(ALICE, &context.id, submission(&context, document()))
                .await,
            Err(StoreError::ProposalAlreadySubmitted)
        ));
        if proposed {
            assert!(matches!(
                h.store
                    .accept_adaptation(ALICE, &context.id, accept(&context))
                    .await,
                Err(StoreError::InvalidRequest)
            ));
        }
        assert_eq!(h.counts().await, (1, 0, if proposed { 1 } else { 0 }, 0, 0));
        h.originals_unchanged(&source).await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn settled_contexts_submissions_proposals_and_accepted_revisions_are_immutable() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.proposed(&source, &operation(), 0).await;
    let saved = h
        .store
        .accept_adaptation(ALICE, &context.id, accept(&context))
        .await
        .unwrap();
    let mut config = h.config.clone();
    config.user("cantos_app");
    let app = connect(&config).await;
    for sql in [
        "UPDATE adaptation_runs SET frozen_input=frozen_input",
        "UPDATE adaptation_runs SET input_version=input_version",
        "DELETE FROM adaptation_runs",
        "TRUNCATE adaptation_runs CASCADE",
        "UPDATE adaptation_submissions SET receipt=receipt",
        "DELETE FROM adaptation_submissions",
        "UPDATE adaptation_proposals SET proposal=proposal",
        "DELETE FROM adaptation_proposals",
        "UPDATE adaptation_acceptances SET request=request",
        "UPDATE script_revisions SET canonical_export=canonical_export",
        "ALTER TABLE adaptation_submissions DISABLE TRIGGER ALL",
    ] {
        assert_eq!(
            app.batch_execute(sql)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "42501",
            "{sql}"
        );
    }
    assert_eq!(
        app.batch_execute("UPDATE adaptation_runs SET status='succeeded' WHERE status='accepted'")
            .await
            .unwrap_err()
            .code()
            .unwrap()
            .code(),
        "55000"
    );
    for sql in [
        "UPDATE adaptation_runs SET frozen_input=frozen_input || '{\"forged_fact\":true}'::jsonb",
        "UPDATE adaptation_runs SET input_version='a1'",
        "DELETE FROM adaptation_runs",
        "TRUNCATE adaptation_runs CASCADE",
        "UPDATE adaptation_submissions SET receipt=receipt",
        "DELETE FROM adaptation_submissions",
        "TRUNCATE adaptation_submissions",
        "UPDATE adaptation_proposals SET proposal=proposal",
        "DELETE FROM adaptation_proposals",
        "UPDATE adaptation_acceptances SET request=request",
    ] {
        assert_eq!(
            h.admin
                .batch_execute(sql)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "55000",
            "{sql}"
        );
    }
    assert_eq!(
        h.store
            .load(ALICE, &saved.script_id, Some(1))
            .await
            .unwrap(),
        saved
    );
    h.admin
        .batch_execute(
            "ALTER TABLE adaptation_submissions DISABLE TRIGGER adaptation_submission_immutable",
        )
        .await
        .unwrap();
    h.admin.execute("UPDATE adaptation_submissions SET receipt=jsonb_set(receipt,'{generation,host_tool}','\"forged-host\"'::jsonb) WHERE run_id=$1",&[&context.id]).await.unwrap();
    h.admin
        .batch_execute(
            "ALTER TABLE adaptation_submissions ENABLE TRIGGER adaptation_submission_immutable",
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store.load_adaptation_context(ALICE, &context.id).await,
        Err(StoreError::CorruptRevision)
    ));
    assert_eq!(
        h.store
            .load(ALICE, &saved.script_id, Some(1))
            .await
            .unwrap(),
        saved
    );
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn frozen_source_checksum_extractor_prompt_contract_and_base_digest_tampering_are_detected() {
    for (path, replacement) in [
        ("{source,sha256}", json!("0".repeat(64))),
        (
            "{source,outcome,extraction,extractor_version}",
            json!("forged-extractor"),
        ),
        ("{prompt_version}", json!("forged-prompt")),
        ("{contract_version}", json!("forged-contract")),
        ("{context_version}", json!("c2")),
        ("{base,content_digest}", json!("c1:forged")),
    ] {
        let h = Harness::new(None).await;
        let source = h.import(ALICE).await;
        let first = h.proposed(&source, &operation(), 0).await;
        let saved = h
            .store
            .accept_adaptation(ALICE, &first.id, accept(&first))
            .await
            .unwrap();
        let context = h.context(&source, &saved.script_id, 1).await;
        h.admin
            .batch_execute("ALTER TABLE adaptation_runs DISABLE TRIGGER adaptation_run_guard")
            .await
            .unwrap();
        h.admin.execute("UPDATE adaptation_runs SET frozen_input=jsonb_set(frozen_input,$2::text::text[],$3::text::jsonb) WHERE id=$1",&[&context.id,&path,&replacement.to_string()]).await.unwrap();
        h.admin
            .batch_execute("ALTER TABLE adaptation_runs ENABLE TRIGGER adaptation_run_guard")
            .await
            .unwrap();
        assert!(
            matches!(
                h.store.load_adaptation_context(ALICE, &context.id).await,
                Err(StoreError::CorruptRevision)
            ),
            "{path}"
        );
        assert!(
            matches!(
                h.store
                    .submit_adaptation_proposal(
                        ALICE,
                        &context.id,
                        submission(&context, document())
                    )
                    .await,
                Err(StoreError::CorruptRevision)
            ),
            "{path}"
        );
        assert_eq!(
            h.store
                .load(ALICE, &saved.script_id, Some(1))
                .await
                .unwrap(),
            saved
        );
        assert_eq!(h.counts().await, (2, 0, 1, 1, 1));
        assert_eq!(h.submissions().await, 1);
        h.originals_unchanged(&source).await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn authorization_expiry_and_revocation_after_lock_wait_roll_back_context_submit_and_accept() {
    for action in ["context", "submit", "accept"] {
        for expired in [false, true] {
            let h = Harness::new(None).await;
            let source = h.import(ALICE).await;
            let context = if action == "accept" {
                Some(h.proposed(&source, &operation(), 0).await)
            } else if action == "submit" {
                Some(h.context(&source, &operation(), 0).await)
            } else {
                None
            };
            let before = h.counts().await;
            let submissions = h.submissions().await;
            let mut blocker = connect(&h.config).await;
            let tx = blocker.transaction().await.unwrap();
            if let Some(context) = &context {
                tx.query_one(
                    "SELECT id FROM adaptation_runs WHERE id=$1 FOR UPDATE",
                    &[&context.id],
                )
                .await
                .unwrap();
            } else {
                tx.query_one(
                    "SELECT pg_advisory_xact_lock(hashtextextended('adaptation:alice',0))",
                    &[],
                )
                .await
                .unwrap();
            }
            if expired {
                h.admin.execute("UPDATE sessions SET expires_at=clock_timestamp()+interval '200 milliseconds' WHERE actor_id='alice'",&[]).await.unwrap();
            }
            let store = h.store.clone();
            let context = context.clone();
            let context_intent = context_request(&source, &operation(), 0);
            let task = tokio::spawn(async move {
                match action {
                    "context" => store
                        .create_adaptation_context(ALICE, context_intent)
                        .await
                        .map(|_| ()),
                    "submit" => {
                        let context = context.unwrap();
                        store
                            .submit_adaptation_proposal(
                                ALICE,
                                &context.id,
                                submission(&context, document()),
                            )
                            .await
                            .map(|_| ())
                    }
                    "accept" => {
                        let context = context.unwrap();
                        store
                            .accept_adaptation(ALICE, &context.id, accept(&context))
                            .await
                            .map(|_| ())
                    }
                    _ => unreachable!(),
                }
            });
            blocked_connections(&h.admin, 1).await;
            if expired {
                h.admin
                    .query_one("SELECT pg_sleep(0.4)", &[])
                    .await
                    .unwrap();
            } else {
                h.admin
                    .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
                    .await
                    .unwrap();
            }
            tx.commit().await.unwrap();
            assert!(
                matches!(task.await.unwrap(), Err(StoreError::Unauthenticated)),
                "{action}, expiry={expired}"
            );
            assert_eq!(h.counts().await, before);
            assert_eq!(h.submissions().await, submissions);
        }
    }
}

/// This independent typed record preserves the historical a1 serialized field order.
// No current caller metadata is fabricated from these old provider fields.
#[derive(Serialize)]
struct LegacyFrozen {
    request: StartAdaptationRequest,
    source: ImportResponse,
    base: Option<RevisionResponse>,
    provider: AdaptationProviderMetadata,
    generation_record_id: String,
    rights_record_id: String,
}
fn labelled_digest<T: Serialize>(label: &[u8], value: &T) -> String {
    let mut digest = Sha256::new();
    digest.update(label);
    digest.update(serde_json::to_vec(value).unwrap());
    format!("{:x}", digest.finalize())
}
async fn seed_legacy(
    h: &Harness,
    source: &ImportResponse,
    accepted: bool,
) -> (String, StartAdaptationRequest, Option<RevisionResponse>) {
    let id = operation();
    let script = operation();
    let provider = AdaptationProviderMetadata {
        provider: "preserved-historical-record".into(),
        endpoint: "http://127.0.0.1:historical-only".into(),
        model: "archived-model-name".into(),
        local_model_digest: Some("archived-weight-record".into()),
        prompt_version: "cantos-radio-adapt-1".into(),
        contract_version: "cantos-adaptation-1".into(),
        config: AdaptationConfig {
            temperature_milli: 100,
            seed: 7,
            num_context: 8192,
            num_predict: 1024,
            timeout_seconds: 120,
        },
    };
    let request = StartAdaptationRequest {
        operation_id: operation(),
        source_id: source.id.clone(),
        script_id: script.clone(),
        expected_revision: 0,
        expected_provider: provider.clone(),
        rights_authorization: true,
    };
    let frozen = LegacyFrozen {
        request: request.clone(),
        source: source.clone(),
        base: None,
        provider,
        generation_record_id: format!("gen_{}", id.replace('-', "")),
        rights_record_id: format!("rights_{}", id.replace('-', "")),
    };
    for (kind, evidence) in [
        ("generation", &frozen.generation_record_id),
        ("rights", &frozen.rights_record_id),
    ] {
        h.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice',$1,$2,'independently seeded historical record; no current generation or publication claim')",&[&kind,&evidence]).await.unwrap();
    }
    let proposal = admit_output(
        document().as_bytes(),
        source,
        &TrustedBinding {
            source_id: source.id.clone(),
            source_sha256: source.sha256.clone(),
            generation_record_id: frozen.generation_record_id.clone(),
            rights_record_id: frozen.rights_record_id.clone(),
            id_namespace: id.replace('-', ""),
            base_script_json: None,
        },
    )
    .unwrap();
    let input_digest = labelled_digest(b"cantos/adaptation-input/a1\n", &frozen);
    let json = serde_json::to_string(&frozen).unwrap();
    h.admin.execute("INSERT INTO adaptation_runs(id,owner_id,operation_id,source_id,script_id,frozen_input,input_digest,status) VALUES($1,'alice',$2,$3,$4,$5::text::jsonb,$6,$7)",&[&id,&request.operation_id,&source.id,&script,&json,&input_digest,&if accepted{"accepted"}else{"succeeded"}]).await.unwrap();
    h.admin
        .execute("INSERT INTO adaptation_attempts(run_id) VALUES($1)", &[&id])
        .await
        .unwrap();
    let observation=json!({"problem":null,"usage":{"input_tokens":87,"output_tokens":123},"cost":null,"output_sha256":sha(document().as_bytes()),"output_byte_len":document().len()}).to_string();
    h.admin.execute("INSERT INTO adaptation_observations(run_id,kind,observation) VALUES($1,'provider_result',$2::text::jsonb)",&[&id,&observation]).await.unwrap();
    h.admin.execute("INSERT INTO adaptation_proposals(run_id,proposal,digest) VALUES($1,$2::text::jsonb,$3)",&[&id,&serde_json::to_string(&proposal).unwrap(),&labelled_digest(b"cantos/adaptation-proposal/p1\n",&proposal)]).await.unwrap();
    let saved = if accepted {
        let acceptance = AcceptAdaptationRequest {
            operation_id: operation(),
            expected_revision: 0,
            script_json: proposal.script_json,
            reviewed_findings: true,
        };
        let saved = h
            .store
            .save(
                ALICE,
                &script,
                SaveRevisionRequest {
                    operation_id: acceptance.operation_id.clone(),
                    expected_revision: 0,
                    script_json: acceptance.script_json.clone(),
                },
            )
            .await
            .unwrap();
        h.admin.execute("INSERT INTO adaptation_acceptances(run_id,owner_id,operation_id,request,script_id,revision) VALUES($1,'alice',$2,$3::text::jsonb,$4,1)",&[&id,&acceptance.operation_id,&serde_json::to_string(&acceptance).unwrap(),&script]).await.unwrap();
        Some(saved)
    } else {
        None
    };
    (id, request, saved)
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn historical_a1_valid_and_accepted_records_keep_frozen_metadata_receipts_and_exports_readable(
) {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    for accepted in [false, true] {
        let (id, request, saved) = seed_legacy(&h, &source, accepted).await;
        let value = h.store.load_adaptation(ALICE, &id).await.unwrap();
        assert_eq!(value.request, request);
        assert_eq!(value.provider, request.expected_provider);
        assert_eq!(value.source_sha256, source.sha256);
        assert_eq!(value.extractor_version, "cantos-import-1");
        assert_eq!(
            value.status,
            if accepted {
                AdaptationStatus::Accepted
            } else {
                AdaptationStatus::Succeeded
            }
        );
        assert_eq!(value.accepted_revision, saved);
        assert_eq!(
            value.usage,
            Some(AdaptationUsage {
                input_tokens: Some(87),
                output_tokens: Some(123)
            })
        );
        assert_eq!(value.cost, None);
        assert_eq!(
            h.reopened().load_adaptation(ALICE, &id).await.unwrap(),
            value
        );
        let (status, review) = h
            .http(
                "GET",
                &format!("/adaptations/{id}/review"),
                Some(ALICE),
                None,
            )
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(review["workflow"], "legacy");
        assert_eq!(
            serde_json::from_value::<AdaptationRunResponse>(review["run"].clone()).unwrap(),
            value
        );
        assert_eq!(
            serde_json::from_value::<ImportResponse>(review["source"].clone()).unwrap(),
            source
        );
        if accepted {
            let acceptance: AcceptAdaptationRequest = serde_json::from_str(
                &h.admin
                    .query_one(
                        "SELECT request::text FROM adaptation_acceptances WHERE run_id=$1",
                        &[&id],
                    )
                    .await
                    .unwrap()
                    .get::<_, String>(0),
            )
            .unwrap();
            assert_eq!(
                h.reopened()
                    .accept_adaptation(ALICE, &id, acceptance)
                    .await
                    .unwrap(),
                saved.unwrap()
            );
        }
        let (status, _) = h
            .http("POST", "/adaptations", Some(ALICE), Some(json!(request)))
            .await;
        assert_eq!(
            status,
            StatusCode::SERVICE_UNAVAILABLE,
            "retired inference cannot create or dispatch an old request"
        );
    }
    assert_eq!(h.counts().await, (2, 2, 2, 1, 1));
    assert_eq!(h.submissions().await, 0);
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn bounded_caller_context_reads_keep_application_role_authorized_index_baseline() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let seeded_at = Instant::now();
    let mut last = None;
    for _ in 0..1000 {
        last = Some(h.proposed(&source, &operation(), 0).await);
    }
    let seed_ms = seeded_at.elapsed().as_secs_f64() * 1000.0;
    let context = last.unwrap();
    assert_eq!(h.counts().await, (1000, 0, 1000, 0, 0));
    assert_eq!(h.submissions().await, 1000);
    h.admin.batch_execute("ANALYZE adaptation_runs; ANALYZE adaptation_submissions; ANALYZE adaptation_proposals; ANALYZE source_records; ANALYZE sessions; ANALYZE actors").await.unwrap();
    let mut config = h.config.clone();
    config.user("cantos_app");
    let app = connect(&config).await;
    let point_plan:Vec<String>=app.query("EXPLAIN (ANALYZE, BUFFERS) SELECT id,frozen_input,status FROM adaptation_runs WHERE id=$1 AND owner_id=$2 FOR UPDATE",&[&context.id,&"alice"]).await.unwrap().into_iter().map(|row|row.get(0)).collect();
    let replay_plan:Vec<String>=app.query("EXPLAIN (ANALYZE, BUFFERS) SELECT id FROM adaptation_runs WHERE owner_id=$1 AND operation_id=$2",&[&"alice",&context.request.operation_id]).await.unwrap().into_iter().map(|row|row.get(0)).collect();
    assert!(point_plan.iter().any(|line| {
        line.contains("adaptation_runs_pkey") || line.contains("adaptation_run_owner_identity")
    }));
    assert!(replay_plan
        .iter()
        .any(|line| line.contains("adaptation_runs_owner_id_operation_id_key")));
    let path = format!("/adaptations/{}/context", context.id);
    for _ in 0..5 {
        assert_eq!(
            h.http("GET", &path, Some(ALICE), None).await.0,
            StatusCode::OK
        );
    }
    let mut timings_ms = vec![];
    for _ in 0..30 {
        let began = Instant::now();
        let (status, value) = h.http("GET", &path, Some(ALICE), None).await;
        timings_ms.push(began.elapsed().as_secs_f64() * 1000.0);
        assert_eq!(status, StatusCode::OK);
        assert_eq!(
            serde_json::from_value::<AdaptationContextResponse>(value).unwrap(),
            context
        );
    }
    timings_ms.sort_by(f64::total_cmp);
    let p50_ms = timings_ms[14];
    let p95_ms = timings_ms[28];
    let p99_ms = timings_ms[29];
    let pg_version: String = app.query_one("SELECT version()", &[]).await.unwrap().get(0);
    fs::create_dir_all("../../target/adaptation-evidence").unwrap();
    fs::write("../../target/adaptation-evidence/point-lookups.json",serde_json::to_string_pretty(&json!({
        "postgresql":pg_version,"database":h.database,"fixture_kind":"original synthetic Vietnamese source and externally submitted synthetic proposal; no inference",
        "proposal_count":1000,"source_bytes":ORIGINAL.len(),"proposal_bytes":context.proposal.as_ref().unwrap().script_json.len(),"seed_ms":seed_ms,
        "concurrency":1,"warm_up_reads":5,"samples":30,"target_warm_p95_ms":250,
        "p50_ms":p50_ms,"p95_ms":p95_ms,"p99_ms":p99_ms,"timings_ms":timings_ms,
        "point_plan":point_plan,"replay_plan":replay_plan,
        "dbsp":"defer: actor-authorized indexed point lookups; no repeated aggregate consumer",
        "limits":"warm in-process HTTP Router with real application-role PostgreSQL; no socket, cold cache, high concurrency or production workload"
    })).unwrap()).unwrap();
    assert!(p95_ms <= 250.0, "declared warm p95 exceeded250ms: {p95_ms}");
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn connection_death_after_receipt_insert_before_commit_rolls_back_and_exact_delivery_recovers(
) {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.context(&source, &operation(), 0).await;
    h.admin.batch_execute("CREATE FUNCTION test_submission_barrier() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock(614937281); RETURN NEW; END $$; CREATE TRIGGER test_submission_barrier BEFORE INSERT ON adaptation_proposals FOR EACH ROW EXECUTE FUNCTION test_submission_barrier();").await.unwrap();
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one("SELECT pg_advisory_xact_lock(614937281)", &[])
        .await
        .unwrap();
    let intent = submission(&context, document());
    let store = h.store.clone();
    let id = context.id.clone();
    let delivered = intent.clone();
    let task = tokio::spawn(async move {
        store
            .submit_adaptation_proposal(ALICE, &id, delivered)
            .await
    });
    blocked_connections(&h.admin, 1).await;
    let pid:i32=h.admin.query_one("SELECT pid FROM pg_stat_activity WHERE application_name='cantos-adaptation-test' AND wait_event_type='Lock' AND query LIKE 'INSERT INTO adaptation_proposals%'",&[]).await.unwrap().get(0);
    assert_eq!(h.submissions().await, 0, "uncommitted receipt is invisible");
    assert_eq!(h.counts().await, (1, 0, 0, 0, 0));
    let terminated: bool = h
        .admin
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .await
        .unwrap()
        .get(0);
    assert!(terminated);
    assert!(matches!(
        bounded(task).await.unwrap(),
        Err(StoreError::Unavailable)
    ));
    tx.commit().await.unwrap();
    h.admin.batch_execute("DROP TRIGGER test_submission_barrier ON adaptation_proposals; DROP FUNCTION test_submission_barrier();").await.unwrap();
    assert_eq!(h.submissions().await, 0);
    assert_eq!(h.counts().await, (1, 0, 0, 0, 0));
    assert_eq!(
        h.reopened()
            .load_adaptation_context(ALICE, &context.id)
            .await
            .unwrap(),
        context
    );
    let receipt = h
        .reopened()
        .submit_adaptation_proposal(ALICE, &context.id, intent.clone())
        .await
        .unwrap();
    assert_eq!(receipt.status, AdaptationSubmissionStatus::Valid);
    assert_eq!(
        h.reopened()
            .submit_adaptation_proposal(ALICE, &context.id, intent)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(h.submissions().await, 1);
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn draft_validation_checks_real_ir_with_auth_origin_and_bounds_without_writes_or_approval() {
    let h = Harness::new(None).await;
    let source = h.import(ALICE).await;
    let context = h.proposed(&source, &operation(), 0).await;
    let script_json = context.proposal.as_ref().unwrap().script_json.clone();
    let request = ValidateScriptRequest { script_json };
    let before = h.counts().await;
    let submissions = h.submissions().await;
    let evidence_before: i64 = h
        .admin
        .query_one("SELECT count(*) FROM script_evidence", &[])
        .await
        .unwrap()
        .get(0);
    for token in [None, Some("invalid-session")] {
        let (status, error) = h
            .http("POST", "/validation", token, Some(json!(request)))
            .await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(error["code"], "unauthenticated");
    }
    let (status, error) = h
        .raw_http(
            "POST",
            "/validation",
            Some(ALICE),
            serde_json::to_vec(&request).unwrap(),
            "http://foreign.invalid",
        )
        .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(error["code"], "forbidden");
    for (bytes, expected_status) in [
        (b"{bad".to_vec(), StatusCode::BAD_REQUEST),
        (vec![b' '; 8 * 1024 * 1024 + 1], StatusCode::BAD_REQUEST),
    ] {
        let (status, error) = h
            .raw_http("POST", "/validation", Some(ALICE), bytes, ORIGIN)
            .await;
        assert_eq!(status, expected_status);
        assert_eq!(error["code"], "invalid_request");
    }
    for forged in [
        json!({"script_json":request.script_json,"approved":true}),
        json!({"script_json":{}}),
        json!({}),
    ] {
        let (status, error) = h
            .http("POST", "/validation", Some(ALICE), Some(forged))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(error["code"], "invalid_request");
    }
    let mut unsupported: Value = serde_json::from_str(&request.script_json).unwrap();
    unsupported["schema_version"] = json!("99.0.0");
    let mut unresolved: Value = serde_json::from_str(&request.script_json).unwrap();
    unresolved["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["speaker_id"] =
        json!("speaker_missing");
    let act = &unresolved["episode"]["acts"][0];
    let scene = &act["scenes"][0];
    let line = &scene["dialogues"][0];
    let speaker_path = format!(
        "episode/act:{}/scene:{}/dialogue:{}/speaker_id",
        act["id"].as_str().unwrap(),
        scene["id"].as_str().unwrap(),
        line["id"].as_str().unwrap()
    );
    let mut unknown: Value = serde_json::from_str(&request.script_json).unwrap();
    unknown["creator_extension"] = json!({"must_not_be_discarded":true});
    for (script_json, issue) in [
        (
            unsupported.to_string(),
            json!({"path":"$","rule":"UnsupportedSchemaVersion"}),
        ),
        (
            unresolved.to_string(),
            json!({"path":speaker_path,"rule":"unknown_speaker"}),
        ),
        (
            unknown.to_string(),
            json!({"path":"$","rule":"InvalidDocument"}),
        ),
        (
            " ".repeat(2 * 1024 * 1024 + 1),
            json!({"path":"$","rule":"DocumentTooLarge"}),
        ),
    ] {
        let (status, error) = h
            .http(
                "POST",
                "/validation",
                Some(ALICE),
                Some(json!({"script_json":script_json})),
            )
            .await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(error["code"], "invalid_script");
        assert_eq!(error["issues"], json!([issue]));
    }
    // Bob has no source/script access. Validation previews his supplied document only and
    // returns no evidence permission, review approval, revision or target existence facts.
    for token in [ALICE, BOB] {
        let (status, value) = h
            .http("POST", "/validation", Some(token), Some(json!(request)))
            .await;
        assert_eq!(status, StatusCode::OK);
        let response: ScriptValidationResponse = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(response.issues, vec![]);
        assert_eq!(value, json!({"issues":[]}));
    }
    let (status, error) = h
        .http(
            "POST",
            &format!("/scripts/{}/revisions", operation()),
            Some(BOB),
            Some(json!(SaveRevisionRequest {
                expected_revision: 0,
                operation_id: operation(),
                script_json: request.script_json,
            })),
        )
        .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(error["code"], "evidence_unavailable");
    assert_eq!(h.counts().await, before);
    assert_eq!(h.submissions().await, submissions);
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM script_evidence", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        evidence_before
    );
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM script_reviews", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    h.originals_unchanged(&source).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn studio_editor_import_proposal_edit_accept_review_save_and_reopen_preserve_history() {
    let h = Harness::new(Some("cantos_test_studio_editor")).await;
    h.admin
        .batch_execute("GRANT INSERT ON script_reviews,script_review_operations TO cantos_app")
        .await
        .unwrap();
    let import_request = ImportRequest {
        metadata: ImportMetadata {
            operation_id: operation(),
            file_name: "Vọng Đài — editor journey.txt".into(),
            format: ImportFormat::Txt,
            reference: "Original repository-authored Studio editor journey fixture".into(),
            rights_holder: Some("Synthetic fixture author".into()),
            permission_evidence: Some(
                "Local integration tests only; publication rights unknown".into(),
            ),
            usage_scope: Some("Private test adaptation/editor review".into()),
        },
        original_bytes: ORIGINAL.as_bytes().to_vec(),
    };
    let (status, value) = h
        .http("POST", "/imports", Some(ALICE), Some(json!(import_request)))
        .await;
    assert_eq!(status, StatusCode::OK);
    let source: ImportResponse = serde_json::from_value(value).unwrap();
    assert_eq!(source.sha256, sha(ORIGINAL.as_bytes()));
    let script = operation();
    let context_intent = context_request(&source, &script, 0);
    let (status, value) = h
        .http(
            "POST",
            "/adaptations/contexts",
            Some(ALICE),
            Some(json!(context_intent)),
        )
        .await;
    assert_eq!(status, StatusCode::CREATED);
    let context: AdaptationContextResponse = serde_json::from_value(value).unwrap();
    let proposal_intent = submission(&context, document());
    let (status, _) = h
        .http(
            "POST",
            &format!("/adaptations/{}/proposals", context.id),
            Some(ALICE),
            Some(json!(proposal_intent)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let (status, comparison) = h
        .http(
            "GET",
            &format!("/adaptations/{}/review", context.id),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(comparison["workflow"], "caller");
    assert_eq!(comparison["context"]["source"], json!(source));
    let proposed: AdaptationContextResponse =
        serde_json::from_value(comparison["context"].clone()).unwrap();
    let original_proposal: Value =
        serde_json::from_str(&proposed.proposal.as_ref().unwrap().script_json).unwrap();
    let mut draft = original_proposal.clone();
    let missing_speaker =
        draft["episode"]["acts"][0]["scenes"][0]["dialogues"][2]["speaker_id"].clone();
    for character in draft["characters"].as_array_mut().unwrap() {
        if character["id"] == missing_speaker {
            character["name"] = json!("Minh — người gác cửa");
            character["personality"] = json!("Tác giả đã xác nhận danh tính; vẫn dè dặt.");
        }
    }
    draft["episode"]["acts"][0]["scenes"][0]["title"] = json!("Bên cánh cửa — đã đối chiếu");
    draft["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"] =
        json!("Chúng mình sẽ chờ đến bình minh.");
    draft["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["delivery"] =
        json!({"emotion":"hopeful","intensity_permille":525});
    let validation = ValidateScriptRequest {
        script_json: draft.to_string(),
    };
    let (status, validation_receipt) = h
        .http("POST", "/validation", Some(ALICE), Some(json!(validation)))
        .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(validation_receipt, json!({"issues":[]}));
    assert_eq!(h.counts().await, (1, 0, 1, 0, 0));
    let acceptance = AcceptAdaptationRequest {
        operation_id: operation(),
        expected_revision: 0,
        script_json: validation.script_json,
        reviewed_findings: true,
    };
    let accept_path = format!("/adaptations/{}/accept", context.id);
    let (status, value) = h
        .http("POST", &accept_path, Some(ALICE), Some(json!(acceptance)))
        .await;
    assert_eq!(status, StatusCode::OK);
    let first: RevisionResponse = serde_json::from_value(value).unwrap();
    assert_eq!(first.revision, 1);
    let first_value: Value = serde_json::from_str(&first.script_json).unwrap();
    assert_eq!(first_value, draft);
    assert_eq!(first_value["provenance"], original_proposal["provenance"]);
    assert_eq!(first_value["work"], original_proposal["work"]);
    assert_eq!(first_value["adaptation"], original_proposal["adaptation"]);
    assert_eq!(
        first_value["episode"]["acts"][0]["scenes"][0]["sound_cues"],
        original_proposal["episode"]["acts"][0]["scenes"][0]["sound_cues"]
    );
    for (edited, original) in first_value["characters"]
        .as_array()
        .unwrap()
        .iter()
        .zip(original_proposal["characters"].as_array().unwrap())
    {
        assert_eq!(edited["id"], original["id"]);
        assert_eq!(edited["role"], original["role"]);
    }
    let review_intent = ReviewRequest {
        operation_id: operation(),
        revision: 1,
    };
    let review_path = format!("/scripts/{script}/reviews");
    let (status, value) = h
        .http(
            "POST",
            &review_path,
            Some(ALICE),
            Some(json!(review_intent)),
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let review: ReviewResponse = serde_json::from_value(value).unwrap();
    assert_eq!(review.content_digest, first.content_digest);
    assert_eq!(review.export_digest, first.export_digest);
    let source_path = format!("/scripts/{script}/sources/{}", source.id);
    let (status, value) = h.http("GET", &source_path, Some(ALICE), None).await;
    assert_eq!(status, StatusCode::OK);
    let linked_source: SourceResponse = serde_json::from_value(value).unwrap();
    assert_eq!(linked_source.original_text.as_bytes(), ORIGINAL.as_bytes());
    assert_eq!(linked_source.sha256, source.sha256);

    let save_intent = edit(&first, 1);
    let save_path = format!("/scripts/{script}/revisions");
    h.admin
        .execute(
            "INSERT INTO script_members(script_id,actor_id,role) VALUES($1,'bob','reader')",
            &[&script],
        )
        .await
        .unwrap();
    for (path, request) in [
        (&save_path, json!(save_intent)),
        (&review_path, json!(review_intent)),
    ] {
        let (status, error) = h.http("POST", path, Some(BOB), Some(request)).await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(error["code"], "forbidden");
    }
    assert_eq!(h.counts().await, (1, 0, 1, 1, 1));
    let (status, value) = h
        .http("POST", &save_path, Some(ALICE), Some(json!(save_intent)))
        .await;
    assert_eq!(status, StatusCode::OK);
    let second: RevisionResponse = serde_json::from_value(value).unwrap();
    assert_eq!(second.revision, 2);
    let second_value: Value = serde_json::from_str(&second.script_json).unwrap();
    assert_eq!(second_value["provenance"], first_value["provenance"]);
    assert_eq!(second_value["characters"], first_value["characters"]);
    assert_eq!(
        second_value["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["id"],
        first_value["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["id"]
    );
    assert_eq!(
        second_value["episode"]["acts"][0]["scenes"][0]["dialogues"][1]["text"],
        "Chúng mình sẽ chờ đến sáng."
    );
    let stale_draft = edit(&first, 1);
    let (status, error) = h
        .http("POST", &save_path, Some(ALICE), Some(json!(stale_draft)))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(
        error,
        json!({"code":"stale_revision","current_revision":2,"issues":[]})
    );
    assert_eq!(h.counts().await, (1, 0, 1, 1, 2));
    // Exact repeated actions produce their original receipt even after the head moved.
    assert_eq!(
        h.http("POST", &accept_path, Some(ALICE), Some(json!(acceptance)))
            .await,
        (StatusCode::OK, json!(first))
    );
    assert_eq!(
        h.http("POST", &save_path, Some(ALICE), Some(json!(save_intent)))
            .await,
        (StatusCode::OK, json!(second))
    );
    assert_eq!(
        h.http(
            "POST",
            &review_path,
            Some(ALICE),
            Some(json!(review_intent))
        )
        .await,
        (StatusCode::OK, json!(review))
    );
    let history_path = format!("/scripts/{script}/history?after_revision=0&limit=1");
    let (status, value) = h.http("GET", &history_path, Some(BOB), None).await;
    assert_eq!(status, StatusCode::OK);
    let history: HistoryResponse = serde_json::from_value(value).unwrap();
    assert_eq!(history.revisions.len(), 1);
    assert_eq!(history.revisions[0].revision, 1);
    assert_eq!(history.reviews, vec![review.clone()]);
    assert_eq!(history.next_after, Some(1));
    let (status, value) = h
        .http(
            "GET",
            &format!("/scripts/{script}/history?after_revision=1&limit=1"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(status, StatusCode::OK);
    let last: HistoryResponse = serde_json::from_value(value).unwrap();
    assert_eq!(last.revisions.len(), 1);
    assert_eq!(last.revisions[0].revision, 2);
    assert_eq!(last.reviews, vec![]);
    assert_eq!(last.next_after, None);
    let reopened = h.reopened();
    assert_eq!(reopened.load(ALICE, &script, None).await.unwrap(), second);
    assert_eq!(reopened.load(ALICE, &script, Some(1)).await.unwrap(), first);
    assert_eq!(
        reopened.source(BOB, &script, &source.id).await.unwrap(),
        linked_source
    );
    let accepted = reopened
        .load_adaptation_context(ALICE, &context.id)
        .await
        .unwrap();
    assert_eq!(accepted.proposal, proposed.proposal);
    assert_eq!(accepted.accepted_revision, Some(first.clone()));
    assert_eq!(h.counts().await, (1, 0, 1, 1, 2));
    h.originals_unchanged(&source).await;
    let reviews: i64 = h
        .admin
        .query_one("SELECT count(*) FROM script_reviews", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(reviews, 1);
    fs::create_dir_all("../../target/studio-editor-evidence").unwrap();
    fs::write("../../target/studio-editor-evidence/editor-api-journey.json", serde_json::to_string_pretty(&json!({
        "database":h.database,"token":ALICE,"script":script,"original":ORIGINAL,"source":source,
        "import_request":import_request,"context_request":context_intent,"context":accepted,
        "proposal_request":proposal_intent,"acceptance":acceptance,"first":first,
        "save_request":save_intent,"second":second,"review_request":review_intent,"review":review,
        "linked_source":linked_source,"history_first":history,"history_last":last,
        "counts":{"runs":1,"attempts":0,"submissions":1,"proposals":1,"acceptances":1,"revisions":2,"reviews":1,"review_operations":1,"sources":1,"evidence":3,"scripts":1},
        "provenance":"repository-authored synthetic Vietnamese source/proposal; no inference"
    })).unwrap()).unwrap();
}
