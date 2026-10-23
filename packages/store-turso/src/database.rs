//! The owner of one Turso file: exclusive ownership, startup verification against the schema
//! registry, one serialized writer, read-only readers and verified online backup.
//!
//! Why one writer: the engine's default journal admits a single writer and the deferred-upgrade
//! path fails with a stale-snapshot error, so every mutation begins `BEGIN IMMEDIATE` on the
//! owner's writer connection. This replaces PostgreSQL advisory and row locks only by ordering;
//! it does not replace conditional updates that recheck a revision, which stay the caller's job.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::Duration;

use sha2::{Digest, Sha256};
use tokio::sync::{Mutex, MutexGuard};
use turso::{Builder, Connection, IntoParams, Row, Value};

use crate::clock::Clock;
use crate::error::{CommitFault, OpenError, StoreFault};
use crate::owner::{lock_path, private_directory_of, OwnerLock};
use crate::registry::{
    plan_startup, AppliedMigration, Intent, Migration, Observation, Plan, PresentFile, Refusal,
    Registry, HISTORY_TABLE,
};
use crate::sql;
use crate::value::UnixMicros;

/// Matches the three-second `lock_timeout` of the PostgreSQL store it replaces.
pub const DEFAULT_BUSY_TIMEOUT: Duration = Duration::from_secs(3);

const HISTORY_DDL: &str = "CREATE TABLE cantos_schema_history(\
    version INTEGER PRIMARY KEY CHECK (version >= 1), \
    name TEXT NOT NULL, \
    checksum BLOB NOT NULL CHECK (length(checksum) = 32), \
    applied_at INTEGER NOT NULL CHECK (applied_at >= 0)) STRICT; \
    CREATE TRIGGER cantos_schema_history_no_update BEFORE UPDATE ON cantos_schema_history \
    BEGIN SELECT RAISE(ABORT, 'cantos:immutable.schema_history'); END; \
    CREATE TRIGGER cantos_schema_history_no_delete BEFORE DELETE ON cantos_schema_history \
    BEGIN SELECT RAISE(ABORT, 'cantos:immutable.schema_history'); END;";

pub struct OpenRequest {
    pub path: PathBuf,
    pub registry: Registry,
    pub intent: Intent,
    pub clock: Arc<dyn Clock>,
    pub busy_timeout: Duration,
}

impl OpenRequest {
    pub fn new(
        path: impl Into<PathBuf>,
        registry: Registry,
        intent: Intent,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            path: path.into(),
            registry,
            intent,
            clock,
            busy_timeout: DEFAULT_BUSY_TIMEOUT,
        }
    }
}

pub struct OwnedDatabase {
    database: turso::Database,
    writer: Mutex<Connection>,
    registry: Registry,
    clock: Arc<dyn Clock>,
    path: PathBuf,
    busy_timeout: Duration,
    // Declared last so the engine handles close before the ownership lock is released.
    owner: Arc<OwnerLock>,
}

impl OwnedDatabase {
    /// Take exclusive ownership, inspect the file read-only, refuse anything the registry does
    /// not allow, then execute exactly the planned creation or migrations.
    pub async fn open(request: OpenRequest) -> Result<Self, OpenError> {
        let owner = OwnerLock::acquire(&request.path)?;
        let observed = observe_path(&request.path).await?;
        let plan = plan_startup(&request.registry, &observed, request.intent)
            .map_err(OpenError::Refused)?;
        let database = build(&request.path, false).await?;
        let writer = database.connect().map_err(StoreFault::from)?;
        configure(&writer, request.busy_timeout, false).await?;
        ensure_durable_journal(&writer).await?;
        run_plan(&writer, &request.registry, plan, request.clock.as_ref()).await?;
        let after = Observation::Present(observe_connection(&writer).await?);
        match plan_startup(&request.registry, &after, Intent::Serve) {
            Ok(Plan::Ready) => {}
            Ok(_) | Err(_) => {
                return Err(OpenError::Refused(Refusal::HistoryDiverged { version: 0 }))
            }
        }
        Ok(Self {
            database,
            writer: Mutex::new(writer),
            registry: request.registry,
            clock: request.clock,
            path: request.path,
            busy_timeout: request.busy_timeout,
            owner: Arc::new(owner),
        })
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn registry(&self) -> &Registry {
        &self.registry
    }

    /// Begin the only kind of mutation: `BEGIN IMMEDIATE` on the serialized writer. The trusted
    /// time is read after the writer is ours, so timestamps follow commit order.
    pub async fn begin_write(&self) -> Result<WriteTransaction<'_>, StoreFault> {
        let guard = self.writer.lock().await;
        self.begin_on(guard).await
    }

    /// [`Self::begin_write`] with a bounded wait for the serialized writer. A stalled
    /// transaction queues every writer behind it, so a request path states how long it may
    /// wait; the timeout is [`StoreFault::Busy`] and nothing was changed.
    pub async fn begin_write_within(
        &self,
        wait: Duration,
    ) -> Result<WriteTransaction<'_>, StoreFault> {
        let guard = tokio::time::timeout(wait, self.writer.lock())
            .await
            .map_err(|_| StoreFault::Busy)?;
        self.begin_on(guard).await
    }

    async fn begin_on<'a>(
        &'a self,
        guard: MutexGuard<'a, Connection>,
    ) -> Result<WriteTransaction<'a>, StoreFault> {
        recover(&guard).await?;
        let now = self.clock.now().map_err(|_| StoreFault::Clock)?;
        guard.execute("BEGIN IMMEDIATE", ()).await?;
        Ok(WriteTransaction {
            guard,
            now,
            owner: self.owner.clone(),
        })
    }

    /// A new connection that cannot write. Its snapshot is isolated from the open writer.
    pub async fn reader(&self) -> Result<ReadConnection, StoreFault> {
        let conn = self.database.connect()?;
        configure(&conn, self.busy_timeout, true).await?;
        Ok(ReadConnection {
            conn,
            owner: self.owner.clone(),
        })
    }

    /// Reserve a private attempt directory, copy under the writer, verify and publish with an
    /// atomic no-clobber hard link. Cancellation/failure removes only this attempt's temporary
    /// files; a destination published by another attempt is never removed.
    /// The engine has no backup API, so the copy is the only supported online path.
    pub async fn backup_to(&self, destination: &Path) -> Result<BackupReceipt, BackupError> {
        private_directory_of(destination).map_err(BackupError::Destination)?;
        if destination == self.path
            || fs::symlink_metadata(destination).is_ok()
            || fs::symlink_metadata(lock_path(destination)).is_ok()
        {
            return Err(BackupError::DestinationExists);
        }
        let attempt = BackupAttempt::reserve(destination)?;
        let literal = attempt
            .path
            .to_str()
            .filter(|text| !text.contains('\0'))
            .ok_or(BackupError::Io(std::io::ErrorKind::InvalidInput))?
            .replace('\'', "''");
        let copied = async {
            let guard = self.writer.lock().await;
            recover(&guard).await?;
            // The engine copies the snapshot this connection last saw, and a connection that
            // predates a schema change fails after creating the file. A read first brings the
            // connection up to date; without it the copy omits other connections' newest commits.
            sql::scalar_integer(&guard, "SELECT count(*) FROM sqlite_schema", ()).await?;
            guard
                .execute(&format!("VACUUM INTO '{literal}'"), ())
                .await
                .map_err(StoreFault::from)?;
            row_manifest(&guard).await
        }
        .await;
        let live = copied?;
        let mut receipt = self.verify_copy(&attempt.path, &live).await?;
        attempt.publish(destination)?;
        receipt.path = destination.to_path_buf();
        Ok(receipt)
    }

    async fn verify_copy(
        &self,
        destination: &Path,
        live: &[(String, i64)],
    ) -> Result<BackupReceipt, BackupError> {
        let database = build(destination, true)
            .await
            .map_err(BackupError::from_open)?;
        let conn = database.connect().map_err(StoreFault::from)?;
        let check = sql::scalar_text(&conn, "PRAGMA integrity_check").await?;
        if check != "ok" {
            return Err(BackupError::IntegrityCheckFailed);
        }
        let observed = Observation::Present(observe_connection(&conn).await?);
        plan_startup(&self.registry, &observed, Intent::Serve).map_err(BackupError::Mismatch)?;
        if row_manifest(&conn).await? != live {
            return Err(BackupError::ContentMismatch);
        }
        drop(conn);
        drop(database);
        let mut file =
            fs::File::open(destination).map_err(|error| BackupError::Io(error.kind()))?;
        let mut hasher = Sha256::new();
        let mut bytes = 0_u64;
        let mut buffer = [0_u8; 64 * 1024];
        loop {
            let read = file
                .read(&mut buffer)
                .map_err(|error| BackupError::Io(error.kind()))?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
            bytes += read as u64;
        }
        let writable = fs::OpenOptions::new()
            .write(true)
            .open(destination)
            .map_err(|error| BackupError::Io(error.kind()))?;
        writable
            .sync_all()
            .map_err(|error| BackupError::Io(error.kind()))?;
        if let Some(parent) = destination.parent().filter(|p| !p.as_os_str().is_empty()) {
            fs::File::open(parent)
                .and_then(|dir| dir.sync_all())
                .map_err(|error| BackupError::Io(error.kind()))?;
        }
        Ok(BackupReceipt {
            path: destination.to_path_buf(),
            bytes,
            sha256: hasher.finalize().into(),
        })
    }
}

/// An open `BEGIN IMMEDIATE` transaction. Dropping it without `commit` or `rollback` leaves the
/// transaction open until the next `begin_write`, which rolls it back before beginning.
pub struct WriteTransaction<'a> {
    guard: MutexGuard<'a, Connection>,
    now: UnixMicros,
    owner: Arc<OwnerLock>,
}

impl WriteTransaction<'_> {
    /// The trusted timestamp of this transaction; SQL must not read the wall clock.
    pub fn now(&self) -> UnixMicros {
        self.now
    }

    pub async fn execute(&self, sql: &str, params: impl IntoParams) -> Result<u64, StoreFault> {
        Ok(self.guard.execute(sql, params).await?)
    }

    /// Drain or drop the returned `Rows` before the next statement: a write while a `RETURNING`
    /// cursor is open is refused as busy by the engine.
    pub async fn query(&self, sql: &str, params: impl IntoParams) -> Result<Rows, StoreFault> {
        Ok(Rows {
            inner: self.guard.query(sql, params).await?,
            _owner: self.owner.clone(),
        })
    }

    pub async fn commit(self) -> Result<(), CommitFault> {
        match self.guard.execute("COMMIT", ()).await {
            Ok(_) => Ok(()),
            Err(error) => {
                let fault = CommitFault::classify(error);
                if matches!(fault, CommitFault::Rejected(_)) {
                    // A rejected COMMIT (deferred foreign key, busy) leaves the transaction open.
                    let _ = self.guard.execute("ROLLBACK", ()).await;
                }
                Err(fault)
            }
        }
    }

    pub async fn rollback(self) -> Result<(), StoreFault> {
        self.guard.execute("ROLLBACK", ()).await?;
        Ok(())
    }
}

pub struct ReadConnection {
    conn: Connection,
    owner: Arc<OwnerLock>,
}

impl ReadConnection {
    pub async fn query(&self, sql: &str, params: impl IntoParams) -> Result<Rows, StoreFault> {
        Ok(Rows {
            inner: self.conn.query(sql, params).await?,
            _owner: self.owner.clone(),
        })
    }

    /// Pin one snapshot for every following read until [`Self::end_snapshot`], so a multi-query
    /// read (a page plus its count, a head plus its history) cannot observe two commits.
    pub async fn begin_snapshot(&self) -> Result<(), StoreFault> {
        self.conn.execute("BEGIN", ()).await?;
        Ok(())
    }

    /// Release the pinned snapshot. Reads cannot fail to commit; a failure here is reported but
    /// never changes data.
    pub async fn end_snapshot(&self) -> Result<(), StoreFault> {
        if !self.conn.is_autocommit()? {
            self.conn.execute("ROLLBACK", ()).await?;
        }
        Ok(())
    }
}

/// A query cursor retains the owner lease even after its connection, transaction and database
/// wrappers close. No raw statement/connection escape can detach those engine handles from it.
pub struct Rows {
    inner: turso::Rows,
    // Keep last: the statement and its engine handles must close before releasing ownership.
    _owner: Arc<OwnerLock>,
}

impl Rows {
    pub async fn next(&mut self) -> Result<Option<Row>, StoreFault> {
        Ok(self.inner.next().await?)
    }

    pub fn column_count(&self) -> usize {
        self.inner.column_count()
    }

    pub fn column_name(&self, index: usize) -> Result<String, StoreFault> {
        Ok(self.inner.column_name(index)?)
    }

    pub fn column_names(&self) -> Vec<String> {
        self.inner.column_names()
    }

    pub fn column_index(&self, name: &str) -> Result<usize, StoreFault> {
        Ok(self.inner.column_index(name)?)
    }

    pub fn columns(&self) -> Vec<turso::Column> {
        self.inner.columns()
    }
}

/// `create_dir` reserves this destination across tasks/processes before waiting for the writer.
/// An interrupted process leaves a visible reservation for operator reconciliation, never an
/// apparently complete backup. We never adopt or remove an existing reservation.
struct BackupAttempt {
    directory: PathBuf,
    path: PathBuf,
}

impl BackupAttempt {
    fn reserve(destination: &Path) -> Result<Self, BackupError> {
        let mut name = destination.as_os_str().to_owned();
        name.push(".backup");
        let directory = PathBuf::from(name);
        let mut builder = fs::DirBuilder::new();
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            builder.mode(0o700);
        }
        builder
            .create(&directory)
            .map_err(|error| match error.kind() {
                std::io::ErrorKind::AlreadyExists => BackupError::DestinationExists,
                kind => BackupError::Io(kind),
            })?;
        Ok(Self {
            path: directory.join("copy.db"),
            directory,
        })
    }

    fn publish(&self, destination: &Path) -> Result<(), BackupError> {
        if fs::symlink_metadata(lock_path(destination)).is_ok() {
            return Err(BackupError::DestinationExists);
        }
        // Unlike rename, hard_link refuses an existing path atomically on both supported OSes.
        fs::hard_link(&self.path, destination).map_err(|error| match error.kind() {
            std::io::ErrorKind::AlreadyExists => BackupError::DestinationExists,
            kind => BackupError::Io(kind),
        })?;
        let (parent, _) = private_directory_of(destination).map_err(BackupError::Destination)?;
        fs::File::open(parent)
            .and_then(|directory| directory.sync_all())
            .map_err(|error| BackupError::Io(error.kind()))
    }
}

impl Drop for BackupAttempt {
    fn drop(&mut self) {
        // Only exact paths inside the directory this attempt atomically created are ours.
        for suffix in ["", "-wal", "-shm"] {
            let mut name = self.path.as_os_str().to_owned();
            name.push(suffix);
            let _ = fs::remove_file(PathBuf::from(name));
        }
        let _ = fs::remove_dir(&self.directory);
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BackupReceipt {
    pub path: PathBuf,
    pub bytes: u64,
    pub sha256: [u8; 32],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BackupError {
    Destination(OpenError),
    /// The destination, owner-lock sidecar or backup reservation exists; never overwrite/adopt.
    DestinationExists,
    Store(StoreFault),
    IntegrityCheckFailed,
    Mismatch(Refusal),
    /// The copy's per-table row counts differ from the live file read under the same writer lock.
    ContentMismatch,
    Io(std::io::ErrorKind),
}

impl BackupError {
    fn from_open(error: OpenError) -> Self {
        match error {
            OpenError::Store(fault) => Self::Store(fault),
            OpenError::Io(kind) => Self::Io(kind),
            other => Self::Destination(other),
        }
    }
}

impl From<StoreFault> for BackupError {
    fn from(fault: StoreFault) -> Self {
        Self::Store(fault)
    }
}

impl From<turso::Error> for BackupError {
    fn from(error: turso::Error) -> Self {
        Self::Store(error.into())
    }
}

impl std::fmt::Display for BackupError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            Self::Destination(_) => "the backup destination is not acceptable",
            Self::DestinationExists => "the backup destination already exists",
            Self::Store(_) => "the backup failed in the database engine",
            Self::IntegrityCheckFailed => "the backup failed the integrity check",
            Self::Mismatch(_) => "the backup is not a current file of this service",
            Self::ContentMismatch => "the backup does not hold the rows of the live file",
            Self::Io(_) => "the backup failed on file I/O",
        })
    }
}

impl std::error::Error for BackupError {}

/// `(table, row count)` for every user table, ordered by name. Equal manifests on the live file
/// (read while the writer is held) and on its copy prove the copy omitted no committed rows.
async fn row_manifest(conn: &Connection) -> Result<Vec<(String, i64)>, StoreFault> {
    let mut rows = conn
        .query(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND substr(name, 1, 7) <> 'sqlite_' ORDER BY name",
            (),
        )
        .await?;
    let mut tables = Vec::new();
    while let Some(row) = rows.next().await? {
        tables.push(sql::text(&row, 0)?);
    }
    drop(rows);
    let mut manifest = Vec::with_capacity(tables.len());
    for table in tables {
        let quoted = table.replace('"', "\"\"");
        let count =
            sql::scalar_integer(conn, &format!("SELECT count(*) FROM \"{quoted}\""), ()).await?;
        manifest.push((table, count));
    }
    Ok(manifest)
}

async fn build(path: &Path, read_only: bool) -> Result<turso::Database, OpenError> {
    let text = path
        .to_str()
        .ok_or(OpenError::Io(std::io::ErrorKind::InvalidInput))?;
    // No experimental feature is enabled: `VACUUM INTO` works without any, and the experimental
    // vacuum flag would only switch on the in-place rewrite, which no Cantos path wants.
    Builder::new_local(text)
        .read_only(read_only)
        .build()
        .await
        .map_err(|error| OpenError::Store(error.into()))
}

/// Foreign keys are off by default on every connection, so each one turns them on and proves it.
async fn configure(conn: &Connection, busy: Duration, read_only: bool) -> Result<(), StoreFault> {
    conn.busy_timeout(busy)?;
    conn.execute("PRAGMA foreign_keys = ON", ()).await?;
    if sql::scalar_integer(conn, "PRAGMA foreign_keys", ()).await? != 1 {
        return Err(StoreFault::Engine);
    }
    if read_only {
        conn.execute("PRAGMA query_only = 1", ()).await?;
        if sql::scalar_integer(conn, "PRAGMA query_only", ()).await? != 1 {
            return Err(StoreFault::Engine);
        }
    }
    Ok(())
}

/// A fully synchronous write-ahead journal is the durability the migration promises. A different
/// engine default must be a decision, not an accident.
async fn ensure_durable_journal(conn: &Connection) -> Result<(), StoreFault> {
    conn.execute("PRAGMA synchronous = FULL", ()).await?;
    let synchronous = sql::scalar_integer(conn, "PRAGMA synchronous", ()).await?;
    let journal = sql::scalar_text(conn, "PRAGMA journal_mode").await?;
    if synchronous != 2 || !journal.eq_ignore_ascii_case("wal") {
        return Err(StoreFault::Engine);
    }
    Ok(())
}

/// Roll back what a dropped or cancelled transaction left on the writer.
async fn recover(conn: &Connection) -> Result<(), StoreFault> {
    if !conn.is_autocommit()? {
        conn.execute("ROLLBACK", ()).await?;
    }
    Ok(())
}

async fn observe_path(path: &Path) -> Result<Observation, OpenError> {
    let main = match fs::metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            return Ok(Observation::Absent)
        }
        Err(error) => return Err(OpenError::Io(error.kind())),
    };
    if !main.is_file() {
        return Err(OpenError::Io(std::io::ErrorKind::InvalidInput));
    }
    // A crash right after creation can leave committed data only in the write-ahead log.
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    let wal_bytes = fs::metadata(PathBuf::from(wal)).map_or(0, |meta| meta.len());
    if main.len() == 0 && wal_bytes == 0 {
        return Ok(Observation::Absent);
    }
    let database = build(path, true).await?;
    let conn = database.connect().map_err(StoreFault::from)?;
    Ok(Observation::Present(observe_connection(&conn).await?))
}

async fn observe_connection(conn: &Connection) -> Result<PresentFile, StoreFault> {
    let header = |value: i64| i32::try_from(value).map_err(|_| StoreFault::Corrupt);
    let application_id = header(sql::scalar_integer(conn, "PRAGMA application_id", ()).await?)?;
    let user_version = header(sql::scalar_integer(conn, "PRAGMA user_version", ()).await?)?;
    let user_objects = u64::try_from(
        sql::scalar_integer(
            conn,
            "SELECT count(*) FROM sqlite_schema WHERE substr(name, 1, 7) <> 'sqlite_'",
            (),
        )
        .await?,
    )
    .map_err(|_| StoreFault::Corrupt)?;
    let has_history = sql::scalar_integer(
        conn,
        "SELECT count(*) FROM sqlite_schema WHERE type = 'table' AND name = ?1",
        [HISTORY_TABLE],
    )
    .await?
        == 1;
    let applied = if has_history {
        let mut rows = conn
            .query(
                "SELECT version, name, checksum FROM cantos_schema_history ORDER BY version",
                (),
            )
            .await?;
        let mut applied = Vec::new();
        while let Some(row) = rows.next().await? {
            applied.push(AppliedMigration {
                version: u32::try_from(sql::integer(&row, 0)?).map_err(|_| StoreFault::Corrupt)?,
                name: sql::text(&row, 1)?,
                checksum: sql::blob(&row, 2)?
                    .try_into()
                    .map_err(|_| StoreFault::Corrupt)?,
            });
        }
        Some(applied)
    } else {
        None
    };
    Ok(PresentFile {
        application_id,
        user_version,
        user_objects,
        applied,
    })
}

async fn run_plan(
    conn: &Connection,
    registry: &Registry,
    plan: Plan,
    clock: &dyn Clock,
) -> Result<(), OpenError> {
    let failed =
        |version: u32| move |fault: StoreFault| OpenError::MigrationFailed { version, fault };
    match plan {
        Plan::Ready => Ok(()),
        Plan::Create(migrations) => {
            let now = clock.now().map_err(|_| failed(0)(StoreFault::Clock))?;
            transact(conn, 0, async |version| {
                conn.execute(
                    &format!("PRAGMA application_id = {}", registry.application_id()),
                    (),
                )
                .await?;
                conn.execute(&format!("PRAGMA user_version = {}", registry.epoch()), ())
                    .await?;
                conn.execute_batch(HISTORY_DDL).await?;
                for migration in migrations {
                    version.set(migration.version);
                    apply(conn, migration, now).await?;
                }
                Ok(())
            })
            .await
        }
        Plan::Upgrade(migrations) => {
            for migration in migrations {
                let now = clock
                    .now()
                    .map_err(|_| failed(migration.version)(StoreFault::Clock))?;
                transact(conn, migration.version, async |_| {
                    let applied = sql::scalar_integer(
                        conn,
                        "SELECT coalesce(max(version), 0) FROM cantos_schema_history",
                        (),
                    )
                    .await?;
                    if applied != i64::from(migration.version) - 1 {
                        return Err(StoreFault::Engine);
                    }
                    apply(conn, migration, now).await
                })
                .await?;
            }
            Ok(())
        }
    }
}

/// Which migration a failure belongs to, for `MigrationFailed`.
struct Progress(std::cell::Cell<u32>);

impl Progress {
    fn set(&self, version: u32) {
        self.0.set(version);
    }
}

async fn transact<F>(conn: &Connection, first: u32, body: F) -> Result<(), OpenError>
where
    F: AsyncFnOnce(&Progress) -> Result<(), StoreFault>,
{
    transact_with_commit(conn, first, body, async || conn.execute("COMMIT", ()).await).await
}

/// Keep the commit acknowledgement as an explicit boundary so tests can lose it after the real
/// engine has committed. The production path always supplies the actual engine COMMIT above.
async fn transact_with_commit<F, C>(
    conn: &Connection,
    first: u32,
    body: F,
    commit: C,
) -> Result<(), OpenError>
where
    F: AsyncFnOnce(&Progress) -> Result<(), StoreFault>,
    C: AsyncFnOnce() -> Result<u64, turso::Error>,
{
    let progress = Progress(std::cell::Cell::new(first));
    let failed = |progress: &Progress, fault: StoreFault| OpenError::MigrationFailed {
        version: progress.0.get(),
        fault,
    };
    conn.execute("BEGIN IMMEDIATE", ())
        .await
        .map_err(|error| failed(&progress, error.into()))?;
    if let Err(fault) = body(&progress).await {
        let _ = conn.execute("ROLLBACK", ()).await;
        return Err(failed(&progress, fault));
    }
    match commit().await {
        Ok(_) => Ok(()),
        Err(error) => {
            let fault = CommitFault::classify(error);
            match fault {
                CommitFault::Rejected(fault) => {
                    let _ = conn.execute("ROLLBACK", ()).await;
                    Err(failed(&progress, fault))
                }
                CommitFault::Unknown(fault) => Err(OpenError::MigrationCommitUnknown {
                    version: progress.0.get(),
                    fault,
                }),
            }
        }
    }
}

async fn apply(
    conn: &Connection,
    migration: &Migration,
    now: UnixMicros,
) -> Result<(), StoreFault> {
    conn.execute_batch(migration.sql).await?;
    // Migration SQL that ended the runner's transaction would leave the history row outside it.
    if conn.is_autocommit()? {
        return Err(StoreFault::Misuse);
    }
    conn.execute(
        "INSERT INTO cantos_schema_history(version, name, checksum, applied_at) VALUES (?1, ?2, ?3, ?4)",
        turso::params_from_iter([
            Value::Integer(i64::from(migration.version)),
            Value::Text(migration.name.to_owned()),
            Value::Blob(migration.checksum().to_vec()),
            Value::Integer(now.get()),
        ]),
    )
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::TestRun;

    #[tokio::test]
    async fn a_lost_migration_commit_acknowledgement_is_not_reported_as_rolled_back() {
        let run = TestRun::new("migration_unknown");
        let path = run.path("migration.db");
        let database = build(&path, false).await.unwrap();
        let conn = database.connect().unwrap();
        conn.execute("CREATE TABLE receipt(id INTEGER PRIMARY KEY) STRICT", ())
            .await
            .unwrap();
        let failure = transact_with_commit(
            &conn,
            7,
            async |_| {
                conn.execute("INSERT INTO receipt VALUES (1)", ()).await?;
                Ok(())
            },
            async || {
                conn.execute("COMMIT", ()).await?;
                Err(turso::Error::IoError(
                    std::io::ErrorKind::ConnectionAborted,
                    "injected lost acknowledgement after the real commit",
                ))
            },
        )
        .await
        .unwrap_err();
        assert_eq!(
            failure,
            OpenError::MigrationCommitUnknown {
                version: 7,
                fault: StoreFault::Io(std::io::ErrorKind::ConnectionAborted),
            }
        );
        assert!(failure.to_string().contains("commit outcome is unknown"));
        assert!(!failure.to_string().contains("rolled back"));
        drop(conn);
        drop(database);
        let reopened = build(&path, true).await.unwrap();
        assert_eq!(
            sql::scalar_integer(
                &reopened.connect().unwrap(),
                "SELECT count(*) FROM receipt",
                ()
            )
            .await
            .unwrap(),
            1
        );
    }
}
