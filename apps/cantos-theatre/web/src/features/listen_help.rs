//! Classify why the Work detail Listen action has no target.
use crate::{application::public_work::PageStatus, messages::Key};

/// Why the first-unit lookup produced no Listen target. Loading and failure are real states with
/// their own copy and recovery; only the two "nothing to play" conclusions carry detail.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ListenAbsence {
    Loading,
    Failed,
    /// The whole list was read and no public unit has audio.
    Unavailable,
    /// More of the list exists, so the absence is only of the part loaded so far.
    NoTarget,
}

impl ListenAbsence {
    pub(crate) fn of(status: Option<&PageStatus>, list_complete: bool) -> Self {
        match status {
            Some(PageStatus::Loading) => Self::Loading,
            Some(PageStatus::Failed(_)) => Self::Failed,
            _ if list_complete => Self::Unavailable,
            _ => Self::NoTarget,
        }
    }

    pub(crate) const fn reason(self) -> Key {
        match self {
            Self::Loading => Key::PublicWorkUnitsLoading,
            Self::Failed => Key::PublicWorkUnitsFailed,
            Self::Unavailable => Key::WorkListenUnavailable,
            Self::NoTarget => Key::WorkListenNoTarget,
        }
    }

    pub(crate) const fn is_busy(self) -> bool {
        matches!(self, Self::Loading)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::public_work::ReadFailure;

    #[test]
    fn absence_separates_loading_failure_and_the_two_conclusions() {
        assert_eq!(
            ListenAbsence::of(Some(&PageStatus::Loading), true),
            ListenAbsence::Loading
        );
        assert_eq!(
            ListenAbsence::of(Some(&PageStatus::Failed(ReadFailure::Unavailable)), false),
            ListenAbsence::Failed
        );
        assert_eq!(
            ListenAbsence::of(Some(&PageStatus::Idle), true),
            ListenAbsence::Unavailable
        );
        assert_eq!(ListenAbsence::of(None, false), ListenAbsence::NoTarget);
    }

    #[test]
    fn loading_alone_is_busy() {
        assert!(ListenAbsence::Loading.is_busy());
        for absence in [
            ListenAbsence::Failed,
            ListenAbsence::Unavailable,
            ListenAbsence::NoTarget,
        ] {
            assert!(!absence.is_busy());
        }
    }
}
