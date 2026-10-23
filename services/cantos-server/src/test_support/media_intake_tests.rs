//! Real Turso + real filesystem + real synthetic Opus. A pause wraps the filesystem port;
//! it delays after durable persistence and never substitutes for bytes, SQL or inspection.
use super::*;
use crate::{
    platform::{
        store::{utc_timestamp, Store, StoreError},
        turso::Acknowledgement,
    },
    test_support::{
        editorial::{fixture, request, ALICE, SCRIPT},
        ServerFixture,
    },
};
use crate::contract::studio::{AudioIntentResponse, CreateAudioIntentRequest};
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::Notify;
static MEDIA_CASE: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());
struct PausedFilesystem {
    inner: Arc<FilesystemStorage>,
    entered: Arc<Notify>,
    release: Arc<Notify>,
    pause: AtomicBool,
}
impl PrivateStorage for PausedFilesystem {
    fn put_if_absent(
        &self,
        payload: super::super::port::VerifiedPayload,
    ) -> super::super::port::StorageFuture<()> {
        let future = self.inner.put_if_absent(payload);
        let entered = self.entered.clone();
        let release = self.release.clone();
        let pause = self.pause.swap(false, Ordering::SeqCst);
        Box::pin(async move {
            future.await?;
            if pause {
                entered.notify_one();
                release.notified().await;
            }
            Ok(())
        })
    }
    fn verify(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
    ) -> super::super::port::StorageFuture<()> {
        self.inner.verify(key, integrity)
    }
    fn open_verified(
        &self,
        key: ObjectKey,
        integrity: ObjectIntegrity,
        range: Option<super::super::super::domain::object_storage::NormalizedByteRange>,
    ) -> super::super::port::StorageFuture<super::super::port::StorageRead> {
        self.inner.open_verified(key, integrity, range)
    }
}
fn opus() -> Vec<u8> {
    let out = std::process::Command::new("ffmpeg")
        .args([
            "-hide_banner",
            "-loglevel",
            "error",
            "-f",
            "lavfi",
            "-i",
            "sine=frequency=440:sample_rate=48000:duration=1",
            "-ac",
            "1",
            "-c:a",
            "libopus",
            "-b:a",
            "20k",
            "-vbr",
            "off",
            "-f",
            "ogg",
            "pipe:1",
        ])
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    out.stdout
}
async fn world(
    pause: bool,
) -> (
    ServerFixture,
    Store,
    std::path::PathBuf,
    Arc<PausedFilesystem>,
    CreateAudioIntentRequest,
    AudioIntentResponse,
) {
    let f = fixture().await;
    let saved = f.store().save(ALICE, SCRIPT, request(0)).await.unwrap();
    let root = std::fs::canonicalize(f.dir())
        .unwrap()
        .join("private-audio");
    let adapter = Arc::new(PausedFilesystem {
        inner: Arc::new(FilesystemStorage::new(&root).unwrap()),
        entered: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
        pause: AtomicBool::new(pause),
    });
    let target = StorageTargetId::parse("legacy-filesystem").unwrap();
    let registry = StorageRegistry {
        active: target.clone(),
        targets: BTreeMap::from([(target, adapter.clone() as Arc<dyn PrivateStorage>)]),
    };
    let mut store = f.store().clone();
    store.media_storage = Some(Arc::new(registry));
    let request = CreateAudioIntentRequest {
        operation_id: uuid::Uuid::new_v4().to_string(),
        expected_revision: 1,
        expected_content_digest: saved.content_digest,
        profile_id: "opus-mono-20-v1".into(),
    };
    let intent = store
        .create_audio_intent(ALICE, SCRIPT, request.clone())
        .await
        .unwrap();
    (f, store, root, adapter, request, intent)
}
#[tokio::test]
async fn owner_deactivation_after_inspection_is_rechecked_before_receipt_commit() {
    let _case = MEDIA_CASE.lock().await;
    let (f, store, root, adapter, _, intent) = world(true).await;
    let upload_store = store.clone();
    let id = intent.id.clone();
    let upload = tokio::spawn(async move {
        upload_store
            .upload_audio_intent(ALICE, SCRIPT, &id, opus())
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(8),
        adapter.entered.notified(),
    )
    .await
    .unwrap();
    assert_eq!(std::fs::read_dir(root.join("objects")).unwrap().count(), 1);
    // This independent mutation must acquire the writer while object I/O is paused.
    f.exec("UPDATE actors SET active=0 WHERE id='alice'", &[])
        .await
        .unwrap();
    adapter.release.notify_one();
    assert!(matches!(
        upload.await.unwrap(),
        Err(StoreError::Unauthenticated)
    ));
    assert_eq!(
        f.count("SELECT count(*) FROM audio_intake_receipts", &[])
            .await,
        0
    );
}
#[tokio::test]
async fn cancellation_after_private_persistence_has_no_metadata_and_restart_retry_recovers() {
    let _case = MEDIA_CASE.lock().await;
    let (mut f, store, root, adapter, _, intent) = world(true).await;
    let bytes = opus();
    let sent = bytes.clone();
    let upload_store = store.clone();
    let id = intent.id.clone();
    let upload = tokio::spawn(async move {
        upload_store
            .upload_audio_intent(ALICE, SCRIPT, &id, sent)
            .await
    });
    tokio::time::timeout(
        std::time::Duration::from_secs(8),
        adapter.entered.notified(),
    )
    .await
    .unwrap();
    upload.abort();
    assert!(upload.await.unwrap_err().is_cancelled());
    adapter.release.notify_one();
    drop(store);
    f.restart().await;
    let store = f.store().clone().with_audio_storage(&root).unwrap();
    assert!(store
        .load_audio_intent(ALICE, SCRIPT, &intent.id)
        .await
        .unwrap()
        .receipt
        .is_none());
    let receipt = store
        .upload_audio_intent(ALICE, SCRIPT, &intent.id, bytes.clone())
        .await
        .unwrap();
    assert_eq!(
        store
            .upload_audio_intent(ALICE, SCRIPT, &intent.id, bytes)
            .await
            .unwrap(),
        receipt
    );
    assert_eq!(
        f.count("SELECT count(*) FROM audio_intake_receipts", &[])
            .await,
        1
    );
    assert_eq!(std::fs::read_dir(root.join("objects")).unwrap().count(), 1);
}
#[tokio::test]
async fn terminated_database_transaction_cannot_commit_acceptance_and_restart_can_reconcile() {
    let _case = MEDIA_CASE.lock().await;
    for acknowledgement in [
        None,
        Some(Acknowledgement::LostAfterCommit),
        Some(Acknowledgement::LostAfterRollback),
    ] {
        let (mut f, store, root, _, request, _) = world(false).await;
        let bytes = opus();
        // First accept real bytes, then bind the same content object to a separate pending intent.
        let first = store
            .create_audio_intent(
                ALICE,
                SCRIPT,
                CreateAudioIntentRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    ..request.clone()
                },
            )
            .await
            .unwrap();
        let mut receipt = store
            .upload_audio_intent(ALICE, SCRIPT, &first.id, bytes.clone())
            .await
            .unwrap();
        let pending = store
            .create_audio_intent(
                ALICE,
                SCRIPT,
                CreateAudioIntentRequest {
                    operation_id: uuid::Uuid::new_v4().to_string(),
                    ..request
                },
            )
            .await
            .unwrap();
        receipt.id = uuid::Uuid::new_v4().to_string();
        receipt.intent_id = pending.id.clone();
        receipt.operation_id = pending.operation_id.clone();
        let mut uncertain = store.clone();
        if let Some(a) = acknowledgement {
            uncertain.acknowledgement = a;
        }
        let mut connection = uncertain.connection().await.unwrap();
        let tx = connection.transaction().await.unwrap();
        receipt.accepted_at = utc_timestamp(tx.now_micros()).unwrap();
        let stored = RegistryStoredObject {
            object_key: receipt.media.sha256.clone(),
            storage_target: StorageTargetId::parse("legacy-filesystem").unwrap(),
        };
        crate::contexts::media::turso::insert_receipt(&tx, &receipt, &stored)
            .await
            .unwrap();
        if acknowledgement.is_some() {
            assert!(tx.commit().await.unwrap_err().outcome_unknown());
        } else {
            drop(tx);
        }
        drop(connection);
        drop(uncertain);
        drop(store);
        f.restart().await;
        let store = f.store().clone().with_audio_storage(&root).unwrap();
        let durable = matches!(acknowledgement, Some(Acknowledgement::LostAfterCommit));
        assert_eq!(
            store
                .load_audio_intent(ALICE, SCRIPT, &pending.id)
                .await
                .unwrap()
                .receipt,
            if durable { Some(receipt.clone()) } else { None }
        );
        let recovered = store
            .upload_audio_intent(ALICE, SCRIPT, &pending.id, bytes.clone())
            .await
            .unwrap();
        if durable {
            assert_eq!(recovered, receipt);
        }
        assert_eq!(
            store
                .upload_audio_intent(ALICE, SCRIPT, &pending.id, bytes)
                .await
                .unwrap(),
            recovered
        );
        assert_eq!(
            f.count("SELECT count(*) FROM audio_intake_receipts", &[])
                .await,
            2
        );
    }
}
#[tokio::test]
async fn upload_intent_lost_acknowledgement_reconciles_by_actor_and_operation() {
    let _case = MEDIA_CASE.lock().await;
    for acknowledgement in [
        Acknowledgement::LostAfterCommit,
        Acknowledgement::LostAfterRollback,
    ] {
        let (mut f, store, root, _, mut request, _) = world(false).await;
        request.operation_id = uuid::Uuid::new_v4().to_string();
        let mut uncertain = store.clone();
        uncertain.acknowledgement = acknowledgement;
        assert!(matches!(
            uncertain
                .create_audio_intent(ALICE, SCRIPT, request.clone())
                .await,
            Err(StoreError::Unavailable)
        ));
        let durable = matches!(acknowledgement, Acknowledgement::LostAfterCommit);
        assert_eq!(
            f.count("SELECT count(*) FROM audio_upload_intents", &[])
                .await,
            if durable { 2 } else { 1 }
        );
        drop(uncertain);
        drop(store);
        f.restart().await;
        let store = f.store().clone().with_audio_storage(&root).unwrap();
        let recovered = store
            .create_audio_intent(ALICE, SCRIPT, request.clone())
            .await
            .unwrap();
        assert_eq!(
            store
                .create_audio_intent(ALICE, SCRIPT, request)
                .await
                .unwrap(),
            recovered
        );
        assert_eq!(
            f.count("SELECT count(*) FROM audio_upload_intents", &[])
                .await,
            2
        );
    }
}
