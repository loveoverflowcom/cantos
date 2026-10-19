//! Web-only range presentation. Playback decisions remain in the existing reducer.
use crate::core::playback::CallbackIdentity;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct SeekBinding {
    pub identity: CallbackIdentity,
    pub duration_ms: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PositionObservation {
    pub identity: CallbackIdentity,
    pub position_ms: u32,
}

/// Root-owned presentation fact, so navigation cannot discard an outstanding seek.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PendingSeek {
    pub binding: SeekBinding,
    pub position_ms: u32,
}

impl PendingSeek {
    pub fn presentation(self) -> Scrub {
        Scrub::PendingSeek {
            binding: self.binding,
            position_ms: self.position_ms,
        }
    }

    pub fn reconcile(
        self,
        current: Option<SeekBinding>,
        observed: Option<PositionObservation>,
    ) -> Option<Self> {
        matches!(
            self.presentation().reconcile(current, observed),
            Scrub::PendingSeek { .. }
        )
        .then_some(self)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum Scrub {
    #[default]
    FollowingEngine,
    Preview {
        binding: SeekBinding,
        position_ms: u32,
    },
    PendingSeek {
        binding: SeekBinding,
        position_ms: u32,
    },
    /// Keep late input/change from a canceled pointer gesture inert until release.
    Canceled,
}

impl Scrub {
    pub fn begin(binding: Option<SeekBinding>, position_ms: u32) -> Self {
        match binding {
            Some(binding) if binding.duration_ms > 0 => Self::Preview {
                binding,
                position_ms: position_ms.min(binding.duration_ms),
            },
            _ => Self::FollowingEngine,
        }
    }

    pub fn preview(self, current: Option<SeekBinding>, position_ms: u32) -> Self {
        match self {
            Self::Canceled => self,
            Self::Preview { binding, .. } if Some(binding) != current => Self::Canceled,
            Self::FollowingEngine | Self::Preview { .. } | Self::PendingSeek { .. } => {
                Self::begin(current, position_ms)
            }
        }
    }

    /// Consuming the draft before dispatch makes change/pointerup/blur ordering idempotent.
    pub fn commit(self, current: Option<SeekBinding>) -> Option<u32> {
        match self {
            Self::Preview {
                binding,
                position_ms,
            } if Some(binding) == current => Some(position_ms.min(binding.duration_ms)),
            _ => None,
        }
    }

    pub fn reconcile(
        self,
        current: Option<SeekBinding>,
        observed: Option<PositionObservation>,
    ) -> Self {
        match self {
            Self::Preview { binding, .. } if Some(binding) != current => Self::Canceled,
            Self::PendingSeek { binding, .. } if Some(binding) != current => Self::FollowingEngine,
            Self::PendingSeek {
                binding,
                position_ms,
            } if observed
                == Some(PositionObservation {
                    identity: binding.identity,
                    position_ms,
                }) =>
            {
                Self::FollowingEngine
            }
            _ => self,
        }
    }

    pub fn cancel(self) -> Self {
        match self {
            Self::Preview { .. } => Self::Canceled,
            _ => self,
        }
    }

    pub fn release(self) -> Self {
        match self {
            Self::Preview { .. } | Self::Canceled => Self::FollowingEngine,
            _ => self,
        }
    }

    pub fn displayed_position(self) -> Option<u32> {
        match self {
            Self::Preview { position_ms, .. } | Self::PendingSeek { position_ms, .. } => {
                Some(position_ms)
            }
            Self::FollowingEngine | Self::Canceled => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ids::{EpisodeId, WorkId},
        playback::TrackId,
    };

    fn binding(generation: u32) -> SeekBinding {
        SeekBinding {
            identity: CallbackIdentity {
                track: TrackId::Fixture(EpisodeId {
                    work: WorkId::River,
                    index: 0,
                }),
                generation,
            },
            duration_ms: 60_000,
        }
    }

    #[test]
    fn a_thousand_reversing_inputs_produce_one_final_commit() {
        let binding = Some(binding(1));
        let mut scrub = Scrub::begin(binding, 10_000);
        for input in 0..1000 {
            let position = if input % 2 == 0 { 55_000 } else { 5_000 };
            scrub = scrub.preview(binding, position);
            assert_eq!(scrub.displayed_position(), Some(position));
        }
        assert_eq!(scrub.commit(binding), Some(5_000));
        // The shell consumes the draft before the first dispatch.
        scrub = Scrub::FollowingEngine;
        assert_eq!(scrub.commit(binding), None);
        assert_eq!(scrub.release().commit(binding), None);
    }

    #[test]
    fn source_change_restore_and_other_commands_invalidate_the_gesture() {
        let original = Some(binding(1));
        let preview = Scrub::begin(original, 40_000);
        let changed = Some(binding(2));
        assert_eq!(preview.commit(changed), None);
        let canceled = preview.reconcile(changed, None);
        assert_eq!(canceled, Scrub::Canceled);
        assert_eq!(canceled.preview(changed, 50_000).commit(changed), None);
        assert_eq!(canceled.release(), Scrub::FollowingEngine);
        assert_eq!(preview.reconcile(None, None), Scrub::Canceled);
    }

    #[test]
    fn pointer_cancel_escape_and_blur_never_commit() {
        let current = Some(binding(1));
        let canceled = Scrub::begin(current, 40_000).cancel();
        assert_eq!(canceled.commit(current), None);
        assert_eq!(canceled.preview(current, 55_000).commit(current), None);
        assert_eq!(canceled.release(), Scrub::FollowingEngine);
        // A fresh keyboard or pointer interaction can start after release.
        assert_eq!(
            Scrub::begin(current, 0)
                .preview(current, 60_000)
                .commit(current),
            Some(60_000)
        );
    }

    #[test]
    fn bounds_use_the_bound_source_and_unknown_duration_cannot_seek() {
        let current = Some(binding(1));
        assert_eq!(
            Scrub::begin(current, u32::MAX).commit(current),
            Some(60_000)
        );
        assert_eq!(Scrub::begin(current, 0).commit(current), Some(0));
        assert_eq!(Scrub::begin(None, 30_000).commit(None), None);
        let zero = Some(SeekBinding {
            duration_ms: 0,
            ..binding(1)
        });
        assert_eq!(Scrub::begin(zero, 30_000), Scrub::FollowingEngine);
        let shortened = Some(SeekBinding {
            duration_ms: 20_000,
            ..binding(1)
        });
        assert_eq!(Scrub::begin(current, 50_000).commit(shortened), None);
    }

    #[test]
    fn pending_seek_waits_for_the_exact_generation_and_position() {
        let current = Some(binding(2));
        let pending = Scrub::PendingSeek {
            binding: binding(2),
            position_ms: 10_000,
        };
        for observed in [
            None,
            Some(PositionObservation {
                identity: binding(1).identity,
                position_ms: 10_000,
            }),
            Some(PositionObservation {
                identity: binding(2).identity,
                position_ms: 55_000,
            }),
        ] {
            assert_eq!(pending.reconcile(current, observed), pending);
            assert_eq!(pending.displayed_position(), Some(10_000));
            assert_eq!(pending.commit(current), None);
        }
        assert_eq!(
            pending.reconcile(
                current,
                Some(PositionObservation {
                    identity: binding(2).identity,
                    position_ms: 10_000,
                })
            ),
            Scrub::FollowingEngine
        );
        assert_eq!(
            pending.reconcile(Some(binding(3)), None),
            Scrub::FollowingEngine
        );
    }
}
