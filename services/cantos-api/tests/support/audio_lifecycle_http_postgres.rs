//! Real Router/application, app-role PostgreSQL and private filesystem; encoded test signals only.
use super::*;
use std::{os::unix::fs::PermissionsExt, path::PathBuf, process::Command};
const UPLOAD: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
struct PrivateRoot(PathBuf, bool);
impl PrivateRoot {
    fn new() -> Self {
        let path = fs::canonicalize(env::temp_dir())
            .unwrap()
            .join(format!("cantos-lifecycle-{}", operation()));
        fs::create_dir(&path).unwrap();
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
        Self(path, false)
    }
    fn encode(&self, bitrate: &str) -> Vec<u8> {
        let path = self.0.join(format!("signal-{}.opus", operation()));
        let output = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-ac",
                "1",
                "-c:a",
                "libopus",
                "-b:a",
                bitrate,
                "-vbr",
                "off",
            ])
            .arg(&path)
            .output()
            .unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        fs::read(path).unwrap()
    }
}
impl Drop for PrivateRoot {
    fn drop(&mut self) {
        if !self.1 {
            let _ = fs::remove_dir_all(&self.0);
        }
    }
}
struct AudioHarness {
    h: Harness,
    root: PrivateRoot,
    unit: String,
    content: Value,
    brief: Value,
}
impl AudioHarness {
    async fn new(recording: bool, profile: &str) -> Self {
        let mut h = Harness::new().await;
        h.admin.batch_execute("GRANT SELECT,INSERT ON audio_submissions,audio_submission_revocations,audio_renditions,audio_reviews,audio_release_candidates TO cantos_app;").await.unwrap();
        let root = PrivateRoot::new();
        h.store = h.store.clone().with_audio_storage(&root.0).unwrap();
        h.app = router(
            AppState {
                store: h.store.clone(),
                web_origin: ORIGIN.into(),
            },
            "unused-public-root",
        );
        let mut work_body = work_request(if recording { "podcast" } else { "novel" });
        work_body["details"]["rights_refs"] = json!(["catalog-rights"]);
        let w = h
            .json("POST", "/works", Some(ALICE), Some(work_body))
            .await
            .success(StatusCode::CREATED)["work"]
            .clone();
        let cu = h.unit(&w, "Tập có âm thanh riêng tư").await;
        let unit = cu["units"][0]["id"].as_str().unwrap().to_owned();
        let content = h
            .json(
                "POST",
                &format!("/content-units/{unit}/content/revisions"),
                Some(ALICE),
                Some(content_request(
                    0,
                    2,
                    if recording {
                        json!({"kind":"recording"})
                    } else {
                        json!({"kind":"text","text":"Người kể đã trở về.","source":null})
                    },
                )),
            )
            .await
            .success(StatusCode::CREATED)["content"]
            .clone();
        let mut req = brief_http::brief_request(&content, 0, recording);
        req["guidance"]["delivery"]["profile_id"] = json!(profile);
        let brief = h
            .json(
                "POST",
                &format!("/content-units/{unit}/production-briefs"),
                Some(ALICE),
                Some(req),
            )
            .await
            .success(StatusCode::CREATED)["brief"]
            .clone();
        Self {
            h,
            root,
            unit,
            content,
            brief,
        }
    }
    async fn request(&self, bytes: &[u8]) -> Value {
        let b = &self.brief;
        json!({"operation_id":operation(),"id":operation(),"binding":{"work_id":b["work"]["id"],"unit_id":self.unit,"content_revision_id":b["content"]["id"],"content_revision_digest":b["content"]["revision_digest"],"brief_id":b["id"],"brief_version":b["version"],"brief_digest":b["digest"],"profile_id":b["guidance"]["delivery"]["profile_id"]},"expected_sha256":format!("{:x}",Sha256::digest(bytes)),"expected_byte_length":bytes.len(),"declarations":{"producer":"Synthetic external producer","method":"recorded test signal","provenance":"Owned synthetic sine fixture; no speech or human listening","rights_refs":["catalog-rights"],"quality_report":"NOT_RUN listening"},"upload_token":UPLOAD,"expires_at":now(&self.h).await+7_200_000})
    }
    fn path(&self) -> String {
        format!("/content-units/{}/audio-submissions", self.unit)
    }
    async fn prepare(&self, request: Value) -> Value {
        self.h
            .json("POST", &self.path(), Some(ALICE), Some(request))
            .await
            .success(StatusCode::CREATED)
    }
    async fn counts(&self) -> Value {
        let text:String=self.h.admin.query_one("SELECT json_build_object('submissions',(SELECT count(*) FROM audio_submissions),'renditions',(SELECT count(*) FROM audio_renditions),'reviews',(SELECT count(*) FROM audio_reviews),'candidates',(SELECT count(*) FROM audio_release_candidates),'revocations',(SELECT count(*) FROM audio_submission_revocations))::text",&[]).await.unwrap().get(0);
        serde_json::from_str(&text).unwrap()
    }
}
async fn now(h: &Harness) -> i64 {
    h.admin
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .await
        .unwrap()
        .get(0)
}
async fn upload(app: Router, id: &str, token: &str, bytes: Vec<u8>) -> Reply {
    let response = app
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/v1/studio/producer/audio-submissions/{id}/bytes"
                ))
                .header("origin", ORIGIN)
                .header("content-type", "application/octet-stream")
                .header("accept", "application/json")
                .header("authorization", format!("Bearer {token}"))
                .body(Body::from(bytes))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    let status = response.status();
    let body: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    schema(
        if status.is_success() {
            "AudioRenditionDocument"
        } else {
            "ApiError"
        },
        &body,
    );
    Reply { status, body }
}
fn review_request(r: &Value, kind: &str, decision: &str) -> Value {
    json!({"operation_id":operation(),"rendition_digest":r["digest"],"sha256":r["verified_media"]["sha256"],"kind":kind,"decision":decision,"notes":"Named fixture review only; no listening acceptance","evidence_refs":["synthetic-test-evidence"]})
}
#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_all_profiles_recording_and_text_admit_real_bytes_replay_and_keep_declarations_separate(
) {
    for (i, (profile, rate)) in [
        ("opus-mono-20-v1", "20k"),
        ("opus-mono-24-v1", "24k"),
        ("opus-mono-32-v1", "32k"),
        ("opus-mono-40-v1", "40k"),
    ]
    .into_iter()
    .enumerate()
    {
        let a = AudioHarness::new(i % 2 == 0, profile).await;
        let bytes = a.root.encode(rate);
        let req = a.request(&bytes).await;
        let pending = a.prepare(req.clone()).await;
        assert_eq!(pending["state"], "awaiting_bytes");
        let id = req["id"].as_str().unwrap();
        assert_eq!(a.prepare(req.clone()).await, pending);
        let r = upload(a.h.app.clone(), id, UPLOAD, bytes.clone())
            .await
            .success(StatusCode::OK);
        assert_eq!(r["binding"], req["binding"]);
        assert_eq!(r["declarations"], req["declarations"]);
        assert_eq!(r["verified_media"]["sha256"], req["expected_sha256"]);
        assert_eq!(r["verified_media"]["byte_length"], bytes.len());
        assert_eq!(r["verified_media"]["profile_id"], profile);
        assert!(r.get("object_key").is_none());
        assert!(r.get("upload_token").is_none());
        assert_eq!(
            upload(a.h.app.clone(), id, UPLOAD, bytes)
                .await
                .success(StatusCode::OK),
            r
        );
        let status =
            a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
                .await
                .success(StatusCode::OK);
        assert_eq!(status["state"], "admitted");
        assert_eq!(status["rendition"], r);
        assert_eq!(a.counts().await["renditions"], 1);
        let mut changed = req.clone();
        changed["expected_sha256"] = json!("f".repeat(64));
        a.h.json("POST", &a.path(), Some(ALICE), Some(changed))
            .await
            .error(StatusCode::CONFLICT, "operation_reused");
        let mut cfg = a.h.config.clone();
        cfg.user("cantos_app");
        let restarted = Store::new(cfg)
            .unwrap()
            .with_audio_storage(&a.root.0)
            .unwrap();
        let restored = restarted
            .load_audio_submission(ALICE, &a.unit, id)
            .await
            .unwrap();
        assert_eq!(serde_json::to_value(restored).unwrap(), status);
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_wrong_owner_scope_digest_unknown_profile_and_stale_brief_make_no_partial_records() {
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let req = a.request(&bytes).await;
    let id = req["id"].as_str().unwrap();
    let empty = a.counts().await;
    a.h.json("POST", &a.path(), Some(BOB), Some(req.clone()))
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(a.counts().await, empty);
    for field in [
        "brief_id",
        "content_revision_id",
        "content_revision_digest",
        "profile_id",
        "work_id",
    ] {
        let mut bad = req.clone();
        bad["operation_id"] = json!(operation());
        bad["id"] = json!(operation());
        bad["binding"][field] = json!(if field == "profile_id" {
            "aac-mono-40-v1".into()
        } else {
            operation()
        });
        a.h.json("POST", &a.path(), Some(ALICE), Some(bad))
            .await
            .error(StatusCode::BAD_REQUEST, "invalid_request");
        assert_eq!(a.counts().await, empty);
    }
    a.h.json(
        "GET",
        "/content-units/%FF/audio-submissions/not-id",
        Some(ALICE),
        None,
    )
    .await
    .error(StatusCode::BAD_REQUEST, "invalid_request");
    a.h.json("PATCH", &a.path(), Some(ALICE), Some(json!({})))
        .await
        .error(StatusCode::METHOD_NOT_ALLOWED, "invalid_request");
    a.h.json(
        "PATCH",
        &format!("/scripts/{}/audio-intents", operation()),
        Some(ALICE),
        Some(json!({})),
    )
    .await
    .error(StatusCode::METHOD_NOT_ALLOWED, "invalid_request");
    let response =
        a.h.app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!("/api/v1/studio/producer/audio-submissions/{id}"))
                    .header("origin", ORIGIN)
                    .header("authorization", format!("Bearer {UPLOAD}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
    assert_eq!(response.status(), StatusCode::METHOD_NOT_ALLOWED);
    let error: Value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(error["code"], "invalid_request");
    schema("ApiError", &error);
    a.prepare(req.clone()).await;
    let before = a.counts().await;
    upload(a.h.app.clone(), id, ALICE, bytes.clone())
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    upload(a.h.app.clone(), id, UPLOAD, b"wrong digest".to_vec())
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    a.h.json("GET", &format!("{}/{id}", a.path()), Some(BOB), None)
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    a.h.json(
        "GET",
        &format!("/content-units/{}/audio-submissions/{id}", operation()),
        Some(ALICE),
        None,
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(a.counts().await, before);
    a.h.json(
        "POST",
        &format!("/content-units/{}/production-briefs", a.unit),
        Some(ALICE),
        Some(brief_http::brief_request(&a.content, 1, true)),
    )
    .await
    .success(StatusCode::CREATED);
    upload(a.h.app.clone(), id, UPLOAD, bytes)
        .await
        .error(StatusCode::CONFLICT, "stale_production_inputs");
    assert_eq!(a.counts().await, before);
    assert_eq!(
        a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
            .await
            .success(StatusCode::OK)["state"],
        "stale"
    );
}
#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_concurrent_finalize_deduplicates_object_and_rendition_and_failure_retries_cleanly() {
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let req = a.request(&bytes).await;
    let id = req["id"].as_str().unwrap();
    a.prepare(req.clone()).await;
    a.h.admin.batch_execute("CREATE FUNCTION fail_audio_commit() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'owned test failpoint after private object persistence'; END $$; CREATE TRIGGER fail_audio BEFORE INSERT ON audio_renditions FOR EACH ROW EXECUTE FUNCTION fail_audio_commit();").await.unwrap();
    upload(a.h.app.clone(), id, UPLOAD, bytes.clone())
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    assert_eq!(a.counts().await["renditions"], 0);
    let entries = fs::read_dir(a.root.0.join("objects"))
        .unwrap()
        .map(|e| e.unwrap().file_name())
        .collect::<Vec<_>>();
    assert_eq!(entries.len(), 1);
    assert!(!entries[0].to_string_lossy().starts_with(".pending"));
    a.h.admin
        .batch_execute(
            "DROP TRIGGER fail_audio ON audio_renditions; DROP FUNCTION fail_audio_commit();",
        )
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        upload(a.h.app.clone(), id, UPLOAD, bytes.clone()),
        upload(a.h.app.clone(), id, UPLOAD, bytes.clone())
    );
    let r = left.success(StatusCode::OK);
    assert_eq!(right.success(StatusCode::OK), r);
    assert_eq!(a.counts().await["renditions"], 1);
    assert_eq!(fs::read_dir(a.root.0.join("objects")).unwrap().count(), 1);
    let mut second = a.request(&bytes).await;
    second["upload_token"] = json!("e".repeat(64));
    let other = second["id"].as_str().unwrap().to_owned();
    a.prepare(second).await;
    let r2 = upload(a.h.app.clone(), &other, &"e".repeat(64), bytes)
        .await
        .success(StatusCode::OK);
    assert_ne!(r["id"], r2["id"]);
    assert_eq!(r["verified_media"], r2["verified_media"]);
    assert_eq!(fs::read_dir(a.root.0.join("objects")).unwrap().count(), 1);
}
#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_reviews_candidates_bind_exact_bytes_and_latest_decisions_without_a_public_release() {
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let req = a.request(&bytes).await;
    let id = req["id"].as_str().unwrap();
    a.prepare(req.clone()).await;
    let r = upload(a.h.app.clone(), id, UPLOAD, bytes)
        .await
        .success(StatusCode::OK);
    let path = format!("{}/{id}/reviews", a.path());
    let mut records = vec![];
    for kind in ["qc", "rights", "editorial"] {
        let request = review_request(&r, kind, "approved");
        let record =
            a.h.json("POST", &path, Some(ALICE), Some(request.clone()))
                .await
                .success(StatusCode::CREATED);
        assert_eq!(
            a.h.json("POST", &path, Some(ALICE), Some(request.clone()))
                .await
                .success(StatusCode::CREATED),
            record
        );
        let mut conflict = request;
        conflict["decision"] = json!("rejected");
        a.h.json("POST", &path, Some(ALICE), Some(conflict))
            .await
            .error(StatusCode::CONFLICT, "operation_reused");
        records.push(record);
    }
    let candidates = format!("{}/{id}/release-candidates", a.path());
    let request = json!({"operation_id":operation(),"rendition_digest":r["digest"]});
    let candidate =
        a.h.json("POST", &candidates, Some(ALICE), Some(request.clone()))
            .await
            .success(StatusCode::CREATED);
    assert_eq!(
        candidate["blockers"],
        json!(["public_delivery_not_implemented"])
    );
    assert_eq!(
        a.h.json("POST", &candidates, Some(ALICE), Some(request.clone()))
            .await
            .success(StatusCode::CREATED),
        candidate
    );
    a.h.json(
        "POST",
        &path,
        Some(BOB),
        Some(review_request(&r, "qc", "approved")),
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    let mut wrong = review_request(&r, "qc", "approved");
    wrong["sha256"] = json!("f".repeat(64));
    a.h.json("POST", &path, Some(ALICE), Some(wrong))
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    let rejection =
        a.h.json(
            "POST",
            &path,
            Some(ALICE),
            Some(review_request(&r, "qc", "rejected")),
        )
        .await
        .success(StatusCode::CREATED);
    assert_eq!(rejection["sequence"], 4);
    let read_path = format!(
        "{}/{}",
        candidates,
        candidate["candidate"]["id"].as_str().unwrap()
    );
    let blocked =
        a.h.json("GET", &read_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(blocked["candidate"], candidate["candidate"]);
    assert_eq!(
        blocked["blockers"],
        json!([
            "qc_not_approved",
            "candidate_reviews_changed",
            "public_delivery_not_implemented"
        ])
    );
    a.h.json(
        "POST",
        &path,
        Some(ALICE),
        Some(review_request(&r, "rights", "withdrawn")),
    )
    .await
    .success(StatusCode::CREATED);
    let status =
        a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(status["reviews"].as_array().unwrap().len(), 3);
    assert_eq!(a.counts().await["reviews"], 5);
    for endpoint in [
        "/api/v1/theatre/audio",
        "/api/v1/theatre/releases",
        "/api/v1/admin/audio",
        "/api/v1/studio/releases",
    ] {
        let response =
            a.h.app
                .clone()
                .oneshot(
                    Request::builder()
                        .uri(endpoint)
                        .body(Body::empty())
                        .unwrap(),
                )
                .await
                .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
    let app = connect(&{
        let mut c = a.h.config.clone();
        c.user("cantos_app");
        c
    })
    .await;
    for table in [
        "audio_submissions",
        "audio_renditions",
        "audio_reviews",
        "audio_release_candidates",
        "audio_submission_revocations",
    ] {
        let error = app
            .batch_execute(&format!("DELETE FROM {table}"))
            .await
            .unwrap_err();
        assert_eq!(error.code().unwrap().code(), "42501");
        let error =
            a.h.admin
                .batch_execute(&format!("TRUNCATE {table} CASCADE"))
                .await
                .unwrap_err();
        assert_eq!(error.code().unwrap().code(), "55000");
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_revocation_expiry_and_two_way_read_upload_credential_separation_are_enforced() {
    for upload_first in [true, false] {
        let a = AudioHarness::new(true, "opus-mono-24-v1").await;
        let bytes = a.root.encode("24k");
        let req = a.request(&bytes).await;
        let id = req["id"].as_str().unwrap();
        let grant = json!({"operation_id":operation(),"id":operation(),"brief_digest":a.brief["digest"],"token":UPLOAD,"expires_at":now(&a.h).await+7_200_000,"capabilities":a.brief["guidance"]["required_capabilities"]});
        let path = format!(
            "/content-units/{}/production-briefs/1/producer-grants",
            a.unit
        );
        if upload_first {
            a.prepare(req.clone()).await;
            a.h.json("POST", &path, Some(ALICE), Some(grant))
                .await
                .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
            a.h.json(
                "DELETE",
                &format!("{}/{id}/upload-permission", a.path()),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::NO_CONTENT);
            upload(a.h.app.clone(), id, UPLOAD, bytes)
                .await
                .error(StatusCode::UNAUTHORIZED, "unauthenticated");
            assert_eq!(a.counts().await["revocations"], 1);
        } else {
            a.h.json("POST", &path, Some(ALICE), Some(grant))
                .await
                .success(StatusCode::CREATED);
            a.h.json("POST", &a.path(), Some(ALICE), Some(req))
                .await
                .error(StatusCode::BAD_REQUEST, "invalid_request");
            assert_eq!(a.counts().await["submissions"], 0);
        }
    }
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let mut req = a.request(&bytes).await;
    req["expires_at"] = json!(now(&a.h).await + 250);
    let id = req["id"].as_str().unwrap().to_owned();
    a.prepare(req).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    upload(a.h.app.clone(), &id, UPLOAD, bytes)
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert_eq!(a.counts().await["renditions"], 0);
    assert_eq!(
        a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
            .await
            .success(StatusCode::OK)["state"],
        "expired"
    );
}
#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_actual_corruption_missing_and_tampered_objects_fail_closed_and_cleanup_staging() {
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = b"not ogg or an audio file".to_vec();
    let req = a.request(&bytes).await;
    let id = req["id"].as_str().unwrap();
    a.prepare(req.clone()).await;
    upload(a.h.app.clone(), id, UPLOAD, bytes)
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "audio_rejected");
    assert_eq!(a.counts().await["renditions"], 0);
    assert_eq!(fs::read_dir(a.root.0.join("objects")).unwrap().count(), 0);
    let bytes = a.root.encode("24k");
    let mut req = a.request(&bytes).await;
    req["upload_token"] = json!("e".repeat(64));
    let id = req["id"].as_str().unwrap().to_owned();
    a.prepare(req).await;
    let r = upload(a.h.app.clone(), &id, &"e".repeat(64), bytes)
        .await
        .success(StatusCode::OK);
    let object = a
        .root
        .0
        .join("objects")
        .join(r["verified_media"]["sha256"].as_str().unwrap());
    fs::set_permissions(&object, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&object, b"tampered").unwrap();
    fs::set_permissions(&object, fs::Permissions::from_mode(0o400)).unwrap();
    a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
    fs::remove_file(object).unwrap();
    a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
    assert_eq!(a.counts().await["renditions"], 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_expiry_after_rendition_insert_rolls_back_and_preserves_only_private_orphan() {
    const LOCK: i64 = 7_215_143;
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let mut req = a.request(&bytes).await;
    req["expires_at"] = json!(now(&a.h).await + 2500);
    let id = req["id"].as_str().unwrap().to_owned();
    a.prepare(req).await;
    a.h.admin.batch_execute(&format!("CREATE FUNCTION hold_audio_insert() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN PERFORM pg_advisory_xact_lock({LOCK}); RETURN NEW; END $$; CREATE TRIGGER hold_audio BEFORE INSERT ON audio_renditions FOR EACH ROW EXECUTE FUNCTION hold_audio_insert();")).await.unwrap();
    a.h.admin
        .execute("SELECT pg_advisory_lock($1)", &[&LOCK])
        .await
        .unwrap();
    let app = a.h.app.clone();
    let upload_id = id.clone();
    let pending = tokio::spawn(async move { upload(app, &upload_id, UPLOAD, bytes).await });
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let blocked:bool=a.h.admin.query_one("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND wait_event='advisory' AND query LIKE 'INSERT INTO audio_renditions%')",&[]).await.unwrap().get(0);
        if blocked {
            break;
        }
        assert!(
            Instant::now() < deadline,
            "rendition INSERT did not reach owned failpoint"
        );
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    assert_eq!(fs::read_dir(a.root.0.join("objects")).unwrap().count(), 1);
    tokio::time::sleep(Duration::from_millis(2700)).await;
    a.h.admin
        .execute("SELECT pg_advisory_unlock($1)", &[&LOCK])
        .await
        .unwrap();
    pending
        .await
        .unwrap()
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert_eq!(a.counts().await["renditions"], 0);
    assert_eq!(
        a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
            .await
            .success(StatusCode::OK)["state"],
        "expired"
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_read_upload_grant_race_admits_only_one_capability_for_a_secret() {
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let req = a.request(&bytes).await;
    let grant = json!({"operation_id":operation(),"id":operation(),"brief_digest":a.brief["digest"],"token":UPLOAD,"expires_at":now(&a.h).await+7_200_000,"capabilities":a.brief["guidance"]["required_capabilities"]});
    let path = format!(
        "/content-units/{}/production-briefs/1/producer-grants",
        a.unit
    );
    let submission_path = a.path();
    let (read, write) = tokio::join!(
        a.h.json("POST", &path, Some(ALICE), Some(grant)),
        a.h.json("POST", &submission_path, Some(ALICE), Some(req))
    );
    assert_ne!(read.status.is_success(), write.status.is_success());
    let count:i64=a.h.admin.query_one("SELECT (SELECT count(*) FROM production_producer_grants)+(SELECT count(*) FROM audio_submissions)",&[]).await.unwrap().get(0);
    assert_eq!(count, 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_restart_fixture_pins_real_private_bytes_receipts_reviews_candidates() {
    let mut a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let req = a.request(&bytes).await;
    let id = req["id"].as_str().unwrap();
    a.prepare(req.clone()).await;
    let r = upload(a.h.app.clone(), id, UPLOAD, bytes.clone())
        .await
        .success(StatusCode::OK);
    let review =
        a.h.json(
            "POST",
            &format!("{}/{id}/reviews", a.path()),
            Some(ALICE),
            Some(review_request(&r, "qc", "approved")),
        )
        .await
        .success(StatusCode::CREATED);
    let candidate_request = json!({"operation_id":operation(),"rendition_digest":r["digest"]});
    let candidate =
        a.h.json(
            "POST",
            &format!("{}/{id}/release-candidates", a.path()),
            Some(ALICE),
            Some(candidate_request.clone()),
        )
        .await
        .success(StatusCode::CREATED);
    let status =
        a.h.json("GET", &format!("{}/{id}", a.path()), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    a.root.1 = true;
    fs::create_dir_all(EVIDENCE).unwrap();
    fs::write(format!("{EVIDENCE}/audio-restart.json"),serde_json::to_vec_pretty(&json!({"database":a.h.database,"root":a.root.0,"unit":a.unit,"request":req,"rendition":r,"review":review,"candidate_request":candidate_request,"candidate":candidate,"status":status,"token":ALICE,"upload_token":UPLOAD,"provenance":"Actual Ogg Opus synthetic encoded signal, real app-role PostgreSQL, private filesystem; no speech/listening/production release"})).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL + trusted media tools required"]
async fn audio_read_baseline_uses_current_owner_app_role_and_exact_bounded_point_queries() {
    let a = AudioHarness::new(true, "opus-mono-24-v1").await;
    let bytes = a.root.encode("24k");
    let req = a.request(&bytes).await;
    let id = req["id"].as_str().unwrap();
    a.prepare(req.clone()).await;
    let r = upload(a.h.app.clone(), id, UPLOAD, bytes.clone())
        .await
        .success(StatusCode::OK);
    let review_request = review_request(&r, "qc", "approved");
    a.h.json(
        "POST",
        &format!("{}/{id}/reviews", a.path()),
        Some(ALICE),
        Some(review_request),
    )
    .await
    .success(StatusCode::CREATED);
    let candidate =
        a.h.json(
            "POST",
            &format!("{}/{id}/release-candidates", a.path()),
            Some(ALICE),
            Some(json!({"operation_id":operation(),"rendition_digest":r["digest"]})),
        )
        .await
        .success(StatusCode::CREATED);
    let candidate_id = candidate["candidate"]["id"].as_str().unwrap();
    let candidate_path = format!("{}/{id}/release-candidates/{candidate_id}", a.path());
    let mut candidate_samples = vec![];
    let mut samples = vec![];
    let path = format!("{}/{id}", a.path());
    for _ in 0..20 {
        let start = Instant::now();
        let status =
            a.h.json("GET", &path, Some(ALICE), None)
                .await
                .success(StatusCode::OK);
        assert_eq!(status["rendition"], r);
        samples.push(start.elapsed().as_secs_f64() * 1000.0);
        let start = Instant::now();
        assert_eq!(
            a.h.json("GET", &candidate_path, Some(ALICE), None)
                .await
                .success(StatusCode::OK),
            candidate
        );
        candidate_samples.push(start.elapsed().as_secs_f64() * 1000.0);
    }
    for timings in [&samples, &candidate_samples] {
        let mut sorted = timings.clone();
        sorted.sort_by(f64::total_cmp);
        assert!(
            sorted[18] <= 500.0,
            "local warm p95 exceeds workload regression target"
        );
    }
    let mut config = a.h.config.clone();
    config.user("cantos_app");
    let app = connect(&config).await;
    let role: String = app
        .query_one("SELECT current_user", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(role, "cantos_app");
    let mut plans = serde_json::Map::new();
    for (name, sql, key) in [
        (
            "submission",
            "SELECT * FROM audio_submissions WHERE id=$1",
            id,
        ),
        (
            "rendition",
            "SELECT * FROM audio_renditions WHERE submission_id=$1",
            id,
        ),
        (
            "reviews",
            "SELECT * FROM audio_reviews WHERE rendition_id=$1 ORDER BY sequence DESC LIMIT 1001",
            r["id"].as_str().unwrap(),
        ),
    ] {
        let lines = app
            .query(&format!("EXPLAIN (ANALYZE,BUFFERS) {sql}"), &[&key])
            .await
            .unwrap()
            .iter()
            .map(|row| row.get::<_, String>(0))
            .collect::<Vec<_>>();
        plans.insert(name.into(), json!({"sql":sql,"plan":lines}));
    }
    let candidate_sql = "SELECT * FROM audio_release_candidates WHERE id=$1 AND rendition_id=$2";
    let candidate_plan = app
        .query(
            &format!("EXPLAIN (ANALYZE,BUFFERS) {candidate_sql}"),
            &[&candidate_id, &r["id"].as_str().unwrap()],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get::<_, String>(0))
        .collect::<Vec<_>>();
    plans.insert(
        "candidate".into(),
        json!({"sql":candidate_sql,"plan":candidate_plan}),
    );
    fs::create_dir_all(EVIDENCE).unwrap();
    fs::write(format!("{EVIDENCE}/audio-read-baseline.json"),serde_json::to_vec_pretty(&json!({"role":role,"postgres":a.h.admin.query_one("SHOW server_version",&[]).await.unwrap().get::<_,String>(0),"samples_ms":samples,"candidate_samples_ms":candidate_samples,"plans":plans,"object_bytes":bytes.len(),"target_p95_ms":500,"targets_are_local_workload_only":true,"dbsp":"reject: authorized bounded point reads; no hot aggregate/CDC consumer; no DBSP change","scope":"Warm local synthetic 1submission/1rendition/1review/1candidate;20status+20candidate actual Router/app-role/current-owner status reads with actual private hash verification; no production cardinality/SLO claim"})).unwrap()).unwrap();
}
