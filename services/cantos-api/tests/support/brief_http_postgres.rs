//! The parent target's disposable cluster/harness exercises actual Router and app-role SQL.
use super::*;

pub(super) fn brief_request(content: &Value, version: u64, recording: bool) -> Value {
    let mut value: Value = serde_json::from_str(if recording {
        include_str!("../../../../packages/contracts/fixtures/briefs/v1/create-recording.json")
    } else {
        include_str!("../../../../packages/contracts/fixtures/briefs/v1/create-narration.json")
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
    h.admin
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp())*1000)::bigint",
            &[],
        )
        .await
        .unwrap()
        .get(0)
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn brief_text_recording_profiles_replay_and_conflicting_creation_are_real_http() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn producer_scope_studio_cookie_capabilities_revocation_and_owner_isolation() {
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
        .query_one(
            "SELECT encode(snapshot,'escape') FROM production_briefs",
            &[]
        )
        .await
        .unwrap()
        .get::<_, String>(0)
        .contains(&token));
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn brief_content_rights_guidance_and_archival_drift_block_producers_preserving_snapshots() {
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn brief_source_and_script_export_pin_owning_contexts_and_preserve_originals() {
    let h = Harness::new().await;
    let imported = h
        .store
        .import_manuscript(
            ALICE,
            cantos_contracts::ImportRequest {
                metadata: cantos_contracts::ImportMetadata {
                    operation_id: operation(),
                    file_name: "source.txt".into(),
                    format: cantos_contracts::ImportFormat::Txt,
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn source_brief_refuses_legacy_failed_and_oversized_receipts_without_advancing_head() {
    let h = Harness::new().await;
    cantos_api::platform::operator::record_source(
        &h.config,
        "alice",
        "legacy-brief",
        "Synthetic preserved legacy source",
        ORIGINAL,
    )
    .await
    .unwrap();
    let mut sources = vec![(
        "legacy-brief".to_owned(),
        format!("{:x}", Sha256::digest(ORIGINAL.as_bytes())),
        StatusCode::UNPROCESSABLE_ENTITY,
        "evidence_unavailable",
    )];
    for (name, format, bytes) in [
        (
            "failed.docx",
            cantos_contracts::ImportFormat::Docx,
            b"invalid docx".to_vec(),
        ),
        (
            "large.txt",
            cantos_contracts::ImportFormat::Txt,
            "Đoạn thử nghiệm. ".repeat(12000).into_bytes(),
        ),
    ] {
        let receipt = h
            .store
            .import_manuscript(
                ALICE,
                cantos_contracts::ImportRequest {
                    metadata: cantos_contracts::ImportMetadata {
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
                    "SELECT count(*) FROM production_briefs WHERE unit_id=$1",
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn brief_atomic_rollback_concurrent_versions_and_sql_immutability() {
    let mut h = Harness::new().await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    h.admin.batch_execute("CREATE FUNCTION brief_fault() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'injected before brief head'; END; $$; CREATE TRIGGER brief_fault BEFORE INSERT ON production_brief_heads FOR EACH ROW EXECUTE FUNCTION brief_fault()").await.unwrap();
    let request = brief_request(&c, 0, false);
    h.json("POST", &path, Some(ALICE), Some(request.clone()))
        .await
        .error(StatusCode::SERVICE_UNAVAILABLE, "unavailable");
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM production_briefs", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    h.admin
        .batch_execute(
            "DROP TRIGGER brief_fault ON production_brief_heads; DROP FUNCTION brief_fault()",
        )
        .await
        .unwrap();
    let tx = h.admin.transaction().await.unwrap();
    tx.query_one("SELECT revision FROM catalog_clock FOR UPDATE", &[])
        .await
        .unwrap();
    let mut tasks = Vec::new();
    for body in [request, brief_request(&c, 0, false)] {
        let app = h.app.clone();
        let p = path.clone();
        tasks.push(tokio::spawn(async move {
            super::request(
                app,
                "POST",
                &p,
                Some(ALICE),
                serde_json::to_vec(&body).unwrap(),
                ("application/json", "application/json", ORIGIN),
            )
            .await
        }));
    }
    tx.commit().await.unwrap();
    let replies = [
        tasks.remove(0).await.unwrap(),
        tasks.remove(0).await.unwrap(),
    ];
    assert_eq!(
        replies
            .iter()
            .filter(|r| r.status == StatusCode::CREATED)
            .count(),
        1
    );
    assert_eq!(
        replies
            .iter()
            .filter(|r| r.status == StatusCode::CONFLICT)
            .count(),
        1
    );
    for sql in ["UPDATE production_briefs SET digest='pb-r1:sha256:0000000000000000000000000000000000000000000000000000000000000000'","DELETE FROM production_briefs","TRUNCATE production_briefs CASCADE"] {assert_eq!(h.admin.batch_execute(sql).await.unwrap_err().code(),Some(&tokio_postgres::error::SqlState::OBJECT_NOT_IN_PREREQUISITE_STATE));}
    h.admin.batch_execute("ALTER TABLE production_briefs DISABLE TRIGGER production_briefs_immutable; UPDATE production_briefs SET snapshot=convert_to('{}','UTF8'); ALTER TABLE production_briefs ENABLE TRIGGER production_briefs_immutable").await.unwrap();
    h.json("GET", &format!("{path}/1"), Some(ALICE), None)
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn producer_expiry_while_waiting_and_inactive_owner_are_checked_before_export() {
    let mut h = Harness::new().await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    let b = h
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
        Some(grant_request(&b, now(&h).await + 200, &token)),
    )
    .await
    .success(StatusCode::CREATED);
    let lock = h.admin.transaction().await.unwrap();
    lock.query_one("SELECT revision FROM catalog_clock FOR UPDATE", &[])
        .await
        .unwrap();
    let app = h.app.clone();
    let uid = unit.clone();
    let bearer = token.clone();
    let waiting = tokio::spawn(async move { producer(app, &uid, 1, Some(&bearer)).await });
    tokio::time::sleep(Duration::from_millis(250)).await;
    lock.commit().await.unwrap();
    waiting
        .await
        .unwrap()
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    let token = "f".repeat(64);
    h.json(
        "POST",
        &format!("{path}/1/producer-grants"),
        Some(ALICE),
        Some(grant_request(&b, now(&h).await + 60000, &token)),
    )
    .await
    .success(StatusCode::CREATED);
    h.admin
        .batch_execute("UPDATE actors SET active=false WHERE id='alice'")
        .await
        .unwrap();
    producer(h.app.clone(), &unit, 1, Some(&token))
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn brief_read_baseline_uses_actual_authorized_export_and_indexed_pins() {
    let h = Harness::new().await;
    let (unit, c) = ready(&h, false).await;
    let path = format!("/content-units/{unit}/production-briefs");
    let mut last = Value::Null;
    let start = Instant::now();
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
    let write_ms = start.elapsed().as_secs_f64() * 1000.;
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
    for _ in 0..20 {
        let start = Instant::now();
        assert_eq!(
            producer(h.app.clone(), &unit, 30, Some(&token))
                .await
                .success(StatusCode::OK),
            last["brief"]
        );
        samples.push(start.elapsed().as_secs_f64() * 1000.);
    }
    let plan:Vec<String>=h.admin.query("EXPLAIN (ANALYZE,BUFFERS) SELECT * FROM production_briefs WHERE unit_id=$1 AND version=$2",&[&unit,&30_i64]).await.unwrap().iter().map(|row|row.get(0)).collect();
    fs::create_dir_all(EVIDENCE).unwrap();
    fs::write(format!("{EVIDENCE}/brief-read-baseline.json"),serde_json::to_vec_pretty(&json!({"versions":30,"samples":20,"warm_export_ms":samples,"write_total_ms":write_ms,"pinned_plan":plan,"scope":"Synthetic warm local Router/application/app-role PostgreSQL; no production SLO or DBSP gain claimed"})).unwrap()).unwrap();
}
