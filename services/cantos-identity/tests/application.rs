//! Application journeys over real Turso files and real Argon2id, with an in-process capture
//! mail port. These establish the identity application's behavior; the production executable
//! and its RPC boundary are covered separately in `process.rs`.
#![cfg(feature = "service")]

mod support;

use std::{net::IpAddr, sync::Arc, time::Duration};

use cantos_identity::{
    application::{
        ClientAddress, Dispatch, IdentityService, Limits, ServiceError, Settings, SignupOutcome,
    },
    credentials::KdfPool,
    domain::{
        admission::{AccountState, Purpose},
        ids::OperationId,
        mailbox_proof::{ProofId, ProofState},
    },
    mail::SignupLinkBase,
    persistence::{IdentityStore, ProofCommandId},
};
use cantos_store_turso::{
    registry::Intent,
    testing::{FixedClock, TestRun},
};
use support::{operation, Behavior, CaptureMailer, ParsedLink};

const NOW: i64 = 1_700_000_000;
const OWNER: &str = "owner@example.com";
const GOOD: &str = "a correct horse battery";

struct Harness {
    run: TestRun,
    clock: Arc<FixedClock>,
    store: Arc<IdentityStore>,
    mail: Arc<CaptureMailer>,
    service: Arc<IdentityService>,
}

fn ip(text: &str) -> ClientAddress {
    ClientAddress(text.parse::<IpAddr>().unwrap())
}

fn command(value: u128) -> ProofCommandId {
    ProofCommandId::from_bytes(value.to_be_bytes()).unwrap()
}

fn set_time(clock: &FixedClock, seconds: i64) {
    clock.set(seconds * 1_000_000);
}

fn build(
    store: &Arc<IdentityStore>,
    clock: &Arc<FixedClock>,
    mail: &Arc<CaptureMailer>,
    kdf: KdfPool,
) -> Arc<IdentityService> {
    Arc::new(
        IdentityService::new(
            Arc::clone(store),
            clock.clone(),
            mail.clone(),
            Settings {
                link: SignupLinkBase::parse("https://theatre.example/signup").unwrap(),
                kdf,
                limits: Limits::standard(),
            },
        )
        .unwrap(),
    )
}

fn pool() -> KdfPool {
    KdfPool::new(2, 8, Duration::from_secs(30))
}

async fn harness() -> Harness {
    let run = TestRun::new("identity-application");
    let clock = FixedClock::new(NOW * 1_000_000);
    let store = Arc::new(
        IdentityStore::open(run.path("identity.db"), Intent::Migrate, clock.clone())
            .await
            .unwrap(),
    );
    let mail = CaptureMailer::new();
    let service = build(&store, &clock, &mail, pool());
    Harness {
        run,
        clock,
        store,
        mail,
        service,
    }
}

impl Harness {
    async fn start(&self, op: u128, email: &str) -> Result<Dispatch, ServiceError> {
        self.service
            .start_theatre_signup(operation(op), email, ip("198.51.100.1"))
            .await
            .map(|started| started.dispatch)
    }

    async fn complete(
        &self,
        cmd: u128,
        link: &ParsedLink,
        password: &str,
    ) -> Result<SignupOutcome, ServiceError> {
        self.service
            .complete_theatre_signup(
                command(cmd),
                OperationId::from_bytes(link.operation).unwrap(),
                ProofId::from_bytes(link.proof).unwrap(),
                link.secret,
                password,
                true,
                ip("203.0.113.5"),
            )
            .await
    }

    async fn login(
        &self,
        op: u128,
        email: &str,
        password: &str,
        from: &str,
    ) -> Result<(), ServiceError> {
        self.service
            .authenticate_password(operation(op), Purpose::Theatre, email, password, ip(from))
            .await
            .map(|_| ())
    }

    /// Start, read the mail, complete: the only way these tests create an account.
    async fn register(&self, op: u128, cmd: u128, email: &str, password: &str) -> SignupOutcome {
        self.start(op, email).await.unwrap();
        let (to, link) = self.mail.last();
        assert_eq!(to, email);
        self.complete(cmd, &link, password).await.unwrap()
    }

    fn all_file_bytes(&self) -> Vec<Vec<u8>> {
        std::fs::read_dir(self.run.dir())
            .unwrap()
            .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
            .collect()
    }
}

fn contains(haystack: &[u8], needle: &[u8]) -> bool {
    haystack
        .windows(needle.len())
        .any(|window| window == needle)
}

#[tokio::test]
async fn signup_then_login_survives_a_restart_with_a_stable_subject() {
    let h = harness().await;
    assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::TransportAccepted));
    assert_eq!(h.mail.recipients(), [OWNER]);
    let (_, link) = h.mail.last();
    let outcome = h.complete(100, &link, GOOD).await.unwrap();
    let SignupOutcome::Completed { subject, epoch } = outcome else {
        panic!("expected a fresh completion, got {outcome:?}")
    };
    assert_eq!(epoch, h.service.epoch());

    let fact = h
        .service
        .authenticate_password(
            operation(7),
            Purpose::Theatre,
            OWNER,
            GOOD,
            ip("203.0.113.5"),
        )
        .await
        .unwrap();
    assert_eq!(fact.subject, subject);
    assert_eq!(fact.purpose, Purpose::Theatre);
    assert_eq!(fact.epoch, epoch);
    assert_eq!(fact.operation, operation(7));
    assert_eq!(fact.security_version.get(), 1);
    assert_eq!(fact.authenticated_at_micros, NOW * 1_000_000);

    // Stop everything and reopen the same file in serve mode.
    let (run, clock, mail) = (h.run, h.clock, h.mail);
    drop(h.service);
    drop(h.store);
    let store = Arc::new(
        IdentityStore::open(run.path("identity.db"), Intent::Serve, clock.clone())
            .await
            .unwrap(),
    );
    assert_eq!(
        store.epoch(),
        epoch,
        "ordinary reopen keeps the instance epoch"
    );
    let service = build(&store, &clock, &mail, pool());
    let again = service
        .authenticate_password(
            operation(8),
            Purpose::Theatre,
            OWNER,
            GOOD,
            ip("203.0.113.5"),
        )
        .await
        .unwrap();
    assert_eq!(again.subject, subject);
    assert_eq!(again.operation, operation(8));
    // Neither the password nor a proof secret was ever written to the database files.
    for bytes in std::fs::read_dir(run.dir())
        .unwrap()
        .map(|entry| std::fs::read(entry.unwrap().path()).unwrap())
    {
        assert!(!contains(&bytes, GOOD.as_bytes()));
        assert!(!contains(&bytes, &link.secret));
        assert!(!contains(&bytes, support::hex(&link.secret).as_bytes()));
    }
}

#[tokio::test]
async fn an_initiator_gets_no_proof_and_cannot_set_the_password_of_the_mailbox_owner() {
    let h = harness().await;
    // "A" starts a signup for B's address. The start carries no password and returns no proof.
    assert_eq!(
        h.start(1, "victim@example.com").await,
        Ok(Dispatch::TransportAccepted)
    );
    assert_eq!(h.mail.recipients(), ["victim@example.com"]);
    let (_, link) = h.mail.last();
    // A knows the operation id it chose, but not the proof locator or secret.
    let guess = ParsedLink {
        operation: link.operation,
        proof: [9; 16],
        secret: [9; 32],
    };
    assert_eq!(
        h.complete(100, &guess, "attacker password 1").await,
        Err(ServiceError::Denied)
    );
    let right_locator = ParsedLink {
        secret: [9; 32],
        ..link.clone()
    };
    assert_eq!(
        h.complete(101, &right_locator, "attacker password 1").await,
        Err(ServiceError::Denied)
    );
    // B holds the mailed link and chooses their own password.
    h.complete(102, &link, "the victim chose this")
        .await
        .unwrap();
    assert_eq!(
        h.login(
            1,
            "victim@example.com",
            "attacker password 1",
            "198.51.100.9"
        )
        .await,
        Err(ServiceError::Denied)
    );
    h.login(
        2,
        "victim@example.com",
        "the victim chose this",
        "198.51.100.9",
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn wrong_expired_cancelled_and_replaced_proofs_are_refused_and_create_nothing() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, link) = h.mail.last();
    let mut wrong = link.clone();
    wrong.secret[0] ^= 1;
    assert_eq!(
        h.complete(100, &wrong, GOOD).await,
        Err(ServiceError::Denied)
    );
    // Resending inside the cooldown mails nothing and keeps the first proof.
    assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::NotAttempted));
    assert_eq!(h.mail.count(), 1);
    // After the cooldown the same operation is re-mailed with a NEW proof; the old one dies.
    set_time(&h.clock, NOW + 61);
    assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::TransportAccepted));
    assert_eq!(h.mail.count(), 2);
    let (_, fresh) = h.mail.last();
    assert_ne!(fresh.proof, link.proof);
    assert_ne!(fresh.secret, link.secret);
    assert_eq!(
        h.complete(101, &link, GOOD).await,
        Err(ServiceError::Denied)
    );
    // The new proof expires exactly at its TTL.
    set_time(&h.clock, NOW + 61 + 900);
    assert_eq!(
        h.complete(102, &fresh, GOOD).await,
        Err(ServiceError::Denied)
    );
    // A different mailbox cannot reuse the operation, and the answer is the generic one.
    let mails = h.mail.count();
    assert_eq!(
        h.start(1, "other@example.com").await,
        Ok(Dispatch::NotAttempted)
    );
    assert_eq!(h.mail.count(), mails);
    // A cancelled proof is terminal.
    h.start(2, "cancelled@example.com").await.unwrap();
    let (_, cancelled) = h.mail.last();
    let record = h
        .store
        .signup(OperationId::from_bytes(cancelled.operation).unwrap())
        .await
        .unwrap()
        .unwrap();
    h.store
        .cancel_mailbox_proof(command(900), record.proof.binding(), record.proof.id())
        .await
        .unwrap();
    assert_eq!(
        h.complete(103, &cancelled, GOOD).await,
        Err(ServiceError::Denied)
    );
    for email in [OWNER, "cancelled@example.com"] {
        assert_eq!(
            h.login(5, email, GOOD, "198.51.100.9").await,
            Err(ServiceError::Denied)
        );
    }
}

#[tokio::test]
async fn polling_state_does_not_consume_the_proof() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, link) = h.mail.last();
    let workflow = OperationId::from_bytes(link.operation).unwrap();
    for _ in 0..2 {
        // What a status poll or a link scanner can observe is storage reads and repeated starts.
        assert_eq!(
            h.store
                .signup(workflow)
                .await
                .unwrap()
                .unwrap()
                .proof
                .state(),
            ProofState::Pending
        );
        assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::NotAttempted));
    }
    h.complete(100, &link, GOOD).await.unwrap();
}

#[tokio::test]
async fn an_unconfirmed_or_too_short_password_consumes_nothing() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, link) = h.mail.last();
    let call = |password: &'static str, explicit: bool| {
        let service = Arc::clone(&h.service);
        let link = link.clone();
        async move {
            service
                .complete_theatre_signup(
                    command(100),
                    OperationId::from_bytes(link.operation).unwrap(),
                    ProofId::from_bytes(link.proof).unwrap(),
                    link.secret,
                    password,
                    explicit,
                    ip("203.0.113.5"),
                )
                .await
        }
    };
    assert_eq!(call(GOOD, false).await, Err(ServiceError::InvalidRequest));
    assert_eq!(
        call("short", true).await,
        Err(ServiceError::InvalidPassword)
    );
    assert_eq!(
        h.store
            .signup(OperationId::from_bytes(link.operation).unwrap())
            .await
            .unwrap()
            .unwrap()
            .proof
            .state(),
        ProofState::Pending
    );
    h.complete(100, &link, GOOD).await.unwrap();
}

#[tokio::test]
async fn an_existing_account_is_never_reset_linked_or_mailed_by_a_new_signup() {
    let h = harness().await;
    let SignupOutcome::Completed { subject, .. } = h.register(1, 100, OWNER, GOOD).await else {
        panic!("first signup completes")
    };
    // Another party starts a signup for the same address: same generic answer, no mail.
    assert_eq!(h.start(2, OWNER).await, Ok(Dispatch::NotAttempted));
    assert_eq!(h.mail.count(), 1);
    h.login(1, OWNER, GOOD, "198.51.100.9").await.unwrap();
    assert_eq!(
        h.login(2, OWNER, "an attacker supplied one", "198.51.100.9")
            .await,
        Err(ServiceError::Denied)
    );
    let state = h.service.security_state(subject).await.unwrap();
    assert_eq!(
        (state.state, state.version.get()),
        (AccountState::Active, 1)
    );
}

#[tokio::test]
async fn two_proofs_for_one_address_create_one_account_and_the_loser_is_denied() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, first) = h.mail.last();
    h.start(2, OWNER).await.unwrap();
    let (_, second) = h.mail.last();
    let (a, b) = tokio::join!(
        h.complete(100, &first, "the first password!"),
        h.complete(101, &second, "the second password")
    );
    let (winner, loser) = match (a, b) {
        (Ok(_), Err(error)) => ("the first password!", error),
        (Err(error), Ok(_)) => ("the second password", error),
        other => panic!("expected exactly one winner, got {other:?}"),
    };
    assert_eq!(loser, ServiceError::Denied);
    h.login(1, OWNER, winner, "198.51.100.9").await.unwrap();
}

#[tokio::test]
async fn concurrent_completions_of_one_link_have_one_winner_whose_password_is_the_only_one() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, link) = h.mail.last();
    let passwords: Vec<String> = (0..4).map(|n| format!("concurrent password {n}")).collect();
    let attempts: Vec<_> = passwords
        .iter()
        .enumerate()
        .map(|(n, password)| {
            let (service, link, password) =
                (Arc::clone(&h.service), link.clone(), password.clone());
            tokio::spawn(async move {
                service
                    .complete_theatre_signup(
                        command(200 + n as u128),
                        OperationId::from_bytes(link.operation).unwrap(),
                        ProofId::from_bytes(link.proof).unwrap(),
                        link.secret,
                        &password,
                        true,
                        ip("203.0.113.5"),
                    )
                    .await
            })
        })
        .collect();
    let mut winners = Vec::new();
    for (n, attempt) in attempts.into_iter().enumerate() {
        match attempt.await.unwrap() {
            Ok(SignupOutcome::Completed { .. }) => winners.push(n),
            Ok(other) => panic!("unexpected {other:?}"),
            Err(error) => assert_eq!(error, ServiceError::Denied),
        }
    }
    assert_eq!(winners.len(), 1);
    for (n, password) in passwords.iter().enumerate() {
        let result = h
            .login(
                10 + n as u128,
                OWNER,
                password,
                &format!("198.51.100.{}", 10 + n),
            )
            .await;
        assert_eq!(result.is_ok(), n == winners[0], "password {n}");
    }
}

#[tokio::test]
async fn a_lost_response_is_reconciled_by_the_identical_command_and_never_by_a_changed_one() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, link) = h.mail.last();
    let SignupOutcome::Completed { subject, .. } = h.complete(100, &link, GOOD).await.unwrap()
    else {
        panic!("completes")
    };
    // The client never saw the answer and retries the identical command.
    assert_eq!(
        h.complete(100, &link, GOOD).await,
        Ok(SignupOutcome::Replayed {
            subject,
            epoch: h.service.epoch()
        })
    );
    // The same command with a different password is not a replay and changes nothing.
    assert_eq!(
        h.complete(100, &link, "a different password").await,
        Err(ServiceError::Denied)
    );
    // A new command cannot reuse the consumed proof.
    assert_eq!(
        h.complete(101, &link, GOOD).await,
        Err(ServiceError::Denied)
    );
    h.login(1, OWNER, GOOD, "198.51.100.9").await.unwrap();
    assert_eq!(
        h.login(2, OWNER, "a different password", "198.51.100.9")
            .await,
        Err(ServiceError::Denied)
    );
    let events = h.store.events(0, 10).await.unwrap();
    assert_eq!(events.len(), 1, "replays add no security event");
}

#[tokio::test]
async fn every_authentication_failure_is_one_indistinguishable_denial() {
    let h = harness().await;
    let SignupOutcome::Completed { subject, .. } = h.register(1, 100, OWNER, GOOD).await else {
        panic!("completes")
    };
    h.store
        .disable_subject(
            operation(50),
            subject,
            h.service.security_state(subject).await.unwrap().version,
            h.service.epoch(),
        )
        .await
        .unwrap();
    h.register(2, 101, "second@example.com", "the second one!!")
        .await;
    for (email, password) in [
        ("nobody@example.com", GOOD), // unknown account
        ("second@example.com", GOOD), // wrong password
        (OWNER, GOOD),                // right password, disabled account
        (OWNER, "not the password"),  // disabled and wrong
    ] {
        assert_eq!(
            h.login(9, email, password, "198.51.100.9").await,
            Err(ServiceError::Denied),
            "{email}"
        );
    }
    h.login(10, "second@example.com", "the second one!!", "198.51.100.9")
        .await
        .unwrap();
    let state = h.service.security_state(subject).await.unwrap();
    assert_eq!(state.state, AccountState::Disabled);
}

#[tokio::test]
async fn studio_is_never_authenticated_with_a_password_and_bad_input_is_refused() {
    let h = harness().await;
    h.register(1, 100, OWNER, GOOD).await;
    assert_eq!(
        h.service
            .authenticate_password(
                operation(1),
                Purpose::Studio,
                OWNER,
                GOOD,
                ip("198.51.100.9")
            )
            .await,
        Err(ServiceError::Unsupported)
    );
    for (email, password) in [
        ("not an email", GOOD),
        (OWNER, ""),
        (OWNER, &"x".repeat(513)[..]),
    ] {
        assert_eq!(
            h.login(1, email, password, "198.51.100.9").await,
            Err(ServiceError::InvalidRequest)
        );
    }
}

#[tokio::test]
async fn login_failures_throttle_one_pair_temporarily_without_locking_the_account() {
    let h = harness().await;
    h.register(1, 100, OWNER, GOOD).await;
    for n in 1..=5 {
        assert_eq!(
            h.login(n, OWNER, "guess number wrong", "198.51.100.66")
                .await,
            Err(ServiceError::Denied)
        );
    }
    // The attacker's address is now throttled even with the right password...
    assert!(matches!(
        h.login(20, OWNER, GOOD, "198.51.100.66").await,
        Err(ServiceError::Throttled(_))
    ));
    // ...but the real owner, from elsewhere, is not locked out.
    h.login(21, OWNER, GOOD, "203.0.113.77").await.unwrap();
    // And the throttle is temporary.
    set_time(&h.clock, NOW + 901);
    h.login(22, OWNER, GOOD, "198.51.100.66").await.unwrap();
}

#[tokio::test]
async fn signup_starts_are_budgeted_per_mailbox_and_per_client_address() {
    let h = harness().await;
    for op in 1..=3 {
        h.service
            .start_theatre_signup(operation(op), "budget@example.com", ip("198.51.100.1"))
            .await
            .unwrap();
    }
    assert!(matches!(
        h.service
            .start_theatre_signup(operation(4), "budget@example.com", ip("198.51.100.2"))
            .await,
        Err(ServiceError::Throttled(_))
    ));
    for op in 0..20_u128 {
        h.service
            .start_theatre_signup(
                operation(100 + op),
                &format!("u{op}@example.com"),
                ip("198.51.100.3"),
            )
            .await
            .unwrap();
    }
    assert!(matches!(
        h.service
            .start_theatre_signup(operation(200), "late@example.com", ip("198.51.100.3"))
            .await,
        Err(ServiceError::Throttled(_))
    ));
}

#[tokio::test]
async fn mail_failures_fail_closed_and_a_refused_message_can_be_re_sent_for_the_same_operation() {
    let h = harness().await;
    // A relay refusal: nothing was sent, the proof stays pending but unreachable, and the answer
    // is the same generic one (no per-recipient outage signal).
    h.mail.set(Behavior::Reject);
    assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::TransportRefused));
    assert_eq!(h.mail.count(), 0);
    let record = h.store.signup(operation(1)).await.unwrap().unwrap();
    assert_eq!(record.proof.state(), ProofState::Pending);
    // Inside the cooldown nothing is retried; after it the same operation is re-mailed with a
    // replacement proof and completes.
    h.mail.set(Behavior::Deliver);
    assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::NotAttempted));
    set_time(&h.clock, NOW + 61);
    assert_eq!(h.start(1, OWNER).await, Ok(Dispatch::TransportAccepted));
    let (_, link) = h.mail.last();
    h.complete(100, &link, GOOD).await.unwrap();
    // An uncertain relay answer leaves the proof pending: the message may exist.
    h.mail.set(Behavior::Uncertain);
    assert_eq!(
        h.start(2, "maybe@example.com").await,
        Ok(Dispatch::TransportUncertain)
    );
    let (_, maybe) = h.mail.last();
    h.complete(101, &maybe, GOOD).await.unwrap();
}

#[tokio::test]
async fn a_start_answers_the_same_for_a_registered_and_an_unregistered_address() {
    let h = harness().await;
    h.register(1, 100, OWNER, GOOD).await;
    // An operation bound to the attacker's own mailbox, replayed for two different targets:
    // the registered address and a fresh one must be indistinguishable.
    h.start(2, "mine@example.com").await.unwrap();
    let mails = h.mail.count();
    let registered = h.start(2, OWNER).await;
    let unregistered = h.start(2, "fresh@example.com").await;
    assert_eq!(registered, Ok(Dispatch::NotAttempted));
    assert_eq!(registered, unregistered);
    // A concluded operation is answered the same way too.
    let (_, mine) = h.mail.last();
    h.complete(101, &mine, GOOD).await.unwrap();
    assert_eq!(
        h.start(2, "mine@example.com").await,
        Ok(Dispatch::NotAttempted)
    );
    assert_eq!(h.start(2, OWNER).await, Ok(Dispatch::NotAttempted));
    assert_eq!(h.mail.count(), mails);
}

#[tokio::test]
async fn concurrent_identical_commands_never_replay_a_different_password() {
    let h = harness().await;
    h.start(1, OWNER).await.unwrap();
    let (_, link) = h.mail.last();
    let attempts: Vec<_> = ["first password chosen", "second password chosen"]
        .into_iter()
        .map(|password| {
            let (service, link) = (Arc::clone(&h.service), link.clone());
            tokio::spawn(async move {
                service
                    .complete_theatre_signup(
                        command(300),
                        OperationId::from_bytes(link.operation).unwrap(),
                        ProofId::from_bytes(link.proof).unwrap(),
                        link.secret,
                        password,
                        true,
                        ip("203.0.113.5"),
                    )
                    .await
            })
        })
        .collect();
    let mut results = Vec::new();
    for attempt in attempts {
        results.push(attempt.await.unwrap());
    }
    // The identical command can win once; the other caller (different password) must not be
    // told it succeeded, whichever of them reached the transaction first.
    assert_eq!(
        results.iter().filter(|r| r.is_ok()).count(),
        1,
        "{results:?}"
    );
    assert!(results.contains(&Err(ServiceError::Denied)), "{results:?}");
    let (winner, loser) = if results[0].is_ok() {
        ("first password chosen", "second password chosen")
    } else {
        ("second password chosen", "first password chosen")
    };
    h.login(1, OWNER, winner, "198.51.100.9").await.unwrap();
    assert_eq!(
        h.login(2, OWNER, loser, "198.51.100.9").await,
        Err(ServiceError::Denied)
    );
}

#[tokio::test]
async fn kdf_saturation_refuses_with_busy_and_releases_all_capacity() {
    let h = harness().await;
    h.register(1, 100, OWNER, GOOD).await;
    let service = build(
        &h.store,
        &h.clock,
        &h.mail,
        KdfPool::new(1, 0, Duration::from_millis(1)),
    );
    let tasks: Vec<_> = (0..12_u128)
        .map(|n| {
            let service = Arc::clone(&service);
            tokio::spawn(async move {
                service
                    .authenticate_password(
                        operation(n + 1),
                        Purpose::Theatre,
                        OWNER,
                        GOOD,
                        ip(&format!("203.0.113.{}", n + 1)),
                    )
                    .await
                    .map(|_| ())
            })
        })
        .collect();
    let mut results = Vec::new();
    for task in tasks {
        results.push(task.await.unwrap());
    }
    assert!(
        results.contains(&Ok(())),
        "at least one request got a worker"
    );
    assert!(
        results.contains(&Err(ServiceError::Busy)),
        "the rest were refused: {results:?}"
    );
    assert!(results
        .iter()
        .all(|r| matches!(r, Ok(()) | Err(ServiceError::Busy))));
    // Capacity is fully released: a later request succeeds.
    service
        .authenticate_password(
            operation(99),
            Purpose::Theatre,
            OWNER,
            GOOD,
            ip("203.0.113.200"),
        )
        .await
        .unwrap();
}

#[tokio::test]
async fn files_hold_no_plaintext_password_after_many_operations() {
    let h = harness().await;
    h.register(1, 100, OWNER, "plaintext must not persist")
        .await;
    h.login(1, OWNER, "plaintext must not persist", "198.51.100.9")
        .await
        .unwrap();
    for bytes in h.all_file_bytes() {
        assert!(!contains(&bytes, b"plaintext must not persist"));
    }
}
