//! Orchestration only. Durable compare-and-set fences commit before issuer effects.
use std::{future::Future, pin::Pin, sync::Arc};

use uuid::Uuid;

use super::domain::{
    BlockReason, Candidate, CreatedPerson, Email, MailKind, NetworkKey, PendingRegistration,
    RegistrationError, RegistrationMail, RegistrationPolicy, SetupLink, TokenHash,
};

pub type RegistrationFuture<'a, T> =
    Pin<Box<dyn Future<Output = Result<T, RegistrationError>> + Send + 'a>>;

#[derive(Debug, PartialEq, Eq)]
pub enum PrepareOutcome {
    Deliver(PendingRegistration),
    Suppressed,
}

/// SQL owns commit time and serializes by the case-insensitive email deduplication key.
/// Every false/None result is a refused effect, never permission to bypass a fence.
pub trait Repository: Send + Sync {
    fn prepare(
        &self,
        candidate: Candidate,
        policy: RegistrationPolicy,
    ) -> RegistrationFuture<'_, PrepareOutcome>;
    fn claim(&self, token_hash: TokenHash) -> RegistrationFuture<'_, Option<PendingRegistration>>;
    fn cancel(&self, token_hash: TokenHash) -> RegistrationFuture<'_, ()>;
    fn created(&self, id: Uuid, person: CreatedPerson) -> RegistrationFuture<'_, bool>;
    fn begin_setup(&self, id: Uuid) -> RegistrationFuture<'_, bool>;
    fn begin_delivery(&self, id: Uuid) -> RegistrationFuture<'_, bool>;
    fn complete(&self, id: Uuid) -> RegistrationFuture<'_, bool>;
    fn block(&self, id: Uuid, reason: BlockReason) -> RegistrationFuture<'_, ()>;
}

pub trait Issuer: Send + Sync {
    fn create_new(
        &self,
        registration: PendingRegistration,
    ) -> RegistrationFuture<'_, CreatedPerson>;
    fn setup_intent(&self, person: CreatedPerson) -> RegistrationFuture<'_, SetupLink>;
}

pub trait Mailer: Send + Sync {
    fn send(&self, mail: RegistrationMail) -> RegistrationFuture<'_, ()>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerificationOutcome {
    SetupSent,
    Unavailable,
}

pub struct RegistrationService {
    repository: Arc<dyn Repository>,
    issuer: Arc<dyn Issuer>,
    mailer: Arc<dyn Mailer>,
    web_origin: String,
    policy: RegistrationPolicy,
}

impl RegistrationService {
    pub fn new(
        repository: Arc<dyn Repository>,
        issuer: Arc<dyn Issuer>,
        mailer: Arc<dyn Mailer>,
        web_origin: &str,
        policy: RegistrationPolicy,
    ) -> Result<Self, RegistrationError> {
        let origin = url::Url::parse(web_origin).map_err(|_| RegistrationError::Configuration)?;
        let loopback = matches!(origin.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (origin.scheme() != "https" && !(origin.scheme() == "http" && loopback))
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.query().is_some()
            || origin.fragment().is_some()
            || origin.path() != "/"
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            repository,
            issuer,
            mailer,
            web_origin: origin.as_str().trim_end_matches('/').to_owned(),
            policy,
        })
    }

    /// No issuer or mail I/O: HTTP can return the same 202 before dispatching bounded mail work.
    /// Resends rotate the proof only while provisioning has never started.
    pub async fn prepare(
        &self,
        email: &str,
        network_key: NetworkKey,
    ) -> Result<Option<RegistrationMail>, RegistrationError> {
        let email = Email::parse(email)?;
        let token = random_token()?;
        let token_hash = TokenHash::from_token(&token)?;
        let candidate = Candidate {
            registration: PendingRegistration::new(Uuid::new_v4(), email)?,
            token_hash,
            network_key,
        };
        match self.repository.prepare(candidate, self.policy).await? {
            PrepareOutcome::Suppressed => Ok(None),
            PrepareOutcome::Deliver(registration) => Ok(Some(RegistrationMail::verification(
                registration.email().clone(),
                format!("{}/#register-verify?token={token}", self.web_origin),
                Uuid::new_v4(),
            ))),
        }
    }

    /// A dropped task stores no raw token. A new request after cooldown rotates it safely.
    pub async fn send_verification(&self, mail: RegistrationMail) -> Result<(), RegistrationError> {
        if mail.kind() != MailKind::Verification {
            return Err(RegistrationError::InvalidState);
        }
        self.mailer.send(mail).await
    }

    /// An explicit user cancellation only invalidates an unconsumed proof. It cannot revoke
    /// somebody else's session/account or interrupt a provisioning effect already dispatched.
    pub async fn cancel(&self, token: &str) -> Result<(), RegistrationError> {
        match TokenHash::from_token(token) {
            Ok(hash) => self.repository.cancel(hash).await,
            Err(RegistrationError::InvalidToken) => Ok(()),
            Err(error) => Err(error),
        }
    }

    pub async fn verify(&self, token: &str) -> Result<VerificationOutcome, RegistrationError> {
        let hash = match TokenHash::from_token(token) {
            Ok(hash) => hash,
            Err(RegistrationError::InvalidToken) => return Ok(VerificationOutcome::Unavailable),
            Err(error) => return Err(error),
        };
        let Some(registration) = self.repository.claim(hash).await? else {
            return Ok(VerificationOutcome::Unavailable);
        };
        // Claim commits Creating before the first request. Cancellation/unknown outcomes cannot
        // acquire a second create lease, even after a process restart or lost HTTP response.
        let id = registration.id();
        let person = match self.issuer.create_new(registration.clone()).await {
            Ok(person) if person.registration_id() == id && person.subject() == id => person,
            Ok(_) => return self.refuse(id, BlockReason::BindingMismatch).await,
            Err(RegistrationError::IssuerRejected | RegistrationError::Conflict) => {
                return self.refuse(id, BlockReason::CreateRejected).await;
            }
            Err(_) => return self.refuse(id, BlockReason::CreateUncertain).await,
        };
        if !self.repository.created(id, person.clone()).await?
            || !self.repository.begin_setup(id).await?
        {
            return Ok(VerificationOutcome::Unavailable);
        }
        let setup = match self.issuer.setup_intent(person.clone()).await {
            Ok(setup) => setup,
            Err(RegistrationError::IssuerRejected | RegistrationError::Conflict) => {
                return self.refuse(id, BlockReason::SetupRejected).await;
            }
            Err(_) => return self.refuse(id, BlockReason::SetupUncertain).await,
        };
        if !self.repository.begin_delivery(id).await? {
            return Ok(VerificationOutcome::Unavailable);
        }
        let mail = RegistrationMail::credential_setup(&registration, &person, setup)?;
        match self.mailer.send(mail).await {
            Ok(()) => {
                if self.repository.complete(id).await? {
                    Ok(VerificationOutcome::SetupSent)
                } else {
                    Ok(VerificationOutcome::Unavailable)
                }
            }
            Err(RegistrationError::DeliveryRejected) => {
                self.refuse(id, BlockReason::SetupDeliveryRejected).await
            }
            Err(_) => self.refuse(id, BlockReason::SetupDeliveryUncertain).await,
        }
    }

    async fn refuse(
        &self,
        id: Uuid,
        reason: BlockReason,
    ) -> Result<VerificationOutcome, RegistrationError> {
        self.repository.block(id, reason).await?;
        Ok(VerificationOutcome::Unavailable)
    }
}

fn random_token() -> Result<String, RegistrationError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| RegistrationError::Unavailable)?;
    Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
}

#[cfg(test)]
pub(crate) mod test_support;
#[cfg(test)]
mod tests;
