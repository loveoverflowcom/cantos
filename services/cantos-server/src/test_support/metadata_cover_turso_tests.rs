//! Synthetic permission records, one generic owner-published metadata Work and an existing 2×3 AVIF_RGB
//! fixture in a real disposable Turso/private object root. No generated cover or real consent is
//! substituted. The non-login principal is populated through checked domain/SQL fixture seams;
//! this does not establish an operator image-intake workflow or authenticate a human owner.
use super::{AVIF_RGB, AVIF_RGB_SHA256, ORIGIN, STUDIO};
use crate::{
    adapters::inbound::http::{metadata_cover_test_router, AppState},
    contexts::{
        catalog::domain::{Work, WorkId},
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
    contract::studio as dto,
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

const WORK: &str = "65190000-0000-4000-8000-000000000001";

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
        let mut seed = crate::test_support::WorkSeed::podcast(
            0x6519_0000,
            "Mùa trăng metadata",
            &["Chương tổng hợp"],
        );
        seed.units[0].stage = crate::test_support::Stage::Staged;
        let seeded = fixture.seed_work(None, &seed).await;
        assert_eq!(seeded.work_id, WORK);
        fixture.studio_caller("alice", super::TOKEN).await;
        let work = store
            .catalog_work(super::TOKEN, &WorkId::parse(WORK).unwrap())
            .await
            .unwrap();
        let mut connection = store.connection().await.unwrap();
        let now = fixture.clock_reading().unwrap() / 1000;
        let intent_id = uuid::Uuid::new_v4().to_string();
        let intent = ImageIntent::create(
            PrepareImage::admit(&dto::CreateImageIntentRequest {
                operation_id: uuid::Uuid::new_v4().to_string(),
                id: intent_id,
                expected_sha256: AVIF_RGB_SHA256.into(),
                expected_byte_length: AVIF_RGB.len() as u64,
                content_type: dto::ImageMime::Avif,
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
            .inspect(AVIF_RGB.to_vec(), ImageMime::Avif)
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
        tx.commit().await.unwrap();
        fixture.publish_work_metadata(&seeded).await;
        Self {
            clock,
            fixture,
            store,
            work,
            image: asset,
        }
    }

    fn listener(&self) -> ListenerService {
        ListenerService::new(self.store.clone())
            .with_cover_delivery(PublicMediaOrigin::parse(ORIGIN).unwrap(), &self.store)
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
async fn generation_cover_requires_image_clearance_and_revalidates_real_bytes_after_revocation() {
    let world = MetadataWorld::new().await;
    let enabled = world.listener();
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    world.consent(None, true).await; // Per-image display clearance remains independent of text publication.
    let page = enabled
        .metadata_works("", None, None, 20, None)
        .await
        .unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items.iter().filter(|w| w.cover.is_some()).count(), 1);
    let actual = enabled.metadata_work(WORK).await.unwrap().cover.unwrap();
    assert_eq!(actual.sha256, AVIF_RGB_SHA256);
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
    let path = format!("/api/v1/theatre/works/{WORK}/cover/{AVIF_RGB_SHA256}");
    let response = app
        .clone()
        .oneshot(Request::get(&path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()["cache-control"], "public, no-cache");
    assert_eq!(
        response.into_body().collect().await.unwrap().to_bytes(),
        AVIF_RGB
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
    assert_eq!(
        old.status(),
        StatusCode::BAD_REQUEST,
        "retired selector is rejected"
    );
    let admitted = enabled.admit_cover(WORK, AVIF_RGB_SHA256).await.unwrap();
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
    let enabled = world.listener();
    world.consent(Some(world.work.revision()), true).await;
    assert!(enabled.metadata_work(WORK).await.unwrap().cover.is_some());
    world.clock.advance(60_000_000);
    assert_eq!(enabled.metadata_work(WORK).await.unwrap().cover, None);
    assert!(matches!(
        enabled.admit_cover(WORK, AVIF_RGB_SHA256).await,
        Err(ListenerError::NotFound)
    ));
    world.consent(Some(world.work.revision()), true).await;
    assert!(enabled.metadata_work(WORK).await.unwrap().cover.is_some());
    let state = world
        .store
        .metadata_publication_state(super::TOKEN, WORK)
        .await
        .unwrap();
    crate::contexts::media::MetadataPublicationService::new(world.store.clone())
        .settle(
            super::TOKEN,
            WORK,
            dto::MetadataPublicationRequest::Withdraw {
                operation_id: uuid::Uuid::new_v4().to_string(),
                expected_version: state.version,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        enabled.metadata_work(WORK).await,
        Err(ListenerError::NotFound)
    ));
    assert!(matches!(
        enabled.admit_cover(WORK, AVIF_RGB_SHA256).await,
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
        world.listener(),
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
