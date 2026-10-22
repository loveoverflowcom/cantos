//! Pure Admin policy. A verifier's authentication must precede construction of RoleEvidence.
use crate::platform::identity::domain::{ActorId, ExternalIdentityKey};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    Auditor,
    IdentityOperator,
}
impl Role {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auditor => "auditor",
            Self::IdentityOperator => "identity_operator",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    Capabilities,
    RevokeAccess,
}
impl Action {
    pub(super) fn as_str(self) -> &'static str {
        match self {
            Self::Capabilities => "capabilities",
            Self::RevokeAccess => "revoke_admin_access",
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AdminError {
    Configuration,
    Credential,
    ProviderUnavailable,
    MissingRole,
    UnknownRole,
    StaleRole,
    ActorMissing,
    InactiveActor,
    Revoked,
    Forbidden,
    InvalidRequest,
    TargetMissing,
    OperationReused,
    AuditUnavailable,
    Unavailable,
    CorruptRecord,
}

impl AdminError {
    pub(super) fn audit_outcome(self) -> &'static str {
        match self {
            Self::Configuration
            | Self::ProviderUnavailable
            | Self::AuditUnavailable
            | Self::Unavailable
            | Self::CorruptRecord => "unavailable",
            Self::Credential
            | Self::MissingRole
            | Self::UnknownRole
            | Self::StaleRole
            | Self::ActorMissing
            | Self::InactiveActor
            | Self::Revoked
            | Self::Forbidden
            | Self::InvalidRequest
            | Self::TargetMissing => "denied",
            Self::OperationReused => "conflict",
        }
    }
}

/// Sealed, nonserializable, request-local evidence. No actor or resource ACL authority.
pub struct RoleEvidence {
    key: ExternalIdentityKey,
    role: Role,
    observed: u64,
    valid_until: u64,
}
impl RoleEvidence {
    /// For an [`super::AdminVerifier`] only: the credential was verified and the role observed now.
    pub fn verified(key: ExternalIdentityKey, role: Role, observed: u64, valid_until: u64) -> Self {
        Self {
            key,
            role,
            observed,
            valid_until,
        }
    }
    pub(super) fn valid_until(&self) -> u64 {
        self.valid_until
    }
    pub(super) fn key(&self) -> &ExternalIdentityKey {
        &self.key
    }
}

pub(super) struct AdminPermit {
    actor: ActorId,
    action: Action,
    role: Role,
    observed: u64,
    valid_until: u64,
}
impl AdminPermit {
    pub(super) fn ensure_current(&self, now: u64) -> Result<(), AdminError> {
        if now < self.observed || now >= self.valid_until {
            Err(AdminError::StaleRole)
        } else {
            Ok(())
        }
    }
    pub(super) fn role(&self) -> Role {
        self.role
    }
    pub(super) fn actor(&self) -> &ActorId {
        &self.actor
    }
    pub(super) fn action(&self) -> Action {
        self.action
    }
}

pub fn map_groups(groups: &[String]) -> Result<Role, AdminError> {
    if groups.is_empty() {
        return Err(AdminError::MissingRole);
    }
    if groups.len() > 2 {
        return Err(AdminError::UnknownRole);
    }
    let mut role = Role::Auditor;
    for group in groups {
        match group.as_str() {
            "cantos_system_auditors" => (),
            "cantos_identity_operators" => role = Role::IdentityOperator,
            _ => return Err(AdminError::UnknownRole),
        }
    }
    Ok(role)
}

pub(super) fn authorize(
    proof: &RoleEvidence,
    actor: &ActorId,
    active: bool,
    revoked: bool,
    action: Action,
    now: u64,
) -> Result<AdminPermit, AdminError> {
    if now < proof.observed || now >= proof.valid_until {
        return Err(AdminError::StaleRole);
    }
    if !active {
        return Err(AdminError::InactiveActor);
    }
    if revoked {
        return Err(AdminError::Revoked);
    }
    match (proof.role, action) {
        (Role::Auditor, Action::RevokeAccess) => Err(AdminError::Forbidden),
        (Role::Auditor | Role::IdentityOperator, Action::Capabilities)
        | (Role::IdentityOperator, Action::RevokeAccess) => Ok(AdminPermit {
            actor: actor.clone(),
            action,
            role: proof.role,
            observed: proof.observed,
            valid_until: proof.valid_until,
        }),
    }
}
