//! Pure revision identity, retry and concurrency decisions. No HTTP, SQL, UI or clock.
use sha2::{Digest, Sha256};

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
    use super::{
        decide_review, decide_save, export_digest, PriorOperation, ReviewDecision, SaveDecision,
    };

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
