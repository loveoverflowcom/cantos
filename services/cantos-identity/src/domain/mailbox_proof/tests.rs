use super::{
    InvalidProofTime, MailboxProof, ProofAction, ProofBinding, ProofDenied, ProofId, ProofIntent,
    ProofState, ProofTime,
};
use crate::domain::{
    admission::Purpose,
    ids::{InstanceEpoch, NilIdentifier, OperationId, SubjectId},
    security_version::SecurityVersion,
};

fn time(seconds: u64) -> ProofTime {
    ProofTime::new(seconds).unwrap()
}

fn id(value: u128) -> ProofId {
    ProofId::from_bytes(value.to_be_bytes()).unwrap()
}

fn binding(operation: u128, epoch: u128, intent: ProofIntent) -> ProofBinding {
    ProofBinding::new(
        OperationId::from_bytes(operation.to_be_bytes()).unwrap(),
        InstanceEpoch::from_bytes(epoch.to_be_bytes()).unwrap(),
        intent,
    )
}

fn recovery(subject: u128, version: u64, purpose: Purpose) -> ProofIntent {
    ProofIntent::CredentialRecovery {
        subject: SubjectId::from_bytes(subject.to_be_bytes()).unwrap(),
        version: SecurityVersion::new(version).unwrap(),
        purpose,
    }
}

fn pending() -> MailboxProof {
    MailboxProof::issue(
        id(1),
        binding(2, 3, ProofIntent::TheatreSignup),
        time(10),
        time(20),
    )
    .unwrap()
}

#[test]
fn restored_terminal_states_keep_the_exact_binding_and_never_reopen_pending() {
    let original = pending();
    for state in [
        ProofState::Pending,
        ProofState::Consumed,
        ProofState::Cancelled,
    ] {
        let restored = MailboxProof::restore(
            original.id(),
            original.binding(),
            original.issued_at(),
            original.expires_at(),
            state,
        )
        .unwrap();
        assert_eq!(restored.id(), original.id());
        assert_eq!(restored.binding(), original.binding());
        assert_eq!(restored.issued_at(), original.issued_at());
        assert_eq!(restored.expires_at(), original.expires_at());
        assert_eq!(restored.state(), state);
        for (issued, expires) in [(10, 10), (20, 10)] {
            assert_eq!(
                MailboxProof::restore(
                    original.id(),
                    original.binding(),
                    time(issued),
                    time(expires),
                    state
                ),
                Err(ProofDenied::InvalidWindow),
            );
        }
        let expected = match state {
            ProofState::Pending => Ok(ProofState::Consumed),
            ProofState::Consumed => Err(ProofDenied::Consumed),
            ProofState::Cancelled => Err(ProofDenied::Cancelled),
        };
        assert_eq!(
            restored
                .plan_confirmation(
                    restored.id(),
                    restored.binding(),
                    ProofAction::ExplicitConfirmation,
                    time(15)
                )
                .map(|plan| plan.next().state()),
            expected
        );
    }
}

#[test]
fn the_plan_exposes_exact_typed_storage_facts_without_an_authority_value() {
    let proof = pending();
    assert_eq!(proof.id(), id(1));
    assert_eq!(proof.binding(), binding(2, 3, ProofIntent::TheatreSignup));
    assert_eq!(proof.binding().operation().to_bytes(), 2_u128.to_be_bytes());
    assert_eq!(proof.binding().epoch().to_bytes(), 3_u128.to_be_bytes());
    assert_eq!(proof.binding().intent(), ProofIntent::TheatreSignup);
    assert_eq!(proof.issued_at(), time(10));
    assert_eq!(proof.expires_at(), time(20));
    assert_eq!(proof.state(), ProofState::Pending);
    let change = proof
        .plan_cancellation(proof.id(), proof.binding())
        .unwrap();
    assert_eq!(change.expected(), proof);
    assert_eq!(change.next().state(), ProofState::Cancelled);
    assert_eq!(change.next().id(), proof.id());
    assert_eq!(change.next().binding(), proof.binding());
    assert_eq!(change.next().issued_at(), proof.issued_at());
    assert_eq!(change.next().expires_at(), proof.expires_at());
}

#[test]
fn proof_locators_and_times_preserve_their_complete_storage_domains() {
    assert_eq!(ProofId::from_bytes([0; 16]), Err(NilIdentifier));
    for bit in 0..128 {
        let bytes = (1_u128 << bit).to_be_bytes();
        assert_eq!(ProofId::from_bytes(bytes).unwrap().to_bytes(), bytes);
    }
    assert_eq!(
        ProofId::from_bytes([255; 16]).unwrap().to_bytes(),
        [255; 16]
    );
    for seconds in [0, 1, i64::MAX as u64 - 1, i64::MAX as u64] {
        assert_eq!(time(seconds).seconds(), seconds);
    }
    for seconds in [i64::MAX as u64 + 1, u64::MAX] {
        assert_eq!(ProofTime::new(seconds), Err(InvalidProofTime));
    }
}

#[test]
fn a_proof_window_is_nonempty_and_does_not_wrap_at_the_storage_limit() {
    let original = pending();
    for (issued, expires) in [(0, 0), (10, 9), (10, 10), (i64::MAX as u64, 0)] {
        assert_eq!(
            MailboxProof::issue(original.id, original.binding, time(issued), time(expires)),
            Err(ProofDenied::InvalidWindow)
        );
    }
    let maximum = MailboxProof::issue(
        original.id,
        original.binding,
        time(i64::MAX as u64 - 1),
        time(i64::MAX as u64),
    )
    .unwrap();
    assert_eq!(
        maximum
            .plan_confirmation(
                maximum.id,
                maximum.binding,
                ProofAction::ExplicitConfirmation,
                time(i64::MAX as u64 - 1),
            )
            .unwrap()
            .next()
            .state(),
        ProofState::Consumed
    );
    assert_eq!(
        maximum.plan_confirmation(
            maximum.id,
            maximum.binding,
            ProofAction::ExplicitConfirmation,
            time(i64::MAX as u64),
        ),
        Err(ProofDenied::Expired)
    );
}

#[test]
fn confirmation_includes_issuance_but_excludes_the_exact_expiry_second() {
    let proof = pending();
    for (now, expected) in [
        (0, Err(ProofDenied::BeforeIssue)),
        (9, Err(ProofDenied::BeforeIssue)),
        (10, Ok(ProofState::Consumed)),
        (19, Ok(ProofState::Consumed)),
        (20, Err(ProofDenied::Expired)),
        (21, Err(ProofDenied::Expired)),
        (i64::MAX as u64, Err(ProofDenied::Expired)),
    ] {
        let result = proof.plan_confirmation(
            proof.id,
            proof.binding,
            ProofAction::ExplicitConfirmation,
            time(now),
        );
        assert_eq!(result.map(|change| change.next().state()), expected);
        if let Ok(change) = result {
            assert_eq!(change.expected(), proof);
            assert_eq!(
                change.next(),
                MailboxProof {
                    state: ProofState::Consumed,
                    ..proof
                }
            );
        }
        assert_eq!(proof, pending());
    }
}

#[test]
fn polling_and_prefetch_cannot_plan_consumption_in_any_state() {
    let proof = pending();
    // Knowing the initiating operation/locator is deliberately insufficient. The core
    // receives no setup token, credential, session or AuthenticationFact to return.
    let consumed = proof
        .plan_confirmation(
            proof.id,
            proof.binding,
            ProofAction::ExplicitConfirmation,
            time(15),
        )
        .unwrap()
        .next();
    let cancelled = proof
        .plan_cancellation(proof.id, proof.binding)
        .unwrap()
        .next();
    for snapshot in [proof, consumed, cancelled] {
        let state = snapshot.state();
        assert_eq!(
            snapshot.plan_confirmation(
                snapshot.id,
                snapshot.binding,
                ProofAction::Inspect,
                time(15)
            ),
            Err(ProofDenied::NotExplicitConfirmation)
        );
        assert_eq!(snapshot.state(), state);
    }
}

#[test]
fn every_operation_rejects_a_stale_or_rebound_workflow() {
    let proof = pending();
    for current in [
        binding(4, 3, ProofIntent::TheatreSignup),
        binding(2, 4, ProofIntent::TheatreSignup),
        binding(2, 3, recovery(5, 1, Purpose::Theatre)),
        binding(2, 3, recovery(5, 1, Purpose::Studio)),
    ] {
        assert_eq!(
            proof.plan_confirmation(
                proof.id,
                current,
                ProofAction::ExplicitConfirmation,
                time(15)
            ),
            Err(ProofDenied::BindingChanged)
        );
        assert_eq!(
            proof.plan_cancellation(proof.id, current),
            Err(ProofDenied::BindingChanged)
        );
        assert_eq!(
            proof.plan_replacement(proof.id, current, id(4), time(15), time(25), 5),
            Err(ProofDenied::BindingChanged)
        );
        assert_eq!(proof, pending());
    }
    assert_eq!(
        proof.plan_confirmation(
            id(4),
            proof.binding,
            ProofAction::ExplicitConfirmation,
            time(15)
        ),
        Err(ProofDenied::ProofChanged)
    );
    assert_eq!(
        proof.plan_cancellation(id(4), proof.binding),
        Err(ProofDenied::ProofChanged)
    );
    assert_eq!(
        proof.plan_replacement(id(4), proof.binding, id(5), time(15), time(25), 5),
        Err(ProofDenied::ProofChanged)
    );
}

#[test]
fn recovery_never_crosses_subject_version_surface_or_signup_intent() {
    let expected = binding(2, 3, recovery(5, 7, Purpose::Theatre));
    let proof = MailboxProof::issue(id(1), expected, time(10), time(20)).unwrap();
    for intent in [
        recovery(6, 7, Purpose::Theatre),
        recovery(5, 8, Purpose::Theatre),
        recovery(5, 7, Purpose::Studio),
        ProofIntent::TheatreSignup,
    ] {
        assert_eq!(
            proof.plan_confirmation(
                proof.id,
                binding(2, 3, intent),
                ProofAction::ExplicitConfirmation,
                time(15),
            ),
            Err(ProofDenied::BindingChanged)
        );
    }
    assert_eq!(
        proof
            .plan_confirmation(
                proof.id,
                expected,
                ProofAction::ExplicitConfirmation,
                time(15)
            )
            .unwrap()
            .next()
            .state(),
        ProofState::Consumed
    );
}

#[test]
fn consumed_and_cancelled_challenges_are_terminal_for_every_mutator() {
    let proof = pending();
    let consumed = proof
        .plan_confirmation(
            proof.id,
            proof.binding,
            ProofAction::ExplicitConfirmation,
            time(15),
        )
        .unwrap();
    let cancelled = proof.plan_cancellation(proof.id, proof.binding).unwrap();
    assert_eq!(cancelled.expected(), proof);
    assert_eq!(
        cancelled.next(),
        MailboxProof {
            state: ProofState::Cancelled,
            ..proof
        }
    );
    for (change, denial) in [
        (consumed, ProofDenied::Consumed),
        (cancelled, ProofDenied::Cancelled),
    ] {
        let terminal = change.next();
        for now in [10, 15, 20, 30] {
            assert_eq!(
                terminal.plan_confirmation(
                    terminal.id,
                    terminal.binding,
                    ProofAction::ExplicitConfirmation,
                    time(now)
                ),
                Err(denial)
            );
            assert_eq!(
                terminal.plan_cancellation(terminal.id, terminal.binding),
                Err(denial)
            );
            assert_eq!(
                terminal.plan_replacement(
                    terminal.id,
                    terminal.binding,
                    id(4),
                    time(now),
                    time(40),
                    5
                ),
                Err(denial)
            );
            assert_eq!(terminal, change.next());
        }
    }
    // Two plans from the same snapshot both expect Pending. After one is committed,
    // the other expected snapshot disagrees: a real owner must reject it atomically.
    assert_eq!(consumed.expected(), cancelled.expected());
    assert_ne!(consumed.expected(), cancelled.next());
    assert_ne!(cancelled.expected(), consumed.next());
}

#[test]
fn replacement_revokes_the_old_locator_and_keeps_exact_binding() {
    let proof = pending();
    for now in [15, 20, 30] {
        let result =
            proof.plan_replacement(proof.id, proof.binding, id(4), time(now), time(now + 10), 5);
        let expected =
            MailboxProof::issue(id(4), proof.binding, time(now), time(now + 10)).unwrap();
        assert_eq!(result.map(|change| change.next()), Ok(expected));
        let change = result.unwrap();
        assert_eq!(change.expected(), proof);
        assert_eq!(change.next(), expected);
        let next = change.next();
        assert_eq!(
            next.plan_confirmation(
                proof.id,
                proof.binding,
                ProofAction::ExplicitConfirmation,
                time(now)
            ),
            Err(ProofDenied::ProofChanged)
        );
        assert_eq!(
            next.plan_cancellation(proof.id, proof.binding),
            Err(ProofDenied::ProofChanged)
        );
        assert_eq!(
            next.plan_replacement(
                proof.id,
                proof.binding,
                id(5),
                time(now + 5),
                time(now + 10),
                5
            ),
            Err(ProofDenied::ProofChanged)
        );
        assert_eq!(
            next.plan_confirmation(
                next.id,
                next.binding,
                ProofAction::ExplicitConfirmation,
                time(now)
            )
            .unwrap()
            .next()
            .state(),
            ProofState::Consumed
        );
    }
}

#[test]
fn replacement_checks_freshness_cooldown_clock_and_window_before_any_change() {
    let proof = pending();
    for (replacement, now, expiry, cooldown, denial) in [
        (1, 15, 25, 5, ProofDenied::ReusedProofId),
        (4, 15, 25, 0, ProofDenied::InvalidCooldown),
        (4, 9, 25, 5, ProofDenied::BeforeIssue),
        (4, 14, 25, 5, ProofDenied::Cooldown),
        (4, 15, 15, 5, ProofDenied::InvalidWindow),
        (4, 15, 14, 5, ProofDenied::InvalidWindow),
        (
            4,
            i64::MAX as u64,
            i64::MAX as u64,
            u32::MAX,
            ProofDenied::InvalidWindow,
        ),
    ] {
        assert_eq!(
            proof.plan_replacement(
                proof.id,
                proof.binding,
                id(replacement),
                time(now),
                time(expiry),
                cooldown
            ),
            Err(denial)
        );
        assert_eq!(proof, pending());
    }
    let near_limit = MailboxProof::issue(
        proof.id,
        proof.binding,
        time(i64::MAX as u64 - 5),
        time(i64::MAX as u64),
    )
    .unwrap();
    assert_eq!(
        near_limit.plan_replacement(
            near_limit.id,
            near_limit.binding,
            id(4),
            time(i64::MAX as u64 - 1),
            time(i64::MAX as u64),
            u32::MAX
        ),
        Err(ProofDenied::Cooldown)
    );
}

#[test]
fn all_four_step_traces_preserve_terminal_state_and_expected_snapshot() {
    // Six explicit actions, 6^4 = 1,296 sequences. This asserts laws on the actual
    // production transitions, not a second handwritten reducer or database double.
    for trace in 0_u32..6_u32.pow(4) {
        let mut choices = trace;
        let mut proof = pending();
        let mut confirmations = 0;
        for step in 0..4 {
            let before = proof;
            let now = time(10 + step * 5);
            let action = choices % 6;
            choices /= 6;
            let result = match action {
                0 => proof.plan_confirmation(
                    proof.id,
                    proof.binding,
                    ProofAction::ExplicitConfirmation,
                    now,
                ),
                1 => proof.plan_confirmation(proof.id, proof.binding, ProofAction::Inspect, now),
                2 => proof.plan_cancellation(proof.id, proof.binding),
                3 => proof.plan_replacement(
                    proof.id,
                    proof.binding,
                    id(10 + u128::from(step)),
                    now,
                    time(now.seconds() + 10),
                    5,
                ),
                4 => proof.plan_confirmation(
                    id(100),
                    proof.binding,
                    ProofAction::ExplicitConfirmation,
                    now,
                ),
                5 => proof.plan_confirmation(
                    proof.id,
                    binding(100, 3, ProofIntent::TheatreSignup),
                    ProofAction::ExplicitConfirmation,
                    now,
                ),
                _ => unreachable!(),
            };
            if let Ok(change) = result {
                assert_eq!(change.expected(), before);
                assert_eq!(before.state(), ProofState::Pending);
                proof = change.next();
                assert_eq!(proof.binding, before.binding);
                assert!(proof.issued_at < proof.expires_at);
                if proof.state() == ProofState::Consumed {
                    confirmations += 1;
                }
            } else {
                assert_eq!(proof, before);
            }
            assert!(confirmations <= 1);
            if before.state() != ProofState::Pending {
                assert_eq!(proof, before);
            }
        }
    }
}
