//! File-backed behavior of `OwnedDatabase` against the real pinned engine: creation and
//! refusal, ownership, transactions, interleavings, typed outcomes and verified backup.
//! Every database lives in a run-owned private directory.

use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cantos_store_turso::clock::{Clock, ClockFault};
use cantos_store_turso::error::{
    CommitFault, ConstraintKind, DirectoryFault, GuardCode, OpenError, StoreFault,
};
use cantos_store_turso::registry::{
    DatabaseKind, Intent, Migration, Refusal, Registry, HISTORY_TABLE,
};
use cantos_store_turso::testing::{FixedClock, TestRun};
use cantos_store_turso::value::UnixMicros;
use cantos_store_turso::{
    params_from_iter, BackupError, OpenRequest, OwnedDatabase, Value, DEFAULT_BUSY_TIMEOUT,
};

const ITEMS: Migration = Migration {
    version: 1,
    name: "create_items",
    sql: "CREATE TABLE items(id INTEGER PRIMARY KEY, label TEXT NOT NULL) STRICT;",
};
const LABELS: Migration = Migration {
    version: 2,
    name: "create_labels",
    sql: "CREATE TABLE labels(id INTEGER PRIMARY KEY, item INTEGER NOT NULL REFERENCES items(id)) STRICT;",
};
const BROKEN_SECOND: Migration = Migration {
    version: 2,
    name: "create_labels",
    sql:
        "CREATE TABLE labels(id INTEGER PRIMARY KEY); CREATE TABLE labels(id INTEGER PRIMARY KEY);",
};
const ITEMS_ALTERED: Migration = Migration {
    version: 1,
    name: "create_items",
    sql: "CREATE TABLE items(id INTEGER PRIMARY KEY, label TEXT) STRICT;",
};
const NEVER_COMMITS: Migration = Migration {
    version: 1,
    name: "create_items",
    sql: "CREATE TABLE items(id INTEGER PRIMARY KEY) STRICT; COMMIT;",
};

static ONE: [Migration; 1] = [ITEMS];
static TWO: [Migration; 2] = [ITEMS, LABELS];
static BROKEN: [Migration; 2] = [ITEMS, BROKEN_SECOND];
static ALTERED: [Migration; 1] = [ITEMS_ALTERED];

const FIXTURE: [Migration; 1] = [Migration {
    version: 1,
    name: "guards_and_cycles",
    sql: "\
        CREATE TABLE settled(id INTEGER PRIMARY KEY, v TEXT NOT NULL) STRICT; \
        CREATE TRIGGER settled_no_update BEFORE UPDATE ON settled BEGIN SELECT RAISE(ABORT, 'cantos:immutable.settled'); END; \
        CREATE TABLE head(id INTEGER PRIMARY KEY, rev INTEGER REFERENCES rev(id) DEFERRABLE INITIALLY DEFERRED) STRICT; \
        CREATE TABLE rev(id INTEGER PRIMARY KEY, head INTEGER NOT NULL REFERENCES head(id) DEFERRABLE INITIALLY DEFERRED) STRICT; \
        CREATE TABLE counter(id INTEGER PRIMARY KEY CHECK (id = 1), n INTEGER NOT NULL) STRICT; \
        INSERT INTO counter VALUES (1, 0); \
        CREATE TABLE ledger(id INTEGER PRIMARY KEY, at INTEGER NOT NULL) STRICT;",
}];

fn registry(kind: DatabaseKind, epoch: u32, migrations: &'static [Migration]) -> Registry {
    Registry::new(kind, epoch, migrations).expect("valid test registry")
}

fn server(migrations: &'static [Migration]) -> Registry {
    registry(DatabaseKind::Server, 1, migrations)
}

fn request(
    run: &TestRun,
    registry: Registry,
    intent: Intent,
    clock: Arc<dyn Clock>,
) -> OpenRequest {
    OpenRequest::new(run.path("server.db"), registry, intent, clock)
}

async fn create(run: &TestRun, registry: Registry) -> OwnedDatabase {
    let clock = FixedClock::new(1_700_000_000_000_000);
    OwnedDatabase::open(request(run, registry, Intent::Migrate, clock))
        .await
        .expect("the file is created")
}

async fn integer(db: &OwnedDatabase, sql: &str) -> i64 {
    let reader = db.reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let row = rows.next().await.unwrap().expect("a row");
    let Value::Integer(value) = row.get_value(0).unwrap() else {
        panic!("not an integer: {sql}");
    };
    while rows.next().await.unwrap().is_some() {}
    value
}

/// Open the file directly (no owner lock), for tests that tamper with or inspect it while no
/// `OwnedDatabase` holds it.
async fn raw(path: &Path) -> turso::Connection {
    let database = turso::Builder::new_local(&path.to_string_lossy())
        .build()
        .await
        .unwrap();
    let conn = database.connect().unwrap();
    // The Database must outlive the connection for the whole test; leaking it is deliberate.
    std::mem::forget(database);
    conn
}

async fn raw_integer(conn: &turso::Connection, sql: &str) -> i64 {
    let mut rows = conn.query(sql, ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let Value::Integer(value) = row.get_value(0).unwrap() else {
        panic!("not an integer")
    };
    while rows.next().await.unwrap().is_some() {}
    value
}

#[tokio::test]
async fn creation_marks_the_header_and_records_every_migration_with_the_trusted_time() {
    let run = TestRun::new("create");
    let db = create(&run, server(&TWO)).await;
    assert_eq!(
        integer(&db, "PRAGMA application_id").await,
        i64::from(DatabaseKind::Server.application_id())
    );
    assert_eq!(integer(&db, "PRAGMA user_version").await, 1);
    assert_eq!(
        integer(&db, &format!("SELECT count(*) FROM {HISTORY_TABLE}")).await,
        2
    );
    assert_eq!(
        integer(
            &db,
            &format!("SELECT count(*) FROM {HISTORY_TABLE} WHERE applied_at = 1700000000000000")
        )
        .await,
        2
    );
    assert_eq!(integer(&db, "SELECT count(*) FROM items").await, 0);
    drop(db);
    let again = OwnedDatabase::open(request(
        &run,
        server(&TWO),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .expect("a current file serves");
    assert_eq!(again.registry().latest(), 2);
}

#[tokio::test]
async fn a_failed_creation_leaves_an_empty_unmarked_file_that_can_be_created_again() {
    let run = TestRun::new("create_fail");
    let failure = OwnedDatabase::open(request(
        &run,
        server(&BROKEN),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .err()
    .expect("the second migration is invalid");
    assert!(
        matches!(failure, OpenError::MigrationFailed { version: 2, .. }),
        "{failure:?}"
    );
    {
        let conn = raw(&run.path("server.db")).await;
        assert_eq!(raw_integer(&conn, "PRAGMA application_id").await, 0);
        assert_eq!(
            raw_integer(&conn, "SELECT count(*) FROM sqlite_schema").await,
            0
        );
    }
    let db = create(&run, server(&TWO)).await;
    assert_eq!(
        integer(&db, &format!("SELECT count(*) FROM {HISTORY_TABLE}")).await,
        2
    );
}

#[tokio::test]
async fn migration_sql_that_ends_the_runner_transaction_is_refused_at_registry_construction() {
    static COMMITS: [Migration; 1] = [NEVER_COMMITS];
    assert!(Registry::new(DatabaseKind::Server, 1, &COMMITS).is_err());
}

#[tokio::test]
async fn serving_refuses_pending_migrations_and_migrating_applies_only_the_suffix() {
    let run = TestRun::new("upgrade");
    drop(create(&run, server(&ONE)).await);
    let refused = OwnedDatabase::open(request(
        &run,
        server(&TWO),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(
        refused,
        OpenError::Refused(Refusal::PendingMigrations {
            applied: 1,
            latest: 2
        })
    );
    let clock = FixedClock::new(1_800_000_000_000_000);
    let db = OwnedDatabase::open(request(&run, server(&TWO), Intent::Migrate, clock))
        .await
        .unwrap();
    assert_eq!(
        integer(&db, &format!("SELECT count(*) FROM {HISTORY_TABLE}")).await,
        2
    );
    // The first row keeps its original time; only the new migration carries the new one.
    assert_eq!(
        integer(
            &db,
            &format!("SELECT applied_at FROM {HISTORY_TABLE} WHERE version = 1")
        )
        .await,
        1_700_000_000_000_000
    );
    assert_eq!(
        integer(
            &db,
            &format!("SELECT applied_at FROM {HISTORY_TABLE} WHERE version = 2")
        )
        .await,
        1_800_000_000_000_000
    );
}

#[tokio::test]
async fn a_failed_upgrade_rolls_back_only_its_own_migration() {
    let run = TestRun::new("upgrade_fail");
    drop(create(&run, server(&ONE)).await);
    let failure = OwnedDatabase::open(request(
        &run,
        server(&BROKEN),
        Intent::Migrate,
        FixedClock::new(5),
    ))
    .await
    .err()
    .unwrap();
    assert!(
        matches!(failure, OpenError::MigrationFailed { version: 2, .. }),
        "{failure:?}"
    );
    let db = OwnedDatabase::open(request(
        &run,
        server(&ONE),
        Intent::Serve,
        FixedClock::new(5),
    ))
    .await
    .expect("the earlier schema is untouched and still current for its own binary");
    assert_eq!(
        integer(&db, &format!("SELECT count(*) FROM {HISTORY_TABLE}")).await,
        1
    );
    assert_eq!(
        integer(
            &db,
            "SELECT count(*) FROM sqlite_schema WHERE name = 'labels'"
        )
        .await,
        0
    );
}

#[tokio::test]
async fn files_that_are_not_ours_are_refused_without_being_written() {
    // Not a database at all.
    let run = TestRun::new("refuse_garbage");
    std::fs::write(run.path("server.db"), vec![0x41_u8; 4096]).unwrap();
    let garbage = OwnedDatabase::open(request(
        &run,
        server(&ONE),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(garbage, OpenError::Store(StoreFault::NotDatabase));
    assert_eq!(
        std::fs::read(run.path("server.db")).unwrap(),
        vec![0x41_u8; 4096]
    );

    // A database that has tables but no Cantos marker is never adopted.
    let run = TestRun::new("refuse_unmarked");
    {
        let conn = raw(&run.path("server.db")).await;
        conn.execute("CREATE TABLE stranger(x INTEGER) STRICT", ())
            .await
            .unwrap();
    }
    for intent in [Intent::Migrate, Intent::Serve] {
        let error = OwnedDatabase::open(request(&run, server(&ONE), intent, FixedClock::new(1)))
            .await
            .err()
            .unwrap();
        assert_eq!(error, OpenError::Refused(Refusal::UnmarkedNonEmpty));
    }
    let conn = raw(&run.path("server.db")).await;
    assert_eq!(raw_integer(&conn, "PRAGMA application_id").await, 0);
    assert_eq!(
        raw_integer(&conn, "SELECT count(*) FROM sqlite_schema").await,
        1
    );
}

#[tokio::test]
async fn another_services_file_a_different_epoch_and_a_newer_history_are_refused() {
    let run = TestRun::new("refuse_marked");
    drop(create(&run, registry(DatabaseKind::Identity, 1, &TWO)).await);
    let wrong_kind = OwnedDatabase::open(request(
        &run,
        server(&TWO),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(
        wrong_kind,
        OpenError::Refused(Refusal::ForeignApplication {
            found: DatabaseKind::Identity.application_id()
        })
    );

    let run = TestRun::new("refuse_epoch");
    drop(create(&run, server(&TWO)).await);
    let wrong_epoch = OwnedDatabase::open(request(
        &run,
        registry(DatabaseKind::Server, 2, &TWO),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(
        wrong_epoch,
        OpenError::Refused(Refusal::EpochMismatch {
            found: 1,
            expected: 2
        })
    );

    let newer = OwnedDatabase::open(request(
        &run,
        server(&ONE),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(
        newer,
        OpenError::Refused(Refusal::NewerThanBinary {
            applied: 2,
            known: 1
        })
    );
}

#[tokio::test]
async fn a_changed_migration_or_a_tampered_history_is_detected() {
    let run = TestRun::new("diverge");
    drop(create(&run, server(&ONE)).await);
    let edited = OwnedDatabase::open(request(
        &run,
        server(&ALTERED),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(
        edited,
        OpenError::Refused(Refusal::HistoryDiverged { version: 1 })
    );

    // The history table is append-only while the guard exists; remove it the way an attacker
    // with file access would, forge the checksum, and the next open still refuses.
    {
        let conn = raw(&run.path("server.db")).await;
        let blocked = conn
            .execute(&format!("UPDATE {HISTORY_TABLE} SET name = 'x'"), ())
            .await
            .map_err(StoreFault::from)
            .err();
        assert_eq!(
            blocked,
            Some(StoreFault::Guard(
                GuardCode::new("immutable.schema_history").unwrap()
            ))
        );
        conn.execute("DROP TRIGGER cantos_schema_history_no_update", ())
            .await
            .unwrap();
        conn.execute(
            &format!("UPDATE {HISTORY_TABLE} SET checksum = zeroblob(32)"),
            (),
        )
        .await
        .unwrap();
    }
    let forged = OwnedDatabase::open(request(
        &run,
        server(&ONE),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(
        forged,
        OpenError::Refused(Refusal::HistoryDiverged { version: 1 })
    );
}

#[tokio::test]
async fn one_owner_per_file_and_a_private_directory() {
    let run = TestRun::new("ownership");
    let first = create(&run, server(&ONE)).await;
    let second = OwnedDatabase::open(request(
        &run,
        server(&ONE),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(second, OpenError::AlreadyOwned);
    drop(first);
    OwnedDatabase::open(request(
        &run,
        server(&ONE),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .expect("ownership is released when the owner closes");

    let missing = OwnedDatabase::open(OpenRequest::new(
        run.path("absent-dir/server.db"),
        server(&ONE),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .err()
    .unwrap();
    assert_eq!(missing, OpenError::Directory(DirectoryFault::Missing));

    #[cfg(unix)]
    {
        use std::os::unix::fs::{symlink, PermissionsExt};
        let shared = run.path("shared");
        std::fs::create_dir(&shared).unwrap();
        std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
        let open = OwnedDatabase::open(OpenRequest::new(
            shared.join("server.db"),
            server(&ONE),
            Intent::Migrate,
            FixedClock::new(1),
        ))
        .await
        .err()
        .unwrap();
        assert_eq!(open, OpenError::Directory(DirectoryFault::TooPermissive));
        assert!(
            !shared.join("server.db").exists(),
            "nothing is created in a shared directory"
        );

        symlink(run.path("elsewhere.db"), run.path("link.db")).unwrap();
        let linked = OwnedDatabase::open(OpenRequest::new(
            run.path("link.db"),
            server(&ONE),
            Intent::Migrate,
            FixedClock::new(1),
        ))
        .await
        .err()
        .unwrap();
        assert_eq!(linked, OpenError::Directory(DirectoryFault::SymlinkRefused));
        assert!(!run.path("elsewhere.db").exists());
    }
}

#[tokio::test]
async fn every_connection_enforces_foreign_keys_and_readers_cannot_write() {
    let run = TestRun::new("fk_readers");
    let db = create(&run, server(&TWO)).await;
    let tx = db.begin_write().await.unwrap();
    let dangling = tx
        .execute("INSERT INTO labels VALUES (1, 99)", ())
        .await
        .unwrap_err();
    assert_eq!(
        dangling,
        StoreFault::Constraint {
            kind: ConstraintKind::ForeignKey,
            target: None
        }
    );
    tx.rollback().await.unwrap();
    assert_eq!(integer(&db, "SELECT count(*) FROM labels").await, 0);

    let reader = db.reader().await.unwrap();
    let write = reader.query("INSERT INTO items VALUES (1, 'x')", ()).await;
    assert!(write.is_err(), "a read-only connection executed a write");
    assert_eq!(integer(&db, "SELECT count(*) FROM items").await, 0);
}

#[tokio::test]
async fn commit_publishes_atomically_rollback_discards_and_readers_keep_their_snapshot() {
    let run = TestRun::new("commit_rollback");
    let db = create(&run, server(&ONE)).await;
    let tx = db.begin_write().await.unwrap();
    tx.execute("INSERT INTO items VALUES (1, 'a')", ())
        .await
        .unwrap();
    tx.execute("INSERT INTO items VALUES (2, 'b')", ())
        .await
        .unwrap();
    assert_eq!(
        integer(&db, "SELECT count(*) FROM items").await,
        0,
        "uncommitted rows are invisible"
    );
    tx.commit().await.unwrap();
    assert_eq!(integer(&db, "SELECT count(*) FROM items").await, 2);

    let tx = db.begin_write().await.unwrap();
    tx.execute("DELETE FROM items", ()).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(integer(&db, "SELECT count(*) FROM items").await, 2);
}

#[tokio::test]
async fn a_dropped_transaction_is_rolled_back_before_the_next_one_begins() {
    let run = TestRun::new("dropped");
    let db = create(&run, server(&ONE)).await;
    {
        let tx = db.begin_write().await.unwrap();
        tx.execute("INSERT INTO items VALUES (1, 'lost')", ())
            .await
            .unwrap();
        // Dropped without commit or rollback, as a cancelled request would.
    }
    let tx = db.begin_write().await.unwrap();
    assert_eq!(
        tx.query("SELECT count(*) FROM items", ())
            .await
            .unwrap()
            .next()
            .await
            .unwrap()
            .unwrap()
            .get_value(0)
            .unwrap(),
        Value::Integer(0)
    );
    tx.commit().await.unwrap();
    assert_eq!(integer(&db, "SELECT count(*) FROM items").await, 0);
}

#[tokio::test]
async fn guards_arrive_typed_and_a_deferred_foreign_key_failure_is_a_rejected_commit() {
    let run = TestRun::new("guards");
    let db = create(&run, registry(DatabaseKind::Server, 1, &FIXTURE)).await;
    let tx = db.begin_write().await.unwrap();
    tx.execute("INSERT INTO settled VALUES (1, 'a')", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    let tx = db.begin_write().await.unwrap();
    let update = tx
        .execute("UPDATE settled SET v = 'b'", ())
        .await
        .unwrap_err();
    assert_eq!(
        update,
        StoreFault::Guard(GuardCode::new("immutable.settled").unwrap())
    );
    tx.rollback().await.unwrap();

    // The head <-> revision cycle commits when both sides arrive in one transaction.
    let tx = db.begin_write().await.unwrap();
    tx.execute("INSERT INTO head VALUES (1, 1)", ())
        .await
        .unwrap();
    tx.execute("INSERT INTO rev VALUES (1, 1)", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();

    // A dangling deferred reference is only seen at COMMIT, which must be a definite rejection.
    let tx = db.begin_write().await.unwrap();
    tx.execute("INSERT INTO head VALUES (2, 77)", ())
        .await
        .unwrap();
    let rejected = tx.commit().await.unwrap_err();
    assert_eq!(
        rejected,
        CommitFault::Rejected(StoreFault::Constraint {
            kind: ConstraintKind::ForeignKey,
            target: None
        })
    );
    assert_eq!(integer(&db, "SELECT count(*) FROM head").await, 1);
    // Rejected means rolled back at once: a foreign writer can take the lock without waiting.
    let foreign = raw(&run.path("server.db")).await;
    foreign
        .execute("BEGIN IMMEDIATE", ())
        .await
        .expect("the rejected transaction released the write lock");
    foreign.execute("ROLLBACK", ()).await.unwrap();
    let next = db
        .begin_write()
        .await
        .expect("the writer is reusable after a rejected commit");
    next.rollback().await.unwrap();
}

struct FailsOnce {
    failures: std::sync::atomic::AtomicU32,
}

impl Clock for FailsOnce {
    fn now(&self) -> Result<UnixMicros, ClockFault> {
        use std::sync::atomic::Ordering;
        if self
            .failures
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |n| n.checked_sub(1))
            .is_ok()
        {
            return Err(ClockFault);
        }
        Ok(UnixMicros::new(9).unwrap())
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn the_clock_is_read_after_the_writer_is_held_and_a_clock_fault_begins_nothing() {
    let run = TestRun::new("clock");
    let clock = FixedClock::new(100);
    let db = Arc::new(
        OwnedDatabase::open(request(
            &run,
            registry(DatabaseKind::Server, 1, &FIXTURE),
            Intent::Migrate,
            clock.clone(),
        ))
        .await
        .unwrap(),
    );
    let first = db.begin_write().await.unwrap();
    assert_eq!(first.now().get(), 100);
    let waiting = {
        let db = db.clone();
        tokio::spawn(async move { db.begin_write().await.map(|tx| tx.now().get()) })
    };
    tokio::time::sleep(Duration::from_millis(50)).await;
    assert!(
        !waiting.is_finished(),
        "a second writer waits for the first"
    );
    clock.set(200);
    first.commit().await.unwrap();
    assert_eq!(
        waiting.await.unwrap().unwrap(),
        200,
        "time is taken once the writer is ours"
    );

    let failing = Arc::new(FailsOnce { failures: 1.into() });
    let run = TestRun::new("clock_fault");
    let db = OwnedDatabase::open(request(
        &run,
        registry(DatabaseKind::Server, 1, &FIXTURE),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .unwrap();
    drop(db);
    let db = OwnedDatabase::open(request(
        &run,
        registry(DatabaseKind::Server, 1, &FIXTURE),
        Intent::Serve,
        failing,
    ))
    .await
    .unwrap();
    assert_eq!(db.begin_write().await.err(), Some(StoreFault::Clock));
    db.begin_write()
        .await
        .expect("no transaction was left open by the clock fault")
        .rollback()
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_read_modify_write_transactions_are_serialized_without_lost_updates() {
    let run = TestRun::new("serialized");
    let db = Arc::new(create(&run, registry(DatabaseKind::Server, 1, &FIXTURE)).await);
    let mut tasks = Vec::new();
    for _ in 0..16 {
        let db = db.clone();
        tasks.push(tokio::spawn(async move {
            for _ in 0..10 {
                let tx = db.begin_write().await.unwrap();
                let mut rows = tx
                    .query("SELECT n FROM counter WHERE id = 1", ())
                    .await
                    .unwrap();
                let Value::Integer(n) = rows.next().await.unwrap().unwrap().get_value(0).unwrap()
                else {
                    panic!("counter is not an integer")
                };
                while rows.next().await.unwrap().is_some() {}
                drop(rows);
                tokio::task::yield_now().await;
                tx.execute(
                    "UPDATE counter SET n = ?1 WHERE id = 1",
                    params_from_iter([Value::Integer(n + 1)]),
                )
                .await
                .unwrap();
                tx.commit().await.unwrap();
            }
        }));
    }
    for task in tasks {
        task.await.unwrap();
    }
    assert_eq!(integer(&db, "SELECT n FROM counter").await, 160);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn a_foreign_writer_makes_begin_write_busy_after_the_timeout_and_changes_nothing() {
    let run = TestRun::new("busy");
    let mut open = request(
        &run,
        registry(DatabaseKind::Server, 1, &FIXTURE),
        Intent::Migrate,
        FixedClock::new(1),
    );
    open.busy_timeout = Duration::from_millis(150);
    let db = OwnedDatabase::open(open).await.unwrap();
    // A second engine handle in the same process stands in for any writer outside the owner.
    let foreign = raw(&run.path("server.db")).await;
    foreign.execute("BEGIN IMMEDIATE", ()).await.unwrap();
    let started = Instant::now();
    let busy = db.begin_write().await.err();
    assert_eq!(busy, Some(StoreFault::Busy));
    assert!(
        started.elapsed() >= Duration::from_millis(100),
        "{:?}",
        started.elapsed()
    );
    foreign.execute("ROLLBACK", ()).await.unwrap();
    db.begin_write()
        .await
        .expect("free once the foreign writer left")
        .rollback()
        .await
        .unwrap();
    assert_eq!(integer(&db, "SELECT count(*) FROM ledger").await, 0);
    assert_eq!(DEFAULT_BUSY_TIMEOUT, Duration::from_secs(3));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backup_is_verified_checksummed_restorable_and_never_overwrites() {
    let run = TestRun::new("backup");
    let db = Arc::new(create(&run, registry(DatabaseKind::Server, 1, &FIXTURE)).await);

    // Commit ledger rows continuously while the backup runs; each row's id is the previous + 1.
    let writer = {
        let db = db.clone();
        tokio::spawn(async move {
            for id in 1_i64..=400 {
                let tx = db.begin_write().await.unwrap();
                tx.execute(
                    "INSERT INTO ledger VALUES (?1, ?2)",
                    params_from_iter([Value::Integer(id), Value::Integer(tx.now().get())]),
                )
                .await
                .unwrap();
                tx.commit().await.unwrap();
                tokio::task::yield_now().await;
            }
        })
    };
    while integer(&db, "SELECT count(*) FROM ledger").await < 40 {
        tokio::time::sleep(Duration::from_millis(2)).await;
    }
    let before = integer(&db, "SELECT count(*) FROM ledger").await;
    let target = run.path("backup.db");
    let receipt = db
        .backup_to(&target)
        .await
        .expect("the backup succeeds under load");
    writer.await.unwrap();

    assert_eq!(receipt.bytes, std::fs::metadata(&target).unwrap().len());
    assert_ne!(receipt.sha256, [0; 32]);
    assert_eq!(
        db.backup_to(&target).await.unwrap_err(),
        BackupError::DestinationExists
    );
    assert_eq!(
        db.backup_to(db.path()).await.unwrap_err(),
        BackupError::DestinationExists
    );

    // Restore: the copy opens as a current file of the same service, with a gap-free ledger.
    drop(db);
    let restored_dir = TestRun::new("backup_restored");
    std::fs::copy(&target, restored_dir.path("server.db")).unwrap();
    let restored = OwnedDatabase::open(OpenRequest::new(
        restored_dir.path("server.db"),
        registry(DatabaseKind::Server, 1, &FIXTURE),
        Intent::Serve,
        FixedClock::new(1),
    ))
    .await
    .expect("a verified backup restores as a current file");
    let rows = integer(&restored, "SELECT count(*) FROM ledger").await;
    let max = integer(&restored, "SELECT max(id) FROM ledger").await;
    assert!(rows >= before, "{rows} < {before}");
    assert_eq!(
        rows, max,
        "the copy is transaction-consistent: ids are contiguous"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_backup_destination_in_a_shared_directory_is_refused_before_any_copy() {
    use std::os::unix::fs::PermissionsExt;
    let run = TestRun::new("backup_shared");
    let db = create(&run, server(&ONE)).await;
    let shared = run.path("shared");
    std::fs::create_dir(&shared).unwrap();
    std::fs::set_permissions(&shared, std::fs::Permissions::from_mode(0o755)).unwrap();
    let error = db.backup_to(&shared.join("copy.db")).await.unwrap_err();
    assert_eq!(
        error,
        BackupError::Destination(OpenError::Directory(DirectoryFault::TooPermissive))
    );
    assert!(!shared.join("copy.db").exists());
}

#[tokio::test]
async fn a_writer_wait_is_bounded_and_the_queue_recovers_when_the_holder_finishes() {
    let run = TestRun::new("bounded-writer");
    let db = create(&run, Registry::new(DatabaseKind::Server, 1, &ONE).unwrap()).await;
    let held = db.begin_write().await.unwrap();
    let started = Instant::now();
    let refused = db.begin_write_within(Duration::from_millis(80)).await;
    assert!(matches!(refused, Err(StoreFault::Busy)));
    assert!(started.elapsed() >= Duration::from_millis(80));
    held.rollback().await.unwrap();
    let tx = db
        .begin_write_within(Duration::from_millis(80))
        .await
        .expect("the writer is free again");
    tx.execute("INSERT INTO items(label) VALUES ('after')", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(integer(&db, "SELECT count(*) FROM items").await, 1);
}

async fn count_items(reader: &cantos_store_turso::ReadConnection) -> i64 {
    let mut rows = reader
        .query("SELECT count(*) FROM items", ())
        .await
        .unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let Value::Integer(n) = row.get_value(0).unwrap() else {
        panic!("not an integer")
    };
    while rows.next().await.unwrap().is_some() {}
    n
}

#[tokio::test]
async fn a_pinned_read_snapshot_ignores_commits_made_after_it_began() {
    let run = TestRun::new("read-snapshot");
    let db = create(&run, Registry::new(DatabaseKind::Server, 1, &ONE).unwrap()).await;
    let reader = db.reader().await.unwrap();
    reader.begin_snapshot().await.unwrap();
    assert_eq!(count_items(&reader).await, 0);
    let tx = db.begin_write().await.unwrap();
    tx.execute("INSERT INTO items(label) VALUES ('later')", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(count_items(&reader).await, 0, "the snapshot is pinned");
    reader.end_snapshot().await.unwrap();
    assert_eq!(count_items(&reader).await, 1, "a new read sees the commit");
    // Ending twice is harmless, and a reader still cannot write.
    reader.end_snapshot().await.unwrap();
    assert!(reader
        .query("INSERT INTO items(label) VALUES ('x')", ())
        .await
        .is_err());
}
