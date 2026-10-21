//! Real pinned-engine proof storage tests. These private, synthetic files prove
//! storage behavior, never mailbox delivery, setup, authentication or RPC acceptance.

use std::{sync::Arc, time::Duration};

use cantos_store_turso::error::{ConstraintKind, GuardCode, OpenError, StoreFault};
use cantos_store_turso::registry::{DatabaseKind, Intent, Migration, Refusal, Registry};
use cantos_store_turso::testing::{FixedClock, TestRun};
use cantos_store_turso::{OpenRequest, OwnedDatabase, Value};

use super::{
    registry, IdentityStore, IdentityStoreError, ProofCommandId, ProofMaterial, ProofReceiptAction,
    ProofSecret, ProofStoreError,
};
use crate::domain::admission::{AccountState, Purpose};
use crate::domain::ids::{InstanceEpoch, OperationId, SubjectId};
use crate::domain::mailbox_proof::{
    MailboxProof, ProofAction, ProofBinding, ProofDenied, ProofIntent, ProofState, ProofTime,
};
use crate::domain::security_version::SecurityVersion;

const NOW: u64 = 1_700_000_000;

fn command(value: u128) -> ProofCommandId {
    ProofCommandId::from_bytes(value.to_be_bytes()).unwrap()
}

fn operation(value: u128) -> OperationId {
    OperationId::from_bytes(value.to_be_bytes()).unwrap()
}

fn subject(value: u128) -> SubjectId {
    SubjectId::from_bytes(value.to_be_bytes()).unwrap()
}

fn version(value: u64) -> SecurityVersion {
    SecurityVersion::new(value).unwrap()
}

fn time(seconds: u64) -> ProofTime {
    ProofTime::new(seconds).unwrap()
}

fn set_time(clock: &FixedClock, seconds: u64) {
    clock.set(i64::try_from(seconds * 1_000_000).unwrap());
}

fn binding(store: &IdentityStore, workflow: u128) -> ProofBinding {
    ProofBinding::new(
        operation(workflow),
        store.epoch(),
        ProofIntent::TheatreSignup,
    )
}

fn recovery(store: &IdentityStore, workflow: u128, who: SubjectId, v: u64) -> ProofBinding {
    ProofBinding::new(
        operation(workflow),
        store.epoch(),
        ProofIntent::CredentialRecovery {
            subject: who,
            version: version(v),
            purpose: Purpose::Theatre,
        },
    )
}

fn guard(code: &str) -> StoreFault {
    StoreFault::Guard(GuardCode::new(code).unwrap())
}

fn wrong_secret(material: &ProofMaterial) -> ProofSecret {
    let mut bytes = *material.secret().expose();
    bytes[0] ^= 1;
    ProofSecret::from_bytes(bytes)
}

async fn create(run: &TestRun, clock: Arc<FixedClock>) -> IdentityStore {
    IdentityStore::open(run.path("proofs.db"), Intent::Migrate, clock)
        .await
        .unwrap()
}

async fn reopen(run: &TestRun, clock: Arc<FixedClock>) -> IdentityStore {
    IdentityStore::open(run.path("proofs.db"), Intent::Serve, clock)
        .await
        .unwrap()
}

async fn count(store: &IdentityStore, sql: &str) -> i64 {
    let reader = store.database.reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let Value::Integer(value) = row.get_value(0).unwrap() else {
        panic!("expected an INTEGER count")
    };
    assert!(rows.next().await.unwrap().is_none());
    value
}

async fn counts(store: &IdentityStore) -> (i64, i64, i64) {
    (
        count(store, "SELECT COUNT(*) FROM mailbox_proof_issues").await,
        count(store, "SELECT COUNT(*) FROM mailbox_proof_workflows").await,
        count(store, "SELECT COUNT(*) FROM mailbox_proof_receipts").await,
    )
}

async fn current(store: &IdentityStore, binding: ProofBinding) -> MailboxProof {
    store
        .mailbox_proof(binding.operation())
        .await
        .unwrap()
        .unwrap()
}

async fn assert_changed_epoch_refused_before_writer(
    store: &IdentityStore,
    changed: ProofBinding,
    first: &ProofMaterial,
    second: &ProofMaterial,
) {
    let held = store.database.begin_write().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        assert_eq!(
            store
                .issue_mailbox_proof(command(1), changed, first, time(NOW + 60))
                .await,
            Err(ProofStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .issue_mailbox_proof(command(2), changed, second, time(NOW + 60))
                .await,
            Err(ProofStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .confirm_mailbox_proof(
                    command(3),
                    changed,
                    first.id(),
                    first.secret(),
                    ProofAction::ExplicitConfirmation
                )
                .await,
            Err(ProofStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .cancel_mailbox_proof(command(4), changed, first.id())
                .await,
            Err(ProofStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .replace_mailbox_proof(command(5), changed, first.id(), second, time(NOW + 120), 1)
                .await,
            Err(ProofStoreError::EpochChanged)
        );
    })
    .await
    .expect("changed epoch must refuse without acquiring the held writer");
    held.rollback().await.unwrap();
    for id in 2..=5 {
        assert_eq!(store.mailbox_receipt(command(id)).await.unwrap(), None);
    }
}

#[tokio::test]
async fn issue_persists_digest_and_reopens_same_epoch_and_bound_receipt() {
    let run = TestRun::new("identity-proof-reopen");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let epoch = store.epoch();
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 300))
        .await
        .unwrap();
    assert_eq!(issued.command, command(1));
    assert_eq!(issued.action, ProofReceiptAction::Issued);
    assert_eq!(issued.proof.id(), material.id());
    assert_eq!(issued.proof.binding(), bound);
    assert_eq!(issued.proof.issued_at(), time(NOW));
    assert_eq!(issued.proof.expires_at(), time(NOW + 300));
    assert_eq!(issued.proof.state(), ProofState::Pending);
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(
        store.mailbox_receipt(command(1)).await.unwrap(),
        Some(issued)
    );
    assert_eq!(counts(&store).await, (1, 1, 1));

    {
        let reader = store.database.reader().await.unwrap();
        let mut rows = reader
            .query("SELECT secret_digest FROM mailbox_proof_issues", ())
            .await
            .unwrap();
        let row = rows.next().await.unwrap().unwrap();
        let Value::Blob(digest) = row.get_value(0).unwrap() else {
            panic!("expected a digest BLOB")
        };
        assert_eq!(digest.len(), 32);
        // A boolean assertion avoids printing the synthetic secret on failure.
        assert!(digest.as_slice() != material.secret().expose().as_slice());
        assert!(rows.next().await.unwrap().is_none());
    }
    drop(store);
    set_time(&clock, NOW + 1);
    let store = reopen(&run, clock).await;
    assert_eq!(store.epoch(), epoch);
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(
        store.mailbox_receipt(command(1)).await.unwrap(),
        Some(issued)
    );
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), bound, &material, time(NOW + 300))
            .await
            .unwrap(),
        issued
    );
    assert_eq!(counts(&store).await, (1, 1, 1));
}

#[tokio::test]
async fn additive_migration_preserves_v1_subject_epoch_events_and_receipts() {
    const V1: [Migration; 1] = [super::MIGRATIONS[0]];
    let run = TestRun::new("identity-proof-v1-upgrade");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let epoch = InstanceEpoch::from_bytes(1_u128.to_be_bytes()).unwrap();
    let db = OwnedDatabase::open(OpenRequest::new(
        run.path("proofs.db"),
        Registry::new(DatabaseKind::Identity, 1, &V1).unwrap(),
        Intent::Migrate,
        clock.clone(),
    ))
    .await
    .unwrap();
    let tx = db.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO identity_instance(singleton,instance_epoch) VALUES(1,?1)",
        [Value::Blob(epoch.to_bytes().to_vec())],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(?1,1,1)",
        [Value::Blob(subject(1).to_bytes().to_vec())],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO security_events(seq,subject_id,security_version,state,kind,recorded_at) VALUES(1,?1,1,1,1,?2)",
        [Value::Blob(subject(1).to_bytes().to_vec()), Value::Integer((NOW * 1_000_000) as i64)],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO operation_receipts(operation_id,subject_id,kind,expected_version,committed_version,event_seq) VALUES(?1,?2,1,0,1,1)",
        [Value::Blob(operation(11).to_bytes().to_vec()), Value::Blob(subject(1).to_bytes().to_vec())],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(db);
    assert!(matches!(
        IdentityStore::open(run.path("proofs.db"), Intent::Serve, clock.clone()).await,
        Err(IdentityStoreError::Open(OpenError::Refused(
            Refusal::PendingMigrations {
                applied: 1,
                latest: 3
            }
        )))
    ));
    let store = create(&run, clock).await;
    assert_eq!(store.epoch(), epoch);
    let stored = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(stored.version, version(1));
    assert_eq!(stored.state, AccountState::Active);
    assert_eq!(store.events(0, 10).await.unwrap().len(), 1);
    assert_eq!(
        store
            .receipt(operation(11))
            .await
            .unwrap()
            .unwrap()
            .event_seq,
        1
    );
    assert_eq!(counts(&store).await, (0, 0, 0));
    let material = ProofMaterial::generate().unwrap();
    let receipt = store
        .issue_mailbox_proof(command(11), binding(&store, 11), &material, time(NOW + 60))
        .await
        .unwrap();
    assert_eq!(receipt.proof.state(), ProofState::Pending);
    assert_eq!(counts(&store).await, (1, 1, 1));
    // Equal raw bytes are legal across the security-operation, proof-command
    // and immutable workflow namespaces; each retains its own bound receipt.
    assert_eq!(command(11).to_bytes(), operation(11).to_bytes());
    assert_eq!(
        store
            .receipt(operation(11))
            .await
            .unwrap()
            .unwrap()
            .event_seq,
        1
    );
    assert_eq!(
        store.mailbox_receipt(command(11)).await.unwrap(),
        Some(receipt)
    );
    assert_eq!(store.events(0, 10).await.unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_issue_window_and_changed_accepted_payload_leave_no_extra_rows() {
    let run = TestRun::new("identity-proof-issue-negative");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), bound, &material, time(NOW))
            .await,
        Err(ProofStoreError::Denied(ProofDenied::InvalidWindow))
    );
    assert_eq!(counts(&store).await, (0, 0, 0));
    let issued = store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), bound, &material, time(NOW + 61))
            .await,
        Err(ProofStoreError::OperationReused)
    );
    let other = ProofMaterial::generate().unwrap();
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), bound, &other, time(NOW + 60))
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(
        store
            .issue_mailbox_proof(command(2), bound, &other, time(NOW + 60))
            .await,
        Err(ProofStoreError::Exists)
    );
    assert_eq!(store.mailbox_receipt(command(2)).await.unwrap(), None);
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(counts(&store).await, (1, 1, 1));
}

#[tokio::test]
async fn wrong_secret_inspect_and_changed_binding_cannot_consume_or_commit_receipt() {
    let run = TestRun::new("identity-proof-possession-action");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                bound,
                material.id(),
                &wrong_secret(&material),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::InvalidSecret)
    );
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(3),
                bound,
                material.id(),
                material.secret(),
                ProofAction::Inspect
            )
            .await,
        Err(ProofStoreError::Denied(
            ProofDenied::NotExplicitConfirmation
        ))
    );
    store
        .create_subject(operation(29), subject(9), store.epoch())
        .await
        .unwrap();
    let changed = recovery(&store, 1, subject(9), 1);
    assert_eq!(
        store
            .cancel_mailbox_proof(command(4), changed, material.id())
            .await,
        Err(ProofStoreError::Denied(ProofDenied::BindingChanged))
    );
    for id in [2, 3, 4] {
        assert_eq!(store.mailbox_receipt(command(id)).await.unwrap(), None);
    }
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(5),
                binding(&store, 99),
                material.id(),
                material.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Missing)
    );
    assert_eq!(
        store
            .cancel_mailbox_proof(command(5), binding(&store, 99), material.id())
            .await,
        Err(ProofStoreError::Missing)
    );
    assert_eq!(store.mailbox_receipt(command(5)).await.unwrap(), None);
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(counts(&store).await, (1, 1, 1));
}

#[tokio::test]
async fn clock_before_issue_and_exact_expiry_refuse_but_last_valid_second_consumes() {
    let run = TestRun::new("identity-proof-time-fences");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    set_time(&clock, NOW - 1);
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                bound,
                material.id(),
                material.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Denied(ProofDenied::BeforeIssue))
    );
    set_time(&clock, NOW + 60);
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(3),
                bound,
                material.id(),
                material.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Denied(ProofDenied::Expired))
    );
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(store.mailbox_receipt(command(2)).await.unwrap(), None);
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);

    // A different workflow proves the last admissible second without treating
    // clock rollback as a production expiry-revival policy.
    let second = binding(&store, 2);
    let second_material = ProofMaterial::generate().unwrap();
    store
        .issue_mailbox_proof(command(4), second, &second_material, time(NOW + 120))
        .await
        .unwrap();
    set_time(&clock, NOW + 119);
    let consumed = store
        .confirm_mailbox_proof(
            command(5),
            second,
            second_material.id(),
            second_material.secret(),
            ProofAction::ExplicitConfirmation,
        )
        .await
        .unwrap();
    assert_eq!(consumed.proof.state(), ProofState::Consumed);
    assert_eq!(current(&store, second).await, consumed.proof);
    assert_eq!(counts(&store).await, (2, 2, 3));
}

#[tokio::test]
async fn lost_confirmation_response_replays_historical_receipt_without_new_effect() {
    let run = TestRun::new("identity-proof-lost-response");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    // Commit to the real engine and discard the returned value, modelling a
    // response lost after commit rather than fabricating CommitFault::Unknown.
    store
        .confirm_mailbox_proof(
            command(2),
            bound,
            material.id(),
            material.secret(),
            ProofAction::ExplicitConfirmation,
        )
        .await
        .unwrap();
    drop(store);
    set_time(&clock, NOW + 100);
    let store = reopen(&run, clock).await;
    let reconciled = store
        .confirm_mailbox_proof(
            command(2),
            bound,
            material.id(),
            material.secret(),
            ProofAction::ExplicitConfirmation,
        )
        .await
        .unwrap();
    assert_eq!(reconciled.action, ProofReceiptAction::Confirmed);
    assert_eq!(reconciled.proof.state(), ProofState::Consumed);
    assert_eq!(
        store.mailbox_receipt(command(2)).await.unwrap(),
        Some(reconciled)
    );
    assert_eq!(current(&store, bound).await, reconciled.proof);
    let historical = store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    assert_eq!(historical, issued);
    assert_eq!(historical.proof.state(), ProofState::Pending);
    assert_eq!(current(&store, bound).await.state(), ProofState::Consumed);
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                bound,
                material.id(),
                &wrong_secret(&material),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                bound,
                material.id(),
                material.secret(),
                ProofAction::Inspect
            )
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(
        store
            .cancel_mailbox_proof(command(2), bound, material.id())
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                recovery(&store, 1, subject(9), 1),
                material.id(),
                material.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(counts(&store).await, (1, 1, 2));
}

#[tokio::test]
async fn replacement_has_exact_cooldown_and_retires_old_locator_and_digest() {
    let run = TestRun::new("identity-proof-replacement");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let first = ProofMaterial::generate().unwrap();
    let second = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &first, time(NOW + 60))
        .await
        .unwrap();
    assert_eq!(
        store
            .replace_mailbox_proof(command(2), bound, first.id(), &second, time(NOW + 120), 0)
            .await,
        Err(ProofStoreError::Denied(ProofDenied::InvalidCooldown))
    );
    set_time(&clock, NOW + 9);
    assert_eq!(
        store
            .replace_mailbox_proof(command(2), bound, first.id(), &second, time(NOW + 120), 10)
            .await,
        Err(ProofStoreError::Denied(ProofDenied::Cooldown))
    );
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(counts(&store).await, (1, 1, 1));
    set_time(&clock, NOW + 10);
    let replaced = store
        .replace_mailbox_proof(command(2), bound, first.id(), &second, time(NOW + 120), 10)
        .await
        .unwrap();
    assert_eq!(replaced.action, ProofReceiptAction::Replaced);
    assert_eq!(replaced.proof.id(), second.id());
    assert_eq!(replaced.proof.issued_at(), time(NOW + 10));
    assert_eq!(replaced.proof.state(), ProofState::Pending);
    assert_eq!(current(&store, bound).await, replaced.proof);
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(3),
                bound,
                first.id(),
                first.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Denied(ProofDenied::ProofChanged))
    );
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(3),
                bound,
                second.id(),
                first.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::InvalidSecret)
    );
    assert_eq!(
        store
            .replace_mailbox_proof(command(2), bound, first.id(), &second, time(NOW + 121), 10)
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(
        store
            .replace_mailbox_proof(command(2), bound, first.id(), &second, time(NOW + 120), 11)
            .await,
        Err(ProofStoreError::OperationReused)
    );
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
    assert_eq!(counts(&store).await, (2, 1, 2));
    let confirmed = store
        .confirm_mailbox_proof(
            command(3),
            bound,
            second.id(),
            second.secret(),
            ProofAction::ExplicitConfirmation,
        )
        .await
        .unwrap();
    assert_eq!(confirmed.proof.state(), ProofState::Consumed);
    assert_eq!(current(&store, bound).await, confirmed.proof);
    assert_eq!(counts(&store).await, (2, 1, 3));
}

#[tokio::test]
async fn expired_pending_can_replace_and_cannot_reuse_any_issued_material() {
    let run = TestRun::new("identity-proof-historical-uniqueness");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let first = ProofMaterial::generate().unwrap();
    let second = ProofMaterial::generate().unwrap();
    store
        .issue_mailbox_proof(command(1), bound, &first, time(NOW + 1))
        .await
        .unwrap();
    set_time(&clock, NOW + 10);
    let replaced = store
        .replace_mailbox_proof(command(2), bound, first.id(), &second, time(NOW + 100), 10)
        .await
        .unwrap();
    set_time(&clock, NOW + 20);
    // This is not an immediate same-ID denial: the formerly issued locator and
    // digest remain reserved even after its replacement.
    let result = store
        .replace_mailbox_proof(command(3), bound, second.id(), &first, time(NOW + 200), 10)
        .await;
    assert!(matches!(
        result,
        Err(ProofStoreError::Storage(StoreFault::Constraint {
            kind: ConstraintKind::Unique,
            ..
        }))
    ));
    assert_eq!(current(&store, bound).await, replaced.proof);
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
    assert_eq!(counts(&store).await, (2, 1, 2));

    // Isolate digest uniqueness from the proof_id primary key. The positive control
    // uses the identical fresh locator/workflow/window and changes only the digest.
    let fresh = ProofMaterial::generate().unwrap();
    let reader = store.database.reader().await.unwrap();
    let mut rows = reader
        .query(
            "SELECT secret_digest FROM mailbox_proof_issues WHERE proof_id=?1",
            [first.id().to_bytes().to_vec()],
        )
        .await
        .unwrap();
    let Value::Blob(old_digest) = rows.next().await.unwrap().unwrap().get_value(0).unwrap() else {
        panic!("expected a stored digest")
    };
    assert_eq!(old_digest.len(), 32);
    assert!(rows.next().await.unwrap().is_none());
    drop(rows);
    let mut rows = reader
        .query(
            "SELECT count(*) FROM mailbox_proof_issues WHERE proof_id=?1",
            [fresh.id().to_bytes().to_vec()],
        )
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get_value(0).unwrap(),
        Value::Integer(0)
    );
    drop(rows);
    drop(reader);
    let insert = "INSERT INTO mailbox_proof_issues(proof_id,workflow_id,secret_digest,issued_at,expires_at) VALUES(?1,?2,?3,?4,?5)";
    let values = |digest: Vec<u8>| {
        cantos_store_turso::params_from_iter([
            Value::Blob(fresh.id().to_bytes().to_vec()),
            Value::Blob(bound.operation().to_bytes().to_vec()),
            Value::Blob(digest),
            Value::Integer((NOW + 20) as i64),
            Value::Integer((NOW + 200) as i64),
        ])
    };
    let tx = store.database.begin_write().await.unwrap();
    assert_eq!(
        tx.execute(insert, values(old_digest.clone())).await,
        Err(StoreFault::Constraint {
            kind: ConstraintKind::Unique,
            target: Some("mailbox_proof_issues.secret_digest".to_owned()),
        })
    );
    tx.rollback().await.unwrap();
    assert_eq!(counts(&store).await, (2, 1, 2));

    let fresh_digest = vec![0xA5; 32];
    assert_ne!(fresh_digest, old_digest);
    let tx = store.database.begin_write().await.unwrap();
    assert_eq!(tx.execute(insert, values(fresh_digest)).await.unwrap(), 1);
    tx.commit().await.unwrap();
    assert_eq!(counts(&store).await, (3, 1, 2));
    assert_eq!(current(&store, bound).await, replaced.proof);
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
}

#[tokio::test]
async fn cancellation_at_expiry_is_durable_and_terminal_for_new_commands() {
    let run = TestRun::new("identity-proof-cancel-terminal");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    let replacement = ProofMaterial::generate().unwrap();
    store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 1))
        .await
        .unwrap();
    set_time(&clock, NOW + 1);
    let cancelled = store
        .cancel_mailbox_proof(command(2), bound, material.id())
        .await
        .unwrap();
    assert_eq!(cancelled.action, ProofReceiptAction::Cancelled);
    assert_eq!(cancelled.proof.state(), ProofState::Cancelled);
    assert_eq!(
        store
            .cancel_mailbox_proof(command(2), bound, material.id())
            .await
            .unwrap(),
        cancelled
    );
    assert_eq!(
        store
            .cancel_mailbox_proof(command(3), bound, material.id())
            .await,
        Err(ProofStoreError::Denied(ProofDenied::Cancelled))
    );
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(3),
                bound,
                material.id(),
                material.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Denied(ProofDenied::Cancelled))
    );
    assert_eq!(
        store
            .replace_mailbox_proof(
                command(3),
                bound,
                material.id(),
                &replacement,
                time(NOW + 100),
                1
            )
            .await,
        Err(ProofStoreError::Denied(ProofDenied::Cancelled))
    );
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
    drop(store);
    let store = reopen(&run, clock).await;
    assert_eq!(current(&store, bound).await, cancelled.proof);
    assert_eq!(
        store.mailbox_receipt(command(2)).await.unwrap(),
        Some(cancelled)
    );
    assert_eq!(counts(&store).await, (1, 1, 2));
}

#[tokio::test]
async fn confirmation_and_cancellation_race_has_one_terminal_winner() {
    let run = TestRun::new("identity-proof-confirm-cancel-race");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    let (confirmed, cancelled) = tokio::join!(
        store.confirm_mailbox_proof(
            command(2),
            bound,
            material.id(),
            material.secret(),
            ProofAction::ExplicitConfirmation
        ),
        store.cancel_mailbox_proof(command(3), bound, material.id())
    );
    match (confirmed, cancelled) {
        (Ok(winner), Err(ProofStoreError::Denied(ProofDenied::Consumed))) => {
            assert_eq!(winner.action, ProofReceiptAction::Confirmed);
            assert_eq!(current(&store, bound).await, winner.proof);
            assert_eq!(
                store.mailbox_receipt(command(2)).await.unwrap(),
                Some(winner)
            );
            assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
        }
        (Err(ProofStoreError::Denied(ProofDenied::Cancelled)), Ok(winner)) => {
            assert_eq!(winner.action, ProofReceiptAction::Cancelled);
            assert_eq!(current(&store, bound).await, winner.proof);
            assert_eq!(store.mailbox_receipt(command(2)).await.unwrap(), None);
            assert_eq!(
                store.mailbox_receipt(command(3)).await.unwrap(),
                Some(winner)
            );
        }
        _ => panic!("expected exactly one committed terminal outcome"),
    }
    assert_eq!(counts(&store).await, (1, 1, 2));
}

#[tokio::test]
async fn identical_concurrent_commands_share_one_receipt_and_effect() {
    let run = TestRun::new("identity-proof-identical-race");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    let (first, second) = tokio::join!(
        store.issue_mailbox_proof(command(1), bound, &material, time(NOW + 60)),
        store.issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
    );
    assert_eq!(first.unwrap(), second.unwrap());
    let (first, second) = tokio::join!(
        store.confirm_mailbox_proof(
            command(2),
            bound,
            material.id(),
            material.secret(),
            ProofAction::ExplicitConfirmation
        ),
        store.confirm_mailbox_proof(
            command(2),
            bound,
            material.id(),
            material.secret(),
            ProofAction::ExplicitConfirmation
        )
    );
    let accepted = first.unwrap();
    assert_eq!(accepted, second.unwrap());
    assert_eq!(current(&store, bound).await, accepted.proof);
    assert_eq!(counts(&store).await, (1, 1, 2));
}

#[tokio::test]
async fn receipt_failure_rolls_back_confirmation_and_replacement_issue_reservation() {
    let run = TestRun::new("identity-proof-statement-rollback");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let first = ProofMaterial::generate().unwrap();
    let second = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &first, time(NOW + 60))
        .await
        .unwrap();
    let tx = store.database.begin_write().await.unwrap();
    tx.execute("CREATE TRIGGER test_fail_proof_receipt BEFORE INSERT ON mailbox_proof_receipts BEGIN SELECT RAISE(ABORT, 'cantos:test.fail_proof_receipt'); END", ()).await.unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        store
            .issue_mailbox_proof(command(4), binding(&store, 2), &second, time(NOW + 120))
            .await,
        Err(ProofStoreError::Storage(guard("test.fail_proof_receipt")))
    );
    assert_eq!(store.mailbox_proof(operation(2)).await.unwrap(), None);
    assert_eq!(store.mailbox_receipt(command(4)).await.unwrap(), None);
    assert_eq!(counts(&store).await, (1, 1, 1));
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                bound,
                first.id(),
                first.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Storage(guard("test.fail_proof_receipt")))
    );
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(store.mailbox_receipt(command(2)).await.unwrap(), None);
    assert_eq!(counts(&store).await, (1, 1, 1));
    set_time(&clock, NOW + 10);
    assert_eq!(
        store
            .replace_mailbox_proof(command(3), bound, first.id(), &second, time(NOW + 120), 10)
            .await,
        Err(ProofStoreError::Storage(guard("test.fail_proof_receipt")))
    );
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
    assert_eq!(counts(&store).await, (1, 1, 1));
    let tx = store.database.begin_write().await.unwrap();
    // Remove only this test's injected trigger. Production guards stay installed.
    tx.execute("DROP TRIGGER test_fail_proof_receipt", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let retried = store
        .replace_mailbox_proof(command(3), bound, first.id(), &second, time(NOW + 120), 10)
        .await
        .unwrap();
    assert_eq!(retried.proof.id(), second.id());
    assert_eq!(current(&store, bound).await, retried.proof);
    assert_eq!(counts(&store).await, (2, 1, 2));
}

#[tokio::test]
async fn recovery_requires_authoritative_subject_and_current_active_version() {
    let run = TestRun::new("identity-proof-recovery-fences");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let material = ProofMaterial::generate().unwrap();
    let missing = recovery(&store, 1, subject(1), 1);
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), missing, &material, time(NOW + 60))
            .await,
        Err(ProofStoreError::Identity(
            IdentityStoreError::SubjectMissing
        ))
    );
    store
        .create_subject(operation(21), subject(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .issue_mailbox_proof(
                command(1),
                recovery(&store, 1, subject(1), 2),
                &material,
                time(NOW + 60)
            )
            .await,
        Err(ProofStoreError::Identity(IdentityStoreError::Conflict))
    );
    let issued = store
        .issue_mailbox_proof(command(1), missing, &material, time(NOW + 60))
        .await
        .unwrap();
    store
        .disable_subject(operation(22), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .confirm_mailbox_proof(
                command(2),
                missing,
                material.id(),
                material.secret(),
                ProofAction::ExplicitConfirmation
            )
            .await,
        Err(ProofStoreError::Identity(IdentityStoreError::Inactive))
    );
    assert_eq!(
        store
            .cancel_mailbox_proof(command(3), missing, material.id())
            .await,
        Err(ProofStoreError::Identity(IdentityStoreError::Inactive))
    );
    assert_eq!(current(&store, missing).await, issued.proof);
    assert_eq!(store.mailbox_receipt(command(2)).await.unwrap(), None);
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
    // The historical issue receipt remains reconcilable after account changes;
    // it supplies no fresh Active snapshot or recovery capability.
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), missing, &material, time(NOW + 60))
            .await
            .unwrap(),
        issued
    );
    assert_eq!(
        store.subject(subject(1)).await.unwrap().unwrap().state,
        AccountState::Disabled
    );
    assert_eq!(counts(&store).await, (1, 1, 1));
}

#[tokio::test]
async fn changed_epoch_refuses_new_and_replayed_commands_before_writer_access() {
    let run = TestRun::new("identity-proof-epoch-before-writer");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock.clone()).await;
    let bound = binding(&store, 1);
    let first = ProofMaterial::generate().unwrap();
    let second = ProofMaterial::generate().unwrap();
    let issued = store
        .issue_mailbox_proof(command(1), bound, &first, time(NOW + 60))
        .await
        .unwrap();
    let other_epoch = InstanceEpoch::from_bytes(if store.epoch().to_bytes() == [1; 16] {
        [2; 16]
    } else {
        [1; 16]
    })
    .unwrap();
    let changed = ProofBinding::new(bound.operation(), other_epoch, bound.intent());
    assert_changed_epoch_refused_before_writer(&store, changed, &first, &second).await;
    assert_eq!(current(&store, bound).await, issued.proof);
    assert_eq!(counts(&store).await, (1, 1, 1));
    drop(store);
    let store = reopen(&run, clock).await;
    assert_changed_epoch_refused_before_writer(&store, changed, &first, &second).await;
    assert_eq!(
        store
            .issue_mailbox_proof(command(1), bound, &first, time(NOW + 60))
            .await
            .unwrap(),
        issued
    );
    assert_eq!(counts(&store).await, (1, 1, 1));
}

#[tokio::test]
async fn proof_occupancy_without_instance_metadata_refuses_fresh_epoch() {
    let run = TestRun::new("identity-proof-metadata-occupancy");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let db = OwnedDatabase::open(OpenRequest::new(
        run.path("proofs.db"),
        registry().unwrap(),
        Intent::Migrate,
        clock.clone(),
    ))
    .await
    .unwrap();
    let tx = db.begin_write().await.unwrap();
    // Issue reservations can exist without a workflow. This valid schema fixture
    // proves that proof data alone prevents silently inventing a new epoch.
    tx.execute("INSERT INTO mailbox_proof_issues(proof_id,workflow_id,secret_digest,issued_at,expires_at) VALUES(?1,?2,?3,1,2)", [Value::Blob(1_u128.to_be_bytes().to_vec()), Value::Blob(operation(1).to_bytes().to_vec()), Value::Blob(vec![7; 32])]).await.unwrap();
    tx.commit().await.unwrap();
    drop(db);
    assert!(matches!(
        IdentityStore::open(run.path("proofs.db"), Intent::Migrate, clock.clone()).await,
        Err(IdentityStoreError::CorruptRecord)
    ));
    let db = OwnedDatabase::open(OpenRequest::new(
        run.path("proofs.db"),
        registry().unwrap(),
        Intent::Serve,
        clock,
    ))
    .await
    .unwrap();
    let reader = db.reader().await.unwrap();
    let mut rows = reader.query("SELECT (SELECT COUNT(*) FROM identity_instance),(SELECT COUNT(*) FROM mailbox_proof_issues)", ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert_eq!(row.get_value(0).unwrap(), Value::Integer(0));
    assert_eq!(row.get_value(1).unwrap(), Value::Integer(1));
    assert!(rows.next().await.unwrap().is_none());
}

#[tokio::test]
async fn direct_mutations_cannot_rebind_or_reopen_proofs_or_change_history() {
    let run = TestRun::new("identity-proof-immutable-guards");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    let attempts = [
        ("UPDATE mailbox_proof_issues SET expires_at=expires_at+1", "identity.immutable_proof_issue"),
        ("DELETE FROM mailbox_proof_issues", "identity.immutable_proof_issue"),
        ("DELETE FROM mailbox_proof_workflows", "identity.immutable_proof_workflow"),
        ("UPDATE mailbox_proof_workflows SET intent=2,recovery_subject=x'00000000000000000000000000000001',recovery_version=1,purpose=1,state=2", "identity.immutable_proof_workflow"),
        ("UPDATE mailbox_proof_workflows SET state=state", "identity.proof_transition"),
        ("UPDATE mailbox_proof_receipts SET request_digest=zeroblob(32)", "identity.immutable_proof_receipt"),
        ("DELETE FROM mailbox_proof_receipts", "identity.immutable_proof_receipt"),
    ];
    for (sql, expected) in attempts {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(tx.execute(sql, ()).await, Err(guard(expected)));
        tx.rollback().await.unwrap();
        assert_eq!(current(&store, bound).await.state(), ProofState::Pending);
        assert_eq!(counts(&store).await, (1, 1, 1));
    }
    let consumed = store
        .confirm_mailbox_proof(
            command(2),
            bound,
            material.id(),
            material.secret(),
            ProofAction::ExplicitConfirmation,
        )
        .await
        .unwrap();
    let tx = store.database.begin_write().await.unwrap();
    assert_eq!(
        tx.execute("UPDATE mailbox_proof_workflows SET state=1", ())
            .await,
        Err(guard("identity.proof_transition"))
    );
    tx.rollback().await.unwrap();
    assert_eq!(current(&store, bound).await, consumed.proof);
    assert_eq!(counts(&store).await, (1, 1, 2));
}

#[tokio::test]
async fn receipt_and_current_locator_require_exact_issue_workflow_association() {
    let run = TestRun::new("identity-proof-composite-fk");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let first = ProofMaterial::generate().unwrap();
    let second = ProofMaterial::generate().unwrap();
    let first_bound = binding(&store, 1);
    let second_bound = binding(&store, 2);
    store
        .issue_mailbox_proof(command(1), first_bound, &first, time(NOW + 60))
        .await
        .unwrap();
    store
        .issue_mailbox_proof(command(2), second_bound, &second, time(NOW + 60))
        .await
        .unwrap();
    let foreign_key = StoreFault::Constraint {
        kind: ConstraintKind::ForeignKey,
        target: None,
    };
    let tx = store.database.begin_write().await.unwrap();
    assert_eq!(
        tx.execute(
            "UPDATE mailbox_proof_workflows SET current_proof_id=?1 WHERE workflow_id=?2",
            [
                Value::Blob(second.id().to_bytes().to_vec()),
                Value::Blob(first_bound.operation().to_bytes().to_vec())
            ]
        )
        .await,
        Err(foreign_key.clone())
    );
    tx.rollback().await.unwrap();
    let tx = store.database.begin_write().await.unwrap();
    // Fresh command IDs and non-unique result IDs keep these failures from being
    // accidentally satisfied by a primary-key/UNIQUE constraint instead of FK.
    assert_eq!(tx.execute("INSERT INTO mailbox_proof_receipts(command_id,workflow_id,action,request_digest,result_proof_id,result_state) VALUES(?1,?2,1,?3,?4,1)", [Value::Blob(command(3).to_bytes().to_vec()), Value::Blob(first_bound.operation().to_bytes().to_vec()), Value::Blob(vec![9; 32]), Value::Blob(second.id().to_bytes().to_vec())]).await, Err(foreign_key));
    tx.rollback().await.unwrap();
    assert_eq!(store.mailbox_receipt(command(3)).await.unwrap(), None);
    assert_eq!(current(&store, first_bound).await.id(), first.id());
    assert_eq!(counts(&store).await, (2, 2, 2));

    let tx = store.database.begin_write().await.unwrap();
    tx.execute("INSERT INTO mailbox_proof_receipts(command_id,workflow_id,action,request_digest,result_proof_id,result_state) VALUES(?1,?2,1,?3,?4,1)", [Value::Blob(command(3).to_bytes().to_vec()), Value::Blob(first_bound.operation().to_bytes().to_vec()), Value::Blob(vec![9; 32]), Value::Blob(first.id().to_bytes().to_vec())]).await.unwrap();
    tx.commit().await.unwrap();
    let matching = store.mailbox_receipt(command(3)).await.unwrap().unwrap();
    assert_eq!(matching.command, command(3));
    assert_eq!(matching.action, ProofReceiptAction::Issued);
    assert_eq!(matching.proof, current(&store, first_bound).await);
    assert_eq!(counts(&store).await, (2, 2, 3));
}

#[tokio::test]
async fn schema_checks_windows_recovery_shape_epoch_and_receipt_result_state() {
    let run = TestRun::new("identity-proof-schema-constraints");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    let store = create(&run, clock).await;
    let check = StoreFault::Constraint {
        kind: ConstraintKind::Check,
        target: None,
    };
    let tx = store.database.begin_write().await.unwrap();
    assert_eq!(tx.execute("INSERT INTO mailbox_proof_issues(proof_id,workflow_id,secret_digest,issued_at,expires_at) VALUES(?1,?2,?3,1,1)", [Value::Blob(1_u128.to_be_bytes().to_vec()), Value::Blob(operation(1).to_bytes().to_vec()), Value::Blob(vec![7; 32])]).await, Err(check.clone()));
    tx.rollback().await.unwrap();
    assert_eq!(counts(&store).await, (0, 0, 0));

    let bound = binding(&store, 1);
    let material = ProofMaterial::generate().unwrap();
    store
        .issue_mailbox_proof(command(1), bound, &material, time(NOW + 60))
        .await
        .unwrap();
    for (action, state) in [(1, 2), (2, 1), (3, 2), (4, 3)] {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(tx.execute("INSERT INTO mailbox_proof_receipts(command_id,workflow_id,action,request_digest,result_proof_id,result_state) VALUES(?1,?2,?3,?4,?5,?6)", [Value::Blob(command(2).to_bytes().to_vec()), Value::Blob(bound.operation().to_bytes().to_vec()), Value::Integer(action), Value::Blob(vec![9; 32]), Value::Blob(material.id().to_bytes().to_vec()), Value::Integer(state)]).await, Err(check.clone()));
        tx.rollback().await.unwrap();
        assert_eq!(store.mailbox_receipt(command(2)).await.unwrap(), None);
        assert_eq!(counts(&store).await, (1, 1, 1));
    }

    let other_epoch = if store.epoch().to_bytes() == [1; 16] {
        [2; 16]
    } else {
        [1; 16]
    };
    let signup_sql = "INSERT INTO mailbox_proof_workflows(workflow_id,instance_epoch,intent,recovery_subject,recovery_version,purpose,current_proof_id,state) VALUES(?1,?2,1,NULL,NULL,NULL,?3,1)";
    let invalid_recovery_sql = "INSERT INTO mailbox_proof_workflows(workflow_id,instance_epoch,intent,recovery_subject,recovery_version,purpose,current_proof_id,state) VALUES(?1,?2,2,NULL,1,1,?3,1)";
    for (epoch, sql, expected) in [
        (
            other_epoch,
            signup_sql,
            Some(StoreFault::Constraint {
                kind: ConstraintKind::ForeignKey,
                target: None,
            }),
        ),
        (store.epoch().to_bytes(), invalid_recovery_sql, Some(check)),
        (store.epoch().to_bytes(), signup_sql, None),
    ] {
        let tx = store.database.begin_write().await.unwrap();
        // Valid unreceipted issue, distinct ID/digest and matching workflow: no
        // unrelated FK/UNIQUE failure can satisfy the negative control.
        tx.execute("INSERT INTO mailbox_proof_issues(proof_id,workflow_id,secret_digest,issued_at,expires_at) VALUES(?1,?2,?3,1,2)", [Value::Blob(2_u128.to_be_bytes().to_vec()), Value::Blob(operation(2).to_bytes().to_vec()), Value::Blob(vec![8; 32])]).await.unwrap();
        let inserted = tx
            .execute(
                sql,
                [
                    Value::Blob(operation(2).to_bytes().to_vec()),
                    Value::Blob(epoch.to_vec()),
                    Value::Blob(2_u128.to_be_bytes().to_vec()),
                ],
            )
            .await;
        match expected {
            Some(error) => {
                assert_eq!(inserted, Err(error));
                tx.rollback().await.unwrap();
                assert_eq!(store.mailbox_proof(operation(2)).await.unwrap(), None);
                assert_eq!(counts(&store).await, (1, 1, 1));
            }
            None => {
                assert_eq!(inserted, Ok(1));
                tx.commit().await.unwrap();
                assert_eq!(
                    current(&store, binding(&store, 2)).await.state(),
                    ProofState::Pending
                );
                assert_eq!(counts(&store).await, (2, 2, 1));
            }
        }
    }
}
