//! Persistent fixture shell. Marker/plan validation precedes database effects. READY is the
//! last immutable publication, after HTTP, native SQL, integrity, reopen and snapshot checks.
use super::{
    checks,
    plan::{self, Plan, ACTORS},
};
use crate::{http, sample_files};
use axum::{http::StatusCode, Router};
use cantos_server::{
    adapters::inbound::http::{api_router, AppState},
    contexts::catalog::{
        domain::{OwnerId, WorkId},
        CatalogCommand, MemberRole,
    },
    platform::{
        store::{token_hash, Store},
        turso::registry,
    },
    test_support::open_dataset_store,
};
use cantos_store_turso::{
    registry::Intent, testing::FixedClock, OpenRequest, OwnedDatabase, Value as SqlValue,
};
use serde_json::{json, Value};
use std::{fs, path::Path};

pub const ORIGIN: &str = "http://127.0.0.1:8080";
pub fn schema() -> Value {
    let r = registry().unwrap();
    json!({"application_id":r.application_id(),"epoch":r.epoch(),"migrations":r.migrations().iter().map(|m|json!({"version":m.version,"name":m.name,"sha256":m.checksum().iter().map(|b|format!("{b:02x}")).collect::<String>()})).collect::<Vec<_>>()})
}
pub fn generator_hash() -> String {
    plan::hash(
        [
            include_str!("plan.rs"),
            include_str!("runtime.rs"),
            include_str!("checks.rs"),
            include_str!("../../local_dataset_turso.rs"),
            include_str!("../../../../../packages/store-turso/src/search_probe.rs"),
            include_str!("../../fixtures/local-datasets/checks.sql"),
            plan::REGISTRY,
            plan::CORPUS,
        ]
        .join("\0"),
    )
}
pub fn marker(root: &Path, p: &Plan) -> Value {
    json!({"dataset_id":plan::DATASET,"root":root,"selection":{"profile":p.profile,"seed":p.seed,"size":p.size},"plan_sha256":plan::hash(serde_json::to_vec(p).unwrap()),"generator_sha256":generator_hash(),"schema":schema(),"engine":cantos_store_turso::capability::ENGINE_PIN})
}
pub fn validate_paths(root: &Path) {
    assert!(
        root.is_absolute() && root.file_name().is_some(),
        "explicit absolute dedicated root required"
    );
    assert!(
        !root
            .components()
            .any(|c| matches!(c, std::path::Component::ParentDir)),
        "parent traversal refused"
    );
    for path in root.ancestors() {
        assert!(!path.join(".git").exists(), "dataset outside Git required");
        assert!(
            path == root
                || ["sample-owner.json", "server.db", "identity.db"]
                    .iter()
                    .all(|name| !path.join(name).exists()),
            "dataset overlaps an existing owner"
        );
        if let Ok(m) = fs::symlink_metadata(path) {
            assert!(!m.file_type().is_symlink(), "symlink root/ancestor refused");
        }
    }
    if root.exists() {
        assert!(fs::symlink_metadata(root).unwrap().is_dir());
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(root).unwrap().permissions().mode() & 0o777,
            0o700,
            "private dataset root required"
        );
        for entry in fs::read_dir(root).unwrap() {
            let entry = entry.unwrap();
            assert!(
                entry.file_type().unwrap().is_file(),
                "unexpected non-file in dataset root"
            );
        }
    }
}
pub fn read_json(path: &Path) -> Value {
    assert!(
        fs::symlink_metadata(path).unwrap().file_type().is_file(),
        "regular file required"
    );
    serde_json::from_slice(&fs::read(path).unwrap()).unwrap()
}
pub fn write(root: &Path, name: &str, value: &Value) {
    sample_files::write_once(&root.join(name), &serde_json::to_vec_pretty(value).unwrap()).unwrap();
}
pub fn validate_marker(root: &Path, p: &Plan) {
    validate_paths(root);
    assert_eq!(
        read_json(&root.join("sample-owner.json")),
        marker(root, p),
        "foreign or changed dataset/generator/schema fingerprint; retained without effects"
    );
}
fn inputs(p: &Plan) -> Vec<(&'static str, Value)> {
    let mut files = vec![
        ("plan.json", serde_json::to_value(p).unwrap()),
        (
            "substring-corpus.json",
            serde_json::from_str(plan::CORPUS).unwrap(),
        ),
        (
            "negative-requests.json",
            json!(checks::negative_requests(p)),
        ),
    ];
    let mut groups = std::collections::BTreeMap::new();
    for w in &p.works {
        *groups
            .entry((
                ACTORS[w.actor],
                w.kind.as_str(),
                if w.archived { "archived" } else { "draft" },
            ))
            .or_insert(0usize) += 1;
    }
    files.push((
        "oracles.json",
        json!({"counts_by_owner_kind_lifecycle":groups.iter().map(|((owner,kind,lifecycle),count)|json!({"owner":owner,"kind":kind,"lifecycle":lifecycle,"count":count})).collect::<Vec<_>>(),"expected_works":p.works,"request_ids":p.steps.iter().map(|s|s.id.as_str()).collect::<Vec<_>>(),"studio_order":"UUID; units (position,id); history revision","theatre_expected":"no visible records; no audio/release/disclosure","browser":"NOT_RUN"}),
    ));
    if p.profile == "vector-corpus" {
        files.push((
            "engine-corpus.json",
            json!({"dimension":3,"metric":"Euclidean L2","tolerance":1e-6,"documents":cantos_store_turso::search_probe::VECTOR_DOCUMENTS,"query":[0,0,0],"expected_order":["a","b","d","c","e"],"expected_distance":[0,0,3,5,12],"tie_break":"id ascending","index":"NOT_RUN: full scan only"}),
        ));
    }
    if p.profile == "fts-corpus" {
        files.push((
            "engine-corpus.json",
            json!({"documents":cantos_store_turso::search_probe::FTS_DOCUMENTS,"candidate_membership":cantos_store_turso::search_probe::FTS_QUERIES,"runtime":"UNSUPPORTED until fts feature is reviewed/enabled and corpus is qualified"}),
        ));
    }
    files
}
pub fn validate_inputs(root: &Path, p: &Plan) {
    for (name, value) in inputs(p) {
        assert_eq!(read_json(&root.join(name)), value, "changed input: {name}");
    }
    assert_eq!(
        fs::read(root.join("checks.sql")).unwrap(),
        include_bytes!("../../fixtures/local-datasets/checks.sql")
    );
}
pub fn initialize(root: &Path, p: &Plan) {
    validate_paths(root);
    sample_files::initialize_root(root, &serde_json::to_vec_pretty(&marker(root, p)).unwrap())
        .unwrap();
    for (name, value) in inputs(p) {
        write(root, name, &value);
    }
    sample_files::write_once(
        &root.join("checks.sql"),
        include_bytes!("../../fixtures/local-datasets/checks.sql"),
    )
    .unwrap();
}
pub async fn open(root: &Path, name: &str, intent: Intent) -> OwnedDatabase {
    OwnedDatabase::open(OpenRequest::new(
        root.join(name),
        registry().unwrap(),
        intent,
        FixedClock::new(plan::CLOCK),
    ))
    .await
    .unwrap()
}
pub async fn store(root: &Path) -> Store {
    open_dataset_store(
        root.join("server.db"),
        Intent::Serve,
        FixedClock::new(plan::CLOCK),
    )
    .await
    .unwrap()
}
pub fn router(store: &Store) -> Router {
    api_router(AppState {
        store: store.clone(),
        web_origin: ORIGIN.into(),
    })
}
pub async fn bootstrap(root: &Path) -> Vec<String> {
    let tokens = root.join("test-callers.json");
    if !tokens.exists() {
        assert!(
            !root.join("server.db").exists(),
            "cannot recover missing credentials by adopting an existing DB"
        );
        let callers:Vec<_>=ACTORS.iter().map(|actor|json!({"actor":actor,"session_id":uuid::Uuid::new_v4().to_string(),"token":format!("{}{}",uuid::Uuid::new_v4().simple(),uuid::Uuid::new_v4().simple())})).collect();
        write(root, "test-callers.json", &json!(callers));
    }
    let callers = read_json(&tokens);
    let callers = callers.as_array().unwrap();
    assert_eq!(callers.len(), ACTORS.len());
    let db = open(
        root,
        "server.db",
        if root.join("server.db").exists() {
            Intent::Serve
        } else {
            Intent::Migrate
        },
    )
    .await;
    let tx = db.begin_write().await.unwrap();
    for (i, c) in callers.iter().enumerate() {
        assert_eq!(c["actor"], ACTORS[i]);
        let actor = ACTORS[i];
        let token = c["token"].as_str().unwrap();
        let session = c["session_id"].as_str().unwrap();
        tx.execute(
            "INSERT INTO actors(id) SELECT ?1 WHERE NOT EXISTS(SELECT 1 FROM actors WHERE id=?1)",
            [actor],
        )
        .await
        .unwrap();
        tx.execute("INSERT INTO development_sessions(token_hash,actor_id,expires_at,session_id) SELECT ?1,?2,?3,?4 WHERE NOT EXISTS(SELECT 1 FROM development_sessions WHERE session_id=?4)",(token_hash(token),actor,plan::CLOCK+86_400_000_000,session)).await.unwrap();
    }
    tx.commit().await.unwrap();
    drop(db);
    callers
        .iter()
        .map(|c| c["token"].as_str().unwrap().into())
        .collect()
}
pub fn tokens(root: &Path) -> Vec<String> {
    read_json(&root.join("test-callers.json"))
        .as_array()
        .unwrap()
        .iter()
        .map(|c| c["token"].as_str().unwrap().into())
        .collect()
}

pub async fn seed(
    store: &Store,
    host: &http::HttpHost,
    p: &Plan,
    tokens: &[String],
    stop_after: Option<usize>,
) {
    for (i, s) in p.steps.iter().enumerate() {
        if stop_after == Some(i) {
            return;
        }
        if s.method == "MEMBER" {
            store
                .catalog_execute(
                    &tokens[s.actor],
                    &s.id,
                    CatalogCommand::SetMember {
                        work_id: WorkId::parse(&s.path).unwrap(),
                        expected_revision: s.body["expected_revision"].as_u64().unwrap(),
                        actor_id: OwnerId::parse(s.body["actor_id"].as_str().unwrap()).unwrap(),
                        role: Some(if s.body["role"] == "editor" {
                            MemberRole::Editor
                        } else {
                            MemberRole::Reader
                        }),
                    },
                )
                .await
                .unwrap();
        } else {
            let reply = host
                .json(
                    &s.method,
                    &format!("/api/v1/studio{}", s.path),
                    Some(&tokens[s.actor]),
                    None,
                    Some(&s.body),
                )
                .await;
            let _: Value = reply.success(StatusCode::from_u16(s.status).unwrap());
        }
    }
}
pub async fn sql_rows(db: &OwnedDatabase, sql: &str) -> Vec<Value> {
    let reader = db.reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let mut values = vec![];
    while let Some(row) = rows.next().await.unwrap() {
        let SqlValue::Text(s) = row.get_value(0).unwrap() else {
            panic!("JSON row expected")
        };
        values.push(serde_json::from_str(&s).unwrap());
    }
    values
}
pub async fn observation(db: &OwnedDatabase) -> Value {
    let mut result = serde_json::Map::new();
    for (name,sql) in [
        ("works","SELECT json_object('id',id,'owner',owner_id,'kind',kind,'lifecycle',lifecycle,'revision',head_revision) FROM catalog_works ORDER BY id"),
        ("units","SELECT json_object('id',id,'work_id',work_id,'position',position,'lifecycle',lifecycle,'revision',head_revision) FROM catalog_units ORDER BY work_id,position,id"),
        ("work_history","SELECT json_object('id',work_id,'revision',revision,'snapshot',hex(snapshot)) FROM catalog_work_revisions ORDER BY work_id,revision"),
        ("unit_history","SELECT json_object('id',unit_id,'revision',revision,'snapshot',hex(snapshot)) FROM catalog_unit_revisions ORDER BY unit_id,revision"),
        ("receipts","SELECT json_object('actor',actor_id,'id',operation_id,'fingerprint',fingerprint,'digest',result_digest,'bytes',hex(result_bytes)) FROM catalog_operations ORDER BY actor_id,operation_id"),
        ("members","SELECT json_object('work',work_id,'actor',actor_id,'role',role) FROM catalog_members ORDER BY work_id,actor_id"),
        ("counts","SELECT json_object('clock',(SELECT revision FROM catalog_clock),'releases',(SELECT count(*) FROM audio_releases),'images',(SELECT count(*) FROM media_image_assets),'sources',(SELECT count(*) FROM source_records))"),
    ] {result.insert(name.into(),json!(sql_rows(db,sql).await));}
    Value::Object(result)
}
pub async fn complete(root: &Path, p: &Plan, create: bool) -> Value {
    validate_marker(root, p);
    validate_inputs(root, p);
    if !create {
        assert!(
            root.join("ready.json").is_file(),
            "incomplete dataset; resume create with exactly the same selection"
        );
    }
    let tokens = if create {
        bootstrap(root).await
    } else {
        tokens(root)
    };
    let started = std::time::Instant::now();
    let s = store(root).await;
    let host = http::HttpHost::start(router(&s)).await;
    if create {
        seed(&s, &host, p, &tokens, None).await;
    }
    checks::http(&host, p, &tokens).await;
    host.stop().await;
    drop(s);
    let db = open(root, "server.db", Intent::Serve).await;
    db.check_integrity().await.unwrap();
    let observed = observation(&db).await;
    checks::sql(p, &observed);
    if p.profile == "substring-search" {
        checks::substring(&db, p).await;
    }
    let logical = plan::hash(serde_json::to_vec(&observed).unwrap());
    if create && !root.join("sample.db").exists() {
        db.backup_to(&root.join("sample.db")).await.unwrap();
    }
    drop(db);
    // A fresh owner/router replays the original requests, including accepted responses lost
    // before the manifest was written. No new operation identity is minted on a retry.
    let s = store(root).await;
    let host = http::HttpHost::start(router(&s)).await;
    if create {
        seed(&s, &host, p, &tokens, None).await;
    }
    checks::http(&host, p, &tokens).await;
    host.stop().await;
    drop(s);
    let db = open(root, "server.db", Intent::Serve).await;
    assert_eq!(
        observation(&db).await,
        observed,
        "reopen/replay mutated logical records"
    );
    drop(db);
    let copied = open(root, "sample.db", Intent::Serve).await;
    copied.check_integrity().await.unwrap();
    assert_eq!(
        observation(&copied).await,
        observed,
        "snapshot differs from native runtime DB"
    );
    drop(copied);
    let capabilities = match p.profile.as_str() {
        "vector-corpus" => {
            let r = cantos_store_turso::search_probe::vectors().await;
            json!({"vector":{"status":r.status,"detail":r.detail,"checks":r.checks},"product":"NOT_RUN"})
        }
        "fts-corpus" => {
            let r = cantos_store_turso::search_probe::fts().await;
            json!({"fts":{"status":r.status,"detail":r.detail,"checks":r.checks},"product":"NOT_RUN"})
        }
        _ => {
            json!({"fts":"NOT_RUN","vector":"NOT_RUN","product_search":"existing substring contract unchanged"})
        }
    };
    let duration_ms = started.elapsed().as_millis();
    let ready = json!({"state":"READY","dataset_id":plan::DATASET,"plan_sha256":plan::hash(serde_json::to_vec(p).unwrap()),"logical_sha256":logical,"schema":schema(),"engine":cantos_store_turso::capability::ENGINE_PIN});
    let source = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .unwrap();
    assert!(source.status.success());
    let oracles = read_json(&root.join("oracles.json"));
    let corpus: Value = serde_json::from_str(plan::CORPUS).unwrap();
    let query_ids: Vec<_> = corpus["queries"]
        .as_array()
        .unwrap()
        .iter()
        .map(|q| &q["id"])
        .collect();
    let manifest = json!({
        "binding": marker(root,p),
        "source_sha": String::from_utf8(source.stdout).unwrap().trim(),
        "clock_us": plan::CLOCK,
        "profile": p.profile, "seed": p.seed, "size": p.size,
        "logical_sha256": logical,
        "counts": {"works":p.works.len(), "units":p.works.iter().map(|w|w.units.len()).sum::<usize>(), "operations":p.steps.len()},
        "counts_by_owner_kind_lifecycle": oracles["counts_by_owner_kind_lifecycle"],
        "request_ids": oracles["request_ids"],
        "substring_corpus_query_ids": query_ids,
        "expected": p.works, "capabilities": capabilities,
        "evidence": {
            "sql_http":"PASS", "integrity":"PASS", "reopen_snapshot":"PASS",
            "browser":"NOT_RUN: #565 authorized Studio provisioning required",
            "publication":"NOT_RUN: metadata-only private fixtures",
            "seed_elapsed_ms":duration_ms,
            "database_bytes":fs::metadata(root.join("server.db")).unwrap().len(),
            "database_wal_bytes":fs::metadata(root.join("server.db-wal")).map_or(0, |m| m.len()),
            "snapshot_bytes":fs::metadata(root.join("sample.db")).unwrap().len()
        }
    });
    if create {
        // Preserve first-run measurements/provenance; repeated runs validate the stable binding.
        if !root.join("manifest.json").exists() {
            write(root, "manifest.json", &manifest);
        }
    }
    let prior = read_json(&root.join("manifest.json"));
    for field in [
        "binding",
        "clock_us",
        "profile",
        "seed",
        "size",
        "logical_sha256",
        "counts",
        "counts_by_owner_kind_lifecycle",
        "request_ids",
        "substring_corpus_query_ids",
        "expected",
        "capabilities",
    ] {
        assert_eq!(prior[field], manifest[field], "changed manifest: {field}");
    }
    if create {
        write(root, "ready.json", &ready);
    } else {
        assert_eq!(read_json(&root.join("ready.json")), ready);
    }
    manifest
}
