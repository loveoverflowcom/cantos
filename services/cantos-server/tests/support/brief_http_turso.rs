//! The parent target exercises real Router/application/native Turso and run-owned fixtures.
use super::*;

pub(super) fn brief_request(content: &Value, version: u64, recording: bool) -> Value {
    let mut value: Value = serde_json::from_str(if recording {
        include_str!("../../contracts/studio/briefs/v1/fixtures/create-recording.json")
    } else {
        include_str!("../../contracts/studio/briefs/v1/fixtures/create-narration.json")
    })
    .unwrap();
    value["operation_id"] = json!(operation());
    value["id"] = json!(operation());
    value["expected_version"] = json!(version);
    value["content_revision_id"] = content["id"].clone();
    value["content_revision_digest"] = content["revision_digest"].clone();
    value
}
async fn ready(h: &Harness, recording: bool) -> (String, Value) {
    let w = h
        .create(ALICE, if recording { "podcast" } else { "novel" })
        .await;
    let cu = h.unit(&w, "Chương hoặc tập có brief").await;
    let unit = cu["units"][0]["id"].as_str().unwrap().to_owned();
    let c = h
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
    (unit, c)
}
async fn now(h: &Harness) -> i64 {
    h.admin.0.clock_reading().unwrap() / 1_000
}
pub(super) async fn producer(app: Router, unit: &str, version: u64, token: Option<&str>) -> Reply {
    let mut builder = Request::builder()
        .uri(format!(
            "/api/v1/studio/producer/content-units/{unit}/production-briefs/{version}"
        ))
        .header("accept", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    let response = app
        .oneshot(builder.body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.headers()["cache-control"], "no-store");
    assert_eq!(response.headers()["x-content-type-options"], "nosniff");
    let status = response.status();
    let value =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    schema(
        if status.is_success() {
            "ProductionBriefDocument"
        } else {
            "ApiError"
        },
        &value,
    );
    Reply {
        status,
        body: value,
    }
}
fn grant_request(brief: &Value, expires: i64, token: &str) -> Value {
    json!({"operation_id":operation(),"id":operation(),"brief_digest":brief["digest"],"token":token,"expires_at":expires,"capabilities":brief["guidance"]["required_capabilities"]})
}

#[tokio::test]
async fn brief_text_recording_profiles_replay_and_conflicting_creation_are_real_http() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    for recording in [false, true] {
        let (unit, c) = ready(&h, recording).await;
        let path = format!("/content-units/{unit}/production-briefs");
        let mut submitted = brief_request(&c, 0, recording);
        let original = h
            .json("POST", &path, Some(ALICE), Some(submitted.clone()))
            .await
            .success(StatusCode::CREATED);
        assert_eq!(original["freshness"], "current");
        assert_eq!(original["brief"]["content"], c);
        assert_eq!(
            h.json("POST", &path, Some(ALICE), Some(submitted.clone()))
                .await
                .success(StatusCode::CREATED),
            original
        );
        let mut nfd = submitted.clone();
        nfd["guidance"]["editorial"]["notes"] = json!("Đọc rõ, giữ hơi thở tự nhiên.");
        assert_eq!(
            h.json("POST", &path, Some(ALICE), Some(nfd))
                .await
                .success(StatusCode::CREATED),
            original
        );
        let mut bad = submitted.clone();
        bad["guidance"]["delivery"]["profile_id"] = json!("opus-mono-32-v1");
        h.json("POST", &path, Some(ALICE), Some(bad))
            .await
            .error(StatusCode::CONFLICT, "operation_reused");
        let mut duplicate = submitted.clone();
        duplicate["operation_id"] = json!(operation());
        duplicate["expected_version"] = json!(1);
        h.json("POST", &path, Some(ALICE), Some(duplicate))
            .await
            .error(StatusCode::CONFLICT, "operation_reused");
        for (offset, profile) in ["opus-mono-20-v1", "opus-mono-32-v1", "opus-mono-40-v1"]
            .into_iter()
            .enumerate()
        {
            submitted = brief_request(&c, (offset + 1) as u64, recording);
            submitted["guidance"]["delivery"]["profile_id"] = json!(profile);
            let next = h
                .json("POST", &path, Some(ALICE), Some(submitted))
                .await
                .success(StatusCode::CREATED);
            assert_eq!(next["brief"]["version"], offset + 2);
            assert_eq!(next["brief"]["guidance"]["delivery"]["profile_id"], profile);
        }
        let old = h
            .json("GET", &format!("{path}/1"), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
        assert_eq!(old["brief"], original["brief"]);
        assert_eq!(old["freshness"], "guidance_changed");
        for profile in ["aac-mono-40-v1", "opus-mono-24-v2", ""] {
            let mut bad = brief_request(&c, 4, recording);
            bad["guidance"]["delivery"]["profile_id"] = json!(profile);
            h.json("POST", &path, Some(ALICE), Some(bad))
                .await
                .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
        }
    }
}

#[tokio::test]
async fn producer_scope_studio_cookie_capabilities_revocation_and_owner_isolation() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    let brief = h
        .json(
            "POST",
            &path,
            Some(ALICE),
            Some(brief_request(&c, 0, false)),
        )
        .await
        .success(StatusCode::CREATED)["brief"]
        .clone();
    let grant_path = format!("{path}/1/producer-grants");
    let token = "d".repeat(64);
    let body = grant_request(&brief, now(&h).await + 60000, &token);
    h.json("POST", &grant_path, Some(BOB), Some(body.clone()))
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    let work = h
        .json(
            "GET",
            &format!("/works/{}", brief["work"]["id"].as_str().unwrap()),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    h.member(&work, MemberRole::Reader).await;
    h.json("GET", &format!("{path}/1"), Some(BOB), None)
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    let mut missing = body.clone();
    missing["capabilities"] = json!(["ogg_opus_mono"]);
    h.json("POST", &grant_path, Some(ALICE), Some(missing))
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    let mut reuse_cookie = body.clone();
    reuse_cookie["token"] = json!(ALICE);
    h.json("POST", &grant_path, Some(ALICE), Some(reuse_cookie))
        .await
        .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    let granted = h
        .json("POST", &grant_path, Some(ALICE), Some(body.clone()))
        .await
        .success(StatusCode::CREATED);
    assert_eq!(granted["permission"], "read_instructions");
    assert!(granted.get("token").is_none());
    assert_eq!(
        h.json("POST", &grant_path, Some(ALICE), Some(body.clone()))
            .await
            .success(StatusCode::CREATED),
        granted
    );
    let mut changed = body.clone();
    changed["expires_at"] = json!(now(&h).await + 120000);
    h.json("POST", &grant_path, Some(ALICE), Some(changed))
        .await
        .error(StatusCode::CONFLICT, "operation_reused");
    assert_eq!(
        producer(h.app.clone(), &unit, 1, Some(&token))
            .await
            .success(StatusCode::OK),
        brief
    );
    producer(h.app.clone(), &unit, 1, Some(ALICE))
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    producer(h.app.clone(), &operation(), 1, Some(&token))
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    producer(h.app.clone(), &unit, 1, None)
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    h.json(
        "GET",
        &format!("/producer/content-units/{unit}/production-briefs/1"),
        Some(ALICE),
        None,
    )
    .await
    .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    let gid = granted["id"].as_str().unwrap();
    for _ in 0..2 {
        assert_eq!(
            h.json(
                "DELETE",
                &format!("/content-units/{unit}/producer-grants/{gid}"),
                Some(ALICE),
                None
            )
            .await
            .status,
            StatusCode::NO_CONTENT
        );
    }
    producer(h.app.clone(), &unit, 1, Some(&token))
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM production_producer_revocations", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        1
    );
    let hashes: Vec<Vec<u8>> = h
        .admin
        .query("SELECT token_hash FROM production_producer_grants", &[])
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(hashes.len(), 1);
    assert_ne!(hashes[0], token_hash(&token));
    assert!(!h
        .admin
        .query_one("SELECT CAST(snapshot AS TEXT) FROM production_briefs", &[])
        .await
        .unwrap()
        .get::<_, String>(0)
        .contains(&token));
}

#[tokio::test]
async fn brief_content_rights_guidance_and_archival_drift_block_producers_preserving_snapshots() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    for reason in ["content", "rights", "guidance", "archive"] {
        let (unit, c) = ready(&h, false).await;
        let path = format!("/content-units/{unit}/production-briefs");
        let submitted = brief_request(&c, 0, false);
        let first = h
            .json("POST", &path, Some(ALICE), Some(submitted.clone()))
            .await
            .success(StatusCode::CREATED);
        let brief = &first["brief"];
        let token = Uuid::new_v4().simple().to_string().repeat(2);
        h.json(
            "POST",
            &format!("{path}/1/producer-grants"),
            Some(ALICE),
            Some(grant_request(brief, now(&h).await + 60000, &token)),
        )
        .await
        .success(StatusCode::CREATED);
        match reason {
            "content" => {
                let mut append = content_request(1, 3, c["details"]["body"].clone());
                append["details"]["provenance_note"] =
                    json!("Corrected attribution, same effective text.");
                h.json(
                    "POST",
                    &format!("/content-units/{unit}/content/revisions"),
                    Some(ALICE),
                    Some(append),
                )
                .await
                .success(StatusCode::CREATED);
            }
            "rights" => {
                h.json("PATCH",&format!("/works/{}",brief["work"]["id"].as_str().unwrap()),Some(ALICE),Some(json!({"operation_id":operation(),"expected_revision":3,"details":{"rights_refs":["catalog-rights"]}}))).await.success(StatusCode::OK);
            }
            "guidance" => {
                h.json(
                    "POST",
                    &path,
                    Some(ALICE),
                    Some(brief_request(&c, 1, false)),
                )
                .await
                .success(StatusCode::CREATED);
            }
            "archive" => {
                h.json(
                    "DELETE",
                    &format!("/works/{}", brief["work"]["id"].as_str().unwrap()),
                    Some(ALICE),
                    Some(json!({"operation_id":operation(),"expected_revision":3})),
                )
                .await
                .success(StatusCode::OK);
            }
            _ => unreachable!(),
        }
        producer(h.app.clone(), &unit, 1, Some(&token))
            .await
            .error(StatusCode::CONFLICT, "stale_production_inputs");
        let stale = h
            .json("GET", &format!("{path}/1"), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
        assert_eq!(stale["brief"], *brief);
        assert_eq!(
            stale["freshness"],
            match reason {
                "content" => "content_changed",
                "rights" => "rights_changed",
                "guidance" => "guidance_changed",
                _ => "archived",
            }
        );
        assert_eq!(
            h.json("POST", &path, Some(ALICE), Some(submitted))
                .await
                .success(StatusCode::CREATED),
            stale
        );
    }
}

#[tokio::test]
async fn brief_source_and_script_export_pin_owning_contexts_and_preserve_originals() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let imported = h
        .store
        .import_manuscript(
            ALICE,
            cantos_server::contract::studio::ImportRequest {
                metadata: cantos_server::contract::studio::ImportMetadata {
                    operation_id: operation(),
                    file_name: "source.txt".into(),
                    format: cantos_server::contract::studio::ImportFormat::Txt,
                    reference: "Synthetic source for private instructions".into(),
                    rights_holder: None,
                    permission_evidence: None,
                    usage_scope: None,
                },
                original_bytes: ORIGINAL.as_bytes().to_vec(),
            },
        )
        .await
        .unwrap();
    let script_id = operation();
    let saved = h
        .store
        .save(
            ALICE,
            &script_id,
            SaveRevisionRequest {
                expected_revision: 0,
                operation_id: operation(),
                script_json: SCRIPT.into(),
            },
        )
        .await
        .unwrap();
    for variant in ["source", "script"] {
        let w = h.create(ALICE, "story").await;
        let cu = h.unit(&w, "Bản nguồn chính xác").await;
        let unit = cu["units"][0]["id"].as_str().unwrap();
        let body = if variant == "source" {
            json!({"kind":"source","source":{"id":imported.id,"sha256":imported.sha256}})
        } else {
            json!({"kind":"script","script":{"id":script_id,"revision":1,"export_digest":saved.export_digest},"source":null})
        };
        let c = h
            .json(
                "POST",
                &format!("/content-units/{unit}/content/revisions"),
                Some(ALICE),
                Some(content_request(0, 2, body)),
            )
            .await
            .success(StatusCode::CREATED)["content"]
            .clone();
        let mut request = brief_request(&c, 0, false);
        if variant == "script" {
            request["guidance"]["required_capabilities"] =
                json!(["narration", "script_ir", "ogg_opus_mono"]);
        }
        let b = h
            .json(
                "POST",
                &format!("/content-units/{unit}/production-briefs"),
                Some(ALICE),
                Some(request),
            )
            .await
            .success(StatusCode::CREATED)["brief"]
            .clone();
        if variant == "source" {
            assert_eq!(
                b["instructions"]["receipt"],
                serde_json::to_value(&imported).unwrap()
            );
            assert!(!b["instructions"]["receipt_digest"]
                .as_str()
                .unwrap()
                .is_empty());
        } else {
            assert_eq!(
                b["instructions"]["revision"],
                serde_json::to_value(&saved).unwrap()
            );
        }
        h.json(
            "GET",
            &format!("/content-units/{unit}/production-briefs/1"),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    }
    assert_eq!(
        h.store.original_import(ALICE, &imported.id).await.unwrap(),
        ORIGINAL.as_bytes()
    );
    assert_eq!(
        h.store.load(ALICE, &script_id, Some(1)).await.unwrap(),
        saved
    );
}

#[tokio::test]
async fn source_brief_refuses_legacy_failed_and_oversized_receipts_without_advancing_head() {
    let h = Harness::new().await;
    let _media_lane = native_sql::media_lane().await;
    h.admin.execute("INSERT INTO script_evidence VALUES('alice','source','legacy-brief','Synthetic legacy input')",&[]).await.unwrap();
    h.admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','legacy-brief','Synthetic preserved legacy source',?1,?2,0)",&[&ORIGINAL,&format!("{:x}",Sha256::digest(ORIGINAL.as_bytes()))]).await.unwrap();
    let mut sources = vec![(
        "legacy-brief".to_owned(),
        format!("{:x}", Sha256::digest(ORIGINAL.as_bytes())),
        StatusCode::UNPROCESSABLE_ENTITY,
        "evidence_unavailable",
    )];
    for (name, format, bytes) in [
        (
            "failed.docx",
            cantos_server::contract::studio::ImportFormat::Docx,
            b"invalid docx".to_vec(),
        ),
        (
            "large.txt",
            cantos_server::contract::studio::ImportFormat::Txt,
            "Đoạn thử nghiệm. ".repeat(12000).into_bytes(),
        ),
    ] {
        let receipt = h
            .store
            .import_manuscript(
                ALICE,
                cantos_server::contract::studio::ImportRequest {
                    metadata: cantos_server::contract::studio::ImportMetadata {
                        operation_id: operation(),
                        file_name: name.into(),
                        format,
                        reference: "Synthetic negative export fixture".into(),
                        rights_holder: None,
                        permission_evidence: None,
                        usage_scope: None,
                    },
                    original_bytes: bytes,
                },
            )
            .await
            .unwrap();
        sources.push((
            receipt.id,
            receipt.sha256,
            StatusCode::UNPROCESSABLE_ENTITY,
            "invalid_request",
        ));
    }
    for (id, sha256, status, code) in sources {
        let w = h.create(ALICE, "story").await;
        let cu = h.unit(&w, "Nguồn chưa thể bàn giao").await;
        let unit = cu["units"][0]["id"].as_str().unwrap();
        let c = h
            .json(
                "POST",
                &format!("/content-units/{unit}/content/revisions"),
                Some(ALICE),
                Some(content_request(
                    0,
                    2,
                    json!({"kind":"source","source":{"id":id,"sha256":sha256}}),
                )),
            )
            .await
            .success(StatusCode::CREATED)["content"]
            .clone();
        h.json(
            "POST",
            &format!("/content-units/{unit}/production-briefs"),
            Some(ALICE),
            Some(brief_request(&c, 0, false)),
        )
        .await
        .error(status, code);
        assert_eq!(
            h.admin
                .query_one(
                    "SELECT count(*) FROM production_briefs WHERE unit_id=?1",
                    &[&unit]
                )
                .await
                .unwrap()
                .get::<_, i64>(0),
            0
        );
    }
}
#[tokio::test]
async fn brief_atomic_rollback_concurrent_versions_and_sql_immutability() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    h.admin.execute("CREATE TRIGGER brief_fault BEFORE INSERT ON production_brief_heads BEGIN SELECT RAISE(ABORT,'synthetic fault'); END",&[]).await.unwrap();
    let intent = brief_request(&c, 0, false);
    h.json("POST", &path, Some(ALICE), Some(intent.clone()))
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    assert_eq!(
        h.admin
            .0
            .count("SELECT count(*) FROM production_briefs", &[])
            .await,
        0
    );
    assert_eq!(
        h.admin
            .0
            .count("SELECT count(*) FROM production_brief_heads", &[])
            .await,
        0
    );
    h.admin
        .execute("DROP TRIGGER brief_fault", &[])
        .await
        .unwrap();
    let blocker = h.admin.0.writer().await;
    let mut a = Box::pin(h.json("POST", &path, Some(ALICE), Some(intent.clone())));
    let mut b = Box::pin(h.json(
        "POST",
        &path,
        Some(ALICE),
        Some(brief_request(&c, 0, false)),
    ));
    assert!(futures_util::poll!(&mut a).is_pending());
    assert!(futures_util::poll!(&mut b).is_pending());
    blocker.commit().await.unwrap();
    let (a, b) = tokio::join!(a, b);
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::CREATED)
            .count(),
        1
    );
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1
    );
    for sql in [
        "UPDATE production_briefs SET digest=digest",
        "DELETE FROM production_briefs",
        "INSERT OR REPLACE INTO production_briefs SELECT * FROM production_briefs",
    ] {
        assert!(h
            .admin
            .0
            .try_exec(sql, &[])
            .await
            .unwrap_err()
            .guard()
            .is_some());
    }
    h.admin
        .execute("DROP TRIGGER production_briefs_no_update", &[])
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE production_briefs SET snapshot=?1",
            &[&b"{}".to_vec()],
        )
        .await
        .unwrap();
    h.json("GET", &format!("{path}/1"), Some(ALICE), None)
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
}
#[tokio::test]
async fn brief_read_baseline_uses_actual_authorized_export_and_indexed_pins() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    let start = Instant::now();
    let mut last = Value::Null;
    for version in 0..30 {
        last = h
            .json(
                "POST",
                &path,
                Some(ALICE),
                Some(brief_request(&c, version, false)),
            )
            .await
            .success(StatusCode::CREATED);
    }
    let writes_ms = start.elapsed().as_secs_f64() * 1000.;
    let token = "d".repeat(64);
    h.json(
        "POST",
        &format!("{path}/30/producer-grants"),
        Some(ALICE),
        Some(grant_request(&last["brief"], now(&h).await + 60000, &token)),
    )
    .await
    .success(StatusCode::CREATED);
    let mut samples = Vec::new();
    for i in 0..25 {
        let start = Instant::now();
        assert_eq!(
            producer(h.app.clone(), &unit, 30, Some(&token))
                .await
                .success(StatusCode::OK),
            last["brief"]
        );
        if i >= 5 {
            samples.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    let plan: Vec<String> = h
        .admin
        .query(
            "EXPLAIN QUERY PLAN SELECT * FROM production_briefs WHERE unit_id=?1 AND version=?2",
            &[&unit, &30_i64],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(3))
        .collect();
    assert!(
        plan.iter()
            .any(|p| p.contains("SEARCH") && p.contains("INDEX")),
        "{plan:?}"
    );
    samples.sort_by(f64::total_cmp);
    assert!(samples[18] < 500.);
    eprintln!("Turso 0.8.3 real producer export,30 versions,write_ms={writes_ms};5 warmup/20 sample ms={samples:?};plan={plan:?};local synthetic workload only");
}

#[tokio::test]
async fn producer_expiry_while_waiting_and_inactive_owner_are_checked_before_export() {
    let _media_lane = native_sql::media_lane().await;
    let clock = cantos_store_turso::testing::FixedClock::new(1_800_000_000_000_000);
    let h = Harness::with_fixture(NativeSql(
        cantos_server::test_support::ServerFixture::with_clock(clock.clone()).await,
    ))
    .await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    let brief = h
        .json(
            "POST",
            &path,
            Some(ALICE),
            Some(brief_request(&c, 0, false)),
        )
        .await
        .success(StatusCode::CREATED)["brief"]
        .clone();
    let token = "e".repeat(64);
    h.json(
        "POST",
        &format!("{path}/1/producer-grants"),
        Some(ALICE),
        Some(grant_request(&brief, now(&h).await + 200, &token)),
    )
    .await
    .success(StatusCode::CREATED);
    let blocker = h.admin.0.writer().await;
    // A pure query observes a pinned committed snapshot without waiting for the writer.
    tokio::time::timeout(
        Duration::from_secs(1),
        producer(h.app.clone(), &unit, 1, Some(&token)),
    )
    .await
    .unwrap()
    .success(StatusCode::OK);
    clock.advance(201_000);
    producer(h.app.clone(), &unit, 1, Some(&token))
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    blocker.commit().await.unwrap();
    let token = "f".repeat(64);
    h.json(
        "POST",
        &format!("{path}/1/producer-grants"),
        Some(ALICE),
        Some(grant_request(&brief, now(&h).await + 60000, &token)),
    )
    .await
    .success(StatusCode::CREATED);
    h.admin
        .execute("UPDATE actors SET active=0 WHERE id='alice'", &[])
        .await
        .unwrap();
    producer(h.app.clone(), &unit, 1, Some(&token))
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
}
