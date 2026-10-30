//! Studio preview (ADR 0054 D03/D01b, #650) through the real Studio router, application and a
//! run-owned native Turso file. Synthetic Vietnamese metadata; no mocked store.
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
    Router,
};
use cantos_server::contexts::catalog::{
    domain::{OwnerId, WorkId},
    CatalogCommand, MemberRole,
};
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    platform::store::Store,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

#[path = "support/native_sql.rs"]
mod native_sql;
use native_sql::NativeSql;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CAROL: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const ORIGIN: &str = "http://127.0.0.1:8080";

struct Harness {
    admin: NativeSql,
    store: Store,
    app: Router,
}

struct Reply {
    status: StatusCode,
    headers: HeaderMap,
    body: Value,
}

impl Reply {
    fn ok(self) -> Value {
        assert_eq!(self.status, StatusCode::OK, "{}", self.body);
        schema("StudioPreview", &self.body);
        self.body
    }

    fn refused(self, status: StatusCode, code: &str) -> Value {
        assert_eq!(self.status, status, "{}", self.body);
        assert_eq!(self.body["code"], code, "{}", self.body);
        self.body
    }
}

fn operation() -> String {
    Uuid::new_v4().to_string()
}

fn schema(definition: &str, body: &Value) {
    let root: Value = serde_json::from_str(include_str!(
        "../contracts/studio/preview/v1/preview.schema.json"
    ))
    .unwrap();
    let selected = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "$defs": root["$defs"], "$ref": format!("#/$defs/{definition}")
    });
    let errors: Vec<_> = jsonschema::validator_for(&selected)
        .unwrap()
        .iter_errors(body)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "{definition}: {errors:?}; {body}");
}

impl Harness {
    async fn new() -> Self {
        let admin = NativeSql::new().await;
        for (actor, token) in [("alice", ALICE), ("bob", BOB), ("carol", CAROL)] {
            admin.0.studio_caller(actor, token).await;
        }
        let store = admin.store();
        let app = router(
            AppState {
                store: store.clone(),
                web_origin: ORIGIN.into(),
            },
            "apps/cantos-studio/web/dist",
        );
        Self { admin, store, app }
    }

    async fn send(
        &self,
        method: &str,
        path: &str,
        token: Option<&str>,
        body: Option<Value>,
    ) -> Reply {
        let mut builder = Request::builder()
            .method(method)
            .uri(format!("/api/v1/studio{path}"))
            .header("origin", ORIGIN)
            .header("content-type", "application/json")
            .header("accept", "application/json");
        if let Some(token) = token {
            builder = builder.header("cookie", format!("cantos_session={token}"));
        }
        let body = body.map_or_else(Vec::new, |body| serde_json::to_vec(&body).unwrap());
        let response = self
            .app
            .clone()
            .oneshot(builder.body(Body::from(body)).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        Reply {
            status,
            headers,
            body: serde_json::from_slice(&bytes).unwrap(),
        }
    }

    async fn preview(&self, token: Option<&str>, work: &str, query: &str) -> Reply {
        self.send(
            "GET",
            &format!("/works/{work}/preview?{query}"),
            token,
            None,
        )
        .await
    }

    async fn create_work(&self, kind: &str) -> Value {
        self.send(
            "POST",
            "/works",
            Some(ALICE),
            Some(json!({
                "operation_id": operation(), "id": operation(), "kind": kind,
                "details": {
                    "title": "Ánh đèn cuối sân khấu",
                    "synopsis": "Một lời hẹn dưới mái hiên nhà hát cũ.",
                    "language": "vi-VN", "visibility": "public",
                    "rights_refs": []
                }
            })),
        )
        .await
        .created()["work"]
            .clone()
    }

    /// Adds a Unit against the Work's current revision and returns the whole outcome.
    async fn unit(&self, work: &Value, title: &str) -> Value {
        self.send(
            "POST",
            &format!("/works/{}/content-units", work["id"].as_str().unwrap()),
            Some(ALICE),
            Some(json!({
                "operation_id": operation(), "id": operation(),
                "kind": if work["kind"] == "podcast" { "episode" } else { "chapter" },
                "expected_work_revision": work["revision"],
                "details": {"title": title, "description": format!("Mô tả — {title}")}
            })),
        )
        .await
        .created()
    }

    async fn units(&self, work: &mut Value, count: usize) -> Vec<Value> {
        let mut units = Vec::new();
        for index in 1..=count {
            let outcome = self.unit(work, &format!("Chương {index}")).await;
            *work = outcome["work"].clone();
            units.push(outcome["units"][0].clone());
        }
        units
    }

    async fn set_member(&self, work: &Value, role: Option<MemberRole>) -> Value {
        let outcome = self
            .store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::SetMember {
                    work_id: WorkId::parse(work["id"].as_str().unwrap()).unwrap(),
                    expected_revision: work["revision"].as_u64().unwrap(),
                    actor_id: OwnerId::parse("bob").unwrap(),
                    role,
                },
            )
            .await
            .unwrap();
        serde_json::to_value(outcome.work).unwrap()
    }

    /// Every table a preview must leave untouched: Catalog, its clock and receipts, and the
    /// publication generations, pointers and events.
    async fn footprint(&self) -> Value {
        let text: String = self
            .admin
            .query_one(
                "SELECT json_object(\
                 'works',(SELECT count(*) FROM catalog_works),\
                 'work_revisions',(SELECT count(*) FROM catalog_work_revisions),\
                 'units',(SELECT count(*) FROM catalog_units),\
                 'unit_revisions',(SELECT count(*) FROM catalog_unit_revisions),\
                 'members',(SELECT count(*) FROM catalog_members),\
                 'operations',(SELECT count(*) FROM catalog_operations),\
                 'clock',(SELECT revision FROM catalog_clock),\
                 'generations',(SELECT count(*) FROM catalog_metadata_generations),\
                 'entries',(SELECT count(*) FROM catalog_metadata_generation_entries),\
                 'events',(SELECT count(*) FROM catalog_metadata_publication_events),\
                 'pointers',(SELECT count(*) FROM catalog_metadata_publication_pointers))",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        serde_json::from_str(&text).unwrap()
    }
}

trait Created {
    fn created(self) -> Value;
}
impl Created for Reply {
    fn created(self) -> Value {
        assert_eq!(self.status, StatusCode::CREATED, "{}", self.body);
        self.body
    }
}

fn id(value: &Value) -> &str {
    value["id"].as_str().unwrap()
}

#[tokio::test]
async fn the_owner_previews_exactly_the_saved_head_with_the_narrow_dto_and_private_headers() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("novel").await;
    let units = h.units(&mut work, 2).await;
    let revision = work["revision"].as_u64().unwrap();
    let reply = h
        .preview(Some(ALICE), id(&work), &format!("work_revision={revision}"))
        .await;
    assert_eq!(reply.headers["cache-control"], "no-store");
    assert_eq!(reply.headers["x-content-type-options"], "nosniff");
    assert_eq!(reply.headers["referrer-policy"], "no-referrer");
    assert_eq!(reply.headers["content-type"], "application/json");
    assert!(reply.headers.get("set-cookie").is_none());
    let body = reply.ok();
    // Independent expected value: the allowlist, spelled out, not derived from a DTO.
    assert_eq!(
        body,
        json!({
            "work": {
                "id": id(&work), "kind": "novel", "revision": revision,
                "title": "Ánh đèn cuối sân khấu",
                "synopsis": "Một lời hẹn dưới mái hiên nhà hát cũ.",
                "language": "vi-VN"
            },
            "units": [
                {"id": id(&units[0]), "kind": "chapter", "revision": 1, "ordinal": 1,
                 "title": "Chương 1", "description": "Mô tả — Chương 1"},
                {"id": id(&units[1]), "kind": "chapter", "revision": 1, "ordinal": 2,
                 "title": "Chương 2", "description": "Mô tả — Chương 2"}
            ],
            "next_cursor": null
        })
    );
    let text = body.to_string();
    for private in [
        "alice",
        "visibility",
        "rights_refs",
        "owner_id",
        "created_at",
    ] {
        assert!(!text.contains(private), "{private}");
    }
}

#[tokio::test]
async fn owner_reader_and_editor_may_preview_while_strangers_guests_and_absent_works_may_not() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("story").await;
    h.units(&mut work, 1).await;
    let wid = id(&work).to_owned();

    let work = h.set_member(&work, Some(MemberRole::Reader)).await;
    let query = format!("work_revision={}", work["revision"]);
    for token in [ALICE, BOB] {
        h.preview(Some(token), &wid, &query).await.ok();
    }
    let work = h.set_member(&work, Some(MemberRole::Editor)).await;
    let query = format!("work_revision={}", work["revision"]);
    h.preview(Some(BOB), &wid, &query).await.ok();

    // Wrong role/Work and a missing Work are the same answer, so existence does not leak.
    let stranger = h
        .preview(Some(CAROL), &wid, &query)
        .await
        .refused(StatusCode::NOT_FOUND, "not_found");
    let absent = h
        .preview(Some(ALICE), &operation(), &query)
        .await
        .refused(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(stranger, absent);
    h.preview(None, &wid, &query)
        .await
        .refused(StatusCode::UNAUTHORIZED, "unauthenticated");
    // A stranger is told nothing about the revision, even a stale one.
    h.preview(Some(CAROL), &wid, "work_revision=1")
        .await
        .refused(StatusCode::NOT_FOUND, "not_found");

    // Revocation is current authority on the next request: no lingering preview right.
    let work = h.set_member(&work, None).await;
    h.preview(
        Some(BOB),
        &wid,
        &format!("work_revision={}", work["revision"]),
    )
    .await
    .refused(StatusCode::NOT_FOUND, "not_found");
}

#[tokio::test]
async fn only_the_named_saved_head_is_previewed_and_a_moved_head_is_stale_never_substituted() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("podcast").await;
    let units = h.units(&mut work, 1).await;
    let wid = id(&work).to_owned();
    let saved = work["revision"].as_u64().unwrap();
    h.preview(Some(ALICE), &wid, &format!("work_revision={saved}"))
        .await
        .ok();

    // The creator saves a new revision of the Unit; the earlier banner may not show it.
    let edited = h
        .send(
            "PATCH",
            &format!("/content-units/{}", id(&units[0])),
            Some(ALICE),
            Some(json!({
                "operation_id": operation(), "expected_revision": 1,
                "expected_work_revision": saved,
                "details": {"title": "Tập mở đầu — bản sửa"}
            })),
        )
        .await;
    assert_eq!(edited.status, StatusCode::OK, "{}", edited.body);
    let head = edited.body["work"]["revision"].as_u64().unwrap();
    assert!(head > saved);
    let stale = h
        .preview(Some(ALICE), &wid, &format!("work_revision={saved}"))
        .await
        .refused(StatusCode::CONFLICT, "stale_revision");
    assert_eq!(stale["current_revision"], head);
    // A revision that does not exist yet is just as stale.
    h.preview(Some(ALICE), &wid, &format!("work_revision={}", head + 1))
        .await
        .refused(StatusCode::CONFLICT, "stale_revision");
    let body = h
        .preview(Some(ALICE), &wid, &format!("work_revision={head}"))
        .await
        .ok();
    assert_eq!(body["work"]["revision"], head);
    assert_eq!(body["units"][0]["title"], "Tập mở đầu — bản sửa");
    assert_eq!(body["units"][0]["revision"], 2);
    assert_eq!(body["units"][0]["kind"], "episode");
}

#[tokio::test]
async fn an_archived_work_is_refused_and_an_archived_unit_never_takes_a_rank() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("novel").await;
    let units = h.units(&mut work, 3).await;
    let wid = id(&work).to_owned();
    let archived = h
        .send(
            "DELETE",
            &format!("/content-units/{}", id(&units[1])),
            Some(ALICE),
            Some(json!({
                "operation_id": operation(), "expected_revision": 1,
                "expected_work_revision": work["revision"]
            })),
        )
        .await;
    assert_eq!(archived.status, StatusCode::OK, "{}", archived.body);
    let work = archived.body["work"].clone();
    let body = h
        .preview(
            Some(ALICE),
            &wid,
            &format!("work_revision={}", work["revision"]),
        )
        .await
        .ok();
    let shown: Vec<_> = body["units"]
        .as_array()
        .unwrap()
        .iter()
        .map(|unit| {
            (
                unit["title"].as_str().unwrap(),
                unit["ordinal"].as_u64().unwrap(),
            )
        })
        .collect();
    assert_eq!(shown, [("Chương 1", 1), ("Chương 3", 2)]);

    let archived_work = h
        .send(
            "DELETE",
            &format!("/works/{wid}"),
            Some(ALICE),
            Some(json!({"operation_id": operation(), "expected_revision": work["revision"]})),
        )
        .await;
    assert_eq!(
        archived_work.status,
        StatusCode::OK,
        "{}",
        archived_work.body
    );
    let head = archived_work.body["work"]["revision"].as_u64().unwrap();
    let refusal = h
        .preview(Some(ALICE), &wid, &format!("work_revision={head}"))
        .await
        .refused(StatusCode::CONFLICT, "catalog_conflict");
    assert_eq!(
        refusal["issues"],
        json!([{"path": "lifecycle", "rule": "archived"}])
    );
}

#[tokio::test]
async fn pages_keep_one_rank_across_the_walk_and_any_catalog_write_ends_it() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("novel").await;
    h.units(&mut work, 5).await;
    let wid = id(&work).to_owned();
    let revision = work["revision"].as_u64().unwrap();
    let query = format!("work_revision={revision}&limit=2");

    let first = h.preview(Some(ALICE), &wid, &query).await.ok();
    let second_query = format!("{query}&cursor={}", first["next_cursor"].as_str().unwrap());
    let second = h.preview(Some(ALICE), &wid, &second_query).await.ok();
    let third_query = format!("{query}&cursor={}", second["next_cursor"].as_str().unwrap());
    let third = h.preview(Some(ALICE), &wid, &third_query).await.ok();
    let ranks: Vec<_> = [&first, &second, &third]
        .iter()
        .flat_map(|page| page["units"].as_array().unwrap())
        .map(|unit| unit["ordinal"].as_u64().unwrap())
        .collect();
    assert_eq!(ranks, [1, 2, 3, 4, 5]);
    assert_eq!(third["next_cursor"], Value::Null);
    // The Work is repeated verbatim on every page: each page names the same exact revision.
    assert_eq!(first["work"], third["work"]);

    // An unrelated Catalog write moves the epoch, so a half-finished walk restarts...
    h.create_work("story").await;
    h.preview(Some(ALICE), &wid, &second_query)
        .await
        .refused(StatusCode::CONFLICT, "stale_cursor");
    // ...and a restart on the unchanged saved head succeeds.
    h.preview(Some(ALICE), &wid, &query).await.ok();
    // A change to this Work itself makes the named revision stale before any cursor is read.
    h.set_member(&work, Some(MemberRole::Reader)).await;
    h.preview(Some(ALICE), &wid, &second_query)
        .await
        .refused(StatusCode::CONFLICT, "stale_revision");
    h.preview(Some(ALICE), &wid, &format!("{query}&cursor=zz"))
        .await
        .refused(StatusCode::BAD_REQUEST, "invalid_request");
}

#[tokio::test]
async fn a_cursor_is_bound_to_its_actor_and_revision() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("novel").await;
    h.units(&mut work, 3).await;
    let work = h.set_member(&work, Some(MemberRole::Reader)).await;
    let wid = id(&work).to_owned();
    let query = format!("work_revision={}&limit=1", work["revision"]);
    let first = h.preview(Some(ALICE), &wid, &query).await.ok();
    let cursor = first["next_cursor"].as_str().unwrap();
    h.preview(Some(BOB), &wid, &format!("{query}&cursor={cursor}"))
        .await
        .refused(StatusCode::BAD_REQUEST, "invalid_request");
    h.preview(
        Some(ALICE),
        &wid,
        &format!(
            "work_revision={}&limit=1&cursor={cursor}",
            work["revision"].as_u64().unwrap() + 1
        ),
    )
    .await
    .refused(StatusCode::CONFLICT, "stale_revision");
}

#[tokio::test]
async fn malformed_requests_are_refused_before_any_read_and_leave_no_trace() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("story").await;
    h.units(&mut work, 1).await;
    let wid = id(&work).to_owned();
    let revision = work["revision"].as_u64().unwrap();
    for query in [
        String::new(),
        "work_revision=0".into(),
        "work_revision=-1".into(),
        "work_revision=9223372036854775808".into(),
        "work_revision=x".into(),
        format!("work_revision={revision}&limit=0"),
        format!("work_revision={revision}&limit=51"),
        format!("work_revision={revision}&visibility=public"),
        format!("work_revision={revision}&work_revision={revision}"),
    ] {
        let reply = h.preview(Some(ALICE), &wid, &query).await;
        assert_eq!(
            reply.status,
            StatusCode::BAD_REQUEST,
            "{query}: {}",
            reply.body
        );
        assert_eq!(reply.body["code"], "invalid_request", "{query}");
        assert_eq!(reply.headers["referrer-policy"], "no-referrer", "{query}");
    }
    h.preview(
        Some(ALICE),
        "not-a-uuid",
        &format!("work_revision={revision}"),
    )
    .await
    .refused(StatusCode::UNPROCESSABLE_ENTITY, "invalid_catalog");
    // Only GET reads a preview.
    let post = h
        .send(
            "POST",
            &format!("/works/{wid}/preview?work_revision={revision}"),
            Some(ALICE),
            Some(json!({})),
        )
        .await;
    assert_eq!(post.status, StatusCode::METHOD_NOT_ALLOWED);
}

#[tokio::test]
async fn previewing_writes_nothing_and_publishes_nothing() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let mut work = h.create_work("novel").await;
    h.units(&mut work, 3).await;
    let work = h.set_member(&work, Some(MemberRole::Editor)).await;
    let wid = id(&work).to_owned();
    let query = format!("work_revision={}", work["revision"]);
    let before = h.footprint().await;
    for token in [ALICE, BOB, ALICE, BOB] {
        h.preview(Some(token), &wid, &query).await.ok();
    }
    h.preview(Some(CAROL), &wid, &query)
        .await
        .refused(StatusCode::NOT_FOUND, "not_found");
    h.preview(Some(ALICE), &wid, "work_revision=1")
        .await
        .refused(StatusCode::CONFLICT, "stale_revision");
    assert_eq!(h.footprint().await, before);
    assert_eq!(before["generations"], 0);
    assert_eq!(before["pointers"], 0);
    // The Work was never made visible: the owner's publication state is still never-published.
    let state = h
        .send(
            "GET",
            &format!("/works/{wid}/metadata-publication"),
            Some(ALICE),
            None,
        )
        .await;
    assert_eq!(state.status, StatusCode::OK, "{}", state.body);
    assert_eq!(state.body["lifecycle"], "never_published");
}
