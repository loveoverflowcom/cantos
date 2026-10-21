//! Real disposable PostgreSQL evidence, selected by the existing xtask PostgreSQL gate.
use std::{env, net::IpAddr};

use tokio_postgres::{Client, Config, NoTls};

use super::*;
use crate::platform::{
    postgres::{local_config, migrate},
    theatre_registration::domain::NetworkKey,
};

const NAMESPACE: &str = "https://identity.example.test/oauth2/openid/cantos-theatre-web-bff";

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
    repository: PostgresRegistrationRepository,
}

impl Harness {
    async fn new() -> Self {
        let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("cargo xtask verify postgres");
        let mut config = local_config(&url).unwrap();
        assert_eq!(config.get_user(), Some("cantos_test_admin"));
        assert_eq!(config.get_dbname(), Some("postgres"));
        let db = format!("cantos_test_{}", Uuid::new_v4().simple());
        connect(&config)
            .await
            .batch_execute(&format!("CREATE DATABASE {db}"))
            .await
            .unwrap();
        config.dbname(&db);
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        admin.batch_execute(
            "GRANT SELECT,INSERT ON theatre_registrations,theatre_registration_mail_starts TO cantos_app;
             GRANT UPDATE(token_hash,expires_at,last_sent_at,phase,person_id,block_reason) ON theatre_registrations TO cantos_app;",
        ).await.unwrap();
        let mut app_config = config.clone();
        app_config.user("cantos_app");
        let repository =
            PostgresRegistrationRepository::new(Store::new(app_config).unwrap(), NAMESPACE)
                .unwrap();
        Self {
            admin,
            config,
            repository,
        }
    }

    async fn age_challenge(&self, id: Uuid) {
        self.admin.execute(
            "UPDATE theatre_registrations SET last_sent_at=clock_timestamp()-interval '31 seconds' WHERE namespace=$1 AND id=$2::text::uuid",
            &[&NAMESPACE, &id.to_string()],
        ).await.unwrap();
    }

    async fn phase(&self, id: Uuid) -> String {
        self.admin
            .query_one(
                "SELECT phase FROM theatre_registrations WHERE namespace=$1 AND id=$2::text::uuid",
                &[&NAMESPACE, &id.to_string()],
            )
            .await
            .unwrap()
            .get(0)
    }

    async fn mail_starts(&self) -> i64 {
        self.admin
            .query_one("SELECT count(*) FROM theatre_registration_mail_starts", &[])
            .await
            .unwrap()
            .get(0)
    }
}

fn candidate(email: &str, token_digit: char, network: u8) -> Candidate {
    Candidate {
        registration: PendingRegistration::new(Uuid::new_v4(), Email::parse(email).unwrap())
            .unwrap(),
        token_hash: token(token_digit),
        network_key: NetworkKey::from_peer(IpAddr::from([192, 0, 2, network]), &[7; 32]).unwrap(),
    }
}

fn token(digit: char) -> TokenHash {
    TokenHash::from_token(&digit.to_string().repeat(64)).unwrap()
}

fn policy(global: u32, network: u32) -> RegistrationPolicy {
    RegistrationPolicy::new(300, 30, 3600, global, network).unwrap()
}

fn delivered(outcome: PrepareOutcome) -> PendingRegistration {
    match outcome {
        PrepareOutcome::Deliver(registration) => registration,
        PrepareOutcome::Suppressed => panic!("expected a new verification-mail start"),
    }
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn resend_rotates_one_email_proof_and_preserves_the_original_person_binding() {
    let h = Harness::new().await;
    let first = delivered(
        h.repository
            .prepare(candidate("Minh@EXAMPLE.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    assert_eq!(
        h.repository
            .prepare(candidate("minh@example.test", 'b', 1), policy(8, 4))
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    h.age_challenge(first.id()).await;
    let resent = delivered(
        h.repository
            .prepare(candidate("minh@example.test", 'b', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    assert_eq!(resent, first);
    assert_eq!(resent.email().as_str(), "Minh@example.test");
    assert_eq!(h.mail_starts().await, 2);
    assert_eq!(h.repository.claim(token('a')).await.unwrap(), None);
    assert_eq!(h.repository.claim(token('b')).await.unwrap(), Some(first));
    assert_eq!(h.repository.claim(token('b')).await.unwrap(), None);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn concurrent_claim_and_prepare_grant_only_one_create_and_one_initial_mail() {
    let h = Harness::new().await;
    let first = candidate("anh@example.test", 'a', 1);
    let second = candidate("ANH@example.test", 'b', 1);
    let (one, two) = tokio::join!(
        h.repository.prepare(first, policy(8, 4)),
        h.repository.prepare(second, policy(8, 4))
    );
    let accepted = [one.unwrap(), two.unwrap()];
    assert_eq!(
        accepted
            .iter()
            .filter(|outcome| matches!(outcome, PrepareOutcome::Deliver(_)))
            .count(),
        1
    );
    let hash = if matches!(accepted[0], PrepareOutcome::Deliver(_)) {
        token('a')
    } else {
        token('b')
    };
    let (one, two) = tokio::join!(h.repository.claim(hash), h.repository.claim(hash));
    assert_eq!(
        [one.unwrap(), two.unwrap()].into_iter().flatten().count(),
        1
    );
    assert_eq!(h.mail_starts().await, 1);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn namespace_and_network_budgets_are_durable_and_atomic_across_connections() {
    let h = Harness::new().await;
    let (one, two) = tokio::join!(
        h.repository
            .prepare(candidate("one@example.test", 'a', 1), policy(2, 1)),
        h.repository
            .prepare(candidate("two@example.test", 'b', 1), policy(2, 1))
    );
    assert_eq!(
        [one.unwrap(), two.unwrap()]
            .into_iter()
            .filter(|v| matches!(v, PrepareOutcome::Deliver(_)))
            .count(),
        1
    );
    delivered(
        h.repository
            .prepare(candidate("three@example.test", 'c', 2), policy(2, 1))
            .await
            .unwrap(),
    );
    let restarted =
        PostgresRegistrationRepository::new(h.repository.store.clone(), NAMESPACE).unwrap();
    assert_eq!(
        restarted
            .prepare(candidate("four@example.test", 'd', 3), policy(2, 1))
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    assert_eq!(h.mail_starts().await, 2);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn cancel_and_expiry_refuse_create_but_allow_a_later_fresh_challenge() {
    let h = Harness::new().await;
    let registration = delivered(
        h.repository
            .prepare(candidate("lan@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    h.age_challenge(registration.id()).await;
    h.repository.cancel(token('a')).await.unwrap();
    h.repository.cancel(token('a')).await.unwrap();
    assert_eq!(h.phase(registration.id()).await, "cancelled");
    assert_eq!(h.repository.claim(token('a')).await.unwrap(), None);
    assert_eq!(
        delivered(
            h.repository
                .prepare(candidate("lan@example.test", 'b', 1), policy(8, 4))
                .await
                .unwrap()
        ),
        registration
    );
    h.admin.execute(
        "UPDATE theatre_registrations SET expires_at=clock_timestamp()-interval '1 second',last_sent_at=clock_timestamp()-interval '60 seconds' WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &registration.id().to_string()],
    ).await.unwrap();
    assert_eq!(h.repository.claim(token('b')).await.unwrap(), None);
    assert_eq!(h.phase(registration.id()).await, "awaiting_verification");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn expiry_is_rechecked_after_the_claim_waits_for_a_registration_row_lock() {
    let h = Harness::new().await;
    let registration = delivered(
        h.repository
            .prepare(candidate("tuyet@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    let mut locker = connect(&h.config).await;
    let transaction = locker.transaction().await.unwrap();
    transaction
        .query_one(
            "SELECT id::text FROM theatre_registrations WHERE namespace=$1 AND id=$2::text::uuid FOR UPDATE",
            &[&NAMESPACE, &registration.id().to_string()],
        )
        .await
        .unwrap();
    let repository = h.repository.clone();
    let claim = tokio::spawn(async move { repository.claim(token('a')).await });
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let waiting: bool = h.admin.query_one(
                "SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname=current_database() AND usename='cantos_app' AND wait_event_type='Lock' AND query LIKE 'SELECT id::text,email FROM theatre_registrations%')", &[],
            ).await.unwrap().get(0);
            if waiting {
                break;
            }
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    transaction.execute(
        "UPDATE theatre_registrations SET expires_at=clock_timestamp()-interval '1 second',last_sent_at=clock_timestamp()-interval '60 seconds' WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &registration.id().to_string()],
    ).await.unwrap();
    transaction.commit().await.unwrap();
    assert_eq!(claim.await.unwrap().unwrap(), None);
    assert_eq!(h.phase(registration.id()).await, "awaiting_verification");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn cancel_racing_claim_either_prevents_create_or_leaves_the_claim_fence_intact() {
    let h = Harness::new().await;
    let registration = delivered(
        h.repository
            .prepare(candidate("trang@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    let (claim, cancel) = tokio::join!(
        h.repository.claim(token('a')),
        h.repository.cancel(token('a'))
    );
    cancel.unwrap();
    let expected = if claim.unwrap().is_some() {
        "creating"
    } else {
        "cancelled"
    };
    assert_eq!(h.phase(registration.id()).await, expected);
    assert_eq!(h.repository.claim(token('a')).await.unwrap(), None);
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn create_unknown_and_lost_responses_can_never_reopen_public_provisioning() {
    let h = Harness::new().await;
    let registration = delivered(
        h.repository
            .prepare(candidate("bao@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    assert_eq!(
        h.repository.claim(token('a')).await.unwrap(),
        Some(registration.clone())
    );
    let restarted =
        PostgresRegistrationRepository::new(h.repository.store.clone(), NAMESPACE).unwrap();
    assert_eq!(restarted.claim(token('a')).await.unwrap(), None);
    assert_eq!(
        restarted
            .prepare(candidate("bao@example.test", 'b', 1), policy(8, 4))
            .await
            .unwrap(),
        PrepareOutcome::Suppressed
    );
    restarted.cancel(token('a')).await.unwrap();
    assert_eq!(h.phase(registration.id()).await, "creating");
    assert!(!restarted.begin_setup(registration.id()).await.unwrap());
    restarted
        .block(registration.id(), BlockReason::CreateUncertain)
        .await
        .unwrap();
    assert_eq!(h.phase(registration.id()).await, "blocked");
    assert_eq!(restarted.claim(token('a')).await.unwrap(), None);
    let reason: String = h.admin.query_one(
        "SELECT block_reason FROM theatre_registrations WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &registration.id().to_string()],
    ).await.unwrap().get(0);
    assert_eq!(reason, "create_uncertain");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn each_positive_create_setup_delivery_fence_advances_at_most_once() {
    let h = Harness::new().await;
    let registration = delivered(
        h.repository
            .prepare(candidate("ha@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    let id = registration.id();
    assert!(!h.repository.complete(id).await.unwrap());
    h.repository.claim(token('a')).await.unwrap().unwrap();
    let person = CreatedPerson::confirmed(id, id).unwrap();
    assert!(h.repository.created(id, person.clone()).await.unwrap());
    assert!(!h.repository.created(id, person).await.unwrap());
    assert!(h.repository.begin_setup(id).await.unwrap());
    assert!(!h.repository.begin_setup(id).await.unwrap());
    assert!(h.repository.begin_delivery(id).await.unwrap());
    assert!(!h.repository.begin_delivery(id).await.unwrap());
    assert!(h.repository.complete(id).await.unwrap());
    assert!(!h.repository.complete(id).await.unwrap());
    h.repository
        .block(id, BlockReason::SetupDeliveryUncertain)
        .await
        .unwrap();
    assert_eq!(h.phase(id).await, "complete");
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn another_issuer_cannot_claim_or_advance_the_original_registration() {
    let h = Harness::new().await;
    let registration = delivered(
        h.repository
            .prepare(candidate("vy@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    let other = PostgresRegistrationRepository::new(
        h.repository.store.clone(),
        "https://other.example.test/issuer",
    )
    .unwrap();
    assert_eq!(other.claim(token('a')).await.unwrap(), None);
    other.cancel(token('a')).await.unwrap();
    assert!(!other.begin_setup(registration.id()).await.unwrap());
    other
        .block(registration.id(), BlockReason::CreateUncertain)
        .await
        .unwrap();
    assert_eq!(h.phase(registration.id()).await, "awaiting_verification");
    assert_eq!(
        h.repository.claim(token('a')).await.unwrap(),
        Some(registration)
    );
}

#[tokio::test]
#[ignore = "xtask disposable PostgreSQL; no live issuer or mail"]
async fn baseline_replay_preserves_checksums_and_database_refuses_phase_rollback_or_audit_edits() {
    let h = Harness::new().await;
    let previous: Vec<(i32, Vec<u8>)> = h
        .admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(previous.len(), 4);
    // Current baseline replay preserves existing authoritative records.
    h.admin
        .batch_execute("INSERT INTO actors(id) VALUES('registration-legacy-fixture');")
        .await
        .unwrap();
    migrate(&h.config).await.unwrap();
    migrate(&h.config).await.unwrap();
    let retained: Vec<(i32, Vec<u8>)> = h
        .admin
        .query(
            "SELECT version,checksum FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect();
    assert_eq!(retained, previous);
    assert!(h
        .admin
        .query_one(
            "SELECT active FROM actors WHERE id='registration-legacy-fixture'",
            &[]
        )
        .await
        .unwrap()
        .get::<_, bool>(0));
    // Reapply grants only inside this fixture; migrations deliberately do not activate roles.
    h.admin.batch_execute(
        "GRANT SELECT,INSERT ON theatre_registrations,theatre_registration_mail_starts TO cantos_app;
         GRANT UPDATE(token_hash,expires_at,last_sent_at,phase,person_id,block_reason) ON theatre_registrations TO cantos_app;",
    ).await.unwrap();
    let registration = delivered(
        h.repository
            .prepare(candidate("nhat@example.test", 'a', 1), policy(8, 4))
            .await
            .unwrap(),
    );
    h.repository.claim(token('a')).await.unwrap().unwrap();
    let rollback = h.admin.execute(
        "UPDATE theatre_registrations SET phase='awaiting_verification' WHERE namespace=$1 AND id=$2::text::uuid",
        &[&NAMESPACE, &registration.id().to_string()],
    ).await.unwrap_err();
    assert_eq!(rollback.code().unwrap().code(), "23514");
    let audit_edit = h
        .admin
        .execute("DELETE FROM theatre_registration_mail_starts", &[])
        .await
        .unwrap_err();
    assert_eq!(audit_edit.code().unwrap().code(), "55000");
    let grants = h.admin.query_one(
        "SELECT EXISTS(SELECT 1 FROM pg_class c CROSS JOIN LATERAL aclexplode(COALESCE(c.relacl,acldefault('r',c.relowner))) a WHERE c.oid='theatre_registrations'::regclass AND a.grantee=0),has_table_privilege('cantos_app','theatre_registration_mail_starts','DELETE')", &[],
    ).await.unwrap();
    assert!(!grants.get::<_, bool>(0));
    assert!(!grants.get::<_, bool>(1));
    let columns: Vec<String> = h.admin.query(
        "SELECT column_name FROM information_schema.columns WHERE table_name IN ('theatre_registrations','theatre_registration_mail_starts')", &[],
    ).await.unwrap().iter().map(|row| row.get(0)).collect();
    assert!(!columns.iter().any(|v| [
        "password",
        "raw_token",
        "setup_url",
        "access_token",
        "peer_ip"
    ]
    .contains(&v.as_str())));
}
