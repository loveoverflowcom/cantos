//! Cross-context historical migration fixtures on the actual pinned engine.
//! Test-only module declared by platform/turso/tests.rs; never part of a runtime build.
use super::*;

async fn history_of(owner: &OwnedDatabase) -> Vec<Vec<Value>> {
    let reader = owner.reader().await.unwrap();
    let mut rows = reader
        .query(
            "SELECT version,name,checksum FROM cantos_schema_history ORDER BY version",
            (),
        )
        .await
        .unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push((0..3).map(|index| row.get_value(index).unwrap()).collect());
    }
    out
}

/// Historical upgrade evidence starts with the exact registered prefix, never a fresh v24
/// file called a v23 fixture. This snapshot covers opaque Unicode/BLOB revisions and both
/// live and revoked authority; the separate Linux rehearsal owns the real two-image journey.
#[tokio::test]
async fn historical_v23_upgrade_preserves_rows_checksums_and_revocation_and_is_repeatable() {
    let run = TestRun::new("historical-v23-v24");
    let path = run.path("server.db");
    let old_registry =
        Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..23]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(
        &path,
        old_registry,
        Intent::Migrate,
        clock(),
    ))
    .await
    .unwrap();
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id,active) VALUES('alice',1),('bob',0)",
        (),
    )
    .await
    .unwrap();
    tx.execute(
        &format!("INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('cantos-identity','{SUBJECT}','alice',7)"),
        (),
    )
    .await
    .unwrap();
    tx.execute(
        SESSION_INSERT,
        params_from_iter(session(S1, "theatre-native")),
    )
    .await
    .unwrap();
    tx.execute(
        SESSION_INSERT,
        params_from_iter(with(
            with(session(S2, "theatre-web"), 11, Value::Integer(1)),
            12,
            text("logout"),
        )),
    )
    .await
    .unwrap();
    let work = "12345678-1234-1234-1234-123456789abc";
    tx.execute(
        "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES(?1,'alice','story','draft',1)",
        [work],
    )
    .await
    .unwrap();
    let snapshot = "{\"title\":\"Trăng bên cửa sổ\",\"note\":\"e\u{301} / é\"}";
    tx.execute(
        "INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,1,?2,?3,'alice')",
        params_from_iter([text(work), Value::Blob(snapshot.as_bytes().to_vec()), text(&"a".repeat(64))]),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let history = history_of(&old).await;
    old.check_integrity().await.unwrap();
    drop(old);
    let before = bytes(&path);
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 23,
            latest: 24
        }))
    );
    assert_eq!(bytes(&path), before);
    for _ in 0..2 {
        let upgraded = open(&path, Intent::Migrate).await.unwrap();
        upgraded.owner().check_integrity().await.unwrap();
        assert_eq!(history_of(upgraded.owner()).await[..23], history);
        assert_eq!(
            values(&upgraded, "SELECT snapshot FROM catalog_work_revisions").await,
            vec![vec![Value::Blob(snapshot.as_bytes().to_vec())]]
        );
        assert_eq!(
            values(&upgraded, "SELECT id,active FROM actors ORDER BY id").await,
            vec![
                vec![text("alice"), Value::Integer(1)],
                vec![text("bob"), Value::Integer(0)]
            ]
        );
        assert_eq!(
            values(
                &upgraded,
                "SELECT id,revoked,revoked_reason FROM identity_sessions ORDER BY id"
            )
            .await,
            vec![
                vec![text(S2), Value::Integer(1), text("logout")],
                vec![text(S1), Value::Integer(0), Value::Null]
            ]
        );
        assert_eq!(
            values(&upgraded, "SELECT count(*) FROM media_cover_disclosures").await,
            vec![vec![Value::Integer(0)]]
        );
    }
    match OwnedDatabase::open(OpenRequest::new(
        &path,
        old_registry,
        Intent::Serve,
        clock(),
    ))
    .await
    {
        Err(error) => assert_eq!(
            error,
            OpenError::Refused(Refusal::NewerThanBinary {
                applied: 24,
                known: 23
            })
        ),
        Ok(_) => panic!("an image-only rollback must refuse v24"),
    }
}

#[tokio::test]
async fn historical_v24_ddl_failure_keeps_v23_history_and_repeated_attempts_cannot_claim_success() {
    let run = TestRun::new("historical-v24-failure");
    let path = run.path("server.db");
    let old_registry =
        Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..23]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(
        &path,
        old_registry,
        Intent::Migrate,
        clock(),
    ))
    .await
    .unwrap();
    let tx = old.begin_write().await.unwrap();
    // A conflicting unregistered object injects a real engine DDL error; no registered
    // migration/checksum is edited and no SQLite substitute is involved.
    tx.execute(
        "CREATE TABLE media_cover_disclosures(conflict INTEGER) STRICT",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let history = history_of(&old).await;
    drop(old);
    for _ in 0..2 {
        assert_eq!(
            refused(&path, Intent::Migrate).await,
            ServerDatabaseError::Open(OpenError::MigrationFailed {
                version: 24,
                fault: StoreFault::Engine
            })
        );
        let old = OwnedDatabase::open(OpenRequest::new(
            &path,
            old_registry,
            Intent::Serve,
            clock(),
        ))
        .await
        .unwrap();
        assert_eq!(history_of(&old).await, history);
        old.check_integrity().await.unwrap();
    }
}
