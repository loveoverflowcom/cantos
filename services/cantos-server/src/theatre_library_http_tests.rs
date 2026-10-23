//! The Library through the production HTTP router over a real file-backed `server.db`, and the
//! production CMP client against it. The principal is the trusted-host one (a synthetic verifier),
//! not an Identity session: the production session composition is `tests/theatre_business_journey.rs`.
//! Ported from the retired PostgreSQL oracles. Nothing here establishes the creator -> publication
//! pipeline, a device, or a live Identity provider.
use crate::contract::theatre::library as dto;
use crate::{
    adapters::inbound::http::{router_with_listener_identity, AppState},
    application::Store,
    contexts::listening::{
        auth::{
            AuthenticationError, AuthenticationFuture, ListenerAuthenticator, ListenerCredentials,
        },
        LibraryService,
    },
    test_support::library::{
        bookmark as bookmark_write, bookmark_delete, history_clear, history_start, principal,
        progress as progress_write, remember_audio, saved as saved_write, uid, LibraryWorld,
    },
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

// ---- transport: the production router over a trusted host principal ---------------------------

struct SyntheticVerifiedListener;
impl ListenerAuthenticator for SyntheticVerifiedListener {
    fn authenticate(&self, c: ListenerCredentials) -> AuthenticationFuture {
        let actor = match c.expose_to_verifier() {
            "synthetic-listener-alice" => Some("alice"),
            "synthetic-listener-bob" => Some("bob"),
            _ => None,
        };
        Box::pin(async move {
            actor
                .map(principal)
                .ok_or(AuthenticationError::Unauthenticated)
        })
    }
}

fn app(store: Store) -> Router {
    router_with_listener_identity(
        AppState {
            store,
            web_origin: "http://127.0.0.1:8080".into(),
        },
        "unused-static-root",
        Arc::new(SyntheticVerifiedListener),
    )
}

async fn request(
    app: &Router,
    method: &str,
    path: &str,
    actor: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let r = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(format!("/api/v1/theatre{path}"))
                .header(
                    "authorization",
                    format!("Bearer synthetic-listener-{actor}"),
                )
                .header("content-type", "application/json")
                .body(body.map_or(Body::empty(), |v| Body::from(v.to_string())))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = r.status();
    assert_eq!(r.headers()["cache-control"], "no-store");
    let value = serde_json::from_slice(&r.into_body().collect().await.unwrap().to_bytes()).unwrap();
    let mut root: Value = serde_json::from_str(include_str!(
        "../../../services/cantos-server/contracts/theatre/library/v1/library.schema.json"
    ))
    .unwrap();
    if status.is_success() && path.contains("presentation=1") {
        let definition = if path.starts_with("/library/continue") {
            "PresentedContinuePage"
        } else if path.starts_with("/library/saved") {
            "PresentedSavedPage"
        } else if path.starts_with("/library/history") {
            "PresentedHistoryPage"
        } else {
            panic!("unexpected presentation route: {path}");
        };
        root = json!({"$defs":root["$defs"],"$ref":format!("#/$defs/{definition}")});
    }
    assert!(
        jsonschema::validator_for(&root).unwrap().is_valid(&value),
        "{value}"
    );
    (status, value)
}

// ---- restart, presentation and what is never disclosed -----------------------------------------
// ---- bookmarks ---------------------------------------------------------------------------------

fn bm(n: u64) -> String {
    uid(0xb00 + n)
}

fn at(a: &dto::AudioIdentity, tenths: u64) -> u64 {
    a.duration_ms * tenths / 10
}

async fn put(r: &Router, actor: &str, w: &dto::BookmarkWrite) -> (StatusCode, Value) {
    request(
        r,
        "PUT",
        &format!("/library/bookmarks/{}", w.bookmark_id),
        actor,
        Some(serde_json::to_value(w).unwrap()),
    )
    .await
}

async fn remove(r: &Router, actor: &str, id: u64, d: &dto::BookmarkDelete) -> (StatusCode, Value) {
    request(
        r,
        "POST",
        &format!("/library/bookmarks/{}/delete", bm(id)),
        actor,
        Some(serde_json::to_value(d).unwrap()),
    )
    .await
}

async fn list(r: &Router, actor: &str, query: &str) -> (StatusCode, Value) {
    request(r, "GET", &format!("/library/bookmarks{query}"), actor, None).await
}

fn ids(page: &Value) -> Vec<String> {
    page["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|i| i["bookmark"]["bookmark_id"].as_str().unwrap().to_owned())
        .collect()
}
#[tokio::test]
async fn presented_library_survives_restart_and_does_not_disclose_edited_studio_heads() {
    let mut w = LibraryWorld::new().await;
    let alice = principal("alice");
    let a = w.audio.clone();
    remember_audio(&w.service(), &alice, &a).await;
    let expected = dto::AudioPresentation {
        work: dto::WorkPresentation {
            work_id: a.work_id.clone(),
            publication_id: a.publication_id.clone(),
            title: "Ánh đèn cuối sân khấu".into(),
            kind: crate::contract::theatre::discovery::WorkKind::Podcast,
            language: "vi-VN".into(),
        },
        unit_id: a.unit_id.clone(),
        unit_title: "Tập có âm thanh riêng tư".into(),
    };
    // A later, unreleased edit of the Work head must not label the owned release.
    w.fixture
        .edit_work(&w.work, |details| {
            details.title = "Bản nháp riêng chưa phát hành".into()
        })
        .await;
    w.fixture.restart().await;
    let restarted = w.service();
    let continued = restarted
        .presented_continue_listening(&alice, 1, None)
        .await
        .unwrap();
    assert_eq!(continued.items[0].presentation, Some(expected.clone()));
    assert_eq!(continued.items[0].progress.audio, a);
    assert_eq!(continued.items[0].resume, dto::ResumeStatus::ExactAvailable);
    assert_eq!(
        restarted
            .presented_saved_works(&alice, 1, None)
            .await
            .unwrap()
            .items[0]
            .presentation,
        Some(expected.work.clone())
    );
    assert_eq!(
        restarted
            .presented_history(&alice, 1, None)
            .await
            .unwrap()
            .items[0]
            .presentation,
        Some(expected.clone())
    );
    let router = app(w.fixture.store().clone());
    for (path, presentation) in [
        (
            "/library/continue",
            serde_json::to_value(&expected).unwrap(),
        ),
        (
            "/library/saved",
            serde_json::to_value(&expected.work).unwrap(),
        ),
        ("/library/history", serde_json::to_value(&expected).unwrap()),
    ] {
        let (status, legacy) = request(&router, "GET", path, "alice", None).await;
        assert_eq!(status, StatusCode::OK);
        assert!(legacy["items"][0].get("presentation").is_none());
        let (status, presented) = request(
            &router,
            "GET",
            &format!("{path}?presentation=1&limit=1"),
            "alice",
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(presented["items"][0]["presentation"], presentation);
        let (_, bob) = request(
            &router,
            "GET",
            &format!("{path}?presentation=1&limit=1"),
            "bob",
            None,
        )
        .await;
        assert_eq!(bob["items"], json!([]));
    }
}
#[tokio::test]
async fn presented_library_never_substitutes_replacement_titles_for_owned_audio() {
    let w = LibraryWorld::new().await;
    let alice = principal("alice");
    let service = w.service();
    remember_audio(&service, &alice, &w.audio).await;
    let replacement = w
        .fixture
        .publish_replacement(None, &w.work, &w.work.units[0])
        .await;
    let replacement_id = replacement.release_id.clone().unwrap();
    let continued = service
        .presented_continue_listening(&alice, 1, None)
        .await
        .unwrap();
    assert_eq!(continued.items[0].progress.audio, w.audio);
    assert_eq!(
        continued.items[0].resume,
        dto::ResumeStatus::ReleaseReplaced
    );
    assert_eq!(continued.items[0].presentation, None);
    let history = service.presented_history(&alice, 1, None).await.unwrap();
    assert_eq!(history.items[0].audio, w.audio);
    assert_eq!(history.items[0].resume, dto::ResumeStatus::ReleaseReplaced);
    assert_eq!(history.items[0].presentation, None);
    // Saved is about the Work, so the Work's current representative labels it.
    let saved = service
        .presented_saved_works(&alice, 1, None)
        .await
        .unwrap();
    assert_eq!(
        saved.items[0].presentation.as_ref().unwrap().publication_id,
        replacement_id
    );
    assert_eq!(saved.items[0].saved.work_id, w.audio.work_id);
    // Withdrawing the replacement leaves no title anywhere, and the records themselves remain.
    let mut after = w.work.units[0].clone();
    after.chain = replacement;
    w.fixture.withdraw_release("alice", &after).await;
    let router = app(w.fixture.store().clone());
    for path in ["/library/continue", "/library/saved", "/library/history"] {
        let (_, page) = request(
            &router,
            "GET",
            &format!("{path}?presentation=1"),
            "alice",
            None,
        )
        .await;
        assert_eq!(page["items"].as_array().unwrap().len(), 1);
        assert_eq!(page["items"][0]["presentation"], Value::Null);
        if path != "/library/saved" {
            assert_eq!(page["items"][0]["resume"], "access_unavailable");
        }
    }
}
#[tokio::test]
async fn clear_preserves_progress_saved_and_sessions_and_the_router_is_account_isolated() {
    let w = LibraryWorld::new().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    let router = app(w.fixture.store().clone());
    let progress = progress_write(
        &a,
        1,
        90,
        0,
        a.duration_ms,
        dto::ProgressKind::MarkCompleted,
    );
    assert_eq!(
        request(
            &router,
            "PUT",
            &format!("/progress/{}", a.publication_id),
            "alice",
            Some(serde_json::to_value(&progress).unwrap())
        )
        .await
        .0,
        StatusCode::OK
    );
    assert!(service
        .history(&p, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    service
        .set_saved(&p, &a.work_id, saved_write(1, 90, 0, true))
        .await
        .unwrap();
    let start = history_start(&a, 1, 90, 99, 0);
    let r = service.playback_started(&p, start.clone()).await.unwrap();
    assert_eq!(
        service.history(&p, 20, None).await.unwrap().items[0]
            .progress
            .as_ref()
            .unwrap()
            .completion,
        dto::Completion::Completed
    );
    service
        .clear_history(&p, history_clear(2, 90, 1, 0))
        .await
        .unwrap();
    // A delayed start of the old epoch replays its receipt and does not resurrect the history.
    assert_eq!(service.playback_started(&p, start).await.unwrap(), r);
    assert!(service
        .history(&p, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(service.saved(&p, &a.work_id).await.unwrap().saved);
    assert_eq!(
        service
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        dto::Completion::Completed
    );
    assert_eq!(w.count("listener_playback_sessions", "alice").await, 1);
    // A fresh start of the old epoch is a conflict that names the server's epoch.
    let late = service
        .playback_started(&p, history_start(&a, 3, 90, 100, 0))
        .await
        .unwrap();
    assert_eq!(late.outcome, dto::WriteOutcome::HistoryEpochConflict);
    let (_, bob) = request(&router, "GET", "/library/history", "bob", None).await;
    assert_eq!(bob["items"], json!([]));
    let (_, bob) = request(
        &router,
        "GET",
        &format!("/progress/{}", a.publication_id),
        "bob",
        None,
    )
    .await;
    assert_eq!(bob, json!({"resource":"progress","current":null}));
    let (c, s) = request(
        &router,
        "GET",
        &format!("/operations/{}", progress.operation.operation_id),
        "bob",
        None,
    )
    .await;
    assert_eq!(
        (c, s["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
}
#[tokio::test]
async fn latest_completed_release_suppresses_older_unfinished_work_without_erasing_either_register()
{
    let w = LibraryWorld::new().await;
    let (a, service, p) = (w.audio.clone(), w.service(), principal("alice"));
    service
        .put_progress(
            &p,
            &a.publication_id,
            progress_write(&a, 1, 90, 0, 100, dto::ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    let next = w
        .fixture
        .publish_replacement(None, &w.work, &w.work.units[0])
        .await;
    let b = crate::test_support::library::audio_of(&w.work, &w.work.units[0], &next);
    service
        .put_progress(
            &p,
            &b.publication_id,
            progress_write(
                &b,
                1,
                90,
                0,
                b.duration_ms,
                dto::ProgressKind::MarkCompleted,
            ),
        )
        .await
        .unwrap();
    assert!(service
        .continue_listening(&p, 1, None)
        .await
        .unwrap()
        .items
        .is_empty());
    let (_, page) = request(
        &app(w.fixture.store().clone()),
        "GET",
        "/library/continue?limit=1",
        "alice",
        None,
    )
    .await;
    assert_eq!(page, json!({"items":[],"next_cursor":null}));
    assert_eq!(
        service
            .progress(&p, &a.publication_id)
            .await
            .unwrap()
            .unwrap()
            .position_ms,
        100
    );
    assert_eq!(
        service
            .progress(&p, &b.publication_id)
            .await
            .unwrap()
            .unwrap()
            .completion,
        dto::Completion::Completed
    );
}
#[tokio::test]
async fn bookmark_create_list_delete_over_http_survive_restart_isolate_accounts_and_bound_positions(
) {
    let mut w = LibraryWorld::new().await;
    let a = w.audio.clone();
    let router = app(w.fixture.store().clone());
    let first = bookmark_write(1, &a, 1, 90, at(&a, 1));
    let (status, accepted) = put(&router, "alice", &first).await;
    assert_eq!(
        (status, accepted["outcome"].as_str()),
        (StatusCode::OK, Some("accepted"))
    );
    assert_eq!(
        accepted["state"]["current"]["audio"],
        serde_json::to_value(&a).unwrap()
    );
    // The same operation again is byte-identical, whatever happened in between.
    assert_eq!(
        put(&router, "alice", &first).await,
        (StatusCode::OK, accepted.clone())
    );
    assert_eq!(
        put(&router, "alice", &bookmark_write(1, &a, 2, 90, at(&a, 1)))
            .await
            .1["outcome"],
        "already_current"
    );
    for (id, seq, pos) in [(2, 1, at(&a, 2)), (3, 1, at(&a, 3)), (4, 1, a.duration_ms)] {
        assert_eq!(
            put(&router, "alice", &bookmark_write(id, &a, seq, 90, pos))
                .await
                .0,
            StatusCode::OK
        );
    }
    drop(router);
    w.fixture.restart().await;
    let router = app(w.fixture.store().clone());
    let (status, page) = list(&router, "alice", "?limit=3").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(ids(&page), vec![bm(4), bm(3), bm(2)]);
    let item = &page["items"][0];
    assert_eq!(item["resume"], "exact_available");
    assert_eq!(item["presentation"]["unit_id"], a.unit_id);
    assert_eq!(
        item["presentation"]["work"]["publication_id"],
        a.publication_id
    );
    let cursor = page["next_cursor"].as_str().unwrap().to_owned();
    let (_, rest) = list(&router, "alice", &format!("?limit=3&cursor={cursor}")).await;
    assert_eq!(ids(&rest), vec![bm(1)]);
    assert!(rest["next_cursor"].is_null());
    // A walk across a process restart is a refused cursor, never a silent restart of the walk.
    drop(router);
    w.fixture.restart().await;
    let router = app(w.fixture.store().clone());
    let (status, refused) = list(&router, "alice", &format!("?limit=3&cursor={cursor}")).await;
    assert_eq!(
        (status, refused["code"].as_str()),
        (StatusCode::BAD_REQUEST, Some("invalid_cursor"))
    );
    // Bounds: past the verified duration, an unknown release and a foreign path ID are refused.
    let (status, error) = put(
        &router,
        "alice",
        &bookmark_write(5, &a, 1, 90, a.duration_ms + 1),
    )
    .await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::UNPROCESSABLE_ENTITY, Some("invalid_position"))
    );
    let mut unknown = bookmark_write(5, &a, 1, 90, 1);
    unknown.audio.publication_id = uid(0xdead);
    let (status, error) = put(&router, "alice", &unknown).await;
    assert_eq!(
        (status, error["code"].as_str()),
        (StatusCode::NOT_FOUND, Some("not_found"))
    );
    let mismatched = bookmark_write(6, &a, 1, 90, 1);
    let (status, _) = request(
        &router,
        "PUT",
        &format!("/library/bookmarks/{}", bm(7)),
        "alice",
        Some(serde_json::to_value(&mismatched).unwrap()),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        list(&router, "alice", "?presentation=1").await.0,
        StatusCode::BAD_REQUEST
    );
    for limit in ["0", "51"] {
        assert_eq!(
            list(&router, "alice", &format!("?limit={limit}")).await.0,
            StatusCode::BAD_REQUEST
        );
    }
    // Authentication and account isolation.
    assert_eq!(
        list(&router, "mallory", "").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        put(&router, "mallory", &first).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(ids(&list(&router, "bob", "").await.1), Vec::<String>::new());
    assert_eq!(
        remove(&router, "bob", 1, &bookmark_delete(1, 91)).await.0,
        StatusCode::NOT_FOUND
    );
    let (_, theirs) = put(&router, "bob", &bookmark_write(1, &a, 1, 91, at(&a, 5))).await;
    assert_eq!(theirs["outcome"], "accepted");
    assert_eq!(theirs["state"]["current"]["position_ms"], at(&a, 5));
    assert_eq!(ids(&list(&router, "bob", "").await.1), vec![bm(1)]);
    // Delete: tombstone, never listed again, replays never resurrect it.
    let (status, gone) = remove(&router, "alice", 1, &bookmark_delete(3, 90)).await;
    assert_eq!(
        (status, gone["outcome"].as_str()),
        (StatusCode::OK, Some("accepted"))
    );
    assert_eq!(gone["state"]["current"]["deleted"], true);
    assert_eq!(
        ids(&list(&router, "alice", "?limit=50").await.1),
        vec![bm(4), bm(3), bm(2)]
    );
    assert_eq!(
        put(&router, "alice", &first).await,
        (StatusCode::OK, accepted.clone())
    );
    let (status, refused) = put(&router, "alice", &bookmark_write(1, &a, 4, 90, at(&a, 1))).await;
    assert_eq!(
        (status, refused["outcome"].as_str()),
        (StatusCode::CONFLICT, Some("bookmark_deleted"))
    );
    assert_eq!(refused["resolution"], "keep_server");
    assert_eq!(refused["state"]["current"]["deleted"], true);
    assert_eq!(
        remove(&router, "alice", 1, &bookmark_delete(5, 90)).await.1["outcome"],
        "already_current"
    );
    assert_eq!(
        ids(&list(&router, "alice", "?limit=50").await.1),
        vec![bm(4), bm(3), bm(2)]
    );
    // Lost response: the receipt is readable and carries the original answer.
    let (status, receipt) = request(
        &router,
        "GET",
        &format!("/operations/{}", first.operation.operation_id),
        "alice",
        None,
    )
    .await;
    assert_eq!((status, receipt), (StatusCode::OK, accepted));
    // Bookmarks leave Saved, progress and history alone (and the converse).
    let service = LibraryService::new(w.fixture.store().clone());
    let p = principal("alice");
    remember_audio(&service, &p, &a).await;
    assert_eq!(
        service.bookmarks(&p, 50, None).await.unwrap().items.len(),
        3
    );
    assert_eq!(
        service.saved_works(&p, 20, None).await.unwrap().items.len(),
        1
    );
}

/// The production CMP client (sync adapter, SQLite store, codec, HTTP client) against this real
/// router over loopback HTTP and the real `server.db`. The Kotlin side is `LiveBookmarkApiTest`;
/// this harness owns the server, the seeded rows and the final database assertions, so a Kotlin
/// pass cannot stand in for them. It runs through the Theatre trusted-host principal, not a real
/// Identity session (the production composition is covered by `tests/theatre_business_journey.rs`).
///
/// ```sh
/// CANTOS_TEST_GRADLE=/path/to/gradle cargo test -p cantos-server --lib \
///   cmp_production_client_against_the_real_api -- --ignored
/// ```
#[tokio::test]
#[ignore = "requires Gradle and the CMP desktop toolchain (CANTOS_TEST_GRADLE or gradle on PATH); no audio decoder or real identity provider"]
async fn cmp_production_client_against_the_real_api() {
    const SEEDED: u64 = 22;
    let w = LibraryWorld::new().await;
    let (a, service) = (w.audio.clone(), w.service());
    assert!(
        a.duration_ms >= 100,
        "fixture audio too short for distinct bookmark positions"
    );
    let alice = principal("alice");
    for n in 1..=SEEDED {
        service
            .create_bookmark(&alice, &bm(n), bookmark_write(n, &a, 1, 90, n))
            .await
            .unwrap();
    }
    // This fixture deliberately has a trusted-host principal rather than Identity admission.
    // Stamp at the fixture server, never by echoing the client's expected session in Kotlin.
    let router = app(w.fixture.store().clone()).layer(axum::middleware::map_response(
        |mut response: axum::response::Response| async move {
            response.headers_mut().insert(
                "x-cantos-theatre-session",
                "00000000-0000-0000-0000-000000000001".parse().unwrap(),
            );
            response
        },
    ));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let server = tokio::spawn(async move { axum::serve(listener, router).await });
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let identity = serde_json::to_string(&a).unwrap();
    let gradle = std::env::var("CANTOS_TEST_GRADLE").unwrap_or_else(|_| "gradle".into());
    let output = tokio::task::spawn_blocking(move || {
        std::process::Command::new(gradle)
            .current_dir(root)
            .args([
                "--project-dir",
                "apps/cantos-theatre/mobile",
                "--no-daemon",
                "-Dorg.gradle.java.installations.auto-download=false",
                "--max-workers",
                "2",
                "-Pkotlin.compiler.execution.strategy=in-process",
                "--no-configuration-cache",
                "-i",
                ":shared:desktopTest",
                "--rerun",
                "--tests",
                "com.cantos.theatre.adapters.library.LiveBookmarkApiTest",
            ])
            .env("CANTOS_LIVE_API_ORIGIN", format!("http://127.0.0.1:{port}"))
            .env("CANTOS_LIVE_AUDIO", identity)
            .env("CANTOS_LIVE_SEEDED", SEEDED.to_string())
            .output()
    })
    .await
    .unwrap()
    .expect("gradle could not be started");
    server.abort();
    let log = String::from_utf8_lossy(&output.stdout).into_owned();
    assert!(
        output.status.success(),
        "{log}\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        log.contains("LIVE-BOOKMARK-OK"),
        "the live test was skipped, not passed:\n{log}"
    );
    println!(
        "{}",
        log.lines()
            .find(|l| l.contains("LIVE-BOOKMARK-OK"))
            .unwrap_or_default()
            .trim()
    );
    // The database is the authority: what the client believes must be exactly what it holds.
    assert_eq!(
        w.fixture.bookmark_rows("alice", false).await,
        SEEDED as i64 + 2
    );
    assert_eq!(w.fixture.bookmark_rows("alice", true).await, 1);
    assert_eq!(
        w.count("listener_bookmarks", "bob").await,
        0,
        "no row for another account"
    );
    let (duplicates, foreign) = w
        .fixture
        .bookmark_anomalies("alice", &a.publication_id)
        .await;
    assert_eq!(duplicates, 0, "no duplicate at one release position");
    assert_eq!(foreign, 0, "every row names the exact release");
}
