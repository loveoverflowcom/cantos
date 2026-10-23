//! Small production reconcile/revision kernel, also the exact Kani target.
use super::DomainError;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Admission {
    Accept(u64),
    AlreadyCurrent,
    ListenerChooses,
    KeepServer,
}
pub fn successor(current: u64) -> Result<u64, DomainError> {
    if current > i64::MAX as u64 {
        return Err(DomainError::InvalidRevision);
    }
    if current == i64::MAX as u64 {
        return Err(DomainError::RevisionExhausted);
    }
    Ok(current + 1)
}
pub fn classify(
    base: u64,
    current: u64,
    same: bool,
    protected: bool,
    checkpoint: bool,
) -> Result<Admission, DomainError> {
    if base > i64::MAX as u64 || current > i64::MAX as u64 {
        return Err(DomainError::InvalidRevision);
    }
    if base == current {
        return successor(current).map(Admission::Accept);
    }
    if current > 0 && same {
        return Ok(Admission::AlreadyCurrent);
    }
    Ok(if current > 0 && protected && checkpoint {
        Admission::KeepServer
    } else {
        Admission::ListenerChooses
    })
}
#[cfg(kani)]
pub mod kani_proofs {
    use super::*;
    #[kani::proof]
    #[kani::unwind(2)]
    pub fn progress_reconciliation_never_blindly_overwrites_or_wraps() {
        let base: u64 = kani::any();
        let current: u64 = kani::any();
        let protected: bool = kani::any();
        let checkpoint: bool = kani::any();
        let here: u64 = kani::any();
        let there: u64 = kani::any();
        let same_completion: bool = kani::any();
        let same = here == there && same_completion;
        let result = classify(base, current, same, protected, checkpoint);
        let oracle = if base > i64::MAX as u64 || current > i64::MAX as u64 {
            Err(DomainError::InvalidRevision)
        } else if base == current && current == i64::MAX as u64 {
            Err(DomainError::RevisionExhausted)
        } else if base == current {
            Ok(Admission::Accept(current + 1))
        } else if current != 0 && same {
            Ok(Admission::AlreadyCurrent)
        } else if current != 0 && protected && checkpoint {
            Ok(Admission::KeepServer)
        } else {
            Ok(Admission::ListenerChooses)
        };
        assert_eq!(result, oracle);
        if let Ok(Admission::Accept(next)) = result {
            assert_eq!(base, current);
            assert!(next > current && next <= i64::MAX as u64);
        }
        kani::cover!(
            base == 0 && current == 0 && result == Ok(Admission::Accept(1)),
            "first"
        );
        kani::cover!(
            here < there && matches!(result, Ok(Admission::Accept(_))),
            "rewind"
        );
        kani::cover!(
            matches!(result, Ok(Admission::AlreadyCurrent)),
            "equal stale"
        );
        kani::cover!(
            matches!(result, Ok(Admission::KeepServer)),
            "protected delayed"
        );
        kani::cover!(
            matches!(result, Ok(Admission::ListenerChooses)),
            "competing"
        );
        kani::cover!(result == Err(DomainError::RevisionExhausted), "exhaustion");
        kani::cover!(
            result == Err(DomainError::InvalidRevision),
            "invalid revision"
        );
    }
}
