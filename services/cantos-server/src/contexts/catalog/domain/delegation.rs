//! Work-scoped delegation (ADR 0028): one owner-issued grant lets a registered delegate actor
//! perform closed Catalog actions on exactly one Work. Pure: the shell supplies randomness,
//! authoritative time and current actor/Work/revocation facts; nothing here performs I/O.
use std::collections::BTreeSet;
use std::fmt;

use sha2::{Digest, Sha256};

use super::{content::ContentDetails, Lifecycle, OwnerId, Work, WorkId};
use crate::platform::ids::canonical_uuid;

/// Upper bound of one grant's validity. Shorter lifetimes are chosen per grant by the owner.
pub const MAX_DELEGATION_LIFETIME_MS: i64 = 30 * 24 * 60 * 60 * 1000;
/// Visible purpose prefix for secret scanning; it is not a checksum or authentication.
pub const DELEGATION_TOKEN_PREFIX: &str = "cantos_dlg_";
const SECRET_BYTES: usize = 32;
const LOOKUP_DOMAIN: &[u8] = b"cantos/work-delegation/v1\n";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationError {
    InvalidId,
    UnknownAction,
    NoActions,
    DuplicateAction,
    SelfDelegation,
    InvalidLifetime,
    InvalidToken,
}

/// Why a well-formed grant does not authorize a request. Lifecycle denials make the whole
/// credential unusable; resource and action denials refuse one request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegationDenial {
    Revoked,
    NotYetValid,
    Expired,
    GrantorInactive,
    DelegateInactive,
    OutsideWork,
    GrantorNotOwner,
    WorkArchived,
    ActionNotGranted,
    LinkNotDelegated,
}

/// Closed delegated operations. Review, publication, rights, membership, archive and grant
/// issuance have no variant, so no grant can express them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum DelegatedAction {
    WorkRead,
    ContentRead,
    ContentWrite,
}
impl DelegatedAction {
    pub const ALL: [Self; 3] = [Self::WorkRead, Self::ContentRead, Self::ContentWrite];
    pub fn as_str(self) -> &'static str {
        match self {
            Self::WorkRead => "work:read",
            Self::ContentRead => "content:read",
            Self::ContentWrite => "content:write",
        }
    }
    pub fn parse(value: &str) -> Result<Self, DelegationError> {
        Self::ALL
            .into_iter()
            .find(|action| action.as_str() == value)
            .ok_or(DelegationError::UnknownAction)
    }
}

/// A non-empty set; a duplicate is rejected rather than silently merged.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegatedActions(BTreeSet<DelegatedAction>);
impl DelegatedActions {
    pub fn new(
        actions: impl IntoIterator<Item = DelegatedAction>,
    ) -> Result<Self, DelegationError> {
        let mut set = BTreeSet::new();
        for action in actions {
            if !set.insert(action) {
                return Err(DelegationError::DuplicateAction);
            }
        }
        if set.is_empty() {
            return Err(DelegationError::NoActions);
        }
        Ok(Self(set))
    }
    pub fn contains(&self, action: DelegatedAction) -> bool {
        self.0.contains(&action)
    }
    /// Canonical ascending order, used for responses and request fingerprints.
    pub fn iter(&self) -> impl Iterator<Item = DelegatedAction> + '_ {
        self.0.iter().copied()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct DelegationGrantId(String);
impl DelegationGrantId {
    pub fn parse(value: &str) -> Result<Self, DelegationError> {
        canonical_uuid(value)
            .map(Self)
            .ok_or(DelegationError::InvalidId)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Server-minted 256-bit bearer secret. It is shown once and never persisted; only
/// [`DelegationSecret::lookup_hash`] is stored. `Debug` never prints the secret.
#[derive(Clone, PartialEq, Eq)]
pub struct DelegationSecret([u8; SECRET_BYTES]);
impl DelegationSecret {
    /// The shell supplies bytes from the operating system CSPRNG.
    pub fn from_random(bytes: [u8; SECRET_BYTES]) -> Self {
        Self(bytes)
    }
    /// Exact prefix plus 64 lowercase hex characters; any other spelling is not a credential.
    pub fn parse(bearer: &str) -> Result<Self, DelegationError> {
        let hex = bearer
            .strip_prefix(DELEGATION_TOKEN_PREFIX)
            .filter(|hex| hex.len() == SECRET_BYTES * 2)
            .ok_or(DelegationError::InvalidToken)?;
        let mut bytes = [0; SECRET_BYTES];
        for (byte, pair) in bytes.iter_mut().zip(hex.as_bytes().as_chunks::<2>().0) {
            *byte = (nibble(pair[0])? << 4) | nibble(pair[1])?;
        }
        Ok(Self(bytes))
    }
    pub fn encode(&self) -> String {
        let mut encoded = String::with_capacity(DELEGATION_TOKEN_PREFIX.len() + SECRET_BYTES * 2);
        encoded.push_str(DELEGATION_TOKEN_PREFIX);
        for byte in self.0 {
            encoded.push_str(&format!("{byte:02x}"));
        }
        encoded
    }
    /// Purpose-separated from Studio sessions and producer grants, which hash other inputs.
    pub fn lookup_hash(&self) -> [u8; 32] {
        let mut hash = Sha256::new();
        hash.update(LOOKUP_DOMAIN);
        hash.update(self.0);
        hash.finalize().into()
    }
}
impl fmt::Debug for DelegationSecret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("DelegationSecret(<redacted>)")
    }
}
fn nibble(byte: u8) -> Result<u8, DelegationError> {
    match byte {
        b'0'..=b'9' => Ok(byte - b'0'),
        b'a'..=b'f' => Ok(byte - b'a' + 10),
        _ => Err(DelegationError::InvalidToken),
    }
}

/// Owner-supplied grant intent. The grantor is the authenticated owner, never request input.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GrantRequest {
    pub id: DelegationGrantId,
    pub work_id: WorkId,
    pub delegate: OwnerId,
    pub actions: DelegatedActions,
    pub expires_at: i64,
}

impl GrantRequest {
    /// Retry fingerprint over every owner-chosen field in canonical form; the secret is never
    /// an input. Each field's alphabet excludes the newline and comma separators.
    pub fn digest(&self) -> String {
        let actions: Vec<_> = self.actions.iter().map(DelegatedAction::as_str).collect();
        let mut hash = Sha256::new();
        hash.update(b"cantos/delegation-request/v1\n");
        hash.update(format!(
            "{}\n{}\n{}\n{}\n{}",
            self.work_id.as_str(),
            self.id.as_str(),
            self.delegate.as_str(),
            actions.join(","),
            self.expires_at
        ));
        format!("dg-request1:sha256:{:x}", hash.finalize())
    }
}

/// Immutable issued scope. Constructed only by [`DelegationGrant::issue`] or by
/// [`DelegationGrant::restore`] from a stored row, both enforcing the same invariants.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationGrant {
    id: DelegationGrantId,
    work_id: WorkId,
    grantor: OwnerId,
    delegate: OwnerId,
    actions: DelegatedActions,
    issued_at: i64,
    expires_at: i64,
}
impl DelegationGrant {
    pub fn issue(
        request: GrantRequest,
        grantor: OwnerId,
        now: i64,
    ) -> Result<Self, DelegationError> {
        Self::restore(
            request.id,
            request.work_id,
            grantor,
            request.delegate,
            request.actions,
            now,
            request.expires_at,
        )
    }
    pub fn restore(
        id: DelegationGrantId,
        work_id: WorkId,
        grantor: OwnerId,
        delegate: OwnerId,
        actions: DelegatedActions,
        issued_at: i64,
        expires_at: i64,
    ) -> Result<Self, DelegationError> {
        if grantor == delegate {
            return Err(DelegationError::SelfDelegation);
        }
        let lifetime = expires_at.checked_sub(issued_at);
        if issued_at < 0 || !lifetime.is_some_and(|ms| 0 < ms && ms <= MAX_DELEGATION_LIFETIME_MS) {
            return Err(DelegationError::InvalidLifetime);
        }
        Ok(Self {
            id,
            work_id,
            grantor,
            delegate,
            actions,
            issued_at,
            expires_at,
        })
    }
    pub fn id(&self) -> &DelegationGrantId {
        &self.id
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub fn grantor(&self) -> &OwnerId {
        &self.grantor
    }
    pub fn delegate(&self) -> &OwnerId {
        &self.delegate
    }
    pub fn actions(&self) -> &DelegatedActions {
        &self.actions
    }
    pub fn issued_at(&self) -> i64 {
        self.issued_at
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }

    /// Credential lifecycle, independent of the requested resource. Every fact is current.
    pub fn admit(&self, facts: &CurrentFacts) -> Result<(), DelegationDenial> {
        if facts.revoked {
            return Err(DelegationDenial::Revoked);
        }
        if facts.now < self.issued_at {
            return Err(DelegationDenial::NotYetValid);
        }
        if facts.now >= self.expires_at {
            return Err(DelegationDenial::Expired);
        }
        if !facts.grantor_active {
            return Err(DelegationDenial::GrantorInactive);
        }
        if !facts.delegate_active {
            return Err(DelegationDenial::DelegateInactive);
        }
        Ok(())
    }

    /// The authoritative Work of the requested resource must be exactly the granted Work,
    /// still owned by the grantor and not archived. Foreign Works are indistinguishable from
    /// missing ones, so they are checked before any fact about that Work is used.
    pub fn admit_work(&self, facts: &CurrentFacts, work: &Work) -> Result<(), DelegationDenial> {
        self.admit(facts)?;
        if work.id() != &self.work_id {
            return Err(DelegationDenial::OutsideWork);
        }
        if work.owner_id() != &self.grantor {
            return Err(DelegationDenial::GrantorNotOwner);
        }
        if work.lifecycle() == Lifecycle::Archived {
            return Err(DelegationDenial::WorkArchived);
        }
        Ok(())
    }

    pub fn authorize(
        &self,
        facts: &CurrentFacts,
        work: &Work,
        action: DelegatedAction,
    ) -> Result<DelegatedPermit, DelegationDenial> {
        self.admit_work(facts, work)?;
        if !self.actions.contains(action) {
            return Err(DelegationDenial::ActionNotGranted);
        }
        Ok(DelegatedPermit {
            grant: self.id.clone(),
            delegate: self.delegate.clone(),
            action,
        })
    }
}

/// Current facts resolved by the shell under the Catalog fence, never from the token.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CurrentFacts {
    pub now: i64,
    pub revoked: bool,
    pub grantor_active: bool,
    pub delegate_active: bool,
}

/// Request-bound proof that one action passed every check. Private fields: only
/// [`DelegationGrant::authorize`] can construct it, and it is never serialized.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegatedPermit {
    grant: DelegationGrantId,
    delegate: OwnerId,
    action: DelegatedAction,
}
impl DelegatedPermit {
    pub fn grant(&self) -> &DelegationGrantId {
        &self.grant
    }
    /// The recorded actor of a delegated effect: the delegate, never the grantor.
    pub fn delegate(&self) -> &OwnerId {
        &self.delegate
    }
    pub fn action(&self) -> DelegatedAction {
        self.action
    }
}

/// A delegated write may carry only text or a recording: Source and Script links belong to
/// Ingestion/Editorial rights that a Catalog grant does not convey. Rights references keep
/// Catalog's existing owner-only change rule, which the application applies to every writer.
pub fn delegated_content_admissible(details: &ContentDetails) -> Result<(), DelegationDenial> {
    if details.body().source().is_some() || details.body().script().is_some() {
        return Err(DelegationDenial::LinkNotDelegated);
    }
    Ok(())
}

#[cfg(test)]
#[path = "delegation_tests.rs"]
mod tests;
