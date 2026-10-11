//! Durable orchestration evidence in the runner's freshly created PostgreSQL cluster.
//! Script, rights, audio and cost facts are synthetic; no commercial provider is called.
use std::{env, path::PathBuf, sync::Arc};

use cantos_api::{
    jobs::{ExecutionMode, LeaseDuration, RetryPolicy, RunEvent, RunStatus},
    postgres::{
        durable::{AttemptWork, DurableError, JobLease, JobSummary, StepWork},
        local_config, migrate, token_hash, Store, StoreError,
    },
    script_ir::read_script,
    tts::{
        attempt::{DeliveryState, FailureClass},
        ChargeObservation, ReceiptOutcome, TtsReceipt,
    },
};
use cantos_contracts::{
    ApproveProductionRequest, CharacterCasting, FreezeProductionRequest, ProductionBudget,
    ProductionCurrency, ProductionRate, ProductionRightsDeclaration, ProductionRightsScope,
    ProductionRightsStatus, ProductionRightsSubject, ProductionSettings,
    ProductionSnapshotResponse, ReviewRequest, SaveProductionRightsRequest,
    SaveProductionSettingsRequest, SaveRevisionRequest, SynthesisPerformance,
};
use serde_json::Value;
use sha2::{Digest, Sha256};
use tokio::sync::Barrier;
use tokio_postgres::{Client, Config, NoTls};
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");

struct Harness {
    admin: Client,
    app_config: Config,
    store: Store,
    script: String,
    snapshot: ProductionSnapshotResponse,
    root: PathBuf,
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

impl Harness {
    async fn new(limit: u64) -> Self {
        Self::new_with_upgrade(limit, false).await
    }

    async fn new_with_upgrade(limit: u64, upgrade: bool) -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("use cargo xtask verify postgres");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let cluster = connect(&config).await;
        let database = format!("cantos_test_{}", Uuid::new_v4().simple());
        cluster
            .batch_execute(&format!("CREATE DATABASE {database}"))
            .await
            .unwrap();
        config.dbname(&database);
        if upgrade {
            let seeded = connect(&config).await;
            seeded.batch_execute("CREATE TABLE cantos_migrations(version integer PRIMARY KEY, checksum bytea NOT NULL); REVOKE ALL ON cantos_migrations FROM PUBLIC;").await.unwrap();
            for (version, sql) in [
                (
                    1_i32,
                    include_str!("../migrations/0001_script_revisions.sql"),
                ),
                (2, include_str!("../migrations/0002_editorial_handoff.sql")),
                (3, include_str!("../migrations/0003_manuscript_import.sql")),
                (4, include_str!("../migrations/0004_ai_adaptation.sql")),
                (5, include_str!("../migrations/0005_caller_adaptation.sql")),
                (6, include_str!("../migrations/0006_production_inputs.sql")),
            ] {
                seeded.batch_execute(sql).await.unwrap();
                seeded
                    .execute(
                        "INSERT INTO cantos_migrations(version,checksum) VALUES($1,$2)",
                        &[&version, &token_hash(sql)],
                    )
                    .await
                    .unwrap();
            }
        } else {
            migrate(&config).await.unwrap();
        }
        let admin = connect(&config).await;
        admin.batch_execute("GRANT USAGE ON SCHEMA public TO cantos_app; GRANT SELECT ON ALL TABLES IN SCHEMA public TO cantos_app; GRANT INSERT ON scripts,script_revisions,revision_evidence,script_evidence,script_reviews,script_review_operations,production_settings,production_rights_claims,production_snapshots,production_approvals TO cantos_app; GRANT UPDATE(head_revision) ON scripts TO cantos_app; GRANT UPDATE(revoked) ON sessions TO cantos_app; GRANT EXECUTE ON FUNCTION production_lock_actor(text),production_lock_member(text,text) TO cantos_app; INSERT INTO actors(id) VALUES('alice'),('bob');").await.unwrap();
        for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
            admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,CURRENT_TIMESTAMP+interval '1 hour')", &[&token_hash(token),&actor]).await.unwrap();
        }
        for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
            admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice',$1,$2,'Original synthetic test evidence; no legal clearance')", &[&kind,&id]).await.unwrap();
        }
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-durable-test");
        let store = Store::new(app_config.clone()).unwrap();
        let script = Uuid::new_v4().to_string();
        store
            .save(
                ALICE,
                &script,
                SaveRevisionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 0,
                    script_json: SCRIPT.into(),
                },
            )
            .await
            .unwrap();
        store
            .review(
                ALICE,
                &script,
                ReviewRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    revision: 1,
                },
            )
            .await
            .unwrap();
        store
            .save_production_settings(
                ALICE,
                &script,
                SaveProductionSettingsRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 1,
                    expected_settings_version: 0,
                    settings: settings(limit),
                },
            )
            .await
            .unwrap();
        let mut claims: Vec<_> = read_script(SCRIPT.as_bytes())
            .unwrap()
            .evidence_refs()
            .into_iter()
            .filter(|(kind, _)| *kind == "rights")
            .map(|(_, id)| {
                claim(
                    id,
                    ProductionRightsSubject::Evidence {
                        record_id: id.into(),
                    },
                )
            })
            .collect();
        claims.push(claim(
            "voice_fixture",
            ProductionRightsSubject::Voice {
                provider_id: "espeak-ng-local".into(),
                model_id: "espeak-ng-1.52.0".into(),
                voice_id: "vi".into(),
            },
        ));
        for claim in claims {
            store
                .save_production_rights(
                    ALICE,
                    &script,
                    SaveProductionRightsRequest {
                        operation_id: Uuid::new_v4().to_string(),
                        expected_version: 0,
                        claim,
                    },
                )
                .await
                .unwrap();
        }
        let preview = store.production_preview(ALICE, &script).await.unwrap();
        let snapshot = store
            .freeze_production(
                ALICE,
                &script,
                FreezeProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 1,
                    settings_version: preview.settings_version,
                    input_digest: preview.input_digest,
                },
            )
            .await
            .unwrap();
        assert!(
            snapshot.eligibility.inputs_eligible,
            "synthetic fixture admission findings: {:?}",
            snapshot.eligibility.findings
        );
        store
            .approve_production(
                ALICE,
                &script,
                &snapshot.id,
                ApproveProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    input_digest: snapshot.document.input_digest.clone(),
                },
            )
            .await
            .unwrap();
        if upgrade {
            migrate(&config).await.unwrap();
        }
        let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../target/durable-job-evidence")
            .join(Uuid::new_v4().to_string());
        std::fs::create_dir_all(&root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(&root, std::fs::Permissions::from_mode(0o700)).unwrap();
        }
        Self {
            admin,
            app_config,
            store,
            script,
            snapshot,
            root,
        }
    }

    fn reopened(&self) -> Store {
        Store::new(self.app_config.clone()).unwrap()
    }

    async fn enqueue(&self, mode: ExecutionMode) -> JobSummary {
        self.store
            .enqueue_job(
                ALICE,
                &self.script,
                &self.snapshot.id,
                &Uuid::new_v4().to_string(),
                mode,
                policy(),
            )
            .await
            .unwrap()
    }

    async fn lease(&self, worker: &str) -> JobLease {
        self.store
            .claim_step(worker, ExecutionMode::SyntheticFake, ttl())
            .await
            .unwrap()
            .unwrap()
    }

    async fn dispatch(&self, lease: &JobLease) -> AttemptWork {
        match self.store.prepare_step(lease).await.unwrap() {
            StepWork::Dispatch(work) => *work,
            _ => panic!("expected a new persisted dispatch intent"),
        }
    }

    async fn expire(&self, lease: &JobLease) {
        self.admin.execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE id=$1", &[&lease.step_id]).await.unwrap();
    }

    async fn wait_backoff(&self) {
        tokio::time::timeout(std::time::Duration::from_secs(5), async {
            loop {
                let ready: bool = self.admin.query_one("SELECT COALESCE(bool_and(not_before<=clock_timestamp()),true) FROM durable_steps WHERE status='pending'", &[]).await.unwrap().get(0);
                if ready { return; }
                tokio::task::yield_now().await;
            }
        }).await.expect("persisted retry deadline did not become ready within its bounded policy");
    }

    async fn outcome(&self, work: &AttemptWork) -> (TtsReceipt, Vec<u8>, String) {
        self.store.authorize_dispatch(work).await.unwrap();
        let bytes = wav();
        let mut receipt = work.intent.clone();
        receipt.outcome = ReceiptOutcome::Output {
            audio_sha256: format!("{:x}", Sha256::digest(&bytes)),
        };
        receipt.charge = ChargeObservation::Reported {
            currency: ProductionCurrency::VND,
            amount_minor: 1,
            reference: "synthetic-fixture-charge-no-provider-billing".into(),
        };
        let path = self.root.join(format!("{}.wav", receipt.attempt_id));
        std::fs::write(&path, &bytes).unwrap();
        (receipt, bytes, path.to_str().unwrap().to_owned())
    }
}

fn settings(limit: u64) -> ProductionSettings {
    ProductionSettings {
        bindings: ["narrator", "an", "minh"]
            .into_iter()
            .map(|id| CharacterCasting {
                character_id: id.into(),
                provider_id: "espeak-ng-local".into(),
                model_id: "espeak-ng-1.52.0".into(),
                voice_id: "vi".into(),
                language: "vi-VN".into(),
                voice_rights_record_id: "voice_fixture".into(),
                performance: SynthesisPerformance {
                    rate_permille: 1_000,
                    pitch_semitones: 0,
                    emotion: Some("neutral".into()),
                    intensity_permille: Some(0),
                },
                pronunciation: vec![],
            })
            .collect(),
        budget: ProductionBudget {
            currency: ProductionCurrency::VND,
            limit_minor: limit,
            rate: Some(ProductionRate {
                reference: "Synthetic fixture, no quoted provider price".into(),
                version: "fixture-v1".into(),
                units_per_charge: 1_000,
                amount_minor: 1,
            }),
            scope: "private integration checks only".into(),
            territory: "private-planning".into(),
        },
    }
}

fn claim(id: &str, subject: ProductionRightsSubject) -> ProductionRightsDeclaration {
    ProductionRightsDeclaration {
        record_id: id.into(),
        subject,
        scope: ProductionRightsScope::ProductionSynthesis,
        rights_holder: "Synthetic fixture author".into(),
        languages: vec!["vi-VN".into()],
        territory: "private-planning".into(),
        attribution: "Original synthetic fixtures".into(),
        restrictions: "private-planning-only".into(),
        permitted_scope: "private integration checks only".into(),
        status: ProductionRightsStatus::Granted,
        reference: "Recorded synthetic assertion; no production legal clearance".into(),
        valid_from_unix: 0,
        valid_until_unix: None,
    }
}

fn wav() -> Vec<u8> {
    let mut bytes = Vec::new();
    bytes.extend_from_slice(b"RIFF");
    bytes.extend_from_slice(&40_u32.to_le_bytes());
    bytes.extend_from_slice(b"WAVEfmt ");
    bytes.extend_from_slice(&16_u32.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&1_u16.to_le_bytes());
    bytes.extend_from_slice(&22_050_u32.to_le_bytes());
    bytes.extend_from_slice(&44_100_u32.to_le_bytes());
    bytes.extend_from_slice(&2_u16.to_le_bytes());
    bytes.extend_from_slice(&16_u16.to_le_bytes());
    bytes.extend_from_slice(b"data");
    bytes.extend_from_slice(&4_u32.to_le_bytes());
    bytes.extend_from_slice(&[0, 0, 1, 0]);
    bytes
}

fn ttl() -> LeaseDuration {
    LeaseDuration::new(30).unwrap()
}

fn policy() -> RetryPolicy {
    RetryPolicy::new(2, 1, 2).unwrap()
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn migration_upgrades_seeded_production_inputs_and_replay_preserves_exact_frozen_bytes() {
    let h = Harness::new_with_upgrade(4, true).await;
    let snapshot = h
        .store
        .production_snapshot(ALICE, &h.script, &h.snapshot.id)
        .await
        .unwrap();
    assert_eq!(snapshot.document, h.snapshot.document);
    assert!(snapshot.eligibility.approval_current);
    let mut admin_config = h.app_config.clone();
    admin_config.user("cantos_test_admin");
    migrate(&admin_config).await.unwrap();
    assert_eq!(
        h.store
            .production_snapshot(ALICE, &h.script, &h.snapshot.id)
            .await
            .unwrap()
            .document,
        snapshot.document
    );
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    assert_eq!(job.total_steps, 4);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn bounded_app_role_read_and_claim_plans_are_recorded_without_a_scale_claim() {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let app = connect(&h.app_config).await;
    let summary_plan = app.query("EXPLAIN (ANALYZE,BUFFERS) SELECT j.id,j.status,j.reason,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id) AS total,(SELECT count(*) FROM durable_steps s WHERE s.job_id=j.id AND s.status='succeeded') AS done FROM durable_jobs j WHERE j.run_id=$1", &[&job.run_id]).await.unwrap();
    let claim_plan = app.query("EXPLAIN (ANALYZE,BUFFERS) SELECT j.id,j.run_id,j.status FROM durable_jobs j JOIN durable_runs r ON r.id=j.run_id WHERE j.status IN ('queued','running') AND r.mode='synthetic_fake' AND EXISTS(SELECT 1 FROM durable_steps s WHERE s.job_id=j.id AND ((s.status='pending' AND s.not_before<=clock_timestamp()) OR (s.status='leased' AND s.lease_expires_at<=clock_timestamp()))) ORDER BY j.created_at,j.id LIMIT 1 FOR UPDATE OF j SKIP LOCKED", &[]).await.unwrap();
    let mut durations = Vec::new();
    for _ in 0..9 {
        let started = std::time::Instant::now();
        let summary = h.store.job_status(ALICE, &job.run_id).await.unwrap();
        assert_eq!(summary.total_steps, 4);
        durations.push(started.elapsed().as_secs_f64() * 1_000.0);
    }
    durations.sort_by(f64::total_cmp);
    let mut report = format!("Fixture: one owner, one run, four frozen dialogues; app role; no throughput/scale claim.\njob_status nine samples median_ms={:.3} p95_nearest_rank_ms={:.3}\nsummary plan:\n", durations[4],durations[8]);
    for row in summary_plan {
        report.push_str(&row.get::<_, String>(0));
        report.push('\n');
    }
    report.push_str("claim selection plan:\n");
    for row in claim_plan {
        report.push_str(&row.get::<_, String>(0));
        report.push('\n');
    }
    std::fs::write(h.root.join("read-plans.log"), report).unwrap();
    println!(
        "Read/claim plan evidence: {}",
        h.root.join("read-plans.log").display()
    );
    assert_invariants(&h).await;
}

async fn assert_invariants(h: &Harness) {
    let budgets = h.admin.query("SELECT b.reserved_minor,b.settled_minor,b.limit_minor,(SELECT COALESCE(sum(r.amount_minor),0)::bigint FROM durable_reservations r LEFT JOIN durable_settlements x ON x.reservation_id=r.id WHERE r.run_id=b.run_id AND x.reservation_id IS NULL),(SELECT COALESCE(sum(x.amount_minor),0)::bigint FROM durable_settlements x JOIN durable_reservations r ON r.id=x.reservation_id WHERE r.run_id=b.run_id AND x.kind<>'released') FROM durable_budgets b", &[]).await.unwrap();
    for budget in budgets {
        let reserved: i64 = budget.get(0);
        let settled: i64 = budget.get(1);
        let limit: i64 = budget.get(2);
        check_budget(reserved, settled, limit, budget.get(3), budget.get(4)).unwrap();
    }
    let rows = h.admin.query("SELECT s.dialogue_id,s.input_fingerprint,s.status,s.accepted_attempt_id,a.intent_bytes,o.receipt_bytes,r.input_digest,p.document_bytes FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id JOIN durable_runs r ON r.id=j.run_id JOIN production_snapshots p ON p.id=r.snapshot_id LEFT JOIN durable_attempts a ON a.id=s.accepted_attempt_id LEFT JOIN durable_observations o ON o.attempt_id=s.accepted_attempt_id", &[]).await.unwrap();
    for row in rows {
        let dialogue: String = row.get(0);
        let document: cantos_contracts::FrozenProductionDocument =
            serde_json::from_slice(&row.get::<_, Vec<u8>>(7)).unwrap();
        assert_eq!(document.input_digest, row.get::<_, String>(6));
        let line = document
            .resolved
            .iter()
            .find(|line| line.dialogue_id == dialogue)
            .unwrap();
        let mut hash = Sha256::new();
        hash.update(b"cantos/durable-input/v1\n");
        hash.update(serde_json::to_vec(line).unwrap());
        assert_eq!(
            row.get::<_, String>(1),
            format!("durable-input-v1:sha256:{:x}", hash.finalize()),
            "step fingerprint differs from its exact frozen line"
        );
        let accepted: Option<String> = row.get(3);
        assert_eq!(row.get::<_, String>(2) == "succeeded", accepted.is_some());
        if let Some(accepted) = accepted {
            let intent: TtsReceipt = serde_json::from_slice(&row.get::<_, Vec<u8>>(4)).unwrap();
            let receipt: TtsReceipt = serde_json::from_slice(&row.get::<_, Vec<u8>>(5)).unwrap();
            assert_eq!(receipt.attempt_id, accepted);
            assert_eq!(receipt.attempt_id, intent.attempt_id);
            assert_eq!(receipt.operation_id, intent.operation_id);
            assert_eq!(receipt.dialogue_id, dialogue);
            assert_eq!(receipt.input_digest, document.input_digest);
            assert!(matches!(receipt.outcome, ReceiptOutcome::Output { .. }));
        }
    }
    let missing: i64 = h.admin.query_one("SELECT count(*) FROM durable_attempts a LEFT JOIN durable_reservations r ON r.id=a.reservation_id WHERE r.id IS NULL OR r.step_id<>a.step_id", &[]).await.unwrap().get(0);
    assert_eq!(
        missing, 0,
        "an attempt lacks its same-step durable reservation"
    );
}

fn check_budget(
    reserved: i64,
    settled: i64,
    limit: i64,
    ledger_reserved: i64,
    ledger_settled: i64,
) -> Result<(), &'static str> {
    if reserved != ledger_reserved {
        return Err("reservation ledger and budget counter disagree");
    }
    if settled != ledger_settled {
        return Err("settlement ledger and budget counter disagree");
    }
    if reserved < 0 || settled < 0 || settled > limit || reserved > limit - settled {
        return Err("reserved plus settled exceeds frozen budget");
    }
    Ok(())
}

#[test]
fn independent_budget_checker_rejects_corrupted_counters_and_overrun() {
    assert_eq!(check_budget(1, 3, 4, 1, 3), Ok(()));
    assert_eq!(
        check_budget(2, 2, 4, 1, 2),
        Err("reservation ledger and budget counter disagree")
    );
    assert_eq!(
        check_budget(1, 3, 4, 1, 2),
        Err("settlement ledger and budget counter disagree")
    );
    assert_eq!(
        check_budget(2, 3, 4, 2, 3),
        Err("reserved plus settled exceeds frozen budget")
    );
}

// A rejected owner command, including an exact replay, preserves every durable row and
// the immutable production inputs. Read raw database records, not the store's summary.
async fn expiry_persisted_state(h: &Harness) -> Value {
    let mut state = serde_json::Map::new();
    for table in [
        "durable_runs",
        "durable_jobs",
        "durable_steps",
        "durable_budgets",
        "durable_reservations",
        "durable_attempts",
        "durable_dispatches",
        "durable_observations",
        "durable_settlements",
        "durable_resolutions",
        "durable_controls",
        "scripts",
        "script_revisions",
        "revision_evidence",
        "script_evidence",
        "script_reviews",
        "script_review_operations",
        "production_settings",
        "production_rights_claims",
        "production_snapshots",
        "production_approvals",
    ] {
        // Table names are the fixed fixture inventory above, never caller input.
        let query = format!(
            "SELECT COALESCE(jsonb_agg(data ORDER BY data::text COLLATE \"C\"), '[]'::jsonb)::text \
             FROM (SELECT to_jsonb(record) AS data FROM {table} record) rows"
        );
        let text: String = h.admin.query_one(&query, &[]).await.unwrap().get(0);
        state.insert(table.into(), serde_json::from_str(&text).unwrap());
    }
    Value::Object(state)
}

async fn expire_session_behind_owned_lock<F>(
    h: &Harness,
    lock_query: &str,
    resource: &str,
    waiter_query: &str,
    caller: F,
) -> Result<JobSummary, DurableError>
where
    F: std::future::Future<Output = Result<JobSummary, DurableError>>,
{
    // Warm the existing pool before imposing the short expiry window. Do not change the
    // production statement, lock or idle-transaction timeouts to make a test pass.
    assert_eq!(h.store.authenticate(ALICE).await.unwrap(), "alice");
    let mut blocker_config = h.app_config.clone();
    blocker_config.application_name("cantos-expiry-blocker");
    let mut blocker = connect(&blocker_config).await;
    let blocker_pid: i32 = blocker
        .query_one("SELECT pg_backend_pid()", &[])
        .await
        .unwrap()
        .get(0);
    let tx = blocker.transaction().await.unwrap();
    tx.query_one(lock_query, &[&resource]).await.unwrap();
    assert_eq!(
        h.admin
            .execute(
                "UPDATE sessions SET expires_at=clock_timestamp()+interval '2 seconds' \
                 WHERE token_hash=$1",
                &[&token_hash(ALICE)],
            )
            .await
            .unwrap(),
        1
    );

    let monitor = async {
        let observed = tokio::time::timeout(std::time::Duration::from_millis(2_500), async {
            let valid_at_lock = loop {
                let witness = h
                    .admin
                    .query_one(
                        "SELECT EXISTS(SELECT 1 FROM pg_stat_activity a \
                         WHERE a.datname=current_database() AND a.usename='cantos_app' \
                         AND a.application_name='cantos-durable-test' AND a.state='active' \
                         AND a.wait_event_type='Lock' AND $1=ANY(pg_blocking_pids(a.pid)) \
                         AND a.query LIKE $2), \
                         (SELECT expires_at>clock_timestamp() FROM sessions WHERE token_hash=$3)",
                        &[&blocker_pid, &waiter_query, &token_hash(ALICE)],
                    )
                    .await?;
                if witness.get::<_, bool>(0) {
                    break witness.get::<_, bool>(1);
                }
                tokio::task::yield_now().await;
            };
            if !valid_at_lock {
                return Ok::<_, tokio_postgres::Error>((false, false));
            }
            loop {
                let expired: bool = h
                    .admin
                    .query_one(
                        "SELECT expires_at<=clock_timestamp() FROM sessions WHERE token_hash=$1",
                        &[&token_hash(ALICE)],
                    )
                    .await?
                    .get(0);
                if expired {
                    return Ok::<_, tokio_postgres::Error>((valid_at_lock, true));
                }
                tokio::task::yield_now().await;
            }
        })
        .await;
        // Always release this test's blocker before returning any failure or asserting.
        let released = tx.rollback().await;
        match (observed, released) {
            (Ok(Ok((true, true))), Ok(())) => {
                println!(
                    "expiry witness: query={waiter_query}; owned_blocker_pid={blocker_pid}; \
                     session_valid_at_lock=true; database_expired=true; blocker_rolled_back=true"
                );
                Ok(())
            }
            (observation, release) => Err(format!(
                "owned lock witness (valid_at_lock, database_expired): {observation:?}; \
                 rollback: {release:?}"
            )),
        }
    };
    // Both futures live in this task: a failed monitor cannot abandon a spawned request.
    let (result, monitored) = tokio::join!(
        tokio::time::timeout(std::time::Duration::from_secs(6), caller),
        monitor,
    );
    monitored.expect("owner command did not reach the intended lock before session expiry");
    result.expect("owner command did not finish after its blocker was released")
}

async fn assert_enqueue_expiry_case(replay: bool) {
    let h = Harness::new(4).await;
    let operation = Uuid::new_v4().to_string();
    if replay {
        h.store
            .enqueue_job(
                ALICE,
                &h.script,
                &h.snapshot.id,
                &operation,
                ExecutionMode::SyntheticFake,
                policy(),
            )
            .await
            .unwrap();
    }
    let before = expiry_persisted_state(&h).await;
    let result = expire_session_behind_owned_lock(
        &h,
        "SELECT id FROM scripts WHERE id=$1 FOR UPDATE",
        &h.script,
        "SELECT owner_id,head_revision FROM scripts%FOR UPDATE",
        h.store.enqueue_job(
            ALICE,
            &h.script,
            &h.snapshot.id,
            &operation,
            ExecutionMode::SyntheticFake,
            policy(),
        ),
    )
    .await;
    let after = expiry_persisted_state(&h).await;
    assert!(
        matches!(
            result,
            Err(DurableError::Store(StoreError::Unauthenticated))
        ),
        "replay={replay}: expected Unauthenticated after session expiry, got {result:?}; \
         persisted_state_unchanged={}",
        after == before,
    );
    assert_eq!(
        after, before,
        "replay={replay}: rejected enqueue changed persisted state"
    );
    assert_invariants(&h).await;
}

async fn assert_control_expiry_case(replay: bool) {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let operation = Uuid::new_v4().to_string();
    if replay {
        h.store
            .control_job(ALICE, &job.run_id, &operation, RunEvent::Pause)
            .await
            .unwrap();
    }
    let before = expiry_persisted_state(&h).await;
    let result = expire_session_behind_owned_lock(
        &h,
        "SELECT id FROM durable_jobs WHERE run_id=$1 FOR UPDATE",
        &job.run_id,
        "SELECT * FROM durable_jobs WHERE run_id=%FOR UPDATE",
        h.store
            .control_job(ALICE, &job.run_id, &operation, RunEvent::Pause),
    )
    .await;
    let after = expiry_persisted_state(&h).await;
    assert!(
        matches!(
            result,
            Err(DurableError::Store(StoreError::Unauthenticated))
        ),
        "replay={replay}: expected Unauthenticated after session expiry, got {result:?}; \
         persisted_state_unchanged={}",
        after == before,
    );
    assert_eq!(
        after, before,
        "replay={replay}: rejected control changed persisted state"
    );
    assert_invariants(&h).await;
}

async fn assert_resolution_expiry_case(replay: bool) {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let lease = h.lease("expiry-unresolved-attempt").await;
    // Persist intent/reservation only. No provider send or audio generation occurs.
    let work = h.dispatch(&lease).await;
    h.store
        .control_job(
            ALICE,
            &job.run_id,
            &Uuid::new_v4().to_string(),
            RunEvent::Pause,
        )
        .await
        .unwrap();
    let evidence = "Synthetic fixture owner attestation; no provider was called";
    if replay {
        h.store
            .resolve_not_executed(ALICE, &job.run_id, &work.intent.attempt_id, evidence)
            .await
            .unwrap();
    }
    let before = expiry_persisted_state(&h).await;
    let result = expire_session_behind_owned_lock(
        &h,
        "SELECT id FROM durable_jobs WHERE run_id=$1 FOR UPDATE",
        &job.run_id,
        "SELECT id,status FROM durable_jobs WHERE run_id=%FOR UPDATE",
        h.store
            .resolve_not_executed(ALICE, &job.run_id, &work.intent.attempt_id, evidence),
    )
    .await;
    let after = expiry_persisted_state(&h).await;
    assert!(
        matches!(
            result,
            Err(DurableError::Store(StoreError::Unauthenticated))
        ),
        "replay={replay}: expected Unauthenticated after session expiry, got {result:?}; \
         persisted_state_unchanged={}",
        after == before,
    );
    assert_eq!(
        after, before,
        "replay={replay}: rejected resolution changed persisted state"
    );
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn enqueue_rechecks_expired_session_after_script_lock_for_new() {
    assert_enqueue_expiry_case(false).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn enqueue_rechecks_expired_session_after_script_lock_for_exact_replay() {
    assert_enqueue_expiry_case(true).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn control_rechecks_expired_session_after_job_lock_for_new() {
    assert_control_expiry_case(false).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn control_rechecks_expired_session_after_job_lock_for_exact_replay() {
    assert_control_expiry_case(true).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn resolution_rechecks_expired_session_after_job_lock_for_new() {
    assert_resolution_expiry_case(false).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn resolution_rechecks_expired_session_after_job_lock_for_exact_replay() {
    assert_resolution_expiry_case(true).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn duplicate_start_intent_observation_and_accept_make_one_immutable_result() {
    let h = Harness::new(4).await;
    let operation = Uuid::new_v4().to_string();
    let job = h
        .store
        .enqueue_job(
            ALICE,
            &h.script,
            &h.snapshot.id,
            &operation,
            ExecutionMode::SyntheticFake,
            policy(),
        )
        .await
        .unwrap();
    let replay = h
        .reopened()
        .enqueue_job(
            ALICE,
            &h.script,
            &h.snapshot.id,
            &operation,
            ExecutionMode::SyntheticFake,
            policy(),
        )
        .await
        .unwrap();
    assert_eq!(job.run_id, replay.run_id);
    assert_eq!(job.job_id, replay.job_id);
    let lease = h.lease("first").await;
    let work = h.dispatch(&lease).await;
    let (receipt, audio, path) = h.outcome(&work).await;
    assert!(matches!(
        h.reopened().authorize_dispatch(&work).await,
        Err(DurableError::Conflict)
    ));
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    h.reopened()
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    // Reconciliation of the exact persisted output may accept under the original valid fence.
    h.store
        .accept_step(&lease, &receipt.attempt_id, &audio)
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .accept_step(&lease, &receipt.attempt_id, &audio)
            .await,
        Err(DurableError::LeaseLost)
    ));
    let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_runs),(SELECT count(*) FROM durable_attempts),(SELECT count(*) FROM durable_observations),(SELECT count(*) FROM durable_settlements),(SELECT count(*) FROM durable_steps WHERE status='succeeded')", &[]).await.unwrap();
    for column in 0..5 {
        assert_eq!(row.get::<_, i64>(column), 1);
    }
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_workers_claim_distinct_steps_and_reserve_without_budget_overrun() {
    let h = Harness::new(4).await;
    h.enqueue(ExecutionMode::SyntheticFake).await;
    let gate = Arc::new(Barrier::new(5));
    let mut tasks = Vec::new();
    for worker in ["one", "two", "three", "four"] {
        let store = h.reopened();
        let gate = gate.clone();
        tasks.push(tokio::spawn(async move {
            gate.wait().await;
            let lease = tokio::time::timeout(std::time::Duration::from_secs(5), async {
                loop {
                    if let Some(lease) = store
                        .claim_step(worker, ExecutionMode::SyntheticFake, ttl())
                        .await
                        .unwrap()
                    {
                        break lease;
                    }
                    tokio::task::yield_now().await;
                }
            })
            .await
            .expect("all four real connections must eventually claim a distinct step");
            let work = match store.prepare_step(&lease).await.unwrap() {
                StepWork::Dispatch(work) => work,
                _ => panic!("a distinct step should dispatch once within frozen budget"),
            };
            (lease, work)
        }));
    }
    gate.wait().await;
    let mut steps = std::collections::BTreeSet::new();
    for task in tasks {
        let (lease, _) = task.await.unwrap();
        assert!(steps.insert(lease.step_id));
    }
    assert_eq!(steps.len(), 4);
    assert!(h
        .store
        .claim_step("fifth", ExecutionMode::SyntheticFake, ttl())
        .await
        .unwrap()
        .is_none());
    let row = h
        .admin
        .query_one(
            "SELECT reserved_minor,settled_minor,limit_minor FROM durable_budgets",
            &[],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, i64>(0), 4);
    assert_eq!(row.get::<_, i64>(1), 0);
    assert_eq!(row.get::<_, i64>(2), 4);
    let app = connect(&h.app_config).await;
    let error = app
        .execute("UPDATE durable_budgets SET reserved_minor=5", &[])
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::CHECK_VIOLATION)
    );
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn receipt_linkage_and_current_frozen_scope_block_wrong_and_stale_results() {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    assert!(matches!(
        h.store.job_status(BOB, &job.run_id).await,
        Err(DurableError::Store(
            cantos_api::postgres::StoreError::NotFound
        ))
    ));
    let lease = h.lease("frozen").await;
    let work = h.dispatch(&lease).await;
    let (receipt, audio, path) = h.outcome(&work).await;
    for field in ["attempt", "operation", "digest", "dialogue", "voice"] {
        let mut bad = receipt.clone();
        match field {
            "attempt" => bad.attempt_id = Uuid::new_v4().to_string(),
            "operation" => bad.operation_id = Uuid::new_v4().to_string(),
            "digest" => bad.input_digest = format!("production-p1:sha256:{}", "0".repeat(64)),
            "dialogue" => {
                bad.dialogue_id = h
                    .snapshot
                    .document
                    .resolved
                    .iter()
                    .find(|line| line.dialogue_id != receipt.dialogue_id)
                    .unwrap()
                    .dialogue_id
                    .clone()
            }
            "voice" => bad.provider.voice_id = "vi-vn-x-south".into(),
            _ => unreachable!(),
        }
        assert!(
            matches!(
                h.store
                    .observe_attempt(&work, &bad, Some(&path), Some(&audio))
                    .await,
                Err(DurableError::Conflict)
            ),
            "mismatched receipt field {field} was not rejected"
        );
    }
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    let mut script: Value = serde_json::from_str(SCRIPT).unwrap();
    script["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] =
        Value::String("Bản thảo mới, không phải đầu vào đã đóng băng.".into());
    h.store
        .save(
            ALICE,
            &h.script,
            SaveRevisionRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_revision: 1,
                script_json: script.to_string(),
            },
        )
        .await
        .unwrap();
    assert!(
        matches!(h.store.accept_step(&lease, &receipt.attempt_id, &audio).await,
        Err(DurableError::Store(cantos_api::postgres::StoreError::ProductionBlocked(issues)))
            if issues.iter().any(|issue| issue.rule == "revision_changed"))
    );
    assert_eq!(work.document, h.snapshot.document);
    let attempts: i64 = h
        .admin
        .query_one("SELECT count(*) FROM durable_attempts", &[])
        .await
        .unwrap()
        .get(0);
    let accepted: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM durable_steps WHERE accepted_attempt_id IS NOT NULL",
            &[],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 1);
    assert_eq!(accepted, 0);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn settled_attempt_history_and_budget_policy_are_immutable_in_postgresql() {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let lease = h.lease("immutable").await;
    let work = h.dispatch(&lease).await;
    let (receipt, audio, path) = h.outcome(&work).await;
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    let app = connect(&h.app_config).await;
    for statement in [
        "UPDATE durable_runs SET input_digest=input_digest",
        "DELETE FROM durable_attempts",
        "UPDATE durable_observations SET receipt_bytes=receipt_bytes",
        "DELETE FROM durable_reservations",
        "UPDATE durable_settlements SET amount_minor=amount_minor",
    ] {
        let error = app.batch_execute(statement).await.unwrap_err();
        assert_eq!(
            error.code(),
            Some(&tokio_postgres::error::SqlState::INSUFFICIENT_PRIVILEGE),
            "{statement}"
        );
    }
    let error = app
        .execute(
            "UPDATE durable_budgets SET limit_minor=99 WHERE run_id=$1",
            &[&job.run_id],
        )
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::OBJECT_NOT_IN_PREREQUISITE_STATE)
    );
    let error = h
        .admin
        .batch_execute("DELETE FROM durable_attempts")
        .await
        .unwrap_err();
    assert_eq!(
        error.code(),
        Some(&tokio_postgres::error::SqlState::OBJECT_NOT_IN_PREREQUISITE_STATE)
    );
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn expired_claim_without_intent_recovers_but_stale_worker_cannot_write() {
    let h = Harness::new(4).await;
    h.enqueue(ExecutionMode::SyntheticFake).await;
    let first = h.lease("stale").await;
    h.expire(&first).await;
    let second = h.lease("replacement").await;
    assert_eq!(second.step_id, first.step_id);
    assert!(second.token > first.token);
    assert!(matches!(
        h.store.heartbeat_step(&first, ttl()).await,
        Err(DurableError::LeaseLost)
    ));
    assert!(matches!(
        h.store.prepare_step(&first).await,
        Err(DurableError::LeaseLost)
    ));
    let work = h.dispatch(&second).await;
    let (receipt, audio, path) = h.outcome(&work).await;
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    assert!(matches!(
        h.store
            .accept_step(&first, &receipt.attempt_id, &audio)
            .await,
        Err(DurableError::LeaseLost)
    ));
    h.store
        .accept_step(&second, &receipt.attempt_id, &audio)
        .await
        .unwrap();
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn unresolved_intent_after_lease_expiry_pauses_without_a_second_dispatch() {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let first = h.lease("worker-before-crash").await;
    let work = h.dispatch(&first).await;
    h.expire(&first).await;
    let next = h.lease("worker-after-crash").await;
    assert_eq!(next.step_id, first.step_id);
    assert!(matches!(
        h.reopened().prepare_step(&next).await.unwrap(),
        StepWork::Blocked
    ));
    let state = h.store.job_status(ALICE, &job.run_id).await.unwrap();
    assert_eq!(state.status, RunStatus::Paused);
    let attempts: i64 = h
        .admin
        .query_one("SELECT count(*) FROM durable_attempts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 1);
    assert!(h
        .store
        .claim_step("never-repeat", ExecutionMode::SyntheticFake, ttl())
        .await
        .unwrap()
        .is_none());
    let outcome = TtsReceipt {
        outcome: ReceiptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: DeliveryState::MayHaveExecuted,
        },
        ..work.intent.clone()
    };
    h.store
        .observe_attempt(&work, &outcome, None, None)
        .await
        .unwrap();
    let attempts: i64 = h
        .admin
        .query_one("SELECT count(*) FROM durable_attempts", &[])
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 1);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn pause_and_cancel_fence_inflight_output_but_preserve_outcome_accounting() {
    for event in [RunEvent::Pause, RunEvent::Cancel] {
        let h = Harness::new(4).await;
        let job = h.enqueue(ExecutionMode::SyntheticFake).await;
        let lease = h.lease("inflight").await;
        let work = h.dispatch(&lease).await;
        let (receipt, audio, path) = h.outcome(&work).await;
        let controlled = h
            .store
            .control_job(ALICE, &job.run_id, &Uuid::new_v4().to_string(), event)
            .await
            .unwrap();
        assert_eq!(
            controlled.status,
            if event == RunEvent::Pause {
                RunStatus::Paused
            } else {
                RunStatus::Cancelled
            }
        );
        h.store
            .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
            .await
            .unwrap();
        assert!(matches!(
            h.store
                .accept_step(&lease, &receipt.attempt_id, &audio)
                .await,
            Err(DurableError::LeaseLost)
        ));
        let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_observations),(SELECT count(*) FROM durable_settlements),(SELECT count(*) FROM durable_steps WHERE accepted_attempt_id IS NOT NULL)", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 1);
        assert_eq!(row.get::<_, i64>(1), 1);
        assert_eq!(row.get::<_, i64>(2), 0);
        if event == RunEvent::Pause {
            h.store
                .control_job(
                    ALICE,
                    &job.run_id,
                    &Uuid::new_v4().to_string(),
                    RunEvent::Resume,
                )
                .await
                .unwrap();
            let next = h.lease("resumed").await;
            match h.store.prepare_step(&next).await.unwrap() {
                StepWork::Recover(saved, output) => {
                    assert_eq!(saved.intent.attempt_id, receipt.attempt_id);
                    assert_eq!(output.receipt, receipt);
                    h.store
                        .accept_step(&next, &receipt.attempt_id, &audio)
                        .await
                        .unwrap();
                }
                _ => panic!("resume must recover the exact observed output, never dispatch again"),
            }
        } else {
            assert!(matches!(
                h.store
                    .control_job(
                        ALICE,
                        &job.run_id,
                        &Uuid::new_v4().to_string(),
                        RunEvent::Resume
                    )
                    .await,
                Err(DurableError::Conflict)
            ));
        }
        assert_invariants(&h).await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_retries_racing_for_the_last_budget_unit_create_only_one_intent() {
    let h = Harness::new(4).await;
    h.enqueue(ExecutionMode::SyntheticFake).await;
    let mut attempts = Vec::new();
    for worker in ["initial-a", "initial-b", "initial-c", "initial-d"] {
        let lease = h.lease(worker).await;
        attempts.push(h.dispatch(&lease).await);
    }
    for (number, work) in attempts.iter().enumerate() {
        let mut receipt = work.intent.clone();
        receipt.outcome = ReceiptOutcome::Failure {
            class: FailureClass::Timeout,
            delivery: if number < 3 {
                DeliveryState::ConfirmedNotExecuted
            } else {
                DeliveryState::NotSent
            },
        };
        if number < 3 {
            receipt.charge = ChargeObservation::Reported {
                currency: ProductionCurrency::VND,
                amount_minor: 1,
                reference: "synthetic-definite-failure-fee-budget-race".into(),
            };
        }
        h.store
            .observe_attempt(work, &receipt, None, None)
            .await
            .unwrap();
    }
    h.wait_backoff().await;
    let first = h.lease("last-a").await;
    let second = h.lease("last-b").await;
    assert_ne!(first.step_id, second.step_id);
    let gate = Arc::new(Barrier::new(3));
    let mut tasks = Vec::new();
    for lease in [first, second] {
        let store = h.reopened();
        let gate = gate.clone();
        tasks.push(tokio::spawn(async move {
            gate.wait().await;
            store.prepare_step(&lease).await
        }));
    }
    gate.wait().await;
    let mut dispatches = 0;
    let mut blocked = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(StepWork::Dispatch(_)) => dispatches += 1,
            Ok(StepWork::Blocked) => blocked += 1,
            other => panic!("last-budget race returned an unexpected outcome: {other:?}"),
        }
    }
    assert_eq!((dispatches, blocked), (1, 1));
    let row = h.admin.query_one("SELECT reserved_minor,settled_minor,limit_minor,(SELECT count(*) FROM durable_attempts) FROM durable_budgets", &[]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 3);
    assert_eq!(row.get::<_, i64>(2), 4);
    assert_eq!(row.get::<_, i64>(3), 5);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn definitive_failures_respect_backoff_and_the_persisted_retry_bound() {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let first = h.lease("retry-one").await;
    let work = h.dispatch(&first).await;
    let mut failure = work.intent.clone();
    failure.outcome = ReceiptOutcome::Failure {
        class: FailureClass::RateLimit,
        delivery: DeliveryState::ConfirmedNotExecuted,
    };
    h.store
        .observe_attempt(&work, &failure, None, None)
        .await
        .unwrap();
    let row = h
        .admin
        .query_one(
            "SELECT status,not_before>clock_timestamp() FROM durable_steps WHERE id=$1",
            &[&first.step_id],
        )
        .await
        .unwrap();
    assert_eq!(row.get::<_, String>(0), "pending");
    assert!(row.get::<_, bool>(1), "retry backoff was not persisted");
    // Hide other lines so this fixture observes the real immutable retry deadline only.
    h.admin
        .execute(
            "UPDATE durable_steps SET not_before=clock_timestamp()+interval '1 hour' WHERE id<>$1",
            &[&first.step_id],
        )
        .await
        .unwrap();
    let retry = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        loop {
            if let Some(lease) = h
                .store
                .claim_step("retry-two", ExecutionMode::SyntheticFake, ttl())
                .await
                .unwrap()
            {
                break lease;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("legitimate retry did not become claimable within backoff bound");
    assert_eq!(retry.step_id, first.step_id);
    let second = h.dispatch(&retry).await;
    assert_ne!(second.intent.attempt_id, work.intent.attempt_id);
    let mut failure = second.intent.clone();
    failure.outcome = ReceiptOutcome::Failure {
        class: FailureClass::Timeout,
        delivery: DeliveryState::NotSent,
    };
    h.store
        .observe_attempt(&second, &failure, None, None)
        .await
        .unwrap();
    assert_eq!(
        h.store.job_status(ALICE, &job.run_id).await.unwrap().status,
        RunStatus::Failed
    );
    assert!(h
        .store
        .claim_step("bounded", ExecutionMode::SyntheticFake, ttl())
        .await
        .unwrap()
        .is_none());
    let attempts: i64 = h
        .admin
        .query_one(
            "SELECT count(*) FROM durable_attempts WHERE step_id=$1",
            &[&first.step_id],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(attempts, 2);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn contradictory_late_output_after_nonexecution_attestation_is_preserved_and_quarantined() {
    let h = Harness::new(4).await;
    let job = h.enqueue(ExecutionMode::SyntheticFake).await;
    let lease = h.lease("late-outcome").await;
    let work = h.dispatch(&lease).await;
    let (receipt, audio, path) = h.outcome(&work).await;
    h.store
        .control_job(
            ALICE,
            &job.run_id,
            &Uuid::new_v4().to_string(),
            RunEvent::Pause,
        )
        .await
        .unwrap();
    h.store
        .resolve_not_executed(
            ALICE,
            &job.run_id,
            &receipt.attempt_id,
            "Synthetic operator attestation intentionally contradicted by a later fixture",
        )
        .await
        .unwrap();
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    let state = h.store.job_status(ALICE, &job.run_id).await.unwrap();
    assert_eq!(state.status, RunStatus::Paused);
    assert_eq!(state.reason.as_deref(), Some("resolution_contradiction"));
    assert!(matches!(
        h.store
            .control_job(
                ALICE,
                &job.run_id,
                &Uuid::new_v4().to_string(),
                RunEvent::Resume
            )
            .await,
        Err(DurableError::ReconciliationRequired)
    ));
    let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_attempts),(SELECT count(*) FROM durable_observations),(SELECT count(*) FROM durable_resolutions),(SELECT count(*) FROM durable_steps WHERE accepted_attempt_id IS NOT NULL)", &[]).await.unwrap();
    assert_eq!(row.get::<_, i64>(0), 1);
    assert_eq!(row.get::<_, i64>(1), 1);
    assert_eq!(row.get::<_, i64>(2), 1);
    assert_eq!(row.get::<_, i64>(3), 0);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn explicit_synthetic_zero_fee_settles_zero_while_unknown_output_keeps_provisional_estimate()
{
    let h = Harness::new(4).await;
    h.enqueue(ExecutionMode::SyntheticFake).await;
    let first = h.lease("explicit-zero-fee").await;
    let work = h.dispatch(&first).await;
    let mut receipt = work.intent.clone();
    receipt.outcome = ReceiptOutcome::Failure {
        class: FailureClass::Timeout,
        delivery: DeliveryState::ConfirmedNotExecuted,
    };
    receipt.charge = ChargeObservation::Reported {
        currency: ProductionCurrency::VND,
        amount_minor: 0,
        reference: "synthetic-definite-nonexecution-zero-fee".into(),
    };
    h.store
        .observe_attempt(&work, &receipt, None, None)
        .await
        .unwrap();
    let zero = h.admin.query_one("SELECT b.reserved_minor,b.settled_minor,r.amount_minor,x.amount_minor,x.kind FROM durable_budgets b JOIN durable_reservations r ON r.run_id=b.run_id JOIN durable_settlements x ON x.reservation_id=r.id", &[]).await.unwrap();
    assert_eq!(zero.get::<_, i64>(0), 0);
    assert_eq!(zero.get::<_, i64>(1), 0);
    assert_eq!(zero.get::<_, i64>(2), 1);
    assert_eq!(zero.get::<_, i64>(3), 0);
    assert_eq!(zero.get::<_, String>(4), "reported_synthetic");
    let second = h.lease("unknown-charge-output").await;
    let work = h.dispatch(&second).await;
    let (mut receipt, audio, path) = h.outcome(&work).await;
    receipt.charge = ChargeObservation::Unknown {};
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    let unknown = h.admin.query_one("SELECT b.reserved_minor,b.settled_minor,x.amount_minor,x.kind,o.receipt_bytes FROM durable_budgets b JOIN durable_reservations r ON r.run_id=b.run_id JOIN durable_settlements x ON x.reservation_id=r.id JOIN durable_attempts a ON a.reservation_id=r.id JOIN durable_observations o ON o.attempt_id=a.id WHERE a.id=$1", &[&receipt.attempt_id]).await.unwrap();
    assert_eq!(unknown.get::<_, i64>(0), 0);
    assert_eq!(unknown.get::<_, i64>(1), 1);
    assert_eq!(unknown.get::<_, i64>(2), 1);
    assert_eq!(unknown.get::<_, String>(3), "provisional_local");
    let recorded: TtsReceipt = serde_json::from_slice(&unknown.get::<_, Vec<u8>>(4)).unwrap();
    assert_eq!(recorded.charge, ChargeObservation::Unknown {});
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn stale_definitive_failure_preserves_backoff_before_another_dispatch() {
    let h = Harness::new(4).await;
    h.enqueue(ExecutionMode::SyntheticFake).await;
    let first = h.lease("stalled-rate-limit").await;
    let work = h.dispatch(&first).await;
    h.expire(&first).await;
    h.admin
        .execute(
            "UPDATE durable_steps SET not_before=clock_timestamp()+interval '1 hour' WHERE id<>$1",
            &[&first.step_id],
        )
        .await
        .unwrap();
    let mut failure = work.intent.clone();
    failure.outcome = ReceiptOutcome::Failure {
        class: FailureClass::RateLimit,
        delivery: DeliveryState::ConfirmedNotExecuted,
    };
    h.store
        .observe_attempt(&work, &failure, None, None)
        .await
        .unwrap();
    let next = h.lease("recovery-before-deadline").await;
    assert_eq!(next.step_id, first.step_id);
    assert!(matches!(
        h.store.prepare_step(&next).await.unwrap(),
        StepWork::Blocked
    ));
    let row = h.admin.query_one("SELECT s.status,s.not_before>clock_timestamp(),j.status,(SELECT count(*) FROM durable_attempts) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE s.id=$1", &[&first.step_id]).await.unwrap();
    assert_eq!(row.get::<_, String>(0), "pending");
    assert!(row.get::<_, bool>(1));
    assert_eq!(row.get::<_, String>(2), "running");
    assert_eq!(row.get::<_, i64>(3), 1);
    assert_invariants(&h).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn revoked_current_authority_blocks_final_dispatch_and_retains_not_sent_observation() {
    for revoked_actor in [false, true] {
        let h = Harness::new(4).await;
        h.enqueue(ExecutionMode::SyntheticFake).await;
        let lease = h.lease("intent-before-revocation").await;
        let work = h.dispatch(&lease).await;
        if revoked_actor {
            h.admin
                .batch_execute("UPDATE actors SET active=false WHERE id='alice'")
                .await
                .unwrap();
            assert!(matches!(
                h.store.authorize_dispatch(&work).await,
                Err(DurableError::Store(
                    cantos_api::postgres::StoreError::Unauthenticated
                ))
            ));
        } else {
            let mut revoked = claim(
                "voice_fixture",
                ProductionRightsSubject::Voice {
                    provider_id: "espeak-ng-local".into(),
                    model_id: "espeak-ng-1.52.0".into(),
                    voice_id: "vi".into(),
                },
            );
            revoked.status = ProductionRightsStatus::Revoked;
            h.store
                .save_production_rights(
                    ALICE,
                    &h.script,
                    SaveProductionRightsRequest {
                        operation_id: Uuid::new_v4().to_string(),
                        expected_version: 1,
                        claim: revoked,
                    },
                )
                .await
                .unwrap();
            assert!(matches!(h.store.authorize_dispatch(&work).await,
                Err(DurableError::Store(cantos_api::postgres::StoreError::ProductionBlocked(issues)))
                    if issues.iter().any(|issue| issue.rule == "rights_changed")));
        }
        let mut receipt = work.intent.clone();
        receipt.outcome = ReceiptOutcome::Failure {
            class: FailureClass::Cancelled,
            delivery: DeliveryState::NotSent,
        };
        h.store
            .observe_attempt(&work, &receipt, None, None)
            .await
            .unwrap();
        let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_dispatches),(SELECT count(*) FROM durable_observations),(SELECT count(*) FROM durable_steps WHERE accepted_attempt_id IS NOT NULL)", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 0);
        assert_eq!(row.get::<_, i64>(1), 1);
        assert_eq!(row.get::<_, i64>(2), 0);
        assert_invariants(&h).await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn stale_queued_job_pauses_durably_and_does_not_starve_a_later_valid_job() {
    let h = Harness::new(4).await;
    let stale = h.enqueue(ExecutionMode::SyntheticFake).await;
    let mut script: Value = serde_json::from_str(SCRIPT).unwrap();
    script["episode"]["acts"][0]["scenes"][0]["dialogues"][0]["text"] =
        Value::String("Bản mới đã được xét duyệt.".into());
    h.store
        .save(
            ALICE,
            &h.script,
            SaveRevisionRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_revision: 1,
                script_json: script.to_string(),
            },
        )
        .await
        .unwrap();
    h.store
        .review(
            ALICE,
            &h.script,
            ReviewRequest {
                operation_id: Uuid::new_v4().to_string(),
                revision: 2,
            },
        )
        .await
        .unwrap();
    h.store
        .save_production_settings(
            ALICE,
            &h.script,
            SaveProductionSettingsRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_revision: 2,
                expected_settings_version: 1,
                settings: settings(4),
            },
        )
        .await
        .unwrap();
    let preview = h.store.production_preview(ALICE, &h.script).await.unwrap();
    let fresh_snapshot = h
        .store
        .freeze_production(
            ALICE,
            &h.script,
            FreezeProductionRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_revision: 2,
                settings_version: preview.settings_version,
                input_digest: preview.input_digest,
            },
        )
        .await
        .unwrap();
    h.store
        .approve_production(
            ALICE,
            &h.script,
            &fresh_snapshot.id,
            ApproveProductionRequest {
                operation_id: Uuid::new_v4().to_string(),
                input_digest: fresh_snapshot.document.input_digest.clone(),
            },
        )
        .await
        .unwrap();
    let valid = h
        .store
        .enqueue_job(
            ALICE,
            &h.script,
            &fresh_snapshot.id,
            &Uuid::new_v4().to_string(),
            ExecutionMode::SyntheticFake,
            policy(),
        )
        .await
        .unwrap();
    let first = h.lease("must-pause-stale").await;
    assert_eq!(first.job_id, stale.job_id);
    assert!(matches!(
        h.store.prepare_step(&first).await.unwrap(),
        StepWork::Blocked
    ));
    let stale_state = h.store.job_status(ALICE, &stale.run_id).await.unwrap();
    assert_eq!(stale_state.status, RunStatus::Paused);
    assert_eq!(stale_state.reason.as_deref(), Some("authorization_invalid"));
    let second = h.lease("later-valid").await;
    assert_eq!(second.job_id, valid.job_id);
    let work = h.dispatch(&second).await;
    assert_eq!(work.document, fresh_snapshot.document);
    let (receipt, audio, path) = h.outcome(&work).await;
    h.store
        .observe_attempt(&work, &receipt, Some(&path), Some(&audio))
        .await
        .unwrap();
    h.store
        .accept_step(&second, &receipt.attempt_id, &audio)
        .await
        .unwrap();
    assert_eq!(
        h.store
            .job_status(ALICE, &valid.run_id)
            .await
            .unwrap()
            .completed_steps,
        1
    );
    let stale_attempts: i64 = h.admin.query_one("SELECT count(*) FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id WHERE s.job_id=$1", &[&stale.job_id]).await.unwrap().get(0);
    assert_eq!(stale_attempts, 0);
    assert_invariants(&h).await;
}

#[cfg(unix)]
mod process_crashes {
    use std::{
        fs::{self, File},
        io::{BufRead, BufReader, Write},
        process::{Child, Command, Stdio},
        sync::mpsc::{self, Receiver},
        thread,
        time::{Duration, Instant},
    };

    use super::*;

    struct OwnedWorker {
        child: Child,
        events: Receiver<Value>,
    }

    impl OwnedWorker {
        fn spawn(h: &Harness, point: Option<&str>) -> Self {
            Self::spawn_runtime(h, point, None)
        }

        fn spawn_runtime(h: &Harness, point: Option<&str>, runtime: Option<&str>) -> Self {
            Self::spawn_options(h, point, runtime, true)
        }

        fn spawn_recurring(h: &Harness) -> Self {
            Self::spawn_options(h, None, None, false)
        }

        fn spawn_options(
            h: &Harness,
            point: Option<&str>,
            runtime: Option<&str>,
            once: bool,
        ) -> Self {
            let mut command = Command::new(env!("CARGO_BIN_EXE_cantos-worker"));
            let db = h.app_config.get_dbname().unwrap();
            let port = h.app_config.get_ports()[0];
            let url = format!("postgresql://cantos_app@127.0.0.1:{port}/{db}");
            let log_id = Uuid::new_v4().to_string();
            command
                .env_clear()
                .env("CANTOS_ENV", "development")
                .env("CANTOS_DATABASE_URL", url)
                .arg("--output-root")
                .arg(fs::canonicalize(&h.root).unwrap())
                .arg("--worker-id")
                .arg(format!("process-{log_id}"))
                .arg("--lease-seconds")
                .arg("30")
                .stdin(Stdio::piped())
                .stdout(Stdio::piped())
                .stderr(File::create(h.root.join(format!("{log_id}-stderr.log"))).unwrap());
            if once {
                command.arg("--once");
            }
            if let Some(point) = point {
                command.arg("--failpoint").arg(point);
            }
            if let Some(runtime) = runtime {
                command.arg("--local-espeak").arg(runtime);
            } else {
                command.arg("--test-fake");
            }
            let mut child = command.spawn().unwrap();
            let stdout = child.stdout.take().unwrap();
            let (send, events) = mpsc::channel();
            let mut log = File::create(h.root.join(format!("{log_id}-stdout.log"))).unwrap();
            thread::spawn(move || {
                for line in BufReader::new(stdout).lines() {
                    let line = line.unwrap();
                    writeln!(log, "{line}").unwrap();
                    log.flush().unwrap();
                    if let Ok(event) = serde_json::from_str(&line) {
                        let _ = send.send(event);
                    }
                }
            });
            Self { child, events }
        }

        fn reached(&self, point: &str) -> Value {
            let event = self.wait_event("failpoint");
            assert_eq!(event["point"], point);
            event
        }

        fn wait_event(&self, name: &str) -> Value {
            let deadline = Instant::now() + Duration::from_secs(10);
            loop {
                let event = self
                    .events
                    .recv_timeout(deadline.saturating_duration_since(Instant::now()))
                    .expect("worker never reached requested crash point");
                if event["event"] == name {
                    return event;
                }
            }
        }

        fn kill(&mut self) {
            self.child.kill().unwrap();
            assert!(!self.child.wait().unwrap().success());
        }

        fn proceed(&mut self) {
            let stdin = self.child.stdin.as_mut().unwrap();
            writeln!(stdin, "continue").unwrap();
            stdin.flush().unwrap();
        }

        fn finish(&mut self) {
            self.finish_expected(true);
        }

        fn finish_expected(&mut self, success: bool) {
            let deadline = Instant::now() + Duration::from_secs(15);
            loop {
                if let Some(status) = self.child.try_wait().unwrap() {
                    assert_eq!(
                        status.success(),
                        success,
                        "unexpected worker exit: {status}"
                    );
                    if !success {
                        assert_eq!(status.code(), Some(1));
                    }
                    return;
                }
                assert!(
                    Instant::now() < deadline,
                    "owned worker did not finish within bounded time"
                );
                thread::yield_now();
            }
        }
    }

    impl Drop for OwnedWorker {
        fn drop(&mut self) {
            if self.child.try_wait().ok().flatten().is_none() {
                let _ = self.child.kill();
                let _ = self.child.wait();
            }
        }
    }

    fn ledger(h: &Harness) -> Vec<Value> {
        let directory = h.root.join(".fake-provider");
        if !directory.exists() {
            return vec![];
        }
        fs::read_dir(directory)
            .unwrap()
            .map(|entry| serde_json::from_slice(&fs::read(entry.unwrap().path()).unwrap()).unwrap())
            .collect()
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn actual_kill_after_claim_restarts_from_postgresql_and_completes_once_per_line() {
        let h = Harness::new(4).await;
        let job = h.enqueue(ExecutionMode::SyntheticFake).await;
        let mut killed = OwnedWorker::spawn(&h, Some("after_claim"));
        let event = killed.reached("after_claim");
        assert_eq!(event["job_id"], job.job_id);
        killed.kill();
        assert!(ledger(&h).is_empty());
        h.admin.batch_execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE status='leased'").await.unwrap();
        for _ in 0..4 {
            OwnedWorker::spawn(&h, None).finish();
        }
        let state = h.store.job_status(ALICE, &job.run_id).await.unwrap();
        assert_eq!(state.status, RunStatus::Completed);
        assert_eq!(state.completed_steps, 4);
        let executions = ledger(&h);
        assert_eq!(executions.len(), 4);
        let fingerprints: std::collections::BTreeSet<_> = executions
            .iter()
            .map(|event| event["input_fingerprint"].as_str().unwrap())
            .collect();
        assert_eq!(
            fingerprints.len(),
            4,
            "provider ledger contains duplicate execution for one frozen line"
        );
        OwnedWorker::spawn(&h, None).finish();
        assert_eq!(ledger(&h).len(), 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn actual_kill_before_or_after_provider_execution_never_blindly_redispatches() {
        for point in ["after_intent", "after_provider"] {
            let h = Harness::new(4).await;
            let job = h.enqueue(ExecutionMode::SyntheticFake).await;
            let mut killed = OwnedWorker::spawn(&h, Some(point));
            let event = killed.reached(point);
            assert!(event["attempt_id"].as_str().is_some());
            killed.kill();
            let count = usize::from(point == "after_provider");
            assert_eq!(ledger(&h).len(), count);
            h.admin.batch_execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE status='leased'").await.unwrap();
            OwnedWorker::spawn(&h, None).finish();
            let state = h.store.job_status(ALICE, &job.run_id).await.unwrap();
            assert_eq!(state.status, RunStatus::Paused);
            assert_eq!(state.reason.as_deref(), Some("uncertain_attempt"));
            assert_eq!(ledger(&h).len(), count);
            let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_attempts),(SELECT count(*) FROM durable_observations),(SELECT count(*) FROM durable_steps WHERE status='succeeded')", &[]).await.unwrap();
            assert_eq!(row.get::<_, i64>(0), 1);
            assert_eq!(row.get::<_, i64>(1), 0);
            assert_eq!(row.get::<_, i64>(2), 0);
            assert_invariants(&h).await;
        }
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn actual_kill_after_observation_recovers_exact_output_without_a_second_provider_call() {
        let h = Harness::new(4).await;
        let job = h.enqueue(ExecutionMode::SyntheticFake).await;
        let mut killed = OwnedWorker::spawn(&h, Some("after_observation"));
        let event = killed.reached("after_observation");
        let attempt = event["attempt_id"].as_str().unwrap();
        killed.kill();
        assert_eq!(ledger(&h).len(), 1);
        h.admin.batch_execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE status='leased'").await.unwrap();
        OwnedWorker::spawn(&h, None).finish();
        assert_eq!(ledger(&h).len(), 1);
        let row = h
            .admin
            .query_one(
                "SELECT accepted_attempt_id FROM durable_steps WHERE status='succeeded'",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(row.get::<_, String>(0), attempt);
        for _ in 0..3 {
            OwnedWorker::spawn(&h, None).finish();
        }
        assert_eq!(
            h.store.job_status(ALICE, &job.run_id).await.unwrap().status,
            RunStatus::Completed
        );
        assert_eq!(ledger(&h).len(), 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn cancellation_after_intent_before_send_blocks_the_actual_provider_process() {
        let h = Harness::new(4).await;
        let job = h.enqueue(ExecutionMode::SyntheticFake).await;
        let mut worker = OwnedWorker::spawn(&h, Some("after_intent"));
        worker.reached("after_intent");
        h.store
            .control_job(
                ALICE,
                &job.run_id,
                &Uuid::new_v4().to_string(),
                RunEvent::Cancel,
            )
            .await
            .unwrap();
        worker.proceed();
        worker.finish_expected(false);
        assert!(ledger(&h).is_empty());
        let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_dispatches),(SELECT count(*) FROM durable_observations),(SELECT count(*) FROM durable_steps WHERE accepted_attempt_id IS NOT NULL)", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 0);
        assert_eq!(row.get::<_, i64>(1), 1);
        assert_eq!(row.get::<_, i64>(2), 0);
        assert_eq!(
            h.store.job_status(ALICE, &job.run_id).await.unwrap().status,
            RunStatus::Cancelled
        );
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "explicit installed local eSpeak NG runtime and synthetic source authorization required"]
    async fn installed_local_runtime_job_completes_with_native_audio_and_unknown_actual_charge() {
        let runtime = env::var("CANTOS_DURABLE_ESPEAK_RUNTIME")
            .expect("explicit absolute installed local eSpeak runtime required");
        assert!(std::path::Path::new(&runtime).is_absolute());
        let h = Harness::new(4).await;
        let job = h.enqueue(ExecutionMode::LocalEspeak).await;
        for _ in 0..4 {
            OwnedWorker::spawn_runtime(&h, None, Some(&runtime)).finish();
        }
        let summary = h.store.job_status(ALICE, &job.run_id).await.unwrap();
        assert_eq!(summary.status, RunStatus::Completed);
        assert_eq!(summary.completed_steps, 4);
        assert!(ledger(&h).is_empty());
        let row = h.admin.query_one("SELECT reserved_minor,settled_minor,(SELECT count(*) FROM durable_dispatches) FROM durable_budgets", &[]).await.unwrap();
        assert_eq!(row.get::<_, i64>(0), 0);
        assert_eq!(row.get::<_, i64>(1), 0);
        assert_eq!(row.get::<_, i64>(2), 4);
        let outputs = h
            .admin
            .query(
                "SELECT receipt_bytes,output_path FROM durable_observations",
                &[],
            )
            .await
            .unwrap();
        assert_eq!(outputs.len(), 4);
        let mut facts = Vec::new();
        for row in outputs {
            let receipt: TtsReceipt = serde_json::from_slice(&row.get::<_, Vec<u8>>(0)).unwrap();
            assert_eq!(receipt.charge, ChargeObservation::Unknown {});
            let path: String = row.get(1);
            let audio = fs::read(&path).unwrap();
            let audit =
                cantos_api::tts::audit_receipt(&h.snapshot.document, &receipt, Some(&audio))
                    .unwrap();
            assert_eq!(audit.audio().unwrap().sample_rate_hz(), 22_050);
            let inspected = audit.audio().unwrap();
            facts.push(serde_json::json!({
                "dialogue_id": receipt.dialogue_id,
                "attempt_id": receipt.attempt_id,
                "operation_id": receipt.operation_id,
                "provider": receipt.provider,
                "path": path,
                "audio_sha256": inspected.file_sha256(),
                "frames": inspected.frames(),
                "file_bytes": audio.len(),
                "sample_rate_hz": inspected.sample_rate_hz(),
                "channels": inspected.channels(),
                "bits_per_sample": inspected.bits_per_sample(),
                "inspector_version": inspected.inspector_version(),
                "output_profile_version": inspected.output_profile_version(),
                "actual_charge": receipt.charge,
            }));
        }
        let facts_path = h.root.join("installed-local-audio-facts.json");
        fs::write(
            &facts_path,
            serde_json::to_vec_pretty(&serde_json::json!({
                "evidence": "provider-live-tested-local-espeak-only-structural-audio-measured",
                "source": "repository-original-two-scenes-fixture-synthetic-private-rights",
                "run_id": job.run_id,
                "snapshot_id": h.snapshot.id,
                "script_id": h.script,
                "script_revision": h.snapshot.document.revision.revision,
                "input_digest": h.snapshot.document.input_digest,
                "audio": facts,
                "listening": "NOT_RUN",
                "commercial_dispatch": "NOT_RUN",
            }))
            .unwrap(),
        )
        .unwrap();
        println!(
            "Installed eSpeak durable local output evidence: {}",
            facts_path.display()
        );
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn unavailable_or_invalid_recorded_output_is_quarantined_and_later_job_progresses() {
        for invalid in [false, true] {
            let h = Harness::new(4).await;
            let faulty = h.enqueue(ExecutionMode::SyntheticFake).await;
            let mut killed = OwnedWorker::spawn(&h, Some("after_observation"));
            let event = killed.reached("after_observation");
            killed.kill();
            let path: String = h
                .admin
                .query_one(
                    "SELECT output_path FROM durable_observations WHERE attempt_id=$1",
                    &[&event["attempt_id"].as_str().unwrap()],
                )
                .await
                .unwrap()
                .get(0);
            let path = std::path::PathBuf::from(path);
            // Preserve every original generated byte. Only its recorded locator is unavailable.
            fs::rename(&path, path.with_file_name("preserved-original-output.wav")).unwrap();
            if invalid {
                fs::write(&path, b"synthetic malformed output fixture").unwrap();
            }
            h.admin.batch_execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE status='leased'").await.unwrap();
            let valid = h.enqueue(ExecutionMode::SyntheticFake).await;
            let mut recovery = OwnedWorker::spawn_recurring(&h);
            let quarantined = recovery.wait_event("quarantined");
            assert_eq!(quarantined["job_id"], faulty.job_id);
            let accepted = recovery.wait_event("accepted");
            assert_eq!(accepted["job_id"], valid.job_id);
            recovery.kill();
            let state = h.store.job_status(ALICE, &faulty.run_id).await.unwrap();
            assert_eq!(state.status, RunStatus::Paused);
            assert_eq!(
                state.reason.as_deref(),
                Some(if invalid {
                    "recorded_output_invalid"
                } else {
                    "recorded_output_unavailable"
                })
            );
            assert!(
                h.store
                    .job_status(ALICE, &valid.run_id)
                    .await
                    .unwrap()
                    .completed_steps
                    >= 1
            );
            let executions = ledger(&h);
            assert!(executions.len() >= 2 && executions.len() <= 5);
            assert_eq!(
                executions
                    .iter()
                    .filter(|execution| execution["attempt_id"] == event["attempt_id"])
                    .count(),
                1
            );
            let old_accepted: i64 = h.admin.query_one("SELECT count(*) FROM durable_steps WHERE job_id=$1 AND accepted_attempt_id IS NOT NULL", &[&faulty.job_id]).await.unwrap().get(0);
            assert_eq!(old_accepted, 0);
            assert_invariants(&h).await;
        }
    }
}
