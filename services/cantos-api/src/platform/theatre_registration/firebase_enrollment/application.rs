//! Firebase mailbox proof precedes retained Kanidm provisioning. SQL commits every
//! mutating-effect fence before dispatch; interrupted effects never acquire a replay lease.
use std::sync::Arc;

use uuid::Uuid;

use super::super::{
    application::{Issuer, RegistrationFuture},
    domain::{
        CreatedPerson, Email, NetworkKey, PendingRegistration, RegistrationError,
        RegistrationPolicy,
    },
    firebase_action::{ActionCode, FirebaseActionClient, FirebaseProjectId},
};
use super::domain::{Enrollment, FailureStage, OneShotSetup, Phase, VerificationCredential};

pub type EnrollmentFuture<'a, T> = RegistrationFuture<'a, T>;

#[derive(Debug, PartialEq, Eq)]
pub enum PrepareOutcome {
    /// A committed BootstrapStarted fence for a fresh, never-reused operation and UID.
    Bootstrap(Enrollment),
    /// A committed SendStarted fence/budget for the same live UID and unchanged expiry.
    Resend(Enrollment),
    Suppressed,
}

/// All clocks/locks live in the concrete repository. Every false/None refuses the next
/// effect. Resend never rotates operation/UID/expiry or promises latest-code invalidation.
pub trait Repository: Send + Sync {
    fn prepare(
        &self,
        candidate: Enrollment,
        network: NetworkKey,
        policy: RegistrationPolicy,
    ) -> EnrollmentFuture<'_, PrepareOutcome>;
    /// Recheck a prepared lease after dispatch scheduling; never acquires another lease.
    fn dispatchable(&self, id: Uuid, phase: Phase) -> EnrollmentFuture<'_, bool>;
    fn ready(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn begin_send(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn begin_delivery(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn sent(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn claim_action(&self, id: Uuid) -> EnrollmentFuture<'_, Option<Enrollment>>;
    fn release_inspection(&self, id: Uuid) -> EnrollmentFuture<'_, ()>;
    fn begin_consume(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    /// Only called after exact action consumption; an operation ID alone cannot cancel.
    fn cancel_consumed(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn begin_create(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn created(&self, id: Uuid, person: CreatedPerson) -> EnrollmentFuture<'_, bool>;
    fn begin_setup(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn begin_handoff(&self, id: Uuid) -> EnrollmentFuture<'_, bool>;
    fn block(&self, id: Uuid, reason: FailureStage) -> EnrollmentFuture<'_, ()>;
}

/// Concrete Firebase bootstrap/send adapter plus deterministic fake. A positive create
/// acknowledges this exact password-free, unverified UID/email, never an adopted identity.
/// Send uses only a positively acknowledged durable UID; tokens stay in provider-call RAM.
pub trait MailboxProvider: Send + Sync {
    fn create_unverified(&self, enrollment: Enrollment) -> EnrollmentFuture<'_, ()>;
    fn prepare_verification(
        &self,
        enrollment: Enrollment,
    ) -> EnrollmentFuture<'_, VerificationCredential>;
    fn send_verification(
        &self,
        enrollment: Enrollment,
        credential: VerificationCredential,
    ) -> EnrollmentFuture<'_, ()>;
}

#[derive(Debug)]
enum StartKind {
    Bootstrap,
    Resend,
}

/// A committed lease, never a browser receipt or credential. HTTP sends generic 202 and
/// may dispatch this bounded work afterward without revealing suppression through latency.
#[derive(Debug)]
pub struct StartWork {
    enrollment: Enrollment,
    kind: StartKind,
}

#[derive(Debug)]
pub enum ConfirmationOutcome {
    SetupReady(OneShotSetup),
    Unavailable,
}

pub struct FirebaseEnrollmentService {
    repository: Arc<dyn Repository>,
    mailbox: Arc<dyn MailboxProvider>,
    actions: Arc<FirebaseActionClient>,
    issuer: Arc<dyn Issuer>,
    project: String,
    policy: RegistrationPolicy,
    kanidm_origin: url::Url,
}

impl FirebaseEnrollmentService {
    pub fn new(
        repository: Arc<dyn Repository>,
        mailbox: Arc<dyn MailboxProvider>,
        actions: Arc<FirebaseActionClient>,
        issuer: Arc<dyn Issuer>,
        project: &str,
        policy: RegistrationPolicy,
        kanidm_issuer: &str,
    ) -> Result<Self, RegistrationError> {
        FirebaseProjectId::parse(project).map_err(|_| RegistrationError::Configuration)?;
        let parsed =
            url::Url::parse(kanidm_issuer).map_err(|_| RegistrationError::Configuration)?;
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path() != "/oauth2/openid/cantos-theatre-web-bff"
            || parsed.as_str() != kanidm_issuer
        {
            return Err(RegistrationError::Configuration);
        }
        let kanidm_origin = parsed
            .join("/")
            .map_err(|_| RegistrationError::Configuration)?;
        Ok(Self {
            repository,
            mailbox,
            actions,
            issuer,
            project: project.to_owned(),
            policy,
            kanidm_origin,
        })
    }

    /// Database-only acceptance; no provider token, enrollment ID or setup secret escapes.
    pub async fn prepare(
        &self,
        email: &str,
        network: NetworkKey,
    ) -> Result<Option<StartWork>, RegistrationError> {
        let registration = PendingRegistration::new(Uuid::new_v4(), Email::parse(email)?)?;
        let candidate = Enrollment::new(registration, &self.project)?;
        let work = match self
            .repository
            .prepare(candidate, network, self.policy)
            .await?
        {
            PrepareOutcome::Suppressed => return Ok(None),
            PrepareOutcome::Bootstrap(enrollment) => StartWork {
                enrollment,
                kind: StartKind::Bootstrap,
            },
            PrepareOutcome::Resend(enrollment) => StartWork {
                enrollment,
                kind: StartKind::Resend,
            },
        };
        if work.enrollment.project() != self.project {
            return Err(RegistrationError::InvalidState);
        }
        Ok(Some(work))
    }

    /// Every dropped future leaves the already committed effect phase non-retryable.
    /// A fresh public request never adopts, resets, deletes or recreates that UID.
    pub async fn dispatch(&self, work: StartWork) -> Result<(), RegistrationError> {
        let enrollment = work.enrollment;
        let id = enrollment.id();
        if enrollment.project() != self.project {
            return Err(RegistrationError::InvalidState);
        }
        let phase = match work.kind {
            StartKind::Bootstrap => Phase::BootstrapStarted,
            StartKind::Resend => Phase::SendStarted,
        };
        if !self.repository.dispatchable(id, phase).await? {
            return Ok(());
        }
        if matches!(work.kind, StartKind::Bootstrap) {
            if let Err(error) = self.mailbox.create_unverified(enrollment.clone()).await {
                let stage = if rejected(error) {
                    FailureStage::BootstrapRejected
                } else {
                    FailureStage::BootstrapUncertain
                };
                self.repository.block(id, stage).await?;
                return Err(error);
            }
            if !self.repository.ready(id).await? || !self.repository.begin_send(id).await? {
                return Ok(());
            }
        }
        let credential = match self.mailbox.prepare_verification(enrollment.clone()).await {
            Ok(credential) => credential,
            Err(error) => {
                let stage = if rejected(error) {
                    FailureStage::SendRejected
                } else {
                    FailureStage::SendUncertain
                };
                self.repository.block(id, stage).await?;
                return Err(error);
            }
        };
        if !credential.matches(&enrollment) {
            self.repository
                .block(id, FailureStage::BindingMismatch)
                .await?;
            return Err(RegistrationError::InvalidState);
        }
        // Custom-token exchange awaited the provider. Refresh the database clock and
        // reserve actual mail separately; a late credential cannot extend enrollment TTL.
        if !self.repository.begin_delivery(id).await? {
            return Ok(());
        }
        match self.mailbox.send_verification(enrollment, credential).await {
            Ok(()) => {
                self.repository.sent(id).await?;
                Ok(())
            }
            Err(error) => {
                let stage = if rejected(error) {
                    FailureStage::SendRejected
                } else {
                    FailureStage::SendUncertain
                };
                self.repository.block(id, stage).await?;
                Err(error)
            }
        }
    }

    /// Only this explicit request carrying the actual unconsumed action can receive setup.
    /// Initiator polling, public operation IDs and historical emailVerified state are inert.
    pub async fn confirm(
        &self,
        operation: Uuid,
        code: &str,
    ) -> Result<ConfirmationOutcome, RegistrationError> {
        let Some(enrollment) = self.consume_action(operation, code).await? else {
            return Ok(ConfirmationOutcome::Unavailable);
        };
        self.provision(operation, enrollment).await
    }

    /// Explicit cancellation consumes the same purpose/UID-checked action and terminates
    /// only this unprovisioned operation. It cannot revoke an account or undo an effect
    /// already dispatched by the winning confirmation request.
    pub async fn cancel(&self, operation: Uuid, code: &str) -> Result<(), RegistrationError> {
        if self.consume_action(operation, code).await?.is_some() {
            self.repository.cancel_consumed(operation).await?;
        }
        Ok(())
    }

    async fn consume_action(
        &self,
        operation: Uuid,
        code: &str,
    ) -> Result<Option<Enrollment>, RegistrationError> {
        let code = match ActionCode::parse(code) {
            Ok(code) if !operation.is_nil() => code,
            _ => return Ok(None),
        };
        let Some(enrollment) = self.repository.claim_action(operation).await? else {
            return Ok(None);
        };
        if enrollment.id() != operation || enrollment.project() != self.project {
            self.repository
                .block(operation, FailureStage::BindingMismatch)
                .await?;
            return Ok(None);
        }
        let action = match self
            .actions
            .inspect(enrollment.action_binding()?, code)
            .await
        {
            Ok(action) => action,
            Err(_) => {
                // The check is non-consuming. Releasing only Inspecting cannot undo a
                // concurrent cancellation/expiry or reopen any mutating effect fence.
                self.repository.release_inspection(operation).await?;
                return Ok(None);
            }
        };
        if !self.repository.begin_consume(operation).await? {
            return Ok(None);
        }
        if self.actions.consume(action).await.is_err() {
            self.repository
                .block(operation, FailureStage::ConsumeUncertain)
                .await?;
            return Ok(None);
        }
        Ok(Some(enrollment))
    }

    async fn provision(
        &self,
        operation: Uuid,
        enrollment: Enrollment,
    ) -> Result<ConfirmationOutcome, RegistrationError> {
        // A fresh database-clock fence after the provider wait precedes person creation.
        if !self.repository.begin_create(operation).await? {
            return Ok(ConfirmationOutcome::Unavailable);
        }
        let person = match self
            .issuer
            .create_new(enrollment.registration().clone())
            .await
        {
            Ok(person)
                if person.registration_id() == operation && person.subject() == operation =>
            {
                person
            }
            Ok(_) => return self.refuse(operation, FailureStage::BindingMismatch).await,
            Err(error) => {
                return self
                    .refuse(
                        operation,
                        if rejected(error) {
                            FailureStage::CreateRejected
                        } else {
                            FailureStage::CreateUncertain
                        },
                    )
                    .await;
            }
        };
        if !self.repository.created(operation, person.clone()).await?
            || !self.repository.begin_setup(operation).await?
        {
            return Ok(ConfirmationOutcome::Unavailable);
        }
        let link = match self.issuer.setup_intent(person).await {
            Ok(link) => link,
            Err(error) => {
                return self
                    .refuse(
                        operation,
                        if rejected(error) {
                            FailureStage::SetupRejected
                        } else {
                            FailureStage::SetupUncertain
                        },
                    )
                    .await;
            }
        };
        let setup = match OneShotSetup::new(enrollment.registration(), link, &self.kanidm_origin) {
            Ok(setup) => setup,
            Err(_) => return self.refuse(operation, FailureStage::HandoffRefused).await,
        };
        // Mark release before returning the raw URL. A lost response or failed commit never
        // grants retry/reset/recovery; the bearer is neither stored nor returned by status.
        if !self.repository.begin_handoff(operation).await? {
            return Ok(ConfirmationOutcome::Unavailable);
        }
        Ok(ConfirmationOutcome::SetupReady(setup))
    }

    async fn refuse(
        &self,
        id: Uuid,
        reason: FailureStage,
    ) -> Result<ConfirmationOutcome, RegistrationError> {
        self.repository.block(id, reason).await?;
        Ok(ConfirmationOutcome::Unavailable)
    }
}

fn rejected(error: RegistrationError) -> bool {
    matches!(
        error,
        RegistrationError::IssuerRejected
            | RegistrationError::DeliveryRejected
            | RegistrationError::Conflict
    )
}
