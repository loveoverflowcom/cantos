//! The Turso actor registry on the real pinned engine and run-owned files: the port contract,
//! rollback, controlled interleavings on one owner, reopen, corrupt projections and lost COMMIT
//! acknowledgements. Expected outcomes are literal values from the domain constructors; no
//! second registry model computes them.
use std::{sync::Arc, time::Duration};

use cantos_store_turso::{
    error::StoreFault,
    params_from_iter,
    registry::Intent,
    testing::{FixedClock, TestRun},
    Row, Value,
};

use super::{actor_row, at_most_one, binding_row, storage, Acknowledgement, TursoIdentityRegistry};
use crate::platform::{
    identity::{
        domain::{
            ActorId, ActorRecord, ExternalIdentityKey, IdentityError, LinkOutcome, Registration,
        },
        IdentityRegistry, IdentityService, RegistryError,
    },
    turso::{ServerDatabase, ServerDatabaseError},
};

const T0: i64 = 1_700_000_000_000_000;

fn actor(id: &str) -> ActorId {
    ActorId::new(id).unwrap()
}

fn key(issuer: &str, subject: &str) -> ExternalIdentityKey {
    ExternalIdentityKey::new(issuer, subject).unwrap()
}

fn record(id: &str, active: bool) -> ActorRecord {
    ActorRecord {
        id: actor(id),
        active,
    }
}

fn rejected(error: IdentityError) -> RegistryError {
    RegistryError::Rejected(error)
}

async fn open(run: &TestRun, intent: Intent, clock: Arc<FixedClock>) -> ServerDatabase {
    ServerDatabase::open(run.path("server.db"), intent, clock)
        .await
        .unwrap()
}

/// Operator-side actor rows the registry itself never writes: legacy and deactivated actors.
async fn seed(database: &ServerDatabase, id: &str, active: bool) {
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id,active) VALUES(?1,?2)",
        params_from_iter([Value::Text(id.into()), Value::Integer(i64::from(active))]),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn set_active(database: &ServerDatabase, id: &str, active: bool) {
    let tx = database.owner().begin_write().await.unwrap();
    let changed = tx
        .execute(
            "UPDATE actors SET active=?1 WHERE id=?2",
            params_from_iter([Value::Integer(i64::from(active)), Value::Text(id.into())]),
        )
        .await
        .unwrap();
    assert_eq!(changed, 1);
    tx.commit().await.unwrap();
}

async fn execute(database: &ServerDatabase, sql: &str) {
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute(sql, ()).await.unwrap();
    tx.commit().await.unwrap();
}

/// Every stored row, read back independently of the adapter's decoders.
#[derive(Clone, Debug, PartialEq, Eq)]
struct Stored {
    actors: Vec<(String, i64)>,
    bindings: Vec<(String, String, String, i64)>,
}

fn stored_text(row: &Row, index: usize) -> String {
    match row.get_value(index).unwrap() {
        Value::Text(value) => value,
        other => panic!("column {index} is {other:?}"),
    }
}

fn stored_integer(row: &Row, index: usize) -> i64 {
    match row.get_value(index).unwrap() {
        Value::Integer(value) => value,
        other => panic!("column {index} is {other:?}"),
    }
}

async fn stored(database: &ServerDatabase) -> Stored {
    let reader = database.owner().reader().await.unwrap();
    let mut rows = reader
        .query("SELECT id,active FROM actors ORDER BY id", ())
        .await
        .unwrap();
    let mut actors = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        actors.push((stored_text(&row, 0), stored_integer(&row, 1)));
    }
    let mut rows = reader
        .query(
            "SELECT issuer,subject,actor_id,linked_at FROM actor_external_identities ORDER BY issuer,subject",
            (),
        )
        .await
        .unwrap();
    let mut bindings = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        bindings.push((
            stored_text(&row, 0),
            stored_text(&row, 1),
            stored_text(&row, 2),
            stored_integer(&row, 3),
        ));
    }
    Stored { actors, bindings }
}

fn actors(rows: &[(&str, i64)]) -> Vec<(String, i64)> {
    rows.iter()
        .map(|(id, active)| ((*id).into(), *active))
        .collect()
}

fn bindings(rows: &[(&str, &str, &str, i64)]) -> Vec<(String, String, String, i64)> {
    rows.iter()
        .map(|(issuer, subject, actor, at)| {
            ((*issuer).into(), (*subject).into(), (*actor).into(), *at)
        })
        .collect()
}

#[tokio::test]
async fn registration_lookup_and_linking_follow_the_port_contract_on_a_real_file() {
    let run = TestRun::new("identity-turso-contract");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    seed(&database, "legacy", true).await;
    let service = IdentityService::new(TursoIdentityRegistry::new(database.clone()));
    let first = key("issuer-a", "same-email@example.test");
    assert_eq!(
        service.register(&first, &actor("alice")).await,
        Ok(Registration::Created(actor("alice")))
    );
    clock.set(T0 + 1);
    // Exact replay and a conflicting candidate both resolve to the winner and create nothing.
    for candidate in ["alice", "loser"] {
        assert_eq!(
            service.register(&first, &actor(candidate)).await,
            Ok(Registration::Existing(actor("alice")))
        );
    }
    // The same subject under another issuer is a different identity, never linked by email.
    assert_eq!(
        service
            .register(&key("issuer-b", first.subject()), &actor("bob"))
            .await,
        Ok(Registration::Created(actor("bob")))
    );
    clock.set(T0 + 2);
    let second = key("issuer-a", "AnotherCase");
    assert_eq!(
        service.link(&second, &actor("legacy")).await,
        Ok(LinkOutcome::Linked)
    );
    clock.set(T0 + 3);
    assert_eq!(
        service.link(&second, &actor("legacy")).await,
        Ok(LinkOutcome::Existing)
    );
    assert_eq!(
        service.lookup(&first).await,
        Ok(Some(record("alice", true)))
    );
    assert_eq!(
        service.lookup(&second).await,
        Ok(Some(record("legacy", true)))
    );
    assert_eq!(
        service.lookup(&key("issuer-a", "anothercase")).await,
        Ok(None)
    );
    assert_eq!(
        stored(&database).await,
        Stored {
            actors: actors(&[("alice", 1), ("bob", 1), ("legacy", 1)]),
            bindings: bindings(&[
                ("issuer-a", "AnotherCase", "legacy", T0 + 2),
                ("issuer-a", "same-email@example.test", "alice", T0),
                ("issuer-b", "same-email@example.test", "bob", T0 + 1),
            ]),
        }
    );
}

#[tokio::test]
async fn rejections_and_inactive_or_missing_targets_change_no_row() {
    let run = TestRun::new("identity-turso-rejections");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let registry = TursoIdentityRegistry::new(database.clone());
    let bound = key("issuer", "bound");
    let sleeping = key("issuer", "sleeping");
    let fresh = key("issuer", "fresh");
    registry.register(&bound, &actor("alice")).await.unwrap();
    registry
        .register(&sleeping, &actor("sleeper"))
        .await
        .unwrap();
    seed(&database, "legacy", true).await;
    seed(&database, "dormant", false).await;
    set_active(&database, "sleeper", false).await;
    let before = stored(&database).await;
    assert_eq!(
        registry.link(&bound, &actor("legacy")).await,
        Err(rejected(IdentityError::IdentityAlreadyBound))
    );
    assert_eq!(
        registry.register(&fresh, &actor("legacy")).await,
        Err(rejected(IdentityError::ActorIdCollision))
    );
    assert_eq!(
        registry.link(&fresh, &actor("missing")).await,
        Err(rejected(IdentityError::ActorMissing))
    );
    assert_eq!(
        registry.link(&fresh, &actor("dormant")).await,
        Err(rejected(IdentityError::ActorInactive))
    );
    // An inactive binding is never replaced by a new actor or reactivated by linking.
    assert_eq!(
        registry.register(&sleeping, &actor("replacement")).await,
        Err(rejected(IdentityError::ActorInactive))
    );
    assert_eq!(
        registry.link(&sleeping, &actor("sleeper")).await,
        Err(rejected(IdentityError::ActorInactive))
    );
    assert_eq!(
        registry.lookup(&sleeping).await,
        Ok(Some(record("sleeper", false)))
    );
    assert_eq!(registry.lookup(&fresh).await, Ok(None));
    assert_eq!(stored(&database).await, before);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_registrations_of_one_key_commit_one_binding_and_no_losing_actor() {
    let run = TestRun::new("identity-turso-concurrent");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let registry = TursoIdentityRegistry::new(database.clone());
    let mut expected_actors = Vec::new();
    for round in 0..12 {
        let shared = key("issuer", &format!("subject-{round}"));
        let tasks: Vec<_> = (0..4)
            .map(|candidate| {
                let registry = registry.clone();
                let shared = shared.clone();
                tokio::spawn(async move {
                    registry
                        .register(&shared, &actor(&format!("candidate-{round}-{candidate}")))
                        .await
                })
            })
            .collect();
        let mut outcomes = Vec::new();
        for task in tasks {
            outcomes.push(task.await.unwrap().unwrap());
        }
        let winner = outcomes[0].actor_id().clone();
        assert!(outcomes.iter().all(|outcome| outcome.actor_id() == &winner));
        let created: Vec<_> = outcomes
            .iter()
            .filter(|outcome| matches!(outcome, Registration::Created(_)))
            .collect();
        assert_eq!(
            created,
            [&Registration::Created(winner.clone())],
            "{outcomes:?}"
        );
        assert_eq!(
            registry.lookup(&shared).await,
            Ok(Some(ActorRecord {
                id: winner.clone(),
                active: true
            }))
        );
        expected_actors.push((winner.as_str().to_owned(), 1));
    }
    expected_actors.sort();
    let rows = stored(&database).await;
    assert_eq!(rows.actors, expected_actors);
    assert_eq!(rows.bindings.len(), 12);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_held_writer_orders_competing_requests_and_readers_never_see_uncommitted_bindings() {
    let run = TestRun::new("identity-turso-interleaving");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let registry = TursoIdentityRegistry::new(database.clone());
    let pending = |shared: ExternalIdentityKey, candidate: &'static str| {
        let registry = registry.clone();
        tokio::spawn(async move { registry.register(&shared, &actor(candidate)).await })
    };

    // Another writer of the same owner binds the key first; the request waits, then replays.
    let raced = key("issuer", "raced");
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute("INSERT INTO actors(id) VALUES('winner')", ())
        .await
        .unwrap();
    tx.execute(
        "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('issuer','raced','winner',1)",
        (),
    )
    .await
    .unwrap();
    let waiting = pending(raced.clone(), "loser");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !waiting.is_finished(),
        "a request must not begin while the writer is held"
    );
    assert_eq!(registry.lookup(&raced).await, Ok(None));
    tx.commit().await.unwrap();
    assert_eq!(
        waiting.await.unwrap(),
        Ok(Registration::Existing(actor("winner")))
    );
    assert_eq!(
        registry.lookup(&raced).await,
        Ok(Some(record("winner", true)))
    );

    // A competing writer that rolls back leaves the key free for the waiting request.
    let abandoned = key("issuer", "abandoned");
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute("INSERT INTO actors(id) VALUES('ghost')", ())
        .await
        .unwrap();
    tx.execute(
        "INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('issuer','abandoned','ghost',1)",
        (),
    )
    .await
    .unwrap();
    let waiting = pending(abandoned.clone(), "late");
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!waiting.is_finished());
    assert_eq!(registry.lookup(&abandoned).await, Ok(None));
    tx.rollback().await.unwrap();
    assert_eq!(
        waiting.await.unwrap(),
        Ok(Registration::Created(actor("late")))
    );

    // A deactivation committed while a link waits is the fact the link decides on.
    seed(&database, "target", true).await;
    let tx = database.owner().begin_write().await.unwrap();
    tx.execute("UPDATE actors SET active=0 WHERE id='target'", ())
        .await
        .unwrap();
    let linking = {
        let registry = registry.clone();
        tokio::spawn(async move {
            registry
                .link(&key("issuer", "linked"), &actor("target"))
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(!linking.is_finished());
    tx.commit().await.unwrap();
    assert_eq!(
        linking.await.unwrap(),
        Err(rejected(IdentityError::ActorInactive))
    );

    assert_eq!(
        stored(&database).await,
        Stored {
            actors: actors(&[("late", 1), ("target", 0), ("winner", 1)]),
            bindings: bindings(&[
                ("issuer", "abandoned", "late", T0),
                ("issuer", "raced", "winner", 1),
            ]),
        }
    );
}

#[tokio::test]
async fn a_failed_step_after_the_actor_insert_rolls_back_the_speculative_actor() {
    let run = TestRun::new("identity-turso-partial-failure");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let registry = TursoIdentityRegistry::new(database.clone());
    seed(&database, "winner", true).await;
    // Fault injection through the real engine: a binding appears between the decision and the
    // guarded insert, and another insert is refused by a trigger.
    execute(
        &database,
        "CREATE TRIGGER test_competing_binding AFTER INSERT ON actors WHEN NEW.id = 'loser' \
         BEGIN INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) \
         VALUES('issuer','raced','winner',0); END",
    )
    .await;
    execute(
        &database,
        "CREATE TRIGGER test_refused_binding BEFORE INSERT ON actor_external_identities \
         WHEN NEW.subject = 'failing' BEGIN SELECT RAISE(ABORT, 'cantos:test.injected'); END",
    )
    .await;
    let before = stored(&database).await;
    assert_eq!(
        registry
            .register(&key("issuer", "raced"), &actor("loser"))
            .await,
        Err(rejected(IdentityError::IdentityAlreadyBound))
    );
    assert_eq!(
        registry
            .register(&key("issuer", "failing"), &actor("orphan"))
            .await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(
        registry
            .link(&key("issuer", "failing"), &actor("winner"))
            .await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(stored(&database).await, before);
    assert_eq!(before.actors, actors(&[("winner", 1)]));
    assert!(before.bindings.is_empty());
}

#[tokio::test]
async fn bindings_survive_reopen_and_replay_returns_the_original_actor() {
    let run = TestRun::new("identity-turso-reopen");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let registry = TursoIdentityRegistry::new(database.clone());
    let first = key("issuer", "first");
    let second = key("issuer", "second");
    registry.register(&first, &actor("alice")).await.unwrap();
    clock.set(i64::MAX);
    assert_eq!(
        registry.link(&second, &actor("alice")).await,
        Ok(LinkOutcome::Linked)
    );
    match ServerDatabase::open(run.path("server.db"), Intent::Serve, FixedClock::new(T0)).await {
        Err(error) => assert_eq!(
            error,
            ServerDatabaseError::Open(cantos_store_turso::error::OpenError::AlreadyOwned)
        ),
        Ok(_) => panic!("a second owner must be refused"),
    }
    drop(registry);
    drop(database);

    let database = open(&run, Intent::Serve, FixedClock::new(T0 + 10)).await;
    let registry = TursoIdentityRegistry::new(database.clone());
    assert_eq!(
        registry.lookup(&first).await,
        Ok(Some(record("alice", true)))
    );
    assert_eq!(
        registry.register(&first, &actor("bob")).await,
        Ok(Registration::Existing(actor("alice")))
    );
    assert_eq!(
        registry.link(&second, &actor("alice")).await,
        Ok(LinkOutcome::Existing)
    );
    assert_eq!(
        stored(&database).await,
        Stored {
            actors: actors(&[("alice", 1)]),
            bindings: bindings(&[
                ("issuer", "first", "alice", T0),
                ("issuer", "second", "alice", i64::MAX)
            ]),
        }
    );
}

#[tokio::test]
async fn a_lost_acknowledgement_after_a_real_commit_reconciles_by_exact_key_replay() {
    let run = TestRun::new("identity-turso-unknown-committed");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let lossy = TursoIdentityRegistry::with_acknowledgement(
        database.clone(),
        Acknowledgement::LostAfterCommit,
    );
    let engine = TursoIdentityRegistry::new(database.clone());
    let first = key("issuer", "first");
    let second = key("issuer", "second");
    seed(&database, "legacy", true).await;
    assert_eq!(
        lossy.register(&first, &actor("alice")).await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(
        lossy.link(&second, &actor("legacy")).await,
        Err(RegistryError::Unavailable)
    );
    // A decision that wrote nothing needs no acknowledgement, so it is still exact.
    assert_eq!(
        lossy.register(&first, &actor("carol")).await,
        Ok(Registration::Existing(actor("alice")))
    );
    assert_eq!(
        lossy.link(&first, &actor("legacy")).await,
        Err(rejected(IdentityError::IdentityAlreadyBound))
    );
    clock.set(T0 + 1);
    // Reconcile through the immutable binding: replaying the exact key returns the durable
    // winner and creates nothing, whichever candidate the replay names.
    assert_eq!(engine.lookup(&first).await, Ok(Some(record("alice", true))));
    for candidate in ["alice", "bob"] {
        assert_eq!(
            engine.register(&first, &actor(candidate)).await,
            Ok(Registration::Existing(actor("alice")))
        );
    }
    assert_eq!(
        engine.link(&second, &actor("legacy")).await,
        Ok(LinkOutcome::Existing)
    );
    let expected = Stored {
        actors: actors(&[("alice", 1), ("legacy", 1)]),
        bindings: bindings(&[
            ("issuer", "first", "alice", T0),
            ("issuer", "second", "legacy", T0),
        ]),
    };
    assert_eq!(stored(&database).await, expected);
    drop((lossy, engine, database));
    let database = open(&run, Intent::Serve, FixedClock::new(T0 + 2)).await;
    assert_eq!(stored(&database).await, expected);
}

#[tokio::test]
async fn a_lost_acknowledgement_without_effect_leaves_no_orphan_and_replay_creates_once() {
    let run = TestRun::new("identity-turso-unknown-rolled-back");
    let clock = FixedClock::new(T0);
    let database = open(&run, Intent::Migrate, clock.clone()).await;
    let lossy = TursoIdentityRegistry::with_acknowledgement(
        database.clone(),
        Acknowledgement::LostAfterRollback,
    );
    let engine = TursoIdentityRegistry::new(database.clone());
    let first = key("issuer", "first");
    let second = key("issuer", "second");
    seed(&database, "legacy", true).await;
    assert_eq!(
        lossy.register(&first, &actor("alice")).await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(
        lossy.link(&second, &actor("legacy")).await,
        Err(RegistryError::Unavailable)
    );
    assert_eq!(engine.lookup(&first).await, Ok(None));
    assert_eq!(engine.lookup(&second).await, Ok(None));
    assert_eq!(
        stored(&database).await,
        Stored {
            actors: actors(&[("legacy", 1)]),
            bindings: Vec::new(),
        }
    );
    clock.set(T0 + 1);
    assert_eq!(
        engine.register(&first, &actor("alice")).await,
        Ok(Registration::Created(actor("alice")))
    );
    assert_eq!(
        engine.register(&first, &actor("alice")).await,
        Ok(Registration::Existing(actor("alice")))
    );
    assert_eq!(
        engine.link(&second, &actor("legacy")).await,
        Ok(LinkOutcome::Linked)
    );
    assert_eq!(
        stored(&database).await,
        Stored {
            actors: actors(&[("alice", 1), ("legacy", 1)]),
            bindings: bindings(&[
                ("issuer", "first", "alice", T0 + 1),
                ("issuer", "second", "legacy", T0 + 1)
            ]),
        }
    );
}

/// Production hydration receives malformed `SELECT` projections from the real engine; no table
/// constraint, trigger or foreign-key setting is weakened to construct a row.
async fn projection(database: &ServerDatabase, values: Vec<Value>) -> Row {
    let columns = (1..=values.len())
        .map(|index| format!("?{index}"))
        .collect::<Vec<_>>()
        .join(",");
    let reader = database.owner().reader().await.unwrap();
    let mut rows = reader
        .query(&format!("SELECT {columns}"), params_from_iter(values))
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    assert!(rows.next().await.unwrap().is_none());
    row
}

#[tokio::test]
async fn corrupt_projections_are_refused_by_the_production_decoders() {
    let run = TestRun::new("identity-turso-decoders");
    let database = open(&run, Intent::Migrate, FixedClock::new(T0)).await;
    let text = |value: &str| Value::Text(value.into());
    assert_eq!(
        binding_row(
            projection(
                &database,
                vec![text("alice"), text("alice"), Value::Integer(1)]
            )
            .await
        ),
        Ok(record("alice", true))
    );
    assert_eq!(
        actor_row(projection(&database, vec![text("alice"), Value::Integer(0)]).await),
        Ok(record("alice", false))
    );
    for values in [
        vec![Value::Null, text("alice"), Value::Integer(1)],
        // A binding whose actor row is missing is corrupt, not unbound.
        vec![text("alice"), Value::Null, Value::Null],
        vec![text("alice"), text("bob"), Value::Integer(1)],
        vec![text("Alice"), text("Alice"), Value::Integer(1)],
        vec![text("alice"), text("alice"), Value::Integer(2)],
        vec![text("alice"), text("alice"), Value::Integer(-1)],
        vec![text("alice"), text("alice"), text("1")],
        vec![text("alice"), text("alice"), Value::Real(1.0)],
        vec![text("alice"), text("alice"), Value::Null],
        vec![Value::Integer(1), text("alice"), Value::Integer(1)],
        vec![
            Value::Blob(b"alice".to_vec()),
            text("alice"),
            Value::Integer(1),
        ],
        vec![text("alice"), text("alice")],
    ] {
        assert_eq!(
            binding_row(projection(&database, values.clone()).await),
            Err(RegistryError::CorruptRecord),
            "{values:?}"
        );
    }
    for values in [
        vec![text(""), Value::Integer(1)],
        vec![text("a b"), Value::Integer(1)],
        vec![text(&"a".repeat(65)), Value::Integer(1)],
        vec![text("alice"), Value::Integer(7)],
        vec![Value::Null, Value::Integer(1)],
        vec![text("alice")],
    ] {
        assert_eq!(
            actor_row(projection(&database, values.clone()).await),
            Err(RegistryError::CorruptRecord),
            "{values:?}"
        );
    }
    let reader = database.owner().reader().await.unwrap();
    let rows = reader
        .query("SELECT 'alice',1 UNION ALL SELECT 'alice',1", ())
        .await
        .unwrap();
    assert_eq!(
        at_most_one(rows, actor_row).await,
        Err(RegistryError::CorruptRecord)
    );
}

#[test]
fn engine_corruption_is_never_served_and_other_faults_are_unavailable() {
    for fault in [StoreFault::Corrupt, StoreFault::NotDatabase] {
        assert_eq!(storage(fault), RegistryError::CorruptRecord);
    }
    for fault in [
        StoreFault::Busy,
        StoreFault::StaleSnapshot,
        StoreFault::Io(std::io::ErrorKind::TimedOut),
        StoreFault::Interrupted,
        StoreFault::DatabaseFull,
        StoreFault::ReadOnly,
        StoreFault::Engine,
        StoreFault::Clock,
    ] {
        assert_eq!(storage(fault), RegistryError::Unavailable);
    }
}
