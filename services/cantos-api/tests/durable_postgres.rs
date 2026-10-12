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
        Self::build(limit, upgrade, true).await
    }

    /// A database seeded at migration 6 (before any durable table) that is *not* yet upgraded:
    /// the caller applies later migrations itself, with real rows in place.
    async fn new_before_durable_migrations(limit: u64) -> Self {
        Self::build(limit, true, false).await
    }

    async fn build(limit: u64, upgrade: bool, finish_upgrade: bool) -> Self {
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
        if upgrade && finish_upgrade {
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

impl Harness {
    fn canonical_root(&self) -> PathBuf {
        std::fs::canonicalize(&self.root).unwrap()
    }

    /// The same worker loop the `cantos-worker` binary runs, one claim per call.
    async fn run_worker_once(&self, worker: &str) {
        cantos_api::worker::run_worker(
            self.store.clone(),
            cantos_api::worker::WorkerConfig {
                provider: cantos_api::worker::WorkerProvider::SyntheticFake,
                output_root: self.canonical_root(),
                worker_id: worker.into(),
                lease_duration: ttl(),
                once: true,
                failpoint: None,
            },
        )
        .await
        .unwrap();
    }

    /// Run workers until the job stops making progress on its own.
    async fn drain(&self, run: &str) -> JobSummary {
        for round in 0..40 {
            let state = self.store.job_status(ALICE, run).await.unwrap();
            if !matches!(state.status, RunStatus::Queued | RunStatus::Running) {
                return state;
            }
            self.run_worker_once(&format!("drain-{round}")).await;
        }
        panic!("job did not settle within the bounded number of worker iterations");
    }

    /// Provider executions according to the fake provider's own create-only ledger.
    fn executions(&self) -> usize {
        let directory = self.root.join(".fake-provider");
        if !directory.exists() {
            return 0;
        }
        std::fs::read_dir(directory).unwrap().count()
    }

    async fn count(&self, sql: &str, run: &str) -> i64 {
        self.admin.query_one(sql, &[&run]).await.unwrap().get(0)
    }

    async fn count_all(&self, sql: &str) -> i64 {
        self.admin.query_one(sql, &[]).await.unwrap().get(0)
    }

    /// Save revision `expected + 1` from the original script with text edits, review it and
    /// freeze (and optionally approve) a production snapshot over it.
    async fn revise(
        &self,
        expected: u64,
        edits: &[(&str, &str)],
        approve: bool,
    ) -> ProductionSnapshotResponse {
        self.revise_with(
            expected,
            |mut json| {
                for (from, to) in edits {
                    assert!(json.contains(from), "fixture no longer contains {from:?}");
                    json = json.replace(from, to);
                }
                json
            },
            approve,
        )
        .await
    }

    /// Like `revise`, with the whole edit expressed over the original script text.
    async fn revise_with(
        &self,
        expected: u64,
        edit: impl FnOnce(String) -> String,
        approve: bool,
    ) -> ProductionSnapshotResponse {
        let json = edit(SCRIPT.to_owned());
        self.store
            .save(
                ALICE,
                &self.script,
                SaveRevisionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: expected,
                    script_json: json,
                },
            )
            .await
            .unwrap();
        self.store
            .review(
                ALICE,
                &self.script,
                ReviewRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    revision: expected + 1,
                },
            )
            .await
            .unwrap();
        // Casting is scoped to one script revision: carry the same settings forward.
        let current = self
            .store
            .production_state(ALICE, &self.script)
            .await
            .unwrap()
            .settings
            .expect("settings were saved");
        self.store
            .save_production_settings(
                ALICE,
                &self.script,
                SaveProductionSettingsRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: expected + 1,
                    expected_settings_version: current.version,
                    settings: current.settings,
                },
            )
            .await
            .unwrap();
        let preview = self
            .store
            .production_preview(ALICE, &self.script)
            .await
            .unwrap();
        let snapshot = self
            .store
            .freeze_production(
                ALICE,
                &self.script,
                FreezeProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: expected + 1,
                    settings_version: preview.settings_version,
                    input_digest: preview.input_digest,
                },
            )
            .await
            .unwrap();
        assert!(
            snapshot.eligibility.inputs_eligible,
            "{:?}",
            snapshot.eligibility.findings
        );
        if approve {
            self.store
                .approve_production(
                    ALICE,
                    &self.script,
                    &snapshot.id,
                    ApproveProductionRequest {
                        operation_id: Uuid::new_v4().to_string(),
                        input_digest: snapshot.document.input_digest.clone(),
                    },
                )
                .await
                .unwrap();
        }
        snapshot
    }

    async fn enqueue_on(&self, snapshot: &str, mode: ExecutionMode) -> JobSummary {
        self.store
            .enqueue_job(
                ALICE,
                &self.script,
                snapshot,
                &Uuid::new_v4().to_string(),
                mode,
                policy(),
            )
            .await
            .unwrap()
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
    let rows = h.admin.query("SELECT s.dialogue_id,s.input_fingerprint,s.status,s.accepted_attempt_id,a.intent_bytes,o.receipt_bytes,r.input_digest,p.document_bytes,s.reused_clip_id,s.speech_fingerprint,c.speech_fingerprint,c.owner_id,r.created_by,c.producer,r.mode FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id JOIN durable_runs r ON r.id=j.run_id JOIN production_snapshots p ON p.id=r.snapshot_id LEFT JOIN durable_attempts a ON a.id=s.accepted_attempt_id LEFT JOIN durable_observations o ON o.attempt_id=s.accepted_attempt_id LEFT JOIN speech_clips c ON c.id=s.reused_clip_id", &[]).await.unwrap();
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
        let reused: Option<String> = row.get(8);
        // A step succeeds through exactly one route: a provider attempt or a verified clip.
        assert!(accepted.is_none() || reused.is_none());
        assert_eq!(
            row.get::<_, String>(2) == "succeeded",
            accepted.is_some() || reused.is_some()
        );
        if reused.is_some() {
            // A reused clip carries exactly this line's position-free identity, creator and
            // producer, recomputed here from the frozen document rather than read back.
            let expected = cantos_api::speech_fingerprint::SpeechInput::from_resolved(line)
                .unwrap()
                .fingerprint();
            assert_eq!(row.get::<_, String>(9), expected.as_str());
            assert_eq!(row.get::<_, String>(10), expected.as_str());
            assert_eq!(row.get::<_, String>(11), row.get::<_, String>(12));
            assert_eq!(row.get::<_, String>(13), row.get::<_, String>(14));
        }
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
    // A cache hit is never a provider send: no attempt, reservation, dispatch or observation
    // may hang off a reused step, and every accepted attempt of a fingerprinted step left a registry entry.
    let reused_with_attempts: i64 = h.admin.query_one("SELECT count(*) FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id WHERE s.reused_clip_id IS NOT NULL", &[]).await.unwrap().get(0);
    assert_eq!(
        reused_with_attempts, 0,
        "a reused step has a provider attempt"
    );
    let reused_with_reservations: i64 = h.admin.query_one("SELECT count(*) FROM durable_reservations r JOIN durable_steps s ON s.id=r.step_id WHERE s.reused_clip_id IS NOT NULL", &[]).await.unwrap().get(0);
    assert_eq!(reused_with_reservations, 0, "a reused step reserved budget");
    let unregistered: i64 = h.admin.query_one("SELECT count(*) FROM durable_steps s WHERE s.accepted_attempt_id IS NOT NULL AND s.speech_fingerprint IS NOT NULL AND NOT EXISTS(SELECT 1 FROM speech_clips c WHERE c.attempt_id=s.accepted_attempt_id)", &[]).await.unwrap().get(0);
    assert_eq!(
        unregistered, 0,
        "an accepted attempt of a fingerprinted step has no registry entry"
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

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn actual_kill_between_probe_and_selection_leaves_no_trace_and_restart_reuses_without_the_provider(
    ) {
        let h = Harness::new(4).await;
        let first = h.enqueue(ExecutionMode::SyntheticFake).await;
        for _ in 0..4 {
            OwnedWorker::spawn(&h, None).finish();
        }
        assert_eq!(
            h.store
                .job_status(ALICE, &first.run_id)
                .await
                .unwrap()
                .status,
            RunStatus::Completed
        );
        assert_eq!(ledger(&h).len(), 4);

        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let mut killed = OwnedWorker::spawn(&h, Some("before_reuse"));
        let event = killed.reached("before_reuse");
        assert_eq!(event["job_id"], second.job_id);
        assert!(
            event["attempt_id"].is_null(),
            "a reuse path has no provider attempt"
        );
        killed.kill();
        // The probe happened in the dead process; nothing durable records it.
        let row = h.admin.query_one("SELECT (SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.status='succeeded'),(SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.reused_clip_id IS NOT NULL),(SELECT count(*) FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1)", &[&second.run_id]).await.unwrap();
        assert_eq!(
            (
                row.get::<_, i64>(0),
                row.get::<_, i64>(1),
                row.get::<_, i64>(2)
            ),
            (0, 0, 0)
        );
        assert_eq!(ledger(&h).len(), 4);

        h.admin.batch_execute("UPDATE durable_steps SET lease_expires_at=clock_timestamp()-interval '1 second' WHERE status='leased'").await.unwrap();
        let mut restarted = OwnedWorker::spawn(&h, None);
        let reused = restarted.wait_event("reused");
        assert!(reused["clip_id"].as_str().is_some());
        assert!(reused["attempt_id"].is_null());
        restarted.finish();
        for _ in 0..3 {
            OwnedWorker::spawn(&h, None).finish();
        }
        let state = h.store.job_status(ALICE, &second.run_id).await.unwrap();
        assert_eq!(
            (state.status, state.completed_steps, state.reused_steps),
            (RunStatus::Completed, 4, 4)
        );
        assert_eq!(
            ledger(&h).len(),
            4,
            "no provider execution for a fully verified rerun"
        );
        assert_invariants(&h).await;
    }
}

/// Verified clip reuse: the registry, the attempt-less selection and the plan, against the
/// runner's real PostgreSQL. Output files and the provider-side execution ledger come from the
/// in-process worker and the explicit synthetic provider, never from a commercial provider.
mod clip_reuse {
    use std::{collections::BTreeSet, fs};

    use cantos_api::{
        clip_probe::ClipProber,
        clip_probe::VerifiedObject,
        clip_reuse::{RegisteredClip, StoredClip},
        regeneration::{CacheMiss, EpisodeReason, LineAction, PlanGate, StaleReason, Staleness},
        speech_fingerprint::SpeechInput,
    };

    use super::*;

    const EDITED: (&str, &str) = (
        "Ngày mai, mình có diễn tiếp không?",
        "Ngày mai, mình có diễn tiếp không nhỉ?",
    );

    fn frozen_fingerprints(snapshot: &ProductionSnapshotResponse) -> BTreeSet<String> {
        snapshot
            .document
            .resolved
            .iter()
            .map(|line| {
                SpeechInput::from_resolved(line)
                    .unwrap()
                    .fingerprint()
                    .to_string()
            })
            .collect()
    }

    async fn run_a(h: &Harness) -> JobSummary {
        let job = h.enqueue(ExecutionMode::SyntheticFake).await;
        let state = h.drain(&job.run_id).await;
        assert_eq!(state.status, RunStatus::Completed);
        assert_eq!((state.completed_steps, state.reused_steps), (4, 0));
        state
    }

    async fn attempts(h: &Harness, run: &str) -> i64 {
        h.count("SELECT count(*) FROM durable_attempts a JOIN durable_steps s ON s.id=a.step_id JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1", run).await
    }

    async fn step_state(
        h: &Harness,
        run: &str,
        dialogue: &str,
    ) -> (String, Option<String>, Option<String>) {
        let row = h.admin.query_one("SELECT s.status,s.accepted_attempt_id,s.reused_clip_id FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.dialogue_id=$2", &[&run, &dialogue]).await.unwrap();
        (row.get(0), row.get(1), row.get(2))
    }

    async fn clip_path(h: &Harness, dialogue: &str, run: &str) -> String {
        h.admin.query_one("SELECT c.output_path FROM speech_clips c JOIN durable_steps s ON s.accepted_attempt_id=c.attempt_id JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.dialogue_id=$2", &[&run, &dialogue]).await.unwrap().get(0)
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn every_accepted_step_registers_one_clip_the_database_can_verify_on_its_own() {
        let h = Harness::new(4).await;
        let job = run_a(&h).await;
        let rows = h.admin.query("SELECT c.owner_id,c.speech_fingerprint,c.fingerprint_input,c.producer,c.voice_rights_record_id,c.output_contract,c.audio_sha256,c.audio_bytes,c.output_path,c.attempt_id,s.dialogue_id,s.speech_fingerprint FROM speech_clips c JOIN durable_steps s ON s.accepted_attempt_id=c.attempt_id ORDER BY c.registration_seq", &[]).await.unwrap();
        assert_eq!(rows.len(), 4);
        let mut seen = BTreeSet::new();
        for row in &rows {
            let dialogue: String = row.get(10);
            let line = h
                .snapshot
                .document
                .resolved
                .iter()
                .find(|line| line.dialogue_id == dialogue)
                .unwrap();
            let input = SpeechInput::from_resolved(line).unwrap();
            assert_eq!(row.get::<_, String>(0), "alice");
            assert_eq!(row.get::<_, String>(1), input.fingerprint().as_str());
            assert_eq!(row.get::<_, String>(11), input.fingerprint().as_str());
            assert_eq!(row.get::<_, Vec<u8>>(2), input.canonical_bytes());
            assert_eq!(row.get::<_, String>(3), "synthetic_fake");
            assert_eq!(row.get::<_, String>(4), line.voice_rights_record_id);
            assert_eq!(row.get::<_, String>(5), line.output_contract);
            let bytes = fs::read(row.get::<_, String>(8)).unwrap();
            assert_eq!(
                row.get::<_, String>(6),
                format!("{:x}", Sha256::digest(&bytes))
            );
            assert_eq!(row.get::<_, i64>(7), bytes.len() as i64);
            seen.insert(dialogue);
        }
        assert_eq!(seen.len(), 4);
        assert_eq!(h.executions(), 4);
        assert_eq!(attempts(&h, &job.run_id).await, 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_second_run_over_the_same_snapshot_reuses_every_clip_with_no_provider_call_or_spend()
    {
        let h = Harness::new(4).await;
        let first = run_a(&h).await;
        let before = h.count_all("SELECT count(*) FROM durable_dispatches").await;
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let state = h.drain(&second.run_id).await;
        assert_eq!(state.status, RunStatus::Completed);
        assert_eq!(
            (state.completed_steps, state.reused_steps, state.total_steps),
            (4, 4, 4)
        );
        // The provider's own ledger saw nothing new, and the new run owns no attempt at all.
        assert_eq!(h.executions(), 4);
        assert_eq!(attempts(&h, &second.run_id).await, 0);
        assert_eq!(
            h.count_all("SELECT count(*) FROM durable_dispatches").await,
            before
        );
        assert_eq!(
            h.count(
                "SELECT reserved_minor+settled_minor FROM durable_budgets WHERE run_id=$1",
                &second.run_id
            )
            .await,
            0,
            "a cache hit must not reserve or settle money"
        );
        // Each reused step points at the first run's clip for the same dialogue.
        for dialogue in ["dialogue-01", "dialogue-02", "dialogue-03", "dialogue-04"] {
            let (_, _, reused) = step_state(&h, &second.run_id, dialogue).await;
            let original = clip_path(&h, dialogue, &first.run_id).await;
            let path: String = h
                .admin
                .query_one(
                    "SELECT output_path FROM speech_clips WHERE id=$1",
                    &[&reused.unwrap()],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(path, original);
        }
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn one_edited_dialogue_is_the_only_provider_call_and_reverting_it_costs_nothing() {
        let h = Harness::new(4).await;
        let first = run_a(&h).await;
        let edited = h.revise(1, &[EDITED], true).await;
        let root = h.canonical_root();

        let preview = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                Some(&h.snapshot.id),
                &edited.id,
                ExecutionMode::SyntheticFake,
                Some(&root),
            )
            .await
            .unwrap();
        let plan = &preview.plan;
        assert_eq!(plan.gate, PlanGate::Open);
        let produce: Vec<_> = plan
            .lines
            .iter()
            .filter(|line| matches!(line.action, LineAction::Produce { .. }))
            .map(|line| line.dialogue_id.as_str())
            .collect();
        assert_eq!(produce, ["dialogue-02"]);
        assert_eq!((plan.cost.cache_hits, plan.cost.cache_misses), (3, 1));
        assert_eq!(plan.cost.provider_dispatches_expected, 1);
        let stale: Vec<_> = plan
            .scenes
            .iter()
            .filter(|scene| scene.mix == Staleness::Stale)
            .map(|scene| scene.scene_id.as_str())
            .collect();
        assert_eq!(stale, ["scene-01"]);
        assert!(plan.scenes[0]
            .reasons
            .contains(&StaleReason::SpeechSequenceChanged));
        assert_eq!(plan.episode.master, Staleness::Stale);
        assert_eq!(h.executions(), 4, "planning must not call a provider");

        let second = h.enqueue_on(&edited.id, ExecutionMode::SyntheticFake).await;
        let state = h.drain(&second.run_id).await;
        assert_eq!(
            (state.status, state.completed_steps, state.reused_steps),
            (RunStatus::Completed, 4, 3)
        );
        assert_eq!(attempts(&h, &second.run_id).await, 1);
        assert_eq!(h.executions(), 5);
        let (_, accepted, reused) = step_state(&h, &second.run_id, "dialogue-02").await;
        assert!(accepted.is_some() && reused.is_none());
        let kept = frozen_fingerprints(&edited);
        assert_eq!(kept.len(), 4);

        // Edit then revert: a fresh revision with the original wording finds every old clip.
        let reverted = h.revise(2, &[], true).await;
        let third = h
            .enqueue_on(&reverted.id, ExecutionMode::SyntheticFake)
            .await;
        let state = h.drain(&third.run_id).await;
        assert_eq!(
            (state.status, state.completed_steps, state.reused_steps),
            (RunStatus::Completed, 4, 4)
        );
        assert_eq!(
            h.executions(),
            5,
            "reverting an edit must not synthesize again"
        );
        assert_eq!(attempts(&h, &third.run_id).await, 0);
        let (_, _, reused) = step_state(&h, &third.run_id, "dialogue-02").await;
        let original = clip_path(&h, "dialogue-02", &first.run_id).await;
        let path: String = h
            .admin
            .query_one(
                "SELECT output_path FROM speech_clips WHERE id=$1",
                &[&reused.unwrap()],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(path, original);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_pronunciation_change_regenerates_every_line_of_the_character_that_carries_it() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let mut changed = settings(4);
        for binding in changed
            .bindings
            .iter_mut()
            .filter(|b| b.character_id == "an")
        {
            binding.pronunciation = vec![cantos_contracts::ProductionPronunciation {
                surface: "An".into(),
                replacement: "Ân".into(),
            }];
        }
        h.store
            .save_production_settings(
                ALICE,
                &h.script,
                SaveProductionSettingsRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 1,
                    expected_settings_version: 1,
                    settings: changed,
                },
            )
            .await
            .unwrap();
        let preview = h.store.production_preview(ALICE, &h.script).await.unwrap();
        let snapshot = h
            .store
            .freeze_production(
                ALICE,
                &h.script,
                FreezeProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 1,
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
                &snapshot.id,
                ApproveProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    input_digest: snapshot.document.input_digest.clone(),
                },
            )
            .await
            .unwrap();
        let job = h
            .enqueue_on(&snapshot.id, ExecutionMode::SyntheticFake)
            .await;
        let state = h.drain(&job.run_id).await;
        // `an` speaks dialogue-02 and dialogue-04; narrator and minh keep their clips.
        assert_eq!((state.completed_steps, state.reused_steps), (4, 2));
        assert_eq!(attempts(&h, &job.run_id).await, 2);
        assert_eq!(h.executions(), 6);
        for dialogue in ["dialogue-02", "dialogue-04"] {
            assert!(step_state(&h, &job.run_id, dialogue).await.1.is_some());
        }
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_missing_corrupt_or_swapped_object_is_never_reused_and_the_line_regenerates() {
        for damage in [
            "missing",
            "other_valid_audio",
            "garbage",
            "symlink",
            "empty_file",
        ] {
            let h = Harness::new(4).await;
            let first = run_a(&h).await;
            let victim = clip_path(&h, "dialogue-02", &first.run_id).await;
            match damage {
                "missing" => fs::remove_file(&victim).unwrap(),
                "other_valid_audio" => {
                    // A well-formed 22,050 Hz mono WAV, but not the registered bytes.
                    let mut other = wav();
                    let last = other.len() - 1;
                    other[last] ^= 0x55; // payload only: header and profile stay valid
                    fs::write(&victim, other).unwrap();
                }
                "garbage" => fs::write(&victim, b"not a wav file").unwrap(),
                "empty_file" => fs::write(&victim, b"").unwrap(),
                "symlink" => {
                    let target = h.root.join("elsewhere.wav");
                    fs::write(&target, wav()).unwrap();
                    fs::remove_file(&victim).unwrap();
                    #[cfg(unix)]
                    std::os::unix::fs::symlink(&target, &victim).unwrap();
                }
                _ => unreachable!(),
            }
            let second = h.enqueue(ExecutionMode::SyntheticFake).await;
            let state = h.drain(&second.run_id).await;
            assert_eq!(state.status, RunStatus::Completed, "{damage}");
            assert_eq!(state.reused_steps, 3, "{damage}");
            assert_eq!(attempts(&h, &second.run_id).await, 1, "{damage}");
            assert_eq!(h.executions(), 5, "{damage}");
            let (_, accepted, reused) = step_state(&h, &second.run_id, "dialogue-02").await;
            assert!(accepted.is_some() && reused.is_none(), "{damage}");

            // The damaged clip stays registered but unusable; the newer valid one now wins.
            let third = h.enqueue(ExecutionMode::SyntheticFake).await;
            let state = h.drain(&third.run_id).await;
            assert_eq!(
                (state.status, state.reused_steps),
                (RunStatus::Completed, 4),
                "{damage}"
            );
            assert_eq!(h.executions(), 5, "{damage}");
            let (_, _, reused) = step_state(&h, &third.run_id, "dialogue-02").await;
            let newer: String = h.admin.query_one("SELECT c.id FROM speech_clips c JOIN durable_steps s ON s.accepted_attempt_id=c.attempt_id JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.dialogue_id='dialogue-02'", &[&second.run_id]).await.unwrap().get(0);
            assert_eq!(reused.unwrap(), newer, "{damage}");
            assert_invariants(&h).await;
        }
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_worker_whose_output_root_cannot_see_the_clips_regenerates_instead_of_trusting_the_row(
    ) {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let other_root = h.root.join("other-private-root");
        fs::create_dir(&other_root).unwrap();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&other_root, fs::Permissions::from_mode(0o700)).unwrap();
        }
        for round in 0..4 {
            cantos_api::worker::run_worker(
                h.store.clone(),
                cantos_api::worker::WorkerConfig {
                    provider: cantos_api::worker::WorkerProvider::SyntheticFake,
                    output_root: fs::canonicalize(&other_root).unwrap(),
                    worker_id: format!("elsewhere-{round}"),
                    lease_duration: ttl(),
                    once: true,
                    failpoint: None,
                },
            )
            .await
            .unwrap();
        }
        let state = h.store.job_status(ALICE, &second.run_id).await.unwrap();
        assert_eq!(
            (state.status, state.reused_steps),
            (RunStatus::Completed, 0)
        );
        assert_eq!(attempts(&h, &second.run_id).await, 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn lapsed_authority_blocks_reuse_and_pauses_the_job_without_a_hit() {
        for actor_revoked in [false, true] {
            let h = Harness::new(4).await;
            run_a(&h).await;
            let second = h.enqueue(ExecutionMode::SyntheticFake).await;
            if actor_revoked {
                h.admin
                    .batch_execute("UPDATE actors SET active=false WHERE id='alice'")
                    .await
                    .unwrap();
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
            }
            h.run_worker_once("after-lapse").await;
            let status: String = h
                .admin
                .query_one(
                    "SELECT status FROM durable_jobs WHERE run_id=$1",
                    &[&second.run_id],
                )
                .await
                .unwrap()
                .get(0);
            let reason: Option<String> = h
                .admin
                .query_one(
                    "SELECT reason FROM durable_jobs WHERE run_id=$1",
                    &[&second.run_id],
                )
                .await
                .unwrap()
                .get(0);
            assert_eq!(
                (status.as_str(), reason.as_deref()),
                ("paused", Some("authorization_invalid"))
            );
            assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.reused_clip_id IS NOT NULL", &second.run_id).await, 0);
            assert_eq!(attempts(&h, &second.run_id).await, 0);
            assert_eq!(h.executions(), 4);
            assert_invariants(&h).await;
        }
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn concurrent_workers_satisfy_each_step_once_and_never_reach_the_provider() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let gate = Arc::new(Barrier::new(4));
        let mut tasks = Vec::new();
        for worker in 0..4 {
            let store = h.reopened();
            let root = h.canonical_root();
            let gate = Arc::clone(&gate);
            let run = second.run_id.clone();
            tasks.push(tokio::spawn(async move {
                gate.wait().await;
                for round in 0..12 {
                    if !matches!(
                        store.job_status(ALICE, &run).await.unwrap().status,
                        RunStatus::Queued | RunStatus::Running
                    ) {
                        break;
                    }
                    cantos_api::worker::run_worker(
                        store.clone(),
                        cantos_api::worker::WorkerConfig {
                            provider: cantos_api::worker::WorkerProvider::SyntheticFake,
                            output_root: root.clone(),
                            worker_id: format!("racer-{worker}-{round}"),
                            lease_duration: ttl(),
                            once: true,
                            failpoint: None,
                        },
                    )
                    .await
                    .unwrap();
                }
            }));
        }
        for task in tasks {
            task.await.unwrap();
        }
        let state = h.store.job_status(ALICE, &second.run_id).await.unwrap();
        assert_eq!(
            (state.status, state.completed_steps, state.reused_steps),
            (RunStatus::Completed, 4, 4)
        );
        assert_eq!(h.executions(), 4);
        assert_eq!(attempts(&h, &second.run_id).await, 0);
        let distinct: i64 = h.count("SELECT count(DISTINCT s.id) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.reused_clip_id IS NOT NULL", &second.run_id).await;
        assert_eq!(distinct, 4);
        assert_invariants(&h).await;
    }

    /// A registry row with its locator, read back the way a lookup would return it.
    async fn stored_clip(h: &Harness, id: &str) -> StoredClip {
        let row = h
            .admin
            .query_one(
                "SELECT id,registration_seq,owner_id,speech_fingerprint,producer,voice_rights_record_id,output_contract,audio_sha256,attempt_id,output_path FROM speech_clips WHERE id=$1",
                &[&id],
            )
            .await
            .unwrap();
        StoredClip {
            clip: RegisteredClip {
                id: row.get(0),
                sequence: row.get(1),
                owner_id: row.get(2),
                fingerprint: row.get::<_, String>(3).parse().unwrap(),
                producer: row.get::<_, String>(4).parse().unwrap(),
                voice_rights_record_id: row.get(5),
                output_contract: row.get(6),
                audio_sha256: row.get::<_, String>(7).parse().unwrap(),
            },
            attempt_id: row.get(8),
            output_path: row.get(9),
        }
    }

    /// The witness only a real, fresh probe of the object can issue.
    async fn verified(h: &Harness, id: &str) -> VerifiedObject {
        let stored = stored_clip(h, id).await;
        ClipProber::open(&h.canonical_root())
            .unwrap()
            .inspect(&stored)
            .into_parts()
            .1
            .expect("a fresh, undamaged clip verifies")
    }

    /// Claim the first step of the second run and gather what a worker would hold after its
    /// probe: the lease and the witness for the winning clip.
    async fn prepared_selection(
        h: &Harness,
        worker: &str,
    ) -> (JobLease, VerifiedObject, JobSummary) {
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let lease = h.lease(worker).await;
        let prepared = h
            .store
            .reuse_candidates(&lease)
            .await
            .unwrap()
            .expect("a verified clip exists");
        assert_eq!(prepared.candidates.len(), 1);
        let witness = verified(h, &prepared.candidates[0].clip.id).await;
        (lease, witness, second)
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn selection_is_fenced_idempotent_and_refuses_a_witness_for_another_line() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let (stale, witness, second) = prepared_selection(&h, "stale-worker").await;

        // A witness for a different line's clip is real evidence about the wrong step.
        let other_clip: String = h
            .admin
            .query_one(
                "SELECT id FROM speech_clips WHERE id<>$1 AND speech_fingerprint<>(SELECT speech_fingerprint FROM speech_clips WHERE id=$1) ORDER BY registration_seq LIMIT 1",
                &[&witness.clip_id()],
            )
            .await
            .unwrap()
            .get(0);
        let elsewhere = verified(&h, &other_clip).await;
        assert!(matches!(
            h.store.select_reused_clip(&stale, &elsewhere).await,
            Err(DurableError::Conflict)
        ));
        assert_eq!(
            h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.status='succeeded'", &second.run_id).await,
            0
        );

        // A newer fence takes over; the stale worker cannot select afterwards.
        h.expire(&stale).await;
        let current = h.lease("current-worker").await;
        assert_eq!(current.step_id, stale.step_id);
        assert!(current.token > stale.token);
        assert!(matches!(
            h.store.select_reused_clip(&stale, &witness).await,
            Err(DurableError::LeaseLost)
        ));
        let selected = h
            .store
            .select_reused_clip(&current, &witness)
            .await
            .unwrap();
        assert_eq!(
            (selected.clip_id.as_str(), selected.job_completed),
            (witness.clip_id(), false)
        );
        // Replays and duplicates can never select a second time.
        for lease in [&current, &stale] {
            assert!(matches!(
                h.reopened().select_reused_clip(lease, &witness).await,
                Err(DurableError::LeaseLost)
            ));
        }
        assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.reused_clip_id IS NOT NULL", &second.run_id).await, 1);
        assert_eq!(attempts(&h, &second.run_id).await, 0);
        assert_eq!(h.executions(), 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn two_connections_racing_to_select_one_step_produce_exactly_one_selection() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let (lease, witness, second) = prepared_selection(&h, "racer").await;
        let gate = Arc::new(Barrier::new(2));
        let mut tasks = Vec::new();
        for _ in 0..2 {
            let store = h.reopened();
            let (lease, witness, gate) = (lease.clone(), witness.clone(), Arc::clone(&gate));
            tasks.push(tokio::spawn(async move {
                gate.wait().await;
                store.select_reused_clip(&lease, &witness).await
            }));
        }
        let mut outcomes = Vec::new();
        for task in tasks {
            outcomes.push(task.await.unwrap());
        }
        assert_eq!(
            outcomes.iter().filter(|outcome| outcome.is_ok()).count(),
            1,
            "{outcomes:?}"
        );
        assert!(outcomes
            .iter()
            .filter_map(|outcome| outcome.as_ref().err())
            .all(|error| matches!(error, DurableError::LeaseLost)));
        assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.reused_clip_id IS NOT NULL", &second.run_id).await, 1);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn authority_lapsing_between_lookup_and_selection_falls_back_to_the_pausing_path() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let (lease, witness, second) = prepared_selection(&h, "before-lapse").await;
        h.admin
            .batch_execute("UPDATE actors SET active=false WHERE id='alice'")
            .await
            .unwrap();
        // The selection commits nothing and reports a disagreement, not a store failure.
        assert!(matches!(
            h.store.select_reused_clip(&lease, &witness).await,
            Err(DurableError::Conflict)
        ));
        assert_eq!(
            h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.status='succeeded'", &second.run_id).await,
            0
        );
        // The ordinary path then re-reads authority and pauses the job with its usual reason.
        assert!(matches!(
            h.store.prepare_step(&lease).await.unwrap(),
            StepWork::Blocked
        ));
        let reason: Option<String> = h
            .admin
            .query_one(
                "SELECT reason FROM durable_jobs WHERE run_id=$1",
                &[&second.run_id],
            )
            .await
            .unwrap()
            .get(0);
        assert_eq!(reason.as_deref(), Some("authorization_invalid"));
        assert_eq!(h.executions(), 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn cancel_or_pause_between_lookup_and_selection_prevents_the_selection() {
        for event in [RunEvent::Cancel, RunEvent::Pause] {
            let h = Harness::new(4).await;
            run_a(&h).await;
            let (lease, witness, second) = prepared_selection(&h, "before-control").await;
            h.store
                .control_job(ALICE, &second.run_id, &Uuid::new_v4().to_string(), event)
                .await
                .unwrap();
            assert!(matches!(
                h.store.select_reused_clip(&lease, &witness).await,
                Err(DurableError::LeaseLost)
            ));
            assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.reused_clip_id IS NOT NULL", &second.run_id).await, 0);
            assert_invariants(&h).await;
        }
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_recorded_provider_attempt_is_reconciled_never_bypassed_by_a_cache_hit() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        // A different run whose first step already sent an intent and then lost its worker.
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let lease = h.lease("died-after-intent").await;
        let _work = h.dispatch(&lease).await;
        h.expire(&lease).await;
        let recovered = h.lease("recovery").await;
        assert_eq!(recovered.step_id, lease.step_id);
        assert!(h
            .store
            .reuse_candidates(&recovered)
            .await
            .unwrap()
            .is_none());
        // The ordinary path takes over and pauses for reconciliation.
        assert!(matches!(
            h.store.prepare_step(&recovered).await.unwrap(),
            StepWork::Blocked
        ));
        let state = h.store.job_status(ALICE, &second.run_id).await.unwrap();
        assert_eq!(
            (state.status, state.reason.as_deref()),
            (RunStatus::Paused, Some("uncertain_attempt"))
        );
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_synthetic_fixture_clip_never_satisfies_a_local_engine_run() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let local = h.enqueue(ExecutionMode::LocalEspeak).await;
        let lease = h
            .store
            .claim_step("local-worker", ExecutionMode::LocalEspeak, ttl())
            .await
            .unwrap()
            .unwrap();
        // The lookup never offers another producer's clips to this run...
        let prepared = h.store.reuse_candidates(&lease).await.unwrap().unwrap();
        assert!(prepared.candidates.is_empty());
        // ...and a worker that names one anyway is refused by the full decision.
        let clip_id: String = h
            .admin
            .query_one(
                "SELECT id FROM speech_clips ORDER BY registration_seq LIMIT 1",
                &[],
            )
            .await
            .unwrap()
            .get(0);
        let witness = verified(&h, &clip_id).await;
        assert!(matches!(
            h.store.select_reused_clip(&lease, &witness).await,
            Err(DurableError::Conflict)
        ));
        // And the database itself refuses the same forgery without the application's help.
        let forged = h.admin.batch_execute(&format!("UPDATE durable_steps SET status='succeeded',reused_clip_id='{}',lease_owner=NULL,lease_expires_at=NULL WHERE id='{}'", clip_id, lease.step_id)).await;
        assert!(
            forged.is_err(),
            "producer scope must be enforced by the database"
        );
        assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.status='succeeded'", &local.run_id).await, 0);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn the_database_rejects_forged_registrations_and_mismatched_reuse() {
        let h = Harness::new(4).await;
        let first = run_a(&h).await;
        let row = h.admin.query_one("SELECT id,owner_id,speech_fingerprint,fingerprint_input,producer,voice_rights_record_id,output_contract,audio_sha256,audio_bytes,output_path,attempt_id FROM speech_clips ORDER BY registration_seq LIMIT 1", &[]).await.unwrap();
        let (id, owner, fingerprint, input): (String, String, String, Vec<u8>) =
            (row.get(0), row.get(1), row.get(2), row.get(3));
        let (producer, voice, contract, sha, bytes, path, attempt): (
            String,
            String,
            String,
            String,
            i64,
            String,
            String,
        ) = (
            row.get(4),
            row.get(5),
            row.get(6),
            row.get(7),
            row.get(8),
            row.get(9),
            row.get(10),
        );
        let insert = "INSERT INTO speech_clips(id,owner_id,speech_fingerprint,fingerprint_input,producer,voice_rights_record_id,output_contract,audio_sha256,audio_bytes,output_path,attempt_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)";
        let forged_checksum = "0".repeat(64);
        let forged = h
            .admin
            .execute(
                insert,
                &[
                    &Uuid::new_v4().to_string(),
                    &owner,
                    &fingerprint,
                    &input,
                    &producer,
                    &voice,
                    &contract,
                    &forged_checksum,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(forged.code().map(|code| code.code()), Some("23503"));
        let wrong_document = h
            .admin
            .execute(
                insert,
                &[
                    &Uuid::new_v4().to_string(),
                    &owner,
                    &fingerprint,
                    &b"{\"text\":\"another request\"}".to_vec(),
                    &producer,
                    &voice,
                    &contract,
                    &sha,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(wrong_document.code().map(|code| code.code()), Some("23514"));
        let mut stolen = path.clone();
        stolen.push('x');
        let other_path = h
            .admin
            .execute(
                insert,
                &[
                    &Uuid::new_v4().to_string(),
                    &owner,
                    &fingerprint,
                    &input,
                    &producer,
                    &voice,
                    &contract,
                    &sha,
                    &bytes,
                    &stolen,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(other_path.code().map(|code| code.code()), Some("23503"));
        let mutated = h
            .admin
            .execute(
                "UPDATE speech_clips SET audio_sha256=$2 WHERE id=$1",
                &[&id, &forged_checksum],
            )
            .await
            .unwrap_err();
        assert_eq!(mutated.code().map(|code| code.code()), Some("55000"));
        let deleted = h
            .admin
            .execute("DELETE FROM speech_clips WHERE id=$1", &[&id])
            .await
            .unwrap_err();
        assert_eq!(deleted.code().map(|code| code.code()), Some("55000"));

        // A clip can satisfy only a step with its own fingerprint, and only before any attempt.
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;
        let lease = h.lease("db-check").await;
        let step_fingerprint: String = h
            .admin
            .query_one(
                "SELECT speech_fingerprint FROM durable_steps WHERE id=$1",
                &[&lease.step_id],
            )
            .await
            .unwrap()
            .get(0);
        let other_clip: String = h.admin.query_one("SELECT id FROM speech_clips WHERE speech_fingerprint<>$1 ORDER BY registration_seq LIMIT 1", &[&step_fingerprint]).await.unwrap().get(0);
        let mismatch = h.admin.execute("UPDATE durable_steps SET status='succeeded',reused_clip_id=$2,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1", &[&lease.step_id, &other_clip]).await.unwrap_err();
        assert_eq!(mismatch.code().map(|code| code.code()), Some("23503"));
        let matching: String = h
            .admin
            .query_one(
                "SELECT id FROM speech_clips WHERE speech_fingerprint=$1",
                &[&step_fingerprint],
            )
            .await
            .unwrap()
            .get(0);
        let _work = h.dispatch(&lease).await;
        let after_attempt = h.admin.execute("UPDATE durable_steps SET status='succeeded',reused_clip_id=$2,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1", &[&lease.step_id, &matching]).await.unwrap_err();
        assert_eq!(after_attempt.code().map(|code| code.code()), Some("23503"));
        assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.status='succeeded'", &second.run_id).await, 0);

        // Once a step is reused it is write-once and can never gain a provider attempt.
        let reused_lease = h
            .store
            .claim_step("reuser", ExecutionMode::SyntheticFake, ttl())
            .await
            .unwrap()
            .unwrap();
        let lease = reused_lease;
        let prepared = h
            .store
            .reuse_candidates(&lease)
            .await
            .unwrap()
            .expect("another step of the second run can still reuse");
        let stored = &prepared.candidates[0];
        let witness = verified(&h, &stored.clip.id).await;
        h.store.select_reused_clip(&lease, &witness).await.unwrap();
        let different_clip: String = h
            .admin
            .query_one(
                "SELECT id FROM speech_clips WHERE id<>$1 ORDER BY registration_seq LIMIT 1",
                &[&stored.clip.id],
            )
            .await
            .unwrap()
            .get(0);
        let rewrite = h
            .admin
            .execute(
                "UPDATE durable_steps SET reused_clip_id=$2 WHERE id=$1",
                &[&lease.step_id, &different_clip],
            )
            .await
            .unwrap_err();
        assert_eq!(rewrite.code().map(|code| code.code()), Some("55000"));
        let later_attempt = h.admin.execute("INSERT INTO durable_attempts(id,step_id,lease_token,operation_id,reservation_id,intent_bytes,attempt_number) VALUES($1,$2,1,$3,$4,'{}',1)", &[&Uuid::new_v4().to_string(), &lease.step_id, &Uuid::new_v4().to_string(), &Uuid::new_v4().to_string()]).await.unwrap_err();
        assert_eq!(later_attempt.code().map(|code| code.code()), Some("23514"));
        let _ = first;
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn planning_without_an_output_root_counts_no_hit_and_an_unapproved_snapshot_blocks_the_plan(
    ) {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let report = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                None,
                &h.snapshot.id,
                ExecutionMode::SyntheticFake,
                None,
            )
            .await
            .unwrap();
        assert_eq!(
            report.object_probes,
            cantos_api::postgres::durable::ObjectProbes::NotPerformed
        );
        assert_eq!(
            (report.plan.cost.cache_hits, report.plan.cost.cache_misses),
            (0, 4)
        );
        assert!(report.plan.lines.iter().all(|line| matches!(
            &line.action,
            LineAction::Produce { cache: CacheMiss::Refused { refusals } }
                if refusals.len() == 1 && refusals[0].reason == cantos_api::clip_reuse::Refusal::Unverified
        )));
        let root = h.canonical_root();
        let probed = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                None,
                &h.snapshot.id,
                ExecutionMode::SyntheticFake,
                Some(&root),
            )
            .await
            .unwrap();
        assert_eq!(
            (probed.plan.cost.cache_hits, probed.plan.cost.cache_misses),
            (4, 0)
        );
        // With no previous snapshot there is nothing to compare, so every scene is "new".
        assert_eq!(
            probed.plan.episode.reasons[0],
            EpisodeReason::NoPreviousProduction
        );
        assert!(probed
            .plan
            .scenes
            .iter()
            .all(|scene| scene.reasons.contains(&StaleReason::SceneAdded)));
        // Another execution mode cannot borrow the synthetic clips.
        let local = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                None,
                &h.snapshot.id,
                ExecutionMode::LocalEspeak,
                Some(&root),
            )
            .await
            .unwrap();
        assert_eq!(
            (local.plan.cost.cache_hits, local.plan.cost.cache_misses),
            (0, 4)
        );

        // A frozen but unapproved snapshot cannot be planned for: the gate stays closed.
        let unapproved = h.revise(1, &[EDITED], false).await;
        let closed = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                Some(&h.snapshot.id),
                &unapproved.id,
                ExecutionMode::SyntheticFake,
                Some(&root),
            )
            .await
            .unwrap();
        match &closed.plan.gate {
            PlanGate::Blocked { findings } => {
                assert!(findings.contains(&"approval_not_current".to_owned()))
            }
            PlanGate::Open => panic!("an unapproved snapshot must not open the plan"),
        }
        assert!(closed
            .plan
            .lines
            .iter()
            .all(|line| line.action == LineAction::Blocked));
        assert_eq!(closed.plan.cost.provider_dispatches_expected, 0);
        assert_eq!(h.executions(), 4);
    }
    /// Process a local-engine run through the store the way the worker would, with the
    /// engine's output files in the worker's private layout. No eSpeak binary is involved.
    async fn complete_local_run(h: &Harness) -> JobSummary {
        let job = h.enqueue(ExecutionMode::LocalEspeak).await;
        for round in 0..4_u8 {
            let lease = h
                .store
                .claim_step(&format!("local-{round}"), ExecutionMode::LocalEspeak, ttl())
                .await
                .unwrap()
                .unwrap();
            let work = match h.store.prepare_step(&lease).await.unwrap() {
                StepWork::Dispatch(work) => *work,
                _ => panic!("expected a new dispatch intent"),
            };
            h.store.authorize_dispatch(&work).await.unwrap();
            let mut bytes = wav();
            let last = bytes.len() - 1;
            bytes[last] = round + 1; // distinct audio per line
            let directory = h.canonical_root().join(&work.intent.attempt_id);
            fs::create_dir(&directory).unwrap();
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&directory, fs::Permissions::from_mode(0o700)).unwrap();
            }
            let output = directory.join("output.wav");
            fs::write(&output, &bytes).unwrap();
            let mut receipt = work.intent.clone();
            receipt.outcome = ReceiptOutcome::Output {
                audio_sha256: format!("{:x}", Sha256::digest(&bytes)),
            };
            h.store
                .observe_attempt(&work, &receipt, output.to_str(), Some(&bytes))
                .await
                .unwrap();
            h.store
                .accept_step(&lease, &receipt.attempt_id, &bytes)
                .await
                .unwrap();
        }
        let state = h.store.job_status(ALICE, &job.run_id).await.unwrap();
        assert_eq!(
            (state.status, state.completed_steps),
            (RunStatus::Completed, 4)
        );
        state
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn an_older_clip_of_another_producer_never_hides_or_is_offered_to_a_run_of_the_other_kind(
    ) {
        let h = Harness::new(4).await;
        run_a(&h).await; // oldest: synthetic fixtures
        complete_local_run(&h).await; // newer: the same fingerprints from the local engine
        let producers: i64 = h
            .count_all("SELECT count(DISTINCT producer) FROM speech_clips")
            .await;
        assert_eq!(producers, 2);
        for (mode, producer) in [
            (ExecutionMode::LocalEspeak, "local_espeak"),
            (ExecutionMode::SyntheticFake, "synthetic_fake"),
        ] {
            let run = h.enqueue(mode).await;
            let lease = h
                .store
                .claim_step("scoped", mode, ttl())
                .await
                .unwrap()
                .unwrap();
            let prepared = h.store.reuse_candidates(&lease).await.unwrap().unwrap();
            // Scope is applied before the bound: only same-producer clips are ever offered.
            assert_eq!(prepared.candidates.len(), 1, "{producer}");
            assert_eq!(prepared.candidates[0].clip.producer, mode);
            let witness = verified(&h, &prepared.candidates[0].clip.id).await;
            h.store.select_reused_clip(&lease, &witness).await.unwrap();
            let chosen: String = h.admin.query_one("SELECT c.producer FROM speech_clips c JOIN durable_steps s ON s.reused_clip_id=c.id JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1", &[&run.run_id]).await.unwrap().get(0);
            assert_eq!(chosen, producer);
        }
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn moving_and_reordering_dialogue_keeps_every_clip_and_only_marks_the_touched_scene_mixes_stale(
    ) {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let root = h.canonical_root();

        // Reorder inside scene-01 only: scene-02 is untouched.
        let swapped = h
            .revise_with(
                1,
                |json| {
                    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
                    let dialogues = value["episode"]["acts"][0]["scenes"][0]["dialogues"]
                        .as_array_mut()
                        .unwrap();
                    dialogues.swap(1, 2);
                    serde_json::to_string(&value).unwrap()
                },
                true,
            )
            .await;
        let plan = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                Some(&h.snapshot.id),
                &swapped.id,
                ExecutionMode::SyntheticFake,
                Some(&root),
            )
            .await
            .unwrap()
            .plan;
        assert_eq!((plan.cost.cache_hits, plan.cost.cache_misses), (4, 0));
        assert_eq!(plan.cost.provider_dispatches_expected, 0);
        let stale: Vec<_> = plan
            .scenes
            .iter()
            .filter(|s| s.mix == Staleness::Stale)
            .map(|s| s.scene_id.as_str())
            .collect();
        assert_eq!(stale, ["scene-01"]);
        assert_eq!(plan.scenes[0].reasons, [StaleReason::SpeechSequenceChanged]);

        // Move a line to another scene: the clip stays valid; both scenes' mixes are stale.
        let moved = h
            .revise_with(
                2,
                |json| {
                    let mut value: serde_json::Value = serde_json::from_str(&json).unwrap();
                    let carried = value["episode"]["acts"][0]["scenes"][0]["dialogues"]
                        .as_array_mut()
                        .unwrap()
                        .remove(2);
                    value["episode"]["acts"][0]["scenes"][1]["dialogues"]
                        .as_array_mut()
                        .unwrap()
                        .push(carried);
                    serde_json::to_string(&value).unwrap()
                },
                true,
            )
            .await;
        let plan = h
            .store
            .regeneration_plan(
                ALICE,
                &h.script,
                Some(&swapped.id),
                &moved.id,
                ExecutionMode::SyntheticFake,
                Some(&root),
            )
            .await
            .unwrap()
            .plan;
        assert_eq!((plan.cost.cache_hits, plan.cost.cache_misses), (4, 0));
        // Each revision edits the original script, so the carried line is scene-01's third.
        let relocated = plan
            .lines
            .iter()
            .find(|line| line.dialogue_id == "dialogue-03")
            .unwrap();
        assert_eq!(relocated.scene_id, "scene-02");
        assert!(matches!(
            relocated.change,
            cantos_api::regeneration::LineChange::Unchanged {
                placement_changed: true
            }
        ));
        let stale: Vec<_> = plan
            .scenes
            .iter()
            .filter(|s| s.mix == Staleness::Stale)
            .map(|s| s.scene_id.as_str())
            .collect();
        assert_eq!(stale, ["scene-01", "scene-02"]);

        // And the durable run over the moved layout never reaches the provider.
        let job = h.enqueue_on(&moved.id, ExecutionMode::SyntheticFake).await;
        let state = h.drain(&job.run_id).await;
        assert_eq!(
            (state.status, state.completed_steps, state.reused_steps),
            (RunStatus::Completed, 4, 4)
        );
        assert_eq!(h.executions(), 4);
        assert_eq!(attempts(&h, &job.run_id).await, 0);
        assert_invariants(&h).await;
    }
    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_large_admitted_pronunciation_list_is_accepted_and_registered_never_wedged() {
        let h = Harness::new(4).await;
        let mut changed = settings(4);
        for binding in changed
            .bindings
            .iter_mut()
            .filter(|b| b.character_id == "an")
        {
            // 100 rules at the admission maximum of 512 bytes per field (~106 KB of canonical
            // document): admitted, and well beyond the 64 KiB the registry first assumed.
            binding.pronunciation = (0..100)
                .map(|index| cantos_contracts::ProductionPronunciation {
                    surface: format!("Z{index:03}{}", "q".repeat(500)),
                    replacement: "ế".repeat(170),
                })
                .collect();
        }
        h.store
            .save_production_settings(
                ALICE,
                &h.script,
                SaveProductionSettingsRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_revision: 1,
                    expected_settings_version: 1,
                    settings: changed,
                },
            )
            .await
            .unwrap();
        let preview = h.store.production_preview(ALICE, &h.script).await.unwrap();
        let snapshot = h
            .store
            .freeze_production(
                ALICE,
                &h.script,
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
            "{:?}",
            snapshot.eligibility.findings
        );
        h.store
            .approve_production(
                ALICE,
                &h.script,
                &snapshot.id,
                ApproveProductionRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    input_digest: snapshot.document.input_digest.clone(),
                },
            )
            .await
            .unwrap();
        let job = h
            .enqueue_on(&snapshot.id, ExecutionMode::SyntheticFake)
            .await;
        let state = h.drain(&job.run_id).await;
        assert_eq!(
            (state.status, state.completed_steps),
            (RunStatus::Completed, 4)
        );
        let largest: i64 = h
            .count_all("SELECT max(octet_length(fingerprint_input))::bigint FROM speech_clips")
            .await;
        assert!(
            largest > 65_536,
            "fixture must exceed the original 64 KiB bound: {largest}"
        );
        assert_eq!(h.count_all("SELECT count(*) FROM speech_clips").await, 4);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn the_database_binds_a_registration_to_its_own_step_fingerprint_rights_scope_and_contract(
    ) {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let row = h.admin.query_one("SELECT owner_id,speech_fingerprint,fingerprint_input,producer,voice_rights_record_id,output_contract,audio_sha256,audio_bytes,output_path,attempt_id FROM speech_clips ORDER BY registration_seq LIMIT 1", &[]).await.unwrap();
        let (owner, fingerprint, input): (String, String, Vec<u8>) =
            (row.get(0), row.get(1), row.get(2));
        let (producer, voice, contract, sha, bytes, path, attempt): (
            String,
            String,
            String,
            String,
            i64,
            String,
            String,
        ) = (
            row.get(3),
            row.get(4),
            row.get(5),
            row.get(6),
            row.get(7),
            row.get(8),
            row.get(9),
        );
        let insert = "INSERT INTO speech_clips(id,owner_id,speech_fingerprint,fingerprint_input,producer,voice_rights_record_id,output_contract,audio_sha256,audio_bytes,output_path,attempt_id) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11)";
        let id = || Uuid::new_v4().to_string();

        // A hash-consistent document for some other request: it passes the CHECK, but it is not
        // the fingerprint this attempt's step carries.
        let other_document = b"{\"text\":\"another request\"}".to_vec();
        let mut hash = Sha256::new();
        hash.update(b"cantos/speech-fingerprint/v1\n");
        hash.update(&other_document);
        let other_fingerprint = format!("speech-v1:sha256:{:x}", hash.finalize());
        let bogus_identity = h
            .admin
            .execute(
                insert,
                &[
                    &id(),
                    &owner,
                    &other_fingerprint,
                    &other_document,
                    &producer,
                    &voice,
                    &contract,
                    &sha,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(bogus_identity.code().map(|code| code.code()), Some("23503"));

        // A currently licensed record or another contract that this line was not frozen under.
        let wrong_voice = h
            .admin
            .execute(
                insert,
                &[
                    &id(),
                    &owner,
                    &fingerprint,
                    &input,
                    &producer,
                    &"voice_other_licensed",
                    &contract,
                    &sha,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(wrong_voice.code().map(|code| code.code()), Some("23503"));
        let wrong_contract = h
            .admin
            .execute(
                insert,
                &[
                    &id(),
                    &owner,
                    &fingerprint,
                    &input,
                    &producer,
                    &voice,
                    &"wav-pcm-s16le-mono-24000-v1",
                    &sha,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(wrong_contract.code().map(|code| code.code()), Some("23503"));
        // Another creator, or another kind of producer, than the run that accepted it.
        let wrong_owner = h
            .admin
            .execute(
                insert,
                &[
                    &id(),
                    &"bob",
                    &fingerprint,
                    &input,
                    &producer,
                    &voice,
                    &contract,
                    &sha,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(wrong_owner.code().map(|code| code.code()), Some("23503"));
        let wrong_producer = h
            .admin
            .execute(
                insert,
                &[
                    &id(),
                    &owner,
                    &fingerprint,
                    &input,
                    &"local_espeak",
                    &voice,
                    &contract,
                    &sha,
                    &bytes,
                    &path,
                    &attempt,
                ],
            )
            .await
            .unwrap_err();
        assert_eq!(wrong_producer.code().map(|code| code.code()), Some("23503"));
        assert_eq!(h.count_all("SELECT count(*) FROM speech_clips").await, 4);
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn a_reused_step_gains_no_reservation_and_concurrent_writers_cannot_add_an_attempt() {
        let h = Harness::new(4).await;
        run_a(&h).await;
        let second = h.enqueue(ExecutionMode::SyntheticFake).await;

        // Step one is reused through the store; a reservation for it is then refused.
        let first = h.lease("reuser").await;
        let prepared = h.store.reuse_candidates(&first).await.unwrap().unwrap();
        let witness = verified(&h, &prepared.candidates[0].clip.id).await;
        h.store.select_reused_clip(&first, &witness).await.unwrap();
        let reservation = h.admin.execute("INSERT INTO durable_reservations(id,run_id,step_id,amount_minor,currency) VALUES($1,$2,$3,0,'VND')", &[&Uuid::new_v4().to_string(), &second.run_id, &first.step_id]).await.unwrap_err();
        assert_eq!(reservation.code().map(|code| code.code()), Some("23514"));

        // Step two: an uncommitted reuse on one connection must make a concurrent attempt insert
        // on another connection wait, and then fail once the reuse commits.
        let contended = h.lease("contender").await;
        let prepared = h.store.reuse_candidates(&contended).await.unwrap().unwrap();
        let clip = prepared.candidates[0].clip.id.clone();
        let writer = connect(&h.app_config).await;
        writer.batch_execute("BEGIN").await.unwrap();
        writer
            .execute("UPDATE durable_steps SET status='succeeded',reused_clip_id=$2,lease_owner=NULL,lease_expires_at=NULL WHERE id=$1", &[&contended.step_id, &clip])
            .await
            .unwrap();
        let racer = connect(&h.app_config).await;
        let step = contended.step_id.clone();
        let insert = tokio::spawn(async move {
            racer
                .execute("INSERT INTO durable_attempts(id,step_id,lease_token,operation_id,reservation_id,intent_bytes,attempt_number) VALUES($1,$2,1,$3,$4,'{}',1)", &[&Uuid::new_v4().to_string(), &step, &Uuid::new_v4().to_string(), &Uuid::new_v4().to_string()])
                .await
        });
        tokio::time::sleep(std::time::Duration::from_millis(500)).await;
        assert!(
            !insert.is_finished(),
            "the attempt insert must wait for the uncommitted reuse"
        );
        writer.batch_execute("COMMIT").await.unwrap();
        let outcome = tokio::time::timeout(std::time::Duration::from_secs(5), insert)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            outcome.unwrap_err().code().map(|code| code.code()),
            Some("23514")
        );
        assert_eq!(attempts(&h, &second.run_id).await, 0);
        assert_invariants(&h).await;
    }

    #[tokio::test]
    #[ignore = "disposable PostgreSQL cluster required"]
    async fn upgrading_over_unfinished_pre_registry_steps_keeps_them_working_and_never_reuses_or_registers_them(
    ) {
        let h = Harness::new_before_durable_migrations(4).await;
        let durable = include_str!("../migrations/0007_durable_jobs.sql");
        h.admin.batch_execute(durable).await.unwrap();
        h.admin
            .execute(
                "INSERT INTO cantos_migrations(version,checksum) VALUES(7,$1)",
                &[&token_hash(durable)],
            )
            .await
            .unwrap();
        // A run created by the previous release: four pending steps, no speech fingerprint column.
        let run = Uuid::new_v4().to_string();
        let job = Uuid::new_v4().to_string();
        h.admin.execute("INSERT INTO durable_runs(id,script_id,snapshot_id,created_by,operation_id,input_digest,mode,request_bytes,max_attempts,base_seconds,cap_seconds) VALUES($1,$2,$3,'alice',$4,$5,'synthetic_fake','legacy',2,1,2)", &[&run, &h.script, &h.snapshot.id, &Uuid::new_v4().to_string(), &h.snapshot.document.input_digest]).await.unwrap();
        h.admin
            .execute(
                "INSERT INTO durable_jobs(id,run_id,status) VALUES($1,$2,'queued')",
                &[&job, &run],
            )
            .await
            .unwrap();
        h.admin.execute("INSERT INTO durable_budgets(run_id,currency,limit_minor,reserved_minor,settled_minor) VALUES($1,'VND',4,0,0)", &[&run]).await.unwrap();
        for line in &h.snapshot.document.resolved {
            h.admin.execute("INSERT INTO durable_steps(id,job_id,dialogue_id,input_fingerprint,status) VALUES($1,$2,$3,$4,'pending')", &[&Uuid::new_v4().to_string(), &job, &line.dialogue_id, &cantos_api::jobs::input_fingerprint(line).unwrap()]).await.unwrap();
        }
        let mut admin_config = h.app_config.clone();
        admin_config.user("cantos_test_admin");
        migrate(&admin_config).await.unwrap();

        // Existing rows survive with no position-free identity; the new success rule is in force.
        assert_eq!(h.count("SELECT count(*) FROM durable_steps s JOIN durable_jobs j ON j.id=s.job_id WHERE j.run_id=$1 AND s.speech_fingerprint IS NULL", &run).await, 4);
        let rules: i64 = h.count_all("SELECT count(*) FROM pg_constraint WHERE conrelid='durable_steps'::regclass AND contype='c' AND conname='durable_step_success_source'").await;
        assert_eq!(rules, 1);
        let legacy_rules: i64 = h.count_all("SELECT count(*) FROM pg_constraint WHERE conrelid='durable_steps'::regclass AND contype='c' AND pg_get_constraintdef(oid) LIKE '%succeeded%accepted_attempt_id%' AND conname<>'durable_step_success_source'").await;
        assert_eq!(
            legacy_rules, 0,
            "the superseded success constraint must be gone"
        );

        // The old run completes through the ordinary path: no lookup, no registration.
        let state = h.drain(&run).await;
        assert_eq!(
            (state.status, state.completed_steps, state.reused_steps),
            (RunStatus::Completed, 4, 0)
        );
        assert_eq!(h.executions(), 4);
        assert_eq!(h.count_all("SELECT count(*) FROM speech_clips").await, 0);

        // New runs fingerprint their steps; with no clip yet they dispatch, then later runs reuse.
        let fresh = h.enqueue(ExecutionMode::SyntheticFake).await;
        let state = h.drain(&fresh.run_id).await;
        assert_eq!(
            (state.status, state.reused_steps),
            (RunStatus::Completed, 0)
        );
        assert_eq!(h.executions(), 8);
        let again = h.enqueue(ExecutionMode::SyntheticFake).await;
        let state = h.drain(&again.run_id).await;
        assert_eq!(
            (state.status, state.reused_steps),
            (RunStatus::Completed, 4)
        );
        assert_eq!(h.executions(), 8);
        assert_invariants(&h).await;
    }
}
