//! Pure mailbox-proof transition plans, not proof verification or persisted consumption.
//!
//! The owner must verify the secret, derive the candidate from trusted state, then compare
//! the entire expected snapshot and commit its successor atomically. A copied/stale plan
//! cannot provide a one-winner guarantee. No transition grants setup or login authority.

use std::fmt;

use super::{
    admission::Purpose,
    ids::{InstanceEpoch, NilIdentifier, OperationId, SubjectId},
    security_version::SecurityVersion,
};

/// Public challenge locator, distinct from both the secret proof and its digest.
/// Construction validates shape only; possession of this ID proves nothing.
///
/// ```compile_fail
/// use cantos_identity::domain::{ids::OperationId, mailbox_proof::ProofId};
/// let operation = OperationId::from_bytes([1; 16]).unwrap();
/// let proof: ProofId = operation;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ProofId(u128);

impl ProofId {
    pub const fn from_bytes(bytes: [u8; 16]) -> Result<Self, NilIdentifier> {
        let value = u128::from_be_bytes(bytes);
        if value == 0 {
            return Err(NilIdentifier);
        }
        Ok(Self(value))
    }

    pub const fn to_bytes(self) -> [u8; 16] {
        self.0.to_be_bytes()
    }
}

/// Unix seconds within the nonnegative Turso INTEGER range, supplied by the owner clock.
/// Parsing this number does not establish that the clock is trusted or has not rolled back.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ProofTime(u64);

impl ProofTime {
    pub const fn new(seconds: u64) -> Result<Self, InvalidProofTime> {
        if seconds > i64::MAX as u64 {
            return Err(InvalidProofTime);
        }
        Ok(Self(seconds))
    }

    pub const fn seconds(self) -> u64 {
        self.0
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InvalidProofTime;

impl fmt::Display for InvalidProofTime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("mailbox proof time is outside the storage range")
    }
}

impl std::error::Error for InvalidProofTime {}

/// There is no Studio public-signup variant. Recovery remains a separate flow, carrying
/// the exact subject/security version and surface; it never satisfies passkey assurance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofIntent {
    TheatreSignup,
    CredentialRecovery {
        subject: SubjectId,
        version: SecurityVersion,
        purpose: Purpose,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProofBinding {
    operation: OperationId,
    epoch: InstanceEpoch,
    intent: ProofIntent,
}

impl ProofBinding {
    pub const fn new(operation: OperationId, epoch: InstanceEpoch, intent: ProofIntent) -> Self {
        Self {
            operation,
            epoch,
            intent,
        }
    }

    pub const fn operation(self) -> OperationId {
        self.operation
    }

    pub const fn epoch(self) -> InstanceEpoch {
        self.epoch
    }

    pub const fn intent(self) -> ProofIntent {
        self.intent
    }
}

/// Adapter-established action. GET, prefetch, polling and provider-status observation must
/// map to Inspect; an untrusted client flag cannot establish ExplicitConfirmation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofAction {
    Inspect,
    ExplicitConfirmation,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofState {
    Pending,
    Consumed,
    Cancelled,
}

/// Immutable, shape-valid snapshot, never a verified mailbox or authentication fact.
/// The future persistence adapter must validate stored rows rather than reconstructing
/// every row as Pending through `issue`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MailboxProof {
    id: ProofId,
    binding: ProofBinding,
    issued_at: ProofTime,
    expires_at: ProofTime,
    state: ProofState,
}

/// Internal reasons. A public/private RPC boundary must project generic refusal.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProofDenied {
    InvalidWindow,
    NotExplicitConfirmation,
    BindingChanged,
    ProofChanged,
    BeforeIssue,
    Expired,
    Consumed,
    Cancelled,
    ReusedProofId,
    InvalidCooldown,
    Cooldown,
}

impl fmt::Display for ProofDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::InvalidWindow => "mailbox proof expiry must follow issuance",
            Self::NotExplicitConfirmation => "explicit mailbox confirmation is required",
            Self::BindingChanged => "mailbox proof binding changed",
            Self::ProofChanged => "mailbox proof was replaced",
            Self::BeforeIssue => "mailbox proof time precedes issuance",
            Self::Expired => "mailbox proof expired",
            Self::Consumed => "mailbox proof is already consumed",
            Self::Cancelled => "mailbox proof is cancelled",
            Self::ReusedProofId => "replacement requires a fresh mailbox proof identifier",
            Self::InvalidCooldown => "mailbox proof cooldown must be positive",
            Self::Cooldown => "mailbox proof replacement is too early",
        })
    }
}

impl std::error::Error for ProofDenied {}

/// Compare-and-replace data only. The shell must compare `expected` with the entire
/// authoritative current record inside the shared transaction owner, persist `next` and
/// the operation receipt, and reconcile an unknown outcome before reusing any plan.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[must_use = "a transition plan has no effect until the owner atomically applies it"]
pub struct ProofTransition {
    expected: MailboxProof,
    next: MailboxProof,
}

impl ProofTransition {
    pub const fn expected(self) -> MailboxProof {
        self.expected
    }

    pub const fn next(self) -> MailboxProof {
        self.next
    }
}

impl MailboxProof {
    pub fn issue(
        id: ProofId,
        binding: ProofBinding,
        issued_at: ProofTime,
        expires_at: ProofTime,
    ) -> Result<Self, ProofDenied> {
        Self::restore(id, binding, issued_at, expires_at, ProofState::Pending)
    }

    /// Checked owner-only hydration preserves terminal state instead of reopening Pending.
    /// Shape validation supplies no proof possession, caller authority or setup capability.
    pub(crate) fn restore(
        id: ProofId,
        binding: ProofBinding,
        issued_at: ProofTime,
        expires_at: ProofTime,
        state: ProofState,
    ) -> Result<Self, ProofDenied> {
        if expires_at <= issued_at {
            return Err(ProofDenied::InvalidWindow);
        }
        Ok(Self {
            id,
            binding,
            issued_at,
            expires_at,
            state,
        })
    }

    pub const fn state(self) -> ProofState {
        self.state
    }

    pub const fn id(self) -> ProofId {
        self.id
    }

    pub const fn binding(self) -> ProofBinding {
        self.binding
    }

    pub const fn issued_at(self) -> ProofTime {
        self.issued_at
    }

    pub const fn expires_at(self) -> ProofTime {
        self.expires_at
    }

    /// `matched_id` is the locator derived AFTER actual secret verification. The binding
    /// must come from the current workflow/account and DB epoch, never an echoed request.
    /// This method itself verifies no token, caller, account eligibility or HTTP request.
    pub fn plan_confirmation(
        self,
        matched_id: ProofId,
        current: ProofBinding,
        action: ProofAction,
        now: ProofTime,
    ) -> Result<ProofTransition, ProofDenied> {
        if action != ProofAction::ExplicitConfirmation {
            return Err(ProofDenied::NotExplicitConfirmation);
        }
        self.check_pending(matched_id, current)?;
        if now < self.issued_at {
            return Err(ProofDenied::BeforeIssue);
        }
        if now >= self.expires_at {
            return Err(ProofDenied::Expired);
        }
        Ok(self.transition(Self {
            state: ProofState::Consumed,
            ..self
        }))
    }

    /// Cancellation ends this operation's challenge. A later signup/recovery needs a new
    /// operation; resending cannot resurrect a cancelled or consumed snapshot.
    pub fn plan_cancellation(
        self,
        expected_id: ProofId,
        current: ProofBinding,
    ) -> Result<ProofTransition, ProofDenied> {
        self.check_pending(expected_id, current)?;
        Ok(self.transition(Self {
            state: ProofState::Cancelled,
            ..self
        }))
    }

    /// Policy values are explicit owner inputs. No TTL, send quota or account/IP budget is
    /// silently selected here. Expired pending proofs may be replaced after the cooldown.
    /// Freshness also requires a new secret/digest and durable uniqueness, outside this core.
    pub fn plan_replacement(
        self,
        expected_id: ProofId,
        current: ProofBinding,
        replacement_id: ProofId,
        now: ProofTime,
        expires_at: ProofTime,
        cooldown_seconds: u32,
    ) -> Result<ProofTransition, ProofDenied> {
        self.check_pending(expected_id, current)?;
        if replacement_id == self.id {
            return Err(ProofDenied::ReusedProofId);
        }
        if cooldown_seconds == 0 {
            return Err(ProofDenied::InvalidCooldown);
        }
        let elapsed = now
            .seconds()
            .checked_sub(self.issued_at.seconds())
            .ok_or(ProofDenied::BeforeIssue)?;
        if elapsed < u64::from(cooldown_seconds) {
            return Err(ProofDenied::Cooldown);
        }
        let next = Self::issue(replacement_id, self.binding, now, expires_at)?;
        Ok(self.transition(next))
    }

    fn check_pending(self, id: ProofId, binding: ProofBinding) -> Result<(), ProofDenied> {
        if binding != self.binding {
            return Err(ProofDenied::BindingChanged);
        }
        if id != self.id {
            return Err(ProofDenied::ProofChanged);
        }
        match self.state {
            ProofState::Pending => Ok(()),
            ProofState::Consumed => Err(ProofDenied::Consumed),
            ProofState::Cancelled => Err(ProofDenied::Cancelled),
        }
    }

    fn transition(self, next: Self) -> ProofTransition {
        ProofTransition {
            expected: self,
            next,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs;
