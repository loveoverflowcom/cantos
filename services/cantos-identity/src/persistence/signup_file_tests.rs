//! Real pinned-engine tests for signup and credential storage. The PHC strings here are
//! shape-only fixtures: these tests prove atomicity, binding and race behavior, never password
//! hashing (see `credentials`) or the RPC boundary.

use std::sync::Arc;

use cantos_store_turso::error::{GuardCode, StoreFault};
use cantos_store_turso::registry::{DatabaseKind, Intent, Registry};
use cantos_store_turso::testing::{FixedClock, TestRun};
use cantos_store_turso::{OpenRequest, OwnedDatabase, Value};

use super::{
    IdentityStore, LoginFinalization, PhcString, ProofCommandId, ProofMaterial, ProofStoreError,
    SignupCompletion, SignupCompletionRequest, SignupStoreError,
};
use crate::domain::admission::{AccountState, CompletionDenied, Purpose, RequestBinding};
use crate::domain::email::EmailAddress;
use crate::domain::ids::{InstanceEpoch, OperationId, SubjectId};
use crate::domain::mailbox_proof::{ProofDenied, ProofState, ProofTime};
use crate::domain::security_version::SecurityVersion;

const NOW: u64 = 1_700_000_000;
const PHC: &str = "$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g";

fn command(value: u128) -> ProofCommandId {
    ProofCommandId::from_bytes(value.to_be_bytes()).unwrap()
}

fn operation(value: u128) -> OperationId {
    OperationId::from_bytes(value.to_be_bytes()).unwrap()
}

fn subject(value: u128) -> SubjectId {
    SubjectId::from_bytes(value.to_be_bytes()).unwrap()
}

fn time(seconds: u64) -> ProofTime {
    ProofTime::new(seconds).unwrap()
}

fn email(text: &str) -> EmailAddress {
    EmailAddress::parse(text).unwrap()
}

fn phc() -> PhcString {
    PhcString::new(PHC.to_owned()).unwrap()
}

fn set_time(clock: &FixedClock, seconds: u64) {
    clock.set(i64::try_from(seconds * 1_000_000).unwrap());
}

fn guard(code: &str) -> StoreFault {
    StoreFault::Guard(GuardCode::new(code).unwrap())
}

async fn create(run: &TestRun, clock: Arc<FixedClock>) -> IdentityStore {
    IdentityStore::open(run.path("signup.db"), Intent::Migrate, clock)
        .await
        .unwrap()
}

async fn reopen(run: &TestRun, clock: Arc<FixedClock>) -> IdentityStore {
    IdentityStore::open(run.path("signup.db"), Intent::Serve, clock)
        .await
        .unwrap()
}

fn fixture() -> (TestRun, Arc<FixedClock>) {
    let run = TestRun::new("identity-signup");
    let clock = FixedClock::new((NOW * 1_000_000) as i64);
    (run, clock)
}

async fn count(store: &IdentityStore, sql: &str) -> i64 {
    let reader = store.database.reader().await.unwrap();
    let mut rows = reader.query(sql, ()).await.unwrap();
    let row = rows.next().await.unwrap().unwrap();
    let Value::Integer(value) = row.get_value(0).unwrap() else {
        panic!("expected an INTEGER count")
    };
    value
}

/// subjects, events, emails, credentials, completions, proof issues, proof receipts.
async fn counts(store: &IdentityStore) -> [i64; 7] {
    let mut out = [0; 7];
    for (slot, table) in out.iter_mut().zip([
        "identity_subjects",
        "security_events",
        "identity_emails",
        "password_credentials",
        "signup_completions",
        "mailbox_proof_issues",
        "mailbox_proof_receipts",
    ]) {
        *slot = count(store, &format!("SELECT COUNT(*) FROM {table}")).await;
    }
    out
}

async fn run_sql(store: &IdentityStore, sql: &str) {
    let tx = store.database.begin_write().await.unwrap();
    tx.execute(sql, ()).await.unwrap();
    tx.commit().await.unwrap();
}

async fn begin(store: &IdentityStore, op: u128, cmd: u128, mailbox: &str) -> ProofMaterial {
    let material = ProofMaterial::generate().unwrap();
    store
        .begin_signup(
            command(cmd),
            operation(op),
            &email(mailbox),
            &material,
            time(NOW + 900),
        )
        .await
        .unwrap();
    material
}

async fn complete(
    store: &IdentityStore,
    cmd: u128,
    op: u128,
    material: &ProofMaterial,
    who: u128,
) -> Result<SignupCompletion, SignupStoreError> {
    let phc = phc();
    store
        .complete_signup(&SignupCompletionRequest {
            command: command(cmd),
            operation: operation(op),
            proof: material.id(),
            secret: material.secret(),
            phc: &phc,
            subject: subject(who),
        })
        .await
}

fn denied(error: &SignupStoreError) -> bool {
    matches!(
        error,
        SignupStoreError::Proof(ProofStoreError::Denied(_) | ProofStoreError::InvalidSecret)
    )
}

#[tokio::test]
async fn a_pending_signup_binds_its_mailbox_with_the_first_proof_digest_only() {
    let (run, clock) = fixture();
    let store = create(&run, clock.clone()).await;
    let material = begin(&store, 1, 100, "Owner@Example.com").await;
    let record = store.signup(operation(1)).await.unwrap().unwrap();
    assert_eq!(record.email, email("Owner@example.com"));
    assert_eq!(record.proof.state(), ProofState::Pending);
    assert_eq!(record.proof.id(), material.id());
    assert!(!store.email_registered(&record.email).await.unwrap());
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 1]);
    assert_eq!(store.signup(operation(2)).await.unwrap(), None);
    drop(store);
    // Nothing in any file carries the secret itself: only its domain-separated digest.
    for entry in std::fs::read_dir(run.dir()).unwrap() {
        let bytes = std::fs::read(entry.unwrap().path()).unwrap();
        assert!(
            !bytes
                .windows(32)
                .any(|window| window == material.secret().expose().as_slice()),
            "proof secret reached storage"
        );
    }
    let reopened = reopen(&run, clock).await;
    assert_eq!(
        reopened.signup(operation(1)).await.unwrap().unwrap().email,
        record.email
    );
}

#[tokio::test]
async fn the_mailbox_of_an_operation_cannot_change_and_an_exact_retry_replays() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    // The identical command returns the original receipt and adds nothing.
    let replay = store
        .begin_signup(
            command(100),
            operation(1),
            &email("owner@example.com"),
            &material,
            time(NOW + 900),
        )
        .await
        .unwrap();
    assert_eq!(replay.proof.id(), material.id());
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 1]);
    // A different mailbox needs a different operation: the same one is refused, with no rows.
    let other = ProofMaterial::generate().unwrap();
    let changed = store
        .begin_signup(
            command(101),
            operation(1),
            &email("someone-else@example.com"),
            &other,
            time(NOW + 900),
        )
        .await
        .unwrap_err();
    assert!(
        matches!(changed, SignupStoreError::Proof(ProofStoreError::Exists)),
        "{changed:?}"
    );
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 1]);
    assert_eq!(
        store.signup(operation(1)).await.unwrap().unwrap().email,
        email("owner@example.com")
    );
}

#[tokio::test]
async fn begin_signup_commits_the_proof_and_the_binding_together_or_neither() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    run_sql(
        &store,
        "CREATE TRIGGER fail_binding BEFORE INSERT ON signup_workflows BEGIN SELECT RAISE(ABORT, 'cantos:test.binding'); END",
    )
    .await;
    let material = ProofMaterial::generate().unwrap();
    let error = store
        .begin_signup(
            command(1),
            operation(1),
            &email("a@example.com"),
            &material,
            time(NOW + 900),
        )
        .await
        .unwrap_err();
    assert_eq!(error, SignupStoreError::Storage(guard("test.binding")));
    // No orphan workflow, issue or receipt survived without its mailbox binding.
    assert_eq!(counts(&store).await, [0; 7]);
    assert_eq!(
        count(&store, "SELECT COUNT(*) FROM mailbox_proof_workflows").await,
        0
    );
    run_sql(&store, "DROP TRIGGER fail_binding").await;
    store
        .begin_signup(
            command(1),
            operation(1),
            &email("a@example.com"),
            &material,
            time(NOW + 900),
        )
        .await
        .unwrap();
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 1]);
}

#[tokio::test]
async fn completion_creates_every_effect_together_and_an_identical_replay_adds_none() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    let first = complete(&store, 200, 1, &material, 7).await.unwrap();
    assert_eq!(
        first,
        SignupCompletion::Completed {
            subject: subject(7),
            epoch: store.epoch()
        }
    );
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
    assert_eq!(
        store
            .signup(operation(1))
            .await
            .unwrap()
            .unwrap()
            .proof
            .state(),
        ProofState::Consumed
    );
    assert!(store
        .email_registered(&email("owner@example.com"))
        .await
        .unwrap());
    let stored = store.subject(subject(7)).await.unwrap().unwrap();
    assert_eq!(
        (stored.state, stored.version.get()),
        (AccountState::Active, 1)
    );
    // The security event and its receipt use the signup operation as their idempotency key.
    assert_eq!(store.events(0, 5).await.unwrap().len(), 1);
    assert_eq!(
        store.receipt(operation(1)).await.unwrap().unwrap().subject,
        subject(7)
    );
    let (workflow, who, _) = store
        .completion_for_command(command(200))
        .await
        .unwrap()
        .unwrap();
    assert_eq!((workflow, who), (operation(1), subject(7)));

    let replay = complete(&store, 200, 1, &material, 99).await.unwrap();
    assert_eq!(
        replay,
        SignupCompletion::Replayed {
            subject: subject(7),
            epoch: store.epoch(),
            credential: phc()
        },
        "a replay returns the original subject, never the freshly proposed one"
    );
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
    assert!(store.subject(subject(99)).await.unwrap().is_none());
}

#[tokio::test]
async fn a_replayed_command_with_changed_content_is_refused() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    complete(&store, 200, 1, &material, 7).await.unwrap();
    let other = ProofMaterial::generate().unwrap();
    // Same command, different secret.
    let error = complete(&store, 200, 1, &other, 8).await.unwrap_err();
    assert_eq!(
        error,
        SignupStoreError::Proof(ProofStoreError::OperationReused)
    );
    // Same command aimed at another operation: still the original completion's, so refused.
    let second = begin(&store, 2, 101, "second@example.com").await;
    let error = complete(&store, 200, 2, &second, 9).await.unwrap_err();
    assert!(
        matches!(
            error,
            SignupStoreError::Proof(ProofStoreError::OperationReused)
        ),
        "{error:?}"
    );
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 2, 3]);
    assert!(store.subject(subject(8)).await.unwrap().is_none());
    assert!(store.subject(subject(9)).await.unwrap().is_none());
}

#[tokio::test]
async fn a_failure_in_the_final_write_rolls_back_the_consumed_proof_and_the_account() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    run_sql(
        &store,
        "CREATE TRIGGER fail_completion BEFORE INSERT ON signup_completions BEGIN SELECT RAISE(ABORT, 'cantos:test.completion'); END",
    )
    .await;
    let error = complete(&store, 200, 1, &material, 7).await.unwrap_err();
    assert_eq!(error, SignupStoreError::Storage(guard("test.completion")));
    // Nothing half-created: no subject, event, email, credential or consume receipt, and the
    // proof is still pending and usable.
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 1]);
    assert_eq!(
        store
            .signup(operation(1))
            .await
            .unwrap()
            .unwrap()
            .proof
            .state(),
        ProofState::Pending
    );
    run_sql(&store, "DROP TRIGGER fail_completion").await;
    complete(&store, 201, 1, &material, 7).await.unwrap();
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
}

#[tokio::test]
async fn wrong_expired_cancelled_and_replaced_proofs_never_create_anything() {
    let (run, clock) = fixture();
    let store = create(&run, clock.clone()).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    let stranger = ProofMaterial::generate().unwrap();
    // Wrong secret under the right locator, and a wrong locator.
    let mut flipped = *material.secret().expose();
    flipped[0] ^= 1;
    let phc = phc();
    let wrong_secret = store
        .complete_signup(&SignupCompletionRequest {
            command: command(200),
            operation: operation(1),
            proof: material.id(),
            secret: &super::ProofSecret::from_bytes(flipped),
            phc: &phc,
            subject: subject(7),
        })
        .await
        .unwrap_err();
    assert_eq!(
        wrong_secret,
        SignupStoreError::Proof(ProofStoreError::InvalidSecret)
    );
    let wrong_locator = complete(&store, 201, 1, &stranger, 7).await.unwrap_err();
    assert!(denied(&wrong_locator), "{wrong_locator:?}");
    // Unknown operation.
    assert_eq!(
        complete(&store, 202, 5, &material, 7).await.unwrap_err(),
        SignupStoreError::WorkflowMissing
    );
    // Exactly at expiry the proof is dead.
    set_time(&clock, NOW + 900);
    assert_eq!(
        complete(&store, 203, 1, &material, 7).await.unwrap_err(),
        SignupStoreError::Proof(ProofStoreError::Denied(ProofDenied::Expired))
    );
    // Replacement (after cooldown) retires the first locator and secret.
    set_time(&clock, NOW + 120);
    let fresh = ProofMaterial::generate().unwrap();
    let current = store.signup(operation(1)).await.unwrap().unwrap().proof;
    store
        .replace_mailbox_proof(
            command(300),
            current.binding(),
            current.id(),
            &fresh,
            time(NOW + 1_020),
            60,
        )
        .await
        .unwrap();
    assert!(denied(
        &complete(&store, 204, 1, &material, 7).await.unwrap_err()
    ));
    // A cancelled proof is terminal.
    store
        .cancel_mailbox_proof(command(301), current.binding(), fresh.id())
        .await
        .unwrap();
    assert!(denied(
        &complete(&store, 205, 1, &fresh, 7).await.unwrap_err()
    ));
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 2, 3]);
}

#[tokio::test]
async fn a_standalone_consumption_receipt_cannot_be_replayed_into_an_account() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    let binding = store
        .signup(operation(1))
        .await
        .unwrap()
        .unwrap()
        .proof
        .binding();
    // The storage primitive consumes the proof on its own under command 200 ...
    store
        .confirm_mailbox_proof(
            command(200),
            binding,
            material.id(),
            material.secret(),
            crate::domain::mailbox_proof::ProofAction::ExplicitConfirmation,
        )
        .await
        .unwrap();
    // ... and that historical receipt must not stand in for a fresh consumption.
    let error = complete(&store, 200, 1, &material, 7).await.unwrap_err();
    assert_eq!(
        error,
        SignupStoreError::Proof(ProofStoreError::Denied(ProofDenied::Consumed))
    );
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 2]);
    assert!(store.subject(subject(7)).await.unwrap().is_none());
}

#[tokio::test]
async fn reading_and_polling_state_never_consumes_a_pending_proof() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    for _ in 0..5 {
        let record = store.signup(operation(1)).await.unwrap().unwrap();
        assert_eq!(record.proof.state(), ProofState::Pending);
        assert!(!store.email_registered(&record.email).await.unwrap());
        assert!(store
            .proof_secret_matches(operation(1), material.id(), material.secret())
            .await
            .unwrap());
        assert!(store
            .completion_for_command(command(200))
            .await
            .unwrap()
            .is_none());
    }
    assert_eq!(counts(&store).await, [0, 0, 0, 0, 0, 1, 1]);
    complete(&store, 200, 1, &material, 7).await.unwrap();
}

#[tokio::test]
async fn a_pre_check_matches_only_the_exact_locator_and_secret() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    let other = ProofMaterial::generate().unwrap();
    assert!(store
        .proof_secret_matches(operation(1), material.id(), material.secret())
        .await
        .unwrap());
    assert!(!store
        .proof_secret_matches(operation(1), material.id(), other.secret())
        .await
        .unwrap());
    assert!(!store
        .proof_secret_matches(operation(1), other.id(), material.secret())
        .await
        .unwrap());
    assert!(!store
        .proof_secret_matches(operation(2), material.id(), material.secret())
        .await
        .unwrap());
}

#[tokio::test]
async fn concurrent_completions_of_one_proof_have_exactly_one_winner() {
    let (run, clock) = fixture();
    let store = Arc::new(create(&run, clock).await);
    let material = Arc::new(begin(&store, 1, 100, "owner@example.com").await);
    let attempts: Vec<_> = (0..6_u128)
        .map(|n| {
            let (store, material) = (Arc::clone(&store), Arc::clone(&material));
            tokio::spawn(async move { complete(&store, 200 + n, 1, &material, 10 + n).await })
        })
        .collect();
    let mut winners = Vec::new();
    for attempt in attempts {
        match attempt.await.unwrap() {
            Ok(SignupCompletion::Completed { subject, .. }) => winners.push(subject),
            Ok(other) => panic!("unexpected replay {other:?}"),
            Err(error) => assert!(denied(&error), "{error:?}"),
        }
    }
    assert_eq!(winners.len(), 1);
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
    assert!(store.subject(winners[0]).await.unwrap().is_some());
}

#[tokio::test]
async fn an_email_that_already_has_an_account_cannot_be_taken_over_by_a_second_proof() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let first = begin(&store, 1, 100, "shared@example.com").await;
    let second = begin(&store, 2, 101, "shared@example.com").await;
    complete(&store, 200, 1, &first, 7).await.unwrap();
    let error = complete(&store, 201, 2, &second, 8).await.unwrap_err();
    assert_eq!(error, SignupStoreError::EmailTaken);
    // The loser's whole transaction rolled back: its proof is unconsumed, no second subject.
    assert_eq!(
        store
            .signup(operation(2))
            .await
            .unwrap()
            .unwrap()
            .proof
            .state(),
        ProofState::Pending
    );
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 2, 3]);
    let record = store
        .login_record(&email("shared@example.com"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(record.subject, subject(7));
    // A different spelling is a different address with its own proof, not an adoption.
    let other = begin(&store, 3, 102, "Shared@example.com").await;
    complete(&store, 202, 3, &other, 9).await.unwrap();
    assert_eq!(counts(&store).await, [2, 2, 2, 2, 2, 3, 5]);
}

#[tokio::test]
async fn a_response_lost_after_commit_reconciles_after_reopen_without_another_effect() {
    let (run, clock) = fixture();
    let store = create(&run, clock.clone()).await;
    let material = begin(&store, 1, 100, "owner@example.com").await;
    // The commit succeeded; the caller never saw the result.
    drop(complete(&store, 200, 1, &material, 7).await);
    drop(store);
    let store = reopen(&run, clock).await;
    let (_, who, _) = store
        .completion_for_command(command(200))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(who, subject(7));
    let replay = complete(&store, 200, 1, &material, 8).await.unwrap();
    assert_eq!(
        replay,
        SignupCompletion::Replayed {
            subject: subject(7),
            epoch: store.epoch(),
            credential: phc()
        }
    );
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
    assert_eq!(store.events(0, 5).await.unwrap().len(), 1);
}

async fn registered(store: &IdentityStore) -> super::LoginRecord {
    let material = begin(store, 1, 100, "owner@example.com").await;
    complete(store, 200, 1, &material, 7).await.unwrap();
    store
        .login_record(&email("owner@example.com"))
        .await
        .unwrap()
        .unwrap()
}

fn binding(op: u128) -> RequestBinding {
    RequestBinding::new(operation(op), Purpose::Theatre)
}

#[tokio::test]
async fn finalization_admits_an_unchanged_active_account_at_the_trusted_time() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let record = registered(&store).await;
    assert_eq!(record.state, AccountState::Active);
    assert_eq!(record.credential_version, 1);
    assert_eq!(
        store.finalize_login(&record, binding(5)).await.unwrap(),
        LoginFinalization::Fresh {
            version: SecurityVersion::new(1).unwrap(),
            epoch: store.epoch(),
            at_micros: (NOW * 1_000_000) as i64,
        }
    );
    // The final check wrote nothing.
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
}

#[tokio::test]
async fn a_disable_that_commits_after_the_read_but_before_finalization_denies_login() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let record = registered(&store).await;
    // The verifier has read the record and is "hashing"; the account is disabled meanwhile.
    store
        .disable_subject(operation(50), subject(7), record.version, store.epoch())
        .await
        .unwrap();
    assert_eq!(
        store.finalize_login(&record, binding(5)).await.unwrap(),
        LoginFinalization::Denied(CompletionDenied::InactiveAccount)
    );
    // Reading it afresh shows the account disabled and it still never reaches Fresh.
    let fresh = store
        .login_record(&email("owner@example.com"))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(fresh.state, AccountState::Disabled);
    assert_eq!(
        store.finalize_login(&fresh, binding(6)).await.unwrap(),
        LoginFinalization::Denied(CompletionDenied::InactiveAccount)
    );
}

#[tokio::test]
async fn stale_version_epoch_and_credential_observations_are_denied() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let record = registered(&store).await;
    let mut stale_version = record.clone();
    stale_version.version = SecurityVersion::new(2).unwrap();
    assert_eq!(
        store
            .finalize_login(&stale_version, binding(5))
            .await
            .unwrap(),
        LoginFinalization::Denied(CompletionDenied::SecurityVersionChanged)
    );
    let mut stale_epoch = record.clone();
    stale_epoch.epoch = InstanceEpoch::from_bytes([9; 16]).unwrap();
    assert_eq!(
        store
            .finalize_login(&stale_epoch, binding(5))
            .await
            .unwrap(),
        LoginFinalization::Denied(CompletionDenied::EpochChanged)
    );
    let mut stale_credential = record.clone();
    stale_credential.credential_version = 2;
    assert_eq!(
        store
            .finalize_login(&stale_credential, binding(5))
            .await
            .unwrap(),
        LoginFinalization::CredentialChanged
    );
    let mut other_subject = record;
    other_subject.subject = subject(8);
    assert_eq!(
        store
            .finalize_login(&other_subject, binding(5))
            .await
            .unwrap(),
        LoginFinalization::Denied(CompletionDenied::SubjectChanged)
    );
}

#[tokio::test]
async fn an_old_receipt_grants_no_login_after_the_account_is_disabled() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    let record = registered(&store).await;
    store
        .disable_subject(operation(50), subject(7), record.version, store.epoch())
        .await
        .unwrap();
    // The signup receipts remain, as history, and cannot be turned into an admission.
    assert!(store.mailbox_receipt(command(200)).await.unwrap().is_some());
    assert!(store.receipt(operation(1)).await.unwrap().is_some());
    assert!(matches!(
        store.completion_for_command(command(200)).await.unwrap(),
        Some((_, who, _)) if who == subject(7)
    ));
    assert_eq!(
        store.finalize_login(&record, binding(5)).await.unwrap(),
        LoginFinalization::Denied(CompletionDenied::InactiveAccount)
    );
}

#[tokio::test]
async fn settled_signup_rows_reject_direct_mutation_and_planted_credentials_are_refused() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    registered(&store).await;
    for (sql, code) in [
        (
            "UPDATE signup_workflows SET email='x@example.com'",
            "identity.immutable_signup",
        ),
        ("DELETE FROM signup_workflows", "identity.immutable_signup"),
        (
            "UPDATE identity_emails SET verified_at=0",
            "identity.immutable_email",
        ),
        ("DELETE FROM identity_emails", "identity.immutable_email"),
        (
            "UPDATE password_credentials SET credential_version=2",
            "identity.immutable_credential",
        ),
        (
            "DELETE FROM password_credentials",
            "identity.immutable_credential",
        ),
        (
            "UPDATE signup_completions SET subject_id=zeroblob(16)",
            "identity.immutable_signup_completion",
        ),
        (
            "DELETE FROM signup_completions",
            "identity.immutable_signup_completion",
        ),
    ] {
        let tx = store.database.begin_write().await.unwrap();
        let error = tx.execute(sql, ()).await.unwrap_err();
        assert_eq!(error, guard(code), "{sql}");
        tx.rollback().await.unwrap();
    }
    assert_eq!(counts(&store).await, [1, 1, 1, 1, 1, 1, 2]);
    // A hash planted around the typed path is refused when read, never handed to the KDF.
    run_sql(&store, "DROP TRIGGER password_credentials_no_update").await;
    run_sql(
        &store,
        "UPDATE password_credentials SET phc='$argon2i$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g'",
    )
    .await;
    assert_eq!(
        store
            .login_record(&email("owner@example.com"))
            .await
            .unwrap_err(),
        SignupStoreError::CorruptRecord
    );
}

#[tokio::test]
async fn a_noncanonical_stored_email_is_refused() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    begin(&store, 1, 100, "owner@example.com").await;
    run_sql(&store, "DROP TRIGGER signup_workflows_no_update").await;
    run_sql(
        &store,
        "UPDATE signup_workflows SET email='Owner@EXAMPLE.com'",
    )
    .await;
    assert_eq!(
        store.signup(operation(1)).await.unwrap_err(),
        SignupStoreError::CorruptRecord
    );
}

#[tokio::test]
async fn schema_checks_shape_of_binding_credential_and_completion_rows() {
    let (run, clock) = fixture();
    let store = create(&run, clock).await;
    for sql in [
        "INSERT INTO signup_workflows(workflow_id,email,requested_at) VALUES(zeroblob(16),'a@example.com',0)",
        "INSERT INTO signup_workflows(workflow_id,email,requested_at) VALUES(x'01010101010101010101010101010101','a@example.com',0)",
        "INSERT INTO password_credentials(subject_id,credential_version,phc,created_at) VALUES(x'01010101010101010101010101010101',1,'short',0)",
        "INSERT INTO password_credentials(subject_id,credential_version,phc,created_at) VALUES(x'01010101010101010101010101010101',0,'$argon2id$v=19$m=19456,t=2,p=1$c29tZXNhbHRzb21lc2FsdA$aGFzaGhhc2hoYXNoaGFzaGhhc2hoYXNoaGFzaGhhc2g',0)",
        "INSERT INTO signup_completions(command_id,workflow_id,subject_id) VALUES(x'01010101010101010101010101010101',x'02020202020202020202020202020202',x'03030303030303030303030303030303')",
    ] {
        let tx = store.database.begin_write().await.unwrap();
        assert!(tx.execute(sql, ()).await.is_err(), "{sql}");
        tx.rollback().await.unwrap();
    }
    assert_eq!(counts(&store).await, [0; 7]);
}

#[tokio::test]
async fn a_version_two_file_upgrades_to_signup_storage_keeping_its_data() {
    let (run, clock) = fixture();
    const V2: [cantos_store_turso::registry::Migration; 2] =
        [super::MIGRATIONS[0], super::MIGRATIONS[1]];
    let epoch = InstanceEpoch::from_bytes(1_u128.to_be_bytes()).unwrap();
    let db = OwnedDatabase::open(OpenRequest::new(
        run.path("signup.db"),
        Registry::new(DatabaseKind::Identity, 1, &V2).unwrap(),
        Intent::Migrate,
        clock.clone(),
    ))
    .await
    .unwrap();
    let tx = db.begin_write().await.unwrap();
    tx.execute(
        "INSERT INTO identity_instance(singleton,instance_epoch) VALUES(1,?1)",
        [Value::Blob(epoch.to_bytes().to_vec())],
    )
    .await
    .unwrap();
    tx.execute(
        "INSERT INTO identity_subjects(subject_id,security_version,state) VALUES(?1,1,1)",
        [Value::Blob(subject(1).to_bytes().to_vec())],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
    drop(db);
    assert!(
        IdentityStore::open(run.path("signup.db"), Intent::Serve, clock.clone())
            .await
            .is_err(),
        "serve never applies the pending migration"
    );
    let store = create(&run, clock).await;
    assert_eq!(store.epoch(), epoch);
    assert!(store.subject(subject(1)).await.unwrap().is_some());
    begin(&store, 1, 100, "owner@example.com").await;
    assert_eq!(counts(&store).await, [1, 0, 0, 0, 0, 1, 1]);
}
