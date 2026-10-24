//! Explicit issue #427 sample lane. Real pinned Turso, Studio HTTP and private filesystem
//! objects; no audio admission, publication, public catalog fallback or live database access.
#![cfg(unix)]

#[path = "support/artwork_storage_http.rs"]
mod http;
#[path = "support/sample_catalog_input.rs"]
mod input;

use std::{
    env, fs,
    io::Write,
    os::unix::fs::{DirBuilderExt, OpenOptionsExt, PermissionsExt},
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
    match fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(path)
    {
        Ok(mut file) => {
            file.write_all(bytes).unwrap();
            file.sync_all().unwrap();
            fs::File::open(path.parent().unwrap())
                .unwrap()
                .sync_all()
                .unwrap();
        }
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            assert_eq!(
                fs::read(path).unwrap(),
                bytes,
                "existing sample file differs: {}",
                path.display()
            );
        }
        Err(error) => panic!("cannot create {}: {error}", path.display()),
    }
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
    let marker = root.join("sample-owner.json");
    let expected = json!({"dataset_id":input::DATASET,"input_sha256":input_hash,"root":root,"media_source":source});
    if root.exists() {
        private_dir(root);
        assert!(
            marker.is_file(),
            "existing sample root has no ownership marker; refusing adoption"
        );
        assert_eq!(
            serde_json::from_slice::<Value>(&fs::read(&marker).unwrap()).unwrap(),
            expected,
            "sample root belongs to different input/location"
        );
    } else {
        private_dir(root);
        write_once(&marker, &serde_json::to_vec_pretty(&expected).unwrap());
    }
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
        let asset_id = input::identity(&work.sample_key, "image");
        let prepare = json!({"operation_id":input::identity(&work.sample_key,"prepare-image"),"id":asset_id,"expected_sha256":cover.sha256,"expected_byte_length":cover.bytes.len(),"content_type":"image/png",
            "declarations":{"provenance":format!("Issue #427 image_gen sample; Drive file {}. Supplied claim, not verified ownership or publication clearance.",work.cover.source_drive_file_id),"rights_refs":[RIGHTS]}});
        let _: Value = host
            .json(
                "POST",
                &format!("/api/v1/studio/works/{id}/image-intents"),
                Some(token),
                None,
                Some(&prepare),
            )
            .await
            .success(StatusCode::CREATED);
        let admitted = http::request(
            &host.client,
            &host.base,
            "PUT",
            &format!("/api/v1/studio/works/{id}/image-intents/{asset_id}/bytes"),
            Some(token),
            None,
            Some("image/png"),
            cover.bytes.clone(),
            &[],
        )
        .await;
        let asset: Value = admitted.success(StatusCode::OK);
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
    let records = seed(&serving, &token, &spec, &covers).await;
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
    assert_eq!(seed(&serving, &token, &spec, &covers).await, records);
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
        let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
            .await
            .expect("sample loopback port must be free");
        println!("Sample API listening on http://127.0.0.1:8080; private Studio token remains in studio-token; Ctrl-C stops this owner");
        axum::serve(
            listener,
            api_router(AppState {
                store,
                web_origin: ORIGIN.into(),
            }),
        )
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
