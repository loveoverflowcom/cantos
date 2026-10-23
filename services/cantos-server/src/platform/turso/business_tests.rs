//! Versions 12-15 (the CMP business API closure of #333) on the real pinned engine and run-owned
//! files: the upgrade from the exact registry of eleven that was deployed before them, and the
//! refusal of a file written by a newer or diverged registry. The table-level barriers of the
//! closure are `tests/server_turso_barriers.rs`.
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

#[tokio::test]
async fn the_registry_of_eleven_upgrades_to_the_business_closure_without_rewriting_a_checksum() {
    let run = TestRun::new("server-schema-business-upgrade");
    let path = run.path("server.db");
    let v11 = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..11]).unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v11, Intent::Migrate, clock()))
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
    // A revoked session beside a live one: the revocation must survive the upgrade.
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
    tx.execute(
        "INSERT INTO development_sessions(token_hash,actor_id,expires_at,revoked,session_id) VALUES(zeroblob(32),'alice',9,1,?1)",
        params_from_iter([text(S3)]),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let registered = history_of(&old).await;
    assert_eq!(registered.len(), 11);
    drop(old);
    let before = bytes(&path);

    // Serving never upgrades: the file of eleven is refused unchanged until an explicit migration.
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 11,
            latest: 15,
        }))
    );
    assert_eq!(bytes(&path), before);

    drop(open(&path, Intent::Migrate).await.unwrap());
    let database = open(&path, Intent::Serve).await.unwrap();
    let after = values(
        &database,
        "SELECT version,name,checksum FROM cantos_schema_history ORDER BY version",
    )
    .await;
    // Versions 1-11 keep the exact name and checksum they were registered with.
    assert_eq!(after[..11], registered[..]);
    assert_eq!(
        after[11..]
            .iter()
            .map(|row| row[1].clone())
            .collect::<Vec<_>>(),
        [
            "server_catalog_publication_read",
            "server_production_briefs",
            "server_audio_release_chain",
            "server_listener_library"
        ]
        .map(text)
    );
    assert_eq!(
        values(&database, "SELECT id,active FROM actors ORDER BY id").await,
        [
            [text("alice"), Value::Integer(1)],
            [text("bob"), Value::Integer(0)]
        ]
    );
    assert_eq!(
        values(
            &database,
            "SELECT id,channel,revoked,revoked_reason FROM identity_sessions ORDER BY id"
        )
        .await,
        [
            vec![
                text(S2),
                text("theatre-web"),
                Value::Integer(1),
                text("logout")
            ],
            vec![
                text(S1),
                text("theatre-native"),
                Value::Integer(0),
                Value::Null
            ],
        ]
    );
    assert_eq!(
        values(&database, "SELECT revoked FROM development_sessions").await,
        [[Value::Integer(1)]]
    );
    for table in [
        "catalog_works",
        "catalog_units",
        "production_briefs",
        "audio_releases",
        "audio_publication_pointers",
        "listener_state",
        "listener_progress",
    ] {
        assert_eq!(
            values(&database, &format!("SELECT count(*) FROM {table}")).await,
            [[Value::Integer(0)]],
            "{table} starts empty"
        );
    }
}

#[tokio::test]
async fn a_binary_of_eleven_refuses_the_file_of_fifteen_and_a_diverged_slice_is_never_adopted() {
    let run = TestRun::new("server-schema-business-refusals");
    let path = run.path("server.db");
    drop(open(&path, Intent::Migrate).await.unwrap());
    let before = bytes(&path);
    // What a binary that predates this slice would be: it knows eleven versions only.
    let old = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..11]).unwrap();
    for intent in [Intent::Serve, Intent::Migrate] {
        match OwnedDatabase::open(OpenRequest::new(&path, old, intent, clock())).await {
            Err(OpenError::Refused(Refusal::NewerThanBinary {
                applied: 15,
                known: 11,
            })) => {}
            Err(other) => panic!("{other:?}"),
            Ok(_) => panic!("an older binary must not open a newer file"),
        }
    }
    assert_eq!(bytes(&path), before);
    // A file whose version 12 differs from the registered bytes is diverged, not adopted.
    let altered: &'static [Migration] = Box::leak(
        MIGRATIONS
            .iter()
            .copied()
            .map(|migration| {
                if migration.version == 12 {
                    Migration {
                        sql: "CREATE TABLE diverged_slice(id TEXT PRIMARY KEY) STRICT;",
                        ..migration
                    }
                } else {
                    migration
                }
            })
            .collect::<Vec<_>>()
            .into_boxed_slice(),
    );
    let diverged = run.path("diverged.db");
    foreign(
        &diverged,
        Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, altered).unwrap(),
    )
    .await;
    let before = bytes(&diverged);
    assert!(matches!(
        refused(&diverged, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::HistoryDiverged { version: 12 }))
    ));
    assert_eq!(bytes(&diverged), before);
}
