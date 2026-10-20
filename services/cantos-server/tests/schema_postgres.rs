//! Current schema acceptance in an owned disposable cluster, never a live cutover.
use std::env;

use cantos_server::platform::postgres::{local_config, migrate, verify_schema, SchemaError};
use tokio_postgres::{Client, Config, NoTls};
use uuid::Uuid;

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

async fn database() -> (Config, Client) {
    let mut config = local_config(
        &env::var("CANTOS_TEST_CLUSTER_URL")
            .expect("use cargo xtask verify postgres --suite schema"),
    )
    .unwrap();
    assert_eq!(config.get_user(), Some("cantos_test_admin"));
    assert_eq!(config.get_dbname(), Some("postgres"));
    let name = format!("cantos_test_{}", Uuid::new_v4().simple());
    connect(&config)
        .await
        .batch_execute(&format!("CREATE DATABASE {name}"))
        .await
        .unwrap();
    config.dbname(&name);
    let admin = connect(&config).await;
    (config, admin)
}

async fn registry(admin: &Client) -> Vec<(i32, Vec<u8>)> {
    admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|r| (r.get(0), r.get(1)))
        .collect()
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn fresh_epoch_has_only_current_tables_and_replay_preserves_authoritative_rows() {
    let (config, admin) = database().await;
    migrate(&config).await.unwrap();
    verify_schema(&config).await.unwrap();
    let tables: Vec<String> = admin
        .query(
            "SELECT tablename FROM pg_tables WHERE schemaname='public' ORDER BY tablename",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    for current in [
        "catalog_works",
        "catalog_content_revisions",
        "production_briefs",
        "audio_submissions",
        "audio_releases",
        "listener_progress",
        "listener_bookmarks",
        "actor_external_identities",
        "theatre_firebase_enrollments",
        "adaptation_submissions",
    ] {
        assert!(
            tables.iter().any(|name| name == current),
            "missing {current}"
        );
    }
    assert!(!tables.iter().any(|name| name.starts_with("durable_")
        || [
            "speech_clips",
            "adaptation_attempts",
            "adaptation_observations"
        ]
        .contains(&name.as_str())));
    let columns: Vec<String> = admin
        .query(
            "SELECT column_name FROM information_schema.columns WHERE table_name='adaptation_runs'",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert!(!columns
        .iter()
        .any(|name| ["problem", "dispatch_deadline"].contains(&name.as_str())));
    admin
        .batch_execute("INSERT INTO actors(id) VALUES('schema-owner')")
        .await
        .unwrap();
    let before = registry(&admin).await;
    assert_eq!(before.len(), 3);
    migrate(&config).await.unwrap();
    assert_eq!(registry(&admin).await, before);
    assert_eq!(
        admin
            .query_one("SELECT id FROM actors", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "schema-owner"
    );
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn legacy_registry_is_rejected_without_touching_checksums_data_or_ddl() {
    let (config, admin) = database().await;
    admin.batch_execute("CREATE TABLE cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL);
        INSERT INTO cantos_migrations VALUES(1,decode(repeat('ab',32),'hex')),(28,decode(repeat('cd',32),'hex'));
        CREATE TABLE actors(id text PRIMARY KEY); INSERT INTO actors VALUES('preserve-me');").await.unwrap();
    let before = registry(&admin).await;
    assert_eq!(migrate(&config).await, Err(SchemaError::LegacyDatabase));
    assert_eq!(
        verify_schema(&config).await,
        Err(SchemaError::LegacyDatabase)
    );
    assert_eq!(registry(&admin).await, before);
    assert_eq!(
        admin
            .query_one("SELECT id FROM actors", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "preserve-me"
    );
    assert!(admin
        .query_one("SELECT to_regclass('public.cantos_schema') IS NULL", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn unmarked_objects_and_empty_runtime_startup_never_create_registry() {
    let (config, admin) = database().await;
    assert_eq!(
        verify_schema(&config).await,
        Err(SchemaError::IncompatibleEpoch)
    );
    for create in [
        "CREATE TABLE existing_data(id text)",
        "CREATE SCHEMA custom_data",
        "CREATE FUNCTION existing_function() RETURNS integer LANGUAGE sql AS 'SELECT 1'",
        "CREATE TYPE existing_enum AS ENUM ('preserved')",
    ] {
        let (config, admin) = database().await;
        admin.batch_execute(create).await.unwrap();
        assert_eq!(
            migrate(&config).await,
            Err(SchemaError::UnrecognizedDatabase)
        );
        assert!(admin.query_one("SELECT to_regclass('public.cantos_schema') IS NULL AND to_regclass('public.cantos_migrations') IS NULL", &[]).await.unwrap().get::<_,bool>(0));
    }
    assert!(admin
        .query_one("SELECT to_regclass('public.cantos_schema') IS NULL", &[])
        .await
        .unwrap()
        .get::<_, bool>(0));
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn changed_checksum_and_registry_gaps_are_refused_without_repair() {
    let (config, admin) = database().await;
    migrate(&config).await.unwrap();
    admin
        .batch_execute("UPDATE cantos_migrations SET checksum=decode(repeat('ef',32),'hex')")
        .await
        .unwrap();
    let before = registry(&admin).await;
    assert_eq!(migrate(&config).await, Err(SchemaError::ChecksumMismatch));
    assert_eq!(
        verify_schema(&config).await,
        Err(SchemaError::ChecksumMismatch)
    );
    assert_eq!(registry(&admin).await, before);
    admin
        .batch_execute("DELETE FROM cantos_migrations")
        .await
        .unwrap();
    assert_eq!(migrate(&config).await, Err(SchemaError::RegistryMismatch));
    assert!(registry(&admin).await.is_empty());
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn foreign_epoch_and_unexpected_version_are_refused() {
    let (config, admin) = database().await;
    migrate(&config).await.unwrap();
    admin
        .batch_execute(
            "ALTER TABLE cantos_schema DROP CONSTRAINT cantos_schema_epoch_check;
        UPDATE cantos_schema SET epoch=3",
        )
        .await
        .unwrap();
    assert_eq!(migrate(&config).await, Err(SchemaError::IncompatibleEpoch));
    assert_eq!(
        verify_schema(&config).await,
        Err(SchemaError::IncompatibleEpoch)
    );
    admin
        .batch_execute(
            "UPDATE cantos_schema SET epoch=2;
        INSERT INTO cantos_migrations VALUES(4,decode(repeat('11',32),'hex'))",
        )
        .await
        .unwrap();
    assert_eq!(migrate(&config).await, Err(SchemaError::RegistryMismatch));
    assert_eq!(registry(&admin).await.len(), 4);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn malformed_registry_values_return_typed_errors_without_panicking_or_mutation() {
    for (ddl, expected) in [
        (
            "CREATE TABLE cantos_schema(epoch text); INSERT INTO cantos_schema VALUES('2')",
            SchemaError::IncompatibleEpoch,
        ),
        (
            "CREATE TABLE cantos_schema(epoch integer); INSERT INTO cantos_schema VALUES(NULL)",
            SchemaError::IncompatibleEpoch,
        ),
        (
            "CREATE TABLE cantos_schema(epoch integer); INSERT INTO cantos_schema VALUES(2);
             CREATE TABLE cantos_migrations(version bigint,checksum bytea);
             INSERT INTO cantos_migrations VALUES(1,decode('aa','hex'))",
            SchemaError::RegistryMismatch,
        ),
        (
            "CREATE TABLE cantos_schema(epoch integer); INSERT INTO cantos_schema VALUES(2);
             CREATE TABLE cantos_migrations(version integer,checksum text);
             INSERT INTO cantos_migrations VALUES(1,'not-bytea')",
            SchemaError::RegistryMismatch,
        ),
    ] {
        let (config, admin) = database().await;
        admin.batch_execute(ddl).await.unwrap();
        let snapshot = "SELECT json_build_object('epoch',(SELECT json_agg(s) FROM cantos_schema s),
            'tables',(SELECT json_agg(tablename ORDER BY tablename) FROM pg_tables
                WHERE schemaname='public'))::text";
        let before: String = admin.query_one(snapshot, &[]).await.unwrap().get(0);
        assert_eq!(migrate(&config).await, Err(expected));
        assert_eq!(verify_schema(&config).await, Err(expected));
        let after: String = admin.query_one(snapshot, &[]).await.unwrap().get(0);
        assert_eq!(after, before);
    }
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn failed_baseline_rolls_back_registry_and_ddl_then_allows_clean_retry() {
    let (mut config, admin) = database().await;
    let role = format!("cantos_schema_probe_{}", Uuid::new_v4().simple());
    let name = config.get_dbname().unwrap();
    admin
        .batch_execute(&format!(
            "CREATE ROLE {role} LOGIN; ALTER DATABASE {name} OWNER TO {role};
             REVOKE USAGE ON LANGUAGE sql FROM PUBLIC"
        ))
        .await
        .unwrap();
    config.user(&role);
    // Registry tables are created first, then the first SQL function is denied. The same
    // transaction must undo both registries and every preceding DDL statement.
    assert_eq!(
        migrate(&config).await,
        Err(SchemaError::DatabaseUnavailable)
    );
    assert_eq!(
        admin
            .query_one(
                "SELECT count(*) FROM pg_class c JOIN pg_namespace n ON n.oid=c.relnamespace
                 WHERE n.nspname='public'",
                &[]
            )
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    admin
        .batch_execute("GRANT USAGE ON LANGUAGE sql TO PUBLIC")
        .await
        .unwrap();
    migrate(&config).await.unwrap();
    verify_schema(&config).await.unwrap();
    assert_eq!(registry(&admin).await.len(), 3);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn baseline_keeps_security_definers_pinned_and_public_without_execution_privileges() {
    let (config, admin) = database().await;
    migrate(&config).await.unwrap();
    let functions = admin
        .query(
            "SELECT proname,proconfig FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace
             WHERE n.nspname='public' AND prosecdef ORDER BY proname COLLATE \"C\"",
            &[],
        )
        .await
        .unwrap();
    let names: Vec<String> = functions.iter().map(|row| row.get(0)).collect();
    assert_eq!(
        names,
        [
            "admin_append_audit",
            "admin_identity_facts",
            "admin_operation",
            "admin_revoke_access",
            "bearer_register",
            "catalog_lock_actor",
            "catalog_lock_member",
            "listener_lock_actor",
            "production_lock_actor",
            "production_lock_member",
            "studio_oidc_finish",
            "theatre_firebase_enrollment_receipt",
            "theatre_identity_finish",
            "theatre_native_finish",
        ]
    );
    for function in functions {
        let settings: Vec<String> = function.get(1);
        assert!(settings
            .iter()
            .any(|setting| setting == "search_path=pg_catalog"));
    }
    let public_execution: Vec<String> = admin
        .query(
            "SELECT p.proname FROM pg_proc p JOIN pg_namespace n ON n.oid=p.pronamespace,
                LATERAL aclexplode(coalesce(p.proacl,acldefault('f',p.proowner))) a
             WHERE n.nspname='public' AND a.grantee=0 AND a.privilege_type='EXECUTE'",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(public_execution, Vec::<String>::new());
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn identity_schema_rejects_every_unicode_whitespace_only_issuer_and_subject() {
    let (config, admin) = database().await;
    migrate(&config).await.unwrap();
    admin
        .batch_execute("INSERT INTO actors(id) VALUES('unicode-owner')")
        .await
        .unwrap();
    // Independent identity boundary vectors. U+2028/U+2029 can be silently split into
    // newlines by source materialization tools, weakening the SQL regex while it still parses.
    for whitespace in [
        " ",
        "\u{00a0}",
        "\u{1680}",
        "\u{2000}",
        "\u{2001}",
        "\u{2002}",
        "\u{2003}",
        "\u{2004}",
        "\u{2005}",
        "\u{2006}",
        "\u{2007}",
        "\u{2008}",
        "\u{2009}",
        "\u{200a}",
        "\u{2028}",
        "\u{2029}",
        "\u{202f}",
        "\u{205f}",
        "\u{3000}",
        " \u{2028}\u{2029} ",
    ] {
        for (issuer, subject) in [(whitespace, "subject"), ("issuer", whitespace)] {
            let error = admin
                .execute(
                    "INSERT INTO actor_external_identities(issuer,subject,actor_id)
                     VALUES($1,$2,'unicode-owner')",
                    &[&issuer, &subject],
                )
                .await
                .unwrap_err();
            assert_eq!(
                error.code(),
                Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION),
                "issuer={issuer:?} subject={subject:?}"
            );
        }
    }
    assert_eq!(
        admin
            .query_one("SELECT count(*) FROM actor_external_identities", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        0
    );
    admin
        .execute(
            "INSERT INTO actor_external_identities(issuer,subject,actor_id)
             VALUES('Nhà hát Vọng Đài','Người kể chuyện','unicode-owner')",
            &[],
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn concurrent_bootstrap_commits_one_epoch_and_read_only_app_check_needs_no_ddl() {
    let (config, admin) = database().await;
    let (left, right) = tokio::join!(migrate(&config), migrate(&config));
    left.unwrap();
    right.unwrap();
    assert_eq!(registry(&admin).await.len(), 3);
    admin
        .batch_execute(
            "GRANT USAGE ON SCHEMA public TO cantos_app;
        GRANT SELECT ON cantos_schema,cantos_migrations TO cantos_app",
        )
        .await
        .unwrap();
    let mut app = config.clone();
    app.user("cantos_app");
    verify_schema(&app).await.unwrap();
    let client = connect(&app).await;
    let error = client
        .batch_execute("UPDATE cantos_schema SET epoch=2")
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE)
    );
    let error = client
        .batch_execute("CREATE TABLE unauthorized(id integer)")
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE)
    );
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn bearer_expansion_preserves_exact_baseline_and_refuses_runtime_upgrade() {
    use sha2::{Digest, Sha256};
    let (config, admin) = database().await;
    let baseline = include_str!("../schema/0001_current.sql");
    admin.batch_execute("CREATE TABLE cantos_schema(singleton boolean PRIMARY KEY CHECK(singleton),epoch integer NOT NULL CHECK(epoch=2)); CREATE TABLE cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL); INSERT INTO cantos_schema VALUES(true,2);").await.unwrap();
    admin.batch_execute(baseline).await.unwrap();
    admin
        .execute(
            "INSERT INTO cantos_migrations VALUES(1,$1)",
            &[&Sha256::digest(baseline.as_bytes()).to_vec()],
        )
        .await
        .unwrap();
    admin
        .batch_execute("INSERT INTO actors(id) VALUES('preserved-owner')")
        .await
        .unwrap();
    let old = registry(&admin).await;
    assert_eq!(
        verify_schema(&config).await,
        Err(SchemaError::RegistryMismatch)
    );
    assert_eq!(registry(&admin).await, old);
    migrate(&config).await.unwrap();
    verify_schema(&config).await.unwrap();
    assert_eq!(registry(&admin).await[0], old[0]);
    assert_eq!(
        admin
            .query_one("SELECT id FROM actors", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "preserved-owner"
    );
}

#[cfg(unix)]
#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn prefix_helper_fingerprints_rows_and_sequence_state_and_detects_drift() {
    use std::{fs, process::Command};

    let (config, admin) = database().await;
    migrate(&config).await.unwrap();
    admin
        .batch_execute(
            "INSERT INTO actors(id) VALUES('prefix-fixture-a'),('prefix-fixture-b');
             CREATE SEQUENCE prefix_fixture_sequence;
             SELECT setval('prefix_fixture_sequence',17,false);",
        )
        .await
        .unwrap();
    let directory = env::temp_dir().join(format!("cantos-prefix-fingerprint-{}", Uuid::new_v4()));
    fs::create_dir(&directory).unwrap();
    fs::write(directory.join("baseline-tables"), "actors\ncantos_schema\n").unwrap();
    fs::write(
        directory.join("baseline-sequences"),
        "prefix_fixture_sequence\n",
    )
    .unwrap();
    let helper = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../infra/vps/bin/cantos-prefix-upgrade");
    let fingerprint = |name: &str| {
        let output = directory.join(name);
        let result = Command::new("bash")
            .arg("-c")
            .arg(
                r#"set -Eeuo pipefail
source "$HELPER"
die() { printf '%s\n' "$1" >&2; exit 65; }
epoch2_query() {
  printf '\pset tuples_only on\n\pset format unaligned\n%s\n' "$2" |
    psql -X -q -v ON_ERROR_STOP=1 -h 127.0.0.1 -p "$PORT" -U cantos_test_admin -d "$DB"
}
prefix_fingerprint "$DB" "$DIRECTORY" "$OUTPUT"
"#,
            )
            .env_clear()
            .env("PATH", env::var_os("PATH").unwrap())
            .env("HELPER", &helper)
            .env("PORT", config.get_ports()[0].to_string())
            .env("DB", config.get_dbname().unwrap())
            .env("DIRECTORY", &directory)
            .env("OUTPUT", &output)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        fs::read(output).unwrap()
    };
    let before = fingerprint("before.tsv");
    migrate(&config).await.unwrap();
    assert_eq!(fingerprint("replayed.tsv"), before);
    admin
        .batch_execute("UPDATE actors SET active=false WHERE id='prefix-fixture-a'")
        .await
        .unwrap();
    assert_ne!(fingerprint("changed-row.tsv"), before);
    admin
        .batch_execute(
            "UPDATE actors SET active=true WHERE id='prefix-fixture-a';
             SELECT setval('prefix_fixture_sequence',17,true);",
        )
        .await
        .unwrap();
    assert_ne!(fingerprint("changed-sequence.tsv"), before);
    fs::remove_dir_all(directory).unwrap();
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite schema"]
async fn listener_bookmark_expansion_is_the_exact_suffix_after_registry_one_two() {
    use sha2::{Digest, Sha256};
    let (config, admin) = database().await;
    let baseline = include_str!("../schema/0001_current.sql");
    let bearer = include_str!("../schema/0002_bearer.sql");
    admin.batch_execute("CREATE TABLE cantos_schema(singleton boolean PRIMARY KEY CHECK(singleton),epoch integer NOT NULL CHECK(epoch=2)); CREATE TABLE cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL); INSERT INTO cantos_schema VALUES(true,2);").await.unwrap();
    admin.batch_execute(baseline).await.unwrap();
    admin.batch_execute(bearer).await.unwrap();
    for (version, sql) in [(1, baseline), (2, bearer)] {
        admin
            .execute(
                "INSERT INTO cantos_migrations VALUES($1,$2)",
                &[&version, &Sha256::digest(sql.as_bytes()).to_vec()],
            )
            .await
            .unwrap();
    }
    admin
        .batch_execute("INSERT INTO actors(id) VALUES('kept-owner')")
        .await
        .unwrap();
    let old = registry(&admin).await;
    assert_eq!(old.len(), 2);
    assert!(admin
        .query_one(
            "SELECT to_regclass('public.listener_bookmarks') IS NULL",
            &[]
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    // The runtime binary never upgrades; only the migration actor applies the missing suffix.
    assert_eq!(
        verify_schema(&config).await,
        Err(SchemaError::RegistryMismatch)
    );
    migrate(&config).await.unwrap();
    verify_schema(&config).await.unwrap();
    let after = registry(&admin).await;
    assert_eq!(after.len(), 3);
    assert_eq!(after[..2], old[..], "applied checksums are never rewritten");
    assert_eq!(
        after[2].1,
        Sha256::digest(include_str!("../schema/0003_listener_bookmarks.sql").as_bytes()).to_vec()
    );
    assert_eq!(
        admin
            .query_one("SELECT id FROM actors", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "kept-owner"
    );
    migrate(&config).await.unwrap();
    assert_eq!(registry(&admin).await, after);
}
