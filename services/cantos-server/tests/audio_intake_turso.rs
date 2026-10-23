//! Real run-owned Turso and private-filesystem audio intake; synthetic bytes encoded at runtime.
use std::{env, fs, path::PathBuf, process::Command};

use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use cantos_server::contract::studio::{
    AudioIntentResponse, AudioReceiptResponse, CreateAudioIntentRequest, SaveRevisionRequest,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::store::{Store, StoreError},
    script_ir::read_script,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[path = "support/native_sql.rs"]
mod native_sql;
use native_sql::NativeSql;
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const ORIGIN: &str = "http://127.0.0.1:8080";
const PROFILE: &str = "opus-mono-20-v1";
const MAX_BYTES: usize = 32 * 1024 * 1024;
const SCRIPT: &str =
    include_str!("../contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json");

struct PrivateRoot(PathBuf);

impl PrivateRoot {
    fn new() -> Self {
        let path = fs::canonicalize(env::temp_dir())
            .unwrap()
            .join(format!("cantos-audio-test-{}", Uuid::new_v4()));
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
    admin: NativeSql,
    store: Store,
    app: Router,
    root: PrivateRoot,
    script: String,
    digest: String,
}

impl Harness {
    async fn new() -> Self {
        Self::with_fixture(NativeSql::new().await).await
    }
    async fn with_fixture(admin: NativeSql) -> Self {
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.0.studio_caller(actor, token).await;
            for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
                admin.execute("INSERT INTO script_evidence VALUES(?1,?2,?3,'Synthetic reference; no clearance')", &[&actor,&kind,&id]).await.unwrap();
            }
        }
        let root = PrivateRoot::new();
        let store = admin.store().with_audio_storage(&root.0).unwrap();
        let script = Uuid::new_v4().to_string();
        let saved = store
            .save(ALICE, &script, revision_request(0, SCRIPT.into()))
            .await
            .unwrap();
        let app = make_router(store.clone());
        Self {
            admin,
            store,
            app,
            root,
            script,
            digest: saved.content_digest,
        }
    }

    async fn restart(&mut self) -> Store {
        let replacement = NativeSql::new().await;
        let old = std::mem::replace(&mut self.store, replacement.store());
        self.app = make_router(replacement.store());
        drop(old);
        self.admin.0.restart().await;
        self.store = self.admin.store().with_audio_storage(&self.root.0).unwrap();
        self.app = make_router(self.store.clone());
        self.store.clone()
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

#[tokio::test]
async fn intent_is_explicit_revision_bound_and_exact_operation_replays_original_identity() {
    let _media_lane = native_sql::media_lane().await;
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
async fn invalid_or_stale_target_never_creates_an_intent_and_there_is_no_default_profile() {
    let _media_lane = native_sql::media_lane().await;
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
async fn only_current_owner_can_create_read_or_upload_and_intent_scope_cannot_cross_scripts() {
    let _media_lane = native_sql::media_lane().await;
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
                "INSERT INTO script_members(script_id,actor_id,role) VALUES(?1,'bob',?2)
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
async fn http_keeps_same_origin_cookie_binary_and_current_studio_surface_boundaries() {
    let _media_lane = native_sql::media_lane().await;
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
async fn accepted_receipt_contains_observed_media_and_one_private_object_with_no_delivery_url() {
    let _media_lane = native_sql::media_lane().await;
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
            "SELECT object_key,receipt_bytes FROM audio_intake_receipts WHERE intent_id=?1",
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
async fn same_bytes_replay_exact_receipt_and_changed_valid_media_conflicts_without_replacement() {
    let _media_lane = native_sql::media_lane().await;
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
async fn rejected_media_and_body_limit_leave_intent_pending_with_no_accepted_row() {
    let _media_lane = native_sql::media_lane().await;
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
async fn lost_http_response_reconciles_and_fresh_store_replays_durable_receipt() {
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
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
    let restarted = h.restart().await;
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
async fn new_script_revision_blocks_pending_acceptance_and_preserves_historical_accepted_status() {
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
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
            .await
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
async fn revoked_expired_or_inactive_owner_cannot_read_replay_create_or_accept() {
    let _media_lane = native_sql::media_lane().await;
    for refusal in ["revoked", "expired", "inactive"] {
        let clock =
            cantos_store_turso::testing::FixedClock::new(chrono::Utc::now().timestamp_micros());
        let h = Harness::with_fixture(NativeSql(
            cantos_server::test_support::ServerFixture::with_clock(clock.clone()).await,
        ))
        .await;
        let intent = h.intent().await;
        let accepted = h.intent().await;
        let bytes = opus();
        assert_eq!(
            h.upload(&accepted, Some(ALICE), bytes.clone()).await.0,
            StatusCode::OK
        );
        let path = format!("/api/v1/studio/scripts/{}/audio-intents", h.script);
        let sql = match refusal {
            "revoked" => "UPDATE development_sessions SET revoked=true WHERE actor_id='alice'",
            "expired" => {
                clock.advance(3_600_000_001);
                "SELECT 1"
            }
            "inactive" => "UPDATE actors SET active=false WHERE id='alice'",
            _ => unreachable!(),
        };
        if refusal != "expired" {
            h.admin.execute(sql, &[]).await.unwrap();
        }
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
async fn receipt_insert_rollback_keeps_pending_intent_and_restart_retry_accepts_once() {
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    h.admin
        .execute(
            "CREATE TRIGGER inject_audio_receipt_failure BEFORE INSERT ON audio_intake_receipts BEGIN SELECT RAISE(ABORT,'synthetic fault'); END", &[],
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
        .await
        .load_audio_intent(ALICE, &h.script, &intent.id)
        .await
        .unwrap()
        .receipt
        .is_none());
    h.admin
        .execute("DROP TRIGGER inject_audio_receipt_failure", &[])
        .await
        .unwrap();
    let restarted = h.restart().await;
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
async fn tampered_private_object_blocks_status_and_replay_instead_of_claiming_verified_acceptance()
{
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
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
    let restarted = h.restart().await;
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
async fn overlapping_public_static_roots_never_serve_private_accepted_object_bytes() {
    let _media_lane = native_sql::media_lane().await;
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
async fn audio_enabled_host_blocks_nested_symlinks_and_regular_static_files() {
    let _media_lane = native_sql::media_lane().await;
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
    let editorial_only = router(
        AppState {
            store: h.admin.store(),
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
        "SELECT storage_target_id,object_key,receipt_bytes,receipt_digest FROM audio_intake_receipts WHERE intent_id=?1",
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
async fn changing_the_write_target_preserves_exact_historical_reads_without_copying_or_fallback() {
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
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
    let switched = h.restart().await.with_storage_config(&topology).unwrap();
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
    drop(app);
    drop(switched);
    let missing_target = h
        .restart()
        .await
        .with_storage_config(&missing_topology)
        .unwrap();
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
async fn concurrent_duplicate_intents_and_bytes_commit_one_original_immutable_receipt() {
    let _lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let request = h.intent_request();
    let (a, b) = tokio::join!(
        h.store
            .create_audio_intent(ALICE, &h.script, request.clone()),
        h.store.create_audio_intent(ALICE, &h.script, request)
    );
    let intent = a.unwrap();
    assert_eq!(b.unwrap(), intent);
    let bytes = opus();
    let (a, b) = tokio::join!(
        h.store
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone()),
        h.store
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes)
    );
    assert_eq!(a.unwrap(), b.unwrap());
    assert_eq!(h.counts().await, (1, 1));
    assert_eq!(fs::read_dir(h.root.0.join("objects")).unwrap().count(), 1);
}
#[tokio::test]
async fn settled_intents_and_receipts_reject_update_delete_replace_and_upsert() {
    let _lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let intent = h.intent().await;
    h.store
        .upload_audio_intent(ALICE, &h.script, &intent.id, opus())
        .await
        .unwrap();
    for table in ["audio_upload_intents", "audio_intake_receipts"] {
        for sql in [format!("UPDATE {table} SET profile_id=profile_id"),format!("DELETE FROM {table}"),format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table}"),format!("INSERT INTO {table} SELECT * FROM {table} WHERE true ON CONFLICT(id) DO UPDATE SET profile_id=excluded.profile_id")] {assert!(h.admin.0.try_exec(&sql,&[]).await.unwrap_err().guard().is_some(),"{sql}");}
    }
    assert_eq!(h.counts().await, (1, 1));
}
#[tokio::test]
async fn database_binding_and_receipt_digest_are_independent_integrity_barriers() {
    let _lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
    let id = Uuid::new_v4().to_string();
    let op = Uuid::new_v4().to_string();
    let digest = format!("sir-c1:sha256:{}", "0".repeat(64));
    // The SQL guard and adapter independently bind the exact owner, revision and digest.
    h.admin.execute("INSERT INTO audio_upload_intents(id,script_id,script_revision,content_digest,profile_id,operation_id,created_by,created_at) VALUES(?1,?2,1,?3,?4,?5,'alice',0)",&[&id,&h.script,&digest,&PROFILE,&op]).await.unwrap_err();
    assert_eq!(h.counts().await, (0, 0));
    let intent = h.intent().await;
    h.store
        .upload_audio_intent(ALICE, &h.script, &intent.id, opus())
        .await
        .unwrap();
    h.admin
        .execute("DROP TRIGGER audio_intake_receipts_no_update", &[])
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE audio_intake_receipts SET receipt_bytes=x'7b7d'",
            &[],
        )
        .await
        .unwrap();
    let restarted = h.restart().await;
    assert!(matches!(
        restarted
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
async fn native_migration_replay_preserves_receipt_bytes_digests_storage_targets_and_reads() {
    let _lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
    let intent = h.intent().await;
    let bytes = opus();
    let accepted = h
        .store
        .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone())
        .await
        .unwrap();
    let before = receipt_storage_record(&h, &intent.id).await;
    let history: Vec<(i64, String)> = h
        .admin
        .query(
            "SELECT version,hex(checksum) FROM cantos_schema_history ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    for _ in 0..2 {
        let replacement = NativeSql::new().await;
        let old = std::mem::replace(&mut h.store, replacement.store());
        h.app = make_router(replacement.store());
        drop(old);
        h.admin.0.close();
        let db = cantos_server::platform::turso::ServerDatabase::open(
            h.admin.0.database_path(),
            cantos_store_turso::registry::Intent::Migrate,
            std::sync::Arc::new(cantos_store_turso::clock::SystemClock),
        )
        .await
        .unwrap();
        drop(db);
        h.admin.0.restart().await;
        h.store = h.admin.store().with_audio_storage(&h.root.0).unwrap();
        h.app = make_router(h.store.clone());
    }
    assert_eq!(receipt_storage_record(&h, &intent.id).await, before);
    let after: Vec<(i64, String)> = h
        .admin
        .query(
            "SELECT version,hex(checksum) FROM cantos_schema_history ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect();
    assert_eq!(after, history);
    assert_eq!(
        h.store
            .upload_audio_intent(ALICE, &h.script, &intent.id, bytes.clone())
            .await
            .unwrap(),
        accepted
    );
    let (status, _, actual) = owner_audio_request(&h.app, "GET", &intent.upload_path, vec![]).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(actual, bytes);
    assert_eq!(h.counts().await, (1, 1));
}
