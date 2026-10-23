use std::{fmt, future::Future};

use super::domain::{
    ActorId, ActorRecord, ExternalIdentityKey, IdentityError, LinkOutcome, Registration,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistryError {
    Rejected(IdentityError),
    Unavailable,
    CorruptRecord,
}

impl From<IdentityError> for RegistryError {
    fn from(error: IdentityError) -> Self {
        Self::Rejected(error)
    }
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(error) => error.fmt(f),
            Self::Unavailable => {
                f.write_str("identity registry unavailable; outcome may be unknown")
            }
            Self::CorruptRecord => f.write_str("invalid identity registry record"),
        }
    }
}

impl std::error::Error for RegistryError {}

/// Real consumer: IdentityService used by explicit local operator commands.
/// Implementations serialize mutations per exact key, atomically create actor+binding,
/// leave no losing actor on replay/rejection, and never update/delete/transfer a binding.
pub trait IdentityRegistry: Send + Sync {
    fn lookup(
        &self,
        key: &ExternalIdentityKey,
    ) -> impl Future<Output = Result<Option<ActorRecord>, RegistryError>> + Send;
    fn register(
        &self,
        key: &ExternalIdentityKey,
        candidate: &ActorId,
    ) -> impl Future<Output = Result<Registration, RegistryError>> + Send;
    fn link(
        &self,
        key: &ExternalIdentityKey,
        target: &ActorId,
    ) -> impl Future<Output = Result<LinkOutcome, RegistryError>> + Send;
}
