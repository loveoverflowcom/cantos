//! Explicit issue #427 sample lane. Real pinned Turso, Studio HTTP and private filesystem
//! objects; no audio admission, publication, public catalog fallback or live database access.
#![cfg(unix)]

#[path = "support/artwork_storage_http.rs"]
mod http;
#[path = "support/sample_image_attempts.rs"]
mod image_attempts;
#[path = "support/sample_catalog_input.rs"]
mod input;
#[path = "support/sample_preview_http.rs"]
mod preview;
#[path = "support/sample_catalog_files.rs"]
mod sample_files;

use std::{
    env, fs,
    os::unix::fs::{DirBuilderExt, PermissionsExt},
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::http::StatusCode;
use cantos_server::{
    adapters::inbound::http::{api_router, AppState},
    platform::{operator, store::Store, turso::registry},
};
use cantos_store_turso::{
    clock::SystemClock, registry::Intent, OpenRequest, OwnedDatabase, Value as SqlValue,
};
use serde_json::{json, Value};

const ORIGIN: &str = "http://127.0.0.1:8080";
const RIGHTS: &str = "sample-catalog-artwork-rights";
const SOURCE: &str = "sample-catalog-input-v1";

fn absolute(name: &str) -> PathBuf {
    let path = PathBuf::from(env::var_os(name).unwrap_or_else(|| panic!("missing {name}")));
    assert!(path.is_absolute(), "{name} must be absolute");
    path
}

fn no_symlink(path: &Path) {
    for ancestor in path.ancestors() {
        if let Ok(metadata) = fs::symlink_metadata(ancestor) {
            assert!(
                !metadata.file_type().is_symlink(),
                "symlink path refused: {}",
                ancestor.display()
            );
        }
    }
}

fn private_dir(path: &Path) {
    if !path.exists() {
        fs::DirBuilder::new().mode(0o700).create(path).unwrap();
    }
    assert!(fs::symlink_metadata(path).unwrap().is_dir());
    assert_eq!(
        fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o700,
        "sample directories must be private"
    );
    no_symlink(path);
}

fn write_once(path: &Path, bytes: &[u8]) {
    sample_files::write_once(path, bytes).unwrap_or_else(|error| {
        panic!("cannot publish {}: {error}; existing final/staging bytes are retained. Inspect the owned output or choose a new sample root; no file was overwritten", path.display())
    });
}

fn own_root(root: &Path, source: &Path, input_hash: &str) {
    assert!(
        !root.starts_with(source) && !source.starts_with(root),
        "source/output roots overlap"
    );
    no_symlink(root);
    assert!(
        root.ancestors().all(|path| !path.join(".git").exists()),
        "sample output must be outside Git"
    );
    let expected = json!({"dataset_id":input::DATASET,"input_sha256":input_hash,"root":root,"media_source":source});
    sample_files::initialize_root(root, &serde_json::to_vec_pretty(&expected).unwrap())
        .unwrap_or_else(|error| panic!("cannot initialize {}: {error}; refusing adoption or replacement. Keep existing evidence and choose a new sample root", root.display()));
    private_dir(root);
}

async fn open_database(path: &Path, intent: Intent) -> OwnedDatabase {
    OwnedDatabase::open(OpenRequest::new(
        path,
        registry().unwrap(),
        intent,
        Arc::new(SystemClock),
    ))
    .await
    .expect("Server registry opens the run-owned Turso file")
}

async fn text_rows(database: &OwnedDatabase, sql: &str) -> Vec<String> {
    let reader = database.reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let mut values = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        let SqlValue::Text(value) = row.get_value(0).unwrap() else {
            panic!("expected text")
        };
        values.push(value);
    }
    values
}

async fn counts(database: &OwnedDatabase) -> Value {
    let reader = database.reader().await.unwrap();
    let mut result = serde_json::Map::new();
    for (name, table) in [
        ("works", "catalog_works"),
        ("chapters", "catalog_units"),
        ("images", "media_image_assets"),
        ("artwork_changes", "media_work_artwork_changes"),
        ("catalog_operations", "catalog_operations"),
        ("artwork_operations", "media_artwork_operations"),
        ("audio_submissions", "audio_submissions"),
        ("audio_renditions", "audio_renditions"),
        ("audio_releases", "audio_releases"),
        ("sources", "source_records"),
    ] {
        let mut rows = reader
            .query(&format!("SELECT count(*) FROM {table}"), ())
            .await
            .unwrap();
        let SqlValue::Integer(count) = rows.next().await.unwrap().unwrap().get_value(0).unwrap()
        else {
            panic!("expected count")
        };
        result.insert(name.into(), json!(count));
    }
    Value::Object(result)
}

async fn immutable_records(database: &OwnedDatabase) -> Value {
    let mut records = serde_json::Map::new();
    for (name,sql) in [
        ("works","SELECT hex(snapshot) FROM catalog_work_revisions ORDER BY work_id,revision"),
        ("chapters","SELECT hex(snapshot) FROM catalog_unit_revisions ORDER BY unit_id,revision"),
        ("images","SELECT storage_target || ':' || object_key || ':' || hex(document_bytes) FROM media_image_assets ORDER BY id"),
        ("artwork","SELECT hex(document_bytes) FROM media_work_artwork_changes ORDER BY work_id,work_metadata_revision"),
    ] {
        records.insert(name.into(),json!(text_rows(database,sql).await));
    }
    Value::Object(records)
}

async fn seed(
    host: &http::HttpHost,
    token: &str,
    spec: &input::Input,
    covers: &[input::CoverBytes],
    root: &Path,
    input_sha256: &str,
) -> Value {
    let mut records = Vec::new();
    for (work, cover) in spec.works.iter().zip(covers) {
        let id = input::identity(&work.sample_key, "work");
        let create = json!({"operation_id":input::identity(&work.sample_key,"create-work"),"id":id,"kind":work.kind,
            "details":{"title":work.title,"synopsis":work.description,"language":work.language,"visibility":"private","rights_refs":[]}});
        let created: Value = host
            .json(
                "POST",
                "/api/v1/studio/works",
                Some(token),
                None,
                Some(&create),
            )
            .await
            .success(StatusCode::CREATED);
        assert_eq!(created["work"]["id"], id);
        let mut revision = 1;
        let mut chapters = Vec::new();
        for chapter in &work.chapters {
            let unit_id = input::identity(&chapter.sample_key, "chapter");
            let request = json!({"operation_id":input::identity(&chapter.sample_key,"create-chapter"),"id":unit_id,"kind":"chapter","expected_work_revision":revision,
                "details":{"title":chapter.title,"description":chapter.description}});
            let result: Value = host
                .json(
                    "POST",
                    &format!("/api/v1/studio/works/{id}/content-units"),
                    Some(token),
                    None,
                    Some(&request),
                )
                .await
                .success(StatusCode::CREATED);
            revision += 1;
            assert_eq!(result["work"]["revision"], revision);
            chapters.push(json!({"sample_key":chapter.sample_key,"unit_id":unit_id,"position":chapter.position,"audio":chapter.audio,"availability":"unavailable","publication_id":null}));
        }
        let provenance=format!("Issue #427 image_gen sample; Drive file {}. Supplied claim, not verified ownership or publication clearance.",work.cover.source_drive_file_id);
        let mut journal = image_attempts::Journal::open(
            root,
            image_attempts::Plan {
                key: &work.sample_key,
                work_id: &id,
                input_sha256,
                provenance: &provenance,
            },
            cover,
        )
        .expect("image attempt journal must match this sample");
        let asset = journal
            .admit(host, token, cover)
            .await
            .expect("image admission must complete or retain a recoverable attempt");
        let asset_id = asset["id"].as_str().unwrap();
        assert_eq!(asset["verified_media"]["sha256"], cover.sha256);
        assert_eq!(asset["verified_media"]["byte_length"], cover.bytes.len());
        assert_eq!(asset["verified_media"]["width_px"], cover.width);
        assert_eq!(asset["verified_media"]["height_px"], cover.height);
        let select = json!({"operation_id":input::identity(&work.sample_key,"set-artwork"),"expected_work_revision":revision,"artwork":{"image_asset_id":asset_id,"image_asset_digest":asset["digest"],"alt_text":work.cover.alt}});
        let selection: Value = host
            .json(
                "PATCH",
                &format!("/api/v1/studio/works/{id}/artwork"),
                Some(token),
                None,
                Some(&select),
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(selection["work_revision"], 4);
        let current: Value = host
            .json::<Value>(
                "GET",
                &format!("/api/v1/studio/works/{id}"),
                Some(token),
                None,
                None,
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(current["details"], create["details"]);
        assert_eq!(current["revision"], 4);
        assert_eq!(current["lifecycle"], "draft");
        let units: Value = host
            .json::<Value>(
                "GET",
                &format!("/api/v1/studio/works/{id}/content-units"),
                Some(token),
                None,
                None,
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(units["items"].as_array().unwrap().len(), 2);
        for (chapter, unit) in work.chapters.iter().zip(units["items"].as_array().unwrap()) {
            assert_eq!(unit["id"], input::identity(&chapter.sample_key, "chapter"));
            assert_eq!(unit["details"]["title"], chapter.title);
            assert_eq!(unit["lifecycle"], "draft");
        }
        let bytes_path = format!("/api/v1/studio/works/{id}/image-assets/{asset_id}/bytes");
        let served = http::request(
            &host.client,
            &host.base,
            "GET",
            &bytes_path,
            Some(token),
            None,
            None,
            vec![],
            &[],
        )
        .await;
        assert_eq!(served.status, StatusCode::OK);
        assert_eq!(served.headers["content-type"], "image/png");
        assert_eq!(served.bytes, cover.bytes);
        let head = http::request(
            &host.client,
            &host.base,
            "HEAD",
            &bytes_path,
            Some(token),
            None,
            None,
            vec![],
            &[],
        )
        .await;
        assert_eq!(head.status, StatusCode::OK);
        assert_eq!(
            head.headers["content-length"],
            cover.bytes.len().to_string()
        );
        assert!(head.bytes.is_empty());
        http::request(
            &host.client,
            &host.base,
            "GET",
            &bytes_path,
            None,
            None,
            None,
            vec![],
            &[],
        )
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
        host.json::<Value>(
            "GET",
            &format!("/api/v1/theatre/works/{id}"),
            None,
            None,
            None,
        )
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
        records.push(json!({"sample_key":work.sample_key,"work_id":id,"work_revision":4,"lifecycle":"draft","visibility":"private","cover":{"asset":asset,"alt":work.cover.alt,"authenticated_http_path":bytes_path,"object_key":cover.sha256,"storage_target":"sample-filesystem"},"chapters":chapters}));
    }
    let public: Value = host
        .json::<Value>("GET", "/api/v1/theatre/works", None, None, None)
        .await
        .success(StatusCode::OK);
    assert_eq!(public["items"], json!([]));
    let private: Value = host
        .json::<Value>("GET", "/api/v1/studio/works", Some(token), None, None)
        .await
        .success(StatusCode::OK);
    assert_eq!(private["items"].as_array().unwrap().len(), 6);
    Value::Array(records)
}

async fn verify_failure_boundary(
    store: &Store,
    token: &str,
    root: &Path,
    records: &Value,
    expected: &[u8],
) {
    let cover = &records[0]["cover"];
    let path = cover["authenticated_http_path"].as_str().unwrap();
    let original = root
        .join("media/objects")
        .join(cover["object_key"].as_str().unwrap());
    assert_eq!(fs::read(&original).unwrap(), expected);
    // Negative controls receive an independent filesystem location under the same trusted
    // target ID and share the existing DB owner. Even SIGKILL cannot alter final sample media.
    let run = cantos_store_turso::testing::TestRun::new("sample-negative-media");
    let probe_root = fs::canonicalize(run.dir()).unwrap();
    private_dir(&probe_root.join("media"));
    write_topology(&probe_root);
    let probe_store = configured(store.clone(), &probe_root);
    let serving = host(&probe_store).await;
    let object = probe_root
        .join("media/objects")
        .join(cover["object_key"].as_str().unwrap());
    fs::write(&object, expected).unwrap();
    fs::set_permissions(&object, fs::Permissions::from_mode(0o400)).unwrap();
    let copy = http::request(
        &serving.client,
        &serving.base,
        "GET",
        path,
        Some(token),
        None,
        None,
        vec![],
        &[],
    )
    .await;
    assert_eq!(copy.status, StatusCode::OK);
    assert_eq!(copy.bytes, expected);
    fs::remove_file(&object).unwrap();
    http::request(
        &serving.client,
        &serving.base,
        "GET",
        path,
        Some(token),
        None,
        None,
        vec![],
        &[],
    )
    .await
    .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
    let mut corrupt = expected.to_vec();
    corrupt[0] ^= 1;
    fs::write(&object, corrupt).unwrap();
    fs::set_permissions(&object, fs::Permissions::from_mode(0o400)).unwrap();
    http::request(
        &serving.client,
        &serving.base,
        "GET",
        path,
        Some(token),
        None,
        None,
        vec![],
        &[],
    )
    .await
    .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
    for route in [
        format!("/objects/{}", cover["object_key"].as_str().unwrap()),
        format!("/media/objects/{}", cover["object_key"].as_str().unwrap()),
    ] {
        // Host fallback paths are outside the API policy layer. Assert absence directly;
        // the API helper's mandatory security-header assertions still cover all API routes.
        let response = serving
            .client
            .get(format!("{}{route}", serving.base))
            .send()
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            StatusCode::NOT_FOUND,
            "private object path {route}"
        );
        assert_ne!(
            response
                .headers()
                .get("content-type")
                .and_then(|value| value.to_str().ok()),
            Some("image/png")
        );
        let body: Value = serde_json::from_slice(&response.bytes().await.unwrap()).unwrap();
        assert_eq!(body["code"], "not_found");
    }
    serving.stop().await;
    drop(probe_store);
    assert_eq!(
        fs::read(original).unwrap(),
        expected,
        "negative controls changed final media"
    );
}

fn write_topology(root: &Path) {
    let topology = format!("version=1\n[storage]\nactive_write_target='sample-filesystem'\n[storage.targets.sample-filesystem]\ndriver='filesystem'\nprivate_root={}\n",serde_json::to_string(&root.join("media")).unwrap());
    write_once(&root.join("storage.toml"), topology.as_bytes());
}

fn configured(store: Store, root: &Path) -> Store {
    store
        .with_storage_config(&root.join("storage.toml"))
        .unwrap()
}

async fn host(store: &Store) -> http::HttpHost {
    http::HttpHost::start(api_router(AppState {
        store: store.clone(),
        web_origin: ORIGIN.into(),
    }))
    .await
}

#[tokio::test]
#[ignore = "explicit external-input sample lane; use cargo xtask seed theatre --sample-input ... --sample-root ..."]
async fn sample_catalog() {
    let input_path = absolute("CANTOS_SAMPLE_INPUT");
    let source = fs::canonicalize(absolute("CANTOS_SAMPLE_MEDIA_SOURCE")).unwrap();
    let requested_root = absolute("CANTOS_SAMPLE_ROOT");
    no_symlink(&input_path);
    no_symlink(&source);
    no_symlink(&requested_root);
    let root = fs::canonicalize(requested_root.parent().unwrap())
        .unwrap()
        .join(requested_root.file_name().unwrap());
    let input_bytes = fs::read(&input_path).unwrap();
    let spec = input::parse(&input_bytes).expect("exact six/twelve supplied sample input");
    // All source bytes, hashes, title mappings and complete pixel decoding precede DB effects.
    let covers = input::load_covers(&spec, &source)
        .await
        .expect("all six supplied PNGs must match the pinned package checksums and decode");
    let input_hash = input::hash(&input_bytes);
    own_root(&root, &source, &input_hash);
    private_dir(&root.join("media"));
    private_dir(&root.join("covers"));
    write_once(&root.join("sample-catalog.json"), &input_bytes);
    for (work, cover) in spec.works.iter().zip(&covers) {
        write_once(&root.join(&work.cover.relative_path), &cover.bytes);
    }
    write_topology(&root);

    let database_path = root.join("server.db");
    let inspection = open_database(&database_path, Intent::Migrate).await;
    let actors = text_rows(&inspection, "SELECT id FROM actors ORDER BY id").await;
    assert!(actors.len() <= 1, "unexpected actor in isolated sample DB");
    let sources = text_rows(
        &inspection,
        "SELECT original_text FROM source_records WHERE id='sample-catalog-input-v1'",
    )
    .await;
    if let Some(prior) = sources.first() {
        assert_eq!(
            prior.as_bytes(),
            input_bytes,
            "stored sample source differs"
        );
    }
    let rights = text_rows(
        &inspection,
        "SELECT id FROM script_evidence WHERE kind='rights' AND id='sample-catalog-artwork-rights'",
    )
    .await;
    drop(inspection);
    let store = configured(
        Store::open(&database_path, Intent::Serve).await.unwrap(),
        &root,
    );
    let actor = match actors.first() {
        Some(actor) => actor.clone(),
        None => operator::create_creator(&store).await.unwrap(),
    };
    write_once(
        &root.join("creator.json"),
        &serde_json::to_vec_pretty(&json!({"actor_id":actor})).unwrap(),
    );
    if rights.is_empty() {
        operator::register_evidence(&store,&actor,"rights",RIGHTS,"Issue #427 supplied image_gen sample provenance/rights claim; not independent rights verification, QC or publication clearance").await.unwrap();
    }
    if sources.is_empty() {
        operator::record_source(
            &store,
            &actor,
            SOURCE,
            "sample://cantos-sample-catalog-v1/input",
            std::str::from_utf8(&input_bytes).unwrap(),
        )
        .await
        .unwrap();
    }
    operator::revoke_tokens(&store, &actor).await.unwrap();
    let token = operator::issue_token(&store, &actor, 1).await.unwrap();
    // Private operator convenience only. Tokens never enter the manifest or test output.
    let token_path = root.join("studio-token");
    if token_path.exists() {
        assert!(!fs::symlink_metadata(&token_path)
            .unwrap()
            .file_type()
            .is_symlink());
        fs::remove_file(&token_path).unwrap();
    }
    write_once(&token_path, token.as_bytes());
    let serving = host(&store).await;
    let records = seed(&serving, &token, &spec, &covers, &root, &input_hash).await;
    serving.stop().await;
    verify_failure_boundary(&store, &token, &root, &records, &covers[0].bytes).await;
    drop(store);

    let inspection = open_database(&database_path, Intent::Serve).await;
    let before = counts(&inspection).await;
    let immutable = immutable_records(&inspection).await;
    assert_eq!(before["works"], 6);
    assert_eq!(before["chapters"], 12);
    assert_eq!(before["images"], 6);
    assert_eq!(before["artwork_changes"], 6);
    assert_eq!(before["audio_submissions"], 0);
    assert_eq!(before["audio_renditions"], 0);
    assert_eq!(before["audio_releases"], 0);
    assert_eq!(before["sources"], 1);
    drop(inspection);
    // A new Server owner/router must read the same data and replay every original operation.
    let reopened = configured(
        Store::open(&database_path, Intent::Serve).await.unwrap(),
        &root,
    );
    let serving = host(&reopened).await;
    assert_eq!(
        seed(&serving, &token, &spec, &covers, &root, &input_hash).await,
        records
    );
    serving.stop().await;
    drop(reopened);
    let inspection = open_database(&database_path, Intent::Serve).await;
    assert_eq!(
        counts(&inspection).await,
        before,
        "reseed must not create revisions, receipts or duplicate rows"
    );
    assert_eq!(
        immutable_records(&inspection).await,
        immutable,
        "reseed changed accepted immutable metadata"
    );
    let snapshot = root.join("sample.db");
    if !snapshot.exists() {
        inspection
            .backup_to(&snapshot)
            .await
            .expect("engine-supported verified snapshot");
    }
    drop(inspection);
    let copied = open_database(&snapshot, Intent::Serve).await;
    assert_eq!(counts(&copied).await, before);
    assert_eq!(
        immutable_records(&copied).await,
        immutable,
        "snapshot differs from accepted IDs, revisions, artwork or media locations"
    );
    assert_eq!(
        text_rows(
            &copied,
            "SELECT original_text FROM source_records WHERE id='sample-catalog-input-v1'"
        )
        .await,
        sources_if_new(&input_bytes)
    );
    drop(copied);
    let manifest = json!({"dataset_id":spec.dataset_id,"input_sha256":input_hash,"input":serde_json::from_slice::<Value>(&input_bytes).unwrap(),"counts":before,"works":records,
        "database":{"runtime":"server.db","snapshot":"sample.db","snapshot_sha256":input::hash(&fs::read(&snapshot).unwrap()),"snapshot_byte_length":fs::metadata(&snapshot).unwrap().len(),"registry":"cantos-server epoch 1","engine":"pinned cantos-store-turso"},
        "evidence":{"real_turso_reopen":"PASS","idempotent_http_reseed":"PASS","authenticated_loopback_covers":"PASS","anonymous_cover_denial":"PASS","missing_corrupt_object":"PASS","public_catalog":"empty: private drafts with no audio release","web":"NOT_RUN","android":"NOT_RUN","ios":"NOT_RUN","audio_playback":"NOT_RUN: mock URLs have no bytes"}});
    let bytes = serde_json::to_vec_pretty(&manifest).unwrap();
    if root.join("manifest.json").exists() {
        assert_eq!(
            fs::read(root.join("manifest.json")).unwrap(),
            bytes,
            "stable sample manifest changed"
        );
    } else {
        write_once(&root.join("manifest.json"), &bytes);
    }
    println!("PASS sample catalog: 6 private works, 12 draft chapters, 6 admitted PNGs; reopened, reseeded and verified snapshot at {}",root.display());
    if env::var("CANTOS_SAMPLE_SERVE").as_deref() == Ok("1") {
        let store = configured(
            Store::open(&database_path, Intent::Serve).await.unwrap(),
            &root,
        );
        // The canonical router answers every path it always did; the local preview routes sit in
        // front of it, read through its Studio routes and are never mounted by a production host.
        let canonical = api_router(AppState {
            store,
            web_origin: ORIGIN.into(),
        });
        let dataset = preview::Preview::new(canonical, &token, preview::members(&spec));
        dataset
            .verify()
            .await
            .expect("the Store must hold exactly the declared sample dataset");
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
            .await
            .expect("sample loopback port must be free");
        println!("Sample API listening on http://127.0.0.1:8080; private Studio token remains in studio-token; Ctrl-C stops this owner");
        println!("Local preview reads: {}/works (Theatre Web: TRUNK_BUILD_FEATURES=preview cargo xtask app cantos-theatre --web, then open http://127.0.0.1:8082/?mode=local-db#discover)", preview::NAMESPACE);
        axum::serve(listener, dataset.router())
            .with_graceful_shutdown(async {
                tokio::signal::ctrl_c().await.unwrap();
            })
            .await
            .unwrap();
    }
}

fn sources_if_new(bytes: &[u8]) -> Vec<String> {
    vec![std::str::from_utf8(bytes).unwrap().into()]
}

#[tokio::test]
async fn independent_png_decoder_rejects_corruption_and_decodes_actual_pixels() {
    let png = include_bytes!("../src/contexts/media/image_intake/fixtures/python-rgb-2x3.png");
    assert_eq!(input::decode(png.to_vec()).await, Ok((2, 3)));
    assert_eq!(
        input::decode(vec![1, 2, 3]).await,
        Err(input::InputError::ImageRejected("unsupported_image_format"))
    );
}

#[test]
fn operation_identity_is_stable_scoped_and_canonical() {
    let work = input::identity("ben-suong", "work");
    assert_eq!(work, input::identity("ben-suong", "work"));
    assert_ne!(work, input::identity("ben-suong", "image"));
    assert_ne!(work, input::identity("nguoi-giu-den", "work"));
    assert_eq!(uuid::Uuid::parse_str(&work).unwrap().to_string(), work);
}

#[test]
fn input_refuses_wrong_dataset_before_any_external_effect() {
    assert!(matches!(input::parse(b"{}"), Err(input::InputError::Json)));
    assert!(matches!(
        input::parse(br#"{"dataset_id":"other","story_count":6,"chapter_count":12,"works":[]}"#),
        Err(input::InputError::Dataset)
    ));
    assert!(matches!(input::parse(br#"{"dataset_id":"cantos-sample-catalog-v1","story_count":6,"chapter_count":12,"works":[]}"#),Err(input::InputError::Counts)));
}

fn declared_input() -> Value {
    let works:Vec<_>=input::COVERS.iter().map(|(key,title,_,_)|json!({
        "sample_key":key,"title":title,"kind":"story","description":"Synthetic parser test","language":"vi-VN",
        "author":"Fictional Author","narrator":"Not recorded","genre":"Synthetic","tags":["sample"],
        "cover":{"relative_path":format!("covers/{key}.png"),"source_drive_file_id":"synthetic-parser-only","alt":title},
        "chapters":(0..2).map(|position|json!({"sample_key":format!("{key}/chapter-{:02}",position+1),"position":position,"title":"Synthetic chapter","description":"Parser test only",
            "audio":{"mock_url":format!("https://media.example.invalid/cantos-samples/{key}/chapter-{:02}.mp3",position+1),"is_mock":true,"file_exists":false,"mime_type":"audio/mpeg","duration_ms":null,"sha256":null,"byte_length":null,"playback_verified":false}})).collect::<Vec<_>>()
    })).collect();
    json!({"dataset_id":input::DATASET,"story_count":6,"chapter_count":12,"works":works})
}

#[test]
fn input_preserves_six_twelve_mapping_and_refuses_fabricated_audio_claims() {
    let original = declared_input();
    let parsed = input::parse(&serde_json::to_vec(&original).unwrap()).unwrap();
    assert_eq!(parsed.works.len(), 6);
    assert_eq!(
        parsed
            .works
            .iter()
            .map(|work| work.chapters.len())
            .sum::<usize>(),
        12
    );
    for (path, value, expected) in [
        (
            "/works/0/cover/relative_path",
            json!("../private.png"),
            input::InputError::Mapping,
        ),
        (
            "/works/1/sample_key",
            json!("ben-suong"),
            input::InputError::Mapping,
        ),
        (
            "/works/0/chapters/0/audio/sha256",
            json!("0".repeat(64)),
            input::InputError::AudioClaim,
        ),
        (
            "/works/0/chapters/0/audio/file_exists",
            json!(true),
            input::InputError::AudioClaim,
        ),
        (
            "/works/0/chapters/0/audio/playback_verified",
            json!(true),
            input::InputError::AudioClaim,
        ),
        (
            "/works/0/chapters/0/audio/duration_ms",
            json!(1000),
            input::InputError::AudioClaim,
        ),
    ] {
        let mut candidate = original.clone();
        *candidate.pointer_mut(path).unwrap() = value;
        assert_eq!(
            input::parse(&serde_json::to_vec(&candidate).unwrap()).err(),
            Some(expected)
        );
    }
}

#[tokio::test]
async fn missing_or_replaced_source_is_refused_without_creating_a_database() {
    let run = cantos_store_turso::testing::TestRun::new("sample-input");
    let parsed = input::parse(&serde_json::to_vec(&declared_input()).unwrap()).unwrap();
    let absent = input::load_covers(&parsed, run.dir()).await.err();
    assert_eq!(
        absent,
        Some(input::InputError::SourceMissing(
            "covers/ben-suong.png".into()
        ))
    );
    fs::create_dir(run.path("covers")).unwrap();
    fs::write(
        run.path("covers/ben-suong.png"),
        include_bytes!("../src/contexts/media/image_intake/fixtures/python-rgb-2x3.png"),
    )
    .unwrap();
    assert_eq!(
        input::load_covers(&parsed, run.dir()).await.err(),
        Some(input::InputError::SourceIdentity(
            "covers/ben-suong.png".into()
        ))
    );
    fs::write(
        run.path("covers/ben-suong.png"),
        vec![0u8; input::COVERS[0].3],
    )
    .unwrap();
    assert_eq!(
        input::load_covers(&parsed, run.dir()).await.err(),
        Some(input::InputError::SourceIdentity(
            "covers/ben-suong.png".into()
        ))
    );
    assert!(!run.path("server.db").exists());
}

const RECOVERY_TOKEN: &str = "abababababababababababababababababababababababababababababababab";

async fn recovery_fixture() -> (
    cantos_server::test_support::ServerFixture,
    Arc<cantos_store_turso::testing::FixedClock>,
    PathBuf,
    input::CoverBytes,
) {
    let clock = cantos_store_turso::testing::FixedClock::new(1_800_000_000_000_000);
    let fixture = cantos_server::test_support::ServerFixture::with_clock(clock.clone()).await;
    fixture.seed_actors(&["alice"]).await;
    fixture
        .seed_development_session("alice", RECOVERY_TOKEN, 2)
        .await;
    operator::register_evidence(
        fixture.store(),
        "alice",
        "rights",
        RIGHTS,
        "Synthetic pixel fixture; recovery test only, no publication clearance",
    )
    .await
    .unwrap();
    let root = fs::canonicalize(fixture.dir()).unwrap();
    private_dir(&root.join("media"));
    write_topology(&root);
    let bytes =
        include_bytes!("../src/contexts/media/image_intake/fixtures/python-rgb-2x3.png").to_vec();
    let cover = input::CoverBytes {
        sha256: input::hash(&bytes),
        bytes,
        width: 2,
        height: 3,
    };
    (fixture, clock, root, cover)
}

async fn recovery_work(serving: &http::HttpHost) -> String {
    let work = input::identity("ben-suong", "work");
    let create = json!({"operation_id":input::identity("ben-suong","create-work"),"id":work,"kind":"story","details":{"title":"Synthetic recovery fixture","synopsis":"No audio or publication","language":"vi-VN","visibility":"private","rights_refs":[]}});
    let _: Value = serving
        .json(
            "POST",
            "/api/v1/studio/works",
            Some(RECOVERY_TOKEN),
            None,
            Some(&create),
        )
        .await
        .success(StatusCode::CREATED);
    work
}

fn recovery_journal(root: &Path, work: &str, cover: &input::CoverBytes) -> image_attempts::Journal {
    image_attempts::Journal::open(
        root,
        image_attempts::Plan {
            key: "ben-suong",
            work_id: work,
            input_sha256: "synthetic-recovery-input",
            provenance: "Independent Python 2x3 synthetic pixels; recovery test only",
        },
        cover,
    )
    .unwrap()
}

#[tokio::test]
async fn expired_unadmitted_image_attempt_recovers_after_owner_restart_without_duplicate_asset() {
    let (mut fixture, clock, root, cover) = recovery_fixture().await;
    let store = configured(fixture.store().clone(), &root);
    let serving = host(&store).await;
    let work = recovery_work(&serving).await;
    let journal = recovery_journal(&root, &work, &cover);
    let first = journal.current_request().clone();
    let _: Value = serving
        .json(
            "POST",
            &format!("/api/v1/studio/works/{work}/image-intents"),
            Some(RECOVERY_TOKEN),
            None,
            Some(&first),
        )
        .await
        .success(StatusCode::CREATED);
    // The process stops after prepare committed, before any bytes were submitted.
    drop(journal);
    serving.stop().await;
    drop(store);
    clock.advance(31 * 60 * 1_000_000);
    fixture.restart().await;
    let store = configured(fixture.store().clone(), &root);
    let serving = host(&store).await;
    let expired_path = format!(
        "/api/v1/studio/works/{work}/image-intents/{}/bytes",
        first["id"].as_str().unwrap()
    );
    let refused = http::request(
        &serving.client,
        &serving.base,
        "PUT",
        &expired_path,
        Some(RECOVERY_TOKEN),
        None,
        Some("image/png"),
        cover.bytes.clone(),
        &[],
    )
    .await
    .error(StatusCode::CONFLICT, "invalid_request");
    assert_eq!(
        refused["issues"],
        json!([{"path":"intent","rule":"intent_expired"}])
    );
    let mut journal = recovery_journal(&root, &work, &cover);
    let accepted = journal
        .admit(&serving, RECOVERY_TOKEN, &cover)
        .await
        .unwrap();
    assert_ne!(accepted["id"], first["id"]);
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_image_intents", &[])
            .await,
        2
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_image_assets", &[])
            .await,
        1
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_artwork_operations", &[])
            .await,
        2
    );
    drop(journal);
    serving.stop().await;
    drop(store);
    fixture.restart().await;
    let store = configured(fixture.store().clone(), &root);
    let serving = host(&store).await;
    let mut journal = recovery_journal(&root, &work, &cover);
    assert_eq!(
        journal
            .admit(&serving, RECOVERY_TOKEN, &cover)
            .await
            .unwrap(),
        accepted
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_image_intents", &[])
            .await,
        2
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_image_assets", &[])
            .await,
        1
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_artwork_operations", &[])
            .await,
        2
    );
    serving.stop().await;
}

#[tokio::test]
async fn admitted_image_with_unrecorded_response_is_reconciled_after_expiry_without_new_attempt() {
    let (mut fixture, clock, root, cover) = recovery_fixture().await;
    let store = configured(fixture.store().clone(), &root);
    let serving = host(&store).await;
    let work = recovery_work(&serving).await;
    let journal = recovery_journal(&root, &work, &cover);
    let request = journal.current_request().clone();
    let _: Value = serving
        .json(
            "POST",
            &format!("/api/v1/studio/works/{work}/image-intents"),
            Some(RECOVERY_TOKEN),
            None,
            Some(&request),
        )
        .await
        .success(StatusCode::CREATED);
    let accepted: Value = http::request(
        &serving.client,
        &serving.base,
        "PUT",
        &format!(
            "/api/v1/studio/works/{work}/image-intents/{}/bytes",
            request["id"].as_str().unwrap()
        ),
        Some(RECOVERY_TOKEN),
        None,
        Some("image/png"),
        cover.bytes.clone(),
        &[],
    )
    .await
    .success(StatusCode::OK);
    // Deliberately stop before recording the successful response in the local journal.
    drop(journal);
    serving.stop().await;
    drop(store);
    clock.advance(31 * 60 * 1_000_000);
    fixture.restart().await;
    let store = configured(fixture.store().clone(), &root);
    let serving = host(&store).await;
    let mut journal = recovery_journal(&root, &work, &cover);
    assert_eq!(
        journal
            .admit(&serving, RECOVERY_TOKEN, &cover)
            .await
            .unwrap(),
        accepted
    );
    assert_eq!(journal.current_request(), &request);
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_image_intents", &[])
            .await,
        1
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_image_assets", &[])
            .await,
        1
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM media_artwork_operations", &[])
            .await,
        1
    );
    serving.stop().await;
}

async fn preview_get(serving: &http::HttpHost, path: &str, extra: &[(&str, &str)]) -> http::Reply {
    let mut headers = vec![
        ("host", "127.0.0.1:8082"),
        ("origin", "http://127.0.0.1:8082"),
    ];
    for &(name, value) in extra {
        headers.retain(|(existing, _)| *existing != name);
        headers.push((name, value));
    }
    http::request(
        &serving.client,
        &serving.base,
        "GET",
        path,
        None,
        None,
        None,
        vec![],
        &headers,
    )
    .await
}

#[tokio::test]
async fn sample_preview_reads_current_turso_revisions_enforces_dataset_membership_and_keeps_canonical_router_closed(
) {
    let (fixture, _clock, root, cover) = recovery_fixture().await;
    let store = configured(fixture.store().clone(), &root);
    let canonical_host = host(&store).await;
    // Canonical router never mounts the local preview route.
    preview_get(&canonical_host, "/api/v1/sample-preview/works", &[])
        .await
        .error(StatusCode::NOT_FOUND, "not_found");

    let work = recovery_work(&canonical_host).await;
    let chapter_one = input::identity("ben-suong/chapter-01", "chapter");
    let chapter_two = input::identity("ben-suong/chapter-02", "chapter");
    for (index, (unit_id, title, description)) in [
        (
            &chapter_one,
            "Chương 1: Bến đò sương",
            "Mở đầu bên bến sông",
        ),
        (
            &chapter_two,
            "Chương 2: Lời hẹn cũ",
            "Tiếng gọi dưới gốc đa",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let create_unit = json!({
            "operation_id": input::identity(&format!("ben-suong/chapter-0{}", index + 1), "create-chapter"),
            "id": unit_id,
            "kind": "chapter",
            "expected_work_revision": index + 1,
            "details": {"title": title, "description": description},
        });
        let _: Value = canonical_host
            .json(
                "POST",
                &format!("/api/v1/studio/works/{work}/content-units"),
                Some(RECOVERY_TOKEN),
                None,
                Some(&create_unit),
            )
            .await
            .success(StatusCode::CREATED);
    }
    let mut journal = recovery_journal(&root, &work, &cover);
    let asset = journal
        .admit(&canonical_host, RECOVERY_TOKEN, &cover)
        .await
        .unwrap();
    let select = json!({
        "operation_id": input::identity("ben-suong", "set-artwork"),
        "expected_work_revision": 3,
        "artwork": {
            "image_asset_id": asset["id"],
            "image_asset_digest": asset["digest"],
            "alt_text": "Bến Sương bìa gốc",
        },
    });
    let _: Value = canonical_host
        .json(
            "PATCH",
            &format!("/api/v1/studio/works/{work}/artwork"),
            Some(RECOVERY_TOKEN),
            None,
            Some(&select),
        )
        .await
        .success(StatusCode::OK);

    // Seed a foreign private Work in the same Store to prove dataset membership isolation.
    let foreign_work = "90000000-0000-4000-8000-000000000099";
    let create_foreign = json!({
        "operation_id": "90000000-0000-4000-8000-000000000098",
        "id": foreign_work,
        "kind": "story",
        "details": {
            "title": "Truyện ngoài bộ mẫu",
            "synopsis": "Không thuộc sample dataset",
            "language": "vi-VN",
            "visibility": "private",
            "rights_refs": [],
        },
    });
    let _: Value = canonical_host
        .json(
            "POST",
            "/api/v1/studio/works",
            Some(RECOVERY_TOKEN),
            None,
            Some(&create_foreign),
        )
        .await
        .success(StatusCode::CREATED);
    canonical_host.stop().await;

    let members = vec![preview::Member {
        work_id: work.clone(),
        unit_ids: vec![chapter_one.clone(), chapter_two.clone()],
    }];
    let preview_service = preview::Preview::new(
        api_router(AppState {
            store: store.clone(),
            web_origin: ORIGIN.into(),
        }),
        RECOVERY_TOKEN,
        members.clone(),
    );
    preview_service.verify().await.unwrap();
    let serving = http::HttpHost::start(preview_service.router()).await;

    // Public discovery on the same host still hides the private draft Work.
    let public_works: Value = serving
        .json::<Value>("GET", "/api/v1/theatre/works", None, None, None)
        .await
        .success(StatusCode::OK);
    assert_eq!(public_works["items"], json!([]));

    // Preview list and detail read from Turso without exposing Studio credentials or storage paths.
    let list_reply = preview_get(&serving, "/api/v1/sample-preview/works", &[]).await;
    let list_body = String::from_utf8(list_reply.bytes.clone()).unwrap();
    assert!(!list_body.contains(RECOVERY_TOKEN));
    assert!(!list_body.contains("sample-filesystem"));
    assert!(!list_body.contains("rights"));
    let listed: Value = list_reply.success(StatusCode::OK);
    assert_eq!(listed["schema"], preview::SCHEMA);
    assert_eq!(listed["dataset_id"], input::DATASET);
    assert_eq!(listed["items"].as_array().unwrap().len(), 1);
    assert_eq!(listed["items"][0]["work_id"], work);
    assert_eq!(listed["items"][0]["chapter_count"], 2);
    let cover_path = listed["items"][0]["cover"]["path"]
        .as_str()
        .unwrap()
        .to_owned();
    assert_eq!(
        cover_path,
        format!("/api/v1/sample-preview/works/{work}/cover/{}", cover.sha256)
    );

    let detail: Value = preview_get(
        &serving,
        &format!("/api/v1/sample-preview/works/{work}"),
        &[],
    )
    .await
    .success(StatusCode::OK);
    assert_eq!(detail["work"], listed["items"][0]);
    assert_eq!(
        detail["chapters"],
        json!([
            {
                "unit_id": chapter_one,
                "kind": "chapter",
                "position": 1,
                "title": "Chương 1: Bến đò sương",
                "description": "Mở đầu bên bến sông",
                "audio": "unavailable",
            },
            {
                "unit_id": chapter_two,
                "kind": "chapter",
                "position": 2,
                "title": "Chương 2: Lời hẹn cũ",
                "description": "Tiếng gọi dưới gốc đa",
                "audio": "unavailable",
            }
        ])
    );

    // Cover GET and HEAD serve verified bytes and reject foreign digests.
    let cover_reply = preview_get(&serving, &cover_path, &[]).await;
    assert_eq!(cover_reply.status, StatusCode::OK);
    assert_eq!(cover_reply.headers["content-type"], "image/png");
    assert_eq!(cover_reply.bytes, cover.bytes);
    let head_reply = http::request(
        &serving.client,
        &serving.base,
        "HEAD",
        &cover_path,
        None,
        None,
        None,
        vec![],
        &[
            ("host", "127.0.0.1:8082"),
            ("origin", "http://127.0.0.1:8082"),
        ],
    )
    .await;
    assert_eq!(head_reply.status, StatusCode::OK);
    assert_eq!(
        head_reply.headers["content-length"],
        cover.bytes.len().to_string()
    );
    assert!(head_reply.bytes.is_empty());

    // Foreign Work ID, foreign cover digest, query string, and foreign Host/Origin are refused.
    preview_get(
        &serving,
        &format!("/api/v1/sample-preview/works/{foreign_work}"),
        &[],
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    preview_get(
        &serving,
        &format!(
            "/api/v1/sample-preview/works/{work}/cover/{}",
            "0".repeat(64)
        ),
        &[],
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    preview_get(&serving, "/api/v1/sample-preview/works?extra=1", &[])
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    // Trunk's reverse proxy sends Host: 127.0.0.1 without port; foreign Host/Origin are refused.
    let proxied: Value = preview_get(
        &serving,
        "/api/v1/sample-preview/works",
        &[("host", "127.0.0.1")],
    )
    .await
    .success(StatusCode::OK);
    assert_eq!(proxied["items"].as_array().unwrap().len(), 1);
    preview_get(
        &serving,
        "/api/v1/sample-preview/works",
        &[("host", "evil.example:8080")],
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    preview_get(
        &serving,
        "/api/v1/sample-preview/works",
        &[("origin", "http://evil.example")],
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");

    // DB provenance oracle: updating Work and Chapter metadata through Studio immediately
    // changes the preview response, proving it reads current Turso revisions rather than JSON.
    let patch_work = json!({
        "operation_id": "90000000-0000-4000-8000-000000000001",
        "expected_revision": 4,
        "details": {
            "title": "Bến Sương (Bản sửa DB)",
            "synopsis": "Mô tả đọc trực tiếp từ Turso revision mới.",
        },
    });
    let _: Value = serving
        .json(
            "PATCH",
            &format!("/api/v1/studio/works/{work}"),
            Some(RECOVERY_TOKEN),
            None,
            Some(&patch_work),
        )
        .await
        .success(StatusCode::OK);
    let patch_chapter = json!({
        "operation_id": "90000000-0000-4000-8000-000000000002",
        "expected_revision": 1,
        "expected_work_revision": 5,
        "details": {
            "title": "Chương 1: Đã cập nhật trong Turso",
            "description": "Nội dung mô tả chương mới từ DB",
            "source": null,
            "content_digest": null,
        },
    });
    let _: Value = serving
        .json(
            "PATCH",
            &format!("/api/v1/studio/content-units/{chapter_one}"),
            Some(RECOVERY_TOKEN),
            None,
            Some(&patch_chapter),
        )
        .await
        .success(StatusCode::OK);

    let updated: Value = preview_get(
        &serving,
        &format!("/api/v1/sample-preview/works/{work}"),
        &[],
    )
    .await
    .success(StatusCode::OK);
    assert_eq!(updated["work"]["title"], "Bến Sương (Bản sửa DB)");
    assert_eq!(
        updated["work"]["description"],
        "Mô tả đọc trực tiếp từ Turso revision mới."
    );
    assert_eq!(
        updated["chapters"][0]["title"],
        "Chương 1: Đã cập nhật trong Turso"
    );
    assert_eq!(
        updated["chapters"][0]["description"],
        "Nội dung mô tả chương mới từ DB"
    );

    // Corrupting the private cover object fails closed without leaking filesystem paths.
    let object = root.join("media/objects").join(&cover.sha256);
    fs::remove_file(&object).unwrap();
    let missing_cover = preview_get(&serving, &cover_path, &[])
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "sample_mismatch");
    assert!(!missing_cover.to_string().contains("media/objects"));
    fs::write(&object, &cover.bytes).unwrap();
    fs::set_permissions(&object, fs::Permissions::from_mode(0o400)).unwrap();

    // Adding an extra chapter outside the declared sample membership fails closed.
    let extra_chapter = json!({
        "operation_id": "90000000-0000-4000-8000-000000000003",
        "id": "90000000-0000-4000-8000-000000000004",
        "kind": "chapter",
        "expected_work_revision": 6,
        "details": {"title": "Chương lạ", "description": "Không thuộc bộ mẫu"},
    });
    let _: Value = serving
        .json(
            "POST",
            &format!("/api/v1/studio/works/{work}/content-units"),
            Some(RECOVERY_TOKEN),
            None,
            Some(&extra_chapter),
        )
        .await
        .success(StatusCode::CREATED);
    preview_get(
        &serving,
        &format!("/api/v1/sample-preview/works/{work}"),
        &[],
    )
    .await
    .error(StatusCode::INTERNAL_SERVER_ERROR, "sample_mismatch");

    serving.stop().await;
}
