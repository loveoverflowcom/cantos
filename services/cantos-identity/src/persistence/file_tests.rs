//! Real pinned-engine tests. Every database and sidecar belongs to one private test run.
//! These tests establish storage behavior, never credential, RPC or login acceptance.

use std::{sync::Arc, time::Duration};

use cantos_store_turso::error::{ConstraintKind, GuardCode, OpenError, StoreFault};
use cantos_store_turso::registry::{DatabaseKind, Intent, Refusal, Registry};
use cantos_store_turso::testing::{FixedClock, TestRun};
use cantos_store_turso::{OpenRequest, OwnedDatabase, Value};

use super::{registry, ChangeKind, IdentityStore, IdentityStoreError};
use crate::domain::admission::{
    check_completion, AccountState, Assurance, CompletionDenied, Purpose, RequestBinding,
};
use crate::domain::ids::{InstanceEpoch, OperationId, SubjectId};
use crate::domain::security_version::SecurityVersion;

fn subject(value: u128) -> SubjectId {
    SubjectId::from_bytes(value.to_be_bytes()).unwrap()
}

fn operation(value: u128) -> OperationId {
    OperationId::from_bytes(value.to_be_bytes()).unwrap()
}

fn version(value: u64) -> SecurityVersion {
    SecurityVersion::new(value).unwrap()
}

fn guard(code: &str) -> StoreFault {
    StoreFault::Guard(GuardCode::new(code).unwrap())
}

async fn create(run: &TestRun) -> IdentityStore {
    IdentityStore::open(
        run.path("identity.db"),
        Intent::Migrate,
        FixedClock::new(1_700_000_000_000_000),
    )
    .await
    .unwrap()
}

async fn reopen(run: &TestRun) -> IdentityStore {
    IdentityStore::open(
        run.path("identity.db"),
        Intent::Serve,
        FixedClock::new(1_700_000_001_000_000),
    )
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

async fn assert_changed_epoch_is_rejected(store: &IdentityStore, other_epoch: InstanceEpoch) {
    // Keep the writer occupied: the epoch refusal must occur before waiting for it or
    // looking up a receipt. The deadline detects an incorrect writer-first ordering.
    let held = store.database.begin_write().await.unwrap();
    tokio::time::timeout(Duration::from_secs(1), async {
        assert_eq!(
            store
                .create_subject(operation(21), subject(2), other_epoch)
                .await,
            Err(IdentityStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .create_subject(operation(11), subject(1), other_epoch)
                .await,
            Err(IdentityStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .disable_subject(operation(22), subject(1), version(2), other_epoch)
                .await,
            Err(IdentityStoreError::EpochChanged)
        );
        assert_eq!(
            store
                .disable_subject(operation(12), subject(1), version(1), other_epoch)
                .await,
            Err(IdentityStoreError::EpochChanged)
        );
    })
    .await
    .expect("a changed epoch must refuse without acquiring the writer");
    held.rollback().await.unwrap();
    assert_eq!(store.subject(subject(2)).await.unwrap(), None);
    assert_eq!(store.receipt(operation(21)).await.unwrap(), None);
    assert_eq!(store.receipt(operation(22)).await.unwrap(), None);
    assert_eq!(
        count(store, "SELECT count(*) FROM identity_subjects").await,
        1
    );
    assert_eq!(
        count(store, "SELECT count(*) FROM security_events").await,
        2
    );
    assert_eq!(
        count(store, "SELECT count(*) FROM operation_receipts").await,
        2
    );
}

#[tokio::test]
async fn creation_and_its_exact_receipt_survive_close_and_reopen() {
    let run = TestRun::new("identity-create-reopen");
    let store = create(&run).await;
    let epoch = store.epoch();
    let receipt = store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(receipt.operation, operation(11));
    assert_eq!(receipt.subject, subject(1));
    assert_eq!(receipt.kind, ChangeKind::Created);
    assert_eq!(receipt.expected_version, None);
    assert_eq!(receipt.committed_version, version(1));
    assert_eq!(receipt.event_seq, 1);
    assert_eq!(receipt.epoch, epoch);
    drop(store);

    let store = reopen(&run).await;
    assert_eq!(store.epoch(), epoch);
    assert_eq!(store.receipt(operation(11)).await.unwrap(), Some(receipt));
    let persisted = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(persisted.subject, subject(1));
    assert_eq!(persisted.epoch, epoch);
    assert_eq!(persisted.version, version(1));
    assert_eq!(persisted.state, AccountState::Active);
    let events = store.events(0, 1).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].seq, 1);
    assert_eq!(events[0].subject, subject(1));
    assert_eq!(events[0].version, version(1));
    assert_eq!(events[0].state, AccountState::Active);
    assert_eq!(events[0].kind, ChangeKind::Created);
}

#[tokio::test]
async fn disable_commits_state_version_event_and_receipt_together() {
    let run = TestRun::new("identity-disable-atomic");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    let started = store.subject(subject(1)).await.unwrap().unwrap().snapshot();
    let binding = RequestBinding::new(operation(31), Purpose::Theatre);
    assert_eq!(
        check_completion(started, binding, started, binding, Assurance::Password),
        Ok(())
    );

    let receipt = store
        .disable_subject(operation(12), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(receipt.kind, ChangeKind::Disabled);
    assert_eq!(receipt.expected_version, Some(version(1)));
    assert_eq!(receipt.committed_version, version(2));
    assert_eq!(receipt.event_seq, 2);
    let current = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(current.version, version(2));
    assert_eq!(current.state, AccountState::Disabled);
    assert_eq!(store.receipt(operation(12)).await.unwrap(), Some(receipt));
    let events = store.events(1, 1).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(events[0].seq, 2);
    assert_eq!(events[0].version, version(2));
    assert_eq!(events[0].state, AccountState::Disabled);
    assert_eq!(events[0].kind, ChangeKind::Disabled);
    assert_eq!(
        check_completion(
            started,
            binding,
            current.snapshot(),
            binding,
            Assurance::Password
        ),
        Err(CompletionDenied::InactiveAccount)
    );
}

#[tokio::test]
async fn replay_returns_the_original_receipt_without_claiming_current_active_state() {
    let run = TestRun::new("identity-replay");
    let store = create(&run).await;
    let created = store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    let disabled = store
        .disable_subject(operation(12), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .create_subject(operation(11), subject(1), store.epoch())
            .await
            .unwrap(),
        created
    );
    assert_eq!(
        store
            .disable_subject(operation(12), subject(1), version(1), store.epoch())
            .await
            .unwrap(),
        disabled
    );
    let current = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(current.state, AccountState::Disabled);
    assert_eq!(current.version, version(2));
    assert_eq!(
        count(&store, "SELECT count(*) FROM identity_subjects").await,
        1
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        2
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        2
    );
}

#[tokio::test]
async fn an_operation_id_cannot_change_subject_kind_or_expected_version() {
    let run = TestRun::new("identity-operation-binding");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .create_subject(operation(11), subject(2), store.epoch())
            .await,
        Err(IdentityStoreError::OperationReused)
    );
    assert_eq!(
        store
            .disable_subject(operation(11), subject(1), version(1), store.epoch())
            .await,
        Err(IdentityStoreError::OperationReused)
    );
    store
        .disable_subject(operation(12), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .disable_subject(operation(12), subject(1), version(2), store.epoch())
            .await,
        Err(IdentityStoreError::OperationReused)
    );
    assert_eq!(store.subject(subject(2)).await.unwrap(), None);
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        2
    );
}

#[tokio::test]
async fn rejected_preconditions_leave_no_receipt_or_partial_effect() {
    let run = TestRun::new("identity-negative-preconditions");
    let store = create(&run).await;
    assert_eq!(
        store
            .disable_subject(operation(20), subject(1), version(1), store.epoch())
            .await,
        Err(IdentityStoreError::SubjectMissing)
    );
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .create_subject(operation(21), subject(1), store.epoch())
            .await,
        Err(IdentityStoreError::SubjectExists)
    );
    assert_eq!(
        store
            .disable_subject(operation(22), subject(1), version(2), store.epoch())
            .await,
        Err(IdentityStoreError::Conflict)
    );
    let current = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(current.version, version(1));
    assert_eq!(current.state, AccountState::Active);
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        1
    );
    for id in [20, 21, 22] {
        assert_eq!(store.receipt(operation(id)).await.unwrap(), None);
    }
    store
        .disable_subject(operation(12), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store
            .disable_subject(operation(23), subject(1), version(2), store.epoch())
            .await,
        Err(IdentityStoreError::Inactive)
    );
    assert_eq!(store.receipt(operation(23)).await.unwrap(), None);
}

#[tokio::test]
async fn simultaneous_identical_creations_have_one_durable_effect() {
    let run = TestRun::new("identity-concurrent-replay");
    let store = Arc::new(create(&run).await);
    let (left, right) = tokio::join!(
        store.create_subject(operation(11), subject(1), store.epoch()),
        store.create_subject(operation(11), subject(1), store.epoch()),
    );
    assert_eq!(left.unwrap(), right.unwrap());
    assert_eq!(
        count(&store, "SELECT count(*) FROM identity_subjects").await,
        1
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        1
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        1
    );
}

#[tokio::test]
async fn competing_disables_cannot_both_advance_the_observed_version() {
    let run = TestRun::new("identity-concurrent-cas");
    let store = Arc::new(create(&run).await);
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    let (left, right) = tokio::join!(
        store.disable_subject(operation(12), subject(1), version(1), store.epoch()),
        store.disable_subject(operation(13), subject(1), version(1), store.epoch()),
    );
    let winner = match (left, right) {
        (Ok(receipt), Err(IdentityStoreError::Conflict)) => receipt,
        (Err(IdentityStoreError::Conflict), Ok(receipt)) => receipt,
        other => panic!("exactly one disable must win: {other:?}"),
    };
    assert_eq!(winner.committed_version, version(2));
    assert_eq!(winner.event_seq, 2);
    assert_eq!(
        store.subject(subject(1)).await.unwrap().unwrap().version,
        version(2)
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        2
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        2
    );
}

#[tokio::test]
async fn a_response_lost_after_real_commit_reconciles_after_reopen_without_replay() {
    let run = TestRun::new("identity-lost-response");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    // The real engine committed; the consumer deliberately discards its returned receipt.
    // This is response-loss recovery, not injected engine COMMIT-Unknown evidence.
    let _ = store
        .disable_subject(operation(12), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    drop(store);
    let store = reopen(&run).await;
    let durable = store.receipt(operation(12)).await.unwrap().unwrap();
    assert_eq!(durable.kind, ChangeKind::Disabled);
    assert_eq!(durable.committed_version, version(2));
    assert_eq!(
        store
            .disable_subject(operation(12), subject(1), version(1), store.epoch())
            .await
            .unwrap(),
        durable
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        2
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        2
    );
}

#[tokio::test]
async fn event_insert_failure_rolls_back_the_already_updated_subject() {
    let run = TestRun::new("identity-partial-write-rollback");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    let tx = store.database.begin_write().await.unwrap();
    tx.execute(
        "CREATE TRIGGER test_fail_disable_event BEFORE INSERT ON security_events \
         WHEN NEW.kind=2 BEGIN SELECT RAISE(ABORT,'cantos:test.fail_event'); END;",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();

    assert_eq!(
        store
            .disable_subject(operation(12), subject(1), version(1), store.epoch())
            .await,
        Err(IdentityStoreError::Storage(guard("test.fail_event")))
    );
    let current = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(current.state, AccountState::Active);
    assert_eq!(current.version, version(1));
    assert_eq!(store.receipt(operation(12)).await.unwrap(), None);
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        1
    );
}

#[tokio::test]
async fn settled_events_receipts_ids_and_epoch_reject_direct_mutation() {
    let run = TestRun::new("identity-immutable-records");
    let store = create(&run).await;
    let created = store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    for (sql, code) in [
        ("UPDATE security_events SET recorded_at=recorded_at+1", "identity.immutable_event"),
        ("DELETE FROM security_events", "identity.immutable_event"),
        ("UPDATE operation_receipts SET committed_version=2", "identity.immutable_receipt"),
        ("DELETE FROM operation_receipts", "identity.immutable_receipt"),
        ("UPDATE identity_subjects SET subject_id=x'00000000000000000000000000000002',security_version=2,state=2", "identity.immutable_subject"),
        ("DELETE FROM identity_subjects", "identity.immutable_subject"),
        ("UPDATE identity_instance SET instance_epoch=x'00000000000000000000000000000002'", "identity.immutable_instance"),
        ("DELETE FROM identity_instance", "identity.immutable_instance"),
    ] {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(tx.execute(sql, ()).await, Err(guard(code)), "{sql}");
        tx.rollback().await.unwrap();
    }
    assert_eq!(store.receipt(operation(11)).await.unwrap(), Some(created));
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        1
    );
    assert_eq!(
        store.subject(subject(1)).await.unwrap().unwrap().version,
        version(1)
    );
}

#[tokio::test]
async fn receipts_require_exact_event_subject_and_version_associations() {
    let run = TestRun::new("identity-receipt-event-binding");
    let store = create(&run).await;
    let tx = store.database.begin_write().await.unwrap();
    // These are valid synthetic rows under the real schema. No receipt exists for any
    // fixture event, so UNIQUE(event_seq) cannot mask a broken composite foreign key.
    for sql in [
        "INSERT INTO identity_subjects(subject_id,security_version,state) \
         VALUES(x'00000000000000000000000000000001',2,2)",
        "INSERT INTO identity_subjects(subject_id,security_version,state) \
         VALUES(x'00000000000000000000000000000002',1,1)",
        "INSERT INTO security_events(seq,subject_id,security_version,state,kind,recorded_at) \
         VALUES(101,x'00000000000000000000000000000001',1,1,1,0)",
        "INSERT INTO security_events(seq,subject_id,security_version,state,kind,recorded_at) \
         VALUES(102,x'00000000000000000000000000000002',1,1,1,1)",
        "INSERT INTO security_events(seq,subject_id,security_version,state,kind,recorded_at) \
         VALUES(103,x'00000000000000000000000000000001',2,2,2,2)",
    ] {
        assert_eq!(tx.execute(sql, ()).await.unwrap(), 1);
    }
    tx.commit().await.unwrap();
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        0
    );

    const INSERT_RECEIPT: &str = "INSERT INTO operation_receipts(\
        operation_id,subject_id,kind,expected_version,committed_version,event_seq) \
        VALUES(?1,?2,?3,?4,?5,?6)";
    for (label, id, bound_subject, kind, expected, committed, event_seq) in [
        ("wrong subject", 21, subject(2), 1, 0, 1, 101),
        ("wrong version", 22, subject(1), 2, 2, 3, 103),
        ("different existing event", 23, subject(1), 1, 0, 1, 102),
        ("missing event", 24, subject(1), 1, 0, 1, 104),
    ] {
        let tx = store.database.begin_write().await.unwrap();
        let result = tx
            .execute(
                INSERT_RECEIPT,
                cantos_store_turso::params_from_iter([
                    Value::Blob(operation(id).to_bytes().to_vec()),
                    Value::Blob(bound_subject.to_bytes().to_vec()),
                    Value::Integer(kind),
                    Value::Integer(expected),
                    Value::Integer(committed),
                    Value::Integer(event_seq),
                ]),
            )
            .await;
        assert_eq!(
            result,
            Err(StoreFault::Constraint {
                kind: ConstraintKind::ForeignKey,
                target: None,
            }),
            "{label} must fail the composite event binding, not another constraint"
        );
        tx.rollback().await.unwrap();
        assert_eq!(store.receipt(operation(id)).await.unwrap(), None);
        assert_eq!(
            count(&store, "SELECT count(*) FROM operation_receipts").await,
            0
        );
    }

    // Positive controls use the same previously unreceipted events and statement. They
    // rule out a blanket rejection of all receipt inserts as the negative-test oracle.
    for (id, bound_subject, kind, expected, committed, event_seq) in [
        (31, subject(1), 1, 0, 1, 101),
        (32, subject(1), 2, 1, 2, 103),
        (33, subject(2), 1, 0, 1, 102),
    ] {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(
            tx.execute(
                INSERT_RECEIPT,
                cantos_store_turso::params_from_iter([
                    Value::Blob(operation(id).to_bytes().to_vec()),
                    Value::Blob(bound_subject.to_bytes().to_vec()),
                    Value::Integer(kind),
                    Value::Integer(expected),
                    Value::Integer(committed),
                    Value::Integer(event_seq),
                ]),
            )
            .await
            .unwrap(),
            1
        );
        tx.commit().await.unwrap();
        let receipt = store.receipt(operation(id)).await.unwrap().unwrap();
        assert_eq!(receipt.subject, bound_subject);
        assert_eq!(receipt.committed_version, version(committed as u64));
        assert_eq!(receipt.event_seq, event_seq as u64);
        assert_eq!(receipt.epoch, store.epoch());
    }
    assert_eq!(
        count(&store, "SELECT count(*) FROM operation_receipts").await,
        3
    );
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        3
    );
}

#[tokio::test]
async fn invalid_stored_shapes_and_non_disable_transitions_are_refused() {
    let run = TestRun::new("identity-row-constraints");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    for sql in [
        "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(x'00',1,1)",
        "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(x'00000000000000000000000000000000',1,1)",
        "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(x'00000000000000000000000000000002',0,1)",
        "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(x'00000000000000000000000000000002',1,99)",
    ] {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(
            tx.execute(sql, ()).await,
            Err(StoreFault::Constraint { kind: ConstraintKind::Check, target: None }),
            "{sql}"
        );
        tx.rollback().await.unwrap();
    }
    for sql in [
        "UPDATE identity_subjects SET security_version=3,state=2",
        "UPDATE identity_subjects SET security_version=2,state=1",
    ] {
        let tx = store.database.begin_write().await.unwrap();
        assert_eq!(
            tx.execute(sql, ()).await,
            Err(guard("identity.subject_transition"))
        );
        tx.rollback().await.unwrap();
    }
    assert_eq!(
        count(&store, "SELECT count(*) FROM identity_subjects").await,
        1
    );
    assert_eq!(
        store.subject(subject(1)).await.unwrap().unwrap().version,
        version(1)
    );
}

#[tokio::test]
async fn serve_refuses_missing_epoch_and_migrate_only_initializes_an_empty_store() {
    let run = TestRun::new("identity-epoch-initialization");
    let clock = FixedClock::new(1_700_000_000_000_000);
    let database = OwnedDatabase::open(OpenRequest::new(
        run.path("identity.db"),
        registry().unwrap(),
        Intent::Migrate,
        clock.clone(),
    ))
    .await
    .unwrap();
    drop(database);
    match IdentityStore::open(run.path("identity.db"), Intent::Serve, clock.clone()).await {
        Err(error) => assert_eq!(error, IdentityStoreError::Uninitialized),
        Ok(_) => panic!("Serve must not invent a missing epoch"),
    }
    let store = IdentityStore::open(run.path("identity.db"), Intent::Migrate, clock.clone())
        .await
        .unwrap();
    let epoch = store.epoch();
    drop(store);
    assert_eq!(reopen(&run).await.epoch(), epoch);

    let populated = TestRun::new("identity-no-epoch-with-data");
    let database = OwnedDatabase::open(OpenRequest::new(
        populated.path("identity.db"),
        registry().unwrap(),
        Intent::Migrate,
        clock.clone(),
    ))
    .await
    .unwrap();
    let tx = database.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO identity_subjects(subject_id,security_version,state) \
         VALUES(x'00000000000000000000000000000001',1,1)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(database);
    match IdentityStore::open(populated.path("identity.db"), Intent::Migrate, clock).await {
        Err(error) => assert_eq!(error, IdentityStoreError::CorruptRecord),
        Ok(_) => panic!("missing epoch with data must not be silently initialized"),
    }
}

#[tokio::test]
async fn a_foreign_file_and_second_owner_are_refused_without_adoption() {
    let run = TestRun::new("identity-owner-and-kind");
    let store = create(&run).await;
    match IdentityStore::open(run.path("identity.db"), Intent::Serve, FixedClock::new(1)).await {
        Err(error) => assert_eq!(error, IdentityStoreError::Open(OpenError::AlreadyOwned)),
        Ok(_) => panic!("a second owner must be refused"),
    }
    drop(store);

    let foreign = TestRun::new("identity-foreign-server");
    let identity_registry = registry().unwrap();
    let server_registry = Registry::new(
        DatabaseKind::Server,
        identity_registry.epoch(),
        identity_registry.migrations(),
    )
    .unwrap();
    let database = OwnedDatabase::open(OpenRequest::new(
        foreign.path("identity.db"),
        server_registry,
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .unwrap();
    drop(database);
    match IdentityStore::open(
        foreign.path("identity.db"),
        Intent::Migrate,
        FixedClock::new(1),
    )
    .await
    {
        Err(error) => assert_eq!(
            error,
            IdentityStoreError::Open(OpenError::Refused(Refusal::ForeignApplication {
                found: DatabaseKind::Server.application_id(),
            }))
        ),
        Ok(_) => panic!("a Server-owned file must never become Identity-owned"),
    }
}

#[tokio::test]
async fn event_queries_are_bounded_ordered_and_do_not_return_already_observed_rows() {
    let run = TestRun::new("identity-event-pagination");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    store
        .create_subject(operation(21), subject(2), store.epoch())
        .await
        .unwrap();
    store
        .disable_subject(operation(12), subject(1), version(1), store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store.events(0, 0).await,
        Err(IdentityStoreError::InvalidEventRange)
    );
    assert_eq!(
        store.events(0, 129).await,
        Err(IdentityStoreError::InvalidEventRange)
    );
    assert_eq!(
        store.events(u64::MAX, 1).await,
        Err(IdentityStoreError::InvalidEventRange)
    );
    let first = store.events(0, 2).await.unwrap();
    assert_eq!(
        first.iter().map(|event| event.seq).collect::<Vec<_>>(),
        [1, 2]
    );
    let second = store.events(first[1].seq, 2).await.unwrap();
    assert_eq!(
        second.iter().map(|event| event.seq).collect::<Vec<_>>(),
        [3]
    );
    assert_eq!(second[0].subject, subject(1));
    assert_eq!(second[0].kind, ChangeKind::Disabled);
    assert_eq!(store.events(3, 1).await.unwrap(), []);
}

#[tokio::test]
async fn storage_counter_exhaustion_never_wraps_or_leaves_partial_effects() {
    let run = TestRun::new("identity-counter-exhaustion");
    let store = create(&run).await;
    store
        .create_subject(operation(11), subject(1), store.epoch())
        .await
        .unwrap();
    let tx = store.database.begin_write().await.unwrap();
    // Inject the highest valid storage generation directly, without pretending to have
    // executed the enormous history that would ordinarily lead to this boundary.
    tx.execute(
        "INSERT INTO identity_subjects(subject_id,security_version,state) \
         VALUES(x'00000000000000000000000000000002',9223372036854775807,1)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let tx = store.database.begin_write().await.unwrap();
    assert_eq!(
        tx.execute(
            "UPDATE identity_subjects SET security_version=9223372036854775807,state=2 WHERE subject_id=x'00000000000000000000000000000002'",
            (),
        ).await,
        Err(guard("identity.subject_transition"))
    );
    tx.rollback().await.unwrap();
    assert_eq!(
        store
            .disable_subject(
                operation(21),
                subject(2),
                version(i64::MAX as u64),
                store.epoch()
            )
            .await,
        Err(IdentityStoreError::VersionExhausted)
    );
    let unchanged = store.subject(subject(2)).await.unwrap().unwrap();
    assert_eq!(unchanged.version, version(i64::MAX as u64));
    assert_eq!(unchanged.state, AccountState::Active);
    assert_eq!(store.receipt(operation(21)).await.unwrap(), None);

    let tx = store.database.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO security_events(seq,subject_id,security_version,state,kind,recorded_at) \
         VALUES(9223372036854775807,x'00000000000000000000000000000001',2,2,2,0)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        store
            .create_subject(operation(31), subject(3), store.epoch())
            .await,
        Err(IdentityStoreError::EventSequenceExhausted)
    );
    assert_eq!(store.subject(subject(3)).await.unwrap(), None);
    assert_eq!(store.receipt(operation(31)).await.unwrap(), None);
    assert_eq!(
        store
            .disable_subject(operation(32), subject(1), version(1), store.epoch())
            .await,
        Err(IdentityStoreError::EventSequenceExhausted)
    );
    let unchanged = store.subject(subject(1)).await.unwrap().unwrap();
    assert_eq!(unchanged.version, version(1));
    assert_eq!(unchanged.state, AccountState::Active);
    assert_eq!(store.receipt(operation(32)).await.unwrap(), None);
    assert_eq!(
        count(&store, "SELECT count(*) FROM security_events").await,
        2
    );
}

#[tokio::test]
async fn changed_epoch_refuses_new_and_replayed_operations_before_and_after_reopen() {
    let run = TestRun::new("identity-request-epoch");
    let store = create(&run).await;
    let epoch = store.epoch();
    let other_bytes = if epoch.to_bytes() == [1; 16] {
        [2; 16]
    } else {
        [1; 16]
    };
    let other_epoch = InstanceEpoch::from_bytes(other_bytes).unwrap();
    assert_ne!(epoch, other_epoch);
    let created = store
        .create_subject(operation(11), subject(1), epoch)
        .await
        .unwrap();
    let disabled = store
        .disable_subject(operation(12), subject(1), version(1), epoch)
        .await
        .unwrap();
    assert_changed_epoch_is_rejected(&store, other_epoch).await;
    assert_eq!(
        store
            .create_subject(operation(11), subject(1), epoch)
            .await
            .unwrap(),
        created
    );
    assert_eq!(
        store
            .disable_subject(operation(12), subject(1), version(1), epoch)
            .await
            .unwrap(),
        disabled
    );
    drop(store);

    let store = reopen(&run).await;
    assert_eq!(store.epoch(), epoch);
    assert_changed_epoch_is_rejected(&store, other_epoch).await;
    assert_eq!(
        store
            .create_subject(operation(11), subject(1), epoch)
            .await
            .unwrap(),
        created
    );
    assert_eq!(
        store
            .disable_subject(operation(12), subject(1), version(1), epoch)
            .await
            .unwrap(),
        disabled
    );
}
