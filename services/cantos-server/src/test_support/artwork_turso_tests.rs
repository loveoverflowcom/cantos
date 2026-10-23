//! Actual run-owned native Turso artwork constraints, receipts and Catalog CAS.

use crate::contract::studio as dto;
use crate::test_support::{FixtureSql, ServerFixture};
use cantos_store_turso::error::ConstraintKind;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use super::{effective, insert_change, prior_operation, record_operation, ArtworkOperationKind};
use crate::contexts::catalog::{
    advance_owner_work,
    domain::{Lifecycle, OwnerId, Revision, Visibility, Work, WorkDetails, WorkId, WorkKind},
    owner_work, owner_work_now, owner_work_revision, CatalogCommand, CatalogStoreError, MemberRole,
};
use crate::contexts::media::{
    domain::{
        artwork::{artwork_at, decide_artwork_change, ArtworkDecision, SetWorkArtwork},
        image::{ImageAsset, ImageMetadata, PrepareImage},
    },
    turso::image::{insert_asset, insert_intent, load_asset_by_id, load_intent},
};
use crate::platform::store::{LockMode, Store, StoreError};

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

pub(in crate::contexts::media) struct Harness {
    pub(in crate::contexts::media) admin: ServerFixture,
    pub(in crate::contexts::media) store: Store,
}
impl Harness {
    pub(in crate::contexts::media) async fn new() -> Self {
        let admin = ServerFixture::new().await;
        admin.studio_caller("alice", ALICE).await;
        admin.studio_caller("bob", BOB).await;
        let store = admin.store().clone();
        Self { admin, store }
    }
    pub(in crate::contexts::media) async fn work(&self, token: &str) -> Work {
        self.store
            .catalog_execute(
                token,
                &Uuid::new_v4().to_string(),
                CatalogCommand::CreateWork {
                    id: WorkId::parse(&Uuid::new_v4().to_string()).unwrap(),
                    kind: WorkKind::Story,
                    details: WorkDetails {
                        title: "Ánh đèn cuối sân khấu".into(),
                        synopsis: "Synthetic private metadata".into(),
                        language: "vi-VN".into(),
                        visibility: Visibility::Private,
                        rights_refs: vec![],
                    },
                },
            )
            .await
            .unwrap()
            .work
    }
    async fn image(&self, work: &Work) -> ImageAsset {
        let mut client = self.store.connection().await.unwrap();
        let tx = client.transaction().await.unwrap();
        let owner = owner_work(&tx, ALICE, work.id(), LockMode::Update)
            .await
            .unwrap();
        let now = owner_work_now(&tx).await.unwrap();
        let request = PrepareImage::admit(&dto::CreateImageIntentRequest {
            operation_id: Uuid::new_v4().to_string(),
            id: Uuid::new_v4().to_string(),
            expected_sha256: "1".repeat(64),
            expected_byte_length: 67,
            content_type: dto::ImageMime::Png,
            declarations: dto::ImageDeclarations {
                provenance: "Synthetic SQL receipt fixture, no real image admission".into(),
                rights_refs: vec!["synthetic-rights".into()],
            },
        })
        .unwrap();
        let intent = crate::contexts::media::domain::image::ImageIntent::create(
            request,
            work.id(),
            &owner.actor,
            now,
        )
        .unwrap();
        let bytes = insert_intent(&tx, &intent).await.unwrap();
        record_operation(
            &tx,
            &owner.actor,
            intent.request().operation_id(),
            work.id(),
            ArtworkOperationKind::PrepareImage,
            &intent.request().fingerprint(work.id()).unwrap(),
            &bytes,
        )
        .await
        .unwrap();
        let metadata = ImageMetadata::new(
            "1".repeat(64),
            67,
            crate::contexts::media::domain::image::ImageMime::Png,
            1,
            1,
        )
        .unwrap();
        let asset = ImageAsset::create(&intent, metadata, now).unwrap();
        insert_asset(&tx, &asset, "private-fs", asset.metadata().sha256())
            .await
            .unwrap();
        tx.commit().await.unwrap();
        asset
    }
    async fn change(&self, work: &Work, asset: Option<&ImageAsset>) -> Work {
        let request = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_work_revision: work.revision(),
            artwork: asset.map(|asset| dto::WorkArtworkSelection {
                image_asset_id: asset.id().as_str().into(),
                image_asset_digest: asset.digest().into(),
                alt_text: "Bìa tím dưới ánh đèn".into(),
            }),
        })
        .unwrap();
        let mut client = self.store.connection().await.unwrap();
        let tx = client.transaction().await.unwrap();
        let owner = owner_work(&tx, ALICE, work.id(), LockMode::Update)
            .await
            .unwrap();
        let now = owner_work_now(&tx).await.unwrap();
        let ArtworkDecision::Apply(mutation) =
            decide_artwork_change(&owner.work, &request, asset, None, now).unwrap()
        else {
            panic!("new operation cannot replay")
        };
        let touched =
            advance_owner_work(&tx, ALICE, work.id(), request.expected_work_revision(), now)
                .await
                .unwrap();
        assert_eq!(&touched, mutation.work());
        insert_change(&tx, mutation.change()).await.unwrap();
        let bytes = serde_json::to_vec(mutation.response()).unwrap();
        record_operation(
            &tx,
            &owner.actor,
            request.operation_id(),
            work.id(),
            ArtworkOperationKind::SetArtwork,
            &request.fingerprint(work.id()).unwrap(),
            &bytes,
        )
        .await
        .unwrap();
        let prior = prior_operation(&tx, &owner.actor, request.operation_id())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(prior.response_bytes, bytes);
        assert_eq!(prior.artwork_response().unwrap(), *mutation.response());
        tx.commit().await.unwrap();
        touched
    }
}

#[tokio::test]
async fn owner_facade_restores_exact_old_lifecycle_and_denies_members_and_future_revisions() {
    let h = Harness::new().await;
    let initial = h.work(ALICE).await;
    let member = h
        .store
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::SetMember {
                work_id: initial.id().clone(),
                expected_revision: initial.revision(),
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap()
        .work;
    let archived = h
        .store
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::ArchiveWork {
                id: member.id().clone(),
                expected_revision: member.revision(),
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(archived.lifecycle(), Lifecycle::Archived);
    let mut client = h.store.connection().await.unwrap();
    let tx = client.transaction().await.unwrap();
    assert!(matches!(
        owner_work(&tx, BOB, initial.id(), LockMode::Share).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    let old = owner_work_revision(&tx, ALICE, initial.id(), Some(Revision::new(1).unwrap()))
        .await
        .unwrap();
    assert_eq!(old.work, initial);
    assert_eq!(old.work.lifecycle(), Lifecycle::Draft);
    assert!(matches!(
        owner_work_revision(&tx, ALICE, initial.id(), Some(Revision::new(4).unwrap())).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn effective_artwork_survives_edits_and_clear_is_a_historical_tombstone() {
    let h = Harness::new().await;
    let initial = h.work(ALICE).await;
    let asset = h.image(&initial).await;
    let selected = h.change(&initial, Some(&asset)).await;
    let edited = h
        .store
        .catalog_execute(
            ALICE,
            &Uuid::new_v4().to_string(),
            CatalogCommand::EditWork {
                id: initial.id().clone(),
                expected_revision: selected.revision(),
                details: selected.details().clone(),
            },
        )
        .await
        .unwrap()
        .work;
    let cleared = h.change(&edited, None).await;
    for (revision, expects_cover) in [(1, false), (2, true), (3, true), (4, false)] {
        let mut client = h.store.connection().await.unwrap();
        let tx = client.transaction().await.unwrap();
        let owner = owner_work_revision(
            &tx,
            ALICE,
            initial.id(),
            Some(Revision::new(revision).unwrap()),
        )
        .await
        .unwrap();
        let change = effective(
            &tx,
            &owner.actor,
            initial.id(),
            Revision::new(revision).unwrap(),
        )
        .await
        .unwrap();
        let response = artwork_at(&owner.work, &change.into_iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(response.artwork.is_some(), expects_cover);
        assert_eq!(response.work_revision, revision);
        if revision == 1 {
            assert_eq!(response.changed_at_ms, None);
        } else {
            assert!(response.changed_at_ms.is_some());
        }
        tx.commit().await.unwrap();
    }
    assert_eq!(cleared.revision(), 4);
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM media_work_artwork_changes", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        2
    );
}

#[tokio::test]
async fn rollback_discards_work_snapshot_head_epoch_change_and_operation_together() {
    let h = Harness::new().await;
    let initial = h.work(ALICE).await;
    let before:String=h.admin.query_one("SELECT json_array((SELECT head_revision FROM catalog_works WHERE id=?1),(SELECT count(*) FROM catalog_work_revisions),(SELECT revision FROM catalog_clock),(SELECT count(*) FROM media_work_artwork_changes),(SELECT count(*) FROM media_artwork_operations))",&[&initial.id().as_str()]).await.unwrap().get(0);
    let request = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
        operation_id: Uuid::new_v4().to_string(),
        expected_work_revision: 1,
        artwork: None,
    })
    .unwrap();
    let mut client = h.store.connection().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let owner = owner_work(&tx, ALICE, initial.id(), LockMode::Update)
        .await
        .unwrap();
    let now = owner_work_now(&tx).await.unwrap();
    let ArtworkDecision::Apply(mutation) =
        decide_artwork_change(&owner.work, &request, None, None, now).unwrap()
    else {
        panic!("new operation")
    };
    advance_owner_work(
        &tx,
        ALICE,
        initial.id(),
        request.expected_work_revision(),
        now,
    )
    .await
    .unwrap();
    insert_change(&tx, mutation.change()).await.unwrap();
    record_operation(
        &tx,
        &owner.actor,
        request.operation_id(),
        initial.id(),
        ArtworkOperationKind::SetArtwork,
        &request.fingerprint(initial.id()).unwrap(),
        &serde_json::to_vec(mutation.response()).unwrap(),
    )
    .await
    .unwrap();
    tx.rollback().await.unwrap();
    let after:String=h.admin.query_one("SELECT json_array((SELECT head_revision FROM catalog_works WHERE id=?1),(SELECT count(*) FROM catalog_work_revisions),(SELECT revision FROM catalog_clock),(SELECT count(*) FROM media_work_artwork_changes),(SELECT count(*) FROM media_artwork_operations))",&[&initial.id().as_str()]).await.unwrap().get(0);
    assert_eq!(before, after);
    assert_eq!(
        h.store.catalog_work(ALICE, initial.id()).await.unwrap(),
        initial
    );
}

#[tokio::test]
async fn checked_asset_restore_rejects_validly_typed_column_document_disagreement() {
    let h = Harness::new().await;
    let work = h.work(ALICE).await;
    let asset = h.image(&work).await;
    h.admin
        .batch_execute("DROP TRIGGER media_image_assets_no_update")
        .await
        .unwrap();
    h.admin
        .execute(
            "UPDATE media_image_assets SET width_px=2 WHERE id=?1",
            &[&asset.id().as_str()],
        )
        .await
        .unwrap();
    let mut client = h.store.connection().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let owner = owner_work(&tx, ALICE, work.id(), LockMode::Share)
        .await
        .unwrap();
    assert!(matches!(
        load_asset_by_id(&tx, &owner.actor, work.id(), asset.id()).await,
        Err(StoreError::CorruptRevision)
    ));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn immutable_tables_reject_update_delete_truncate_even_when_dml_is_granted() {
    let h = Harness::new().await;
    let initial = h.work(ALICE).await;
    let asset = h.image(&initial).await;
    h.change(&initial, Some(&asset)).await;
    for table in [
        "media_image_intents",
        "media_image_assets",
        "media_work_artwork_changes",
        "media_artwork_operations",
    ] {
        for statement in [
            format!("UPDATE {table} SET owner_id=owner_id"),
            format!("DELETE FROM {table}"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table}"),
            format!("INSERT INTO {table} SELECT * FROM {table} WHERE 1 ON CONFLICT DO UPDATE SET owner_id=excluded.owner_id"),
        ] {
            let error = h.admin.execute(&statement,&[]).await.unwrap_err();
            assert_eq!(error.guard(), Some("immutable.private_media"));
        }
    }
}

#[tokio::test]
async fn composite_foreign_keys_reject_wrong_owner_work_and_exact_asset_digest() {
    let h = Harness::new().await;
    let initial = h.work(ALICE).await;
    let bob = h.work(BOB).await;
    let asset = h.image(&initial).await;
    let intent_sql="INSERT INTO media_image_intents(id,work_id,owner_id,operation_id,expected_sha256,expected_byte_length,content_type,profile_id,created_at_ms,expires_at_ms,document_bytes,digest) VALUES(?1,?2,?3,?4,?5,1,'image/png','cover-image-v1',0,1800000,?6,?7)";
    let error = h
        .admin
        .execute(
            intent_sql,
            &[
                &Uuid::new_v4().to_string(),
                &bob.id().as_str(),
                &"alice",
                &Uuid::new_v4().to_string(),
                &"1".repeat(64),
                &vec![1u8],
                &format!("ii-r1:sha256:{}", "1".repeat(64)),
            ],
        )
        .await
        .unwrap_err();
    assert_eq!(error.constraint(), Some(ConstraintKind::ForeignKey));
    let sql="INSERT INTO media_work_artwork_changes(work_id,work_metadata_revision,owner_id,image_asset_id,image_asset_digest,alt_text,recorded_by,recorded_at_ms,document_bytes,digest) VALUES(?1,1,?2,?3,?4,'Synthetic cover',?2,0,?5,?6)";
    for (work, owner, digest) in [
        (&bob, "bob", asset.digest().to_owned()),
        (
            &initial,
            "alice",
            format!("ia-r1:sha256:{}", "2".repeat(64)),
        ),
    ] {
        let error = h
            .admin
            .execute(
                sql,
                &[
                    &work.id().as_str(),
                    &owner,
                    &asset.id().as_str(),
                    &digest,
                    &vec![1u8],
                    &format!("wa-r1:sha256:{}", "1".repeat(64)),
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(error.constraint(), Some(ConstraintKind::ForeignKey));
    }
}

#[tokio::test]
async fn checked_operation_replay_preserves_original_bytes_and_rejects_rechecksummed_bad_document()
{
    let h = Harness::new().await;
    let work = h.work(ALICE).await;
    let asset = h.image(&work).await;
    let mut client = h.store.connection().await.unwrap();
    let tx = client.transaction().await.unwrap();
    let owner = owner_work(&tx, ALICE, work.id(), LockMode::Share)
        .await
        .unwrap();
    let intent = load_intent(&tx, &owner.actor, work.id(), asset.intent_id())
        .await
        .unwrap()
        .unwrap();
    let operation = intent.intent.request().operation_id().clone();
    let prior = prior_operation(&tx, &owner.actor, &operation)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(prior.response_bytes, intent.document_bytes);
    assert_eq!(prior.image_intent().unwrap(), intent.intent);
    tx.commit().await.unwrap();
    let mut corrupt_document = intent.intent.document().clone();
    corrupt_document.request.expected_byte_length = 0;
    let bytes = serde_json::to_vec(&corrupt_document).unwrap();
    let checksum = format!("{:x}", Sha256::digest(&bytes));
    h.admin
        .batch_execute("DROP TRIGGER media_artwork_operations_no_update")
        .await
        .unwrap();
    h.admin.execute("UPDATE media_artwork_operations SET response_bytes=?1,response_checksum=?2 WHERE owner_id='alice' AND operation_id=?3",&[&bytes,&checksum,&operation.as_str()]).await.unwrap();
    let tx = client.transaction().await.unwrap();
    let prior = prior_operation(&tx, &OwnerId::parse("alice").unwrap(), &operation)
        .await
        .unwrap()
        .unwrap();
    assert!(matches!(
        prior.image_intent(),
        Err(StoreError::CorruptRevision)
    ));
    tx.rollback().await.unwrap();
}

#[tokio::test]
async fn baseline_replay_preserves_exact_catalog_snapshot_and_operation_bytes() {
    let h = Harness::new().await;
    let original = h.work(ALICE).await;
    let query="SELECT json_object('snapshot',(SELECT json_array(hex(snapshot),snapshot_digest,recorded_by) FROM catalog_work_revisions WHERE work_id=?1 AND revision=1),'operations',(SELECT json_group_array(json_array(actor_id,operation_id,fingerprint,hex(result_bytes),result_digest)) FROM catalog_operations),'head',(SELECT head_revision FROM catalog_works WHERE id=?1),'clock',(SELECT revision FROM catalog_clock))";
    let before: String = h
        .admin
        .query_one(query, &[&original.id().as_str()])
        .await
        .unwrap()
        .get(0);
    let Harness { mut admin, store } = h;
    drop(store);
    admin.restart().await;
    let h = Harness {
        store: admin.store().clone(),
        admin,
    };
    let after: String = h
        .admin
        .query_one(query, &[&original.id().as_str()])
        .await
        .unwrap()
        .get(0);
    assert_eq!(before, after);
    assert_eq!(
        h.store.catalog_work(ALICE, original.id()).await.unwrap(),
        original
    );
    assert_eq!(h.admin.query_one("SELECT (SELECT count(*) FROM media_image_intents)+(SELECT count(*) FROM media_image_assets)+(SELECT count(*) FROM media_work_artwork_changes)+(SELECT count(*) FROM media_artwork_operations)",&[]).await.unwrap().get::<_,i64>(0),0);
}

#[tokio::test]
async fn artwork_and_catalog_edit_race_accepts_one_cas_and_never_half_advances_a_cover() {
    use crate::contexts::catalog::domain::CatalogError;
    use std::sync::Arc;
    use tokio::sync::Barrier;

    let h = Harness::new().await;
    let initial = h.work(ALICE).await;
    let barrier = Arc::new(Barrier::new(2));
    let artwork_store = h.store.clone();
    let artwork_work = initial.clone();
    let artwork_barrier = barrier.clone();
    let artwork = async move {
        let request = SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
            operation_id: Uuid::new_v4().to_string(),
            expected_work_revision: 1,
            artwork: None,
        })
        .unwrap();
        let mut client = artwork_store.connection().await.unwrap();
        let tx = client.transaction().await.unwrap();
        artwork_barrier.wait().await;
        let owner = owner_work(&tx, ALICE, artwork_work.id(), LockMode::Update).await?;
        let now = owner_work_now(&tx).await?;
        let touched = advance_owner_work(
            &tx,
            ALICE,
            artwork_work.id(),
            request.expected_work_revision(),
            now,
        )
        .await?;
        let ArtworkDecision::Apply(mutation) =
            decide_artwork_change(&owner.work, &request, None, None, now).unwrap()
        else {
            panic!("new operation")
        };
        assert_eq!(&touched, mutation.work());
        insert_change(&tx, mutation.change()).await?;
        record_operation(
            &tx,
            &owner.actor,
            request.operation_id(),
            artwork_work.id(),
            ArtworkOperationKind::SetArtwork,
            &request.fingerprint(artwork_work.id()).unwrap(),
            &serde_json::to_vec(mutation.response()).unwrap(),
        )
        .await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok::<Work, CatalogStoreError>(touched)
    };
    let edit_store = h.store.clone();
    let edit_work = initial.clone();
    let edit = async move {
        barrier.wait().await;
        edit_store
            .catalog_execute(
                ALICE,
                &Uuid::new_v4().to_string(),
                CatalogCommand::EditWork {
                    id: edit_work.id().clone(),
                    expected_revision: 1,
                    details: edit_work.details().clone(),
                },
            )
            .await
            .map(|outcome| outcome.work)
    };
    let (artwork, edit) = tokio::join!(artwork, edit);
    let artwork_won = artwork.is_ok();
    assert_ne!(artwork.is_ok(), edit.is_ok());
    let rejected = if artwork_won {
        edit.unwrap_err()
    } else {
        artwork.unwrap_err()
    };
    assert!(matches!(
        rejected,
        CatalogStoreError::Domain(CatalogError::RevisionConflict {
            current_revision: 2,
            expected_revision: 1,
        })
    ));
    assert_eq!(
        h.store
            .catalog_work(ALICE, initial.id())
            .await
            .unwrap()
            .revision(),
        2
    );
    let counts = h.admin.query_one("SELECT (SELECT count(*) FROM catalog_work_revisions WHERE work_id=?1),(SELECT count(*) FROM media_work_artwork_changes WHERE work_id=?1),(SELECT count(*) FROM media_artwork_operations WHERE work_id=?1),(SELECT revision FROM catalog_clock)", &[&initial.id().as_str()]).await.unwrap();
    assert_eq!(counts.get::<_, i64>(0), 2);
    assert_eq!(counts.get::<_, i64>(1), i64::from(artwork_won));
    assert_eq!(counts.get::<_, i64>(2), i64::from(artwork_won));
    assert_eq!(counts.get::<_, i64>(3), 2);
}

#[test]
fn recorded_target_grammar_matches_inspected_private_storage_contract() {
    use crate::contexts::media::turso::image::target_valid;
    for value in ["a", "private-fs", "a0", &"a".repeat(63)] {
        assert!(target_valid(value), "valid inspected target: {value}");
    }
    for value in [
        "",
        "A",
        "0a",
        "-a",
        "a-",
        "a_b",
        "a/b",
        "a.b",
        "a\n",
        "á",
        &"a".repeat(64),
    ] {
        assert!(!target_valid(value), "invalid inspected target: {value}");
    }
}
