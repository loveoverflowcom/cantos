//! Real pinned-engine owner tests. These are not HTTP or production recovery acceptance.

use std::{fs, path::Path};

use cantos_store_turso::{
    error::{OpenError, StoreFault},
    recovery::{RecoveryBarrier, RecoveryFault, RECOVERY_SCHEMA},
    registry::{Intent, Migration, Registry},
    testing::{FixedClock, TestRun, TEST_APPLICATION_ID},
    OpenRequest, OwnedDatabase, Value,
};

const MIGRATIONS: [Migration; 2] = [
    Migration {
        version: 1,
        name: "state",
        sql: "CREATE TABLE state(id INTEGER PRIMARY KEY, revoked INTEGER NOT NULL) STRICT;",
    },
    Migration {
        version: 2,
        name: "owner_recovery",
        sql: RECOVERY_SCHEMA,
    },
];

fn request(path: &Path, intent: Intent) -> OpenRequest {
    OpenRequest::new(
        path,
        Registry::new(TEST_APPLICATION_ID, 1, &MIGRATIONS).unwrap(),
        intent,
        FixedClock::new(1),
    )
}

fn private(path: &Path) {
    use std::os::unix::fs::DirBuilderExt;
    fs::DirBuilder::new().mode(0o700).create(path).unwrap();
}

fn fixture(label: &str) -> (TestRun, std::path::PathBuf, std::path::PathBuf) {
    let run = TestRun::new(label);
    private(&run.path("database"));
    private(&run.path("authority"));
    // macOS's inherited /var TMPDIR is an OS alias. The protocol takes canonical paths;
    // do not weaken its symlink rejection to accommodate a fixture path spelling.
    let root = run.dir().canonicalize().unwrap();
    let db = root.join("database/state.db");
    let fence = root.join("authority/current.fence");
    (run, db, fence)
}

async fn enrolled(db: &Path, fence: &Path) -> OwnedDatabase {
    OwnedDatabase::open(request(db, Intent::Migrate))
        .await
        .unwrap()
        .enroll_recovery_fence(fence)
        .await
        .unwrap()
}

async fn state(owner: &OwnedDatabase) -> i64 {
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query("SELECT revoked FROM state WHERE id=1", ())
        .await
        .unwrap();
    match rows.next().await.unwrap().unwrap().get_value(0).unwrap() {
        Value::Integer(value) => value,
        _ => panic!("typed fixture value"),
    }
}

fn recovery_open(result: Result<OwnedDatabase, OpenError>, expected: RecoveryFault) {
    assert!(
        matches!(result, Err(OpenError::Store(StoreFault::Recovery(value))) if value == expected)
    );
}

#[tokio::test]
async fn exact_current_copy_reopens_but_stale_revocation_copy_is_refused() {
    let (run, db, fence) = fixture("recovery-stale");
    let owner = enrolled(&db, &fence).await;
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,0)", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    private(&run.path("stale"));
    let stale = run.path("stale/state.db");
    owner.backup_to(&stale).await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("UPDATE state SET revoked=1 WHERE id=1", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(state(&owner).await, 1);
    private(&run.path("current"));
    let current = run.path("current/state.db");
    owner.backup_to(&current).await.unwrap();
    drop(owner);
    recovery_open(
        OwnedDatabase::open(request(&stale, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Mismatch,
    );
    // Omitting the source configuration cannot bypass enrollment.
    recovery_open(
        OwnedDatabase::open(request(&stale, Intent::Migrate)).await,
        RecoveryFault::Mismatch,
    );
    let restored =
        OwnedDatabase::open(request(&current, Intent::Serve).with_recovery_fence(&fence))
            .await
            .unwrap();
    assert_eq!(state(&restored).await, 1);
    // A second copy cannot acquire the independently held fence lease.
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Untrusted,
    );
}

#[tokio::test]
async fn explicit_rollback_preserves_current_authority_and_normal_restart() {
    let (_run, db, fence) = fixture("recovery-rollback");
    let owner = enrolled(&db, &fence).await;
    let before = fs::read(&fence).unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,1)", ())
        .await
        .unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(fs::read(&fence).unwrap(), before);
    drop(owner);
    let owner = OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence))
        .await
        .unwrap();
    assert_eq!(fs::read(&fence).unwrap(), before);
    assert!(owner.reader().await.is_ok());
}

#[tokio::test]
async fn loss_of_owner_keeps_pending_even_when_engine_will_rollback() {
    let (_run, db, fence) = fixture("recovery-abandon");
    let owner = enrolled(&db, &fence).await;
    let reader = owner.reader().await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,0)", ())
        .await
        .unwrap();
    drop(tx);
    assert!(matches!(
        reader.query("SELECT 1", ()).await,
        Err(StoreFault::Recovery(RecoveryFault::Pending))
    ));
    drop(reader);
    drop(owner);
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Pending,
    );
}

#[tokio::test]
async fn same_owner_proves_cancelled_write_rollback_without_leaving_service_closed() {
    let (_run, db, fence) = fixture("recovery-known-cancel");
    let owner = enrolled(&db, &fence).await;
    let initial = fs::read(&fence).unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,0)", ())
        .await
        .unwrap();
    drop(tx); // No COMMIT was attempted; this owner's in-memory ticket is still available.
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query("SELECT count(*) FROM state", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get_value(0).unwrap(),
        Value::Integer(0)
    );
    drop(rows);
    drop(reader);
    assert_eq!(fs::read(&fence).unwrap(), initial);
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,1)", ())
        .await
        .unwrap();
    drop(tx);
    owner.finish_pending_writes().await.unwrap(); // Graceful process drain retains the proof.
    drop(owner);
    let owner = OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence))
        .await
        .unwrap();
    assert!(owner.reader().await.is_ok());
}

#[tokio::test]
async fn live_owner_reads_committed_state_during_a_normal_write_without_treating_it_as_restore() {
    let (_run, db, fence) = fixture("recovery-live-read");
    let owner = enrolled(&db, &fence).await;
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,1)", ())
        .await
        .unwrap();
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query("SELECT count(*) FROM state", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get_value(0).unwrap(),
        Value::Integer(0)
    );
    drop(rows);
    drop(reader);
    tx.commit().await.unwrap();
    assert_eq!(state(&owner).await, 1);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_queries_keep_admission_during_commits_and_rollbacks() {
    use std::sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    };
    let (_run, db, fence) = fixture("recovery-concurrent-queries");
    let owner = Arc::new(enrolled(&db, &fence).await);
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,0)", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let start = Arc::new(tokio::sync::Barrier::new(4));
    let done = Arc::new(AtomicBool::new(false));
    let mut readers = Vec::new();
    for _ in 0..3 {
        let (owner, start, done) = (owner.clone(), start.clone(), done.clone());
        readers.push(tokio::spawn(async move {
            let reader = owner.reader().await.unwrap();
            start.wait().await;
            let mut count = 0;
            loop {
                let mut rows = reader
                    .query("SELECT revoked FROM state WHERE id=1", ())
                    .await
                    .unwrap();
                assert_eq!(
                    rows.next().await.unwrap().unwrap().get_value(0).unwrap(),
                    Value::Integer(0)
                );
                count += 1;
                if done.load(Ordering::Acquire) {
                    break;
                }
                tokio::task::yield_now().await;
            }
            count
        }));
    }
    start.wait().await;
    for index in 0..32 {
        let tx = owner.begin_write().await.unwrap();
        if index % 2 == 0 {
            tx.execute("UPDATE state SET revoked=0 WHERE id=1", ())
                .await
                .unwrap();
            tx.commit().await.unwrap();
        } else {
            tx.execute("UPDATE state SET revoked=1 WHERE id=1", ())
                .await
                .unwrap();
            tx.rollback().await.unwrap();
        }
    }
    done.store(true, Ordering::Release);
    for reader in readers {
        assert!(reader.await.unwrap() > 0);
    }
    assert_eq!(state(&owner).await, 0);
}

#[test]
#[ignore = "subprocess worker; invoked only by the bounded crash regression"]
fn recovery_crash_worker() {
    use std::io::Read;
    let root = std::path::PathBuf::from(
        std::env::var_os("CANTOS_RECOVERY_CRASH_TEST_ROOT").expect("private test root"),
    );
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    runtime.block_on(async {
        let db = root.join("database/state.db");
        let fence = root.join("authority/current.fence");
        let owner = OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence))
            .await
            .unwrap();
        let tx = owner.begin_write().await.unwrap();
        tx.execute("INSERT INTO state VALUES(1,1)", ())
            .await
            .unwrap();
        fs::write(root.join("worker-ready"), b"pending-no-commit").unwrap();
        // Parent SIGKILLs this process before COMMIT; destructors cannot retain RAM evidence.
        let mut stop = [0];
        let _ = std::io::stdin().read_exact(&mut stop);
        drop(tx);
    });
}

#[tokio::test]
async fn sigkill_before_commit_cannot_use_the_lost_same_owner_cancellation_witness() {
    use std::{
        process::{Child, Command, Stdio},
        time::{Duration, Instant},
    };
    struct ChildGuard(Child);
    impl Drop for ChildGuard {
        fn drop(&mut self) {
            if matches!(self.0.try_wait(), Ok(None)) {
                let _ = self.0.kill();
                let _ = self.0.wait();
            }
        }
    }
    let (run, db, fence) = fixture("recovery-crash-before-commit");
    let owner = enrolled(&db, &fence).await;
    drop(owner);
    let root = run.dir().canonicalize().unwrap();
    let mut child = ChildGuard(
        Command::new(std::env::current_exe().unwrap())
            .args([
                "--ignored",
                "--exact",
                "recovery_crash_worker",
                "--nocapture",
            ])
            .env("CANTOS_RECOVERY_CRASH_TEST_ROOT", &root)
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .spawn()
            .unwrap(),
    );
    let deadline = Instant::now() + Duration::from_secs(15);
    while !root.join("worker-ready").exists() {
        assert!(
            child.0.try_wait().unwrap().is_none(),
            "worker failed before the tested crash point"
        );
        assert!(
            Instant::now() < deadline,
            "worker did not reach the bounded crash point"
        );
        std::thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(
        fs::read(root.join("worker-ready")).unwrap(),
        b"pending-no-commit"
    );
    child.0.kill().unwrap();
    assert!(!child.0.wait().unwrap().success());
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Pending,
    );
    recovery_open(
        OwnedDatabase::reconcile_recovery_fence(
            request(&db, Intent::Serve).with_recovery_fence(&fence),
        )
        .await,
        RecoveryFault::Mismatch,
    );
}

#[tokio::test]
async fn missing_corrupt_old_or_public_authority_never_becomes_fresh() {
    use std::os::unix::fs::PermissionsExt;
    let (_run, db, fence) = fixture("recovery-bad-source");
    let owner = enrolled(&db, &fence).await;
    let initial = fs::read(&fence).unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,1)", ())
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let current = fs::read(&fence).unwrap();
    drop(owner);
    for (bytes, fault) in [
        (&initial, RecoveryFault::Mismatch),
        (&b"not authority".to_vec(), RecoveryFault::Malformed),
    ] {
        fs::write(&fence, bytes).unwrap();
        recovery_open(
            OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
            fault,
        );
    }
    fs::write(&fence, current).unwrap();
    fs::set_permissions(&fence, fs::Permissions::from_mode(0o644)).unwrap();
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Untrusted,
    );
    fs::remove_file(&fence).unwrap();
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Missing,
    );
}

#[tokio::test]
async fn source_inside_snapshot_root_and_symlinks_are_refused() {
    use std::os::unix::fs::symlink;
    let (run, db, fence) = fixture("recovery-placement");
    let owner = OwnedDatabase::open(request(&db, Intent::Migrate))
        .await
        .unwrap();
    assert!(matches!(
        owner
            .enroll_recovery_fence(&run.path("database/fence"))
            .await,
        Err(StoreFault::Recovery(RecoveryFault::Untrusted))
    ));
    let owner = enrolled(&db, &fence).await;
    drop(owner);
    let saved = run.path("authority/saved");
    fs::rename(&fence, &saved).unwrap();
    symlink(&saved, &fence).unwrap();
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Untrusted,
    );
}

#[tokio::test]
async fn cursor_cannot_outlive_the_fenced_write_authority() {
    let (_run, db, fence) = fixture("recovery-cursor");
    let owner = enrolled(&db, &fence).await;
    let tx = owner.begin_write().await.unwrap();
    let mut rows = tx.query("SELECT 1", ()).await.unwrap();
    drop(tx);
    assert!(matches!(
        rows.next().await,
        Err(StoreFault::Recovery(RecoveryFault::Pending))
    ));
}

#[tokio::test]
async fn lost_fence_ack_only_reconciles_exact_committed_next_state() {
    let (run, db, fence) = fixture("recovery-lost-ack");
    let owner = enrolled(&db, &fence).await;
    private(&run.path("old"));
    let old = run.path("old/state.db");
    owner.backup_to(&old).await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,1)", ())
        .await
        .unwrap();
    // Real engine COMMIT succeeds, but its independent authority path disappears before ACK.
    // Rename the actual directory, so this fault works for root and unprivileged runners.
    let parent = fence.parent().unwrap();
    let held = parent.with_file_name("held-authority");
    fs::rename(parent, &held).unwrap();
    let outcome = tx.commit().await;
    fs::rename(&held, parent).unwrap();
    assert!(matches!(
        outcome,
        Err(cantos_store_turso::error::CommitFault::Unknown(
            StoreFault::Recovery(RecoveryFault::Missing)
        ))
    ));
    assert!(matches!(
        owner.finish_pending_writes().await,
        Err(StoreFault::Recovery(RecoveryFault::Pending))
    ));
    drop(owner);
    recovery_open(
        OwnedDatabase::open(request(&db, Intent::Serve).with_recovery_fence(&fence)).await,
        RecoveryFault::Pending,
    );
    recovery_open(
        OwnedDatabase::reconcile_recovery_fence(
            request(&old, Intent::Serve).with_recovery_fence(&fence),
        )
        .await,
        RecoveryFault::Mismatch,
    );
    let current = OwnedDatabase::reconcile_recovery_fence(
        request(&db, Intent::Serve).with_recovery_fence(&fence),
    )
    .await
    .unwrap();
    assert_eq!(state(&current).await, 1);
    let reconciled = fs::read(&fence).unwrap();
    drop(current);
    let replay = OwnedDatabase::reconcile_recovery_fence(
        request(&db, Intent::Serve).with_recovery_fence(&fence),
    )
    .await
    .unwrap();
    assert_eq!(state(&replay).await, 1);
    assert_eq!(fs::read(&fence).unwrap(), reconciled);
}

#[tokio::test]
async fn pair_barrier_closes_existing_readers_cursors_writers_and_normal_reopen() {
    use std::os::unix::fs::MetadataExt;
    let (run, db, fence) = fixture("recovery-exposure");
    let owner = enrolled(&db, &fence).await;
    drop(owner);
    let root = run.dir().canonicalize().unwrap();
    let barrier = root.join("exposure");
    let operation = [1; 16];
    let ready = format!("CANTOS_RECOVERY_BARRIER_V1\nready {}\n", "01".repeat(16));
    let blocked = format!("CANTOS_RECOVERY_BARRIER_V1\nblocked {}\n", "01".repeat(16));
    fs::write(&barrier, &blocked).unwrap();
    let uid = fs::metadata(&root).unwrap().uid();
    let guarded = || {
        let mut request = request(&db, Intent::Serve).with_recovery_fence(&fence);
        request.recovery_barrier =
            Some(RecoveryBarrier::new(barrier.clone(), operation, uid).unwrap());
        request
    };
    recovery_open(OwnedDatabase::open(guarded()).await, RecoveryFault::Pending);
    fs::write(&barrier, &ready).unwrap();
    let owner = OwnedDatabase::open(guarded()).await.unwrap();
    let reader = owner.reader().await.unwrap();
    let mut cursor = reader.query("SELECT 1", ()).await.unwrap();
    let tx = owner.begin_write().await.unwrap();
    tx.execute("INSERT INTO state VALUES(1,1)", ())
        .await
        .unwrap();
    fs::write(&barrier, &blocked).unwrap();
    assert!(matches!(
        cursor.next().await,
        Err(StoreFault::Recovery(RecoveryFault::Pending))
    ));
    assert!(matches!(
        reader.query("SELECT 1", ()).await,
        Err(StoreFault::Recovery(RecoveryFault::Pending))
    ));
    assert!(matches!(
        tx.commit().await,
        Err(cantos_store_turso::error::CommitFault::Rejected(
            StoreFault::Recovery(RecoveryFault::Pending)
        ))
    ));
    assert!(matches!(
        owner.begin_write().await,
        Err(StoreFault::Recovery(RecoveryFault::Pending))
    ));
    drop(cursor);
    drop(reader);
    drop(owner);
    // A READY record from a previous recovery operation cannot open this configuration.
    fs::write(&barrier, ready.replace(&"01".repeat(16), &"02".repeat(16))).unwrap();
    recovery_open(
        OwnedDatabase::open(guarded()).await,
        RecoveryFault::Mismatch,
    );
    fs::remove_file(&barrier).unwrap();
    recovery_open(OwnedDatabase::open(guarded()).await, RecoveryFault::Missing);
    fs::write(&barrier, &ready).unwrap();
    let owner = OwnedDatabase::open(guarded()).await.unwrap();
    let mut rows = owner
        .reader()
        .await
        .unwrap()
        .query("SELECT count(*) FROM state", ())
        .await
        .unwrap();
    assert_eq!(
        rows.next().await.unwrap().unwrap().get_value(0).unwrap(),
        Value::Integer(0)
    );
}
