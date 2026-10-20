//! Run-owned directories and a deterministic clock for tests. Behind the `testing` feature so
//! no runtime build carries them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;

use crate::clock::{Clock, ClockFault};
use crate::registry::{DatabaseKind, Migration, Registry};
use crate::value::UnixMicros;

/// One table every fixture file has: an append-only ledger the crash and ownership tests write.
pub const LEDGER_MIGRATIONS: [Migration; 1] = [Migration {
    version: 1,
    name: "create_ledger",
    sql: "CREATE TABLE ledger(\
        id INTEGER PRIMARY KEY, \
        payload BLOB NOT NULL CHECK (length(payload) BETWEEN 1 AND 4096), \
        at INTEGER NOT NULL CHECK (at >= 0)) STRICT;",
}];

pub fn ledger_registry(kind: DatabaseKind) -> Registry {
    Registry::new(kind, 1, &LEDGER_MIGRATIONS).expect("the fixture registry is valid")
}

/// A private temporary directory removed when dropped. Every database a test opens lives here,
/// never in a shared or inherited location.
pub struct TestRun {
    dir: PathBuf,
}

impl TestRun {
    pub fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let unique = format!(
            "cantos-turso-{label}-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        );
        let dir = std::env::temp_dir().join(unique);
        let mut builder = std::fs::DirBuilder::new();
        builder.recursive(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&dir)
            .expect("a unique test directory can be created");
        Self { dir }
    }

    pub fn dir(&self) -> &Path {
        &self.dir
    }

    pub fn path(&self, name: &str) -> PathBuf {
        self.dir.join(name)
    }
}

impl Drop for TestRun {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// A clock the test moves by hand.
#[derive(Debug)]
pub struct FixedClock(AtomicI64);

impl FixedClock {
    pub fn new(micros: i64) -> Arc<Self> {
        Arc::new(Self(AtomicI64::new(micros)))
    }

    pub fn set(&self, micros: i64) {
        self.0.store(micros, Ordering::SeqCst);
    }

    pub fn advance(&self, micros: i64) {
        self.0.fetch_add(micros, Ordering::SeqCst);
    }
}

impl Clock for FixedClock {
    fn now(&self) -> Result<UnixMicros, ClockFault> {
        UnixMicros::new(self.0.load(Ordering::SeqCst)).map_err(|_| ClockFault)
    }
}
