//! The pure decision behind `StartTheatreSignup`: issue a first proof, replace a cooled one or
//! do nothing. It reads facts the owner observed inside its own writer transaction and returns a
//! closed outcome; it performs no I/O and grants no authority.
//!
//! Every non-mutating outcome is answered to the caller exactly like a sent message, so the
//! variants exist for the owner's tests and logs, never for the wire.

use super::{
    email::EmailAddress,
    mailbox_proof::{MailboxProof, ProofId, ProofIntent, ProofState},
};

/// What the writer found stored under the requested operation id.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoredOperation {
    /// No workflow of any kind uses this id.
    Unused,
    /// A Theatre signup: its immutable mailbox binding and the proof currently guarding it.
    Signup {
        email: EmailAddress,
        proof: MailboxProof,
    },
    /// A workflow that is not a Theatre signup uses this id.
    Foreign,
}

/// Authoritative state at the instant of the decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StartFacts<'a> {
    pub requested: &'a EmailAddress,
    pub requested_registered: bool,
    pub stored: &'a StoredOperation,
    pub now_seconds: u64,
    pub cooldown_seconds: u32,
}

/// Why a start changed nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartSkip {
    /// The requested address already has an account: it is never reset, linked or mailed.
    EmailRegistered,
    /// The operation is bound to another address, or is not a Theatre signup.
    OperationBound,
    /// The operation's proof was consumed or cancelled: a concluded operation never restarts.
    Concluded,
    /// The current proof is younger than the resend cooldown.
    Cooling,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StartDecision {
    /// Bind the operation to the address and issue its first proof.
    Issue,
    /// Retire `expected` and issue a replacement for the same binding.
    Replace {
        expected: ProofId,
    },
    Skip(StartSkip),
}

pub fn decide_start(facts: &StartFacts<'_>) -> StartDecision {
    if facts.requested_registered {
        return StartDecision::Skip(StartSkip::EmailRegistered);
    }
    let (email, proof) = match facts.stored {
        StoredOperation::Unused => return StartDecision::Issue,
        StoredOperation::Foreign => return StartDecision::Skip(StartSkip::OperationBound),
        StoredOperation::Signup { email, proof } => (email, proof),
    };
    if email != facts.requested || proof.binding().intent() != ProofIntent::TheatreSignup {
        return StartDecision::Skip(StartSkip::OperationBound);
    }
    if proof.state() != ProofState::Pending {
        return StartDecision::Skip(StartSkip::Concluded);
    }
    let cooled = facts
        .now_seconds
        .checked_sub(proof.issued_at().seconds())
        .is_some_and(|elapsed| elapsed >= u64::from(facts.cooldown_seconds));
    if cooled {
        StartDecision::Replace {
            expected: proof.id(),
        }
    } else {
        StartDecision::Skip(StartSkip::Cooling)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{
        ids::{InstanceEpoch, OperationId, SubjectId},
        mailbox_proof::{ProofBinding, ProofTime},
        security_version::SecurityVersion,
    };

    const COOLDOWN: u32 = 60;
    const ISSUED: u64 = 1_000;

    fn email(text: &str) -> EmailAddress {
        EmailAddress::parse(text).unwrap()
    }

    fn proof(state: ProofState, intent: ProofIntent) -> MailboxProof {
        MailboxProof::restore(
            ProofId::from_bytes([3; 16]).unwrap(),
            ProofBinding::new(
                OperationId::from_bytes([1; 16]).unwrap(),
                InstanceEpoch::from_bytes([2; 16]).unwrap(),
                intent,
            ),
            ProofTime::new(ISSUED).unwrap(),
            ProofTime::new(ISSUED + 900).unwrap(),
            state,
        )
        .unwrap()
    }

    fn signup(address: &str, state: ProofState) -> StoredOperation {
        StoredOperation::Signup {
            email: email(address),
            proof: proof(state, ProofIntent::TheatreSignup),
        }
    }

    fn decide(
        requested: &str,
        registered: bool,
        stored: &StoredOperation,
        now: u64,
    ) -> StartDecision {
        decide_start(&StartFacts {
            requested: &email(requested),
            requested_registered: registered,
            stored,
            now_seconds: now,
            cooldown_seconds: COOLDOWN,
        })
    }

    #[test]
    fn an_unused_operation_issues_unless_the_address_is_registered() {
        let unused = StoredOperation::Unused;
        assert_eq!(
            decide("a@example.com", false, &unused, 0),
            StartDecision::Issue
        );
        assert_eq!(
            decide("a@example.com", true, &unused, 0),
            StartDecision::Skip(StartSkip::EmailRegistered)
        );
    }

    #[test]
    fn registration_wins_over_every_stored_operation_state() {
        for stored in [
            StoredOperation::Unused,
            StoredOperation::Foreign,
            signup("a@example.com", ProofState::Pending),
            signup("b@example.com", ProofState::Consumed),
        ] {
            assert_eq!(
                decide("a@example.com", true, &stored, ISSUED + 10_000),
                StartDecision::Skip(StartSkip::EmailRegistered)
            );
        }
    }

    #[test]
    fn an_operation_bound_elsewhere_never_changes() {
        let foreign = StoredOperation::Foreign;
        assert_eq!(
            decide("a@example.com", false, &foreign, ISSUED + 10_000),
            StartDecision::Skip(StartSkip::OperationBound)
        );
        let other = signup("b@example.com", ProofState::Pending);
        assert_eq!(
            decide("a@example.com", false, &other, ISSUED + 10_000),
            StartDecision::Skip(StartSkip::OperationBound)
        );
        // A signup row whose proof is not a signup proof is not a signup binding.
        let recovery = StoredOperation::Signup {
            email: email("a@example.com"),
            proof: proof(
                ProofState::Pending,
                ProofIntent::CredentialRecovery {
                    subject: SubjectId::from_bytes([5; 16]).unwrap(),
                    version: SecurityVersion::new(1).unwrap(),
                    purpose: crate::domain::admission::Purpose::Theatre,
                },
            ),
        };
        assert_eq!(
            decide("a@example.com", false, &recovery, ISSUED + 10_000),
            StartDecision::Skip(StartSkip::OperationBound)
        );
    }

    #[test]
    fn a_concluded_proof_is_never_restarted() {
        for state in [ProofState::Consumed, ProofState::Cancelled] {
            assert_eq!(
                decide(
                    "a@example.com",
                    false,
                    &signup("a@example.com", state),
                    ISSUED + 10_000
                ),
                StartDecision::Skip(StartSkip::Concluded)
            );
        }
    }

    #[test]
    fn a_pending_proof_is_replaced_exactly_from_the_cooldown_boundary() {
        let stored = signup("a@example.com", ProofState::Pending);
        let expected = ProofId::from_bytes([3; 16]).unwrap();
        for (now, decision) in [
            (0, StartDecision::Skip(StartSkip::Cooling)), // clock before issue
            (ISSUED - 1, StartDecision::Skip(StartSkip::Cooling)),
            (ISSUED, StartDecision::Skip(StartSkip::Cooling)),
            (ISSUED + 59, StartDecision::Skip(StartSkip::Cooling)),
            (ISSUED + 60, StartDecision::Replace { expected }),
            (ISSUED + 61, StartDecision::Replace { expected }),
            (u64::MAX, StartDecision::Replace { expected }),
        ] {
            assert_eq!(
                decide("a@example.com", false, &stored, now),
                decision,
                "{now}"
            );
        }
    }
}
