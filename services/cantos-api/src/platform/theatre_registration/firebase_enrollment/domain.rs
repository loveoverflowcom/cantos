//! Firebase enrollment facts and one-way effect phases. No provider status is a session.
use uuid::Uuid;

use super::super::{
    domain::{PendingRegistration, RegistrationError, SetupLink},
    firebase_action::{DedicatedUid, ExpectedActionBinding, FirebaseProjectId},
};

/// Immutable local expectations. Only a successful provider create and durable acknowledgement
/// establish UID provenance; constructing this value does not establish either fact.
#[derive(Clone, PartialEq, Eq)]
pub struct Enrollment {
    registration: PendingRegistration,
    project: String,
    uid: String,
}

impl Enrollment {
    pub fn new(
        registration: PendingRegistration,
        project: &str,
    ) -> Result<Self, RegistrationError> {
        FirebaseProjectId::parse(project).map_err(|_| RegistrationError::Configuration)?;
        let uid = format!("cantos_{}", registration.id().simple());
        DedicatedUid::parse(&uid).map_err(|_| RegistrationError::InvalidState)?;
        Ok(Self {
            registration,
            project: project.to_owned(),
            uid,
        })
    }

    pub fn registration(&self) -> &PendingRegistration {
        &self.registration
    }

    pub fn id(&self) -> Uuid {
        self.registration.id()
    }

    pub fn project(&self) -> &str {
        &self.project
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    pub(super) fn action_binding(&self) -> Result<ExpectedActionBinding, RegistrationError> {
        Ok(ExpectedActionBinding::new(
            self.registration.clone(),
            FirebaseProjectId::parse(&self.project).map_err(|_| RegistrationError::InvalidState)?,
            DedicatedUid::parse(&self.uid).map_err(|_| RegistrationError::InvalidState)?,
        ))
    }
}

impl std::fmt::Debug for Enrollment {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Enrollment([REDACTED])")
    }
}

/// A short-lived provider credential held only between exchange and a fresh delivery
/// fence. The concrete provider creates it from its positively checked exchange response;
/// HTTP, persistence and public status have no construction or serialization surface.
pub struct VerificationCredential {
    enrollment: Enrollment,
    token: String,
}

impl VerificationCredential {
    pub(in crate::platform::theatre_registration) fn new(
        enrollment: Enrollment,
        token: String,
    ) -> Result<Self, RegistrationError> {
        if token.is_empty()
            || token.len() > 16_384
            || !token.bytes().all(|byte| byte.is_ascii_graphic())
        {
            return Err(RegistrationError::DeliveryUncertain);
        }
        Ok(Self { enrollment, token })
    }

    pub(super) fn matches(&self, enrollment: &Enrollment) -> bool {
        self.enrollment == *enrollment
    }

    pub(in crate::platform::theatre_registration) fn into_parts(self) -> (Enrollment, String) {
        (self.enrollment, self.token)
    }
}

impl std::fmt::Debug for VerificationCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerificationCredential([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    BootstrapStarted,
    Ready,
    SendStarted,
    MailStarted,
    AwaitingAction,
    Inspecting,
    Consuming,
    Creating,
    Created,
    SetupStarted,
    HandedOff,
    Blocked,
    Cancelled,
    Expired,
}

impl Phase {
    /// Inspection is read-only and can release its lease; every mutating provider effect
    /// has a one-way durable fence. Cancel/expiry never resurrect this operation's UID.
    pub fn can_advance(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::BootstrapStarted, Self::Ready)
                | (Self::Ready, Self::SendStarted)
                | (Self::SendStarted, Self::MailStarted)
                | (Self::MailStarted, Self::AwaitingAction)
                | (Self::AwaitingAction, Self::SendStarted | Self::Inspecting)
                | (Self::Inspecting, Self::AwaitingAction | Self::Consuming)
                | (Self::Consuming, Self::Creating)
                | (Self::Consuming, Self::Cancelled)
                | (Self::Creating, Self::Created)
                | (Self::Created, Self::SetupStarted)
                | (Self::SetupStarted, Self::HandedOff)
                | (
                    Self::AwaitingAction | Self::Inspecting,
                    Self::Cancelled | Self::Expired
                )
                | (
                    Self::BootstrapStarted
                        | Self::Ready
                        | Self::SendStarted
                        | Self::MailStarted
                        | Self::Inspecting
                        | Self::Consuming
                        | Self::Creating
                        | Self::Created
                        | Self::SetupStarted,
                    Self::Blocked
                )
        )
    }

    pub fn may_resend(self, now: u64, expires: u64, last_sent: u64, cooldown: u32) -> bool {
        self == Self::AwaitingAction
            && now < expires
            && now
                .checked_sub(last_sent)
                .is_some_and(|elapsed| elapsed >= u64::from(cooldown))
    }

    pub fn may_claim(self, now: u64, expires: u64) -> bool {
        self == Self::AwaitingAction && now < expires
    }

    pub fn may_have_successor(self, now: u64, expires: u64) -> bool {
        matches!(self, Self::Cancelled | Self::Expired)
            || (self == Self::AwaitingAction && now >= expires)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FailureStage {
    BootstrapRejected,
    BootstrapUncertain,
    SendRejected,
    SendUncertain,
    ConsumeUncertain,
    CreateRejected,
    CreateUncertain,
    BindingMismatch,
    SetupRejected,
    SetupUncertain,
    HandoffRefused,
}

impl FailureStage {
    pub fn as_code(self) -> &'static str {
        match self {
            Self::BootstrapRejected => "bootstrap_rejected",
            Self::BootstrapUncertain => "bootstrap_uncertain",
            Self::SendRejected => "send_rejected",
            Self::SendUncertain => "send_uncertain",
            Self::ConsumeUncertain => "consume_uncertain",
            Self::CreateRejected => "create_rejected",
            Self::CreateUncertain => "create_uncertain",
            Self::BindingMismatch => "binding_mismatch",
            Self::SetupRejected => "setup_rejected",
            Self::SetupUncertain => "setup_uncertain",
            Self::HandoffRefused => "handoff_refused",
        }
    }
}

/// Only the confirming request can receive this RAM-only value. It cannot be cloned,
/// deserialized or reconstructed from a public receipt, database row or provider status.
pub struct OneShotSetup {
    login: String,
    link: SetupLink,
}

impl OneShotSetup {
    pub(super) fn new(
        registration: &PendingRegistration,
        link: SetupLink,
        kanidm_origin: &url::Url,
    ) -> Result<Self, RegistrationError> {
        let parsed = url::Url::parse(link.as_str()).map_err(|_| RegistrationError::InvalidState)?;
        let mut query = parsed.query_pairs();
        let token = query.next();
        if parsed.origin() != kanidm_origin.origin()
            || parsed.path() != "/ui/reset"
            || parsed.fragment().is_some()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || token.is_none_or(|(name, value)| {
                name != "token"
                    || value.is_empty()
                    || value.len() > 4096
                    || !value.bytes().all(|byte| byte.is_ascii_graphic())
            })
            || query.next().is_some()
        {
            return Err(RegistrationError::InvalidState);
        }
        Ok(Self {
            login: registration.provider_name(),
            link,
        })
    }

    pub fn login(&self) -> &str {
        &self.login
    }

    pub fn link(&self) -> &SetupLink {
        &self.link
    }
}

impl std::fmt::Debug for OneShotSetup {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("OneShotSetup([REDACTED])")
    }
}
