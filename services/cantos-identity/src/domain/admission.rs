//! Recheck security state after expensive verification and before admitting its completion.
//!
//! An `Ok(())` is only this decision's result, never a credential or AuthenticationFact.
//! The shell must establish verifier facts, authenticate the caller, enforce deadlines and
//! one-use operation receipts, and serialize this check with security-changing writes.

use std::fmt;

use super::{
    ids::{InstanceEpoch, OperationId, SubjectId},
    security_version::SecurityVersion,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Purpose {
    Theatre,
    Studio,
}

/// Categorical verifier facts, deliberately not an ordered score. A recovery code or mailbox
/// proof cannot become a second factor by arithmetic or by combining caller-supplied flags.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Assurance {
    Password,
    UserVerifiedPasskey,
    PasskeyWithoutUserVerification,
    RecoveryCode,
    MailboxProof,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccountState {
    Active,
    Disabled,
    Deleted,
    Tombstoned,
}

/// Shape-valid state read by the owner. Construction does not authenticate a subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SecuritySnapshot {
    subject: SubjectId,
    epoch: InstanceEpoch,
    version: SecurityVersion,
    state: AccountState,
}

impl SecuritySnapshot {
    pub const fn new(
        subject: SubjectId,
        epoch: InstanceEpoch,
        version: SecurityVersion,
        state: AccountState,
    ) -> Self {
        Self {
            subject,
            epoch,
            version,
            state,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequestBinding {
    operation: OperationId,
    purpose: Purpose,
}

impl RequestBinding {
    pub const fn new(operation: OperationId, purpose: Purpose) -> Self {
        Self { operation, purpose }
    }
}

/// Internal reasons only. A public/RPC adapter must project generic denial, not account state.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CompletionDenied {
    WrongOperation,
    WrongPurpose,
    SubjectChanged,
    EpochChanged,
    InactiveAccount,
    SecurityVersionChanged,
    InsufficientAssurance,
}

impl fmt::Display for CompletionDenied {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::WrongOperation => "authentication operation changed",
            Self::WrongPurpose => "authentication purpose changed",
            Self::SubjectChanged => "authentication subject changed",
            Self::EpochChanged => "identity instance changed",
            Self::InactiveAccount => "account is not active",
            Self::SecurityVersionChanged => "account security version changed",
            Self::InsufficientAssurance => "authentication assurance is insufficient",
        })
    }
}

impl std::error::Error for CompletionDenied {}

/// Evaluate only fresh, active, identically bound completions. Theatre's password path and
/// Studio's user-verified passkey path are distinct. Recovery proceeds through a separate flow.
/// No credential, caller, replay, persistence, freshness lease or session is verified here.
pub fn check_completion(
    started: SecuritySnapshot,
    expected: RequestBinding,
    current: SecuritySnapshot,
    completed: RequestBinding,
    assurance: Assurance,
) -> Result<(), CompletionDenied> {
    if expected.operation != completed.operation {
        return Err(CompletionDenied::WrongOperation);
    }
    if expected.purpose != completed.purpose {
        return Err(CompletionDenied::WrongPurpose);
    }
    if started.subject != current.subject {
        return Err(CompletionDenied::SubjectChanged);
    }
    if started.epoch != current.epoch {
        return Err(CompletionDenied::EpochChanged);
    }
    if started.state != AccountState::Active || current.state != AccountState::Active {
        return Err(CompletionDenied::InactiveAccount);
    }
    if started.version != current.version {
        return Err(CompletionDenied::SecurityVersionChanged);
    }
    match (expected.purpose, assurance) {
        (Purpose::Theatre, Assurance::Password)
        | (Purpose::Studio, Assurance::UserVerifiedPasskey) => Ok(()),
        (Purpose::Theatre, Assurance::UserVerifiedPasskey)
        | (Purpose::Studio, Assurance::Password)
        | (_, Assurance::PasskeyWithoutUserVerification)
        | (_, Assurance::RecoveryCode)
        | (_, Assurance::MailboxProof) => Err(CompletionDenied::InsufficientAssurance),
    }
}

#[cfg(test)]
mod tests;

#[cfg(kani)]
mod kani_proofs {
    use super::{
        check_completion, AccountState, Assurance, CompletionDenied, Purpose, RequestBinding,
        SecuritySnapshot,
    };
    use crate::domain::{
        ids::{InstanceEpoch, OperationId, SubjectId},
        security_version::SecurityVersion,
    };

    /// Symbolic namespace values and full valid u64 security generations; state/factor/surface
    /// alphabets are exhaustive. Only assumptions are the checked constructors' input domains.
    /// No I/O, stubs or crypto claims. Independent predicate oracle; unwind 2, no business loop.
    #[kani::proof]
    #[kani::unwind(2)]
    fn completion_requires_current_active_bound_security_state() {
        let subject: u128 = kani::any();
        let other_subject: u128 = kani::any();
        let epoch: u128 = kani::any();
        let other_epoch: u128 = kani::any();
        let operation: u128 = kani::any();
        let other_operation: u128 = kani::any();
        let version: u64 = kani::any();
        let other_version: u64 = kani::any();
        kani::assume(subject > 0 && other_subject > 0);
        kani::assume(epoch > 0 && other_epoch > 0);
        kani::assume(operation > 0 && other_operation > 0);
        kani::assume(version > 0 && version <= i64::MAX as u64);
        kani::assume(other_version > 0 && other_version <= i64::MAX as u64);
        let state = match kani::any::<u8>() {
            0 => AccountState::Active,
            1 => AccountState::Disabled,
            2 => AccountState::Deleted,
            _ => AccountState::Tombstoned,
        };
        let other_state = match kani::any::<u8>() {
            0 => AccountState::Active,
            1 => AccountState::Disabled,
            2 => AccountState::Deleted,
            _ => AccountState::Tombstoned,
        };
        let studio: bool = kani::any();
        let other_studio: bool = kani::any();
        let factor: u8 = kani::any();
        kani::assume(factor < 5);
        let assurance = match factor {
            0 => Assurance::Password,
            1 => Assurance::UserVerifiedPasskey,
            2 => Assurance::PasskeyWithoutUserVerification,
            3 => Assurance::RecoveryCode,
            _ => Assurance::MailboxProof,
        };
        let purpose = if studio {
            Purpose::Studio
        } else {
            Purpose::Theatre
        };
        let other_purpose = if other_studio {
            Purpose::Studio
        } else {
            Purpose::Theatre
        };
        let started = SecuritySnapshot::new(
            SubjectId::from_bytes(subject.to_be_bytes()).unwrap(),
            InstanceEpoch::from_bytes(epoch.to_be_bytes()).unwrap(),
            SecurityVersion::new(version).unwrap(),
            state,
        );
        let current = SecuritySnapshot::new(
            SubjectId::from_bytes(other_subject.to_be_bytes()).unwrap(),
            InstanceEpoch::from_bytes(other_epoch.to_be_bytes()).unwrap(),
            SecurityVersion::new(other_version).unwrap(),
            other_state,
        );
        let expected = RequestBinding::new(
            OperationId::from_bytes(operation.to_be_bytes()).unwrap(),
            purpose,
        );
        let completed = RequestBinding::new(
            OperationId::from_bytes(other_operation.to_be_bytes()).unwrap(),
            other_purpose,
        );
        let result = check_completion(started, expected, current, completed, assurance);
        let same_binding = operation == other_operation && studio == other_studio;
        let same_state =
            subject == other_subject && epoch == other_epoch && version == other_version;
        let active = state == AccountState::Active && other_state == AccountState::Active;
        let sufficient = (!studio && factor == 0) || (studio && factor == 1);
        assert_eq!(
            result.is_ok(),
            same_binding && same_state && active && sufficient
        );
        kani::cover!(result.is_ok() && !studio, "Theatre password completion");
        kani::cover!(
            result.is_ok() && studio,
            "Studio verified passkey completion"
        );
        kani::cover!(
            result == Err(CompletionDenied::WrongOperation),
            "late operation"
        );
        kani::cover!(
            result == Err(CompletionDenied::WrongPurpose),
            "wrong surface"
        );
        kani::cover!(
            result == Err(CompletionDenied::SubjectChanged),
            "different subject"
        );
        kani::cover!(
            result == Err(CompletionDenied::EpochChanged),
            "restored instance"
        );
        kani::cover!(
            result == Err(CompletionDenied::InactiveAccount),
            "inactive account"
        );
        kani::cover!(
            result == Err(CompletionDenied::SecurityVersionChanged),
            "stale credential result"
        );
        kani::cover!(
            result == Err(CompletionDenied::InsufficientAssurance) && factor == 3,
            "recovery code is insufficient"
        );
        kani::cover!(
            result == Err(CompletionDenied::InsufficientAssurance) && factor == 2,
            "passkey requires user verification"
        );
    }
}
