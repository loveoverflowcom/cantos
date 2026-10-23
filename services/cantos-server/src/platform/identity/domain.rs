//! Pure, immutable identities and binding decisions; no credential verification lives here.
use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct ActorId(String);

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum IdentityError {
    InvalidActorId,
    InvalidIssuer,
    InvalidSubject,
    ActorMissing,
    ActorInactive,
    ActorIdCollision,
    IdentityAlreadyBound,
}

impl fmt::Display for IdentityError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidActorId => "invalid actor ID",
            Self::InvalidIssuer => "invalid external issuer",
            Self::InvalidSubject => "invalid external subject",
            Self::ActorMissing => "actor does not exist",
            Self::ActorInactive => "actor is inactive",
            Self::ActorIdCollision => "proposed actor ID already exists",
            Self::IdentityAlreadyBound => "external identity belongs to another actor",
        })
    }
}

impl std::error::Error for IdentityError {}

impl ActorId {
    /// Admits the existing actors.id grammar without changing its spelling or meaning.
    pub fn new(value: impl Into<String>) -> Result<Self, IdentityError> {
        let value = value.into();
        if !(1..=64).contains(&value.len())
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'-'))
        {
            return Err(IdentityError::InvalidActorId);
        }
        Ok(Self(value))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for ActorId {
    type Error = IdentityError;
    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

impl From<ActorId> for String {
    fn from(value: ActorId) -> Self {
        value.0
    }
}

/// Exact issuer/subject pair, deliberately excluding email, username and provider groups.
/// Syntax validation does not establish that a provider issued or verified this identity.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(try_from = "RawExternalIdentityKey")]
pub struct ExternalIdentityKey {
    issuer: String,
    subject: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RawExternalIdentityKey {
    issuer: String,
    subject: String,
}

impl TryFrom<RawExternalIdentityKey> for ExternalIdentityKey {
    type Error = IdentityError;
    fn try_from(raw: RawExternalIdentityKey) -> Result<Self, Self::Error> {
        Self::new(raw.issuer, raw.subject)
    }
}

impl ExternalIdentityKey {
    pub fn new(
        issuer: impl Into<String>,
        subject: impl Into<String>,
    ) -> Result<Self, IdentityError> {
        let issuer = issuer.into();
        let subject = subject.into();
        if !valid_external_part(&issuer, 1024) {
            return Err(IdentityError::InvalidIssuer);
        }
        if !valid_external_part(&subject, 512) {
            return Err(IdentityError::InvalidSubject);
        }
        Ok(Self { issuer, subject })
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }

    pub fn subject(&self) -> &str {
        &self.subject
    }
}

fn valid_external_part(value: &str, max: usize) -> bool {
    !value.trim().is_empty() && value.len() <= max && !value.chars().any(char::is_control)
}

/// A registry snapshot, not an ActorContext, token proof, platform role or session.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ActorRecord {
    pub id: ActorId,
    pub active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Registration {
    Created(ActorId),
    Existing(ActorId),
}

impl Registration {
    pub fn actor_id(&self) -> &ActorId {
        match self {
            Self::Created(id) | Self::Existing(id) => id,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum LinkOutcome {
    Linked,
    Existing,
}

/// Same external identity always resolves to the winner; a losing candidate is never created.
pub fn decide_registration(
    binding: Option<&ActorRecord>,
    candidate: &ActorId,
    candidate_exists: bool,
) -> Result<Registration, IdentityError> {
    if let Some(actor) = binding {
        return if actor.active {
            Ok(Registration::Existing(actor.id.clone()))
        } else {
            Err(IdentityError::ActorInactive)
        };
    }
    if candidate_exists {
        return Err(IdentityError::ActorIdCollision);
    }
    Ok(Registration::Created(candidate.clone()))
}

/// Explicit linking never transfers an existing key or reactivates an account.
pub fn decide_link(
    target: Option<&ActorRecord>,
    binding: Option<&ActorId>,
) -> Result<LinkOutcome, IdentityError> {
    let target = target.ok_or(IdentityError::ActorMissing)?;
    if !target.active {
        return Err(IdentityError::ActorInactive);
    }
    match binding {
        None => Ok(LinkOutcome::Linked),
        Some(id) if id == &target.id => Ok(LinkOutcome::Existing),
        Some(_) => Err(IdentityError::IdentityAlreadyBound),
    }
}
