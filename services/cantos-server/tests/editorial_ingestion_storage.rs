//! Engine-level Editorial/Ingestion barriers and v15→v16 compatibility. Raw SQL reaches
//! each CHECK/FK/guard independently of the adapter on real run-owned files.
use cantos_contracts::ReviewRequest;
use cantos_server::{
    platform::{
        store::StoreError,
        turso::{registry, ServerDatabase, ServerDatabaseError, SERVER_EPOCH},
    },
    test_support::{
        editorial::{fixture, import, request, ALICE, SCRIPT, T0},
        ServerFixture,
    },
};
use cantos_store_turso::{
    clock::Clock,
    error::{ConstraintKind, OpenError},
    registry::{DatabaseKind, Intent, Refusal, Registry},
    testing::{FixedClock, TestRun},
    OpenRequest, OwnedDatabase, Value,
};
use std::sync::Arc;
fn clock() -> Arc<dyn Clock> {
    FixedClock::new(T0)
}
async fn open(path: &std::path::Path, intent: Intent) -> Result<OwnedDatabase, OpenError> {
    OwnedDatabase::open(OpenRequest::new(path, registry().unwrap(), intent, clock())).await
}
async fn refused(path: &std::path::Path, intent: Intent) -> ServerDatabaseError {
    match ServerDatabase::open(path, intent, clock()).await {
        Err(e) => e,
        Ok(_) => panic!("file unexpectedly opened"),
    }
}
async fn values(owner: &OwnedDatabase, sql: &str) -> Vec<Vec<Value>> {
    let mut rows = owner.reader().await.unwrap().query(sql, ()).await.unwrap();
    let n = rows.column_names().len();
    let mut result = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        result.push((0..n).map(|i| row.get_value(i).unwrap()).collect());
    }
    result
}
#[tokio::test]
async fn version_fifteen_upgrades_by_suffix_and_preserves_existing_business_rows() {
    let run = TestRun::new("server-editorial-upgrade");
    let path = run.path("server.db");
    let v15 = Registry::new(
        DatabaseKind::Server,
        SERVER_EPOCH,
        &cantos_server::platform::turso::registry()
            .unwrap()
            .migrations()[..15],
    )
    .unwrap();
    let old = OwnedDatabase::open(OpenRequest::new(&path, v15, Intent::Migrate, clock()))
        .await
        .unwrap();
    let tx = old.begin_write().await.unwrap();
    tx.execute("INSERT INTO actors(id) VALUES('alice')", ())
        .await
        .unwrap();
    tx.execute(
        "INSERT INTO listener_state(listener_id,revision) VALUES('alice',7)",
        (),
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    let mut rows = old
        .reader()
        .await
        .unwrap()
        .query(
            "SELECT checksum FROM cantos_schema_history ORDER BY version",
            (),
        )
        .await
        .unwrap();
    let mut checksums = Vec::new();
    while let Some(row) = rows.next().await.unwrap() {
        checksums.push(row.get_value(0).unwrap());
    }
    drop(rows);
    drop(old);
    assert_eq!(
        refused(&path, Intent::Serve).await,
        ServerDatabaseError::Open(OpenError::Refused(Refusal::PendingMigrations {
            applied: 15,
            latest: 16
        }))
    );
    drop(open(&path, Intent::Migrate).await.unwrap());
    let database = open(&path, Intent::Serve).await.unwrap();
    assert_eq!(
        values(
            &database,
            "SELECT checksum FROM cantos_schema_history WHERE version<=15 ORDER BY version"
        )
        .await,
        checksums.into_iter().map(|v| vec![v]).collect::<Vec<_>>()
    );
    assert_eq!(
        values(
            &database,
            "SELECT revision FROM listener_state WHERE listener_id='alice'"
        )
        .await,
        [[Value::Integer(7)]]
    );
    drop(database);
    assert!(matches!(
        OwnedDatabase::open(OpenRequest::new(&path, v15, Intent::Serve, clock())).await,
        Err(OpenError::Refused(Refusal::NewerThanBinary {
            applied: 16,
            known: 15
        }))
    ));
}

async fn refused_sql(fixture: &ServerFixture, sql: &str, constraint: ConstraintKind) {
    let error = fixture.try_exec(sql, &[]).await.unwrap_err();
    assert_eq!(error.constraint(), Some(constraint), "{sql}: {error:?}");
}

#[tokio::test]
async fn settled_editorial_and_ingestion_rows_reject_update_delete_replace_and_upsert() {
    let fixture = fixture().await;
    fixture
        .store()
        .save(ALICE, SCRIPT, request(0))
        .await
        .unwrap();
    fixture
        .store()
        .review(
            ALICE,
            SCRIPT,
            ReviewRequest {
                revision: 1,
                operation_id: uuid::Uuid::new_v4().to_string(),
            },
        )
        .await
        .unwrap();
    fixture
        .store()
        .import_manuscript(ALICE, import())
        .await
        .unwrap();
    for (table, column, code) in [
        ("scripts", "id", "immutable.script_identity"),
        (
            "script_revisions",
            "revision",
            "immutable.editorial_history",
        ),
        ("script_evidence", "id", "immutable.editorial_history"),
        (
            "revision_evidence",
            "evidence_id",
            "immutable.editorial_history",
        ),
        ("script_reviews", "id", "immutable.editorial_history"),
        (
            "script_review_operations",
            "operation_id",
            "immutable.editorial_history",
        ),
        ("source_records", "id", "immutable.ingestion_history"),
    ] {
        let before = fixture
            .count(&format!("SELECT count(*) FROM {table}"), &[])
            .await;
        for sql in [
            if table == "scripts" { "UPDATE scripts SET owner_id='bob'".into() } else { format!("UPDATE {table} SET {column}={column}") },
            format!("DELETE FROM {table}"),
            format!("INSERT OR REPLACE INTO {table} SELECT * FROM {table} LIMIT 1"),
            format!("REPLACE INTO {table} SELECT * FROM {table} LIMIT 1"),
            format!("INSERT INTO {table} SELECT * FROM {table} WHERE 1 LIMIT 1 ON CONFLICT DO UPDATE SET {column}=excluded.{column}"),
        ] {
            let error = fixture.try_exec(&sql, &[]).await.expect_err(&sql);
            assert_eq!(error.guard(), Some(code), "{sql}: {error:?}");
        }
        assert_eq!(
            fixture
                .count(&format!("SELECT count(*) FROM {table}"), &[])
                .await,
            before
        );
    }
    // A different revision/source key must not evade the immutable operation-identity guard.
    for (sql, code) in [
        ("INSERT OR REPLACE INTO script_revisions SELECT script_id,revision+1,expected_revision+1,operation_id,accepted_by,accepted_at,schema_version,canonical_export,content_digest,export_digest FROM script_revisions", "immutable.editorial_history"),
        ("INSERT OR REPLACE INTO source_records SELECT owner_id,kind,'src_other',reference,original_text,sha256,recorded_at,original_bytes,import_metadata,import_outcome,import_digest,imported_by,import_operation_id FROM source_records", "immutable.ingestion_history"),
    ] {
        assert_eq!(fixture.try_exec(sql, &[]).await.unwrap_err().guard(), Some(code));
    }
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM script_revisions", &[])
            .await,
        1
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM source_records", &[])
            .await,
        1
    );
}

#[tokio::test]
async fn editorial_constraints_foreign_keys_and_zero_row_cas_are_engine_enforced() {
    let fixture = fixture().await;
    fixture
        .store()
        .save(ALICE, SCRIPT, request(0))
        .await
        .unwrap();
    let script = SCRIPT;
    for sql in [
        "INSERT INTO scripts(id,owner_id) VALUES('not-a-uuid','alice')".into(),
        "INSERT INTO script_evidence VALUES('alice','unknown','new','invalid kind')".into(),
        "INSERT INTO script_evidence VALUES('alice','rights','UPPER','invalid id')".into(),
        format!("INSERT INTO script_members VALUES('{script}','bob','owner')"),
        format!("UPDATE scripts SET head_revision=-1 WHERE id='{script}'"),
    ] {
        refused_sql(&fixture, &sql, ConstraintKind::Check).await;
    }
    for sql in [
        "INSERT INTO script_evidence VALUES('missing','rights','new','missing actor')".into(),
        format!("INSERT INTO script_members VALUES('{script}','missing','reader')"),
        "INSERT INTO script_members VALUES('00000000-0000-0000-0000-000000000099','bob','reader')".into(),
        format!("INSERT INTO revision_evidence VALUES('{script}',99,'alice','rights','rights-demo')"),
        format!("INSERT INTO revision_evidence VALUES('{script}',1,'alice','rights','missing')"),
        format!("INSERT INTO script_review_operations VALUES('{script}','alice','00000000-0000-0000-0000-000000000010',1)"),
        format!("UPDATE scripts SET head_revision=2 WHERE id='{script}'"),
    ] { refused_sql(&fixture, &sql, ConstraintKind::ForeignKey).await; }

    // A new row identity reaches each revision CHECK (duplicate guards would mask it otherwise).
    for (column, value) in [
        ("revision", "0"),
        ("expected_revision", "-1"),
        ("operation_id", "'invalid'"),
        ("accepted_at", "-1"),
        ("schema_version", "'0.2.0'"),
        ("canonical_export", "X''"),
        ("canonical_export", "zeroblob(2097153)"),
        ("content_digest", "'invalid'"),
        ("export_digest", "'invalid'"),
    ] {
        let columns = [
            "script_id",
            "revision",
            "expected_revision",
            "operation_id",
            "accepted_by",
            "accepted_at",
            "schema_version",
            "canonical_export",
            "content_digest",
            "export_digest",
        ];
        let values: Vec<_> = columns
            .iter()
            .map(|c| {
                if *c == column {
                    value.to_owned()
                } else if *c == "revision" {
                    "2".into()
                } else if *c == "expected_revision" {
                    "1".into()
                } else if *c == "operation_id" {
                    "'00000000-0000-0000-0000-000000000020'".into()
                } else {
                    (*c).to_owned()
                }
            })
            .collect();
        refused_sql(
            &fixture,
            &format!(
                "INSERT INTO script_revisions SELECT {} FROM script_revisions",
                values.join(",")
            ),
            ConstraintKind::Check,
        )
        .await;
    }
    for (column, value) in [
        ("script_id", "'00000000-0000-0000-0000-000000000099'"),
        ("accepted_by", "'missing'"),
    ] {
        let columns = [
            "script_id",
            "revision",
            "expected_revision",
            "operation_id",
            "accepted_by",
            "accepted_at",
            "schema_version",
            "canonical_export",
            "content_digest",
            "export_digest",
        ];
        let values: Vec<_> = columns
            .iter()
            .map(|c| {
                if *c == column {
                    value.to_owned()
                } else if *c == "revision" {
                    "2".into()
                } else if *c == "expected_revision" {
                    "1".into()
                } else if *c == "operation_id" {
                    "'00000000-0000-0000-0000-000000000020'".into()
                } else {
                    (*c).to_owned()
                }
            })
            .collect();
        refused_sql(
            &fixture,
            &format!(
                "INSERT INTO script_revisions SELECT {} FROM script_revisions",
                values.join(",")
            ),
            ConstraintKind::ForeignKey,
        )
        .await;
    }
    assert_eq!(
        fixture
            .exec(
                "UPDATE scripts SET head_revision=?1 WHERE id=?2 AND head_revision=?3",
                &[&2_i64, &SCRIPT, &99_i64]
            )
            .await
            .unwrap(),
        0
    );
    assert_eq!(
        fixture
            .count("SELECT head_revision FROM scripts", &[])
            .await,
        1
    );
}

#[tokio::test]
async fn stored_corruption_is_refused_on_revision_history_review_and_import_reads() {
    for assignment in [
        "canonical_export=X'7b7d'",
        "content_digest='sir-c1:sha256:0000000000000000000000000000000000000000000000000000000000000000'",
        "export_digest='sir-e1:sha256:0000000000000000000000000000000000000000000000000000000000000000'",
        "accepted_at=9223372036854775807",
    ] {
        let fixture = fixture().await;
        fixture.store().save(ALICE, SCRIPT, request(0)).await.unwrap();
        // Simulated operator/storage tampering, not an adapter path or a runtime bypass.
        fixture.exec("DROP TRIGGER script_revisions_no_update", &[]).await.unwrap();
        fixture.exec(&format!("UPDATE script_revisions SET {assignment}"), &[]).await.unwrap();
        assert!(matches!(fixture.store().load(ALICE, SCRIPT, None).await, Err(StoreError::CorruptRevision)));
        assert!(matches!(fixture.store().history(ALICE, SCRIPT, 0, 50).await, Err(StoreError::CorruptRevision)));
        assert!(matches!(fixture.store().review(ALICE, SCRIPT, ReviewRequest { revision: 1, operation_id: uuid::Uuid::new_v4().to_string() }).await, Err(StoreError::CorruptRevision)));
        assert_eq!(fixture.count("SELECT count(*) FROM script_reviews", &[]).await, 0);
    }
    for assignment in [
        "original_bytes=X'01'",
        "original_text=NULL",
        "import_metadata='{}'",
        "import_outcome='{}'",
        "import_operation_id='00000000-0000-0000-0000-000000000099'",
        "import_digest='0000000000000000000000000000000000000000000000000000000000000000'",
        "recorded_at=9223372036854775807",
    ] {
        let fixture = fixture().await;
        let source = fixture
            .store()
            .import_manuscript(ALICE, import())
            .await
            .unwrap();
        fixture
            .exec("DROP TRIGGER source_records_no_update", &[])
            .await
            .unwrap();
        fixture
            .exec(&format!("UPDATE source_records SET {assignment}"), &[])
            .await
            .unwrap();
        assert!(
            matches!(
                fixture.store().load_import(ALICE, &source.id).await,
                Err(StoreError::CorruptRevision)
            ),
            "{assignment}"
        );
        assert!(
            matches!(
                fixture.store().original_import(ALICE, &source.id).await,
                Err(StoreError::CorruptRevision)
            ),
            "{assignment}"
        );
    }
}

#[tokio::test]
async fn source_checks_keep_legacy_text_and_complete_imports_as_the_only_two_shapes() {
    let fixture = fixture().await;
    fixture
        .exec(
            "INSERT INTO script_evidence VALUES('alice','source','src_test','synthetic source')",
            &[],
        )
        .await
        .unwrap();
    let source_columns = [
        "owner_id",
        "kind",
        "id",
        "reference",
        "original_text",
        "sha256",
        "recorded_at",
        "original_bytes",
        "import_metadata",
        "import_outcome",
        "import_digest",
        "imported_by",
        "import_operation_id",
    ];
    let valid = [
        "'alice'",
        "'source'",
        "'src_test'",
        "'synthetic reference'",
        "'original'",
        "'0000000000000000000000000000000000000000000000000000000000000000'",
        "0",
        "X'00'",
        "'{}'",
        "'{}'",
        "'0000000000000000000000000000000000000000000000000000000000000000'",
        "'alice'",
        "'00000000-0000-0000-0000-000000000077'",
    ];
    for (column, value) in [
        ("kind", "'rights'"),
        ("reference", "''"),
        ("reference", "replace(hex(zeroblob(1025)),'0','x')"),
        ("original_text", "''"),
        ("original_text", "replace(hex(zeroblob(524289)),'0','x')"),
        ("sha256", "'INVALID'"),
        ("recorded_at", "-1"),
        ("original_bytes", "NULL"),
        ("original_bytes", "zeroblob(1048577)"),
        ("import_metadata", "NULL"),
        ("import_metadata", "'[]'"),
        ("import_metadata", "'{'"),
        ("import_outcome", "NULL"),
        ("import_outcome", "'1'"),
        ("import_outcome", "'{'"),
        ("import_digest", "NULL"),
        ("import_digest", "'invalid'"),
        ("imported_by", "NULL"),
        ("imported_by", "'bob'"),
        ("import_operation_id", "NULL"),
        ("import_operation_id", "'invalid'"),
    ] {
        let mut values = valid;
        values[source_columns.iter().position(|c| *c == column).unwrap()] = value;
        refused_sql(
            &fixture,
            &format!("INSERT INTO source_records VALUES({})", values.join(",")),
            ConstraintKind::Check,
        )
        .await;
    }
    let mut values = valid;
    values[2] = "'missing'";
    refused_sql(
        &fixture,
        &format!("INSERT INTO source_records VALUES({})", values.join(",")),
        ConstraintKind::ForeignKey,
    )
    .await;
    // Text-only rows from before import receipts remain valid. A null original has no source.
    for text in ["NULL", "''"] {
        let sql = format!("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','src_test','reference',{text},'0000000000000000000000000000000000000000000000000000000000000000',0)");
        refused_sql(&fixture, &sql, ConstraintKind::Check).await;
    }
    fixture.exec("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES('alice','src_test','reference','original','0000000000000000000000000000000000000000000000000000000000000000',0)", &[]).await.unwrap();
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM source_records", &[])
            .await,
        1
    );
}

#[tokio::test]
async fn review_keys_times_and_all_reference_targets_are_checked_by_the_engine() {
    let fixture = fixture().await;
    fixture
        .store()
        .save(ALICE, SCRIPT, request(0))
        .await
        .unwrap();
    let valid = [
        "'00000000-0000-0000-0000-000000000010'",
        "'00000000-0000-0000-0000-000000000001'",
        "1",
        "'00000000-0000-0000-0000-000000000011'",
        "'alice'",
        "0",
    ];
    for (column, value, constraint) in [
        (0, "'invalid'", ConstraintKind::Check),
        (3, "'invalid'", ConstraintKind::Check),
        (5, "-1", ConstraintKind::Check),
        (4, "'missing'", ConstraintKind::ForeignKey),
        (5, "NULL", ConstraintKind::NotNull),
        (2, "2", ConstraintKind::ForeignKey),
        (
            1,
            "'00000000-0000-0000-0000-000000000099'",
            ConstraintKind::ForeignKey,
        ),
    ] {
        let mut values = valid;
        values[column] = value;
        refused_sql(
            &fixture,
            &format!("INSERT INTO script_reviews VALUES({})", values.join(",")),
            constraint,
        )
        .await;
    }
    fixture
        .store()
        .review(
            ALICE,
            SCRIPT,
            ReviewRequest {
                revision: 1,
                operation_id: uuid::Uuid::new_v4().to_string(),
            },
        )
        .await
        .unwrap();
    for (actor, operation, constraint) in [
        ("alice", "invalid", ConstraintKind::Check),
        (
            "missing",
            "00000000-0000-0000-0000-000000000090",
            ConstraintKind::ForeignKey,
        ),
    ] {
        refused_sql(
            &fixture,
            &format!(
                "INSERT INTO script_review_operations VALUES('{SCRIPT}','{actor}','{operation}',1)"
            ),
            constraint,
        )
        .await;
    }
}
