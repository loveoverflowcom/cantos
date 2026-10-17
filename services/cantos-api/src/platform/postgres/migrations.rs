//! Epoch-bound schema bootstrap. Historical epoch 1 is recognized, never upgraded implicitly.
use std::{error::Error, fmt};

use sha2::{Digest, Sha256};
use tokio_postgres::{Config, NoTls, Transaction};

pub const SCHEMA_EPOCH: i32 = 2;
const BASELINE: &str = include_str!("../../../schema/0001_current.sql");

/// Static diagnostics deliberately contain no connection strings or database contents.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SchemaError {
    LegacyDatabase,
    UnrecognizedDatabase,
    IncompatibleEpoch,
    ChecksumMismatch,
    RegistryMismatch,
    UnsupportedPostgres,
    DatabaseUnavailable,
}

impl SchemaError {
    pub fn message(self) -> &'static str {
        match self {
            Self::LegacyDatabase => "legacy schema epoch 1: explicit inventoried backup/export/import cutover required; database unchanged",
            Self::UnrecognizedDatabase => "unmarked nonempty database: refusing schema bootstrap; database unchanged",
            Self::IncompatibleEpoch => "incompatible or missing schema epoch: expected Cantos epoch 2; database unchanged",
            Self::ChecksumMismatch => "schema checksum mismatch: refusing to edit applied history; database unchanged",
            Self::RegistryMismatch => "schema registry mismatch: explicit operator investigation required; database unchanged",
            Self::UnsupportedPostgres => "unsupported PostgreSQL major: current schema supports stable PostgreSQL 17 and 18 only",
            Self::DatabaseUnavailable => "schema check or transaction failed; no partial schema transaction accepted",
        }
    }
}

impl fmt::Display for SchemaError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.message())
    }
}
impl Error for SchemaError {}
impl From<tokio_postgres::Error> for SchemaError {
    fn from(_: tokio_postgres::Error) -> Self {
        Self::DatabaseUnavailable
    }
}

async fn supported_server(tx: &Transaction<'_>) -> Result<(), SchemaError> {
    let version: i32 = tx
        .query_one("SELECT current_setting('server_version_num')::integer", &[])
        .await?
        .get(0);
    if !(170_000..190_000).contains(&version) {
        return Err(SchemaError::UnsupportedPostgres);
    }
    Ok(())
}

async fn marker_exists(tx: &Transaction<'_>) -> Result<bool, SchemaError> {
    Ok(tx
        .query_one(
            "SELECT to_regclass('public.cantos_schema') IS NOT NULL",
            &[],
        )
        .await?
        .get(0))
}

async fn check_registry(tx: &Transaction<'_>) -> Result<(), SchemaError> {
    let epochs = tx
        .query("SELECT epoch FROM public.cantos_schema", &[])
        .await?;
    if epochs.len() != 1 || epochs[0].get::<_, i32>(0) != SCHEMA_EPOCH {
        return Err(SchemaError::IncompatibleEpoch);
    }
    let registered: bool = tx
        .query_one(
            "SELECT to_regclass('public.cantos_migrations') IS NOT NULL",
            &[],
        )
        .await?
        .get(0);
    if !registered {
        return Err(SchemaError::RegistryMismatch);
    }
    let rows = tx
        .query(
            "SELECT version,checksum FROM public.cantos_migrations ORDER BY version",
            &[],
        )
        .await?;
    if rows.len() != 1 || rows[0].get::<_, i32>(0) != 1 {
        return Err(SchemaError::RegistryMismatch);
    }
    if rows[0].get::<_, Vec<u8>>(1) != Sha256::digest(BASELINE.as_bytes()).to_vec() {
        return Err(SchemaError::ChecksumMismatch);
    }
    Ok(())
}

async fn unmarked_database(tx: &Transaction<'_>) -> Result<(), SchemaError> {
    let legacy: bool = tx
        .query_one(
            "SELECT to_regclass('public.cantos_migrations') IS NOT NULL",
            &[],
        )
        .await?
        .get(0);
    if legacy {
        return Err(SchemaError::LegacyDatabase);
    }
    // Include functions, types and non-public schemas: an empty table is still existing data
    // ownership. Bootstrap never adopts an installation merely because its row counts are zero.
    let occupied: bool = tx
        .query_one(
            "SELECT EXISTS(SELECT FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
             WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema')
         OR EXISTS(SELECT FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
             WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema')
         OR EXISTS(SELECT FROM pg_type t JOIN pg_namespace n ON n.oid=t.typnamespace
             WHERE n.nspname !~ '^pg_' AND n.nspname<>'information_schema')
         OR EXISTS(SELECT FROM pg_namespace WHERE nspname !~ '^pg_'
             AND nspname NOT IN ('public','information_schema'))",
            &[],
        )
        .await?
        .get(0);
    if occupied {
        return Err(SchemaError::UnrecognizedDatabase);
    }
    Ok(())
}

/// DDL belongs only to the migration actor. A current installation is checked without writes;
/// a new installation is created atomically. No path resets, drops or upgrades epoch 1.
pub async fn migrate(config: &Config) -> Result<(), SchemaError> {
    let (mut client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let tx = client.transaction().await?;
    tx.batch_execute("SELECT pg_advisory_xact_lock(1128353364)")
        .await?;
    supported_server(&tx).await?;
    if marker_exists(&tx).await? {
        check_registry(&tx).await?;
    } else {
        unmarked_database(&tx).await?;
        tx.batch_execute(
            "CREATE TABLE public.cantos_schema(singleton boolean PRIMARY KEY CHECK(singleton),
                 epoch integer NOT NULL CHECK(epoch=2));
             CREATE TABLE public.cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL);
             REVOKE ALL ON public.cantos_schema,public.cantos_migrations FROM PUBLIC;",
        ).await?;
        tx.batch_execute(BASELINE).await?;
        tx.execute(
            "INSERT INTO public.cantos_schema VALUES(true,$1)",
            &[&SCHEMA_EPOCH],
        )
        .await?;
        tx.execute(
            "INSERT INTO public.cantos_migrations VALUES(1,$1)",
            &[&Sha256::digest(BASELINE.as_bytes()).to_vec()],
        )
        .await?;
    }
    tx.commit().await?;
    Ok(())
}

/// Read-only startup/preflight check; the runtime role receives SELECT on both registry tables.
/// This never bootstraps even an empty database and never takes DDL authority.
pub async fn verify_schema(config: &Config) -> Result<(), SchemaError> {
    let (mut client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let tx = client.build_transaction().read_only(true).start().await?;
    supported_server(&tx).await?;
    if !marker_exists(&tx).await? {
        unmarked_database(&tx).await?;
        return Err(SchemaError::IncompatibleEpoch);
    }
    check_registry(&tx).await?;
    tx.commit().await?;
    Ok(())
}
