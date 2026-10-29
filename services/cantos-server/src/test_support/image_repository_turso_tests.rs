use crate::test_support::FixtureSql;
// Production TursoArtworkRepository transaction methods against a disposable database. The
// AVIF_RGB is really decoded; the recorded object location is a synthetic SQL fixture. This test
// does not call private storage, prove object persistence or enable image HTTP routes.
use std::sync::Arc;

use crate::contract::studio as dto;
use tokio::sync::Barrier;
use uuid::Uuid;

use super::super::{
    ArtworkApplicationError, ArtworkRepository, OwnerWorkScope, RecordedImageLocation,
};
use super::TursoArtworkRepository;
use crate::contexts::{
    catalog::{domain::CatalogError, CatalogCommand, CatalogStoreError},
    media::{
        domain::{
            artwork::SetWorkArtwork,
            image::{ImageIntentId, ImageMime, PrepareImage},
        },
        image_intake::ImageAdmission,
        turso::artwork::tests::Harness,
    },
};

const TOKEN: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const AVIF_RGB: &[u8] =
    include_bytes!("../contexts/media/image_intake/fixtures/python-rgb-2x3.avif");
const AVIF_RGB_SHA256: &str = "ed4244268e5d34fc1e9ed66e65822d5c58333a4de4376c0d14023ebff966891f";

#[tokio::test]
async fn production_repository_prepare_finalize_select_and_catalog_cas_race_preserve_exact_receipts(
) {
    let h = Harness::new().await;
    let work = h.work(TOKEN).await;
    h.admin.batch_execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','synthetic-rights','Synthetic fixture declaration; no rights approval')").await.unwrap();
    let repository = TursoArtworkRepository(h.store.clone());
    let scope = OwnerWorkScope::new(TOKEN, work.id().as_str()).unwrap();
    let request = PrepareImage::admit(&dto::CreateImageIntentRequest {
        operation_id: Uuid::new_v4().to_string(),
        id: Uuid::new_v4().to_string(),
        expected_sha256: AVIF_RGB_SHA256.into(),
        expected_byte_length: AVIF_RGB.len() as u64,
        content_type: dto::ImageMime::Avif,
        declarations: dto::ImageDeclarations {
            provenance: "Independent Python 2x3 synthetic image".into(),
            rights_refs: vec!["synthetic-rights".into()],
        },
    })
    .unwrap();
    let prepared = repository
        .prepare(scope.clone(), request.clone())
        .await
        .unwrap();
    let prepare_replay = repository
        .prepare(scope.clone(), request.clone())
        .await
        .unwrap();
    assert_eq!(prepared.json_bytes, prepare_replay.json_bytes);
    assert_eq!(prepared.value, prepare_replay.value);
    assert_eq!(h.store.catalog_work(TOKEN, work.id()).await.unwrap(), work);

    let image = ImageAdmission::try_acquire()
        .unwrap()
        .inspect(AVIF_RGB.to_vec(), ImageMime::Avif)
        .await
        .unwrap();
    assert_eq!(image.metadata().sha256(), AVIF_RGB_SHA256);
    assert_eq!(
        (image.metadata().width_px(), image.metadata().height_px()),
        (2, 3)
    );
    let id = ImageIntentId::parse(&prepared.value.id).unwrap();
    let accepted = repository
        .finalize(
            scope.clone(),
            id.clone(),
            image.metadata().clone(),
            RecordedImageLocation {
                storage_target: "synthetic-sql-fixture".into(),
                object_key: AVIF_RGB_SHA256.into(),
            },
        )
        .await
        .unwrap();
    // Equal admission on another synthetic target must return the original accepted receipt.
    let accepted_replay = repository
        .finalize(
            scope.clone(),
            id,
            image.metadata().clone(),
            RecordedImageLocation {
                storage_target: "synthetic-other-target".into(),
                object_key: AVIF_RGB_SHA256.into(),
            },
        )
        .await
        .unwrap();
    assert_eq!(accepted.json_bytes, accepted_replay.json_bytes);
    assert_eq!(accepted.value, accepted_replay.value);
    assert_eq!(h.store.catalog_work(TOKEN, work.id()).await.unwrap(), work);
    drop(image);

    let selection = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_work_revision: 1,
        artwork: Some(dto::WorkArtworkSelection {
            image_asset_id: accepted.value.id.clone(),
            image_asset_digest: accepted.value.digest.clone(),
            alt_text: "Bìa thử nghiệm hai nhân ba".into(),
        }),
    })
    .unwrap();
    let context = repository
        .artwork_context(scope.clone(), selection.clone())
        .await
        .unwrap();
    let selected = repository
        .set_artwork(scope.clone(), selection.clone(), context.asset)
        .await
        .unwrap();
    assert_eq!(selected.value.work_revision, 2);
    assert_eq!(selected.value.artwork, selection.document().artwork);
    let replay = repository
        .set_artwork(scope.clone(), selection.clone(), None)
        .await
        .unwrap();
    assert_eq!(selected.json_bytes, replay.json_bytes);
    assert_eq!(
        h.store
            .catalog_work(TOKEN, work.id())
            .await
            .unwrap()
            .revision(),
        2
    );

    let clear = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_work_revision: 2,
        artwork: None,
    })
    .unwrap();
    let barrier = Arc::new(Barrier::new(2));
    let artwork_barrier = barrier.clone();
    let artwork_repository = TursoArtworkRepository(h.store.clone());
    let artwork_scope = scope.clone();
    let artwork = async move {
        artwork_barrier.wait().await;
        artwork_repository
            .set_artwork(artwork_scope, clear, None)
            .await
    };
    let catalog_store = h.store.clone();
    let edit_work = h.store.catalog_work(TOKEN, work.id()).await.unwrap();
    let edit = async move {
        barrier.wait().await;
        catalog_store
            .catalog_execute(
                TOKEN,
                &Uuid::new_v4().to_string(),
                CatalogCommand::EditWork {
                    id: edit_work.id().clone(),
                    expected_revision: 2,
                    details: edit_work.details().clone(),
                },
            )
            .await
    };
    let (clear, edit) = tokio::join!(artwork, edit);
    assert_ne!(clear.is_ok(), edit.is_ok());
    let clear_won = clear.is_ok();
    if clear_won {
        assert!(matches!(
            edit,
            Err(CatalogStoreError::Domain(CatalogError::RevisionConflict {
                current_revision: 3,
                expected_revision: 2,
            }))
        ));
    } else {
        assert!(matches!(
            clear,
            Err(ArtworkApplicationError::Artwork(
                crate::contexts::media::domain::artwork::ArtworkError::Catalog(
                    CatalogError::RevisionConflict {
                        current_revision: 3,
                        expected_revision: 2,
                    }
                )
            ))
        ));
    }
    let counts=h.admin.query_one("SELECT (SELECT head_revision FROM catalog_works WHERE id=$1),(SELECT count(*) FROM catalog_work_revisions WHERE work_id=$1),(SELECT count(*) FROM media_work_artwork_changes WHERE work_id=$1),(SELECT count(*) FROM media_artwork_operations WHERE work_id=$1),(SELECT revision FROM catalog_clock)",&[&work.id().as_str()]).await.unwrap();
    assert_eq!(counts.get::<_, i64>(0), 3);
    assert_eq!(counts.get::<_, i64>(1), 3);
    assert_eq!(counts.get::<_, i64>(2), if clear_won { 2 } else { 1 });
    assert_eq!(counts.get::<_, i64>(3), if clear_won { 3 } else { 2 });
    assert_eq!(counts.get::<_, i64>(4), 3);
    // The old exact operation remains byte-identical after a newer unrelated edit/clear.
    let old_replay = repository
        .set_artwork(scope, selection, None)
        .await
        .unwrap();
    assert_eq!(old_replay.json_bytes, selected.json_bytes);
    assert_eq!(old_replay.value, selected.value);
}

#[tokio::test]
async fn artwork_prepare_finalize_and_selection_reconcile_actual_unknown_commits() {
    for ack in [
        crate::platform::turso::Acknowledgement::LostAfterCommit,
        crate::platform::turso::Acknowledgement::LostAfterRollback,
    ] {
        let h = Harness::new().await;
        let work = h.work(TOKEN).await;
        h.admin.batch_execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','synthetic-rights','Synthetic fixture declaration; no rights approval')").await.unwrap();
        let repository = TursoArtworkRepository(h.store.clone());
        let scope = OwnerWorkScope::new(TOKEN, work.id().as_str()).unwrap();
        let request = PrepareImage::admit(&dto::CreateImageIntentRequest {
            operation_id: Uuid::new_v4().to_string(),
            id: Uuid::new_v4().to_string(),
            expected_sha256: AVIF_RGB_SHA256.into(),
            expected_byte_length: AVIF_RGB.len() as u64,
            content_type: dto::ImageMime::Avif,
            declarations: dto::ImageDeclarations {
                provenance: "Independent Python 2x3 synthetic image".into(),
                rights_refs: vec!["synthetic-rights".into()],
            },
        })
        .unwrap();
        let mut faulty = h.store.clone();
        faulty.acknowledgement = ack;
        let uncertain = TursoArtworkRepository(faulty);
        assert!(matches!(
            uncertain.prepare(scope.clone(), request.clone()).await,
            Err(ArtworkApplicationError::Storage(
                crate::platform::store::StoreError::Unavailable
            ))
        ));
        let prepared = repository
            .prepare(scope.clone(), request.clone())
            .await
            .unwrap();
        let prepare_replay = repository
            .prepare(scope.clone(), request.clone())
            .await
            .unwrap();
        assert_eq!(prepared.json_bytes, prepare_replay.json_bytes);
        assert_eq!(prepared.value, prepare_replay.value);
        assert_eq!(h.store.catalog_work(TOKEN, work.id()).await.unwrap(), work);

        let image = ImageAdmission::try_acquire()
            .unwrap()
            .inspect(AVIF_RGB.to_vec(), ImageMime::Avif)
            .await
            .unwrap();
        assert_eq!(image.metadata().sha256(), AVIF_RGB_SHA256);
        assert_eq!(
            (image.metadata().width_px(), image.metadata().height_px()),
            (2, 3)
        );
        let id = ImageIntentId::parse(&prepared.value.id).unwrap();
        assert!(matches!(
            uncertain
                .finalize(
                    scope.clone(),
                    id.clone(),
                    image.metadata().clone(),
                    RecordedImageLocation {
                        storage_target: "synthetic-sql-fixture".into(),
                        object_key: AVIF_RGB_SHA256.into()
                    }
                )
                .await,
            Err(ArtworkApplicationError::Storage(
                crate::platform::store::StoreError::Unavailable
            ))
        ));
        let accepted = repository
            .finalize(
                scope.clone(),
                id.clone(),
                image.metadata().clone(),
                RecordedImageLocation {
                    storage_target: "synthetic-sql-fixture".into(),
                    object_key: AVIF_RGB_SHA256.into(),
                },
            )
            .await
            .unwrap();
        // Equal admission on another synthetic target must return the original accepted receipt.
        let accepted_replay = repository
            .finalize(
                scope.clone(),
                id,
                image.metadata().clone(),
                RecordedImageLocation {
                    storage_target: "synthetic-other-target".into(),
                    object_key: AVIF_RGB_SHA256.into(),
                },
            )
            .await
            .unwrap();
        assert_eq!(accepted.json_bytes, accepted_replay.json_bytes);
        assert_eq!(accepted.value, accepted_replay.value);
        assert_eq!(h.store.catalog_work(TOKEN, work.id()).await.unwrap(), work);
        drop(image);

        let selection = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_work_revision: 1,
            artwork: Some(dto::WorkArtworkSelection {
                image_asset_id: accepted.value.id.clone(),
                image_asset_digest: accepted.value.digest.clone(),
                alt_text: "Bìa thử nghiệm hai nhân ba".into(),
            }),
        })
        .unwrap();
        let context = repository
            .artwork_context(scope.clone(), selection.clone())
            .await
            .unwrap();
        assert!(matches!(
            uncertain
                .set_artwork(scope.clone(), selection.clone(), context.asset.clone())
                .await,
            Err(ArtworkApplicationError::Storage(
                crate::platform::store::StoreError::Unavailable
            ))
        ));
        let selected = repository
            .set_artwork(scope.clone(), selection.clone(), context.asset)
            .await
            .unwrap();
        assert_eq!(selected.value.work_revision, 2);
        assert_eq!(selected.value.artwork, selection.document().artwork);
        let replay = repository
            .set_artwork(scope.clone(), selection.clone(), None)
            .await
            .unwrap();
        assert_eq!(selected.json_bytes, replay.json_bytes);
        assert_eq!(
            h.store
                .catalog_work(TOKEN, work.id())
                .await
                .unwrap()
                .revision(),
            2
        );
    }
}
