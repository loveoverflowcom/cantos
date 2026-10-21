use super::{
    check_completion, AccountState, Assurance, CompletionDenied, Purpose, RequestBinding,
    SecuritySnapshot,
};
use crate::domain::{
    ids::{InstanceEpoch, OperationId, SubjectId},
    security_version::SecurityVersion,
};

fn snapshot(subject: u128, epoch: u128, version: u64, state: AccountState) -> SecuritySnapshot {
    SecuritySnapshot::new(
        SubjectId::from_bytes(subject.to_be_bytes()).unwrap(),
        InstanceEpoch::from_bytes(epoch.to_be_bytes()).unwrap(),
        SecurityVersion::new(version).unwrap(),
        state,
    )
}

fn binding(operation: u128, purpose: Purpose) -> RequestBinding {
    RequestBinding::new(
        OperationId::from_bytes(operation.to_be_bytes()).unwrap(),
        purpose,
    )
}

const STATES: [AccountState; 4] = [
    AccountState::Active,
    AccountState::Disabled,
    AccountState::Deleted,
    AccountState::Tombstoned,
];
const PURPOSES: [Purpose; 2] = [Purpose::Theatre, Purpose::Studio];
const ASSURANCES: [Assurance; 5] = [
    Assurance::Password,
    Assurance::UserVerifiedPasskey,
    Assurance::PasskeyWithoutUserVerification,
    Assurance::RecoveryCode,
    Assurance::MailboxProof,
];

#[test]
fn only_the_explicit_surface_factor_pairs_are_admitted() {
    let state = snapshot(1, 10, 7, AccountState::Active);
    // A literal oracle, independent of the production match/enum declaration order.
    let cases = [
        (Purpose::Theatre, Assurance::Password, Ok(())),
        (Purpose::Studio, Assurance::UserVerifiedPasskey, Ok(())),
        (
            Purpose::Theatre,
            Assurance::UserVerifiedPasskey,
            Err(CompletionDenied::InsufficientAssurance),
        ),
        // #335 v1 decision: Studio accepts a password; it is not relabelled as a passkey.
        (Purpose::Studio, Assurance::Password, Ok(())),
        (
            Purpose::Theatre,
            Assurance::PasskeyWithoutUserVerification,
            Err(CompletionDenied::InsufficientAssurance),
        ),
        (
            Purpose::Studio,
            Assurance::PasskeyWithoutUserVerification,
            Err(CompletionDenied::InsufficientAssurance),
        ),
        (
            Purpose::Theatre,
            Assurance::RecoveryCode,
            Err(CompletionDenied::InsufficientAssurance),
        ),
        (
            Purpose::Studio,
            Assurance::RecoveryCode,
            Err(CompletionDenied::InsufficientAssurance),
        ),
        (
            Purpose::Theatre,
            Assurance::MailboxProof,
            Err(CompletionDenied::InsufficientAssurance),
        ),
        (
            Purpose::Studio,
            Assurance::MailboxProof,
            Err(CompletionDenied::InsufficientAssurance),
        ),
    ];
    for (purpose, assurance, expected) in cases {
        let request = binding(100, purpose);
        assert_eq!(
            check_completion(state, request, state, request, assurance),
            expected
        );
    }
}

#[test]
fn stale_or_rebound_completions_have_exact_refusals() {
    let initial = snapshot(1, 10, 7, AccountState::Active);
    let expected = binding(100, Purpose::Theatre);
    let cases = [
        (
            initial,
            binding(101, Purpose::Theatre),
            CompletionDenied::WrongOperation,
        ),
        (
            initial,
            binding(100, Purpose::Studio),
            CompletionDenied::WrongPurpose,
        ),
        (
            snapshot(2, 10, 7, AccountState::Active),
            expected,
            CompletionDenied::SubjectChanged,
        ),
        (
            snapshot(1, 11, 7, AccountState::Active),
            expected,
            CompletionDenied::EpochChanged,
        ),
        (
            snapshot(1, 10, 7, AccountState::Disabled),
            expected,
            CompletionDenied::InactiveAccount,
        ),
        (
            snapshot(1, 10, 8, AccountState::Active),
            expected,
            CompletionDenied::SecurityVersionChanged,
        ),
        (
            snapshot(1, 10, 6, AccountState::Active),
            expected,
            CompletionDenied::SecurityVersionChanged,
        ),
    ];
    for (current, completed, error) in cases {
        assert_eq!(
            check_completion(initial, expected, current, completed, Assurance::Password),
            Err(error)
        );
        assert_eq!(initial, snapshot(1, 10, 7, AccountState::Active));
    }
}

#[test]
fn disable_reset_and_restore_fence_already_running_verification() {
    let started = snapshot(1, 10, 7, AccountState::Active);
    let request = binding(100, Purpose::Studio);
    let disabled = snapshot(1, 10, 8, AccountState::Disabled);
    assert_eq!(
        check_completion(
            started,
            request,
            disabled,
            request,
            Assurance::UserVerifiedPasskey
        ),
        Err(CompletionDenied::InactiveAccount)
    );
    let reenabled = snapshot(1, 10, 9, AccountState::Active);
    assert_eq!(
        check_completion(
            started,
            request,
            reenabled,
            request,
            Assurance::UserVerifiedPasskey
        ),
        Err(CompletionDenied::SecurityVersionChanged)
    );
    // Even an older restored version must fail; equality, rather than >=, matters.
    let restored = snapshot(1, 11, 7, AccountState::Active);
    assert_eq!(
        check_completion(
            started,
            request,
            restored,
            request,
            Assurance::UserVerifiedPasskey
        ),
        Err(CompletionDenied::EpochChanged)
    );
    let previously_disabled = snapshot(1, 10, 9, AccountState::Disabled);
    assert_eq!(
        check_completion(
            previously_disabled,
            request,
            reenabled,
            request,
            Assurance::UserVerifiedPasskey
        ),
        Err(CompletionDenied::InactiveAccount)
    );
}

#[test]
fn refusal_precedence_is_stable_when_multiple_fences_change() {
    let started = snapshot(1, 10, 7, AccountState::Active);
    let changed = snapshot(2, 11, 8, AccountState::Disabled);
    let request = binding(100, Purpose::Theatre);
    assert_eq!(
        check_completion(
            started,
            request,
            changed,
            binding(101, Purpose::Studio),
            Assurance::MailboxProof
        ),
        Err(CompletionDenied::WrongOperation)
    );
    assert_eq!(
        check_completion(
            started,
            request,
            changed,
            binding(100, Purpose::Studio),
            Assurance::MailboxProof
        ),
        Err(CompletionDenied::WrongPurpose)
    );
    assert_eq!(
        check_completion(started, request, changed, request, Assurance::MailboxProof),
        Err(CompletionDenied::SubjectChanged)
    );
}

#[test]
fn the_full_finite_state_binding_and_assurance_matrix_has_only_three_success_classes() {
    let mut cases = 0;
    let mut admitted = 0;
    for (before_index, before) in STATES.into_iter().enumerate() {
        for (after_index, after) in STATES.into_iter().enumerate() {
            for subject in [1, 2] {
                for epoch in [10, 11] {
                    for version in [6, 7, 8] {
                        for operation in [100, 101] {
                            for (purpose_index, purpose) in PURPOSES.into_iter().enumerate() {
                                for (completed_index, completed) in PURPOSES.into_iter().enumerate()
                                {
                                    for (factor_index, assurance) in
                                        ASSURANCES.into_iter().enumerate()
                                    {
                                        let started = snapshot(1, 10, 7, before);
                                        let current = snapshot(subject, epoch, version, after);
                                        let result = check_completion(
                                            started,
                                            binding(100, purpose),
                                            current,
                                            binding(operation, completed),
                                            assurance,
                                        );
                                        let expected = before_index == 0
                                            && after_index == 0
                                            && (subject, epoch, version, operation)
                                                == (1, 10, 7, 100)
                                            && purpose_index == completed_index
                                            && [(0, 0), (1, 0), (1, 1)]
                                                .contains(&(purpose_index, factor_index));
                                        assert_eq!(result.is_ok(), expected, "{started:?} {current:?} {purpose:?} {completed:?} {assurance:?}");
                                        cases += 1;
                                        admitted += usize::from(result.is_ok());
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    assert_eq!(cases, 7_680);
    // Theatre password, Studio password (the #335 v1 decision) and Studio verified passkey.
    assert_eq!(admitted, 3);
}
