//! Forward-only retirement of the unused JobSummary feed. Disposable PostgreSQL only.
//! These synthetic legacy rows exercise SQL compatibility, not a current producer workflow.
use std::env;

use cantos_api::platform::postgres::{local_config, migrate};
use sha2::{Digest, Sha256};
use tokio_postgres::{Client, Config, NoTls};
use uuid::Uuid;

const OLD: [&str; 9] = [
    include_str!("../migrations/0001_script_revisions.sql"),
    include_str!("../migrations/0002_editorial_handoff.sql"),
    include_str!("../migrations/0003_manuscript_import.sql"),
    include_str!("../migrations/0004_ai_adaptation.sql"),
    include_str!("../migrations/0005_caller_adaptation.sql"),
    include_str!("../migrations/0006_production_inputs.sql"),
    include_str!("../migrations/0007_durable_jobs.sql"),
    include_str!("../migrations/0008_speech_clips.sql"),
    include_str!("../migrations/0009_durable_step_changes.sql"),
];
const RETIRE: &str = include_str!("../migrations/0029_retire_durable_step_capture.sql");

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

async fn empty_database() -> (Config, Client) {
    let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres");
    let mut config = local_config(&url).unwrap();
    assert_eq!(config.get_user(), Some("cantos_test_admin"));
    assert_eq!(config.get_dbname(), Some("postgres"));
    let database = format!("cantos_test_{}", Uuid::new_v4().simple());
    connect(&config)
        .await
        .batch_execute(&format!("CREATE DATABASE {database}"))
        .await
        .unwrap();
    config.dbname(&database);
    let admin = connect(&config).await;
    (config, admin)
}

async fn legacy_database() -> (Config, Client) {
    let (config, mut admin) = empty_database().await;
    let tx = admin.transaction().await.unwrap();
    tx.batch_execute(
        "CREATE TABLE cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL)",
    )
    .await
    .unwrap();
    for (i, sql) in OLD.iter().enumerate() {
        tx.batch_execute(sql).await.unwrap();
        tx.execute(
            "INSERT INTO cantos_migrations VALUES($1,$2)",
            &[&((i + 1) as i32), &Sha256::digest(sql.as_bytes()).to_vec()],
        )
        .await
        .unwrap();
    }
    tx.commit().await.unwrap();
    (config, admin)
}

async fn seed_legacy_rows(admin: &Client) {
    // Preserve arbitrary historical bytes; no current API accepts these retired records.
    admin.batch_execute(
        "BEGIN;
         INSERT INTO actors(id) VALUES('legacy');
         INSERT INTO scripts(id,owner_id,head_revision) VALUES('script','legacy',1);
         INSERT INTO script_revisions(script_id,revision,expected_revision,operation_id,accepted_by,
             schema_version,canonical_export,content_digest,export_digest)
             VALUES('script',1,0,'save','legacy','0.1.0','historical script bytes',
             'sir-c1:sha256:'||repeat('0',64),'sir-e1:sha256:'||repeat('1',64));
         INSERT INTO production_settings(script_id,version,script_revision,operation_id,recorded_by,
             request_bytes,settings_bytes) VALUES('script',1,1,'settings','legacy','request','settings');
         INSERT INTO production_snapshots(id,script_id,owner_id,script_revision,settings_version,
             operation_id,created_by,input_digest,request_bytes,document_bytes)
             VALUES('snapshot','script','legacy',1,1,'snapshot','legacy',
             'production-p1:sha256:'||repeat('2',64),'request','historical snapshot bytes');
         INSERT INTO durable_runs(id,script_id,snapshot_id,created_by,operation_id,input_digest,mode,
             request_bytes,max_attempts,base_seconds,cap_seconds)
             VALUES('run','script','snapshot','legacy','run','production-p1:sha256:'||repeat('2',64),
             'synthetic_fake','request',1,1,1);
         INSERT INTO durable_jobs(id,run_id) VALUES('job','run');
         INSERT INTO durable_budgets(run_id,currency,limit_minor) VALUES('run','USD',0);
         INSERT INTO durable_steps(id,job_id,dialogue_id,input_fingerprint)
             VALUES('step','job','dialogue','synthetic-fingerprint');
         COMMIT;",
    ).await.unwrap();
}

async fn captured_rows(admin: &Client) -> String {
    admin.query_one(
        "SELECT coalesce(jsonb_agg(to_jsonb(c) ORDER BY txid,seq),'[]'::jsonb)::text FROM durable_step_changes c",
        &[],
    ).await.unwrap().get(0)
}

async fn retained_rows(admin: &Client) -> Vec<String> {
    let mut rows = Vec::new();
    for table in [
        "script_revisions",
        "production_settings",
        "production_snapshots",
        "durable_runs",
        "durable_jobs",
        "durable_budgets",
        "durable_steps",
        "durable_step_changes",
    ] {
        rows.push(admin.query_one(
            &format!("SELECT coalesce(jsonb_agg(row ORDER BY row::text),'[]'::jsonb)::text FROM (SELECT to_jsonb(t) AS row FROM {table} t) s"),
            &[],
        ).await.unwrap().get(0));
    }
    rows
}

async fn capture_enabled(admin: &Client) -> String {
    admin.query_one(
        "SELECT tgenabled::text FROM pg_trigger WHERE tgrelid='public.durable_steps'::regclass AND tgname='durable_steps_change_capture'",
        &[],
    ).await.unwrap().get(0)
}

async fn legacy_triggers(admin: &Client) -> Vec<(String, String)> {
    admin.query(
        "SELECT tgname,tgenabled::text FROM pg_trigger WHERE tgrelid IN ('public.durable_steps'::regclass,'public.durable_step_changes'::regclass) ORDER BY tgname",
        &[],
    ).await.unwrap().iter().map(|r| (r.get(0),r.get(1))).collect()
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite legacy-dbsp"]
async fn retirement_preserves_legacy_rows_checksums_and_stops_only_capture() {
    let (config, admin) = legacy_database().await;
    seed_legacy_rows(&admin).await;
    assert_eq!(capture_enabled(&admin).await, "O");
    let before = retained_rows(&admin).await;
    let mut triggers = legacy_triggers(&admin).await;
    let feed = captured_rows(&admin).await;
    assert_ne!(feed, "[]");
    let checksums = admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap();
    migrate(&config).await.unwrap();
    migrate(&config).await.unwrap();
    assert_eq!(capture_enabled(&admin).await, "D");
    for (name, state) in &mut triggers {
        if name == "durable_steps_change_capture" {
            *state = "D".into();
        }
    }
    assert_eq!(legacy_triggers(&admin).await, triggers);
    assert_eq!(retained_rows(&admin).await, before);
    for row in checksums {
        let version: i32 = row.get(0);
        let checksum: Vec<u8> = row.get(1);
        let retained: Vec<u8> = admin
            .query_one(
                "SELECT checksum FROM cantos_migrations WHERE version=$1",
                &[&version],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(retained, checksum);
    }
    let versions: Vec<i32> = admin
        .query(
            "SELECT version FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|r| r.get(0))
        .collect();
    assert_eq!(versions, (1..=29).collect::<Vec<_>>());
    // Insert and a projected succeeded transition would have written +1 and -old/+new in 0009.
    admin.batch_execute(
        "INSERT INTO durable_steps(id,job_id,dialogue_id,input_fingerprint) VALUES('new-step','job','new-dialogue','new-fingerprint');
         UPDATE durable_steps SET status='leased',lease_owner='synthetic',lease_token=1,
             lease_expires_at=clock_timestamp()+interval '1 minute' WHERE id='step';
         INSERT INTO durable_reservations(id,run_id,step_id,amount_minor,currency) VALUES('reservation','run','step',0,'USD');
         INSERT INTO durable_attempts(id,step_id,lease_token,operation_id,reservation_id,intent_bytes,attempt_number)
             VALUES('attempt','step',1,'attempt','reservation','intent',1);
         INSERT INTO durable_observations(attempt_id,receipt_bytes) VALUES('attempt','receipt');
         UPDATE durable_steps SET status='succeeded',accepted_attempt_id='attempt',lease_owner=NULL,lease_expires_at=NULL WHERE id='step';",
    ).await.unwrap();
    assert_eq!(captured_rows(&admin).await, feed);
    assert_eq!(
        admin
            .query_one("SELECT status FROM durable_steps WHERE id='step'", &[])
            .await
            .unwrap()
            .get::<_, String>(0),
        "succeeded"
    );
    for sql in [
        "UPDATE durable_steps SET input_fingerprint='changed' WHERE id='step'",
        "DELETE FROM durable_steps WHERE id='step'",
        "TRUNCATE durable_steps CASCADE",
        "UPDATE durable_step_changes SET weight=-weight",
    ] {
        let error = admin.batch_execute(sql).await.unwrap_err();
        assert_eq!(error.code().unwrap().code(), "55000", "{sql}");
    }
    assert_eq!(
        admin
            .batch_execute("UPDATE durable_steps SET status='succeeded' WHERE id='new-step'")
            .await
            .unwrap_err()
            .code()
            .unwrap()
            .code(),
        "23514"
    );
    let mut app_config = config.clone();
    app_config.user("cantos_app");
    let app = connect(&app_config).await;
    for sql in [
        "INSERT INTO durable_step_changes(job_id,step_id,weight,succeeded,reused) VALUES('job','forged',1,false,false)",
        "UPDATE durable_step_changes SET weight=-weight",
        "DELETE FROM durable_step_changes",
        "TRUNCATE durable_step_changes",
        "ALTER TABLE durable_steps ENABLE TRIGGER durable_steps_change_capture",
    ] {
        assert_eq!(app.batch_execute(sql).await.unwrap_err().code().unwrap().code(), "42501", "{sql}");
    }
    assert_eq!(captured_rows(&admin).await, feed);
}

#[tokio::test]
#[ignore = "cargo xtask verify postgres --suite legacy-dbsp"]
async fn retirement_is_transactional_and_fresh_install_retains_the_historical_feed() {
    let (config, mut admin) = legacy_database().await;
    seed_legacy_rows(&admin).await;
    let before = retained_rows(&admin).await;
    let tx = admin.transaction().await.unwrap();
    tx.batch_execute(RETIRE).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(capture_enabled(&admin).await, "O");
    assert_eq!(retained_rows(&admin).await, before);
    migrate(&config).await.unwrap();
    assert_eq!(capture_enabled(&admin).await, "D");

    let (fresh, fresh_admin) = empty_database().await;
    migrate(&fresh).await.unwrap();
    assert_eq!(capture_enabled(&fresh_admin).await, "D");
    assert_eq!(captured_rows(&fresh_admin).await, "[]");
    assert!(fresh_admin
        .query_one(
            "SELECT to_regprocedure('public.durable_step_change_capture()') IS NOT NULL",
            &[]
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
}
