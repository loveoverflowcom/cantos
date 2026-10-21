//! Authenticated private audio intake against a newly created disposable PostgreSQL database.
//! Synthetic media is encoded at runtime and is never committed as an audio fixture.
use std::{env, fs, path::PathBuf, process::Command, time::Duration};

use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use cantos_contracts::{
    AudioIntentResponse, AudioReceiptResponse, CreateAudioIntentRequest, SaveRevisionRequest,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::postgres::{local_config, migrate, token_hash, Store, StoreError},
    script_ir::read_script,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_postgres::{Client, Config, NoTls};
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "http://127.0.0.1:8080";
const PROFILE: &str = "opus-mono-20-v1";
const MAX_BYTES: usize = 32 * 1024 * 1024;
const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const RECEIPT_LOCK: i64 = 1_129_453_117;

struct PrivateRoot(PathBuf);

impl PrivateRoot {
    fn new() -> Self {
        let path = env::temp_dir().join(format!("cantos-audio-test-{}", Uuid::new_v4()));
        fs::create_dir(&path).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self(path)
    }
}

impl Drop for PrivateRoot {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Harness {
    admin: Client,
    config: Config,
    store: Store,
    app: Router,
    root: PrivateRoot,
    script: String,
    digest: String,
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

impl Harness {
    async fn new() -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("use cargo xtask verify postgres");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let cluster = connect(&config).await;
        let database = format!("cantos_test_{}", Uuid::new_v4().simple());
        cluster
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .unwrap();
        config.dbname(&database);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin
            .batch_execute(
                "GRANT USAGE ON SCHEMA public TO cantos_app;
                 GRANT SELECT ON actors,sessions,scripts,script_revisions,script_members,
                     script_evidence,revision_evidence,audio_upload_intents,audio_intake_receipts TO cantos_app;
                 GRANT INSERT ON scripts,script_revisions,revision_evidence,script_evidence,
                     audio_upload_intents,audio_intake_receipts TO cantos_app;
                 GRANT UPDATE(head_revision) ON scripts TO cantos_app;
                 GRANT UPDATE(revoked) ON sessions TO cantos_app;
                 GRANT EXECUTE ON FUNCTION production_lock_actor(text) TO cantos_app;
                 INSERT INTO actors(id) VALUES('alice'),('bob');",
            )
            .await
            .unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin
                .execute(
                    "INSERT INTO sessions(token_hash,actor_id,expires_at)
                     VALUES($1,$2,CURRENT_TIMESTAMP+interval '1 hour')",
                    &[&token_hash(token), &actor],
                )
                .await
                .unwrap();
            for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
                admin
                    .execute(
                        "INSERT INTO script_evidence(owner_id,kind,id,description)
                         VALUES($1,$2,$3,'Original synthetic test evidence; no legal clearance')",
                        &[&actor, &kind, &id],
                    )
                    .await
                    .unwrap();
            }
        }
        let root = PrivateRoot::new();
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-audio-intake-test");
        let store = Store::new(app_config)
            .unwrap()
            .with_audio_storage(&root.0)
            .unwrap();
        let script = Uuid::new_v4().to_string();
        let saved = store
            .save(ALICE, &script, revision_request(0, SCRIPT.into()))
            .await
            .unwrap();
        let app = make_router(store.clone());
        Self {
            admin,
            config,
            store,
            app,
            root,
            script,
            digest: saved.content_digest,
        }
    }

    fn restart(&self) -> Store {
        let mut config = self.config.clone();
        config
            .user("cantos_app")
            .application_name("cantos-audio-intake-test");
        Store::new(config)
            .unwrap()
            .with_audio_storage(&self.root.0)
            .unwrap()
    }

    fn intent_request(&self) -> CreateAudioIntentRequest {
        CreateAudioIntentRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_revision: 1,
            expected_content_digest: self.digest.clone(),
            profile_id: PROFILE.into(),
        }
    }

    async fn intent(&self) -> AudioIntentResponse {
        self.store
            .create_audio_intent(ALICE, &self.script, self.intent_request())
            .await
            .unwrap()
    }

    async fn counts(&self) -> (i64, i64) {
        let row = self
            .admin
            .query_one(
                "SELECT (SELECT count(*) FROM audio_upload_intents),
                        (SELECT count(*) FROM audio_intake_receipts)",
                &[],
            )
            .await
            .unwrap();
        (row.get(0), row.get(1))
    }

    async fn request(
        &self,
        method: &str,
        path: &str,
        cookie: Option<String>,
        origin: Option<&str>,
        content_type: Option<&str>,
        bytes: Vec<u8>,
    ) -> (StatusCode, Value) {
        let mut request = Request::builder().method(method).uri(path);
        if let Some(cookie) = cookie {
            request = request.header("cookie", cookie);
        }
        if let Some(origin) = origin {
            request = request.header("origin", origin);
        }
        if let Some(content_type) = content_type {
            request = request.header("content-type", content_type);
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
        let body = serde_json::from_slice(&bytes)
            .unwrap_or_else(|_| Value::String(String::from_utf8_lossy(&bytes).into_owned()));
        (status, body)
    }

    async fn json(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, Value) {
        self.request(
            method,
            path,
            token.map(session_cookie),
            Some(ORIGIN),
            Some("application/json"),
            body.map_or_else(Vec::new, |body| serde_json::to_vec(&body).unwrap()),
        )
        .await
    }

    async fn upload(
        &self,
        intent: &AudioIntentResponse,
        token: Option<&str>,
        bytes: Vec<u8>,
    ) -> (StatusCode, Value) {
        self.request(
            "PUT",
            &intent.upload_path,
            token.map(session_cookie),
            Some(ORIGIN),
            Some("application/octet-stream"),
            bytes,
        )
        .await
    }

    async fn hold_receipt_insert(&self) {
        self.admin
            .batch_execute(&format!(
                "CREATE FUNCTION hold_audio_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
                 BEGIN PERFORM pg_advisory_xact_lock({RECEIPT_LOCK}); RETURN NEW; END $$;
                 CREATE TRIGGER hold_audio_receipt BEFORE INSERT ON audio_intake_receipts
                 FOR EACH ROW EXECUTE FUNCTION hold_audio_receipt();"
            ))
            .await
            .unwrap();
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

fn session_cookie(token: &str) -> String {
    format!("cantos_session={token}")
}

fn revision_request(expected_revision: u64, script_json: String) -> SaveRevisionRequest {
    SaveRevisionRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_revision,
        script_json,
    }
}

fn synthetic_media(frequency: u32, channels: u32, opus: bool) -> Vec<u8> {
    let source = format!("sine=frequency={frequency}:sample_rate=48000:duration=1");
    let channels = channels.to_string();
    let mut command = Command::new("ffmpeg");
    command.args([
        "-hide_banner",
        "-loglevel",
        "error",
        "-nostdin",
        "-f",
        "lavfi",
        "-i",
        &source,
        "-map",
        "0:a:0",
        "-ac",
        &channels,
        "-ar",
        "48000",
    ]);
    if opus {
        command.args([
            "-c:a", "libopus", "-b:a", "20k", "-vbr", "off", "-f", "ogg", "pipe:1",
        ]);
    } else {
        command.args(["-c:a", "pcm_s16le", "-f", "wav", "pipe:1"]);
    }
    let output = command
        .output()
        .expect("the PostgreSQL audio-intake gate requires ffmpeg with libopus");
    assert!(
        output.status.success(),
        "synthetic fixture encoding failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(!output.stdout.is_empty());
    output.stdout
}

fn opus() -> Vec<u8> {
    synthetic_media(440, 1, true)
}

async fn wait_for_receipt_insert(admin: &Client) -> i32 {
    wait_for_query_lock(admin, "INSERT INTO audio_intake_receipts%").await
}

async fn wait_for_query_lock(admin: &Client, query: &str) -> i32 {
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Some(row) = admin
                .query_opt(
                    "SELECT pid FROM pg_stat_activity
                     WHERE application_name='cantos-audio-intake-test'
                       AND datname=current_database()
                       AND wait_event_type='Lock'
                       AND query LIKE $1
                     LIMIT 1",
                    &[&query],
                )
                .await
                .unwrap()
            {
                break row.get(0);
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("the upload never reached its expected database lock")
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn intent_is_explicit_revision_bound_and_exact_operation_replays_original_identity() {
    let h = Harness::new().await;
    let request = h.intent_request();
    let intent = h
        .store
        .create_audio_intent(ALICE, &h.script, request.clone())
        .await
        .unwrap();
    assert_eq!(intent.script_id, h.script);
    assert_eq!(intent.script_revision, 1);
    assert_eq!(intent.content_digest, h.digest);
    assert_eq!(intent.profile_id, PROFILE);
    assert_eq!(intent.created_by, "alice");
    assert_eq!(intent.operation_id, request.operation_id);
    assert_eq!(
        intent.status_path,
        format!(
            "/api/v1/studio/scripts/{}/audio-intents/{}",
            h.script, intent.id
        )
    );
    assert_eq!(intent.upload_path, format!("{}/bytes", intent.status_path));
    assert_eq!(
        h.store
            .create_audio_intent(ALICE, &h.script, request.clone())
            .await
            .unwrap(),
        intent
    );
    for change in ["profile", "revision", "digest"] {
        let mut changed = request.clone();
        match change {
            "profile" => changed.profile_id = "opus-mono-24-v1".into(),
            "revision" => changed.expected_revision = 2,
            "digest" => {
                changed.expected_content_digest = format!("sir-c1:sha256:{}", "0".repeat(64))
            }
            _ => unreachable!(),
        }
        assert!(matches!(
            h.store.create_audio_intent(ALICE, &h.script, changed).await,
            Err(StoreError::OperationReused)
        ));
    }
    let other = Uuid::new_v4().to_string();
    h.store
        .save(ALICE, &other, revision_request(0, SCRIPT.into()))
        .await
        .unwrap();
    assert!(matches!(
        h.store.create_audio_intent(ALICE, &other, request).await,
        Err(StoreError::OperationReused)
    ));
    let pending = h
        .store
        .load_audio_intent(ALICE, &h.script, &intent.id)
        .await
        .unwrap();
    assert_eq!(pending.intent, intent);
    assert_eq!(pending.receipt, None);
    assert_eq!(h.counts().await, (1, 0));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn invalid_or_stale_target_never_creates_an_intent_and_there_is_no_default_profile() {
    let h = Harness::new().await;
    let path = format!("/api/v1/studio/scripts/{}/audio-intents", h.script);
    for profile in ["", "default", "opus-mono-20-v2", "aac-lc-mono-40-v1"] {
        let mut request = h.intent_request();
        request.profile_id = profile.into();
        let (status, body) = h
            .json("POST", &path, Some(ALICE), Some(json!(request)))
            .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{profile}");
        assert_eq!(body["code"], "invalid_request");
    }
    let mut missing = json!(h.intent_request());
    missing.as_object_mut().unwrap().remove("profile_id");
    assert_eq!(
        h.json("POST", &path, Some(ALICE), Some(missing)).await.0,
        StatusCode::BAD_REQUEST
    );
    let mut request = h.intent_request();
    request.expected_revision = 2;
    let (status, body) = h
        .json("POST", &path, Some(ALICE), Some(json!(request)))
        .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "stale_revision");
    assert_eq!(body["current_revision"], 1);
    let mut request = h.intent_request();
    request.expected_content_digest = format!("sir-c1:sha256:{}", "0".repeat(64));
    let (status, body) = h
        .json("POST", &path, Some(ALICE), Some(json!(request)))
        .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["code"], "invalid_request");
    assert!(body["issues"].as_array().unwrap().iter().any(|issue| {
        issue["path"] == "expected_content_digest" && issue["rule"] == "content_digest_mismatch"
    }));
    assert_eq!(h.counts().await, (0, 0));
    for profile in [
        "opus-mono-20-v1",
        "opus-mono-24-v1",
        "opus-mono-32-v1",
        "opus-mono-40-v1",
    ] {
        let mut request = h.intent_request();
        request.profile_id = profile.into();
        let (status, body) = h
            .json("POST", &path, Some(ALICE), Some(json!(request)))
            .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body["profile_id"], profile);
    }
    assert_eq!(h.counts().await, (4, 0));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn only_current_owner_can_create_read_or_upload_and_intent_scope_cannot_cross_scripts() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let path = format!("/api/v1/studio/scripts/{}/audio-intents", h.script);
    for (token, expected) in [
        (None, StatusCode::UNAUTHORIZED),
        (Some(BOB), StatusCode::NOT_FOUND),
    ] {
        assert_eq!(
            h.json("POST", &path, token, Some(json!(h.intent_request())))
                .await
                .0,
            expected
        );
        assert_eq!(
            h.json("GET", &intent.status_path, token, None).await.0,
            expected
        );
        assert_eq!(h.upload(&intent, token, opus()).await.0, expected);
    }
    for role in ["reader", "editor"] {
        h.admin
            .execute(
                "INSERT INTO script_members(script_id,actor_id,role) VALUES($1,'bob',$2)
                 ON CONFLICT(script_id,actor_id) DO UPDATE SET role=EXCLUDED.role",
                &[&h.script, &role],
            )
            .await
            .unwrap();
        assert_eq!(
            h.json("POST", &path, Some(BOB), Some(json!(h.intent_request())))
                .await
                .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            h.json("GET", &intent.status_path, Some(BOB), None).await.0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            h.upload(&intent, Some(BOB), opus()).await.0,
            StatusCode::FORBIDDEN
        );
    }
    for (token, owner) in [(ALICE, "alice"), (BOB, "bob")] {
        let other = Uuid::new_v4().to_string();
        h.store
            .save(token, &other, revision_request(0, SCRIPT.into()))
            .await
            .unwrap();
        let wrong = format!("/api/v1/studio/scripts/{other}/audio-intents/{}", intent.id);
        assert_eq!(
            h.json("GET", &wrong, Some(token), None).await.0,
            StatusCode::NOT_FOUND,
            "{owner}"
        );
        assert_eq!(
            h.request(
                "PUT",
                &format!("{wrong}/bytes"),
                Some(session_cookie(token)),
                Some(ORIGIN),
                Some("application/octet-stream"),
                opus(),
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(h.counts().await, (1, 0));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn http_keeps_same_origin_cookie_binary_and_current_studio_surface_boundaries() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    for origin in [None, Some("https://untrusted.invalid")] {
        let (status, body) = h
            .request(
                "PUT",
                &intent.upload_path,
                Some(session_cookie(ALICE)),
                origin,
                Some("application/octet-stream"),
                opus(),
            )
            .await;
        assert_eq!(status, StatusCode::FORBIDDEN);
        assert_eq!(body["code"], "forbidden");
    }
    let duplicate = format!("cantos_session={ALICE}; cantos_session={BOB}");
    assert_eq!(
        h.request(
            "PUT",
            &intent.upload_path,
            Some(duplicate),
            Some(ORIGIN),
            Some("application/octet-stream"),
            opus()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    for content_type in [None, Some("application/json"), Some("audio/ogg")] {
        assert_eq!(
            h.request(
                "PUT",
                &intent.upload_path,
                Some(session_cookie(ALICE)),
                Some(ORIGIN),
                content_type,
                opus()
            )
            .await
            .0,
            StatusCode::UNSUPPORTED_MEDIA_TYPE
        );
    }
    for prefix in ["/api/v1", "/api/v1/theatre", "/api/v1/admin"] {
        let path = format!("{prefix}/scripts/{}/audio-intents", h.script);
        assert_eq!(
            h.json("POST", &path, Some(ALICE), Some(json!(h.intent_request())))
                .await
                .0,
            StatusCode::NOT_FOUND
        );
        let path = format!("{path}/{}", intent.id);
        assert_eq!(
            h.json("GET", &path, Some(ALICE), None).await.0,
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            h.request(
                "PUT",
                &format!("{path}/bytes"),
                Some(session_cookie(ALICE)),
                Some(ORIGIN),
                Some("application/octet-stream"),
                opus()
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(h.counts().await, (1, 0));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn accepted_receipt_contains_observed_media_and_one_private_object_with_no_delivery_url() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    let (status, body) = h.upload(&intent, Some(ALICE), bytes.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let receipt: AudioReceiptResponse = serde_json::from_value(body.clone()).unwrap();
    assert_eq!(receipt.intent_id, intent.id);
    assert_eq!(receipt.script_id, h.script);
    assert_eq!(receipt.script_revision, 1);
    assert_eq!(receipt.content_digest, h.digest);
    assert_eq!(receipt.profile_id, PROFILE);
    assert_eq!(receipt.operation_id, intent.operation_id);
    assert_eq!(receipt.accepted_by, "alice");
    assert_eq!(
        receipt.media.sha256,
        format!("{:x}", Sha256::digest(&bytes))
    );
    assert_eq!(receipt.media.byte_length, bytes.len() as u64);
    assert_eq!(receipt.media.container, "ogg");
    assert_eq!(receipt.media.codec, "opus");
    assert_eq!(receipt.media.channels, 1);
    assert_eq!(receipt.media.sample_rate_hz, 48_000);
    assert!((990..=1_030).contains(&receipt.media.duration_ms));
    assert!((1..=24_000).contains(&receipt.media.audio_bitrate_bps));
    assert_eq!(receipt.media.profile_id, PROFILE);
    for key in [
        "object_key",
        "path",
        "playback_url",
        "download_url",
        "published",
    ] {
        assert!(body.get(key).is_none(), "private receipt leaked {key}");
        assert!(
            body["media"].get(key).is_none(),
            "private media leaked {key}"
        );
    }
    let row = h
        .admin
        .query_one(
            "SELECT object_key,receipt_bytes FROM audio_intake_receipts WHERE intent_id=$1",
            &[&intent.id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), receipt.media.sha256);
    assert_eq!(
        row.get::<_, Vec<u8>>(1),
        serde_json::to_vec(&receipt).unwrap()
    );
    let object = h.root.0.join("objects").join(&receipt.media.sha256);
    assert_eq!(fs::read(&object).unwrap(), bytes);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(&object).unwrap().permissions().mode() & 0o777,
            0o400
        );
    }
    let (status, pending) = h.json("GET", &intent.status_path, Some(ALICE), None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(pending["receipt"], body);
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn same_bytes_replay_exact_receipt_and_changed_valid_media_conflicts_without_replacement() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    let first = h.upload(&intent, Some(ALICE), bytes.clone()).await;
    assert_eq!(first.0, StatusCode::OK);
    assert_eq!(h.upload(&intent, Some(ALICE), bytes.clone()).await, first);
    let changed = synthetic_media(880, 1, true);
    assert_ne!(changed, bytes);
    let (status, body) = h.upload(&intent, Some(ALICE), changed).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "operation_reused");
    assert_eq!(
        h.json("GET", &intent.status_path, Some(ALICE), None)
            .await
            .1["receipt"],
        first.1
    );
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn rejected_media_and_body_limit_leave_intent_pending_with_no_accepted_row() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let valid = opus();
    for bytes in [
        Vec::new(),
        b"not audio".to_vec(),
        valid[..valid.len() / 2].to_vec(),
        synthetic_media(440, 1, false),
        synthetic_media(440, 2, true),
        vec![0; MAX_BYTES],
    ] {
        let (status, body) = h.upload(&intent, Some(ALICE), bytes).await;
        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(body["code"], "audio_rejected");
        assert_eq!(h.counts().await, (1, 0));
    }
    assert_eq!(
        h.upload(&intent, Some(ALICE), vec![0; MAX_BYTES + 1])
            .await
            .0,
        StatusCode::PAYLOAD_TOO_LARGE
    );
    assert!(h
        .json("GET", &intent.status_path, Some(ALICE), None)
        .await
        .1["receipt"]
        .is_null());
    assert_eq!(h.counts().await, (1, 0));
    assert_eq!(
        h.upload(&intent, Some(ALICE), valid).await.0,
        StatusCode::OK
    );
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn lost_http_response_reconciles_and_fresh_store_replays_durable_receipt() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    let response = h
        .app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(intent.upload_path.as_str())
                .header("origin", ORIGIN)
                .header("cookie", session_cookie(ALICE))
                .header("content-type", "application/octet-stream")
                .body(Body::from(bytes.clone()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    // Deliberately discard the committed response before consuming its receipt body.
    drop(response);
    let restarted = h.restart();
    let status = restarted
        .load_audio_intent(ALICE, &h.script, &intent.id)
        .await
        .unwrap();
    let durable = status.receipt.unwrap();
    assert_eq!(status.intent, intent);
    assert_eq!(
        restarted
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes)
            .await
            .unwrap(),
        durable
    );
    let row = h
        .admin
        .query_one("SELECT receipt_bytes FROM audio_intake_receipts", &[])
        .await
        .unwrap();
    assert_eq!(
        row.get::<_, Vec<u8>>(0),
        serde_json::to_vec(&durable).unwrap()
    );
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn new_script_revision_blocks_pending_acceptance_and_preserves_historical_accepted_status() {
    let h = Harness::new().await;
    let request = h.intent_request();
    let pending = h
        .store
        .create_audio_intent(ALICE, &h.script, request.clone())
        .await
        .unwrap();
    let accepted = h.intent().await;
    let bytes = opus();
    let original = h.upload(&accepted, Some(ALICE), bytes.clone()).await;
    assert_eq!(original.0, StatusCode::OK);
    let mut changed: Value = serde_json::from_str(SCRIPT).unwrap();
    changed["work"]["title"] = json!("Ánh đèn mới của bản thảo tổng hợp");
    h.store
        .save(ALICE, &h.script, revision_request(1, changed.to_string()))
        .await
        .unwrap();
    let (status, body) = h.upload(&pending, Some(ALICE), bytes.clone()).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["code"], "stale_revision");
    assert_eq!(body["current_revision"], 2);
    assert_eq!(
        h.restart()
            .create_audio_intent(ALICE, &h.script, request)
            .await
            .unwrap(),
        pending
    );
    assert_eq!(
        h.upload(&accepted, Some(ALICE), bytes.clone()).await.0,
        StatusCode::CONFLICT
    );
    assert_eq!(
        h.json("GET", &accepted.status_path, Some(ALICE), None)
            .await
            .1["receipt"],
        original.1
    );
    for method in ["GET", "HEAD"] {
        let (status, headers, delivered) =
            owner_audio_request(&h.app, method, &accepted.upload_path, vec![]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "audio/ogg");
        assert_eq!(headers["content-length"], bytes.len().to_string());
        assert_eq!(
            headers["etag"],
            format!("\"{}\"", original.1["media"]["sha256"].as_str().unwrap())
        );
        if method == "GET" {
            assert_eq!(delivered, bytes);
        } else {
            assert!(delivered.is_empty());
        }
    }
    assert_eq!(h.counts().await, (2, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn revoked_expired_or_inactive_owner_cannot_read_replay_create_or_accept() {
    for refusal in ["revoked", "expired", "inactive"] {
        let h = Harness::new().await;
        let intent = h.intent().await;
        let accepted = h.intent().await;
        let bytes = opus();
        assert_eq!(
            h.upload(&accepted, Some(ALICE), bytes.clone()).await.0,
            StatusCode::OK
        );
        let path = format!("/api/v1/studio/scripts/{}/audio-intents", h.script);
        let sql = match refusal {
            "revoked" => "UPDATE sessions SET revoked=true WHERE actor_id='alice'",
            "expired" => {
                "UPDATE sessions SET expires_at=CURRENT_TIMESTAMP-interval '1 second'
                 WHERE actor_id='alice'"
            }
            "inactive" => "UPDATE actors SET active=false WHERE id='alice'",
            _ => unreachable!(),
        };
        h.admin.execute(sql, &[]).await.unwrap();
        assert_eq!(
            h.json("GET", &intent.status_path, Some(ALICE), None)
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{refusal}"
        );
        assert_eq!(
            h.upload(&intent, Some(ALICE), opus()).await.0,
            StatusCode::UNAUTHORIZED,
            "{refusal}"
        );
        assert_eq!(
            h.json("GET", &accepted.status_path, Some(ALICE), None)
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{refusal}"
        );
        assert_eq!(
            h.upload(&accepted, Some(ALICE), bytes).await.0,
            StatusCode::UNAUTHORIZED,
            "{refusal}"
        );
        assert_eq!(
            h.json("POST", &path, Some(ALICE), Some(json!(h.intent_request())))
                .await
                .0,
            StatusCode::UNAUTHORIZED,
            "{refusal}"
        );
        assert_eq!(h.counts().await, (2, 1));
    }
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn owner_deactivation_after_inspection_is_rechecked_before_receipt_commit() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let mut guard = connect(&h.config).await;
    let tx = guard.transaction().await.unwrap();
    // The initial SHARE authorization can proceed; the post-inspection UPDATE lock must wait.
    tx.query_one("SELECT id FROM scripts WHERE id=$1 FOR SHARE", &[&h.script])
        .await
        .unwrap();
    let store = h.store.clone();
    let script = h.script.clone();
    let id = intent.id.clone();
    let bytes = opus();
    let uploading =
        tokio::spawn(async move { store.upload_audio_intent(ALICE, &script, &id, bytes).await });
    wait_for_query_lock(
        &h.admin,
        "SELECT owner_id,head_revision FROM scripts%FOR UPDATE",
    )
    .await;
    h.admin
        .execute("UPDATE actors SET active=false WHERE id='alice'", &[])
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert!(matches!(
        uploading.await.unwrap(),
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(h.counts().await, (1, 0));
    h.admin
        .execute("UPDATE actors SET active=true WHERE id='alice'", &[])
        .await
        .unwrap();
    assert!(h
        .restart()
        .load_audio_intent(ALICE, &h.script, &intent.id)
        .await
        .unwrap()
        .receipt
        .is_none());
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn concurrent_duplicate_intents_and_bytes_commit_one_original_immutable_receipt() {
    let h = Harness::new().await;
    let request = h.intent_request();
    let (left, right) = tokio::join!(
        h.store
            .create_audio_intent(ALICE, &h.script, request.clone()),
        h.store.create_audio_intent(ALICE, &h.script, request),
    );
    let intent = left.unwrap();
    assert_eq!(right.unwrap(), intent);
    assert_eq!(h.counts().await, (1, 0));
    h.hold_receipt_insert().await;
    let mut guard = connect(&h.config).await;
    let tx = guard.transaction().await.unwrap();
    tx.query_one("SELECT pg_advisory_xact_lock($1)", &[&RECEIPT_LOCK])
        .await
        .unwrap();
    let bytes = opus();
    let store = h.store.clone();
    let script = h.script.clone();
    let id = intent.id.clone();
    let first_bytes = bytes.clone();
    let first = tokio::spawn(async move {
        store
            .upload_audio_intent(ALICE, &script, &id, first_bytes)
            .await
    });
    wait_for_receipt_insert(&h.admin).await;
    let store = h.store.clone();
    let script = h.script.clone();
    let id = intent.id.clone();
    let second =
        tokio::spawn(async move { store.upload_audio_intent(ALICE, &script, &id, bytes).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: i64 = h
                .admin
                .query_one(
                    "SELECT count(*) FROM pg_stat_activity
                 WHERE application_name='cantos-audio-intake-test'
                   AND datname=current_database() AND wait_event_type='Lock'",
                    &[],
                )
                .await
                .unwrap()
                .get(0);
            if waiting >= 2 {
                break;
            }
            assert!(
                !second.is_finished(),
                "duplicate completed before first acceptance committed"
            );
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(!first.is_finished());
    tx.commit().await.unwrap();
    let first = first.await.unwrap().unwrap();
    assert_eq!(second.await.unwrap().unwrap(), first);
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn receipt_insert_rollback_keeps_pending_intent_and_restart_retry_accepts_once() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    h.admin
        .batch_execute(
            "CREATE TRIGGER inject_audio_receipt_failure BEFORE INSERT ON audio_intake_receipts
         FOR EACH ROW EXECUTE FUNCTION reject_settled_change()",
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone())
            .await,
        Err(StoreError::Unavailable)
    ));
    assert_eq!(h.counts().await, (1, 0));
    assert!(h
        .restart()
        .load_audio_intent(ALICE, &h.script, &intent.id)
        .await
        .unwrap()
        .receipt
        .is_none());
    h.admin
        .batch_execute("DROP TRIGGER inject_audio_receipt_failure ON audio_intake_receipts")
        .await
        .unwrap();
    let restarted = h.restart();
    let receipt = restarted
        .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone())
        .await
        .unwrap();
    assert_eq!(
        restarted
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn terminated_database_transaction_cannot_commit_acceptance_and_restart_can_reconcile() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    h.hold_receipt_insert().await;
    let mut guard = connect(&h.config).await;
    let tx = guard.transaction().await.unwrap();
    tx.query_one("SELECT pg_advisory_xact_lock($1)", &[&RECEIPT_LOCK])
        .await
        .unwrap();
    let bytes = opus();
    let store = h.store.clone();
    let script = h.script.clone();
    let id = intent.id.clone();
    let upload_bytes = bytes.clone();
    let uploading = tokio::spawn(async move {
        store
            .upload_audio_intent(ALICE, &script, &id, upload_bytes)
            .await
    });
    let pid = wait_for_receipt_insert(&h.admin).await;
    let terminated: bool = h
        .admin
        .query_one("SELECT pg_terminate_backend($1)", &[&pid])
        .await
        .unwrap()
        .get(0);
    assert!(terminated);
    assert!(matches!(
        uploading.await.unwrap(),
        Err(StoreError::Unavailable)
    ));
    tx.commit().await.unwrap();
    assert_eq!(h.counts().await, (1, 0));
    let restarted = h.restart();
    assert!(restarted
        .load_audio_intent(ALICE, &h.script, &intent.id)
        .await
        .unwrap()
        .receipt
        .is_none());
    let receipt = restarted
        .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone())
        .await
        .unwrap();
    assert_eq!(
        restarted
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn settled_intents_and_receipts_reject_update_delete_truncate_even_for_test_admin() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    h.store
        .upload_audio_intent(ALICE, &h.script, &intent.id, opus())
        .await
        .unwrap();
    let mut app_config = h.config.clone();
    app_config.user("cantos_app");
    let app = connect(&app_config).await;
    for table in ["audio_upload_intents", "audio_intake_receipts"] {
        for privilege in ["UPDATE", "DELETE", "TRUNCATE"] {
            let granted: bool = h
                .admin
                .query_one(
                    "SELECT has_table_privilege('cantos_app',$1,$2)",
                    &[&table, &privilege],
                )
                .await
                .unwrap()
                .get(0);
            assert!(
                !granted,
                "application unexpectedly has {privilege} on {table}"
            );
        }
        for sql in [
            format!("UPDATE {table} SET profile_id='opus-mono-24-v1'"),
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
            assert_eq!(
                app.batch_execute(&sql)
                    .await
                    .unwrap_err()
                    .code()
                    .unwrap()
                    .code(),
                "42501"
            );
        }
    }
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn database_binding_and_receipt_digest_are_independent_integrity_barriers() {
    let h = Harness::new().await;
    let id = Uuid::new_v4().to_string();
    let operation = Uuid::new_v4().to_string();
    let wrong_digest = format!("sir-c1:sha256:{}", "0".repeat(64));
    let error = h
        .admin
        .execute(
            "INSERT INTO audio_upload_intents(id,script_id,script_revision,content_digest,
             profile_id,operation_id,created_by) VALUES($1,$2,1,$3,$4,$5,'alice')",
            &[&id, &h.script, &wrong_digest, &PROFILE, &operation],
        )
        .await
        .unwrap_err();
    assert_eq!(error.code().unwrap().code(), "23514");
    assert_eq!(h.counts().await, (0, 0));
    let intent = h.intent().await;
    h.store
        .upload_audio_intent(ALICE, &h.script, &intent.id, opus())
        .await
        .unwrap();
    // Only the disposable administrator bypasses the trigger to simulate disk/row corruption.
    h.admin
        .batch_execute(
            "ALTER TABLE audio_intake_receipts DISABLE TRIGGER audio_receipts_immutable;
         UPDATE audio_intake_receipts SET receipt_bytes='{}'::bytea;
         ALTER TABLE audio_intake_receipts ENABLE TRIGGER audio_receipts_immutable;",
        )
        .await
        .unwrap();
    assert!(matches!(
        h.restart()
            .load_audio_intent(ALICE, &h.script, &intent.id)
            .await,
        Err(StoreError::CorruptRevision)
    ));
    let (status, body) = h.json("GET", &intent.status_path, Some(ALICE), None).await;
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "corrupt_revision");
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn tampered_private_object_blocks_status_and_replay_instead_of_claiming_verified_acceptance()
{
    let h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    let receipt = h
        .store
        .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone())
        .await
        .unwrap();
    let object = h.root.0.join("objects").join(&receipt.media.sha256);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&object, fs::Permissions::from_mode(0o600)).unwrap();
    }
    let mut tampered = bytes.clone();
    tampered[10] ^= 1;
    fs::write(&object, &tampered).unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&object, fs::Permissions::from_mode(0o400)).unwrap();
    }
    let restarted = h.restart();
    assert!(matches!(
        restarted
            .load_audio_intent(ALICE, &h.script, &intent.id)
            .await,
        Err(StoreError::CorruptRevision)
    ));
    assert!(matches!(
        restarted
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes)
            .await,
        Err(StoreError::CorruptRevision)
    ));
    fs::remove_file(&object).unwrap();
    assert!(matches!(
        restarted
            .load_audio_intent(ALICE, &h.script, &intent.id)
            .await,
        Err(StoreError::CorruptRevision)
    ));
    assert_eq!(h.counts().await, (1, 1));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn overlapping_public_static_roots_never_serve_private_accepted_object_bytes() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let receipt = h
        .store
        .upload_audio_intent(ALICE, &h.script, &intent.id, opus())
        .await
        .unwrap();
    let object = h.root.0.join("objects").join(&receipt.media.sha256);
    for public_root in [
        h.root.0.clone(),
        h.root.0.join("objects"),
        h.root.0.parent().unwrap().to_path_buf(),
    ] {
        let relative = object.strip_prefix(&public_root).unwrap();
        let path = format!("/{}", relative.to_str().unwrap());
        let app = router(
            AppState {
                store: h.store.clone(),
                web_origin: ORIGIN.into(),
            },
            public_root.to_str().unwrap(),
        );
        for method in ["GET", "HEAD"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path.as_str())
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND);
        }
    }
    assert_eq!(h.counts().await, (1, 1));
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn audio_enabled_host_blocks_nested_symlinks_and_regular_static_files() {
    use std::os::unix::fs::symlink;

    let h = Harness::new().await;
    let intent = h.intent().await;
    let receipt = h
        .store
        .upload_audio_intent(ALICE, &h.script, &intent.id, opus())
        .await
        .unwrap();
    let public_root = PrivateRoot::new();
    let private_path = fs::canonicalize(&h.root.0).unwrap();
    let public_path = fs::canonicalize(&public_root.0).unwrap();
    assert!(!private_path.starts_with(&public_path));
    assert!(!public_path.starts_with(&private_path));
    fs::write(public_root.0.join("index.html"), b"synthetic public file").unwrap();
    symlink(h.root.0.join("objects"), public_root.0.join("assets")).unwrap();
    let app = router(
        AppState {
            store: h.store.clone(),
            web_origin: ORIGIN.into(),
        },
        public_root.0.to_str().unwrap(),
    );
    let linked_object = format!("/assets/{}", receipt.media.sha256);
    for path in [linked_object.as_str(), "/index.html"] {
        for method in ["GET", "HEAD"] {
            let response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method(method)
                        .uri(path)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{method} {path}");
        }
    }

    // Editorial-only hosts retain ordinary static delivery when audio storage is absent.
    let mut config = h.config.clone();
    config.user("cantos_app");
    let editorial_only = router(
        AppState {
            store: Store::new(config).unwrap(),
            web_origin: ORIGIN.into(),
        },
        public_root.0.to_str().unwrap(),
    );
    for method in ["GET", "HEAD"] {
        let response = editorial_only
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri("/index.html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        if method == "GET" {
            let bytes = response.into_body().collect().await.unwrap().to_bytes();
            assert_eq!(bytes.as_ref(), b"synthetic public file");
        }
    }
    assert_eq!(h.counts().await, (1, 1));
}

/// Real HTTP transport for binary reads and upload responses, independent of JSON helpers.
async fn owner_audio_request(
    app: &Router,
    method: &str,
    path: &str,
    bytes: Vec<u8>,
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header("cookie", session_cookie(ALICE))
        .header("origin", ORIGIN);
    if method == "PUT" {
        request = request.header("content-type", "application/octet-stream");
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::from(bytes)).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let expected_cache_control = match method {
        "GET" | "HEAD" => "private, no-store, no-transform",
        "PUT" => "no-store",
        _ => panic!("unexpected owner audio test method: {method}"),
    };
    assert_eq!(headers["cache-control"], expected_cache_control);
    assert_eq!(headers["x-content-type-options"], "nosniff");
    let bytes = response
        .into_body()
        .collect()
        .await
        .unwrap()
        .to_bytes()
        .to_vec();
    (status, headers, bytes)
}

#[derive(Debug, PartialEq, Eq)]
struct ReceiptStorageRecord {
    target: String,
    object_key: String,
    bytes: Vec<u8>,
    digest: String,
}

async fn receipt_storage_record(h: &Harness, intent: &str) -> ReceiptStorageRecord {
    let row = h.admin.query_one(
        "SELECT storage_target_id,object_key,receipt_bytes,receipt_digest FROM audio_intake_receipts WHERE intent_id=$1",
        &[&intent],
    ).await.unwrap();
    ReceiptStorageRecord {
        target: row.get("storage_target_id"),
        object_key: row.get("object_key"),
        bytes: row.get("receipt_bytes"),
        digest: row.get("receipt_digest"),
    }
}

/// Non-secret test topology. Neither environment credentials nor any network adapter is used.
fn switched_filesystem_topology(h: &Harness, next: &PrivateRoot, include_legacy: bool) -> PathBuf {
    let old_root = fs::canonicalize(&h.root.0).unwrap();
    let new_root = fs::canonicalize(&next.0).unwrap();
    let mut contents = format!(
        "version = 1\n[storage]\nactive_write_target = \"next-filesystem\"\n[storage.targets.next-filesystem]\ndriver = \"filesystem\"\nprivate_root = {}\n",
        serde_json::to_string(new_root.to_str().unwrap()).unwrap(),
    );
    if include_legacy {
        contents.push_str(&format!(
            "[storage.targets.legacy-filesystem]\ndriver = \"filesystem\"\nprivate_root = {}\n",
            serde_json::to_string(old_root.to_str().unwrap()).unwrap(),
        ));
    }
    let path = old_root.join(format!("storage-topology-{include_legacy}.toml"));
    fs::write(&path, contents).unwrap();
    path
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn changing_the_write_target_preserves_exact_historical_reads_without_copying_or_fallback() {
    let h = Harness::new().await;
    let old_intent = h.intent().await;
    let old_bytes = opus();
    let (status, original) = h.upload(&old_intent, Some(ALICE), old_bytes.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let original: AudioReceiptResponse = serde_json::from_value(original).unwrap();
    let before = receipt_storage_record(&h, &old_intent.id).await;
    assert_eq!(before.target, "legacy-filesystem");
    assert_eq!(before.bytes, serde_json::to_vec(&original).unwrap());

    let next = PrivateRoot::new();
    let topology = switched_filesystem_topology(&h, &next, true);
    let switched = h.restart().with_storage_config(&topology).unwrap();
    let app = make_router(switched.clone());
    assert_eq!(fs::read_dir(next.0.join("objects")).unwrap().count(), 0);

    // The old object must be readable before the new target contains any object at all.
    let (status, headers, delivered) =
        owner_audio_request(&app, "GET", &old_intent.upload_path, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-length"], old_bytes.len().to_string());
    assert_eq!(delivered, old_bytes);
    assert_eq!(fs::read_dir(next.0.join("objects")).unwrap().count(), 0);
    assert_eq!(
        switched
            .load_audio_intent(ALICE, &h.script, &old_intent.id)
            .await
            .unwrap()
            .receipt,
        Some(original.clone()),
    );

    let new_intent = switched
        .create_audio_intent(ALICE, &h.script, h.intent_request())
        .await
        .unwrap();
    let new_bytes = synthetic_media(880, 1, true);
    let (status, _, response) =
        owner_audio_request(&app, "PUT", &new_intent.upload_path, new_bytes.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let new_receipt: AudioReceiptResponse = serde_json::from_slice(&response).unwrap();
    assert_ne!(new_receipt.media.sha256, original.media.sha256);
    let new_record = receipt_storage_record(&h, &new_intent.id).await;
    assert_eq!(new_record.target, "next-filesystem");
    assert_eq!(
        fs::read(next.0.join("objects").join(&new_record.object_key)).unwrap(),
        new_bytes
    );
    assert!(!h
        .root
        .0
        .join("objects")
        .join(&new_record.object_key)
        .exists());
    assert!(!next.0.join("objects").join(&before.object_key).exists());

    // A retry still returns the first receipt and uses its original accepted location.
    assert_eq!(
        switched
            .upload_audio_intent(ALICE, &h.script, &old_intent.id, old_bytes.clone())
            .await
            .unwrap(),
        original,
    );
    assert_eq!(receipt_storage_record(&h, &old_intent.id).await, before);
    assert!(!next.0.join("objects").join(&before.object_key).exists());
    for (intent, expected) in [(&old_intent, &old_bytes), (&new_intent, &new_bytes)] {
        let (status, headers, delivered) =
            owner_audio_request(&app, "GET", &intent.upload_path, vec![]).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(headers["content-type"], "audio/ogg");
        assert_eq!(headers["content-length"], expected.len().to_string());
        assert_eq!(&delivered, expected);
    }

    // Removing an historical target never makes the active target a fallback for its rows.
    let missing_topology = switched_filesystem_topology(&h, &next, false);
    let missing_target = h.restart().with_storage_config(&missing_topology).unwrap();
    let missing_app = make_router(missing_target);
    let (status, _, body) =
        owner_audio_request(&missing_app, "GET", &old_intent.upload_path, vec![]).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(
        serde_json::from_slice::<Value>(&body).unwrap()["code"],
        "unavailable"
    );
    let (status, _, delivered) =
        owner_audio_request(&missing_app, "GET", &new_intent.upload_path, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(delivered, new_bytes);
    assert_eq!(receipt_storage_record(&h, &old_intent.id).await, before);
    assert_eq!(h.counts().await, (2, 2));
}

#[tokio::test]
#[ignore = "new disposable PostgreSQL cluster and ffmpeg required"]
async fn baseline_replay_preserves_receipt_bytes_digests_storage_targets_and_reads() {
    let h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    let (status, accepted) = h.upload(&intent, Some(ALICE), bytes.clone()).await;
    assert_eq!(status, StatusCode::OK);
    let accepted: AudioReceiptResponse = serde_json::from_value(accepted).unwrap();
    let before = receipt_storage_record(&h, &intent.id).await;
    let migration_rows = h
        .admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap();
    let original_migrations: Vec<(i32, Vec<u8>)> = migration_rows
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(original_migrations.len(), 4);

    migrate(&h.config).await.unwrap();
    migrate(&h.config).await.unwrap();
    assert_eq!(receipt_storage_record(&h, &intent.id).await, before);
    assert_eq!(before.target, "legacy-filesystem");
    let migrated_rows = h
        .admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap();
    let retained_migrations: Vec<(i32, Vec<u8>)> = migrated_rows
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(retained_migrations, original_migrations);
    assert_eq!(
        h.admin
            .query_one(
                "SELECT count(*) FROM cantos_migrations WHERE version=1",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let target_columns: i64 = h.admin.query_one(
        "SELECT count(*) FROM information_schema.columns WHERE table_schema='public' AND table_name IN ('audio_intake_receipts','audio_renditions') AND column_name='storage_target_id' AND is_nullable='NO' AND column_default LIKE '%legacy-filesystem%'", &[],
    ).await.unwrap().get(0);
    assert_eq!(target_columns, 2);

    let restarted = h.restart();
    assert_eq!(
        restarted
            .load_audio_intent(ALICE, &h.script, &intent.id)
            .await
            .unwrap()
            .receipt,
        Some(accepted)
    );
    let app = make_router(restarted);
    let (status, headers, delivered) =
        owner_audio_request(&app, "GET", &intent.upload_path, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers["content-length"], bytes.len().to_string());
    assert_eq!(delivered, bytes);
    assert_eq!(h.counts().await, (1, 1));
}
