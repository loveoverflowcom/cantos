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
        mailbox_proof::{ProofId, ProofState},
        password::PasswordInput,
        security_version::SecurityVersion,
    },
    events,
    limits::{KeyFactory, Limiter, Throttled},
    mail::{MailFault, Mailer, ProofMail, SignupLinkBase},
    persistence::{
        IdentityStore, IdentityStoreError, LoginFinalization, PhcString, ProofCommandId,
        ProofMaterial, ProofSecret, ProofStoreError, SignupCompletion, SignupCompletionRequest,
        SignupStart, SignupStoreError,
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
    /// No message was attempted and nothing was stored: the address already has an account, the
    /// operation is bound to another address or already concluded, or it was repeated inside
    /// its resend cooldown.
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
    /// Storage or the mail transport could not serve the request. A start with no transport
    /// configured records nothing; a relay that refuses or loses a message does not make a
    /// start fail, it leaves the committed proof pending.
    Unavailable,
    /// A commit's outcome is unknown. Retry the IDENTICAL command to reconcile it.
    OutcomeUnknown,
    /// The operation or command exists with different content, or is already concluded.
    Conflict,
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
    firebase: Option<Arc<dyn crate::verification::VerificationActions>>,
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
            firebase: None,
        })
    }

    pub fn with_firebase(
        mut self,
        provider: Option<Arc<dyn crate::verification::VerificationActions>>,
    ) -> Self {
        self.firebase = provider;
        self
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

    /// Records a pending signup bound to `email` and mails the proof there, or re-mails a
    /// still-pending operation after the resend cooldown. The caller learns only that the
    /// request was well formed: not whether the address already has an account, whether the
    /// operation was already bound or concluded, and never the proof.
    ///
    /// Without a mail transport EVERY start is refused alike, before any lookup or write, so
    /// the outage is a deployment fact and not a per-address signal, and no proof is recorded
    /// that nobody could receive. Otherwise the registered-address, binding, current-proof and
    /// cooldown decision is one store transaction; mail is sent only after it commits, only
    /// when it issued or replaced a proof, and never inside it.
    pub async fn start_theatre_signup(
        &self,
        operation: OperationId,
        email: &str,
        client: ClientAddress,
    ) -> Result<SignupStarted, ServiceError> {
        let email = EmailAddress::parse(email).map_err(|_| ServiceError::InvalidRequest)?;
        if !self.mailer.is_configured() && self.firebase.is_none() {
            return Err(ServiceError::Unavailable);
        }
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
        if let Some(provider) = &self.firebase {
            return self
                .start_firebase(provider.as_ref(), operation, email, now)
                .await;
        }
        let command = new_command()?;
        let material = ProofMaterial::generate().map_err(|_| ServiceError::Unavailable)?;
        let started = self
            .store
            .start_or_resend_signup(
                command,
                operation,
                &email,
                &material,
                policy::PROOF_TTL_SECONDS,
                policy::RESEND_COOLDOWN_SECONDS,
            )
            .await
            .map_err(map_start_store)?;
        let proof = match started {
            SignupStart::Issued(receipt) | SignupStart::Replaced(receipt) => receipt.proof.id(),
            // Answered like every other start: a distinguishable answer would reveal which
            // addresses have accounts or which operations are bound to whom.
            SignupStart::Skipped(_) => {
                return Ok(SignupStarted {
                    dispatch: Dispatch::NotAttempted,
                })
            }
        };
        let link = self.settings.link.link(operation, proof, material.secret());
        let mail = ProofMail::new(email, operation, proof, link);
        let dispatch = match self.mailer.send(&mail).await {
            Ok(()) => Dispatch::TransportAccepted,
            Err(MailFault::Uncertain) => Dispatch::TransportUncertain,
            // `is_configured` held when this start began, so a port reporting this now broke
            // its contract. The proof is already committed and nobody holds its secret, like a
            // refusal; the outage is global, so it alone is reported.
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
        self.complete_native(
            command,
            operation,
            proof,
            secret,
            password,
            explicit_confirmation,
            client,
            false,
        )
        .await
    }

    #[allow(clippy::too_many_arguments)]
    async fn complete_native(
        &self,
        command: ProofCommandId,
        operation: OperationId,
        proof: ProofId,
        secret: [u8; 32],
        password: &str,
        explicit_confirmation: bool,
        client: ClientAddress,
        already_admitted: bool,
    ) -> Result<SignupOutcome, ServiceError> {
        if !explicit_confirmation {
            return Err(ServiceError::InvalidRequest);
        }
        let password =
            Arc::new(PasswordInput::new(password).map_err(|_| ServiceError::InvalidPassword)?);
        let now = self.now_seconds()?;
        let limits = &self.settings.limits;
        if !already_admitted {
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
        }
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

    async fn start_firebase(
        &self,
        provider: &dyn crate::verification::VerificationActions,
        operation: OperationId,
        email: EmailAddress,
        now: u64,
    ) -> Result<SignupStarted, ServiceError> {
        use crate::persistence::firebase::{Start, State};
        use crate::verification::VerificationFault as Fault;
        let material = ProofMaterial::generate().map_err(|_| ServiceError::Unavailable)?;
        let expires = now.saturating_add(policy::PROOF_TTL_SECONDS);
        let envelope = provider
            .seal(
                operation,
                material.id(),
                email.as_str(),
                &self.epoch().to_bytes(),
                expires,
                material.secret(),
            )
            .map_err(|_| ServiceError::Unavailable)?;
        let action = self
            .store
            .start_firebase(
                provider.project(),
                new_command()?,
                operation,
                &email,
                &material,
                &envelope,
                expires,
                policy::RESEND_COOLDOWN_SECONDS,
            )
            .await
            .map_err(map_start_store)?;
        let record = match action {
            Start::Skip => {
                return Ok(SignupStarted {
                    dispatch: Dispatch::NotAttempted,
                })
            }
            Start::Create(record) => {
                let result = provider
                    .create(record.operation, record.email.as_str())
                    .await;
                let next = match result {
                    Ok(()) => State::Created,
                    Err(Fault::Rejected | Fault::Configuration) => State::CreateRejected,
                    Err(Fault::Uncertain) => State::CreateUncertain,
                };
                let recorded = self
                    .store
                    .firebase_transition(
                        provider.project(),
                        record.operation,
                        State::CreateReserved,
                        next,
                        result.is_ok(),
                    )
                    .await
                    .map_err(map_start_store)?;
                if !recorded {
                    return Err(ServiceError::OutcomeUnknown);
                }
                if let Err(fault) = result {
                    return Ok(SignupStarted {
                        dispatch: if fault == Fault::Uncertain {
                            Dispatch::TransportUncertain
                        } else {
                            Dispatch::TransportRefused
                        },
                    });
                }
                if !self
                    .store
                    .firebase_transition(
                        provider.project(),
                        record.operation,
                        State::Created,
                        State::SendReserved,
                        false,
                    )
                    .await
                    .map_err(map_start_store)?
                {
                    return Err(ServiceError::OutcomeUnknown);
                }
                record
            }
            Start::Send(record) => record,
        };
        let result = provider
            .send(record.operation, record.proof, record.email.as_str())
            .await;
        let (state, dispatch) = match result {
            Ok(()) => (State::Sent, Dispatch::TransportAccepted),
            Err(Fault::Rejected | Fault::Configuration) => {
                (State::SendRejected, Dispatch::TransportRefused)
            }
            Err(Fault::Uncertain) => (State::SendUncertain, Dispatch::TransportUncertain),
        };
        if !self
            .store
            .firebase_transition(
                provider.project(),
                record.operation,
                State::SendReserved,
                state,
                false,
            )
            .await
            .map_err(map_start_store)?
        {
            return Err(ServiceError::OutcomeUnknown);
        }
        Ok(SignupStarted { dispatch })
    }

    /// Possession of this exact provider action plus explicit confirmation can release the
    /// matching encrypted native proof internally; the native transaction still consumes it.
    #[allow(clippy::too_many_arguments)]
    pub async fn complete_firebase_signup(
        &self,
        command: ProofCommandId,
        operation: OperationId,
        proof: ProofId,
        code: &str,
        password: &str,
        explicit: bool,
        client: ClientAddress,
    ) -> Result<SignupOutcome, ServiceError> {
        use crate::persistence::firebase::{Claim, State};
        if !explicit || !crate::verification::valid_action_code(code) {
            return Err(ServiceError::InvalidRequest);
        }
        PasswordInput::new(password).map_err(|_| ServiceError::InvalidPassword)?;
        let provider = self.firebase.as_ref().ok_or(ServiceError::Unavailable)?;
        let now = self.now_seconds()?;
        Self::throttle(
            self.settings
                .limits
                .complete_ip
                .admit(self.keys.address("complete-ip", client.0), now),
        )?;
        Self::throttle(self.settings.limits.complete_operation.admit(
            self.keys.pair(
                "complete-operation",
                &format!("{:032x}", u128::from_be_bytes(operation.to_bytes())),
                client.0,
            ),
            now,
        ))?;
        let record = self
            .store
            .firebase_enrollment(provider.project(), operation)
            .await
            .map_err(map_start_store)?
            .filter(|record| {
                record.proof == proof
                    && now < record.expires
                    && record.state != State::ConsumeRejected
            })
            .ok_or(ServiceError::Denied)?;
        let digest = provider
            .confirmation_tag(operation, proof, command, code, password)
            .map_err(|_| ServiceError::Unavailable)?;
        // An acknowledged consume can reconcile only this exact action/command/password.
        if !matches!(record.state, State::ReceiptReady | State::ConsumeReserved) {
            provider
                .inspect(code, record.email.as_str())
                .await
                .map_err(|fault| {
                    if fault == crate::verification::VerificationFault::Rejected {
                        ServiceError::Denied
                    } else {
                        ServiceError::Unavailable
                    }
                })?;
        }
        match self
            .store
            .claim_firebase(provider.project(), operation, proof, digest)
            .await
            .map_err(map_start_store)?
        {
            Claim::Won => {
                if let Err(fault) = provider
                    .consume(operation, code, record.email.as_str())
                    .await
                {
                    if fault == crate::verification::VerificationFault::Rejected {
                        let recorded = self
                            .store
                            .firebase_transition(
                                provider.project(),
                                operation,
                                State::ConsumeReserved,
                                State::ConsumeRejected,
                                false,
                            )
                            .await
                            .map_err(|_| ServiceError::OutcomeUnknown)?;
                        return Err(if recorded {
                            ServiceError::Denied
                        } else {
                            ServiceError::OutcomeUnknown
                        });
                    }
                    return Err(ServiceError::OutcomeUnknown);
                }
                if !self
                    .store
                    .firebase_transition(
                        provider.project(),
                        operation,
                        State::ConsumeReserved,
                        State::ReceiptReady,
                        false,
                    )
                    .await
                    .map_err(map_start_store)?
                {
                    return Err(ServiceError::OutcomeUnknown);
                }
            }
            Claim::Ready => {}
            Claim::Pending => return Err(ServiceError::OutcomeUnknown),
            Claim::Denied => return Err(ServiceError::Denied),
        }
        let secret = provider
            .open(
                operation,
                proof,
                record.email.as_str(),
                &self.epoch().to_bytes(),
                record.expires,
                &record.envelope,
            )
            .map_err(|_| ServiceError::Denied)?;
        self.complete_native(
            command, operation, proof, *secret, password, true, client, true,
        )
        .await
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

/// A start never conflicts with another start: its decision is made where it is applied, so
/// anything but an unknown commit is a storage fault (or an inconsistent record), not a
/// contract answer.
fn map_start_store(error: SignupStoreError) -> ServiceError {
    match error {
        SignupStoreError::Commit(ref fault) if is_unknown(fault) => ServiceError::OutcomeUnknown,
        SignupStoreError::Proof(ProofStoreError::Commit(ref fault)) if is_unknown(fault) => {
            ServiceError::OutcomeUnknown
        }
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

#[cfg(test)]
mod tests {
    use cantos_store_turso::error::{CommitFault, StoreFault};

    use super::*;

    #[test]
    fn a_start_is_never_a_conflict_and_never_hides_an_unknown_commit() {
        let unknown = || CommitFault::Unknown(StoreFault::Busy);
        assert_eq!(
            map_start_store(SignupStoreError::Commit(unknown())),
            ServiceError::OutcomeUnknown
        );
        assert_eq!(
            map_start_store(SignupStoreError::Proof(ProofStoreError::Commit(unknown()))),
            ServiceError::OutcomeUnknown
        );
        for refusal in [
            SignupStoreError::BindingChanged,
            SignupStoreError::Proof(ProofStoreError::Exists),
            SignupStoreError::Proof(ProofStoreError::OperationReused),
            SignupStoreError::Proof(ProofStoreError::Conflict),
            SignupStoreError::CorruptRecord,
            SignupStoreError::Commit(CommitFault::Rejected(StoreFault::Busy)),
        ] {
            assert_eq!(map_start_store(refusal), ServiceError::Unavailable);
        }
    }
}
