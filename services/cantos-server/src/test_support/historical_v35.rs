//! Exact v27 prefix to v35 on real Turso. Synthetic files only, never live restore evidence.
use super::{
    historical::{history_of, owner_values},
    *,
};
use std::collections::BTreeMap;
use std::os::unix::fs::DirBuilderExt;

const WORK: &str = "12345678-1234-1234-1234-123456789abc";
const IMAGE: &str = "20000000-0000-4000-8000-000000000001";

#[test]
fn v27_prefix_matches_the_pinned_running_source_checksums() {
    let pinned: serde_json::Value =
        serde_json::from_str(include_str!("v27-registry.json")).unwrap();
    assert_eq!(
        pinned["source_sha"],
        "89c5964c89cdbb0c50397f7d1263af34a5c3e517"
    );
    let rows = pinned["migrations"].as_array().unwrap();
    assert_eq!(rows.len(), 27);
    for (migration, row) in MIGRATIONS[..27].iter().zip(rows) {
        assert_eq!(row["version"], migration.version);
        assert_eq!(row["name"], migration.name);
        let checksum = migration
            .checksum()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>();
        assert_eq!(row["checksum"], checksum);
    }
}

fn v27() -> Registry {
    Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..27]).unwrap()
}

async fn seed(owner: &OwnedDatabase) {
    let tx = owner.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO actors(id,active) VALUES('alice',1),('bob',0)",
        (),
    )
    .await
    .unwrap();
    tx.execute(&format!("INSERT INTO actor_external_identities(issuer,subject,actor_id,linked_at) VALUES('cantos-identity','{SUBJECT}','alice',7)"), ())
        .await.unwrap();
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
    tx.execute(
        "INSERT INTO catalog_works VALUES(?1,'alice','story','draft',1)",
        [WORK],
    )
    .await
    .unwrap();
    let snapshot = "{\"title\":\"Trăng bên cửa sổ\",\"note\":\"e\u{301} / é\"}";
    let sha = "a".repeat(64);
    tx.execute("INSERT INTO catalog_work_revisions(work_id,revision,snapshot,snapshot_digest,recorded_by) VALUES(?1,1,?2,?3,'alice')",
        params_from_iter([text(WORK), Value::Blob(snapshot.as_bytes().to_vec()), text(&sha)]))
        .await.unwrap();
    tx.execute(
        "INSERT INTO catalog_members VALUES(?1,'bob','reader')",
        [WORK],
    )
    .await
    .unwrap();
    tx.execute("INSERT INTO catalog_operations VALUES('alice','10000000-0000-4000-8000-000000000001',?1,?1,X'00ff7b7d')", [sha.as_str()])
        .await.unwrap();
    tx.execute("INSERT INTO media_image_intents VALUES(?1,?2,'alice','10000000-0000-4000-8000-000000000002',?3,67,'image/png','cover-image-v1',0,1800000,X'7b7d',?4)",
        params_from_iter([text(IMAGE), text(WORK), text(&sha), text(&format!("ii-r1:sha256:{sha}"))]))
        .await.unwrap();
    tx.execute("INSERT INTO media_image_assets VALUES(?1,?1,?2,'alice','local-private',?3,?3,67,'image/png',2,3,'cover-image-v1',1,X'7b7d',?4)",
        params_from_iter([text(IMAGE), text(WORK), text(&sha), text(&format!("ia-r1:sha256:{sha}"))]))
        .await.unwrap();
    tx.execute("INSERT INTO media_work_artwork_changes VALUES(?1,1,'alice',?2,?3,'Ảnh cũ','alice',2,X'7b7d',?4)",
        params_from_iter([text(WORK), text(IMAGE), text(&format!("ia-r1:sha256:{sha}")), text(&format!("wa-r1:sha256:{sha}"))]))
        .await.unwrap();
    tx.execute("INSERT INTO media_cover_disclosures VALUES(?1,1,'alice',?2,'cleared',10000,'alice',3,'10000000-0000-4000-8000-000000000003',?3)",
        params_from_iter([text(WORK), text(&format!("ia-r1:sha256:{sha}")), text(&format!("image-operation1:sha256:{sha}"))]))
        .await.unwrap();
    tx.commit().await.unwrap();
    owner.check_integrity().await.unwrap();
}

async fn snapshot(owner: &OwnedDatabase, tables: &[String]) -> BTreeMap<String, Vec<Vec<Value>>> {
    let mut result = BTreeMap::new();
    for table in tables {
        assert!(table
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_'));
        let mut rows = owner_values(owner, &format!("SELECT * FROM \"{table}\"")).await;
        rows.sort_by_cached_key(|row| format!("{row:?}"));
        result.insert(table.clone(), rows);
    }
    result
}

#[tokio::test]
async fn v27_to_v35_preserves_every_baseline_table_and_revocation_and_is_repeatable() {
    let run = TestRun::new("historical-v27-v35");
    let path = run.path("server.db");
    let old = OwnedDatabase::open(OpenRequest::new(&path, v27(), Intent::Migrate, clock()))
        .await
        .unwrap();
    seed(&old).await;
    let tables = owner_values(&old, "SELECT name FROM sqlite_schema WHERE type='table' AND name NOT LIKE 'sqlite_%' AND name <> 'cantos_schema_history' ORDER BY name").await
        .into_iter().map(|row| match &row[0] { Value::Text(name) => name.clone(), _ => panic!("table name is text") }).collect::<Vec<_>>();
    let before = snapshot(&old, &tables).await;
    let history = history_of(&old).await;
    drop(old);
    let unchanged = bytes(&path);
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 27,
            latest: 35
        }))
    );
    assert_eq!(bytes(&path), unchanged);
    for _ in 0..2 {
        let current = open(&path, Intent::Migrate).await.unwrap();
        current.owner().check_integrity().await.unwrap();
        assert_eq!(snapshot(current.owner(), &tables).await, before);
        let applied = history_of(current.owner()).await;
        assert_eq!(applied.len(), 35);
        assert_eq!(applied[..27], history);
        assert_eq!(
            values(&current, "SELECT active FROM actors WHERE id='bob'").await,
            vec![vec![Value::Integer(0)]]
        );
        assert_eq!(
            values(
                &current,
                &format!("SELECT revoked,revoked_reason FROM identity_sessions WHERE id='{S2}'")
            )
            .await,
            vec![vec![Value::Integer(1), text("logout")]]
        );
        assert_eq!(
            attempt(
                &current,
                &format!(
                    "UPDATE identity_sessions SET revoked=0,revoked_reason=NULL WHERE id='{S2}'"
                ),
                vec![]
            )
            .await,
            Err(guard("immutable.identity_sessions.revocation"))
        );
        assert_eq!(
            attempt(&current, "UPDATE media_image_assets SET width_px=3", vec![]).await,
            Err(guard("immutable.private_media"))
        );
        assert_eq!(
            attempt(&current, "DELETE FROM media_work_artwork_changes", vec![]).await,
            Err(guard("immutable.private_media"))
        );
        // No new metadata publication, consent or viewer grant is inferred from legacy rows.
        for table in [
            "media_metadata_cover_bindings",
            "catalog_metadata_generations",
            "catalog_metadata_publication_events",
        ] {
            assert_eq!(
                values(&current, &format!("SELECT count(*) FROM {table}")).await,
                vec![vec![Value::Integer(0)]]
            );
        }
    }
    let before_old_serve = bytes(&path);
    match OwnedDatabase::open(OpenRequest::new(&path, v27(), Intent::Serve, clock())).await {
        Err(error) => assert_eq!(
            error,
            OpenError::Refused(Refusal::NewerThanBinary {
                applied: 35,
                known: 27
            })
        ),
        Ok(_) => panic!("image-only rollback cannot serve migrated data"),
    }
    assert_eq!(bytes(&path), before_old_serve);
}

#[tokio::test]
async fn v35_failure_keeps_committed_prefix_and_data_without_claiming_completion() {
    let run = TestRun::new("historical-v35-failure");
    let path = run.path("server.db");
    let old = OwnedDatabase::open(OpenRequest::new(&path, v27(), Intent::Migrate, clock()))
        .await
        .unwrap();
    seed(&old).await;
    // A real DDL conflict at v28, without changing registered SQL/checksums.
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "CREATE TABLE media_metadata_cover_bindings(conflict INTEGER) STRICT",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let history = history_of(&old).await;
    let before = owner_values(
        &old,
        "SELECT id,revoked,revoked_reason FROM identity_sessions ORDER BY id",
    )
    .await;
    drop(old);
    for _ in 0..2 {
        assert_eq!(
            refused(&path, Intent::Migrate).await,
            ServerDatabaseError::Open(OpenError::MigrationFailed {
                version: 28,
                fault: StoreFault::Engine
            })
        );
        let old = OwnedDatabase::open(OpenRequest::new(&path, v27(), Intent::Serve, clock()))
            .await
            .unwrap();
        old.check_integrity().await.unwrap();
        assert_eq!(history_of(&old).await, history);
        assert_eq!(
            owner_values(
                &old,
                "SELECT id,revoked,revoked_reason FROM identity_sessions ORDER BY id"
            )
            .await,
            before
        );
    }
}

#[tokio::test]
async fn v32_failure_retains_the_completed_suffix_and_never_permits_image_only_rollback() {
    let run = TestRun::new("historical-v35-partial-failure");
    let path = run.path("server.db");
    let old = OwnedDatabase::open(OpenRequest::new(&path, v27(), Intent::Migrate, clock()))
        .await
        .unwrap();
    seed(&old).await;
    let tx = old.begin_write().await.unwrap();
    tx.execute(
        "CREATE TABLE catalog_v32_events(conflict INTEGER) STRICT",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let original = history_of(&old).await;
    drop(old);
    let v31 = Registry::new(SERVER_APPLICATION_ID, SERVER_EPOCH, &MIGRATIONS[..31]).unwrap();
    let mut committed = None;
    for _ in 0..2 {
        assert_eq!(
            refused(&path, Intent::Migrate).await,
            ServerDatabaseError::Open(OpenError::MigrationFailed {
                version: 32,
                fault: StoreFault::Engine
            })
        );
        let partial = OwnedDatabase::open(OpenRequest::new(&path, v31, Intent::Serve, clock()))
            .await
            .unwrap();
        partial.check_integrity().await.unwrap();
        let history = history_of(&partial).await;
        assert_eq!(history.len(), 31);
        assert_eq!(history[..27], original);
        if let Some(before) = &committed {
            assert_eq!(&history, before);
        }
        committed = Some(history);
        assert_eq!(
            owner_values(
                &partial,
                &format!("SELECT revoked,revoked_reason FROM identity_sessions WHERE id='{S2}'")
            )
            .await,
            vec![vec![Value::Integer(1), text("logout")]]
        );
        assert_eq!(
            owner_values(&partial, "SELECT id,content_type FROM media_image_assets").await,
            vec![vec![text(IMAGE), text("image/png")]]
        );
    }
    match OwnedDatabase::open(OpenRequest::new(&path, v27(), Intent::Serve, clock())).await {
        Err(error) => assert_eq!(
            error,
            OpenError::Refused(Refusal::NewerThanBinary {
                applied: 31,
                known: 27
            })
        ),
        Ok(_) => panic!("partial migration requires forward repair, never an old-image restart"),
    }
}

/// Counterexample, not recovery acceptance: an internally consistent older directory
/// lacks post-cut revocation. Running all new DDL cannot reconstruct that missing fact.
#[tokio::test]
async fn migrating_a_stale_copy_cannot_reconstruct_post_cut_revocation() {
    let run = TestRun::new("historical-v35-recovery-counterexample");
    let live_root = run.path("live");
    let copy_root = run.path("stale");
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&live_root)
        .unwrap();
    std::fs::DirBuilder::new()
        .mode(0o700)
        .create(&copy_root)
        .unwrap();
    let live = live_root.join("server.db");
    let copy = copy_root.join("server.db");
    let old = OwnedDatabase::open(OpenRequest::new(&live, v27(), Intent::Migrate, clock()))
        .await
        .unwrap();
    seed(&old).await;
    drop(old);
    // Copy the entire stopped, run-owned directory, not a live DB/WAL selection.
    for entry in std::fs::read_dir(&live_root).unwrap() {
        let entry = entry.unwrap();
        assert!(entry.file_type().unwrap().is_file());
        std::fs::copy(entry.path(), copy_root.join(entry.file_name())).unwrap();
    }
    let current = OwnedDatabase::open(OpenRequest::new(&live, v27(), Intent::Serve, clock()))
        .await
        .unwrap();
    let tx = current.begin_write().await.unwrap();
    tx.execute(
        &format!("UPDATE identity_sessions SET revoked=1,revoked_reason='logout' WHERE id='{S1}'"),
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(
        owner_values(
            &current,
            &format!("SELECT revoked FROM identity_sessions WHERE id='{S1}'")
        )
        .await,
        vec![vec![Value::Integer(1)]]
    );
    let stale = open(&copy, Intent::Migrate).await.unwrap();
    stale.owner().check_integrity().await.unwrap();
    assert_eq!(history_of(stale.owner()).await.len(), 35);
    // This demonstrates the unsafe state. It must never be labelled restore PASS.
    assert_eq!(
        values(
            &stale,
            &format!("SELECT revoked FROM identity_sessions WHERE id='{S1}'")
        )
        .await,
        vec![vec![Value::Integer(0)]]
    );
}
