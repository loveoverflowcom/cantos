//! Ordered, checksummed schema migrations applied by the migration actor.
use sha2::{Digest, Sha256};
use tokio_postgres::{Config, NoTls};

use super::StoreError;

const MIGRATIONS: [(i32, &str); 15] = [
    (
        1,
        include_str!("../../../migrations/0001_script_revisions.sql"),
    ),
    (
        2,
        include_str!("../../../migrations/0002_editorial_handoff.sql"),
    ),
    (
        3,
        include_str!("../../../migrations/0003_manuscript_import.sql"),
    ),
    (
        4,
        include_str!("../../../migrations/0004_ai_adaptation.sql"),
    ),
    (
        5,
        include_str!("../../../migrations/0005_caller_adaptation.sql"),
    ),
    (
        6,
        include_str!("../../../migrations/0006_production_inputs.sql"),
    ),
    (7, include_str!("../../../migrations/0007_durable_jobs.sql")),
    (8, include_str!("../../../migrations/0008_speech_clips.sql")),
    (
        9,
        include_str!("../../../migrations/0009_durable_step_changes.sql"),
    ),
    (
        10,
        include_str!("../../../migrations/0010_studio_audio_intake.sql"),
    ),
    (11, include_str!("../../../migrations/0011_catalog.sql")),
    (
        12,
        include_str!("../../../migrations/0012_content_identity.sql"),
    ),
    (
        13,
        include_str!("../../../migrations/0013_production_briefs.sql"),
    ),
    (
        14,
        include_str!("../../../migrations/0014_audio_lifecycle.sql"),
    ),
    (
        16,
        include_str!("../../../migrations/0016_audio_publication.sql"),
    ),
];

/// The migration actor owns DDL; the HTTP application role has no DDL rights.
pub async fn migrate(config: &Config) -> Result<(), StoreError> {
    let (mut client, connection) = config.connect(NoTls).await?;
    tokio::spawn(async move {
        let _ = connection.await;
    });
    let tx = client.transaction().await?;
    tx.batch_execute("SELECT pg_advisory_xact_lock(1128353364); CREATE TABLE IF NOT EXISTS cantos_migrations(version integer PRIMARY KEY, checksum bytea NOT NULL); REVOKE ALL ON cantos_migrations FROM PUBLIC;").await?;
    for (version, migration) in MIGRATIONS {
        let checksum = Sha256::digest(migration.as_bytes()).to_vec();
        match tx
            .query_opt(
                "SELECT checksum FROM cantos_migrations WHERE version=$1",
                &[&version],
            )
            .await?
        {
            Some(row) if row.get::<_, Vec<u8>>(0) == checksum => (),
            Some(_) => return Err(StoreError::CorruptRevision),
            None => {
                tx.batch_execute(migration).await?;
                tx.execute(
                    "INSERT INTO cantos_migrations VALUES($1,$2)",
                    &[&version, &checksum],
                )
                .await?;
            }
        }
    }
    tx.commit().await?;
    Ok(())
}
