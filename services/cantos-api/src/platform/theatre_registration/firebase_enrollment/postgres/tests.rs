//! Real PostgreSQL tests selected only by xtask's disposable-cluster gate.
use std::{env, net::IpAddr, time::Duration};

use tokio_postgres::{Client, Config, NoTls};

use super::*;
use crate::platform::postgres::{local_config, migrate};

const NAMESPACE: &str = "https://identity.example.test/oauth2/openid/cantos-theatre-web-bff";
const PROJECT: &str = "cantos-fixture";
const PRODUCTION_REGISTRATION_GRANTS: &str =
    include_str!("../../../../../../../infra/vps/postgres/theatre-registration-grants.sql");

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

struct Harness {
    admin: Client,
    config: Config,
    repository: PostgresFirebaseEnrollmentRepository,
}

impl Harness {
    async fn new() -> Self {
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
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        // The deployed optional grant file, not a hand-written copy: these tests prove it is
        // exactly sufficient for the repository.
        admin
            .batch_execute(PRODUCTION_REGISTRATION_GRANTS)
            .await
            .unwrap();
        let mut app_config = config.clone();
        app_config.user("cantos_app");
        let repository = PostgresFirebaseEnrollmentRepository::new(
            Store::new(app_config).unwrap(),
            NAMESPACE,
            PROJECT,
        )
        .unwrap();
        Self {
            admin,
            config,
            repository,
        }
    }

    async fn phase(&self, id: Uuid) -> String {
        self.admin
            .query_one(
                "SELECT phase FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
                &[&NAMESPACE, &id.to_string()],
            )
            .await
            .unwrap()
            .get(0)
    }

    async fn send_starts(&self) -> i64 {
        self.admin
            .query_one("SELECT count(*) FROM theatre_firebase_send_starts", &[])
            .await
            .unwrap()
            .get(0)
    }

    async fn awaiting(&self, email: &str) -> Enrollment {
        let enrollment = bootstrap(
            self.repository
                .prepare(candidate(email), network(1), policy(8, 4))
                .await
                .unwrap(),
        );
        let id = enrollment.id();
        assert!(self.repository.ready(id).await.unwrap());
        assert!(self.repository.begin_send(id).await.unwrap());
        assert!(self.repository.begin_delivery(id).await.unwrap());
        assert!(self.repository.sent(id).await.unwrap());
        enrollment
    }

    /// Synthetic historical rows use a past send reservation and a bounded deadline.
    /// Only fixture SQL skips the application timing checks; guards/transitions stay on.
    async fn seed_awaiting(&self, email: &str, expires_in_ms: i64) -> Enrollment {
        let enrollment = candidate(email);
        self.admin.execute(
            "WITH clock AS MATERIALIZED(SELECT clock_timestamp() AS now) INSERT INTO theatre_firebase_enrollments(namespace,id,project,uid,email,email_key,accepted_at,expires_at,last_sent_at,phase) SELECT $1,$2::text::uuid,$3,$4,$5,$6,clock.now-interval '120 seconds',clock.now+($7::bigint*interval '1 millisecond'),clock.now-interval '60 seconds','bootstrap_started' FROM clock",
            &[&NAMESPACE, &enrollment.id().to_string(), &PROJECT, &enrollment.uid(), &enrollment.registration().email().as_str(), &enrollment.registration().email().canonical_key(), &expires_in_ms],
        ).await.unwrap();
        for phase in ["ready", "send_started", "mail_started", "awaiting_action"] {
            self.admin.execute(
                "UPDATE theatre_firebase_enrollments SET phase=$3 WHERE namespace=$1 AND id=$2::text::uuid",
                &[&NAMESPACE, &enrollment.id().to_string(), &phase],
            ).await.unwrap();
        }
        enrollment
    }

    async fn cancel_before_consume(&self, id: Uuid) {
        // No public cancellation endpoint or initiating-browser receipt exists.
        // This models an internal/operator cancellation racing the consuming fence.
        self.admin.execute(
            "UPDATE theatre_firebase_enrollments SET phase='cancelled' WHERE namespace=$1 AND id=$2::text::uuid AND phase IN ('awaiting_action','inspecting')",
            &[&NAMESPACE, &id.to_string()],
        ).await.unwrap();
    }

    async fn wait_until_expired(&self, id: Uuid) {
        tokio::time::timeout(Duration::from_secs(5), async {
            loop {
                let expired: bool = self.admin.query_one(
                    "SELECT expires_at<=clock_timestamp() FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
                    &[&NAMESPACE, &id.to_string()],
                ).await.unwrap().get(0);
                if expired {
                    return;
                }
                self.admin.query_one("SELECT pg_sleep(0.02)", &[]).await.unwrap();
            }
        }).await.unwrap();
    }
}

fn candidate(email: &str) -> Enrollment {
    Enrollment::new(
        PendingRegistration::new(Uuid::new_v4(), Email::parse(email).unwrap()).unwrap(),
        PROJECT,
    )
    .unwrap()
}

fn network(last: u8) -> NetworkKey {
    NetworkKey::from_peer(IpAddr::from([192, 0, 2, last]), &[7; 32]).unwrap()
}

fn policy(global: u32, network: u32) -> RegistrationPolicy {
    RegistrationPolicy::new(300, 30, 3600, global, network).unwrap()
}

fn bootstrap(outcome: PrepareOutcome) -> Enrollment {
    match outcome {
        PrepareOutcome::Bootstrap(enrollment) => enrollment,
        other => panic!("expected bootstrap, received {other:?}"),
    }
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn concurrent_start_reserves_exactly_one_bootstrap_and_one_durable_budget_receipt() {
    let h = Harness::new().await;
    let (one, two) = tokio::join!(
        h.repository
            .prepare(candidate("Minh@EXAMPLE.test"), network(1), policy(8, 4)),
        h.repository
            .prepare(candidate("minh@example.test"), network(1), policy(8, 4)),
    );
    let outcomes = [one.unwrap(), two.unwrap()];
    let accepted: Vec<_> = outcomes
        .into_iter()
        .filter_map(|outcome| match outcome {
            PrepareOutcome::Bootstrap(enrollment) => Some(enrollment),
            PrepareOutcome::Suppressed => None,
            PrepareOutcome::Resend(_) => panic!("initial request cannot be a resend"),
        })
        .collect();
    assert_eq!(accepted.len(), 1);
    assert_eq!(h.phase(accepted[0].id()).await, "bootstrap_started");
    assert_eq!(h.send_starts().await, 1);
    assert_eq!(
        h.repository.claim_action(accepted[0].id()).await.unwrap(),
        None
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn resend_reuses_only_the_live_operation_uid_and_preserves_its_absolute_deadline() {
    let h = Harness::new().await;
    let first = h.seed_awaiting("Minh@example.test", 300_000).await;
    let deadline: String = h.admin.query_one(
        "SELECT expires_at::text FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &first.id().to_string()],
    ).await.unwrap().get(0);
    assert_eq!(
        h.repository
            .prepare(candidate("minh@example.test"), network(1), policy(8, 4))
            .await
            .unwrap(),
        PrepareOutcome::Resend(first.clone())
    );
    assert_eq!(h.phase(first.id()).await, "send_started");
    assert_eq!(h.send_starts().await, 1);
    let unchanged: String = h.admin.query_one(
        "SELECT expires_at::text FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &first.id().to_string()],
    ).await.unwrap().get(0);
    assert_eq!(deadline, unchanged);
    assert!(h.repository.begin_delivery(first.id()).await.unwrap());
    assert!(h.repository.sent(first.id()).await.unwrap());
    assert_eq!(
        h.repository
            .prepare(candidate("minh@example.test"), network(1), policy(8, 4))
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    assert_eq!(h.send_starts().await, 1);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn expiry_during_credential_exchange_prevents_the_actual_mail_dispatch_fence() {
    let h = Harness::new().await;
    let enrollment = h.seed_awaiting("exchange-wait@example.test", 1000).await;
    let id = enrollment.id();
    assert_eq!(
        h.repository
            .prepare(
                candidate("exchange-wait@example.test"),
                network(1),
                policy(8, 4)
            )
            .await
            .unwrap(),
        PrepareOutcome::Resend(enrollment)
    );
    assert!(h
        .repository
        .dispatchable(id, Phase::SendStarted)
        .await
        .unwrap());
    assert!(!h.repository.sent(id).await.unwrap());
    // Models the elapsed provider token-exchange wait; no token enters this repository.
    h.wait_until_expired(id).await;
    assert!(!h.repository.begin_delivery(id).await.unwrap());
    assert!(!h
        .repository
        .dispatchable(id, Phase::SendStarted)
        .await
        .unwrap());
    assert_eq!(h.phase(id).await, "send_started");
    let restarted =
        PostgresFirebaseEnrollmentRepository::new(h.repository.store.clone(), NAMESPACE, PROJECT)
            .unwrap();
    assert_eq!(
        restarted
            .prepare(
                candidate("exchange-wait@example.test"),
                network(2),
                policy(8, 4)
            )
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    assert_eq!(h.send_starts().await, 1);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn actual_mail_dispatch_has_a_separate_durable_one_winner_fence() {
    let h = Harness::new().await;
    let enrollment = bootstrap(
        h.repository
            .prepare(
                candidate("mail-fence@example.test"),
                network(1),
                policy(8, 4),
            )
            .await
            .unwrap(),
    );
    let id = enrollment.id();
    assert!(!h.repository.begin_delivery(id).await.unwrap());
    assert!(h.repository.ready(id).await.unwrap());
    assert!(h.repository.begin_send(id).await.unwrap());
    assert!(!h.repository.sent(id).await.unwrap());
    let (one, two) = tokio::join!(
        h.repository.begin_delivery(id),
        h.repository.begin_delivery(id)
    );
    assert_eq!(
        [one.unwrap(), two.unwrap()]
            .into_iter()
            .filter(|allowed| *allowed)
            .count(),
        1
    );
    assert_eq!(h.phase(id).await, "mail_started");
    let restarted =
        PostgresFirebaseEnrollmentRepository::new(h.repository.store.clone(), NAMESPACE, PROJECT)
            .unwrap();
    assert!(!restarted.begin_delivery(id).await.unwrap());
    assert_eq!(
        restarted
            .prepare(
                candidate("mail-fence@example.test"),
                network(2),
                policy(8, 4)
            )
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    restarted
        .block(id, FailureStage::SendUncertain)
        .await
        .unwrap();
    assert_eq!(h.phase(id).await, "blocked");
    assert!(!restarted.sent(id).await.unwrap());
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn global_and_network_limits_are_atomic_and_survive_repository_restart() {
    let h = Harness::new().await;
    let (one, two) = tokio::join!(
        h.repository
            .prepare(candidate("one@example.test"), network(1), policy(2, 1)),
        h.repository
            .prepare(candidate("two@example.test"), network(1), policy(2, 1)),
    );
    assert_eq!(
        [one.unwrap(), two.unwrap()]
            .into_iter()
            .filter(|v| matches!(v, PrepareOutcome::Bootstrap(_)))
            .count(),
        1
    );
    bootstrap(
        h.repository
            .prepare(candidate("three@example.test"), network(2), policy(2, 1))
            .await
            .unwrap(),
    );
    let restarted =
        PostgresFirebaseEnrollmentRepository::new(h.repository.store.clone(), NAMESPACE, PROJECT)
            .unwrap();
    assert_eq!(
        restarted
            .prepare(candidate("four@example.test"), network(3), policy(2, 1))
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    assert_eq!(h.send_starts().await, 2);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn inspection_and_consumption_fences_each_grant_only_one_winner() {
    let h = Harness::new().await;
    let enrollment = h.awaiting("lan@example.test").await;
    let id = enrollment.id();
    let (one, two) = tokio::join!(h.repository.claim_action(id), h.repository.claim_action(id));
    assert_eq!(
        [one.unwrap(), two.unwrap()]
            .into_iter()
            .flatten()
            .collect::<Vec<_>>(),
        [enrollment]
    );
    let (one, two) = tokio::join!(
        h.repository.begin_consume(id),
        h.repository.begin_consume(id)
    );
    assert_eq!(
        [one.unwrap(), two.unwrap()]
            .into_iter()
            .filter(|v| *v)
            .count(),
        1
    );
    let restarted =
        PostgresFirebaseEnrollmentRepository::new(h.repository.store.clone(), NAMESPACE, PROJECT)
            .unwrap();
    assert_eq!(restarted.claim_action(id).await.unwrap(), None);
    assert!(!restarted.begin_consume(id).await.unwrap());
    restarted.release_inspection(id).await.unwrap();
    assert_eq!(h.phase(id).await, "consuming");
    assert_eq!(
        restarted
            .prepare(candidate("lan@example.test"), network(2), policy(8, 4))
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn nonconsuming_inspection_can_release_but_cancellation_never_resurrects_a_uid() {
    let h = Harness::new().await;
    let first = h.awaiting("trang@example.test").await;
    let id = first.id();
    assert_eq!(
        h.repository.claim_action(id).await.unwrap(),
        Some(first.clone())
    );
    h.repository.release_inspection(id).await.unwrap();
    assert_eq!(
        h.repository.claim_action(id).await.unwrap(),
        Some(first.clone())
    );
    h.cancel_before_consume(id).await;
    h.repository.release_inspection(id).await.unwrap();
    assert!(!h.repository.begin_consume(id).await.unwrap());
    assert_eq!(h.phase(id).await, "cancelled");
    let next = bootstrap(
        h.repository
            .prepare(candidate("trang@example.test"), network(1), policy(8, 4))
            .await
            .unwrap(),
    );
    assert_ne!(next.id(), first.id());
    assert_ne!(next.uid(), first.uid());
    assert_eq!(h.repository.claim_action(first.id()).await.unwrap(), None);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn action_bound_cancellation_commits_once_only_after_the_consumption_fence() {
    let h = Harness::new().await;
    let first = h.awaiting("cancel@example.test").await;
    let id = first.id();
    assert!(!h.repository.cancel_consumed(id).await.unwrap());
    assert_eq!(
        h.repository.claim_action(id).await.unwrap(),
        Some(first.clone())
    );
    assert!(!h.repository.cancel_consumed(id).await.unwrap());
    assert!(h.repository.begin_consume(id).await.unwrap());
    assert!(h.repository.cancel_consumed(id).await.unwrap());
    assert!(!h.repository.cancel_consumed(id).await.unwrap());
    assert!(!h.repository.begin_create(id).await.unwrap());
    assert_eq!(h.repository.claim_action(id).await.unwrap(), None);
    let next = bootstrap(
        h.repository
            .prepare(candidate("cancel@example.test"), network(1), policy(8, 4))
            .await
            .unwrap(),
    );
    assert_ne!(next.id(), first.id());
    assert_ne!(next.uid(), first.uid());
    assert_eq!(h.phase(id).await, "cancelled");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn expired_operation_gets_a_new_identity_and_expired_provider_wait_cannot_create() {
    let h = Harness::new().await;
    let expired = h.seed_awaiting("expired@example.test", -1).await;
    let next = bootstrap(
        h.repository
            .prepare(candidate("expired@example.test"), network(1), policy(8, 4))
            .await
            .unwrap(),
    );
    assert_ne!(expired.id(), next.id());
    assert_ne!(expired.uid(), next.uid());
    assert_eq!(h.phase(expired.id()).await, "expired");
    assert_eq!(h.repository.claim_action(expired.id()).await.unwrap(), None);

    let waiting = h.seed_awaiting("provider-wait@example.test", 1000).await;
    assert_eq!(
        h.repository.claim_action(waiting.id()).await.unwrap(),
        Some(waiting.clone())
    );
    assert!(h.repository.begin_consume(waiting.id()).await.unwrap());
    h.wait_until_expired(waiting.id()).await;
    assert!(!h.repository.begin_create(waiting.id()).await.unwrap());
    assert_eq!(h.phase(waiting.id()).await, "consuming");
    assert_eq!(
        h.repository
            .prepare(
                candidate("provider-wait@example.test"),
                network(2),
                policy(8, 4)
            )
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn action_expiry_is_rechecked_after_waiting_for_the_enrollment_lock() {
    let h = Harness::new().await;
    let enrollment = h.seed_awaiting("tuyet@example.test", 1000).await;
    let id = enrollment.id();
    let mut locker = connect(&h.config).await;
    let tx = locker.transaction().await.unwrap();
    assert!(tx.query_one(
        "SELECT expires_at>clock_timestamp() FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid FOR UPDATE",
        &[&NAMESPACE, &id.to_string()],
    ).await.unwrap().get::<_, bool>(0));
    let repository = h.repository.clone();
    let claim = tokio::spawn(async move { repository.claim_action(id).await });
    tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let waiting: bool = h.admin.query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND usename='cantos_app' AND wait_event_type='Lock' AND query LIKE 'SELECT id::text,email,project,uid FROM theatre_firebase_enrollments%')", &[],
            ).await.unwrap().get(0);
            if waiting { break; }
            tokio::task::yield_now().await;
        }
    }).await.unwrap();
    h.wait_until_expired(id).await;
    tx.commit().await.unwrap();
    assert_eq!(claim.await.unwrap().unwrap(), None);
    assert_eq!(h.phase(id).await, "awaiting_action");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn expiry_during_receipt_writes_rolls_back_every_fresh_effect_fence() {
    let h = Harness::new().await;
    for (previous, next) in [
        ("ready", "send_started"),
        ("send_started", "mail_started"),
        ("mail_started", "awaiting_action"),
        ("inspecting", "consuming"),
        ("consuming", "creating"),
        ("consuming", "cancelled"),
        ("created", "setup_started"),
        ("setup_started", "handed_off"),
    ] {
        // Every fixture follows the real SQL phase graph. Send phases need a fresh
        // initial row because AwaitingAction cannot move backwards.
        let enrollment = if matches!(previous, "ready" | "send_started" | "mail_started") {
            let enrollment = candidate(&format!("early-receipt-{next}@example.test"));
            h.admin.execute(
                "WITH clock AS MATERIALIZED(SELECT clock_timestamp() AS now) INSERT INTO theatre_firebase_enrollments(namespace,id,project,uid,email,email_key,accepted_at,expires_at,last_sent_at,phase) SELECT $1,$2::text::uuid,$3,$4,$5,$6,clock.now,clock.now+interval '2 seconds',clock.now,'bootstrap_started' FROM clock",
                &[&NAMESPACE, &enrollment.id().to_string(), &PROJECT, &enrollment.uid(), &enrollment.registration().email().as_str(), &enrollment.registration().email().canonical_key()],
            ).await.unwrap();
            for phase in ["ready", "send_started", "mail_started"] {
                h.admin.execute(
                    "UPDATE theatre_firebase_enrollments SET phase=$3 WHERE namespace=$1 AND id=$2::text::uuid",
                    &[&NAMESPACE, &enrollment.id().to_string(), &phase],
                ).await.unwrap();
                if phase == previous {
                    break;
                }
            }
            enrollment
        } else {
            let enrollment = h
                .seed_awaiting(&format!("receipt-{next}@example.test"), 2000)
                .await;
            let id = enrollment.id();
            assert!(h.repository.claim_action(id).await.unwrap().is_some());
            if previous != "inspecting" {
                assert!(h.repository.begin_consume(id).await.unwrap());
            }
            if matches!(previous, "created" | "setup_started") {
                assert!(h.repository.begin_create(id).await.unwrap());
                let person = CreatedPerson::confirmed(id, id).unwrap();
                assert!(h.repository.created(id, person).await.unwrap());
            }
            if previous == "setup_started" {
                assert!(h.repository.begin_setup(id).await.unwrap());
            }
            enrollment
        };
        let id = enrollment.id();
        let before: i64 = h.admin.query_one(
            "SELECT count(*) FROM theatre_firebase_enrollment_receipts WHERE namespace=$1 AND enrollment_id=$2::text::uuid",
            &[&NAMESPACE, &id.to_string()],
        ).await.unwrap().get(0);
        let mut locker = connect(&h.config).await;
        let lock = locker.transaction().await.unwrap();
        lock.batch_execute(
            "LOCK TABLE theatre_firebase_enrollment_receipts IN ACCESS EXCLUSIVE MODE",
        )
        .await
        .unwrap();
        let repository = h.repository.clone();
        let transition =
            tokio::spawn(async move { repository.advance(id, previous, next, true).await });
        tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let waiting: bool = h.admin.query_one(
                    "SELECT EXISTS(SELECT 1 FROM pg_stat_activity a JOIN pg_locks l ON l.pid=a.pid WHERE a.datname=current_database() AND a.usename='cantos_app' AND a.wait_event_type='Lock' AND NOT l.granted AND l.relation='theatre_firebase_enrollment_receipts'::regclass)",
                    &[],
                ).await.unwrap().get(0);
                if waiting { break; }
                tokio::task::yield_now().await;
            }
        }).await.unwrap();
        assert!(h.admin.query_one(
            "SELECT expires_at>clock_timestamp() FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
            &[&NAMESPACE, &id.to_string()],
        ).await.unwrap().get::<_, bool>(0), "{previous} -> {next} reached receipt wait while fresh");
        h.wait_until_expired(id).await;
        lock.commit().await.unwrap();
        assert!(!transition.await.unwrap().unwrap(), "{previous} -> {next}");
        assert_eq!(h.phase(enrollment.id()).await, previous);
        let after: i64 = h.admin.query_one(
            "SELECT count(*) FROM theatre_firebase_enrollment_receipts WHERE namespace=$1 AND enrollment_id=$2::text::uuid",
            &[&NAMESPACE, &id.to_string()],
        ).await.unwrap().get(0);
        assert_eq!(
            after, before,
            "an expired effect cannot leave a transition receipt"
        );
    }
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn interrupted_bootstrap_send_and_blocked_outcomes_never_gain_an_automatic_retry() {
    let h = Harness::new().await;
    let bootstrap = bootstrap(
        h.repository
            .prepare(
                candidate("bootstrap@example.test"),
                network(1),
                policy(8, 4),
            )
            .await
            .unwrap(),
    );
    let send = h.awaiting("send@example.test").await;
    assert!(h
        .repository
        .claim_action(send.id())
        .await
        .unwrap()
        .is_some());
    assert!(h.repository.begin_consume(send.id()).await.unwrap());
    h.repository
        .block(send.id(), FailureStage::ConsumeUncertain)
        .await
        .unwrap();
    let restarted =
        PostgresFirebaseEnrollmentRepository::new(h.repository.store.clone(), NAMESPACE, PROJECT)
            .unwrap();
    for (email, id, expected) in [
        (
            "bootstrap@example.test",
            bootstrap.id(),
            "bootstrap_started",
        ),
        ("send@example.test", send.id(), "blocked"),
    ] {
        assert_eq!(
            restarted
                .prepare(candidate(email), network(2), policy(8, 4))
                .await
                .unwrap(),
            PrepareOutcome::Suppressed
        );
        assert_eq!(restarted.claim_action(id).await.unwrap(), None);
        assert_eq!(h.phase(id).await, expected);
    }
    let reason: String = h.admin.query_one(
        "SELECT block_reason FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &send.id().to_string()],
    ).await.unwrap().get(0);
    assert_eq!(reason, "consume_uncertain");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn create_and_setup_handoff_acknowledgements_are_one_shot_and_exactly_bound() {
    let h = Harness::new().await;
    let enrollment = h.awaiting("ha@example.test").await;
    let id = enrollment.id();
    assert!(!h.repository.begin_handoff(id).await.unwrap());
    h.repository.claim_action(id).await.unwrap().unwrap();
    assert!(h.repository.begin_consume(id).await.unwrap());
    assert!(h.repository.begin_create(id).await.unwrap());
    let other = Uuid::new_v4();
    assert!(!h
        .repository
        .created(id, CreatedPerson::confirmed(other, other).unwrap())
        .await
        .unwrap());
    let person = CreatedPerson::confirmed(id, id).unwrap();
    assert!(h.repository.created(id, person.clone()).await.unwrap());
    assert!(!h.repository.created(id, person).await.unwrap());
    assert!(h.repository.begin_setup(id).await.unwrap());
    assert!(!h.repository.begin_setup(id).await.unwrap());
    assert!(h.repository.begin_handoff(id).await.unwrap());
    assert!(!h.repository.begin_handoff(id).await.unwrap());
    h.repository
        .block(id, FailureStage::HandoffRefused)
        .await
        .unwrap();
    assert_eq!(h.phase(id).await, "handed_off");
    let phases: Vec<String> = h.admin.query(
        "SELECT phase FROM theatre_firebase_enrollment_receipts WHERE namespace=$1 AND enrollment_id=$2::text::uuid ORDER BY receipt_id",
        &[&NAMESPACE, &id.to_string()],
    ).await.unwrap().into_iter().map(|row| row.get(0)).collect();
    assert_eq!(
        phases,
        [
            "bootstrap_started",
            "ready",
            "send_started",
            "mail_started",
            "awaiting_action",
            "inspecting",
            "consuming",
            "creating",
            "created",
            "setup_started",
            "handed_off"
        ]
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn issuer_and_project_namespaces_cannot_claim_or_advance_another_enrollment() {
    let h = Harness::new().await;
    let enrollment = h.awaiting("vy@example.test").await;
    for (namespace, project) in [
        ("https://other.example.test/issuer", PROJECT),
        (NAMESPACE, "other-fixture"),
    ] {
        let other = PostgresFirebaseEnrollmentRepository::new(
            h.repository.store.clone(),
            namespace,
            project,
        )
        .unwrap();
        assert_eq!(other.claim_action(enrollment.id()).await.unwrap(), None);
        assert!(!other.begin_consume(enrollment.id()).await.unwrap());
        other
            .block(enrollment.id(), FailureStage::ConsumeUncertain)
            .await
            .unwrap();
    }
    assert_eq!(h.phase(enrollment.id()).await, "awaiting_action");
    let wrong_project = Enrollment::new(
        candidate("new@example.test").registration().clone(),
        "other-fixture",
    )
    .unwrap();
    assert_eq!(
        h.repository
            .prepare(wrong_project, network(2), policy(8, 4))
            .await
            .unwrap_err(),
        RegistrationError::InvalidState
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn database_guards_reject_binding_changes_replay_lifetime_extension_and_receipt_fabrication()
{
    let h = Harness::new().await;
    let enrollment = h.awaiting("nhat@example.test").await;
    let id = enrollment.id().to_string();
    for statement in [
        "UPDATE theatre_firebase_enrollments SET phase='bootstrap_started' WHERE namespace=$1 AND id=$2::text::uuid",
        "UPDATE theatre_firebase_enrollments SET email='other@example.test',email_key='other@example.test' WHERE namespace=$1 AND id=$2::text::uuid",
        "UPDATE theatre_firebase_enrollments SET expires_at=expires_at+interval '1 minute' WHERE namespace=$1 AND id=$2::text::uuid",
    ] {
        assert_eq!(h.admin.execute(statement, &[&NAMESPACE, &id]).await.unwrap_err().code().unwrap().code(), "23514");
    }
    let reused = h.admin.execute(
        "INSERT INTO theatre_firebase_enrollments(namespace,id,project,uid,email,email_key,accepted_at,expires_at,last_sent_at,phase) SELECT 'other-namespace',id,project,uid,'other@example.test','other@example.test',accepted_at,expires_at,last_sent_at,'bootstrap_started' FROM theatre_firebase_enrollments WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &id],
    ).await.unwrap_err();
    assert_eq!(reused.code().unwrap().code(), "23505");
    for table in [
        "theatre_firebase_send_starts",
        "theatre_firebase_enrollment_receipts",
    ] {
        let error = h
            .admin
            .execute(&format!("DELETE FROM {table}"), &[])
            .await
            .unwrap_err();
        assert_eq!(error.code().unwrap().code(), "55000");
    }
    let app = h.repository.store.connection().await.unwrap();
    let fabricated = app.execute(
        "INSERT INTO theatre_firebase_enrollment_receipts(namespace,enrollment_id,phase,recorded_at) VALUES($1,$2::text::uuid,'handed_off',clock_timestamp())",
        &[&NAMESPACE, &id],
    ).await.unwrap_err();
    assert_eq!(fabricated.code().unwrap().code(), "42501");
    let columns: Vec<String> = h.admin.query(
        "SELECT column_name FROM information_schema.columns WHERE table_name LIKE 'theatre_firebase_%'",
        &[],
    ).await.unwrap().into_iter().map(|row| row.get(0)).collect();
    assert!(!columns.iter().any(|column| [
        "password",
        "oob_code",
        "action_code",
        "setup_url",
        "access_token",
        "id_token",
        "refresh_token",
        "custom_token",
        "send_token",
        "mail_credential",
        "peer_ip"
    ]
    .contains(&column.as_str())));
    assert_eq!(h.phase(enrollment.id()).await, "awaiting_action");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live provider or mail"]
async fn baseline_replay_preserves_checksums_and_smtp_registration_rows() {
    let h = Harness::new().await;
    let checksums = h
        .admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap();
    let before: Vec<(i32, Vec<u8>)> = checksums
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(before.len(), 2);
    h.admin.batch_execute(
        "INSERT INTO actors(id) VALUES('firebase-legacy-fixture');
         INSERT INTO theatre_registrations(namespace,id,email,email_key,token_hash,expires_at,last_sent_at,phase)
         VALUES('legacy-smtp','10000000-0000-0000-0000-000000000001','Minh@example.test','minh@example.test',decode(repeat('ab',32),'hex'),clock_timestamp()+interval '5 minutes',clock_timestamp(),'awaiting_verification');",
    ).await.unwrap();
    migrate(&h.config).await.unwrap();
    migrate(&h.config).await.unwrap();
    let after: Vec<(i32, Vec<u8>)> = h
        .admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .into_iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(after, before);
    let legacy = h
        .admin
        .query_one(
            "SELECT email,phase FROM theatre_registrations WHERE namespace='legacy-smtp'",
            &[],
        )
        .await
        .unwrap();
    let email: String = legacy.get(0);
    assert_eq!(Email::parse(&email).unwrap().as_str(), "Minh@example.test");
    assert_eq!(legacy.get::<_, String>(1), "awaiting_verification");
    assert!(h
        .admin
        .query_one(
            "SELECT active FROM actors WHERE id='firebase-legacy-fixture'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    let public_access: bool = h.admin.query_one(
        "SELECT EXISTS(SELECT 1 FROM pg_class c CROSS JOIN LATERAL aclexplode(COALESCE(c.relacl,acldefault('r',c.relowner))) a WHERE c.oid='theatre_firebase_enrollments'::regclass AND a.grantee=0)", &[],
    ).await.unwrap().get(0);
    assert!(!public_access);
}
