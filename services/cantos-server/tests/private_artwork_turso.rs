//! L5 acceptance sources: actual loopback HTTP → production artwork service → PostgreSQL
//! application role and the shared private-filesystem registry. Ordinary Rust runs ignore
//! these tests. Compilation or authored assertions are not database/storage acceptance.
//! Use only `cargo xtask verify postgres`, which owns a fresh disposable cluster.
#![cfg(unix)]

#[path = "support/artwork_storage_http.rs"]
mod http;

use std::{
    env, fs,
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::{Path, PathBuf},
};

use axum::http::StatusCode;
use cantos_server::contract::studio as dto;
use cantos_server::{
    adapters::inbound::http::{router, AppState},
    contexts::catalog::{
        domain::{OwnerId, WorkId},
        CatalogCommand, MemberRole,
    },
    platform::store::{token_hash, Store},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[path = "support/native_sql.rs"]
mod native_sql;
use native_sql::NativeSql;
use uuid::Uuid;

use http::{request, HttpHost, Reply};

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CAROL: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const EXPIRED: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const ALICE_FENCE: &str = "00000000-0000-4000-8000-000000000001";
const BOB_FENCE: &str = "00000000-0000-4000-8000-000000000002";
const CAROL_FENCE: &str = "00000000-0000-4000-8000-000000000003";
const EXPIRED_FENCE: &str = "00000000-0000-4000-8000-000000000004";
const ORIGIN: &str = "http://127.0.0.1:8080";
const FIRST_TARGET: &str = "artwork-first";
const NEXT_TARGET: &str = "artwork-next";

#[derive(Clone, Copy)]
struct Fixture {
    bytes: &'static [u8],
    sha256: &'static str,
    mime: dto::ImageMime,
    content_type: &'static str,
}

// The three existing 2x3 fixtures were independently generated with Python struct/zlib
// and Pillow; expected hashes are recorded independently of the production decoder.
const FIXTURES: [Fixture; 3] = [
    Fixture {
        bytes: include_bytes!("../src/contexts/media/image_intake/fixtures/python-rgb-2x3.png"),
        sha256: "7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276",
        mime: dto::ImageMime::Png,
        content_type: "image/png",
    },
    Fixture {
        bytes: include_bytes!("../src/contexts/media/image_intake/fixtures/python-rgba16-2x3.png"),
        sha256: "1e2725064b73e8bdc0c7056ae8bf2d7f17dbcae5a7dac2d5f13fc398076c357d",
        mime: dto::ImageMime::Png,
        content_type: "image/png",
    },
    Fixture {
        bytes: include_bytes!("../src/contexts/media/image_intake/fixtures/pillow-rgb-2x3.jpg"),
        sha256: "a259cc25075434554da937e24a8056b5f9db0608a448c06effac4ba9c950eb46",
        mime: dto::ImageMime::Jpeg,
        content_type: "image/jpeg",
    },
];

struct PrivateRoots {
    parent: PathBuf,
    first: PathBuf,
    next: PathBuf,
    first_config: PathBuf,
    next_config: PathBuf,
}

impl PrivateRoots {
    fn new() -> Self {
        let parent = env::temp_dir().join(format!("cantos-artwork-test-{}", Uuid::new_v4()));
        fs::create_dir(&parent).unwrap();
        fs::set_permissions(&parent, fs::Permissions::from_mode(0o700)).unwrap();
        let roots = Self {
            first: parent.join("first"),
            next: parent.join("next"),
            first_config: parent.join("first.toml"),
            next_config: parent.join("next.toml"),
            parent,
        };
        for (config, active) in [
            (&roots.first_config, FIRST_TARGET),
            (&roots.next_config, NEXT_TARGET),
        ] {
            let topology = format!(
                "version=1\n[storage]\nactive_write_target='{active}'\n\
                 [storage.targets.{FIRST_TARGET}]\ndriver='filesystem'\nprivate_root={}\n\
                 [storage.targets.{NEXT_TARGET}]\ndriver='filesystem'\nprivate_root={}\n",
                serde_json::to_string(&roots.first).unwrap(),
                serde_json::to_string(&roots.next).unwrap(),
            );
            fs::write(config, topology).unwrap();
        }
        roots
    }

    fn object(&self, target: &str, sha256: &str) -> PathBuf {
        let root = match target {
            FIRST_TARGET => &self.first,
            NEXT_TARGET => &self.next,
            _ => panic!("unexpected recorded target"),
        };
        root.join("objects").join(sha256)
    }

    fn snapshot(&self) -> Vec<(String, Vec<u8>)> {
        let mut objects = Vec::new();
        for (target, root) in [(FIRST_TARGET, &self.first), (NEXT_TARGET, &self.next)] {
            for entry in fs::read_dir(root.join("objects")).unwrap() {
                let path = entry.unwrap().path();
                assert!(path.is_file());
                let name = path.file_name().unwrap().to_str().unwrap();
                assert!(!name.starts_with(".pending-"));
                objects.push((format!("{target}/{name}"), fs::read(path).unwrap()));
            }
        }
        objects.sort_by(|one, two| one.0.cmp(&two.0));
        objects
    }
}

impl Drop for PrivateRoots {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.parent);
    }
}

struct Harness {
    admin: NativeSql,
    store: Store,
    host: HttpHost,
    roots: PrivateRoots,
    work: String,
}

impl Harness {
    async fn new() -> Self {
        let admin = NativeSql::new().await;
        admin.0.seed_actors(&["alice", "bob", "carol"]).await;
        admin.execute("INSERT INTO script_evidence VALUES('alice','rights','synthetic-rights','Synthetic declaration; no clearance')",&[]).await.unwrap();
        for (actor, token, fence, seconds) in [
            ("alice", ALICE, ALICE_FENCE, 3600i64),
            ("bob", BOB, BOB_FENCE, 3600),
            ("carol", CAROL, CAROL_FENCE, 3600),
            ("alice", EXPIRED, EXPIRED_FENCE, -1),
        ] {
            let expires = admin.0.clock_reading().unwrap() + seconds * 1_000_000;
            admin.execute("INSERT INTO development_sessions(token_hash,actor_id,session_id,expires_at) VALUES(?1,?2,?3,?4)", &[&token_hash(token),&actor,&fence,&expires]).await.unwrap();
        }
        let roots = PrivateRoots::new();
        let store = configured_store(&admin, &roots.first_config);
        let host = start_host(store.clone()).await;
        let create = work_request();
        let work = create["id"].as_str().unwrap().to_owned();
        let created = host
            .json(
                "POST",
                "/api/v1/studio/works",
                Some(ALICE),
                Some(ALICE_FENCE),
                Some(&create),
            )
            .await;
        let result: Value = created.success(StatusCode::CREATED);
        assert_eq!(result["work"]["id"], work);
        assert_eq!(result["work"]["revision"], 1);
        Self {
            admin,
            store,
            host,
            roots,
            work,
        }
    }

    fn reopen(&self, config_path: &Path, _suffix: &str) -> Store {
        configured_store(&self.admin, config_path)
    }
    async fn restart(&mut self) {
        let replacement = NativeSql::new().await;
        let old = std::mem::replace(&mut self.store, replacement.store());
        let host = std::mem::replace(&mut self.host, start_host(replacement.store()).await);
        host.stop().await;
        drop(old);
        self.admin.0.restart().await;
        self.store = configured_store(&self.admin, &self.roots.first_config);
        self.host = start_host(self.store.clone()).await;
    }
    async fn snapshot(&self) -> (String, Vec<(String, Vec<u8>)>) {
        let rows:String=self.admin.query_one("SELECT json_array((SELECT revision FROM catalog_clock),(SELECT json_group_array(json_array(id,owner_id,kind,lifecycle,head_revision)) FROM (SELECT * FROM catalog_works ORDER BY id)),(SELECT json_group_array(json_array(work_id,revision,hex(snapshot),snapshot_digest,recorded_by)) FROM (SELECT * FROM catalog_work_revisions ORDER BY work_id,revision)),(SELECT json_group_array(json_array(actor_id,operation_id,fingerprint,hex(result_bytes),result_digest)) FROM (SELECT * FROM catalog_operations ORDER BY actor_id,operation_id)),(SELECT json_group_array(json_array(id,work_id,owner_id,operation_id,expected_sha256,expected_byte_length,content_type,profile_id,created_at_ms,expires_at_ms,hex(document_bytes),digest)) FROM (SELECT * FROM media_image_intents ORDER BY id)),(SELECT json_group_array(json_array(id,intent_id,work_id,owner_id,storage_target,object_key,sha256,byte_length,content_type,width_px,height_px,profile_id,admitted_at_ms,hex(document_bytes),digest)) FROM (SELECT * FROM media_image_assets ORDER BY id)),(SELECT json_group_array(json_array(work_id,work_metadata_revision,owner_id,image_asset_id,image_asset_digest,alt_text,recorded_by,recorded_at_ms,hex(document_bytes),digest)) FROM (SELECT * FROM media_work_artwork_changes ORDER BY work_id,work_metadata_revision)),(SELECT json_group_array(json_array(owner_id,operation_id,work_id,kind,fingerprint,hex(response_bytes),response_checksum)) FROM (SELECT * FROM media_artwork_operations ORDER BY owner_id,operation_id)))", &[]).await.unwrap().get(0);
        (rows, self.roots.snapshot())
    }

    async fn recorded_image(
        &self,
        asset: &dto::ImageAssetDocument,
        target: &str,
        fixture: Fixture,
    ) {
        let row = self.admin.query_one(
            "SELECT storage_target,object_key,sha256,byte_length,content_type,width_px,height_px,
                    document_bytes FROM media_image_assets WHERE id=?1 AND work_id=?2",
            &[&asset.id, &self.work],
        ).await.unwrap();
        assert_eq!(row.get::<_, String>(0), target);
        assert_eq!(row.get::<_, String>(1), fixture.sha256);
        assert_eq!(row.get::<_, String>(2), fixture.sha256);
        assert_eq!(row.get::<_, i64>(3), fixture.bytes.len() as i64);
        assert_eq!(row.get::<_, String>(4), fixture.content_type);
        assert_eq!((row.get::<_, i32>(5), row.get::<_, i32>(6)), (2, 3));
        assert_eq!(
            serde_json::from_slice::<dto::ImageAssetDocument>(&row.get::<_, Vec<u8>>(7)).unwrap(),
            *asset
        );
        let object = self.roots.object(target, fixture.sha256);
        assert_eq!(fs::read(&object).unwrap(), fixture.bytes);
        assert_eq!(
            fs::metadata(object).unwrap().permissions().mode() & 0o777,
            0o400
        );
    }
}

fn configured_store(db: &NativeSql, topology: &Path) -> Store {
    db.store().with_storage_config(topology).unwrap()
}

async fn start_host(store: Store) -> HttpHost {
    HttpHost::start(router(
        AppState {
            store,
            web_origin: ORIGIN.into(),
        },
        "/nonexistent-cantos-artwork-dist",
    ))
    .await
}

fn work_request() -> Value {
    json!({
        "operation_id": Uuid::new_v4().to_string(), "id": Uuid::new_v4().to_string(),
        "kind": "story", "details": {
            "title": "Ánh đèn cuối sân khấu", "synopsis": "Synthetic private image acceptance",
            "language": "vi-VN", "visibility": "private", "rights_refs": []
        }
    })
}

fn prepare_request(fixture: Fixture) -> dto::CreateImageIntentRequest {
    assert_eq!(
        format!("{:x}", Sha256::digest(fixture.bytes)),
        fixture.sha256
    );
    dto::CreateImageIntentRequest {
        operation_id: Uuid::new_v4().to_string(),
        id: Uuid::new_v4().to_string(),
        expected_sha256: fixture.sha256.into(),
        expected_byte_length: fixture.bytes.len() as u64,
        content_type: fixture.mime,
        declarations: dto::ImageDeclarations {
            provenance: "Independent Python/Pillow 2x3 synthetic pixels".into(),
            rights_refs: vec!["synthetic-rights".into()],
        },
    }
}

fn path(work: &str, suffix: &str) -> String {
    format!("/api/v1/studio/works/{work}/{suffix}")
}

async fn prepare(host: &HttpHost, work: &str, request: &dto::CreateImageIntentRequest) -> Reply {
    host.json(
        "POST",
        &path(work, "image-intents"),
        Some(ALICE),
        Some(ALICE_FENCE),
        Some(request),
    )
    .await
}

async fn upload(host: &HttpHost, work: &str, intent: &str, fixture: Fixture) -> Reply {
    request(
        &host.client,
        &host.base,
        "PUT",
        &path(work, &format!("image-intents/{intent}/bytes")),
        Some(ALICE),
        Some(ALICE_FENCE),
        Some(fixture.content_type),
        fixture.bytes.to_vec(),
        &[],
    )
    .await
}

async fn admit(host: &HttpHost, work: &str, fixture: Fixture) -> dto::ImageAssetDocument {
    let raw = prepare_request(fixture);
    let receipt = prepare(host, work, &raw).await;
    let intent: dto::ImageIntentDocument = receipt.success(StatusCode::CREATED);
    assert_eq!(intent.request, raw);
    assert_eq!(receipt.headers["x-cantos-studio-session"], ALICE_FENCE);
    let replay = prepare(host, work, &raw).await;
    replay.success::<dto::ImageIntentDocument>(StatusCode::CREATED);
    assert_eq!(replay.bytes, receipt.bytes);
    let proto = request(
        &host.client,
        &host.base,
        "POST",
        &path(work, "image-intents"),
        Some(ALICE),
        Some(ALICE_FENCE),
        Some("application/protobuf"),
        dto::wire::encode_artwork(dto::wire::WireFormat::Protobuf, &raw).unwrap(),
        &[("accept", "application/protobuf")],
    )
    .await;
    assert_eq!(proto.status, StatusCode::CREATED);
    assert_eq!(proto.headers["content-type"], "application/protobuf");
    assert_eq!(proto.headers["vary"], "accept");
    assert_eq!(
        dto::wire::decode_artwork::<dto::ImageIntentDocument>(
            dto::wire::WireFormat::Protobuf,
            &proto.bytes
        )
        .unwrap(),
        intent
    );
    let pending: dto::ImageIntentResponse = host
        .json::<Value>(
            "GET",
            &path(work, &format!("image-intents/{}", raw.id)),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(pending.intent, intent);
    assert_eq!(pending.state, dto::ImageIntentState::Pending);
    assert_eq!(pending.asset, None);
    let received = upload(host, work, &raw.id, fixture).await;
    let asset: dto::ImageAssetDocument = received.success(StatusCode::OK);
    assert_eq!(asset.id, raw.id);
    assert_eq!(asset.intent_id, raw.id);
    assert_eq!(asset.work_id, work);
    assert_eq!(asset.owner_id, "alice");
    assert_eq!(asset.verified_media.sha256, fixture.sha256);
    assert_eq!(asset.verified_media.byte_length, fixture.bytes.len() as u64);
    assert_eq!(asset.verified_media.content_type, fixture.mime);
    assert_eq!(
        (
            asset.verified_media.width_px,
            asset.verified_media.height_px
        ),
        (2, 3)
    );
    let duplicate = upload(host, work, &raw.id, fixture).await;
    duplicate.success::<dto::ImageAssetDocument>(StatusCode::OK);
    assert_eq!(duplicate.bytes, received.bytes);
    let metadata = host
        .json::<Value>(
            "GET",
            &path(work, &format!("image-assets/{}", asset.id)),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await;
    metadata.success::<dto::ImageAssetDocument>(StatusCode::OK);
    assert_eq!(metadata.bytes, received.bytes);
    let proto = request(
        &host.client,
        &host.base,
        "GET",
        &path(work, &format!("image-assets/{}", asset.id)),
        Some(ALICE),
        Some(ALICE_FENCE),
        None,
        vec![],
        &[("accept", "application/protobuf")],
    )
    .await;
    assert_eq!(proto.status, StatusCode::OK);
    assert_eq!(proto.headers["content-type"], "application/protobuf");
    assert_eq!(proto.headers["vary"], "accept");
    assert_eq!(
        dto::wire::decode_artwork::<dto::ImageAssetDocument>(
            dto::wire::WireFormat::Protobuf,
            &proto.bytes
        )
        .unwrap(),
        asset
    );
    let serialized = String::from_utf8(metadata.bytes).unwrap();
    for private in [
        "storage_target",
        "object_key",
        "private_root",
        "http://",
        "https://",
    ] {
        assert!(
            !serialized.contains(private),
            "private location leaked: {private}"
        );
    }
    let admitted: dto::ImageIntentResponse = host
        .json::<Value>(
            "GET",
            &path(work, &format!("image-intents/{}", raw.id)),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(admitted.state, dto::ImageIntentState::Admitted);
    assert_eq!(admitted.asset, Some(asset.clone()));
    asset
}

async fn preview(host: &HttpHost, work: &str, asset: &str, fixture: Fixture) {
    for method in ["GET", "HEAD"] {
        let reply = request(
            &host.client,
            &host.base,
            method,
            &path(work, &format!("image-assets/{asset}/bytes")),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
            vec![],
            &[
                ("accept", "application/protobuf"),
                ("range", "bytes=0-1"),
                ("if-none-match", &format!("\"{}\"", fixture.sha256)),
            ],
        )
        .await;
        assert_eq!(reply.status, StatusCode::OK);
        assert_eq!(reply.headers["content-type"], fixture.content_type);
        assert_eq!(
            reply.headers["content-length"],
            fixture.bytes.len().to_string()
        );
        assert_eq!(reply.headers["etag"], format!("\"{}\"", fixture.sha256));
        assert!(!reply.headers.contains_key("content-range"));
        assert!(!reply.headers.contains_key("location"));
        assert!(!reply.headers.contains_key("set-cookie"));
        assert!(!reply.headers.contains_key("access-control-allow-origin"));
        if method == "HEAD" {
            assert!(reply.bytes.is_empty());
        } else {
            assert_eq!(reply.bytes, fixture.bytes);
        }
    }
}

fn selection(asset: &dto::ImageAssetDocument, revision: u64) -> dto::SetWorkArtworkRequest {
    dto::SetWorkArtworkRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_work_revision: revision,
        artwork: Some(dto::WorkArtworkSelection {
            image_asset_id: asset.id.clone(),
            image_asset_digest: asset.digest.clone(),
            alt_text: "Bìa thử nghiệm dưới ánh đèn".into(),
        }),
    }
}

async fn set(host: &HttpHost, work: &str, body: &dto::SetWorkArtworkRequest) -> Reply {
    host.json(
        "PATCH",
        &path(work, "artwork"),
        Some(ALICE),
        Some(ALICE_FENCE),
        Some(body),
    )
    .await
}

#[tokio::test]
async fn production_http_shared_storage_preserves_three_formats_receipts_targets_and_history_after_reopen(
) {
    let _media_lane = native_sql::media_lane().await;
    let mut h = Harness::new().await;
    assert!(h.store.audio_storage_is_enabled());
    let initial: dto::WorkArtworkResponse = h
        .host
        .json::<Value>(
            "GET",
            &path(&h.work, "artwork"),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(initial.work_revision, 1);
    assert_eq!(initial.artwork, None);
    assert_eq!(initial.changed_at_ms, None);
    let first = admit(&h.host, &h.work, FIXTURES[0]).await;
    h.recorded_image(&first, FIRST_TARGET, FIXTURES[0]).await;
    preview(&h.host, &h.work, &first.id, FIXTURES[0]).await;
    let first_object = h.roots.object(FIRST_TARGET, FIXTURES[0].sha256);
    let first_inode = fs::metadata(&first_object).unwrap().ino();
    let select = selection(&first, 1);
    let selected = set(&h.host, &h.work, &select).await;
    let selected_value: dto::WorkArtworkResponse = selected.success(StatusCode::OK);
    assert_eq!(selected_value.work_revision, 2);
    assert_eq!(selected_value.artwork, select.artwork);

    // Reopen the real file and the object registry; recorded original target remains authoritative.
    h.restart().await;
    let reopened = h.reopen(&h.roots.next_config, "reopened");
    let host = start_host(reopened).await;
    preview(&host, &h.work, &first.id, FIXTURES[0]).await;
    let replay = upload(&host, &h.work, &first.intent_id, FIXTURES[0]).await;
    assert_eq!(
        replay.success::<dto::ImageAssetDocument>(StatusCode::OK),
        first
    );
    let original_receipt: Vec<u8> = h
        .admin
        .query_one(
            "SELECT document_bytes FROM media_image_assets WHERE id=?1",
            &[&first.id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(replay.bytes, original_receipt);
    assert_eq!(fs::metadata(&first_object).unwrap().ino(), first_inode);
    assert!(!h.roots.object(NEXT_TARGET, FIXTURES[0].sha256).exists());
    let second = admit(&host, &h.work, FIXTURES[1]).await;
    let third = admit(&host, &h.work, FIXTURES[2]).await;
    h.recorded_image(&second, NEXT_TARGET, FIXTURES[1]).await;
    h.recorded_image(&third, NEXT_TARGET, FIXTURES[2]).await;
    preview(&host, &h.work, &second.id, FIXTURES[1]).await;
    preview(&host, &h.work, &third.id, FIXTURES[2]).await;
    let replace = selection(&third, 2);
    let replaced: dto::WorkArtworkResponse =
        set(&host, &h.work, &replace).await.success(StatusCode::OK);
    assert_eq!(replaced.work_revision, 3);
    let edit = json!({"operation_id":Uuid::new_v4().to_string(),"expected_revision":3,"details":{"title":"Một tiêu đề mới"}});
    let edited: Value = host
        .json(
            "PATCH",
            &format!("/api/v1/studio/works/{}", h.work),
            Some(ALICE),
            Some(ALICE_FENCE),
            Some(&edit),
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(edited["work"]["revision"], 4);
    let clear = dto::SetWorkArtworkRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_work_revision: 4,
        artwork: None,
    };
    let cleared: dto::WorkArtworkResponse =
        set(&host, &h.work, &clear).await.success(StatusCode::OK);
    assert_eq!(cleared.work_revision, 5);
    assert_eq!(cleared.artwork, None);
    assert!(cleared.changed_at_ms.is_some());
    let inherited = dto::WorkArtworkResponse {
        work_revision: 4,
        ..replaced.clone()
    };
    for expected in [initial, selected_value, replaced, inherited, cleared] {
        let revision = expected.work_revision;
        let historical: dto::WorkArtworkResponse = host
            .json::<Value>(
                "GET",
                &format!(
                    "{}?work_revision={revision}&studio_session_id={ALICE_FENCE}",
                    path(&h.work, "artwork")
                ),
                Some(ALICE),
                Some(ALICE_FENCE),
                None,
            )
            .await
            .success(StatusCode::OK);
        assert_eq!(historical, expected);
    }
    let before = h.snapshot().await;
    let old = set(&host, &h.work, &select).await;
    old.success::<dto::WorkArtworkResponse>(StatusCode::OK);
    assert_eq!(old.bytes, selected.bytes);
    let old_prepare = first.intent_id.clone();
    let intent: dto::ImageIntentResponse = host
        .json::<Value>(
            "GET",
            &path(&h.work, &format!("image-intents/{old_prepare}")),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(
        prepare(&host, &h.work, &intent.intent.request).await.bytes,
        serde_json::to_vec(&intent.intent).unwrap()
    );
    assert_eq!(h.snapshot().await, before);
    let counts = h.admin.query_one("SELECT (SELECT head_revision FROM catalog_works WHERE id=?1),(SELECT count(*) FROM catalog_work_revisions WHERE work_id=?1),(SELECT count(*) FROM media_work_artwork_changes WHERE work_id=?1),(SELECT count(*) FROM media_artwork_operations WHERE work_id=?1),(SELECT count(*) FROM media_image_intents WHERE work_id=?1),(SELECT count(*) FROM media_image_assets WHERE work_id=?1),(SELECT revision FROM catalog_clock)",&[&h.work]).await.unwrap();
    assert_eq!(
        (
            counts.get::<_, i64>(0),
            counts.get::<_, i64>(1),
            counts.get::<_, i64>(2),
            counts.get::<_, i64>(3),
            counts.get::<_, i64>(4),
            counts.get::<_, i64>(5),
            counts.get::<_, i64>(6)
        ),
        (5, 5, 3, 6, 3, 3, 5)
    );
    assert_eq!(h.roots.snapshot().len(), 3);
    for suffix in [
        "artwork?work_revision=6",
        "image-assets/00000000-0000-4000-8000-000000000099",
    ] {
        host.json::<Value>(
            "GET",
            &path(&h.work, suffix),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    }
    for alias in [
        format!("/api/v1/works/{}/artwork", h.work),
        format!(
            "/api/v1/theatre/works/{}/image-assets/{}/bytes",
            h.work, first.id
        ),
    ] {
        host.json::<Value>("GET", &alias, Some(ALICE), None, None)
            .await
            .error(StatusCode::NOT_FOUND, "not_found");
    }
}

#[tokio::test]
async fn production_owner_fence_denies_member_foreign_expired_revoked_and_invalid_commands_without_writes(
) {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let id = WorkId::parse(&h.work).unwrap();
    h.store
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::SetMember {
                work_id: id.clone(),
                expected_revision: 1,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap();
    // Membership really authorizes the ordinary Catalog read, but never the image capability.
    assert_eq!(h.store.catalog_work(BOB, &id).await.unwrap().revision(), 2);
    let asset = admit(&h.host, &h.work, FIXTURES[0]).await;
    let select = selection(&asset, 2);
    let selected = set(&h.host, &h.work, &select).await;
    selected.success::<dto::WorkArtworkResponse>(StatusCode::OK);
    let pending = prepare_request(FIXTURES[1]);
    prepare(&h.host, &h.work, &pending)
        .await
        .success::<dto::ImageIntentDocument>(StatusCode::CREATED);
    let other = work_request();
    h.host
        .json(
            "POST",
            "/api/v1/studio/works",
            Some(ALICE),
            Some(ALICE_FENCE),
            Some(&other),
        )
        .await
        .success::<Value>(StatusCode::CREATED);
    let other_work = other["id"].as_str().unwrap();
    let before = h.snapshot().await;

    for (token, fence, status, code) in [
        (None, None, StatusCode::UNAUTHORIZED, "unauthenticated"),
        (
            Some(BOB),
            Some(BOB_FENCE),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            Some(CAROL),
            Some(CAROL_FENCE),
            StatusCode::NOT_FOUND,
            "not_found",
        ),
        (
            Some(EXPIRED),
            Some(EXPIRED_FENCE),
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
        ),
    ] {
        for suffix in [
            "artwork".to_owned(),
            format!("image-intents/{}", pending.id),
            format!("image-assets/{}", asset.id),
            format!("image-assets/{}/bytes", asset.id),
        ] {
            request(
                &h.host.client,
                &h.host.base,
                "GET",
                &path(&h.work, &suffix),
                token,
                fence,
                None,
                vec![],
                &[],
            )
            .await
            .error(status, code);
        }
        let head = request(
            &h.host.client,
            &h.host.base,
            "HEAD",
            &path(&h.work, &format!("image-assets/{}/bytes", asset.id)),
            token,
            fence,
            None,
            vec![],
            &[],
        )
        .await;
        assert_eq!(head.status, status);
        assert!(head.bytes.is_empty());
        let unauthorized_prepare = prepare_request(FIXTURES[2]);
        h.host
            .json(
                "POST",
                &path(&h.work, "image-intents"),
                token,
                fence,
                Some(&unauthorized_prepare),
            )
            .await
            .error(status, code);
        h.host
            .json(
                "PATCH",
                &path(&h.work, "artwork"),
                token,
                fence,
                Some(&select),
            )
            .await
            .error(status, code);
        request(
            &h.host.client,
            &h.host.base,
            "PUT",
            &path(&h.work, &format!("image-intents/{}/bytes", pending.id)),
            token,
            fence,
            Some(FIXTURES[1].content_type),
            FIXTURES[1].bytes.to_vec(),
            &[],
        )
        .await
        .error(status, code);
    }
    assert_eq!(h.snapshot().await, before);

    // The final Studio session UUID is a transport fence, never a second bearer credential.
    let wrong = h
        .host
        .json::<Value>(
            "GET",
            &path(&h.work, "artwork"),
            Some(ALICE),
            Some(BOB_FENCE),
            None,
        )
        .await;
    wrong.error(StatusCode::CONFLICT, "unauthenticated");
    assert_eq!(wrong.headers["x-cantos-studio-session"], ALICE_FENCE);
    h.host
        .json::<Value>(
            "GET",
            &format!("{}?studio_session_id={BOB_FENCE}", path(&h.work, "artwork")),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .error(StatusCode::CONFLICT, "unauthenticated");
    h.host
        .json::<Value>(
            "GET",
            &format!(
                "{}?studio_session_id={ALICE_FENCE}&studio_session_id={ALICE_FENCE}",
                path(&h.work, "artwork")
            ),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .error(StatusCode::CONFLICT, "unauthenticated");
    request(
        &h.host.client,
        &h.host.base,
        "GET",
        &path(&h.work, "artwork"),
        Some(ALICE),
        Some(ALICE_FENCE),
        None,
        vec![],
        &[("cookie", &format!("cantos_session={BOB}"))],
    )
    .await
    .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    for suffix in [
        format!("image-assets/{}", asset.id),
        format!("image-assets/{}/bytes", asset.id),
        format!("image-intents/{}", pending.id),
    ] {
        h.host
            .json::<Value>(
                "GET",
                &path(other_work, &suffix),
                Some(ALICE),
                Some(ALICE_FENCE),
                None,
            )
            .await
            .error(StatusCode::NOT_FOUND, "not_found");
    }
    let foreign_selection = selection(&asset, 1);
    set(&h.host, other_work, &foreign_selection)
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    let stale = selection(&asset, 2);
    assert_eq!(
        set(&h.host, &h.work, &stale)
            .await
            .error(StatusCode::CONFLICT, "stale_revision")["current_revision"],
        3
    );
    let mut wrong_digest = selection(&asset, 3);
    wrong_digest.artwork.as_mut().unwrap().image_asset_digest =
        format!("ia-r1:sha256:{}", "1".repeat(64));
    assert_eq!(
        set(&h.host, &h.work, &wrong_digest)
            .await
            .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request")["issues"][0]["rule"],
        "asset_binding_mismatch"
    );
    let reused = dto::SetWorkArtworkRequest {
        artwork: None,
        ..select.clone()
    };
    set(&h.host, &h.work, &reused)
        .await
        .error(StatusCode::CONFLICT, "operation_reused");
    let omitted = json!({"operation_id":Uuid::new_v4().to_string(),"expected_work_revision":3});
    h.host
        .json(
            "PATCH",
            &path(&h.work, "artwork"),
            Some(ALICE),
            Some(ALICE_FENCE),
            Some(&omitted),
        )
        .await
        .error(StatusCode::BAD_REQUEST, "invalid_request");
    let upload_path = path(&h.work, &format!("image-intents/{}/bytes", pending.id));
    for (content_type, extra, status, rule) in [
        (
            "application/octet-stream",
            vec![],
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        ),
        (
            "image/png",
            vec![("content-encoding", "gzip")],
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "unsupported_media_type",
        ),
        (
            "image/jpeg",
            vec![],
            StatusCode::UNPROCESSABLE_ENTITY,
            "image_binding_mismatch",
        ),
    ] {
        let error = request(
            &h.host.client,
            &h.host.base,
            "PUT",
            &upload_path,
            Some(ALICE),
            Some(ALICE_FENCE),
            Some(content_type),
            FIXTURES[1].bytes.to_vec(),
            &extra,
        )
        .await
        .error(status, "invalid_request");
        assert_eq!(error["issues"][0]["rule"], rule);
    }
    let mismatch = request(
        &h.host.client,
        &h.host.base,
        "PUT",
        &upload_path,
        Some(ALICE),
        Some(ALICE_FENCE),
        Some("image/png"),
        FIXTURES[0].bytes.to_vec(),
        &[],
    )
    .await
    .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    assert_eq!(mismatch["issues"][0]["rule"], "expected_hash_mismatch");
    request(
        &h.host.client,
        &h.host.base,
        "PATCH",
        &path(&h.work, "artwork"),
        Some(ALICE),
        Some(ALICE_FENCE),
        Some("application/json"),
        serde_json::to_vec(&selection(&asset, 3)).unwrap(),
        &[("origin", "https://foreign.invalid")],
    )
    .await
    .error(StatusCode::FORBIDDEN, "forbidden");
    assert_eq!(h.snapshot().await, before);

    h.admin
        .execute(
            "UPDATE development_sessions SET revoked=true WHERE token_hash=?1",
            &[&token_hash(ALICE)],
        )
        .await
        .unwrap();
    for (method, suffix, body, mime) in [
        ("GET", "artwork".to_owned(), vec![], None),
        ("GET", format!("image-assets/{}", asset.id), vec![], None),
        (
            "GET",
            format!("image-assets/{}/bytes", asset.id),
            vec![],
            None,
        ),
        (
            "PUT",
            format!("image-intents/{}/bytes", asset.intent_id),
            FIXTURES[0].bytes.to_vec(),
            Some("image/png"),
        ),
        (
            "PATCH",
            "artwork".to_owned(),
            serde_json::to_vec(&select).unwrap(),
            Some("application/json"),
        ),
    ] {
        request(
            &h.host.client,
            &h.host.base,
            method,
            &path(&h.work, &suffix),
            Some(ALICE),
            Some(ALICE_FENCE),
            mime,
            body,
            &[],
        )
        .await
        .error(StatusCode::UNAUTHORIZED, "unauthenticated");
    }
    assert_eq!(h.snapshot().await, before);
}

#[tokio::test]
async fn malformed_actual_images_and_corrupt_recorded_objects_fail_closed_without_repair_or_public_fallback(
) {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let unconfigured = start_host(h.admin.store()).await;
    unconfigured
        .json::<Value>(
            "GET",
            &path(&h.work, "artwork"),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    let asset = admit(&h.host, &h.work, FIXTURES[0]).await;

    // Correct declarations for an actual truncated PNG cannot substitute for a full decode.
    let malformed = FIXTURES[1].bytes[..FIXTURES[1].bytes.len() - 1].to_vec();
    let mut raw = prepare_request(FIXTURES[1]);
    raw.expected_sha256 = format!("{:x}", Sha256::digest(&malformed));
    raw.expected_byte_length = malformed.len() as u64;
    prepare(&h.host, &h.work, &raw)
        .await
        .success::<dto::ImageIntentDocument>(StatusCode::CREATED);
    let before = h.snapshot().await;
    let error = request(
        &h.host.client,
        &h.host.base,
        "PUT",
        &path(&h.work, &format!("image-intents/{}/bytes", raw.id)),
        Some(ALICE),
        Some(ALICE_FENCE),
        Some("image/png"),
        malformed,
        &[],
    )
    .await
    .error(StatusCode::UNPROCESSABLE_ENTITY, "invalid_request");
    assert_eq!(error["issues"][0]["rule"], "malformed_image");
    assert_eq!(h.snapshot().await, before);

    // Damage only a test-owned private inode, retaining its mode and length. Even a correct
    // same-key object on the new active target must not replace the original recorded target.
    let original = h.roots.object(FIRST_TARGET, FIXTURES[0].sha256);
    fs::set_permissions(&original, fs::Permissions::from_mode(0o600)).unwrap();
    let mut corrupted = FIXTURES[0].bytes.to_vec();
    *corrupted.last_mut().unwrap() ^= 1;
    fs::write(&original, &corrupted).unwrap();
    fs::set_permissions(&original, fs::Permissions::from_mode(0o400)).unwrap();
    let tempting_fallback = h.roots.object(NEXT_TARGET, FIXTURES[0].sha256);
    fs::write(&tempting_fallback, FIXTURES[0].bytes).unwrap();
    fs::set_permissions(&tempting_fallback, fs::Permissions::from_mode(0o400)).unwrap();
    let switched = start_host(h.reopen(&h.roots.next_config, "corruption")).await;
    let before = h.snapshot().await;
    for method in ["GET", "HEAD"] {
        let reply = request(
            &switched.client,
            &switched.base,
            method,
            &path(&h.work, &format!("image-assets/{}/bytes", asset.id)),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
            vec![],
            &[],
        )
        .await;
        assert_eq!(reply.status, StatusCode::INTERNAL_SERVER_ERROR);
        assert!(!reply.headers.contains_key("etag"));
        assert!(!reply.headers.contains_key("location"));
        if method == "HEAD" {
            assert!(reply.bytes.is_empty());
        } else {
            reply.error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
        }
    }
    set(&switched, &h.work, &selection(&asset, 1))
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
    upload(&switched, &h.work, &asset.intent_id, FIXTURES[0])
        .await
        .error(StatusCode::INTERNAL_SERVER_ERROR, "corrupt_revision");
    assert_eq!(h.snapshot().await, before);
    assert_eq!(fs::read(&original).unwrap(), corrupted);
    let metadata: dto::ImageAssetDocument = switched
        .json::<Value>(
            "GET",
            &path(&h.work, &format!("image-assets/{}", asset.id)),
            Some(ALICE),
            Some(ALICE_FENCE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(metadata, asset);
    let archived = json!({"operation_id":Uuid::new_v4().to_string(),"expected_revision":1});
    switched
        .json(
            "DELETE",
            &format!("/api/v1/studio/works/{}", h.work),
            Some(ALICE),
            Some(ALICE_FENCE),
            Some(&archived),
        )
        .await
        .success::<Value>(StatusCode::OK);
    let before = h.snapshot().await;
    set(
        &switched,
        &h.work,
        &dto::SetWorkArtworkRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_work_revision: 2,
            artwork: None,
        },
    )
    .await
    .error(StatusCode::CONFLICT, "catalog_conflict");
    prepare(&switched, &h.work, &prepare_request(FIXTURES[2]))
        .await
        .error(StatusCode::CONFLICT, "catalog_conflict");
    assert_eq!(h.snapshot().await, before);
}

#[tokio::test]
async fn two_independent_production_http_stores_commit_one_catalog_artwork_cas_winner() {
    let _media_lane = native_sql::media_lane().await;
    let h = Harness::new().await;
    let asset = admit(&h.host, &h.work, FIXTURES[0]).await;
    let files = h.roots.snapshot();
    let second = start_host(configured_store(&h.admin, &h.roots.first_config)).await;
    let selected = selection(&asset, 1);
    let edit = json!({"operation_id":Uuid::new_v4().to_string(),"expected_revision":1,"details":{"title":"Hai HTTP facade chung một owner"}});
    let uri = format!("/api/v1/studio/works/{}", h.work);
    let blocker = h.admin.0.writer().await;
    let mut a = Box::pin(set(&h.host, &h.work, &selected));
    let mut b = Box::pin(second.json("PATCH", &uri, Some(ALICE), Some(ALICE_FENCE), Some(&edit)));
    assert!(futures_util::poll!(&mut a).is_pending());
    assert!(futures_util::poll!(&mut b).is_pending());
    blocker.commit().await.unwrap();
    let (a, b) = tokio::join!(a, b);
    assert_eq!(
        [a.status, b.status]
            .iter()
            .filter(|s| **s == StatusCode::OK)
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
    let row=h.admin.query_one("SELECT head_revision,(SELECT count(*) FROM catalog_work_revisions),(SELECT count(*) FROM media_work_artwork_changes) FROM catalog_works WHERE id=?1",&[&h.work]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 2);
    assert_eq!(row.get::<_, i64>(1), 2);
    assert_eq!(row.get::<_, i64>(2), i64::from(a.status == StatusCode::OK));
    assert_eq!(h.roots.snapshot(), files);
    let mut accepted = selected;
    if a.status != StatusCode::OK {
        accepted.expected_work_revision = 2;
    }
    let first = set(&second, &h.work, &accepted).await;
    first.success::<dto::WorkArtworkResponse>(StatusCode::OK);
    let before = h.snapshot().await;
    assert_eq!(set(&h.host, &h.work, &accepted).await.bytes, first.bytes);
    assert_eq!(h.snapshot().await, before);
    assert_eq!(h.roots.snapshot(), files);
}
