//! The Identity application: signup, password authentication and security-state reads,
//! composed from the store, the bounded KDF pool, the mail port and abuse budgets.
//!
//! It speaks domain types only; the RPC adapter owns wire mapping and caller authorization.
//! The end of password authentication is an [`AuthenticationFact`]: a statement that, at one
//! instant under the writer lock, the password matched and the account was active at the
//! observed security version. It is neither a session nor an authorization.

use std::{net::IpAddr, sync::Arc};

use cantos_store_turso::clock::Clock;

use crate::{
    credentials::{hash_password, verify_password, KdfPool, PoolFault, Verification},
    domain::{
        admission::{AccountState, Assurance, Purpose, RequestBinding},
        email::EmailAddress,
        ids::{InstanceEpoch, OperationId, SubjectId},
        mailbox_proof::{ProofId, ProofState, ProofTime},
        password::PasswordInput,
        security_version::SecurityVersion,
    },
    events,
    limits::{KeyFactory, Limiter, Throttled},
    mail::{MailFault, Mailer, ProofMail, SignupLinkBase},
    persistence::{
        IdentityStore, IdentityStoreError, LoginFinalization, PhcString, ProofCommandId,
        ProofMaterial, ProofSecret, ProofStoreError, SignupCompletion, SignupCompletionRequest,
        SignupStoreError,
    },
    policy,
};

/// The end user's address as asserted by the authenticated Server caller. Identity never
/// derives it from anything inside a request it did not receive from that caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientAddress(pub IpAddr);

/// Everything an observer may learn about mail handling, kept apart from the response: the
/// RPC answer to a start is identical whatever happened here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Dispatch {
    /// No message was attempted: the address already has an account, or the operation was
    /// repeated inside its resend cooldown.
    NotAttempted,
    /// The relay accepted the message. This is not mailbox delivery.
    TransportAccepted,
    /// The relay's final answer was lost. The message may or may not exist.
    TransportUncertain,
    /// The relay refused the message before DATA. The proof stays pending but nobody holds its
    /// secret, so it can never be completed; the same operation may be re-started after the
    /// resend cooldown.
    TransportRefused,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignupStarted {
    pub dispatch: Dispatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SignupOutcome {
    Completed {
        subject: SubjectId,
        epoch: InstanceEpoch,
    },
    /// An identical earlier command created the account: a historical answer, no more.
    Replayed {
        subject: SubjectId,
        epoch: InstanceEpoch,
    },
}

/// Constructible only inside this crate, so a value of this type came from a completed check.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub struct AuthenticationFact {
    pub subject: SubjectId,
    pub purpose: Purpose,
    pub assurance: Assurance,
    pub authenticated_at_micros: i64,
    pub security_version: SecurityVersion,
    pub epoch: InstanceEpoch,
    pub operation: OperationId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecurityState {
    pub subject: SubjectId,
    pub state: AccountState,
    pub version: SecurityVersion,
    pub epoch: InstanceEpoch,
}

/// Outward failures. Authentication and completion refusals share one `Denied`: unknown
/// email, wrong password, disabled account, wrong/expired/consumed proof and email collision
/// are indistinguishable to the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ServiceError {
    InvalidRequest,
    InvalidPassword,
    Throttled(Throttled),
    Denied,
    /// KDF capacity is exhausted. Retry later; nothing was changed.
    Busy,
    /// Storage or mail transport could not serve the request. Nothing was committed, except
    /// that a mail that definitely failed leaves its proof cancelled.
    Unavailable,
    /// A commit's outcome is unknown. Retry the IDENTICAL command to reconcile it.
    OutcomeUnknown,
    /// The operation or command exists with different content, or is already concluded.
    Conflict,
    /// The purpose has no password path in this build.
    Unsupported,
    NotFound,
}

pub struct Limits {
    pub signup_email: Limiter,
    pub signup_ip: Limiter,
    pub complete_ip: Limiter,
    pub complete_operation: Limiter,
    pub login_pair: Limiter,
    pub login_ip: Limiter,
    pub login_account: Limiter,
}

impl Limits {
    pub fn standard() -> Self {
        Self {
            signup_email: Limiter::new(policy::SIGNUP_PER_EMAIL),
            signup_ip: Limiter::new(policy::SIGNUP_PER_IP),
            complete_ip: Limiter::new(policy::COMPLETE_PER_IP),
            complete_operation: Limiter::new(policy::COMPLETE_PER_OPERATION),
            login_pair: Limiter::new(policy::LOGIN_FAILURES_PER_PAIR),
            login_ip: Limiter::new(policy::LOGIN_PER_IP),
            login_account: Limiter::new(policy::LOGIN_FAILURES_PER_ACCOUNT),
        }
    }
}

pub struct Settings {
    pub link: SignupLinkBase,
    pub kdf: KdfPool,
    pub limits: Limits,
}

// ACCEPTANCE-PENDING(#335): the security-event outbox this service writes has no consumer, so
// "revocation within a bound" is not demonstrated. A returned fact is valid only as of its final
// check; the Server must compare `security_version`/`epoch` with a fresh read before a session.
pub struct IdentityService {
    store: Arc<IdentityStore>,
    clock: Arc<dyn Clock>,
    mailer: Arc<dyn Mailer>,
    settings: Settings,
    keys: KeyFactory,
    /// A real, policy-parameter hash verified against when an email has no account, so an
    /// unknown account does not exit before the KDF. Equal timing is not claimed.
    dummy: PhcString,
}

fn random_nonzero() -> Option<[u8; 16]> {
    for _ in 0..4 {
        let mut bytes = [0; 16];
        getrandom::fill(&mut bytes).ok()?;
        if bytes != [0; 16] {
            return Some(bytes);
        }
    }
    None
}

impl IdentityService {
    /// Blocking: hashes one dummy password. Call before serving, off the async executor.
    pub fn new(
        store: Arc<IdentityStore>,
        clock: Arc<dyn Clock>,
        mailer: Arc<dyn Mailer>,
        settings: Settings,
    ) -> Result<Self, ServiceError> {
        let mut filler = [0_u8; 24];
        getrandom::fill(&mut filler).map_err(|_| ServiceError::Unavailable)?;
        let text: String = filler.iter().map(|b| char::from(b'a' + (b % 26))).collect();
        let dummy =
            hash_password(&PasswordInput::new(&text).map_err(|_| ServiceError::Unavailable)?)
                .map_err(|_| ServiceError::Unavailable)?;
        Ok(Self {
            store,
            clock,
            mailer,
            settings,
            keys: KeyFactory::new().map_err(|_| ServiceError::Unavailable)?,
            dummy,
        })
    }

    pub fn epoch(&self) -> InstanceEpoch {
        self.store.epoch()
    }

    fn now_micros(&self) -> Result<i64, ServiceError> {
        self.clock
            .now()
            .map(|t| t.get())
            .map_err(|_| ServiceError::Unavailable)
    }

    fn now_seconds(&self) -> Result<u64, ServiceError> {
        u64::try_from(self.now_micros()? / 1_000_000).map_err(|_| ServiceError::Unavailable)
    }

    fn throttle(result: Result<(), Throttled>) -> Result<(), ServiceError> {
        result.map_err(ServiceError::Throttled)
    }

    // ---- signup -------------------------------------------------------------------------

    /// Records a pending signup bound to `email` and mails the proof there. The caller learns
    /// only that the request was well formed: not whether the address already has an account,
    /// and never the proof.
    pub async fn start_theatre_signup(
        &self,
        operation: OperationId,
        email: &str,
        client: ClientAddress,
    ) -> Result<SignupStarted, ServiceError> {
        let email = EmailAddress::parse(email).map_err(|_| ServiceError::InvalidRequest)?;
        let now = self.now_seconds()?;
        let limits = &self.settings.limits;
        Self::throttle(
            limits
                .signup_ip
                .admit(self.keys.address("signup-ip", client.0), now),
        )?;
        Self::throttle(
            limits
                .signup_email
                .admit(self.keys.text("signup-email", email.as_str()), now),
        )?;
        if self
            .store
            .email_registered(&email)
            .await
            .map_err(|_| ServiceError::Unavailable)?
        {
            return Ok(SignupStarted {
                dispatch: Dispatch::NotAttempted,
            });
        }
        let existing = self
            .store
            .signup(operation)
            .await
            .map_err(|_| ServiceError::Unavailable)?;
        let command = new_command()?;
        let material = ProofMaterial::generate().map_err(|_| ServiceError::Unavailable)?;
        let expires = ProofTime::new(now + policy::PROOF_TTL_SECONDS)
            .map_err(|_| ServiceError::Unavailable)?;
        let proof = match existing {
            None => {
                self.store
                    .begin_signup(command, operation, &email, &material, expires)
                    .await
                    .map_err(map_signup_store)?;
                material.id()
            }
            Some(record) => {
                // An operation bound to another address, or already concluded, is answered like
                // every other start: a distinguishable answer would reveal which addresses have
                // accounts (the registered path above also returns "accepted, nothing sent").
                if record.email != email || record.proof.state() != ProofState::Pending {
                    return Ok(SignupStarted {
                        dispatch: Dispatch::NotAttempted,
                    });
                }
                let cooled = now
                    >= record.proof.issued_at().seconds()
                        + u64::from(policy::RESEND_COOLDOWN_SECONDS);
                if !cooled {
                    return Ok(SignupStarted {
                        dispatch: Dispatch::NotAttempted,
                    });
                }
                self.store
                    .replace_mailbox_proof(
                        command,
                        record.proof.binding(),
                        record.proof.id(),
                        &material,
                        expires,
                        policy::RESEND_COOLDOWN_SECONDS,
                    )
                    .await
                    .map_err(map_proof_store)?;
                material.id()
            }
        };
        let link = self.settings.link.link(operation, proof, material.secret());
        let mail = ProofMail::new(email, operation, proof, link);
        let dispatch = match self.mailer.send(&mail).await {
            Ok(()) => Dispatch::TransportAccepted,
            Err(MailFault::Uncertain) => Dispatch::TransportUncertain,
            // A transport that is not configured at all is a global outage, not a per-recipient
            // signal, so it alone is reported.
            Err(MailFault::NotConfigured) => return Err(ServiceError::Unavailable),
            Err(MailFault::Rejected) => {
                events::emit("mail_refused", "the relay refused a proof message");
                Dispatch::TransportRefused
            }
        };
        Ok(SignupStarted { dispatch })
    }

    /// The mailbox owner's explicit confirmation with the password they chose. Hashing runs
    /// first and outside any transaction; every precondition is then re-read and the proof
    /// consumption, account, email and credential commit together or not at all.
    #[allow(clippy::too_many_arguments)]
    pub async fn complete_theatre_signup(
        &self,
        command: ProofCommandId,
        operation: OperationId,
        proof: ProofId,
        secret: [u8; 32],
        password: &str,
        explicit_confirmation: bool,
        client: ClientAddress,
    ) -> Result<SignupOutcome, ServiceError> {
        if !explicit_confirmation {
            return Err(ServiceError::InvalidRequest);
        }
        let password =
            Arc::new(PasswordInput::new(password).map_err(|_| ServiceError::InvalidPassword)?);
        let now = self.now_seconds()?;
        let limits = &self.settings.limits;
        Self::throttle(
            limits
                .complete_ip
                .admit(self.keys.address("complete-ip", client.0), now),
        )?;
        // Keyed by (operation, address): the operation id is known to whoever started it, so a
        // budget per operation alone would let that party exhaust the mailbox owner's attempts.
        Self::throttle(limits.complete_operation.admit(
            self.keys.pair(
                "complete-operation",
                &format!("{:032x}", u128::from_be_bytes(operation.to_bytes())),
                client.0,
            ),
            now,
        ))?;
        let secret = ProofSecret::from_bytes(secret);

        // Replay of an identical command: no new hash. The supplied password must match the
        // credential that command created, so a replay cannot smuggle in a different password.
        if let Some((workflow, subject, stored)) = self
            .store
            .completion_for_command(command)
            .await
            .map_err(|_| ServiceError::Unavailable)?
        {
            if workflow != operation {
                return Err(ServiceError::Conflict);
            }
            let matched = self.verify(Arc::clone(&password), stored.clone()).await?;
            if matched != Verification::Match {
                return Err(ServiceError::Denied);
            }
            return self
                .finish_completion(command, operation, proof, &secret, &stored, subject)
                .await
                .map(|(outcome, _)| outcome);
        }

        // Cheap refusal before any KDF work: wrong locator/secret, or a proof that is not a
        // live pending one.
        let pending = self
            .store
            .signup(operation)
            .await
            .map_err(|_| ServiceError::Unavailable)?
            .filter(|record| {
                record.proof.state() == ProofState::Pending
                    && record.proof.id() == proof
                    && now < record.proof.expires_at().seconds()
            });
        if pending.is_none() {
            return Err(ServiceError::Denied);
        }
        if !self
            .store
            .proof_secret_matches(operation, proof, &secret)
            .await
            .map_err(|_| ServiceError::Unavailable)?
        {
            return Err(ServiceError::Denied);
        }
        let hashing = Arc::clone(&password);
        let phc = self
            .settings
            .kdf
            .run(move || hash_password(&hashing))
            .await
            .map_err(map_pool)?
            .map_err(|_| ServiceError::Unavailable)?;
        let subject = SubjectId::from_bytes(random_nonzero().ok_or(ServiceError::Unavailable)?)
            .map_err(|_| ServiceError::Unavailable)?;
        let (outcome, stored) = self
            .finish_completion(command, operation, proof, &secret, &phc, subject)
            .await?;
        // A concurrent identical command may have committed between the pre-check and the
        // transaction: its credential, not this call's hash, is what exists. This call's password
        // must match it, or the caller never learns the account was created by someone else.
        if let Some(stored) = stored {
            if self.verify(password, stored).await? != Verification::Match {
                return Err(ServiceError::Denied);
            }
        }
        Ok(outcome)
    }

    async fn finish_completion(
        &self,
        command: ProofCommandId,
        operation: OperationId,
        proof: ProofId,
        secret: &ProofSecret,
        phc: &PhcString,
        subject: SubjectId,
    ) -> Result<(SignupOutcome, Option<PhcString>), ServiceError> {
        let request = SignupCompletionRequest {
            command,
            operation,
            proof,
            secret,
            phc,
            subject,
        };
        match self.store.complete_signup(&request).await {
            Ok(SignupCompletion::Completed { subject, epoch }) => {
                Ok((SignupOutcome::Completed { subject, epoch }, None))
            }
            Ok(SignupCompletion::Replayed {
                subject,
                epoch,
                credential,
            }) => Ok((SignupOutcome::Replayed { subject, epoch }, Some(credential))),
            Err(error) => Err(map_completion(error)),
        }
    }

    // ---- password authentication ---------------------------------------------------------

    pub async fn authenticate_password(
        &self,
        operation: OperationId,
        purpose: Purpose,
        email: &str,
        password: &str,
        client: ClientAddress,
    ) -> Result<AuthenticationFact, ServiceError> {
        if purpose != Purpose::Theatre {
            // Studio requires a user-verified passkey; it is never downgraded to a password.
            return Err(ServiceError::Unsupported);
        }
        let email = EmailAddress::parse(email).map_err(|_| ServiceError::InvalidRequest)?;
        let password = Arc::new(
            PasswordInput::for_verification(password).map_err(|_| ServiceError::InvalidRequest)?,
        );
        let now = self.now_seconds()?;
        let limits = &self.settings.limits;
        let pair = self.keys.pair("login-pair", email.as_str(), client.0);
        let account = self.keys.text("login-account", email.as_str());
        Self::throttle(
            limits
                .login_ip
                .admit(self.keys.address("login-ip", client.0), now),
        )?;
        if let Some(throttled) = limits
            .login_pair
            .exceeded(pair, now)
            .or_else(|| limits.login_account.exceeded(account, now))
        {
            return Err(ServiceError::Throttled(throttled));
        }
        let record = self
            .store
            .login_record(&email)
            .await
            .map_err(|_| ServiceError::Unavailable)?;
        // Unknown accounts verify against a real dummy hash, so they take the KDF path too.
        let target = record
            .as_ref()
            .map_or_else(|| self.dummy.clone(), |r| r.phc.clone());
        let verdict = self.verify(password, target).await;
        let matched = match (&record, verdict) {
            (Some(_), Ok(Verification::Match)) => true,
            (_, Err(error @ (ServiceError::Busy | ServiceError::Unavailable))) => {
                return Err(error)
            }
            _ => false,
        };
        let Some(record) = record.filter(|_| matched) else {
            limits.login_pair.record(pair, now);
            limits.login_account.record(account, now);
            return Err(ServiceError::Denied);
        };
        let finalization = self
            .store
            .finalize_login(&record, RequestBinding::new(operation, purpose))
            .await
            .map_err(|_| ServiceError::Unavailable)?;
        match finalization {
            LoginFinalization::Fresh {
                version,
                epoch,
                at_micros,
            } => {
                limits.login_pair.clear(pair);
                Ok(AuthenticationFact {
                    subject: record.subject,
                    purpose,
                    assurance: Assurance::Password,
                    authenticated_at_micros: at_micros,
                    security_version: version,
                    epoch,
                    operation,
                })
            }
            LoginFinalization::Denied(_) | LoginFinalization::CredentialChanged => {
                Err(ServiceError::Denied)
            }
        }
    }

    async fn verify(
        &self,
        password: Arc<PasswordInput>,
        stored: PhcString,
    ) -> Result<Verification, ServiceError> {
        match self
            .settings
            .kdf
            .run(move || verify_password(&password, &stored))
            .await
            .map_err(map_pool)?
        {
            Ok(verdict) => Ok(verdict),
            // A stored hash outside policy is unusable, which is a denial, never a success.
            Err(_) => Ok(Verification::Mismatch),
        }
    }

    // ---- security state --------------------------------------------------------------------

    pub async fn security_state(&self, subject: SubjectId) -> Result<SecurityState, ServiceError> {
        let stored = self
            .store
            .subject(subject)
            .await
            .map_err(|_| ServiceError::Unavailable)?
            .ok_or(ServiceError::NotFound)?;
        Ok(SecurityState {
            subject: stored.subject,
            state: stored.state,
            version: stored.version,
            epoch: stored.epoch,
        })
    }
}

fn new_command() -> Result<ProofCommandId, ServiceError> {
    ProofCommandId::from_bytes(random_nonzero().ok_or(ServiceError::Unavailable)?)
        .map_err(|_| ServiceError::Unavailable)
}

fn map_pool(fault: PoolFault) -> ServiceError {
    match fault {
        PoolFault::QueueFull | PoolFault::QueueDeadline => ServiceError::Busy,
        PoolFault::JobFailed => ServiceError::Unavailable,
    }
}

fn is_unknown(fault: &cantos_store_turso::error::CommitFault) -> bool {
    matches!(fault, cantos_store_turso::error::CommitFault::Unknown(_))
}

fn map_proof_store(error: ProofStoreError) -> ServiceError {
    match error {
        ProofStoreError::Commit(ref fault) if is_unknown(fault) => ServiceError::OutcomeUnknown,
        ProofStoreError::OperationReused | ProofStoreError::Exists => ServiceError::Conflict,
        _ => ServiceError::Unavailable,
    }
}

fn map_signup_store(error: SignupStoreError) -> ServiceError {
    match error {
        SignupStoreError::Commit(ref fault) if is_unknown(fault) => ServiceError::OutcomeUnknown,
        SignupStoreError::BindingChanged => ServiceError::Conflict,
        SignupStoreError::Proof(inner) => map_proof_store(inner),
        _ => ServiceError::Unavailable,
    }
}

fn map_completion(error: SignupStoreError) -> ServiceError {
    match error {
        SignupStoreError::Commit(ref fault) if is_unknown(fault) => ServiceError::OutcomeUnknown,
        SignupStoreError::Proof(ProofStoreError::Commit(ref fault)) if is_unknown(fault) => {
            ServiceError::OutcomeUnknown
        }
        SignupStoreError::Proof(ProofStoreError::OperationReused) => ServiceError::Conflict,
        SignupStoreError::Identity(IdentityStoreError::Commit(ref fault)) if is_unknown(fault) => {
            ServiceError::OutcomeUnknown
        }
        // Wrong, expired, consumed or cancelled proof; a missing or non-signup operation; an
        // email that already has an account: all one generic denial.
        SignupStoreError::Proof(
            ProofStoreError::Denied(_)
            | ProofStoreError::InvalidSecret
            | ProofStoreError::Missing
            | ProofStoreError::Conflict,
        )
        | SignupStoreError::EmailTaken
        | SignupStoreError::WorkflowMissing
        | SignupStoreError::NotSignup => ServiceError::Denied,
        _ => ServiceError::Unavailable,
    }
}
