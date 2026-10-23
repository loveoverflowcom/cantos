use super::{
    InvalidProofTime, MailboxProof, ProofAction, ProofBinding, ProofDenied, ProofId, ProofIntent,
    ProofState, ProofTime,
};
use crate::domain::{
    admission::Purpose,
    ids::{InstanceEpoch, OperationId, SubjectId},
    security_version::SecurityVersion,
};

/// Full raw u64 clock input; valid full-width IDs/versions and nonempty stored windows.
/// Assumptions are exactly the checked constructors' domains. No loops, stubs, secrets,
/// transport or database. Plans are sequenced only for local terminal-state assertions.
#[cfg(kani)]
#[kani::proof]
#[kani::unwind(2)]
fn mailbox_proof_plans_require_explicit_current_bound_unexpired_confirmation() {
    let raw_now: u64 = kani::any();
    let now = ProofTime::new(raw_now);
    assert_eq!(now.is_ok(), raw_now <= i64::MAX as u64);
    kani::cover!(now == Err(InvalidProofTime), "out-of-storage clock refused");
    let Ok(now) = now else { return };

    let id: u128 = kani::any();
    let matched_id: u128 = kani::any();
    let operation: u128 = kani::any();
    let other_operation: u128 = kani::any();
    let epoch: u128 = kani::any();
    let other_epoch: u128 = kani::any();
    let subject: u128 = kani::any();
    let other_subject: u128 = kani::any();
    let version: u64 = kani::any();
    let other_version: u64 = kani::any();
    let issued: u64 = kani::any();
    let expires: u64 = kani::any();
    kani::assume(id > 0 && matched_id > 0);
    kani::assume(operation > 0 && other_operation > 0);
    kani::assume(epoch > 0 && other_epoch > 0);
    kani::assume(subject > 0 && other_subject > 0);
    kani::assume(version > 0 && version <= i64::MAX as u64);
    kani::assume(other_version > 0 && other_version <= i64::MAX as u64);
    kani::assume(issued < expires && expires <= i64::MAX as u64);
    let signup: bool = kani::any();
    let other_signup: bool = kani::any();
    let studio: bool = kani::any();
    let other_studio: bool = kani::any();
    let explicit: bool = kani::any();
    let intent = if signup {
        ProofIntent::TheatreSignup
    } else {
        ProofIntent::CredentialRecovery {
            subject: SubjectId::from_bytes(subject.to_be_bytes()).unwrap(),
            version: SecurityVersion::new(version).unwrap(),
            purpose: if studio {
                Purpose::Studio
            } else {
                Purpose::Theatre
            },
        }
    };
    let other_intent = if other_signup {
        ProofIntent::TheatreSignup
    } else {
        ProofIntent::CredentialRecovery {
            subject: SubjectId::from_bytes(other_subject.to_be_bytes()).unwrap(),
            version: SecurityVersion::new(other_version).unwrap(),
            purpose: if other_studio {
                Purpose::Studio
            } else {
                Purpose::Theatre
            },
        }
    };
    let binding = ProofBinding::new(
        OperationId::from_bytes(operation.to_be_bytes()).unwrap(),
        InstanceEpoch::from_bytes(epoch.to_be_bytes()).unwrap(),
        intent,
    );
    let current = ProofBinding::new(
        OperationId::from_bytes(other_operation.to_be_bytes()).unwrap(),
        InstanceEpoch::from_bytes(other_epoch.to_be_bytes()).unwrap(),
        other_intent,
    );
    let proof_id = ProofId::from_bytes(id.to_be_bytes()).unwrap();
    let candidate = ProofId::from_bytes(matched_id.to_be_bytes()).unwrap();
    let proof = MailboxProof::issue(
        proof_id,
        binding,
        ProofTime::new(issued).unwrap(),
        ProofTime::new(expires).unwrap(),
    )
    .unwrap();
    let action = if explicit {
        ProofAction::ExplicitConfirmation
    } else {
        ProofAction::Inspect
    };
    let result = proof.plan_confirmation(candidate, current, action, now);
    let same_intent = signup == other_signup
        && (signup
            || (subject == other_subject && version == other_version && studio == other_studio));
    let same_binding = operation == other_operation && epoch == other_epoch && same_intent;
    let in_window = issued <= raw_now && raw_now < expires;
    assert_eq!(
        result.is_ok(),
        explicit && id == matched_id && same_binding && in_window
    );
    if let Ok(change) = result {
        assert_eq!(change.expected(), proof);
        let consumed = change.next();
        assert_eq!(
            consumed,
            MailboxProof {
                state: ProofState::Consumed,
                ..proof
            }
        );
        assert_eq!(
            consumed.plan_confirmation(proof_id, binding, ProofAction::ExplicitConfirmation, now),
            Err(ProofDenied::Consumed)
        );
        assert_eq!(
            consumed.plan_cancellation(proof_id, binding),
            Err(ProofDenied::Consumed)
        );
        assert_eq!(
            consumed.plan_replacement(proof_id, binding, candidate, now, proof.expires_at, 1),
            Err(ProofDenied::Consumed)
        );
        kani::cover!(
            consumed.state() == ProofState::Consumed,
            "sequenced replay refused"
        );
    }
    let cancelled = proof.plan_cancellation(proof_id, binding).unwrap();
    assert_eq!(cancelled.expected(), proof);
    assert_eq!(
        cancelled.next(),
        MailboxProof {
            state: ProofState::Cancelled,
            ..proof
        }
    );
    assert_eq!(
        cancelled.next().plan_confirmation(
            proof_id,
            binding,
            ProofAction::ExplicitConfirmation,
            now
        ),
        Err(ProofDenied::Cancelled)
    );
    assert_eq!(
        cancelled.next().plan_cancellation(proof_id, binding),
        Err(ProofDenied::Cancelled)
    );
    assert_eq!(
        cancelled
            .next()
            .plan_replacement(proof_id, binding, candidate, now, proof.expires_at, 1),
        Err(ProofDenied::Cancelled)
    );
    kani::cover!(
        cancelled.next().state() == ProofState::Cancelled,
        "cancel remains terminal"
    );

    let replacement_id =
        ProofId::from_bytes(if id == u128::MAX { 1_u128 } else { u128::MAX }.to_be_bytes())
            .unwrap();
    if raw_now > issued && raw_now < i64::MAX as u64 {
        let replacement = proof
            .plan_replacement(
                proof_id,
                binding,
                replacement_id,
                now,
                ProofTime::new(raw_now + 1).unwrap(),
                1,
            )
            .unwrap();
        assert_eq!(replacement.expected(), proof);
        let next = replacement.next();
        assert_eq!(next.binding, proof.binding);
        assert_eq!(next.state(), ProofState::Pending);
        assert_eq!(
            next.plan_confirmation(proof_id, binding, ProofAction::ExplicitConfirmation, now),
            Err(ProofDenied::ProofChanged)
        );
        assert_eq!(
            next.plan_cancellation(proof_id, binding),
            Err(ProofDenied::ProofChanged)
        );
        assert_eq!(
            next.plan_replacement(proof_id, binding, candidate, now, proof.expires_at, 1),
            Err(ProofDenied::ProofChanged)
        );
        assert_eq!(
            next.plan_confirmation(
                replacement_id,
                binding,
                ProofAction::ExplicitConfirmation,
                now
            )
            .unwrap()
            .next()
            .state(),
            ProofState::Consumed
        );
        kani::cover!(next.id != proof.id, "replacement refuses prior proof");
    }
    assert_eq!(
        proof.plan_replacement(proof_id, binding, proof_id, now, proof.expires_at, 1),
        Err(ProofDenied::ReusedProofId)
    );

    kani::cover!(result.is_ok() && signup, "signup confirmation plan");
    kani::cover!(
        result.is_ok() && !signup && !studio,
        "Theatre recovery confirmation plan"
    );
    kani::cover!(
        result.is_ok() && !signup && studio,
        "Studio recovery remains separate from authentication"
    );
    kani::cover!(
        result == Err(ProofDenied::NotExplicitConfirmation),
        "inspection grants nothing"
    );
    kani::cover!(
        result == Err(ProofDenied::BindingChanged),
        "stale operation or instance"
    );
    kani::cover!(
        result == Err(ProofDenied::BindingChanged)
            && !signup
            && !other_signup
            && version != other_version,
        "stale recovery version"
    );
    kani::cover!(
        result == Err(ProofDenied::BindingChanged)
            && !signup
            && !other_signup
            && studio != other_studio,
        "cross-surface recovery"
    );
    kani::cover!(
        result == Err(ProofDenied::ProofChanged),
        "wrong proof locator"
    );
    kani::cover!(
        result == Err(ProofDenied::BeforeIssue),
        "clock precedes issuance"
    );
    kani::cover!(
        result == Err(ProofDenied::Expired) && raw_now == expires,
        "exact expiry refused"
    );
    kani::cover!(
        result == Err(ProofDenied::Expired) && raw_now > expires,
        "expired proof refused"
    );
    kani::cover!(raw_now == i64::MAX as u64, "maximum storage time reachable");
}
