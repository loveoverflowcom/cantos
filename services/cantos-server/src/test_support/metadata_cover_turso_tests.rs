//! Synthetic permission records, six original fictional metadata Works and an existing 2×3 PNG
//! fixture in a real disposable Turso/private object root. No generated cover or real consent is
//! substituted. The non-login principal is populated through checked domain/SQL fixture seams;
//! this does not establish an operator image-intake workflow or authenticate a human owner.
use super::{ORIGIN, PNG, PNG_SHA256, STUDIO};
use crate::{
    adapters::inbound::http::{metadata_cover_test_router, AppState},
    contexts::{
        catalog::{
            self,
            domain::{
                metadata_disclosure::FICTIONAL_SAMPLE_SOURCE_SHA256,
                sample_seed_request::{SampleSeedAction, SampleSeedRequest},
                Work,
            },
        },
        media::{
            domain::{
                artwork::{decide_artwork_change, ArtworkDecision, SetWorkArtwork},
                cover_disclosure::{
                    decide_cover_disclosure, CoverDisclosureDecision, DisclosureFacts,
                    RecordCoverDisclosure,
                },
                image::{ImageAsset, ImageIntent, ImageMime, PrepareImage},
            },
            image_application::{objects::SharedImageObjects, ImageObjects},
            image_intake::ImageAdmission,
            turso::{artwork, cover, image},
            ListenerError, ListenerService, PublicMediaOrigin,
        },
    },
    contract::{studio as dto, theatre::discovery},
    test_support::ServerFixture,
};
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use sha2::{Digest, Sha256};
use std::{future::IntoFuture, sync::Arc};
use tower::ServiceExt;

const WORK: &str = "dbe1f751-0e5f-47d2-8910-62e2eafb2dcf";

struct MetadataWorld {
    clock: Arc<cantos_store_turso::testing::FixedClock>,
    fixture: ServerFixture,
    store: crate::application::Store,
    work: Work,
    image: ImageAsset,
}

impl MetadataWorld {
    async fn new() -> Self {
        let clock = cantos_store_turso::testing::FixedClock::new(super::T0);
        let fixture = ServerFixture::with_clock(clock.clone()).await;
        let store = fixture
            .store()
            .clone()
            .with_audio_storage(&fixture.dir().join("private"))
            .unwrap();
        catalog::seed_sample(
            &store,
            &SampleSeedRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                action: SampleSeedAction::Publish,
                source_sha256: FICTIONAL_SAMPLE_SOURCE_SHA256.into(),
                operator_reference: "synthetic operator".into(),
                evidence_reference: "synthetic metadata evidence".into(),
                rights_statement: "isolated metadata fixture".into(),
                confirm_source: true,
                confirm_metadata_rights: true,
            },
            Some(include_bytes!(
                "../../tests/fixtures/fictional-sample-metadata.json"
            )),
        )
        .await
        .unwrap();
        let mut connection = store.connection().await.unwrap();
        let tx = connection.read().await.unwrap();
        let work = catalog::sample_disclosed_work(&tx, WORK)
            .await
            .unwrap()
            .unwrap()
            .work;
        tx.commit().await.unwrap();
        let now = fixture.clock_reading().unwrap() / 1000;
        let intent_id = uuid::Uuid::new_v4().to_string();
        let intent = ImageIntent::create(
            PrepareImage::admit(&dto::CreateImageIntentRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                id: intent_id,
                expected_sha256: PNG_SHA256.into(),
                expected_byte_length: PNG.len() as u64,
                content_type: dto::ImageMime::Png,
                declarations: dto::ImageDeclarations {
                    provenance: "synthetic-private-provenance".into(),
                    rights_refs: vec!["synthetic-image-rights".into()],
                },
            })
            .unwrap(),
            work.id(),
            work.owner_id(),
            now,
        )
        .unwrap();
        let verified = ImageAdmission::from_test_lanes(Arc::new(tokio::sync::Semaphore::new(1)))
            .unwrap()
            .inspect(PNG.to_vec(), ImageMime::Png)
            .await
            .unwrap();
        let asset = ImageAsset::create(&intent, verified.metadata().clone(), now).unwrap();
        let objects = SharedImageObjects::new(store.media_storage.clone());
        let location = objects.persist(verified).await.unwrap();
        let request = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            expected_work_revision: work.revision(),
            artwork: Some(dto::WorkArtworkSelection {
                image_asset_id: asset.id().as_str().into(),
                image_asset_digest: asset.digest().into(),
                alt_text: "Synthetic fixture cover".into(),
            }),
        })
        .unwrap();
        let ArtworkDecision::Apply(mutation) =
            decide_artwork_change(&work, &request, Some(&asset), None, now).unwrap()
        else {
            panic!("new fixture selection");
        };
        let work = mutation.work().clone();
        let bytes = serde_json::to_vec(&work).unwrap();
        let digest = format!("{:x}", Sha256::digest(&bytes));
        let tx = connection.transaction().await.unwrap();
        image::insert_intent(&tx, &intent).await.unwrap();
        image::insert_asset(&tx, &asset, &location.storage_target, &location.object_key)
            .await
            .unwrap();
        tx.execute("INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,?2,?3,?4,?5)",
            &[&WORK, &(work.revision() as i64), &bytes, &digest, &work.owner_id().as_str()]).await.unwrap();
        tx.execute(
            "UPDATE catalog_works SET head_revision=?2 WHERE id=?1",
            &[&WORK, &(work.revision() as i64)],
        )
        .await
        .unwrap();
        artwork::insert_change(&tx, mutation.change())
            .await
            .unwrap();
        tx.execute("INSERT INTO catalog_fictional_metadata_disclosures SELECT work_id,subject_kind,subject_id,sequence+1,owner_id,decision,?2,?3,unit_revision,unit_digest,source_id,source_sha256,rights_kind,rights_id,recorded_by,recorded_at_ms,?4,fingerprint FROM catalog_fictional_metadata_disclosures WHERE subject_kind='work' AND subject_id=?1 ORDER BY sequence DESC LIMIT 1",
            &[&WORK, &(work.revision() as i64), &digest, &uuid::Uuid::new_v4().to_string()]).await.unwrap();
        tx.commit().await.unwrap();
        Self {
            clock,
            fixture,
            store,
            work,
            image: asset,
        }
    }

    fn listener(&self, policy: bool) -> ListenerService {
        let service = ListenerService::new(self.store.clone())
            .with_cover_delivery(PublicMediaOrigin::parse(ORIGIN).unwrap(), &self.store);
        if policy {
            service.with_metadata_cover_policy()
        } else {
            service
        }
    }

    async fn consent(&self, revision: Option<u64>, clear: bool) {
        let mut connection = self.store.connection().await.unwrap();
        let tx = connection.transaction().await.unwrap();
        let now = self.fixture.clock_reading().unwrap() / 1000;
        let request = RecordCoverDisclosure::admit(&dto::RecordCoverDisclosureRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            image_asset_digest: self.image.digest().into(),
            decision: if clear {
                dto::CoverDisclosureKind::Cleared
            } else {
                dto::CoverDisclosureKind::Revoked
            },
            valid_until_ms: clear.then_some((now + 60_000) as u64),
        })
        .unwrap();
        let decision = decide_cover_disclosure(
            &request,
            DisclosureFacts {
                work: &self.work,
                prior: None,
                image_bound: true,
                next_sequence: cover::next_sequence(&tx, self.work.id()).await.unwrap(),
                now_ms: now,
            },
        )
        .unwrap();
        let CoverDisclosureDecision::Apply(record) = decision else {
            panic!("new consent fixture");
        };
        cover::insert_disclosure(&tx, &record).await.unwrap();
        if let Some(revision) = revision {
            cover::insert_metadata_binding(
                &tx,
                &record,
                revision,
                "synthetic exact-revision permission",
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
    }
}

#[tokio::test]
async fn metadata_cover_requires_distinct_scope_and_revalidates_real_bytes_after_revocation() {
    let world = MetadataWorld::new().await;
    let enabled = world.listener(true);
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    world.consent(None, true).await; // existing release clearance cannot widen itself
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    world.consent(Some(1), true).await; // wrong pinned revision is not authority
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    world.consent(Some(world.work.revision()), true).await;
    assert_eq!(
        world
            .listener(false)
            .metadata_work(WORK)
            .await
            .unwrap()
            .cover,
        None
    );
    let page = enabled
        .metadata_works("", None, None, 20, None)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 6);
    assert_eq!(page.items.iter().filter(|w| w.cover.is_some()).count(), 1);
    let actual = enabled.metadata_work(WORK).await.unwrap().cover.unwrap();
    assert_eq!(actual.sha256, PNG_SHA256);
    assert!(!serde_json::to_string(&actual)
        .unwrap()
        .contains("synthetic-private-provenance"));
    let app = metadata_cover_test_router(
        AppState {
            store: world.store.clone(),
            web_origin: STUDIO.into(),
        },
        enabled.clone(),
    );
    let path = format!("/api/v1/theatre/works/{WORK}/cover/{PNG_SHA256}");
    let response = app
        .clone()
        .oneshot(Request::get(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "public, no-cache");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        PNG
    );
    let old = app
        .clone()
        .oneshot(
            Request::get("/api/v1/theatre/works?presentation=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let old: discovery::CoveredWorkPage =
        serde_json::from_slice(&old.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(old.items.is_empty()); // no fake audio release
    let admitted = enabled.admit_cover(WORK, PNG_SHA256).await.unwrap();
    world.consent(None, false).await;
    assert!(matches!(
        enabled.open_cover(&admitted).await,
        Err(ListenerError::NotFound)
    ));
    let response = app
        .oneshot(Request::get(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    assert_eq!(
        world
            .fixture
            .count("SELECT count(*) FROM audio_releases", &[])
            .await,
        0
    );
}

#[tokio::test]
async fn metadata_image_expiry_is_half_open_and_metadata_withdrawal_revokes_the_url() {
    let world = MetadataWorld::new().await;
    let enabled = world.listener(true);
    world.consent(Some(world.work.revision()), true).await;
    assert!(enabled.metadata_work(WORK).await.unwrap().cover.is_some());
    world.clock.advance(60_000_000);
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    assert!(matches!(
        enabled.admit_cover(WORK, PNG_SHA256).await,
        Err(ListenerError::NotFound)
    ));
    world.consent(Some(world.work.revision()), true).await;
    assert!(enabled.metadata_work(WORK).await.unwrap().cover.is_some());
    catalog::seed_sample(
        &world.store,
        &SampleSeedRequest {
            operation_id: uuid::Uuid::new_v4().to_string(),
            action: SampleSeedAction::Withdraw,
            source_sha256: FICTIONAL_SAMPLE_SOURCE_SHA256.into(),
            operator_reference: "synthetic operator".into(),
            evidence_reference: "synthetic withdrawal evidence".into(),
            rights_statement: "withdraw isolated metadata fixture".into(),
            confirm_source: true,
            confirm_metadata_rights: true,
        },
        None,
    )
    .await
    .unwrap();
    assert!(matches!(
        enabled.metadata_work(WORK).await,
        Err(ListenerError::NotFound)
    ));
    assert!(matches!(
        enabled.admit_cover(WORK, PNG_SHA256).await,
        Err(ListenerError::NotFound)
    ));
}

/// Explicit loopback fixture for observing the real Web search against production HTTP/read
/// owners. It shows fictional source metadata, leaves every image undisclosed, and stops after
/// ten minutes. Never invoked by ordinary tests, preflight or CI.
#[tokio::test]
#[ignore = "explicit disposable browser session only"]
async fn metadata_search_browser_fixture() {
    let world = MetadataWorld::new().await;
    let app = metadata_cover_test_router(
        AppState {
            store: world.store.clone(),
            web_origin: "http://127.0.0.1:8082".into(),
        },
        world.listener(false),
    );
    let app = crate::adapters::inbound::http::with_browser_cors(
        app,
        "http://127.0.0.1:8082",
        crate::adapters::inbound::http::WebSurface::Theatre,
    )
    .unwrap();
    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080")
        .await
        .unwrap();
    eprintln!("Disposable fictional metadata HTTP fixture at http://127.0.0.1:8080; image consent absent; no audio.");
    let _ = tokio::time::timeout(
        std::time::Duration::from_secs(600),
        axum::serve(listener, app).into_future(),
    )
    .await;
}
