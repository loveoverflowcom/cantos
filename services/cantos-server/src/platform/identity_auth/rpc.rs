//! The Server's client of Identity's private `cantos.identity.v1` RPC. Identity owns the proto
//! (`services/cantos-identity/contracts/identity/v1/identity.proto`); the Server generates and
//! checks in its own client binding with `tools/proto-codegen` and never imports the Identity
//! crate. A decoded message is untrusted input, never a verified principal: the caller runs
//! [`validate_fact`] against the operation and purpose it started, then still applies its own
//! freshness policy before issuing any session.

use std::fmt;

#[allow(clippy::all, clippy::pedantic)]
#[rustfmt::skip]
mod generated;
pub use generated::{
    identity_client, AccountState, Assurance, AuthenticatePasswordRequest,
    AuthenticatePasswordResponse, AuthenticationFact, CanonicalizeMailboxRequest,
    CanonicalizeMailboxResponse, ClientContext, CompleteTheatreSignupRequest,
    CompleteTheatreSignupResponse, GetSecurityStateRequest, GetSecurityStateResponse,
    GetVerifiedMailboxRequest, Purpose, SetupOutcome, StartTheatreSignupRequest,
    StartTheatreSignupResponse, VerifiedMailboxFact,
};

/// Largest request or response the Server sends or accepts on this channel.
pub const MAX_MESSAGE_BYTES: usize = 8 * 1024;
/// Subject, instance epoch, operation, command and proof identifiers are exactly 16 bytes.
pub const ID_BYTES: usize = 16;

impl fmt::Debug for StartTheatreSignupRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("StartTheatreSignupRequest([REDACTED])")
    }
}

impl fmt::Debug for CompleteTheatreSignupRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("CompleteTheatreSignupRequest([REDACTED])")
    }
}

impl fmt::Debug for AuthenticatePasswordRequest {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AuthenticatePasswordRequest([REDACTED])")
    }
}

/// Why a decoded fact was not accepted. No variant carries message content.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FactRejected {
    Malformed,
    WrongOperation,
    WrongPurpose,
    UnsupportedAssurance,
}

impl fmt::Display for FactRejected {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Malformed => "authentication fact is malformed",
            Self::WrongOperation => "authentication fact belongs to another operation",
            Self::WrongPurpose => "authentication fact belongs to another purpose",
            Self::UnsupportedAssurance => "authentication fact assurance is not accepted",
        })
    }
}

impl std::error::Error for FactRejected {}

/// A fact whose shape and operation/purpose binding were checked by [`validate_fact`]. It is
/// still only what Identity asserted at `authenticated_at_unix_micros`: the consumer owns the
/// freshness window, subject-to-actor binding and session policy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct ValidatedFact {
    pub subject: [u8; ID_BYTES],
    pub instance_epoch: [u8; ID_BYTES],
    pub security_version: u64,
    pub authenticated_at_unix_micros: i64,
    pub purpose: Purpose,
    pub assurance: Assurance,
}

impl fmt::Debug for ValidatedFact {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ValidatedFact")
            .field("purpose", &self.purpose)
            .field("assurance", &self.assurance)
            .field("security_version", &self.security_version)
            .finish_non_exhaustive()
    }
}

fn nonzero_id(bytes: &[u8]) -> Option<[u8; ID_BYTES]> {
    let id: [u8; ID_BYTES] = bytes.try_into().ok()?;
    (id != [0; ID_BYTES]).then_some(id)
}

/// Validates a decoded fact against the operation and purpose the caller started. Theatre and
/// Studio are accepted purposes (#335 v1 decision), and `Password` the only accepted assurance;
/// a Studio fact still says nothing about the Server's Studio roster.
pub fn validate_fact(
    fact: &AuthenticationFact,
    operation: [u8; ID_BYTES],
    purpose: Purpose,
) -> Result<ValidatedFact, FactRejected> {
    let subject = nonzero_id(&fact.subject_id).ok_or(FactRejected::Malformed)?;
    let instance_epoch = nonzero_id(&fact.instance_epoch).ok_or(FactRejected::Malformed)?;
    let echoed = nonzero_id(&fact.operation_id).ok_or(FactRejected::Malformed)?;
    if fact.security_version == 0
        || i64::try_from(fact.security_version).is_err()
        || fact.authenticated_at_unix_micros < 0
    {
        return Err(FactRejected::Malformed);
    }
    if echoed != operation {
        return Err(FactRejected::WrongOperation);
    }
    let fact_purpose = Purpose::try_from(fact.purpose).map_err(|_| FactRejected::Malformed)?;
    if fact_purpose != purpose || purpose == Purpose::Unspecified {
        return Err(FactRejected::WrongPurpose);
    }
    match Assurance::try_from(fact.assurance) {
        Ok(Assurance::Password) => Ok(ValidatedFact {
            subject,
            instance_epoch,
            security_version: fact.security_version,
            authenticated_at_unix_micros: fact.authenticated_at_unix_micros,
            purpose: fact_purpose,
            assurance: Assurance::Password,
        }),
        Ok(Assurance::Unspecified) | Err(_) => Err(FactRejected::UnsupportedAssurance),
    }
}

#[cfg(test)]
mod tests;

impl std::fmt::Debug for generated::CanonicalizeMailboxRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CanonicalizeMailboxRequest([REDACTED])")
    }
}
impl std::fmt::Debug for generated::CanonicalizeMailboxResponse {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CanonicalizeMailboxResponse([REDACTED])")
    }
}
impl std::fmt::Debug for generated::VerifiedMailboxFact {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedMailboxFact([REDACTED])")
    }
}
