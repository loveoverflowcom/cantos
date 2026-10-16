//! Same-database catalog walks over real Studio writes and private Opus intake.
//! VerifiedDeliveryFake is test-only; these tests do not establish public media delivery.
use super::*;
use std::collections::BTreeSet;

struct CatalogWorld {
    h: Harness,
    _root: PrivateRoot,
    bytes: Vec<u8>,
}
struct CatalogWork {
    metadata: Value,
    units: Vec<CatalogUnit>,
}
struct CatalogUnit {
    id: String,
    submission: String,
    rendition: Value,
    candidate: Value,
    pointer_version: u64,
    release: Option<Value>,
}
impl CatalogWorld {
    async fn new() -> Self {
        let mut h = Harness::new().await;
        h.admin.batch_execute("GRANT SELECT,INSERT ON audio_submissions,audio_submission_revocations,audio_renditions,audio_reviews,audio_release_candidates,audio_distribution_clearances,audio_publication_approvals,audio_releases,audio_release_withdrawals,audio_publication_operations,audio_publication_events,audio_publication_pointers,audio_listener_works,audio_listener_units,audio_listener_releases TO cantos_app; GRANT UPDATE(active_release_id,version) ON audio_publication_pointers TO cantos_app; GRANT EXECUTE ON FUNCTION catalog_listener_discoverable(text,text),listener_actor_active(text) TO cantos_app;").await.unwrap();
        let root = PrivateRoot::new();
        let bytes = root.encode("24k");
        h.store = h.store.clone().with_audio_storage(&root.0).unwrap();
        h.app = router_with_publication_delivery(
            AppState {
                store: h.store.clone(),
                web_origin: ORIGIN.into(),
            },
            "unused-public-root",
            Arc::new(VerifiedDeliveryFake),
        );
        Self {
            h,
            _root: root,
            bytes,
        }
    }
    async fn work(&self, id: &str, kind: &str, title: &str, units: &[(&str, &str)]) -> CatalogWork {
        let mut request = work_request(kind);
        request["id"] = json!(id);
        request["details"]["title"] = json!(title);
        request["details"]["synopsis"] = json!("Tác phẩm tổng hợp để kiểm tra danh mục công khai.");
        request["details"]["visibility"] = json!("public");
        request["details"]["rights_refs"] = json!(["catalog-rights"]);
        let mut work = self
            .h
            .json("POST", "/works", Some(ALICE), Some(request))
            .await
            .success(StatusCode::CREATED)["work"]
            .clone();
        for (index, (unit, _)) in units.iter().enumerate() {
            let mut request = unit_request(&work, &format!("Tập thử nghiệm {}", index + 1));
            request["id"] = json!(unit);
            work = self
                .h
                .json(
                    "POST",
                    &format!("/works/{id}/content-units"),
                    Some(ALICE),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED)["work"]
                .clone();
        }
        let mut contents = Vec::new();
        // Content writes also advance the Work revision. Finish them all before making
        // candidates, so every initial candidate pins the final metadata revision.
        for (unit, language) in units {
            let mut request = content_request(
                0,
                work["revision"].as_u64().unwrap(),
                json!({"kind":"recording"}),
            );
            request["details"]["language"] = json!(language);
            let response = self
                .h
                .json(
                    "POST",
                    &format!("/content-units/{unit}/content/revisions"),
                    Some(ALICE),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED);
            work["revision"] = response["work_revision"].clone();
            contents.push(response["content"].clone());
        }
        let mut prepared = Vec::new();
        for ((unit, _), content) in units.iter().zip(contents) {
            let brief = self
                .h
                .json(
                    "POST",
                    &format!("/content-units/{unit}/production-briefs"),
                    Some(ALICE),
                    Some(brief_http::brief_request(&content, 0, true)),
                )
                .await
                .success(StatusCode::CREATED)["brief"]
                .clone();
            // Upload capabilities are unique within the shared database.
            let token = format!("{:x}", Sha256::digest(operation().as_bytes()));
            let request = submission_request(&self.h, unit, &brief, &self.bytes, &token).await;
            let submission = request["id"].as_str().unwrap().to_owned();
            self.h
                .json(
                    "POST",
                    &format!("/content-units/{unit}/audio-submissions"),
                    Some(ALICE),
                    Some(request),
                )
                .await
                .success(StatusCode::CREATED);
            let rendition = upload(self.h.app.clone(), &submission, &token, self.bytes.clone())
                .await
                .success(StatusCode::OK);
            let (candidate, _, _) = approve_candidate(
                &self.h,
                unit,
                &submission,
                &rendition,
                now(&self.h).await + 3_600_000,
            )
            .await;
            prepared.push(CatalogUnit {
                id: (*unit).into(),
                submission,
                rendition,
                candidate,
                pointer_version: 0,
                release: None,
            });
        }
        CatalogWork {
            metadata: work,
            units: prepared,
        }
    }
    async fn refresh_candidate(&self, unit: &mut CatalogUnit, valid_until: i64) {
        (unit.candidate, _, _) = approve_candidate(
            &self.h,
            &unit.id,
            &unit.submission,
            &unit.rendition,
            valid_until,
        )
        .await;
    }
    async fn publish(&self, unit: &mut CatalogUnit) -> Value {
        let active = unit
            .release
            .as_ref()
            .map_or(Value::Null, |r| r["id"].clone());
        let response = self.h.json("POST", &format!("/content-units/{}/release-candidates/{}/publish", unit.id, unit.candidate["id"].as_str().unwrap()), Some(ALICE), Some(json!({"operation_id":operation(),"candidate_digest":unit.candidate["digest"],"expected_pointer_version":unit.pointer_version,"expected_active_release_id":active}))).await.success(StatusCode::CREATED);
        unit.pointer_version = response["pointer_version"].as_u64().unwrap();
        unit.release = Some(response["release"].clone());
        response["release"].clone()
    }
    async fn withdraw(&self, unit: &mut CatalogUnit) {
        let response = self.h.json("POST", &format!("/content-units/{}/publication/withdraw", unit.id), Some(ALICE), Some(json!({"operation_id":operation(),"release_id":unit.release.as_ref().unwrap()["id"],"expected_pointer_version":unit.pointer_version,"reason":"Synthetic catalog walk withdrawal"}))).await.success(StatusCode::OK);
        unit.pointer_version = response["pointer_version"].as_u64().unwrap();
        unit.release = None;
    }
    async fn page(&self, path: &str) -> Value {
        let (status, page) = theatre_call(self.h.app.clone(), "GET", path).await;
        assert_eq!(status, StatusCode::OK, "{page}");
        page
    }
    async fn invalid_cursor(&self, path: &str) {
        let (status, error) = theatre_call(self.h.app.clone(), "GET", path).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{error}");
        assert_eq!(error, json!({"code":"invalid_cursor","retryable":false}));
    }
    async fn walk(&self, base: &str, first: Value) -> Vec<Value> {
        let mut page = first;
        let mut rows = Vec::new();
        for _ in 0..16 {
            rows.extend(page["items"].as_array().unwrap().iter().cloned());
            let Some(cursor) = page["next_cursor"].as_str() else {
                return rows;
            };
            page = self.page(&format!("{base}&cursor={cursor}")).await;
        }
        panic!("bounded synthetic catalog walk did not terminate");
    }
}
fn ids(rows: &[Value], field: &str) -> Vec<String> {
    rows.iter()
        .map(|r| r[field].as_str().unwrap().to_owned())
        .collect()
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn theatre_units_walk_survives_mid_walk_withdraw_publish_and_republish() {
    let world = CatalogWorld::new().await;
    let wid = "11000000-0000-4000-8000-000000000001";
    let unit_ids = [
        "10000000-0000-4000-8000-000000000001",
        "20000000-0000-4000-8000-000000000002",
        "30000000-0000-4000-8000-000000000003",
        "40000000-0000-4000-8000-000000000004",
    ];
    let mut work = world
        .work(
            wid,
            "podcast",
            "Đường về qua bốn tập",
            &unit_ids.map(|id| (id, "vi-VN")),
        )
        .await;
    for unit in &mut work.units[..3] {
        world.publish(unit).await;
    }
    let base = format!("/works/{wid}/units?limit=1");
    let first = world.page(&base).await;
    assert_eq!(first["items"][0]["unit_id"], unit_ids[0]);
    assert_eq!(first["items"][0]["position"], 0);
    let original_publication = first["items"][0]["publication_id"].clone();
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    world.withdraw(&mut work.units[1]).await;
    let reordered = world.h.json("POST", &format!("/works/{wid}/content-units/reorder"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":work.metadata["revision"],"ordered":[unit_ids[3],unit_ids[2],unit_ids[1],unit_ids[0]]}))).await.success(StatusCode::OK);
    assert_eq!(
        reordered["work"]["revision"],
        work.metadata["revision"].as_u64().unwrap() + 1
    );
    // Reorder acknowledges the Work revision; its response does not enumerate units.
    // Read the persisted Studio position before comparing it with the public anchor.
    let studio_unit = world
        .h
        .json(
            "GET",
            &format!("/content-units/{}", unit_ids[0]),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(studio_unit["position"], 3);
    for index in [0, 3] {
        world
            .refresh_candidate(&mut work.units[index], now(&world.h).await + 3_600_000)
            .await;
        world.publish(&mut work.units[index]).await;
    }
    let fresh = world.page(&format!("/works/{wid}/units?limit=10")).await;
    assert_eq!(
        ids(fresh["items"].as_array().unwrap(), "unit_id"),
        [unit_ids[0], unit_ids[3], unit_ids[2]]
    );
    assert_eq!(
        fresh["items"][0]["position"], 0,
        "republish must retain the first index position"
    );
    assert_eq!(
        fresh["items"][1]["position"], 0,
        "newly indexed unit takes its current Studio position"
    );
    assert_ne!(fresh["items"][0]["publication_id"], original_publication);
    assert_eq!(
        fresh["items"][0]["publication_id"],
        work.units[0].release.as_ref().unwrap()["id"]
    );
    assert_eq!(fresh["items"][0]["unit_id"], first["items"][0]["unit_id"]);
    let continuation = format!("{base}&cursor={cursor}");
    let second = world.page(&continuation).await;
    let retried = world.page(&continuation).await;
    assert_eq!(second["items"], retried["items"]);
    assert_eq!(
        second["items"][0]["unit_id"], unit_ids[3],
        "equal positions use UUID text C order"
    );
    assert!(second["next_cursor"].is_string());
    let tail = world.walk(&base, second).await;
    assert_eq!(ids(&tail, "unit_id"), [unit_ids[3], unit_ids[2]]);
    assert_eq!(
        ids(&world.walk(&base, retried).await, "unit_id"),
        ids(&tail, "unit_id")
    );
    let mut walked = ids(first["items"].as_array().unwrap(), "unit_id");
    walked.extend(ids(&tail, "unit_id"));
    assert_eq!(walked.iter().collect::<BTreeSet<_>>().len(), walked.len());
    assert!(!walked.iter().any(|id| id == unit_ids[1]));
    for path in [
        format!("/works?limit=1&cursor={cursor}"),
        format!("/works/{wid}/units?limit=2&cursor={cursor}"),
        format!("/works/12000000-0000-4000-8000-000000000001/units?limit=1&cursor={cursor}"),
    ] {
        world.invalid_cursor(&path).await;
    }
    // Failed uses do not consume or invalidate the original continuation.
    assert_eq!(
        world.page(&continuation).await["items"],
        json!(fresh["items"].as_array().unwrap()[1..2])
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn theatre_works_walk_rechecks_visibility_and_binds_filter_cursors() {
    let world = CatalogWorld::new().await;
    let mut works = Vec::new();
    for index in 1..=4 {
        let wid = if index == 4 {
            "01000000-0000-4000-8000-000000000001".into()
        } else {
            format!("{index}2000000-0000-4000-8000-000000000001")
        };
        let uid = format!("{index}3000000-0000-4000-8000-000000000001");
        works.push(
            world
                .work(
                    &wid,
                    "podcast",
                    &format!("Ánh trăng tập hợp {index}"),
                    &[(&uid, "vi-VN")],
                )
                .await,
        );
    }
    for work in &mut works[..3] {
        world.publish(&mut work.units[0]).await;
    }
    let initial = world.page("/works?limit=10").await;
    let expected = ids(initial["items"].as_array().unwrap(), "work_id");
    assert_eq!(expected.len(), 3);
    let first = world.page("/works?limit=1").await;
    assert_eq!(first["items"][0]["work_id"], expected[0]);
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    let seen = works
        .iter()
        .position(|w| w.metadata["id"] == expected[0])
        .unwrap();
    let withdrawn = works
        .iter()
        .position(|w| w.metadata["id"] == expected[1])
        .unwrap();
    world.withdraw(&mut works[withdrawn].units[0]).await;
    world.publish(&mut works[seen].units[0]).await;
    world.publish(&mut works[3].units[0]).await;
    let fresh = world.page("/works?limit=10").await;
    assert_eq!(fresh["items"][0]["work_id"], works[3].metadata["id"]);
    let republished = fresh["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|w| w["work_id"] == expected[0])
        .unwrap();
    assert_eq!(
        republished["first_published_at_ms"],
        first["items"][0]["first_published_at_ms"]
    );
    let path = format!("/works?limit=1&cursor={cursor}");
    let second = world.page(&path).await;
    assert_eq!(second["items"], world.page(&path).await["items"]);
    let rows = world.walk("/works?limit=1", second).await;
    assert_eq!(ids(&rows, "work_id"), expected[2..]);
    assert!(rows.iter().all(|w| w["work_id"] != works[3].metadata["id"]));
    for path in [
        format!("/works?limit=2&cursor={cursor}"),
        format!("/works?limit=1&kind=story&cursor={cursor}"),
        format!("/works?limit=1&language=en-US&cursor={cursor}"),
        format!("/search?limit=1&q=changed&cursor={cursor}"),
        format!("/works/{}/units?limit=1&cursor={cursor}", expected[0]),
    ] {
        world.invalid_cursor(&path).await;
    }
    assert_eq!(
        world.page(&format!("/works?limit=1&cursor={cursor}")).await["items"],
        json!(rows)
    );
    let filter = "/search?q=%C3%81NH&kind=podcast&language=vi-VN&limit=1";
    let filtered = world.page(filter).await;
    assert!(filtered["next_cursor"].is_string());
    let token = filtered["next_cursor"].as_str().unwrap();
    let filtered_path = format!("{filter}&cursor={token}");
    let continuation = world.page(&filtered_path).await;
    assert_eq!(
        continuation["items"],
        world.page(&filtered_path).await["items"]
    );
    for path in [
        format!("/search?q=other&kind=podcast&language=vi-VN&limit=1&cursor={token}"),
        format!("/search?q=%C3%81NH&kind=story&language=vi-VN&limit=1&cursor={token}"),
        format!("/search?q=%C3%81NH&kind=podcast&language=en-US&limit=1&cursor={token}"),
    ] {
        world.invalid_cursor(&path).await;
    }
    let filtered_rows = world.walk(filter, filtered).await;
    assert_eq!(
        ids(&filtered_rows, "work_id"),
        ids(fresh["items"].as_array().unwrap(), "work_id")
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn theatre_work_filters_follow_the_latest_eligible_representative() {
    let world = CatalogWorld::new().await;
    let wid = "51000000-0000-4000-8000-000000000001";
    let mut work = world
        .work(
            wid,
            "story",
            "Dấu xưa trong mưa",
            &[
                ("52000000-0000-4000-8000-000000000001", "vi-VN"),
                ("53000000-0000-4000-8000-000000000001", "en-US"),
            ],
        )
        .await;
    world.publish(&mut work.units[0]).await;
    work.metadata = world.h.json("PATCH", &format!("/works/{wid}"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":work.metadata["revision"],"details":{"title":"Bình minh mới"}}))).await.success(StatusCode::OK)["work"].clone();
    world
        .refresh_candidate(&mut work.units[1], now(&world.h).await + 3_600_000)
        .await;
    let newer = world.publish(&mut work.units[1]).await;
    assert!(
        newer["published_at"].as_i64().unwrap()
            >= work.units[0].release.as_ref().unwrap()["published_at"]
                .as_i64()
                .unwrap()
    );
    let representative = world.page(&format!("/works/{wid}")).await;
    assert_eq!(representative["title"], "Bình minh mới");
    assert_eq!(representative["language"], "en-US");
    for query in [
        "/search?q=D%E1%BA%A5u&kind=story&language=vi-VN",
        "/works?language=vi-VN",
        "/works?kind=podcast&language=en-US",
    ] {
        assert_eq!(world.page(query).await["items"], json!([]), "{query}");
    }
    assert_eq!(
        world
            .page("/search?q=B%C3%ACnh&kind=story&language=en-US")
            .await["items"][0]["work_id"],
        wid
    );
    world.withdraw(&mut work.units[1]).await;
    let fallback = world
        .page("/search?q=D%E1%BA%A5u&kind=story&language=vi-VN")
        .await;
    assert_eq!(fallback["items"][0]["work_id"], wid);
    assert_eq!(fallback["items"][0]["title"], "Dấu xưa trong mưa");
    assert_eq!(
        world.page("/works?language=en-US").await["items"],
        json!([])
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn theatre_units_walk_rechecks_rights_expiry_on_the_database_clock() {
    let world = CatalogWorld::new().await;
    let wid = "61000000-0000-4000-8000-000000000001";
    let mut work = world
        .work(
            wid,
            "podcast",
            "Quyền có thời hạn",
            &[
                ("62000000-0000-4000-8000-000000000001", "vi-VN"),
                ("63000000-0000-4000-8000-000000000001", "vi-VN"),
                ("64000000-0000-4000-8000-000000000001", "vi-VN"),
            ],
        )
        .await;
    world.publish(&mut work.units[0]).await;
    world.publish(&mut work.units[2]).await;
    // Expensive media setup is complete before the short rights window begins.
    let expiry = now(&world.h).await + 3_000;
    world.refresh_candidate(&mut work.units[1], expiry).await;
    let expiring = world.publish(&mut work.units[1]).await;
    let publication = expiring["id"].as_str().unwrap();
    assert!(
        now(&world.h).await < expiry,
        "fixture must observe the release before expiry"
    );
    let before = world.page(&format!("/works/{wid}/units?limit=10")).await;
    assert_eq!(before["items"].as_array().unwrap().len(), 3);
    let first = world.page(&format!("/works/{wid}/units?limit=1")).await;
    let cursor = first["next_cursor"].as_str().unwrap();
    let deadline = Instant::now() + Duration::from_secs(6);
    while now(&world.h).await < expiry {
        assert!(
            Instant::now() < deadline,
            "database clock did not reach the rights deadline"
        );
        tokio::time::sleep(Duration::from_millis(25)).await;
    }
    let tail = world
        .page(&format!("/works/{wid}/units?limit=1&cursor={cursor}"))
        .await;
    assert_eq!(
        ids(tail["items"].as_array().unwrap(), "unit_id"),
        [work.units[2].id.clone()]
    );
    assert_eq!(tail["next_cursor"], Value::Null);
    let (status, error) = theatre_call(
        world.h.app.clone(),
        "GET",
        &format!("/publications/{publication}"),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(error, json!({"code":"not_found","retryable":false}));
}
