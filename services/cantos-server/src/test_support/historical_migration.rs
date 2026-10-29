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

async fn owner_values(owner: &OwnedDatabase, sql: &str) -> Vec<Vec<Value>> {
    let reader = owner.reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let mut out = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        out.push(
            (0..rows.column_count())
                .map(|i| row.get_value(i).unwrap())
                .collect(),
        );
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
    let v24_registry =
        Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..24]).unwrap();
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
    match OwnedDatabase::open(OpenRequest::new(
        &path,
        v24_registry,
        Intent::Serve,
        clock(),
    ))
    .await
    {
        Err(error) => assert_eq!(
            error,
            OpenError::Refused(Refusal::PendingMigrations {
                applied: 23,
                latest: 24
            })
        ),
        Ok(_) => panic!("v24 serving must refuse a v23 file"),
    }
    assert_eq!(bytes(&path), before);
    for _ in 0..2 {
        let upgraded = OwnedDatabase::open(OpenRequest::new(
            &path,
            v24_registry,
            Intent::Migrate,
            clock(),
        ))
        .await
        .unwrap();
        upgraded.check_integrity().await.unwrap();
        assert_eq!(history_of(&upgraded).await[..23], history);
        assert_eq!(
            owner_values(&upgraded, "SELECT snapshot FROM catalog_work_revisions").await,
            vec![vec![Value::Blob(snapshot.as_bytes().to_vec())]]
        );
        assert_eq!(
            owner_values(&upgraded, "SELECT id,active FROM actors ORDER BY id").await,
            vec![
                vec![text("alice"), Value::Integer(1)],
                vec![text("bob"), Value::Integer(0)]
            ]
        );
        assert_eq!(
            owner_values(
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
            owner_values(&upgraded, "SELECT count(*) FROM media_cover_disclosures").await,
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

#[tokio::test]
async fn v24_to_v27_preserves_history_and_existing_actor_and_refuses_old_serving() {
    let run = TestRun::new("sample-metadata-v24-v27");
    let path = run.path("server.db");
    let v24 = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..24]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v24, Intent::Migrate, clock()))
        .await
        .unwrap();
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id,active) VALUES('alice',1),('bob',0)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let prior = history_of(&old).await;
    drop(old);
    let before = bytes(&path);
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 24,
            latest: u32::try_from(MIGRATIONS.len()).unwrap(),
        }))
    );
    assert_eq!(bytes(&path), before);
    for _ in 0..2 {
        let current = open(&path, Intent::Migrate).await.unwrap();
        current.owner().check_integrity().await.unwrap();
        assert_eq!(history_of(current.owner()).await[..24], prior);
        assert_eq!(
            values(&current, "SELECT id,active FROM actors ORDER BY id").await,
            vec![
                vec![text("alice"), Value::Integer(1)],
                vec![text("bob"), Value::Integer(0)]
            ]
        );
        assert_eq!(
            values(
                &current,
                "SELECT count(*) FROM catalog_sample_metadata_disclosures"
            )
            .await,
            vec![vec![Value::Integer(0)]]
        );
    }
    match OwnedDatabase::open(OpenRequest::new(&path, v24, Intent::Serve, clock())).await {
        Err(error) => assert_eq!(
            error,
            OpenError::Refused(Refusal::NewerThanBinary {
                applied: u32::try_from(MIGRATIONS.len()).unwrap(),
                known: 24
            })
        ),
        Ok(_) => panic!("v24 must refuse a migrated file"),
    }
}

#[tokio::test]
async fn v26_to_v27_preserves_existing_sample_actor_and_action() {
    let run = TestRun::new("fictional-sample-v26-v27");
    let path = run.path("server.db");
    let v26 = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..26]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v26, Intent::Migrate, clock()))
        .await
        .unwrap();
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id) VALUES ('cantos_sample_seed_v1')",
        (),
    )
    .await
    .unwrap();
    tx.execute("INSERT INTO sample_seed_principals VALUES ('cantos_sample_seed_v1','d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212')", ()).await.unwrap();
    tx.execute("INSERT INTO catalog_sample_seed_actions VALUES ('00000000-0000-4000-8000-000000000001','cantos_sample_seed_v1','publish','d0bca64204bebcd169752fbb325b5a79cd1314e121c6e7508db45fc8ab848212','synthetic operator','synthetic evidence','synthetic statement','aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa',1)", ()).await.unwrap();
    tx.commit().await.unwrap();
    let prior = history_of(&old).await;
    drop(old);
    let current = open(&path, Intent::Migrate).await.unwrap();
    current.owner().check_integrity().await.unwrap();
    assert_eq!(history_of(current.owner()).await[..26], prior);
    assert_eq!(
        values(
            &current,
            "SELECT actor_id,action FROM catalog_sample_seed_actions"
        )
        .await,
        vec![vec![text("cantos_sample_seed_v1"), text("publish")]]
    );
    assert_eq!(
        values(&current, "SELECT count(*) FROM fictional_seed_principals").await,
        vec![vec![Value::Integer(0)]]
    );
}

#[tokio::test]
async fn v28_to_v29_preserves_existing_artwork_rows_and_admits_avif() {
    let run = TestRun::new("media-avif-v28-v29");
    let path = run.path("server.db");
    let v28 = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..28]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v28, Intent::Migrate, clock()))
        .await
        .unwrap();
    let work = "12345678-1234-1234-1234-123456789abc";
    let intent = "20000000-0000-4000-8000-000000000001";
    let op = "10000000-0000-4000-8000-000000000001";
    let sha = "a".repeat(64);
    let tx = old.begin_write().await.unwrap();
    tx.execute("INSERT INTO actors(id,active) VALUES('alice',1)", ())
        .await
        .unwrap();
    tx.execute(
        "INSERT INTO catalog_works(id,owner_id,kind,lifecycle,head_revision) VALUES(?1,'alice','story','draft',1)",
        [work],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,1,X'7b7d',?2,'alice')",
        params_from_iter([text(work), text(&sha)]),
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO media_image_intents VALUES(?1,?2,'alice',?3,?4,67,'image/png','cover-image-v1',0,1800000,X'7b7d',?5)",
        params_from_iter([
            text(intent),
            text(work),
            text(op),
            text(&sha),
            text(&format!("ii-r1:sha256:{sha}")),
        ]),
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO media_image_assets VALUES(?1,?1,?2,'alice','local-private',?3,?3,67,'image/png',2,3,'cover-image-v1',1,X'7b7d',?4)",
        params_from_iter([
            text(intent),
            text(work),
            text(&sha),
            text(&format!("ia-r1:sha256:{sha}")),
        ]),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let prior = history_of(&old).await;
    drop(old);

    let current = open(&path, Intent::Migrate).await.unwrap();
    current.owner().check_integrity().await.unwrap();
    assert_eq!(history_of(current.owner()).await[..28], prior);
    assert_eq!(
        values(
            &current,
            "SELECT id,content_type FROM media_image_assets ORDER BY id"
        )
        .await,
        vec![vec![text(intent), text("image/png")]]
    );

    let avif_intent = "20000000-0000-4000-8000-000000000002";
    let avif_op = "10000000-0000-4000-8000-000000000002";
    let avif_sha = "b".repeat(64);
    let tx = current.owner().begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO media_image_intents VALUES(?1,?2,'alice',?3,?4,322,'image/avif','cover-image-v1',0,1800000,X'7b7d',?5)",
        params_from_iter([
            text(avif_intent),
            text(work),
            text(avif_op),
            text(&avif_sha),
            text(&format!("ii-r1:sha256:{avif_sha}")),
        ]),
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO media_image_assets VALUES(?1,?1,?2,'alice','local-private',?3,?3,322,'image/avif',2,3,'cover-image-v1',2,X'7b7d',?4)",
        params_from_iter([
            text(avif_intent),
            text(work),
            text(&avif_sha),
            text(&format!("ia-r1:sha256:{avif_sha}")),
        ]),
    )
    .await
    .unwrap();
    // v30 preserves the old rows, but direct SQL cannot add another legacy intent.
    let legacy_error = tx.execute(
        "INSERT INTO media_image_intents VALUES('20000000-0000-4000-8000-000000000099',?1,'alice','10000000-0000-4000-8000-000000000099',?2,67,'image/png','cover-image-v1',0,1800000,X'7b7d',?3)",
        params_from_iter([text(work), text(&sha), text(&format!("ii-r1:sha256:{sha}"))]),
    ).await.unwrap_err();
    assert_eq!(legacy_error, guard("policy.avif_only"));
    tx.commit().await.unwrap();
}
