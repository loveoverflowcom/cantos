//! Native equivalents of the retained schema oracles. No PG epoch is executed or converted.
use cantos_server::{
    platform::turso::{ServerDatabase, ServerDatabaseError},
    test_support::ServerFixture,
};
use cantos_store_turso::{
    clock::SystemClock,
    error::{OpenError, StoreFault},
    registry::{Intent, Refusal},
};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
fn bytes(path: &Path) -> (Vec<u8>, Option<Vec<u8>>) {
    let mut wal = path.as_os_str().to_owned();
    wal.push("-wal");
    (
        std::fs::read(path).unwrap(),
        std::fs::read(PathBuf::from(wal)).ok(),
    )
}
async fn rejected(path: &Path, intent: Intent) -> ServerDatabaseError {
    match ServerDatabase::open(path, intent, Arc::new(SystemClock)).await {
        Err(e) => e,
        Ok(_) => panic!("damaged history admitted"),
    }
}
#[tokio::test]
async fn fresh_epoch_has_only_current_tables_and_replay_preserves_authoritative_rows() {
    let mut f = ServerFixture::new().await;
    f.seed_actors(&["minh"]).await;
    let actual: std::collections::BTreeSet<String> = f
        .rows(
            "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%'",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.try_get(0).unwrap())
        .collect();
    let inventory: serde_json::Value =
        serde_json::from_str(include_str!("../schema/native-table-ownership.json")).unwrap();
    let mut expected: std::collections::BTreeSet<String> = inventory["dispositions"]["server"]
        .as_array()
        .unwrap()
        .iter()
        .map(|v| v.as_str().unwrap().to_owned())
        .collect();
    // Turso-only tables with no retained PostgreSQL declaration (ownership: check_repository TABLE_OWNER).
    expected.extend([
        "cantos_schema_history".into(),
        "catalog_sample_metadata_disclosures".into(),
        "development_sessions".into(),
        "media_cover_disclosures".into(),
    ]);
    assert_eq!(actual, expected);
    let history=f.rows("SELECT version,name,hex(checksum),applied_at FROM cantos_schema_history ORDER BY version",&[]).await.unwrap().iter().map(|r|(r.try_get::<_,i64>(0).unwrap(),r.try_get::<_,String>(1).unwrap(),r.try_get::<_,String>(2).unwrap(),r.try_get::<_,i64>(3).unwrap())).collect::<Vec<_>>();
    f.close();
    for _ in 0..2 {
        drop(
            ServerDatabase::open(f.database_path(), Intent::Migrate, Arc::new(SystemClock))
                .await
                .unwrap(),
        );
    }
    f.restart().await;
    assert_eq!(
        f.count("SELECT count(*) FROM actors WHERE id='minh'", &[])
            .await,
        1
    );
    let after=f.rows("SELECT version,name,hex(checksum),applied_at FROM cantos_schema_history ORDER BY version",&[]).await.unwrap().iter().map(|r|(r.try_get::<_,i64>(0).unwrap(),r.try_get::<_,String>(1).unwrap(),r.try_get::<_,String>(2).unwrap(),r.try_get::<_,i64>(3).unwrap())).collect::<Vec<_>>();
    assert_eq!(after, history);
}
#[tokio::test]
async fn changed_checksum_and_registry_gaps_are_refused_without_repair() {
    for (mutation, expected) in [
        (
            "DELETE FROM cantos_schema_history",
            Refusal::HistoryDiverged { version: 1 },
        ),
        (
            "DELETE FROM cantos_schema_history WHERE version=2",
            Refusal::HistoryDiverged { version: 2 },
        ),
        (
            "UPDATE cantos_schema_history SET checksum=zeroblob(32) WHERE version=1",
            Refusal::HistoryDiverged { version: 1 },
        ),
    ] {
        let mut f = ServerFixture::new().await;
        f.seed_actors(&["minh"]).await;
        f.exec("DROP TRIGGER cantos_schema_history_no_update", &[])
            .await
            .unwrap();
        f.exec("DROP TRIGGER cantos_schema_history_no_delete", &[])
            .await
            .unwrap();
        f.exec(mutation, &[]).await.unwrap();
        f.close();
        let before = bytes(&f.database_path());
        for intent in [Intent::Serve, Intent::Migrate] {
            assert_eq!(
                rejected(&f.database_path(), intent).await,
                ServerDatabaseError::Open(OpenError::Refused(expected))
            );
            assert_eq!(bytes(&f.database_path()), before);
        }
    }
}
#[tokio::test]
async fn malformed_registry_values_return_typed_errors_without_panicking_or_mutation() {
    for values in [
        "NULL,'name',zeroblob(32),0",
        "4294967296,'name',zeroblob(32),0",
        "1,NULL,zeroblob(32),0",
        "1,'name','not-a-blob',0",
        "1,'name',NULL,0",
        "1,'name',zeroblob(31),0",
    ] {
        let mut f = ServerFixture::new().await;
        f.seed_actors(&["minh"]).await;
        f.exec("DROP TABLE cantos_schema_history", &[])
            .await
            .unwrap();
        // A deliberately malformed fixture table, beyond the production STRICT/guard barrier.
        f.exec(
            "CREATE TABLE cantos_schema_history(version,name,checksum,applied_at)",
            &[],
        )
        .await
        .unwrap();
        f.exec(
            &format!("INSERT INTO cantos_schema_history VALUES({values})"),
            &[],
        )
        .await
        .unwrap();
        f.close();
        let before = bytes(&f.database_path());
        for intent in [Intent::Serve, Intent::Migrate] {
            assert_eq!(
                rejected(&f.database_path(), intent).await,
                ServerDatabaseError::Open(OpenError::Store(StoreFault::Corrupt))
            );
            assert_eq!(bytes(&f.database_path()), before);
        }
    }
}
#[test]
fn retained_epoch_two_registered_sql_remains_exactly_its_reviewed_history() {
    use sha2::{Digest, Sha256};
    for (sql, expected) in [
        (
            include_bytes!("../schema/0001_current.sql").as_slice(),
            "9ed5f5aaec890c5e0df054ae49446ec027306401ac2dc7578ac35fec288bfe6e",
        ),
        (
            include_bytes!("../schema/0002_bearer.sql").as_slice(),
            "cf1f462ccfbae1ac11d6171d3de86828c0b4ddae59cb61eb082d6c0ec84c940f",
        ),
        (
            include_bytes!("../schema/0003_listener_bookmarks.sql").as_slice(),
            "e7beb784e48b4a9b294c9fc09157ef4dfac9f5e2b05bab3bdb1772344264193d",
        ),
        (
            include_bytes!("../schema/0004_identity_sessions.sql").as_slice(),
            "c0f6a7eea8f343ebab29f46986bd2fc826cca8278f24b4bee014971714234852",
        ),
    ] {
        assert_eq!(format!("{:x}", Sha256::digest(sql)), expected);
    }
    // The retained operator prefix lane still binds the same four immutable checksums.
    let registry = include_str!("../../../infra/vps/postgres/epoch2-prefix-registry.tsv");
    assert_eq!(registry.lines().count(), 4);
}
