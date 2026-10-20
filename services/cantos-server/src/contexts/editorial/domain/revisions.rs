//! Pure revision identity, retry and concurrency decisions. No HTTP, SQL, UI or clock.
use sha2::{Digest, Sha256};

/// An accepted sequence is positive and bounded by the immutable PostgreSQL row format.
/// Raw heads still permit zero before the first append; they are not accepted revisions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AcceptedRevision(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SequenceError {
    NonPositiveRevision,
    InvalidPredecessor,
}

impl AcceptedRevision {
    pub(crate) fn from_number(revision: i64) -> Result<Self, SequenceError> {
        if revision <= 0 {
            return Err(SequenceError::NonPositiveRevision);
        }
        Ok(Self(revision as u64))
    }

    pub(crate) fn from_sequence(revision: i64, expected: i64) -> Result<Self, SequenceError> {
        let admitted = Self::from_number(revision)?;
        if expected < 0 || expected as u64 != admitted.expected() {
            return Err(SequenceError::InvalidPredecessor);
        }
        Ok(admitted)
    }

    pub(crate) fn get(self) -> u64 {
        self.0
    }

    pub(crate) fn expected(self) -> u64 {
        self.0 - 1
    }
}

/// Identity of one canonical Script IR export: what an accepted revision stores and every
/// frozen copy of it must still match.
pub fn export_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"cantos/script-export/e1\n");
    digest.update(bytes);
    format!("sir-e1:sha256:{:x}", digest.finalize())
}

#[derive(Debug, PartialEq, Eq)]
pub enum SaveDecision {
    Append { revision: u64 },
    Replay { revision: u64 },
    Stale { current_revision: u64 },
    OperationReused,
    RevisionLimit,
}

pub struct PriorOperation<'a> {
    pub revision: u64,
    pub expected_revision: u64,
    pub export: &'a [u8],
}

#[derive(Debug, PartialEq, Eq)]
pub enum ReviewDecision {
    Create,
    Replay,
    Stale { current_revision: u64 },
    OperationReused,
}

/// A recorded review stays pinned; a new review must name the current head.
pub fn decide_review(
    head: u64,
    revision: u64,
    prior_revision: Option<u64>,
    already_reviewed: bool,
) -> ReviewDecision {
    if let Some(prior) = prior_revision {
        return if prior == revision {
            ReviewDecision::Replay
        } else {
            ReviewDecision::OperationReused
        };
    }
    if already_reviewed {
        return ReviewDecision::Replay;
    }
    if revision != head {
        return ReviewDecision::Stale {
            current_revision: head,
        };
    }
    ReviewDecision::Create
}

/// Retry identity includes the base AND the entire normalized export, including evidence.
/// A performed-content digest alone cannot identify an accepted operation.
pub fn decide_save(
    head: u64,
    expected: u64,
    export: &[u8],
    prior: Option<PriorOperation<'_>>,
) -> SaveDecision {
    if let Some(prior) = prior {
        return if prior.expected_revision == expected && prior.export == export {
            SaveDecision::Replay {
                revision: prior.revision,
            }
        } else {
            SaveDecision::OperationReused
        };
    }
    if head != expected {
        return SaveDecision::Stale {
            current_revision: head,
        };
    }
    if head >= i64::MAX as u64 {
        return SaveDecision::RevisionLimit;
    }
    SaveDecision::Append { revision: head + 1 }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::{
        decide_review, decide_save, export_digest, AcceptedRevision, PriorOperation,
        ReviewDecision, SaveDecision, SequenceError,
    };

    #[test]
    fn stored_accepted_sequences_reject_zero_negative_and_wrong_predecessors() {
        for revision in [i64::MIN, -1, 0] {
            assert_eq!(
                AcceptedRevision::from_number(revision),
                Err(SequenceError::NonPositiveRevision)
            );
        }
        for (revision, expected) in [(1, -1), (1, 1), (2, 0), (i64::MAX, i64::MAX)] {
            assert_eq!(
                AcceptedRevision::from_sequence(revision, expected),
                Err(SequenceError::InvalidPredecessor)
            );
        }
        for revision in [1, 2, i64::MAX] {
            let admitted = AcceptedRevision::from_sequence(revision, revision - 1).unwrap();
            assert_eq!(admitted.get(), revision as u64);
            assert_eq!(admitted.expected(), (revision - 1) as u64);
        }
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_013), ..ProptestConfig::default() })]
        #[test]
        fn reconstruction_matches_an_independent_wider_sequence_oracle(
            revision in any::<i64>(), unrelated_expected in any::<i64>(),
            exact_predecessor in any::<bool>(),
        ) {
            let expected = if exact_predecessor && revision > 0 {
                revision - 1
            } else {
                unrelated_expected
            };
            let valid = revision > 0 && i128::from(expected) + 1 == i128::from(revision);
            let restored = AcceptedRevision::from_sequence(revision, expected);
            prop_assert_eq!(restored.is_ok(), valid);
            if let Ok(admitted) = restored {
                prop_assert_eq!(u128::from(admitted.get()), revision as u128);
                prop_assert_eq!(u128::from(admitted.expected()) + 1, revision as u128);
            }
        }
    }

    #[test]
    fn export_digest_is_the_tagged_sha256_of_the_exact_bytes() {
        // Independent oracle: sha256(b"cantos/script-export/e1\n{}") computed outside Rust.
        assert_eq!(
            export_digest(b"{}"),
            "sir-e1:sha256:ebb9aa27f8c9f1abc82e6547d051d8c3ab5954151fee8852d3c17312e206ea2c"
        );
        assert_ne!(export_digest(b"{}"), export_digest(b"{} "));
    }

    #[test]
    fn review_replay_precedes_staleness_but_never_rebinds_an_operation() {
        assert_eq!(decide_review(3, 1, Some(1), true), ReviewDecision::Replay);
        assert_eq!(decide_review(3, 1, None, true), ReviewDecision::Replay);
        assert_eq!(
            decide_review(3, 2, Some(1), false),
            ReviewDecision::OperationReused
        );
        assert_eq!(
            decide_review(3, 1, None, false),
            ReviewDecision::Stale {
                current_revision: 3
            }
        );
        assert_eq!(decide_review(3, 3, None, false), ReviewDecision::Create);
    }

    #[test]
    fn replay_precedes_staleness_and_binds_base_and_complete_export() {
        let prior = || {
            Some(PriorOperation {
                revision: 1,
                expected_revision: 0,
                export: b"rights-a",
            })
        };
        assert_eq!(
            decide_save(3, 0, b"rights-a", prior()),
            SaveDecision::Replay { revision: 1 }
        );
        assert_eq!(
            decide_save(3, 1, b"rights-a", prior()),
            SaveDecision::OperationReused
        );
        assert_eq!(
            decide_save(3, 0, b"rights-b", prior()),
            SaveDecision::OperationReused
        );
        assert_eq!(
            decide_save(3, 2, b"x", None),
            SaveDecision::Stale {
                current_revision: 3
            }
        );
        assert_eq!(
            decide_save(3, 3, b"x", None),
            SaveDecision::Append { revision: 4 }
        );
        assert_eq!(
            decide_save(i64::MAX as u64, i64::MAX as u64, b"x", None),
            SaveDecision::RevisionLimit
        );
    }
}
