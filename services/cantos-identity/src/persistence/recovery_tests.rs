//! Real Identity/Turso recovery primitives; no RPC/login or production acceptance implied.
use super::*;
use cantos_store_turso::{
    recovery::RecoveryFault,
    testing::{FixedClock, TestRun},
};
use std::{fs, os::unix::fs::DirBuilderExt, path::Path};

fn private(path: &Path) {
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}
fn subject() -> SubjectId {
    SubjectId::from_bytes([1; 16]).unwrap()
}
fn operation(n: u8) -> OperationId {
    OperationId::from_bytes([n; 16]).unwrap()
}

async fn create(run: &TestRun) -> (IdentityStore, PathBuf, PathBuf) {
    let root = run.dir().canonicalize().unwrap();
    private(&root.join("database"));
    private(&root.join("authority"));
    let database = root.join("database/identity.db");
    let fence = root.join("authority/identity.fence");
    let store = IdentityStore::open(&database, Intent::Migrate, FixedClock::new(1))
        .await
        .unwrap()
        .enroll_recovery_fence(&fence)
        .await
        .unwrap();
    (store, database, fence)
}

async fn reopen(database: &Path, fence: &Path) -> IdentityStore {
    IdentityStore::open_with_recovery_fence(
        database,
        Intent::Serve,
        FixedClock::new(2),
        Some(fence.to_owned()),
    )
    .await
    .unwrap()
}

#[tokio::test]
async fn epoch_rotation_preserves_disabled_state_and_exact_receipt_replay() {
    let run = TestRun::new("identity-recovery-disabled");
    let (store, database, fence) = create(&run).await;
    let old_epoch = store.epoch();
    store
        .create_subject(operation(1), subject(), old_epoch)
        .await
        .unwrap();
    private(&run.path("stale"));
    let stale = run.path("stale/identity.db");
    store.database.backup_to(&stale).await.unwrap();
    store
        .disable_subject(
            operation(2),
            subject(),
            SecurityVersion::new(1).unwrap(),
            old_epoch,
        )
        .await
        .unwrap();
    let store = store
        .rotate_restore_epoch(operation(3), old_epoch)
        .await
        .unwrap();
    let new_epoch = store.epoch();
    assert_ne!(new_epoch, old_epoch);
    let snapshot = store.subject(subject()).await.unwrap().unwrap();
    assert_eq!(snapshot.state, AccountState::Disabled);
    assert_eq!(snapshot.version, SecurityVersion::new(2).unwrap());
    assert_eq!(snapshot.epoch, new_epoch);
    let floor = fs::read(&fence).unwrap();
    let store = store
        .rotate_restore_epoch(operation(3), old_epoch)
        .await
        .unwrap();
    assert_eq!(store.epoch(), new_epoch);
    assert_eq!(fs::read(&fence).unwrap(), floor);
    drop(store);
    let stale_open = IdentityStore::open_with_recovery_fence(
        &stale,
        Intent::Serve,
        FixedClock::new(2),
        Some(fence.clone()),
    )
    .await;
    assert!(matches!(
        stale_open,
        Err(IdentityStoreError::Open(OpenError::Store(
            StoreFault::Recovery(RecoveryFault::Mismatch)
        )))
    ));
    let store = reopen(&database, &fence).await;
    assert_eq!(store.epoch(), new_epoch); // Ordinary restart never rotates.
    assert_eq!(
        store.subject(subject()).await.unwrap().unwrap().state,
        AccountState::Disabled
    );
    assert!(matches!(
        store.rotate_restore_epoch(operation(4), old_epoch).await,
        Err(IdentityStoreError::EpochChanged)
    ));
}

#[tokio::test]
async fn unfenced_epoch_rotation_is_refused_without_changing_metadata() {
    let run = TestRun::new("identity-recovery-unfenced");
    let database = run.path("identity.db");
    let store = IdentityStore::open(&database, Intent::Migrate, FixedClock::new(1))
        .await
        .unwrap();
    let epoch = store.epoch();
    assert!(matches!(
        store.rotate_restore_epoch(operation(3), epoch).await,
        Err(IdentityStoreError::Storage(StoreFault::Recovery(
            RecoveryFault::Unenrolled
        )))
    ));
    let store = IdentityStore::open(&database, Intent::Serve, FixedClock::new(2))
        .await
        .unwrap();
    assert_eq!(store.epoch(), epoch);
}

#[tokio::test]
async fn rotation_receipt_replays_after_reopen_and_refuses_another_binding() {
    let run = TestRun::new("identity-recovery-rotation-ack");
    let (store, database, fence) = create(&run).await;
    let epoch = store.epoch();
    // The storage test covers a real lost fence acknowledgement. This test independently
    // covers the Identity receipt binding/replay after reopening its real current file.
    let store = store
        .rotate_restore_epoch(operation(7), epoch)
        .await
        .unwrap();
    let rotated = store.epoch();
    drop(store);
    let store = reopen(&database, &fence).await;
    let committed = fs::read(&fence).unwrap();
    let store = store
        .rotate_restore_epoch(operation(7), epoch)
        .await
        .unwrap();
    assert_eq!(store.epoch(), rotated);
    assert_eq!(fs::read(&fence).unwrap(), committed);
    assert!(matches!(
        store.rotate_restore_epoch(operation(7), rotated).await,
        Err(IdentityStoreError::OperationReused)
    ));
}
