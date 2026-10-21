use super::*;
use cantos_identity_rpc::{Assurance, Purpose};

fn subject(n: u8) -> Subject {
    Subject::from_bytes([n; 16]).unwrap()
}
fn epoch(n: u8) -> Epoch {
    Epoch::from_bytes([n; 16]).unwrap()
}
fn fact(at: i64, version: u64) -> ValidatedFact {
    ValidatedFact {
        subject: [1; 16],
        instance_epoch: [9; 16],
        security_version: version,
        authenticated_at_unix_micros: at,
        purpose: Purpose::Theatre,
        assurance: Assurance::Password,
    }
}
fn state(active: bool, version: u64, epoch_byte: u8) -> SecurityState {
    SecurityState {
        subject: subject(1),
        active,
        version,
        epoch: epoch(epoch_byte),
    }
}

#[test]
fn identifiers_refuse_zero_and_round_trip_through_their_registry_spelling() {
    assert!(Subject::from_bytes([0; 16]).is_none());
    assert!(Subject::from_slice(&[1; 15]).is_none());
    assert!(Epoch::from_slice(&[]).is_none());
    let value = Subject::from_bytes([0xab; 16]).unwrap();
    assert_eq!(value.hex(), "ab".repeat(16));
    assert_eq!(Subject::from_hex(&value.hex()), Some(value));
    for bad in [
        "AB".repeat(16),
        "ab".repeat(15),
        "ab".repeat(17),
        "0".repeat(32),
        "zz".repeat(16),
        String::new(),
    ] {
        assert!(Subject::from_hex(&bad).is_none(), "{bad}");
    }
    assert!(!format!("{value:?}").contains("ab"));
}

#[test]
fn security_state_decoding_checks_shape_subject_and_closed_state() {
    let good = GetSecurityStateResponse {
        subject_id: vec![1; 16],
        state: AccountState::Active as i32,
        security_version: 7,
        instance_epoch: vec![9; 16],
    };
    assert_eq!(
        SecurityState::from_response(&good, subject(1)),
        Ok(state(true, 7, 9))
    );
    let mut disabled = good.clone();
    disabled.state = AccountState::Disabled as i32;
    assert_eq!(
        SecurityState::from_response(&disabled, subject(1)),
        Ok(state(false, 7, 9))
    );
    type Mutation = fn(&mut GetSecurityStateResponse);
    let mutations: [(&str, Mutation); 7] = [
        ("unspecified state", |r| r.state = 0),
        ("unknown state", |r| r.state = 77),
        ("zero version", |r| r.security_version = 0),
        ("version beyond i64", |r| {
            r.security_version = i64::MAX as u64 + 1
        }),
        ("zero epoch", |r| r.instance_epoch = vec![0; 16]),
        ("short subject", |r| r.subject_id = vec![1; 8]),
        ("another subject", |r| r.subject_id = vec![2; 16]),
    ];
    for (name, mutate) in mutations {
        let mut bad = good.clone();
        mutate(&mut bad);
        assert_eq!(
            SecurityState::from_response(&bad, subject(1)),
            Err(MalformedState),
            "{name}"
        );
    }
}

#[test]
fn a_fact_becomes_observed_security_only_inside_the_exact_age_window() {
    let now = 1_000_000_000_000;
    let current = state(true, 3, 9);
    let observed = ObservedSecurity {
        subject: subject(1),
        epoch: epoch(9),
        version: 3,
    };
    // Literal boundaries: exactly 30 s old and exactly 5 s ahead are admitted.
    assert_eq!(
        decide_issue(&fact(now - FACT_MAX_AGE_MICROS, 3), &current, now),
        Ok(observed)
    );
    assert_eq!(
        decide_issue(&fact(now + FACT_MAX_FUTURE_MICROS, 3), &current, now),
        Ok(observed)
    );
    assert_eq!(
        decide_issue(&fact(now - FACT_MAX_AGE_MICROS - 1, 3), &current, now),
        Err(IssueRefusal::FactStale)
    );
    assert_eq!(
        decide_issue(&fact(now + FACT_MAX_FUTURE_MICROS + 1, 3), &current, now),
        Err(IssueRefusal::FactFromFuture)
    );
    // Wire integers at the extremes cannot overflow the age arithmetic.
    assert_eq!(
        decide_issue(&fact(i64::MAX, 3), &current, i64::MIN),
        Err(IssueRefusal::FactStale)
    );
    assert_eq!(
        decide_issue(&fact(0, 3), &current, i64::MAX),
        Err(IssueRefusal::FactStale)
    );
}

#[test]
fn issue_refusals_have_a_stable_precedence_and_exact_variants() {
    let now = 5_000_000_000;
    let fresh = fact(now, 3);
    assert_eq!(
        decide_issue(&fresh, &state(false, 3, 9), now),
        Err(IssueRefusal::Inactive)
    );
    assert_eq!(
        decide_issue(&fresh, &state(true, 3, 8), now),
        Err(IssueRefusal::EpochChanged)
    );
    for version in [2, 4] {
        assert_eq!(
            decide_issue(&fresh, &state(true, version, 9), now),
            Err(IssueRefusal::VersionChanged)
        );
    }
    let mut other = state(true, 3, 9);
    other.subject = subject(2);
    assert_eq!(
        decide_issue(&fresh, &other, now),
        Err(IssueRefusal::SubjectChanged)
    );
    // Subject outranks staleness; staleness outranks the account state.
    assert_eq!(
        decide_issue(&fact(0, 3), &other, now),
        Err(IssueRefusal::SubjectChanged)
    );
    assert_eq!(
        decide_issue(&fact(0, 3), &state(false, 3, 9), now),
        Err(IssueRefusal::FactStale)
    );
}

#[test]
fn admission_over_the_whole_finite_grid_matches_an_independent_oracle() {
    let observed = ObservedSecurity {
        subject: subject(1),
        epoch: epoch(9),
        version: 5,
    };
    let mut admitted = 0;
    let mut cases = 0;
    for other_subject in [false, true] {
        for active in [true, false] {
            for epoch_byte in [9, 10] {
                for version in [4, 5, 6] {
                    let mut current = state(active, version, epoch_byte);
                    if other_subject {
                        current.subject = subject(2);
                    }
                    let expected = if other_subject {
                        Admission::Revoke(RevokeReason::SubjectMismatch)
                    } else if !active {
                        Admission::Revoke(RevokeReason::Disabled)
                    } else if epoch_byte != 9 {
                        Admission::Revoke(RevokeReason::EpochChanged)
                    } else if version == 6 {
                        Admission::Revoke(RevokeReason::VersionAdvanced)
                    } else if version == 4 {
                        Admission::Revoke(RevokeReason::VersionRegressed)
                    } else {
                        Admission::Admit
                    };
                    assert_eq!(decide_admission(&observed, &current), expected);
                    cases += 1;
                    admitted += usize::from(expected == Admission::Admit);
                }
            }
        }
    }
    assert_eq!((cases, admitted), (24, 1));
}

#[test]
fn revoke_reasons_have_distinct_stored_spellings() {
    let all = [
        RevokeReason::SubjectMismatch,
        RevokeReason::SubjectMissing,
        RevokeReason::Disabled,
        RevokeReason::EpochChanged,
        RevokeReason::VersionAdvanced,
        RevokeReason::VersionRegressed,
    ];
    let names: std::collections::BTreeSet<_> = all.iter().map(|r| r.stored()).collect();
    assert_eq!(names.len(), all.len());
}
