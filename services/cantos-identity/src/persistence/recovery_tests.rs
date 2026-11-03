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

#[tokio::test]
async fn epoch_rotation_preserves_mailbox_proof_history_and_refuses_old_proof_authority() {
    use crate::domain::mailbox_proof::{ProofBinding, ProofIntent, ProofTime};
    let run = TestRun::new("identity-recovery-proof-history");
    let (store, database, fence) = create(&run).await;
    let epoch = store.epoch();
    let binding = ProofBinding::new(operation(31), epoch, ProofIntent::TheatreSignup);
    let material = ProofMaterial::generate().unwrap();
    let command = ProofCommandId::from_bytes([32; 16]).unwrap();
    let receipt = store
        .issue_mailbox_proof(command, binding, &material, ProofTime::new(300).unwrap())
        .await
        .unwrap();
    let store = store
        .rotate_restore_epoch(operation(33), epoch)
        .await
        .unwrap();
    assert_ne!(store.epoch(), epoch);
    assert_eq!(store.mailbox_receipt(command).await.unwrap(), Some(receipt));
    assert_eq!(
        store
            .mailbox_proof(binding.operation())
            .await
            .unwrap()
            .unwrap()
            .binding(),
        binding
    );
    assert_eq!(
        store
            .issue_mailbox_proof(command, binding, &material, ProofTime::new(300).unwrap())
            .await,
        Err(ProofStoreError::EpochChanged)
    );
    let fresh = ProofMaterial::generate().unwrap();
    let fresh_binding = ProofBinding::new(operation(34), store.epoch(), ProofIntent::TheatreSignup);
    store
        .issue_mailbox_proof(
            ProofCommandId::from_bytes([35; 16]).unwrap(),
            fresh_binding,
            &fresh,
            ProofTime::new(300).unwrap(),
        )
        .await
        .unwrap();
    // The ledger preserves old history, but cannot authorize a new workflow in that epoch.
    let tx = store.database.begin_write().await.unwrap();
    tx.execute("INSERT INTO mailbox_proof_issues(proof_id,workflow_id,secret_digest,issued_at,expires_at) VALUES(?1,?2,?3,0,300)",
        params_from_iter([Value::Blob(vec![36;16]), Value::Blob(vec![37;16]), Value::Blob(vec![38;32])])).await.unwrap();
    assert_eq!(tx.execute("INSERT INTO mailbox_proof_workflows(workflow_id,instance_epoch,intent,current_proof_id,state) VALUES(?1,?2,1,?3,1)",
        params_from_iter([Value::Blob(vec![37;16]), Value::Blob(epoch.to_bytes().to_vec()), Value::Blob(vec![36;16])])).await,
        Err(StoreFault::Guard(cantos_store_turso::error::GuardCode::new("identity.proof_epoch").unwrap())));
    tx.rollback().await.unwrap();
    for sql in [
        "UPDATE identity_epochs SET instance_epoch=instance_epoch",
        "DELETE FROM identity_epochs",
    ] {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(
            tx.execute(sql, ()).await,
            Err(StoreFault::Guard(
                cantos_store_turso::error::GuardCode::new("identity.immutable_instance").unwrap()
            ))
        );
        tx.rollback().await.unwrap();
    }
    store.check_integrity().await.unwrap();
    let rotated = store.epoch();
    drop(store);
    let store = reopen(&database, &fence).await;
    assert_eq!(store.epoch(), rotated);
    assert_eq!(
        store
            .mailbox_proof(binding.operation())
            .await
            .unwrap()
            .unwrap()
            .binding(),
        binding
    );
}

const LEGACY_V6: [cantos_store_turso::registry::Migration; 6] = [
    MIGRATIONS[0],
    MIGRATIONS[1],
    MIGRATIONS[2],
    MIGRATIONS[3],
    MIGRATIONS[4],
    MIGRATIONS[5],
];
const RETAINED_TABLES: [&str; 15] = [
    "identity_instance",
    "identity_subjects",
    "security_events",
    "operation_receipts",
    "mailbox_proof_issues",
    "mailbox_proof_workflows",
    "mailbox_proof_receipts",
    "signup_workflows",
    "identity_emails",
    "password_credentials",
    "signup_completions",
    "firebase_signup_enrollments",
    "firebase_signup_mailboxes",
    "firebase_verification_events",
    "identity_restore_epochs",
];

async fn retained_rows(store: &IdentityStore) -> Vec<Vec<Vec<Value>>> {
    let reader = store.database.reader().await.unwrap();
    let mut tables = Vec::new();
    for table in RETAINED_TABLES {
        let mut rows = reader
            .query(&format!("SELECT * FROM {table} ORDER BY 1"), ())
            .await
            .unwrap();
        let width = rows.column_count();
        let mut values = Vec::new();
        while let Some(row) = rows.next().await.unwrap() {
            values.push(
                (0..width)
                    .map(|column| row.get_value(column).unwrap())
                    .collect(),
            );
        }
        tables.push(values);
    }
    tables
}

async fn legacy_signup_graph(path: &Path) -> IdentityStore {
    use crate::domain::{email::EmailAddress, mailbox_proof::ProofTime};
    let registry = Registry::new(IDENTITY_APPLICATION_ID, 1, &LEGACY_V6).unwrap();
    let database = OwnedDatabase::open(OpenRequest::new(
        path,
        registry,
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .unwrap();
    let epoch = InstanceEpoch::from_bytes([91; 16]).unwrap();
    let tx = database.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO identity_instance(singleton,instance_epoch) VALUES(1,?1)",
        [epoch.to_bytes().to_vec()],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let store = IdentityStore { database, epoch };
    let material = ProofMaterial::generate().unwrap();
    store
        .begin_signup(
            ProofCommandId::from_bytes([41; 16]).unwrap(),
            operation(42),
            &EmailAddress::parse("retained@example.test").unwrap(),
            &material,
            ProofTime::new(300).unwrap(),
        )
        .await
        .unwrap();
    // Synthetic shape-only credential; this is never a real account/password/hash.
    let phc = PhcString::new("$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g".into()).unwrap();
    store
        .complete_signup(&SignupCompletionRequest {
            command: ProofCommandId::from_bytes([43; 16]).unwrap(),
            operation: operation(42),
            proof: material.id(),
            secret: material.secret(),
            phc: &phc,
            subject: subject(),
        })
        .await
        .unwrap();
    store
        .disable_subject(
            operation(44),
            subject(),
            SecurityVersion::new(1).unwrap(),
            epoch,
        )
        .await
        .unwrap();
    let material = ProofMaterial::generate().unwrap();
    let start = store
        .start_firebase(
            "synthetic-project",
            ProofCommandId::from_bytes([45; 16]).unwrap(),
            operation(46),
            &EmailAddress::parse("pending@example.test").unwrap(),
            &material,
            &[7; 60],
            300,
            1,
        )
        .await
        .unwrap();
    assert!(matches!(start, firebase::Start::Create(_)));
    store
}

#[tokio::test]
async fn v6_populated_signup_and_provider_graph_survives_v7_migration_replay_and_rotation() {
    let run = TestRun::new("identity-recovery-v6-graph");
    let root = run.dir().canonicalize().unwrap();
    private(&root.join("database"));
    let path = root.join("database/identity.db");
    let store = legacy_signup_graph(&path).await;
    let before = retained_rows(&store).await;
    assert!(before[..14].iter().all(|table| !table.is_empty()));
    let epoch = store.epoch();
    drop(store);
    let store = IdentityStore::open(&path, Intent::Migrate, FixedClock::new(2))
        .await
        .unwrap();
    assert!(
        retained_rows(&store).await == before,
        "migration changed retained logical records"
    );
    store.check_integrity().await.unwrap();
    drop(store);
    let store = IdentityStore::open(&path, Intent::Migrate, FixedClock::new(3))
        .await
        .unwrap();
    assert!(
        retained_rows(&store).await == before,
        "repeated migration changed retained logical records"
    );
    private(&root.join("authority"));
    let fence = root.join("authority/outside.fence");
    let store = store.enroll_recovery_fence(&fence).await.unwrap();
    let store = store
        .rotate_restore_epoch(operation(47), epoch)
        .await
        .unwrap();
    assert_eq!(
        store.subject(subject()).await.unwrap().unwrap().state,
        AccountState::Disabled
    );
    let rotated = store.epoch();
    assert_ne!(rotated, epoch);
    store.check_integrity().await.unwrap();
    drop(store);
    let store = reopen(&path, &fence).await;
    assert_eq!(store.epoch(), rotated);
    let store = store
        .rotate_restore_epoch(operation(47), epoch)
        .await
        .unwrap();
    assert_eq!(store.epoch(), rotated);
}

#[tokio::test]
async fn failed_v7_graph_rebuild_rolls_back_all_copies_and_keeps_v6_servable() {
    for conflict in [
        "mailbox_proof_workflows_v7",
        "firebase_verification_events_v7",
    ] {
        let run = TestRun::new("identity-recovery-v7-failed-migration");
        let path = run.path("identity.db");
        let store = legacy_signup_graph(&path).await;
        let epoch = store.epoch();
        let tx = store.database.begin_write().await.unwrap();
        tx.execute(
            &format!("CREATE TABLE {conflict}(foreign_marker INTEGER)"),
            (),
        )
        .await
        .unwrap();
        tx.commit().await.unwrap();
        let before = retained_rows(&store).await;
        drop(store);
        let refused = IdentityStore::open(&path, Intent::Migrate, FixedClock::new(2)).await;
        assert!(matches!(
            refused,
            Err(IdentityStoreError::Open(OpenError::MigrationFailed {
                version: 7,
                fault: StoreFault::Engine
            }))
        ));
        let registry = Registry::new(IDENTITY_APPLICATION_ID, 1, &LEGACY_V6).unwrap();
        let database = OwnedDatabase::open(OpenRequest::new(
            &path,
            registry,
            Intent::Serve,
            FixedClock::new(3),
        ))
        .await
        .unwrap();
        let store = IdentityStore { database, epoch };
        assert!(
            retained_rows(&store).await == before,
            "failed migration changed old records"
        );
        store.check_integrity().await.unwrap();
        let reader = store.database.reader().await.unwrap();
        let mut rows = reader
            .query(
                "SELECT name FROM sqlite_schema WHERE name='identity_epochs'",
                (),
            )
            .await
            .unwrap();
        assert!(
            rows.next().await.unwrap().is_none(),
            "failed migration left new schema behind"
        );
    }
}

#[tokio::test]
async fn already_fenced_v6_upgrade_is_refused_without_mutating_current_authority() {
    let run = TestRun::new("identity-recovery-fenced-v6");
    let root = run.dir().canonicalize().unwrap();
    private(&root.join("database"));
    private(&root.join("authority"));
    let path = root.join("database/identity.db");
    let fence = root.join("authority/current");
    let store = legacy_signup_graph(&path)
        .await
        .enroll_recovery_fence(&fence)
        .await
        .unwrap();
    let before = retained_rows(&store).await;
    let epoch = store.epoch();
    drop(store);
    let floor = fs::read(&fence).unwrap();
    // The owner explicitly accepts an enrolled file only at Plan::Ready. Upgrading an
    // already-enrolled registry needs a separate qualified executor; never shed its floor.
    let refused = IdentityStore::open_with_recovery_fence(
        &path,
        Intent::Migrate,
        FixedClock::new(2),
        Some(fence.clone()),
    )
    .await;
    assert!(matches!(
        refused,
        Err(IdentityStoreError::Open(OpenError::Store(
            StoreFault::Recovery(RecoveryFault::Mismatch)
        )))
    ));
    assert_eq!(fs::read(&fence).unwrap(), floor);
    let registry = Registry::new(IDENTITY_APPLICATION_ID, 1, &LEGACY_V6).unwrap();
    let database = OwnedDatabase::open(
        OpenRequest::new(&path, registry, Intent::Serve, FixedClock::new(3))
            .with_recovery_fence(&fence),
    )
    .await
    .unwrap();
    let store = IdentityStore { database, epoch };
    assert!(
        retained_rows(&store).await == before,
        "refused upgrade changed retained records"
    );
    assert_eq!(store.epoch(), epoch);
    store.check_integrity().await.unwrap();
}
