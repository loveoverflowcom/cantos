//! Explicit reusable local datasets. Ordinary tests exercise only bounded synthetic controls;
//! the persistent create/serve/capability/scale campaign requires cargo xtask seed dataset.
#![cfg(unix)]
#[path = "support/local_dataset/mod.rs"]
mod dataset;
#[path = "support/artwork_storage_http.rs"]
mod http;
#[path = "support/sample_catalog_files.rs"]
mod sample_files;
use cantos_store_turso::testing::TestRun;
use dataset::{
    plan::{self, Plan},
    runtime,
};
use serde_json::{json, Value};

#[tokio::test]
#[ignore = "explicit persistent localhost profile; cargo xtask seed dataset"]
async fn dataset_cli() {
    let selection: Value =
        serde_json::from_str(&std::env::var("CANTOS_DATASET_SELECTION").expect("xtask selection"))
            .unwrap();
    let p = Plan::build(
        selection["profile"].as_str().unwrap(),
        selection["seed"].as_u64().unwrap(),
        Some(selection["size"].as_u64().unwrap() as usize),
    )
    .unwrap();
    let root =
        std::path::PathBuf::from(std::env::var_os("CANTOS_DATASET_ROOT").expect("explicit root"));
    let action = std::env::var("CANTOS_DATASET_ACTION").unwrap();
    if action == "create" {
        runtime::initialize(&root, &p);
    } else {
        runtime::validate_marker(&root, &p);
    }
    if action == "archive-check" {
        return;
    }
    assert!(matches!(action.as_str(), "create" | "verify" | "serve"));
    let manifest=tokio::time::timeout(std::time::Duration::from_secs(1800),runtime::complete(&root,&p,action=="create")).await.expect("bounded 30-minute dataset budget exceeded; no new READY was published, inspect existing records and resume same selection");
    println!("{}",serde_json::to_string_pretty(&json!({"profile":p.profile,"root":root,"evidence":manifest["evidence"],"capabilities":manifest["capabilities"],"logical_sha256":manifest["logical_sha256"]})).unwrap());
    if action == "serve" {
        let s = runtime::store(&root).await;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
            .await
            .expect("occupied port; no unrelated process stopped");
        println!("Test-hosted Catalog API at http://127.0.0.1:8080; private test-callers.json is not a Studio login. Ctrl-C stops this owner. Dataset verification requires the original immutable seed state; archive and recreate after exploratory CRUD.");
        axum::serve(listener, runtime::router(&s))
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.unwrap();
            })
            .await
            .unwrap();
    }
}

#[tokio::test]
async fn clean_runs_interrupted_import_replay_and_snapshot_have_identical_logical_data() {
    let run = TestRun::new("dataset-determinism");
    let p = Plan::build("studio-crud", 568, None).unwrap();
    let first = run.path("first");
    runtime::initialize(&first, &p);
    let tokens = runtime::bootstrap(&first).await;
    let s = runtime::store(&first).await;
    let host = http::HttpHost::start(runtime::router(&s)).await;
    runtime::seed(&s, &host, &p, &tokens, Some(2)).await;
    host.stop().await;
    drop(s);
    assert!(
        !first.join("ready.json").exists(),
        "interrupted import cannot claim READY"
    );
    let a = runtime::complete(&first, &p, true).await;
    let second = run.path("second");
    runtime::initialize(&second, &p);
    let b = runtime::complete(&second, &p, true).await;
    assert_eq!(a["logical_sha256"], b["logical_sha256"]);
    assert!(
        std::fs::read(first.join("test-callers.json")).unwrap()
            != std::fs::read(second.join("test-callers.json")).unwrap(),
        "credentials never derive from seed"
    );
    runtime::complete(&first, &p, false).await;
}
#[tokio::test]
async fn empty_mixed_unicode_permission_profiles_match_real_sql_and_http_oracles() {
    let run = TestRun::new("dataset-smoke");
    for name in [
        "empty",
        "minimal",
        "mixed-kinds",
        "unicode-and-patch",
        "permissions-conflicts",
        "visibility-recovery",
        "pagination-boundaries",
    ] {
        let p = Plan::build(name, 568, None).unwrap();
        let root = run.path(name);
        runtime::initialize(&root, &p);
        runtime::complete(&root, &p, true).await;
    }
}
#[tokio::test]
#[ignore = "explicit all-cardinality pagination campaign; each size is available through cargo xtask seed dataset"]
async fn native_catalog_pagination_boundary_matrix_has_exact_order_and_no_missing_ids() {
    let run = TestRun::new("dataset-pages");
    for size in [0, 1, 19, 20, 21, 49, 50, 51, 101] {
        let p = Plan::build("pagination-boundaries", 568, Some(size)).unwrap();
        let root = run.path(&size.to_string());
        runtime::initialize(&root, &p);
        runtime::complete(&root, &p, true).await;
    }
}
#[tokio::test]
async fn substring_goldens_use_the_real_engine_and_keep_private_http_results_empty() {
    let run = TestRun::new("dataset-substring");
    let p = Plan::build("substring-search", 568, None).unwrap();
    let root = run.path("corpus");
    runtime::initialize(&root, &p);
    runtime::complete(&root, &p, true).await;
}

#[test]
fn foreign_changed_or_symlinked_roots_are_refused_before_database_creation() {
    let run = TestRun::new("dataset-refusals");
    let p = Plan::build("empty", 568, None).unwrap();
    let root = run.path("owned");
    runtime::initialize(&root, &p);
    let other = Plan::build("empty", 569, None).unwrap();
    assert!(std::panic::catch_unwind(|| runtime::initialize(&root, &other)).is_err());
    std::fs::write(root.join("plan.json"), b"{}").unwrap();
    assert!(std::panic::catch_unwind(|| runtime::validate_inputs(&root, &p)).is_err());
    assert!(!root.join("server.db").exists());
    let foreign = run.path("foreign");
    std::fs::create_dir(&foreign).unwrap();
    std::fs::write(foreign.join("evidence"), b"retain").unwrap();
    assert!(std::panic::catch_unwind(|| runtime::initialize(&foreign, &p)).is_err());
    assert_eq!(std::fs::read(foreign.join("evidence")).unwrap(), b"retain");
    let link = run.path("link");
    std::os::unix::fs::symlink(&root, &link).unwrap();
    assert!(std::panic::catch_unwind(|| runtime::initialize(&link, &p)).is_err());
}

#[tokio::test]
async fn theatre_corpus_reuses_typed_release_fixture_and_has_independent_public_membership() {
    use cantos_server::{
        contexts::catalog::domain::{Visibility, WorkKind},
        test_support::{ServerFixture, UnitSeed, WorkSeed},
    };
    let clock = cantos_store_turso::testing::FixedClock::new(plan::CLOCK);
    let mut fixture = ServerFixture::with_clock(clock.clone()).await;
    let corpus: Value = serde_json::from_str(plan::CORPUS).unwrap();
    let mut ids = vec![];
    // Existing owner seam: synthetic typed release chains only. This separate disposable DB
    // never becomes a normal persisted profile, real rights/QC, audio or publication evidence.
    for (i, d) in corpus["documents"].as_array().unwrap().iter().enumerate() {
        let seeded = fixture
            .seed_work(
                None,
                &WorkSeed {
                    owner: "dataset_release_fixture".into(),
                    tag: 5680 + i as u32,
                    kind: WorkKind::parse(d["kind"].as_str().unwrap()).unwrap(),
                    title: d["title"].as_str().unwrap().into(),
                    synopsis: d["description"].as_str().unwrap().into(),
                    language: d["language"].as_str().unwrap().into(),
                    visibility: Visibility::Public,
                    units: vec![UnitSeed::published("Synthetic release fixture")],
                },
            )
            .await;
        ids.push(seeded.work_id);
    }
    let host = http::HttpHost::start(runtime::router(fixture.store())).await;
    for q in corpus["queries"].as_array().unwrap() {
        let mut query = url::form_urlencoded::Serializer::new(String::new());
        query
            .append_pair("q", q["q"].as_str().unwrap())
            .append_pair("limit", "1");
        for field in ["kind", "language"] {
            if let Some(v) = q[field].as_str() {
                query.append_pair(field, v);
            }
        }
        let base = format!("/api/v1/theatre/search?{}", query.finish());
        let mut actual = vec![];
        let mut cursor = None;
        for _ in 0..=ids.len() {
            let path = cursor
                .as_ref()
                .map_or_else(|| base.clone(), |c| format!("{base}&cursor={c}"));
            let page: Value = host
                .json::<Value>("GET", &path, None, None, None)
                .await
                .success(axum::http::StatusCode::OK);
            actual.extend(
                page["items"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .map(|w| w["work_id"].as_str().unwrap().to_owned()),
            );
            cursor = page["next_cursor"].as_str().map(str::to_owned);
            if cursor.is_none() {
                break;
            }
        }
        assert!(cursor.is_none());
        let mut expected: Vec<_> = q["expected"]
            .as_array()
            .unwrap()
            .iter()
            .map(|i| ids[i.as_u64().unwrap() as usize].clone())
            .collect();
        expected.sort();
        assert_eq!(actual, expected, "public membership/order for {}", q["id"]);
    }
    let page: Value = host
        .json::<Value>("GET", "/api/v1/theatre/works?limit=1", None, None, None)
        .await
        .success(axum::http::StatusCode::OK);
    let cursor = page["next_cursor"].as_str().unwrap().to_owned();
    for query in ["limit=1&kind=story", "limit=20", "limit=1&presentation=1"] {
        host.json::<Value>(
            "GET",
            &format!("/api/v1/theatre/works?{query}&cursor={cursor}"),
            None,
            None,
            None,
        )
        .await
        .error(axum::http::StatusCode::BAD_REQUEST, "invalid_cursor");
    }
    host.stop().await;
    fixture.restart().await;
    let host = http::HttpHost::start(runtime::router(fixture.store())).await;
    host.json::<Value>(
        "GET",
        &format!("/api/v1/theatre/works?limit=1&cursor={cursor}"),
        None,
        None,
        None,
    )
    .await
    .error(axum::http::StatusCode::BAD_REQUEST, "invalid_cursor");
    host.stop().await;
}

#[tokio::test]
async fn catalog_cursor_stales_after_another_owner_commit_and_scope_never_changes() {
    let run = TestRun::new("dataset-cursor");
    let p = Plan::build("mixed-kinds", 568, None).unwrap();
    let root = run.path("data");
    runtime::initialize(&root, &p);
    let tokens = runtime::bootstrap(&root).await;
    let s = runtime::store(&root).await;
    let host = http::HttpHost::start(runtime::router(&s)).await;
    runtime::seed(&s, &host, &p, &tokens, None).await;
    let page: Value = host
        .json::<Value>(
            "GET",
            "/api/v1/studio/works?limit=1",
            Some(&tokens[0]),
            None,
            None,
        )
        .await
        .success(axum::http::StatusCode::OK);
    let cursor = page["next_cursor"].as_str().unwrap();
    let child_work = p.works.iter().find(|w| w.units.len() > 1).unwrap();
    host.json::<Value>(
        "GET",
        &format!(
            "/api/v1/studio/works/{}/content-units?limit=1&cursor={cursor}",
            child_work.id
        ),
        Some(&tokens[0]),
        None,
        None,
    )
    .await
    .error(axum::http::StatusCode::BAD_REQUEST, "invalid_request");
    let body = json!({"operation_id":p.id("other-owner-write"),"id":p.id("other-owner-work"),"kind":"story","details":{"title":"Another owner","synopsis":"","language":"en","visibility":"private","rights_refs":[]}});
    let _: Value = host
        .json(
            "POST",
            "/api/v1/studio/works",
            Some(&tokens[1]),
            None,
            Some(&body),
        )
        .await
        .success(axum::http::StatusCode::CREATED);
    host.json::<Value>(
        "GET",
        &format!("/api/v1/studio/works?limit=1&cursor={cursor}"),
        Some(&tokens[0]),
        None,
        None,
    )
    .await
    .error(axum::http::StatusCode::CONFLICT, "stale_cursor");
    host.stop().await;
}
