//! Same-database catalog walks over real Studio writes and private Opus intake.
//! VerifiedDeliveryFake is test-only; these tests do not establish public media delivery.
use super::*;

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
    candidate: Value,
    pointer_version: u64,
    release: Option<Value>,
}
impl CatalogWorld {
    async fn new() -> Self {
        let mut h = Harness::new().await;

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
async fn theatre_units_walk_pins_selection_order_and_restarts_on_a_new_generation() {
    let _media_lane = native_sql::media_lane().await;
    let world = CatalogWorld::new().await;
    let wid = "11000000-0000-4000-8000-000000000001";
    let unit_ids = [
        "10000000-0000-4000-8000-000000000001",
        "20000000-0000-4000-8000-000000000002",
        "30000000-0000-4000-8000-000000000003",
    ];
    let mut work = world
        .work(
            wid,
            "podcast",
            "Đường về qua ba tập",
            &unit_ids.map(|id| (id, "vi-VN")),
        )
        .await;
    // Even an approved active release is private until the owner publishes metadata.
    let release = world.publish(&mut work.units[0]).await;
    assert_eq!(world.page("/works").await["items"], json!([]));
    let selected = publish_metadata(&world.h, wid, &[unit_ids[1], unit_ids[0]]).await;
    let base = format!("/works/{wid}/units?limit=1");
    let first = world.page(&base).await;
    let cursor = first["next_cursor"].as_str().unwrap();
    let rows = world.walk(&base, first.clone()).await;
    assert_eq!(ids(&rows, "unit_id"), [unit_ids[1], unit_ids[0]]);
    assert_eq!(rows[0]["ordinal"], 1);
    assert_eq!(rows[1]["ordinal"], 2);
    assert_eq!(rows[0]["playback"], Value::Null);
    assert_eq!(rows[1]["playback"]["publication_id"], release["id"]);
    assert_eq!(world.page(&format!("/works/{wid}")).await["unit_count"], 2);
    // A Catalog reorder/head edit changes neither selected membership nor its published positions.
    world.h.json("POST", &format!("/works/{wid}/content-units/reorder"), Some(ALICE), Some(json!({"operation_id":operation(),"expected_revision":work.metadata["revision"],"ordered":[unit_ids[2],unit_ids[0],unit_ids[1]]}))).await.success(StatusCode::OK);
    assert_eq!(world.walk(&base, world.page(&base).await).await, rows);
    // Explicit generation replacement changes the walk; a cursor cannot silently cross it.
    let replacement = publish_metadata(&world.h, wid, &[unit_ids[0], unit_ids[1]]).await;
    assert_eq!(
        replacement["version"],
        selected["version"].as_u64().unwrap() + 1
    );
    world
        .invalid_cursor(&format!("{base}&cursor={cursor}"))
        .await;
    assert_eq!(
        ids(&world.walk(&base, world.page(&base).await).await, "unit_id"),
        [unit_ids[0], unit_ids[1]]
    );
    world.withdraw(&mut work.units[0]).await;
    let rows = world.page(&format!("/works/{wid}/units")).await;
    assert_eq!(rows["items"].as_array().unwrap().len(), 2);
    assert_eq!(rows["items"][0]["playback"], Value::Null);
    withdraw_metadata(&world.h, wid).await;
    assert_eq!(world.page("/works").await["items"], json!([]));
    assert_eq!(
        theatre_call(world.h.app.clone(), "GET", &format!("/works/{wid}/units"))
            .await
            .0,
        StatusCode::NOT_FOUND
    );
}

#[tokio::test]
async fn theatre_works_walk_binds_filters_and_uses_pinned_work_not_release_metadata() {
    let _media_lane = native_sql::media_lane().await;
    let world = CatalogWorld::new().await;
    let a = world
        .work(
            "11000000-0000-4000-8000-000000000001",
            "podcast",
            "Ánh đèn thứ nhất",
            &[("10000000-0000-4000-8000-000000000001", "vi-VN")],
        )
        .await;
    let b = world
        .work(
            "22000000-0000-4000-8000-000000000002",
            "podcast",
            "Ánh đèn thứ hai",
            &[("20000000-0000-4000-8000-000000000002", "en")],
        )
        .await;
    world
        .work(
            "33000000-0000-4000-8000-000000000003",
            "podcast",
            "Bản nháp chưa công bố",
            &[("30000000-0000-4000-8000-000000000003", "vi-VN")],
        )
        .await;
    for w in [&a, &b] {
        publish_metadata(
            &world.h,
            w.metadata["id"].as_str().unwrap(),
            &[&w.units[0].id],
        )
        .await;
    }
    let base = "/works?limit=1&kind=podcast&language=vi-VN";
    let first = world.page(base).await;
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    assert_eq!(
        world.walk(base, first).await.len(),
        2,
        "language is pinned editorial Work metadata, not audio's language"
    );
    world
        .invalid_cursor(&format!(
            "/works?limit=1&kind=novel&language=vi-VN&cursor={cursor}"
        ))
        .await;
    let aid = a.metadata["id"].as_str().unwrap();
    world.h.json("PATCH",&format!("/works/{aid}"),Some(ALICE),Some(json!({"operation_id":operation(),"expected_revision":a.metadata["revision"],"details":{"title":"Bản nháp mới","visibility":"private","language":"en"}}))).await.success(StatusCode::OK);
    assert_eq!(
        world.page(&format!("/works/{aid}")).await["title"],
        "Ánh đèn thứ nhất"
    );
    assert_eq!(
        world.page("/search?q=%C3%81nh%20%C4%91%C3%A8n").await["items"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        world
            .page("/search?q=B%E1%BA%A3n%20nh%C3%A1p%20m%E1%BB%9Bi")
            .await["items"],
        json!([])
    );
    withdraw_metadata(&world.h, aid).await;
    assert_eq!(world.walk(base, world.page(base).await).await.len(), 1);
}

#[tokio::test]
async fn theatre_audio_eligibility_changes_do_not_remove_published_metadata() {
    let _media_lane = native_sql::media_lane().await;
    let world = CatalogWorld::new().await;
    let wid = "11000000-0000-4000-8000-000000000001";
    let mut work = world
        .work(
            wid,
            "podcast",
            "Quyền nghe được kiểm tra lại",
            &[("10000000-0000-4000-8000-000000000001", "vi-VN")],
        )
        .await;
    let release = world.publish(&mut work.units[0]).await;
    publish_metadata(&world.h, wid, &[&work.units[0].id]).await;
    assert_eq!(
        world.page(&format!("/works/{wid}/units")).await["items"][0]["playback"]["publication_id"],
        release["id"]
    );
    // Revoking the exact candidate's clearance invalidates audio while retaining metadata.
    let candidate = &work.units[0].candidate;
    world.h.json("POST",&format!("/content-units/{}/release-candidates/{}/distribution-rights",work.units[0].id,candidate["id"].as_str().unwrap()),Some(ALICE),Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"decision":"revoked","rights_refs":["catalog-rights"],"valid_until":now(&world.h).await+3_600_000,"evidence_refs":["catalog-rights"],"notes":"Synthetic revocation of the pinned candidate"}))).await.success(StatusCode::CREATED);
    let units = world.page(&format!("/works/{wid}/units")).await;
    assert_eq!(units["items"].as_array().unwrap().len(), 1);
    assert_eq!(units["items"][0]["playback"], Value::Null);
    assert_eq!(world.page(&format!("/works/{wid}")).await["unit_count"], 1);
    assert_eq!(
        theatre_call(
            world.h.app.clone(),
            "GET",
            &format!("/publications/{}", release["id"].as_str().unwrap())
        )
        .await
        .0,
        StatusCode::NOT_FOUND
    );
}
