//! Pure decisions over verified Identity facts: no clock, socket or storage.
//!
//! A snapshot is evidence for exactly the instant it was read. These functions turn a verified
//! authentication fact and a fresh security-state read into one closed outcome; the shell
//! executes it. Nothing here proves a password, a mailbox or any resource permission.
use crate::platform::identity_auth::rpc::{
    AccountState, GetSecurityStateResponse, ValidatedFact, ID_BYTES,
};

/// Largest age of an authentication fact the Server will still turn into a session.
pub const FACT_MAX_AGE_MICROS: i64 = 30_000_000;
/// Tolerated clock skew for a fact stamped slightly ahead of the Server clock. Both processes
/// share a host over a Unix socket, so this only absorbs clock granularity.
pub const FACT_MAX_FUTURE_MICROS: i64 = 5_000_000;

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct Subject([u8; ID_BYTES]);

#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Epoch([u8; ID_BYTES]);

impl std::fmt::Debug for Subject {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Subject([redacted])")
    }
}
impl std::fmt::Debug for Epoch {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Epoch([redacted])")
    }
}

fn nonzero(bytes: &[u8]) -> Option<[u8; ID_BYTES]> {
    let id: [u8; ID_BYTES] = bytes.try_into().ok()?;
    (id != [0; ID_BYTES]).then_some(id)
}

impl Subject {
    pub fn from_bytes(bytes: [u8; ID_BYTES]) -> Option<Self> {
        (bytes != [0; ID_BYTES]).then_some(Self(bytes))
    }
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        nonzero(bytes).map(Self)
    }
    /// 32 lowercase hex characters: the external subject spelling in the Actor Registry.
    pub fn hex(self) -> String {
        hex(&self.0)
    }
    pub fn from_hex(value: &str) -> Option<Self> {
        unhex(value).and_then(Self::from_bytes)
    }
    pub fn bytes(self) -> [u8; ID_BYTES] {
        self.0
    }
}

impl Epoch {
    pub fn from_slice(bytes: &[u8]) -> Option<Self> {
        nonzero(bytes).map(Self)
    }
    pub fn from_bytes(bytes: [u8; ID_BYTES]) -> Option<Self> {
        (bytes != [0; ID_BYTES]).then_some(Self(bytes))
    }
    pub fn bytes(self) -> [u8; ID_BYTES] {
        self.0
    }
}

pub(crate) fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

pub(crate) fn unhex<const N: usize>(value: &str) -> Option<[u8; N]> {
    if value.len() != N * 2
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return None;
    }
    let mut out = [0; N];
    for (index, byte) in out.iter_mut().enumerate() {
        *byte = u8::from_str_radix(&value[index * 2..index * 2 + 2], 16).ok()?;
    }
    Some(out)
}

/// A decoded `GetSecurityState` whose shape was checked and whose subject is the one asked for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecurityState {
    pub subject: Subject,
    pub active: bool,
    pub version: u64,
    pub epoch: Epoch,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MalformedState;

impl SecurityState {
    pub fn from_response(
        response: &GetSecurityStateResponse,
        asked: Subject,
    ) -> Result<Self, MalformedState> {
        let subject = Subject::from_slice(&response.subject_id).ok_or(MalformedState)?;
        let epoch = Epoch::from_slice(&response.instance_epoch).ok_or(MalformedState)?;
        if subject != asked
            || response.security_version == 0
            || i64::try_from(response.security_version).is_err()
        {
            return Err(MalformedState);
        }
        let active = match AccountState::try_from(response.state) {
            Ok(AccountState::Active) => true,
            Ok(AccountState::Disabled) => false,
            Ok(AccountState::Unspecified) | Err(_) => return Err(MalformedState),
        };
        Ok(Self {
            subject,
            active,
            version: response.security_version,
            epoch,
        })
    }
}

/// What a session observed when it was issued. Immutable for the life of the session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ObservedSecurity {
    pub subject: Subject,
    pub epoch: Epoch,
    pub version: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IssueRefusal {
    FactFromFuture,
    FactStale,
    SubjectChanged,
    Inactive,
    EpochChanged,
    VersionChanged,
}

/// Issue a session only from a fact that is recent and whose subject/epoch/version a fresh read
/// still confirms. The fact is not a license after this instant: admission re-reads the state.
pub fn decide_issue(
    fact: &ValidatedFact,
    current: &SecurityState,
    now_micros: i64,
) -> Result<ObservedSecurity, IssueRefusal> {
    let subject = Subject::from_bytes(fact.subject).ok_or(IssueRefusal::SubjectChanged)?;
    let epoch = Epoch::from_bytes(fact.instance_epoch).ok_or(IssueRefusal::EpochChanged)?;
    if current.subject != subject {
        return Err(IssueRefusal::SubjectChanged);
    }
    // Checked arithmetic: both operands are untrusted wire integers.
    let age = now_micros
        .checked_sub(fact.authenticated_at_unix_micros)
        .ok_or(IssueRefusal::FactStale)?;
    if age < -FACT_MAX_FUTURE_MICROS {
        return Err(IssueRefusal::FactFromFuture);
    }
    if age > FACT_MAX_AGE_MICROS {
        return Err(IssueRefusal::FactStale);
    }
    if !current.active {
        return Err(IssueRefusal::Inactive);
    }
    if current.epoch != epoch {
        return Err(IssueRefusal::EpochChanged);
    }
    if current.version != fact.security_version {
        return Err(IssueRefusal::VersionChanged);
    }
    Ok(ObservedSecurity {
        subject,
        epoch,
        version: fact.security_version,
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RevokeReason {
    SubjectMismatch,
    /// Identity no longer knows the subject (for example a restored older file).
    SubjectMissing,
    Disabled,
    EpochChanged,
    VersionAdvanced,
    VersionRegressed,
}

impl RevokeReason {
    pub(crate) fn stored(self) -> &'static str {
        match self {
            Self::SubjectMismatch => "subject-mismatch",
            Self::SubjectMissing => "subject-missing",
            Self::Disabled => "disabled",
            Self::EpochChanged => "epoch-changed",
            Self::VersionAdvanced => "version-advanced",
            Self::VersionRegressed => "version-regressed",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    Admit,
    /// The session can never be admitted again; the shell revokes its family durably.
    Revoke(RevokeReason),
}

/// Admit only an Active subject whose instance epoch and security version equal what the
/// session observed. Any difference, including a *lower* version (a restored older database),
/// is permanent: a version that later happens to match again never resurrects a revoked family.
pub fn decide_admission(observed: &ObservedSecurity, current: &SecurityState) -> Admission {
    if current.subject != observed.subject {
        return Admission::Revoke(RevokeReason::SubjectMismatch);
    }
    if !current.active {
        return Admission::Revoke(RevokeReason::Disabled);
    }
    if current.epoch != observed.epoch {
        return Admission::Revoke(RevokeReason::EpochChanged);
    }
    match current.version.cmp(&observed.version) {
        std::cmp::Ordering::Equal => Admission::Admit,
        std::cmp::Ordering::Greater => Admission::Revoke(RevokeReason::VersionAdvanced),
        std::cmp::Ordering::Less => Admission::Revoke(RevokeReason::VersionRegressed),
    }
}

#[cfg(test)]
mod tests;

// Test-only event observer for the registered Quint capability; never part of a release build.
#[cfg(test)]
mod quint_conformance_tests;
