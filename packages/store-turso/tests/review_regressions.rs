//! Regressions for the owner, backup publication and capability-evidence boundaries.

use std::future::{poll_fn, Future};
use std::path::Path;
use std::pin::Pin;
use std::process::Command;
use std::task::Poll;

use cantos_store_turso::capability;
use cantos_store_turso::registry::{DatabaseKind, Intent};
use cantos_store_turso::testing::{ledger_registry, FixedClock, TestRun};
use cantos_store_turso::{BackupError, OpenRequest, OwnedDatabase};

async fn database(run: &TestRun) -> OwnedDatabase {
    OwnedDatabase::open(OpenRequest::new(
        run.path("server.db"),
        ledger_registry(DatabaseKind::Server),
        Intent::Migrate,
        FixedClock::new(1),
    ))
    .await
    .unwrap()
}

fn assert_child_owner(run: &TestRun, expected: i32) {
    let output = Command::new(env!("CARGO_BIN_EXE_cantos-turso-capability"))
        .arg("try-open")
        .arg(run.dir())
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(expected),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[tokio::test]
async fn a_reader_keeps_the_owner_lease_after_the_database_is_dropped() {
    let run = TestRun::new("reader_lease");
    let db = database(&run).await;
    let reader = db.reader().await.unwrap();
    drop(db);
    assert_child_owner(&run, 10);
    let mut rows = reader.query("SELECT 1", ()).await.unwrap();
    assert!(rows.next().await.unwrap().is_some());
    drop(rows);
    drop(reader);
    assert_child_owner(&run, 0);
}

#[tokio::test]
async fn reader_rows_keep_the_owner_lease_after_both_parents_are_dropped() {
    let run = TestRun::new("reader_rows_lease");
    let db = database(&run).await;
    let reader = db.reader().await.unwrap();
    let mut rows = reader
        .query("SELECT 1 UNION ALL SELECT 2", ())
        .await
        .unwrap();
    assert!(rows.next().await.unwrap().is_some());
    drop(reader);
    drop(db);
    assert_child_owner(&run, 10);
    assert!(rows.next().await.unwrap().is_some());
    assert!(rows.next().await.unwrap().is_none());
    // Exhaustion does not drop the statement's engine/file handles.
    assert_child_owner(&run, 10);
    drop(rows);
    assert_child_owner(&run, 0);
}

#[tokio::test]
async fn writer_rows_keep_the_owner_lease_after_the_transaction_and_database_close() {
    let run = TestRun::new("writer_rows_lease");
    let db = database(&run).await;
    let tx = db.begin_write().await.unwrap();
    let mut rows = tx.query("SELECT 1 UNION ALL SELECT 2", ()).await.unwrap();
    assert!(rows.next().await.unwrap().is_some());
    drop(tx);
    drop(db);
    assert_child_owner(&run, 10);
    assert!(rows.next().await.unwrap().is_some());
    drop(rows);
    assert_child_owner(&run, 0);
}

async fn poll_once<F: Future>(mut future: Pin<&mut F>) -> Option<F::Output> {
    poll_fn(|cx| {
        Poll::Ready(match future.as_mut().poll(cx) {
            Poll::Ready(result) => Some(result),
            Poll::Pending => None,
        })
    })
    .await
}

#[tokio::test]
async fn overlapping_backups_never_delete_the_winners_verified_file() {
    let run = TestRun::new("backup_overlap");
    let db = database(&run).await;
    let target = run.path("copy.db");
    let tx = db.begin_write().await.unwrap();
    let mut first = std::pin::pin!(db.backup_to(&target));
    let mut second = std::pin::pin!(db.backup_to(&target));
    // Both attempts reach the held writer in the defective implementation. The repaired
    // implementation may refuse the loser immediately at its atomic reservation instead.
    assert!(poll_once(first.as_mut()).await.is_none());
    let early_second = poll_once(second.as_mut()).await;
    tx.rollback().await.unwrap();
    let (first, second) = tokio::join!(first, async {
        match early_second {
            Some(result) => result,
            None => second.await,
        }
    });
    let receipt = first.expect("the first queued backup must survive its competitor");
    assert_eq!(second.unwrap_err(), BackupError::DestinationExists);
    assert_eq!(std::fs::metadata(&target).unwrap().len(), receipt.bytes);
    let preserved = std::fs::read(&target).unwrap();
    assert_eq!(
        db.backup_to(&target).await.unwrap_err(),
        BackupError::DestinationExists
    );
    assert_eq!(std::fs::read(&target).unwrap(), preserved);
}

#[tokio::test]
async fn a_destination_created_while_the_backup_waits_is_preserved() {
    let run = TestRun::new("backup_foreign_destination");
    let db = database(&run).await;
    let target = run.path("copy.db");
    let tx = db.begin_write().await.unwrap();
    let mut attempt = std::pin::pin!(db.backup_to(&target));
    assert!(poll_once(attempt.as_mut()).await.is_none());
    std::fs::write(&target, b"another attempt owns these bytes").unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(attempt.await.unwrap_err(), BackupError::DestinationExists);
    assert_eq!(
        std::fs::read(&target).unwrap(),
        b"another attempt owns these bytes"
    );
    assert_no_backup_staging(run.dir());
}

fn assert_no_backup_staging(dir: &Path) {
    for entry in std::fs::read_dir(dir).unwrap() {
        assert!(!entry
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".backup"));
    }
}

#[tokio::test]
async fn cancellation_cleans_only_its_reserved_staging_and_allows_a_new_attempt() {
    let run = TestRun::new("backup_cancel");
    let db = database(&run).await;
    let target = run.path("copy.db");
    let tx = db.begin_write().await.unwrap();
    let mut attempt = Box::pin(db.backup_to(&target));
    assert!(poll_once(attempt.as_mut()).await.is_none());
    assert!(run.path("copy.db.backup").is_dir());
    drop(attempt);
    assert_no_backup_staging(run.dir());
    assert!(!target.exists());
    tx.rollback().await.unwrap();
    db.backup_to(&target).await.unwrap();
    assert_no_backup_staging(run.dir());
}

#[tokio::test]
async fn an_existing_backup_reservation_is_never_adopted_or_removed() {
    let run = TestRun::new("backup_existing_reservation");
    let db = database(&run).await;
    let target = run.path("copy.db");
    let reservation = run.path("copy.db.backup");
    std::fs::create_dir(&reservation).unwrap();
    let marker = reservation.join("belongs-to-another-attempt");
    std::fs::write(&marker, b"preserve").unwrap();
    assert_eq!(
        db.backup_to(&target).await.unwrap_err(),
        BackupError::DestinationExists
    );
    assert_eq!(std::fs::read(marker).unwrap(), b"preserve");
    assert!(!target.exists());
}

#[tokio::test]
async fn a_probe_setup_error_cannot_match_an_expected_engine_hazard() {
    let run = TestRun::new("probe_setup_error");
    let report = capability::run_all(&run.path("missing_directory")).await;
    assert!(
        report.probes.iter().all(|probe| !probe.as_expected()),
        "{}",
        report.render()
    );
}
