//! Explicit local progress facts. No completion claim, persistence or sync arbitration.
use crate::core::{ids::EpisodeId, playback::ValidatedResumeSnapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub snapshot: ValidatedResumeSnapshot,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ProgressState {
    records: Vec<Progress>,
}
impl ProgressState {
    pub fn records(&self) -> &[Progress] {
        &self.records
    }
    pub fn latest(&self) -> Option<Progress> {
        self.records.last().copied()
    }
    /// Continue is a projection of existing positions, not a second progress store.
    pub fn recent(&self) -> Vec<Progress> {
        let mut works = std::collections::BTreeSet::new();
        self.records
            .iter()
            .rev()
            .filter(|progress| works.insert(progress.snapshot.source().episode().work))
            .copied()
            .collect()
    }
    /// Only admitted positions/speeds can enter retained session progress.
    ///
    /// ```compile_fail
    /// use cantos_theatre::core::{listening::progress::ProgressState, playback::ResumeSnapshot};
    /// fn retain_raw(state: &ProgressState, raw: ResumeSnapshot) {
    ///     state.remember(raw);
    /// }
    /// ```
    pub fn remember(&self, snapshot: ValidatedResumeSnapshot) -> Self {
        let mut next = self.clone();
        next.records
            .retain(|item| item.snapshot.source().episode() != snapshot.source().episode());
        next.records.push(Progress { snapshot });
        next
    }
    pub fn find(&self, episode: EpisodeId) -> Option<Progress> {
        self.records
            .iter()
            .find(|item| item.snapshot.source().episode() == episode)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ids::WorkId,
        playback::{Rejection, ResumeSnapshot, Source},
    };

    fn snapshot(index: usize, position_ms: u32, speed_permille: u16) -> ResumeSnapshot {
        ResumeSnapshot {
            source: Source::new(
                EpisodeId {
                    work: WorkId::Mountain,
                    index,
                },
                60_000,
            )
            .unwrap(),
            position_ms,
            speed_permille,
        }
    }

    fn remember_raw(
        state: &ProgressState,
        raw: ResumeSnapshot,
    ) -> Result<ProgressState, Rejection> {
        ValidatedResumeSnapshot::try_from(raw).map(|snapshot| state.remember(snapshot))
    }

    #[test]
    fn out_of_bounds_checkpoint_cannot_replace_valid_progress() {
        let state = remember_raw(&ProgressState::default(), snapshot(0, 15_000, 1000)).unwrap();
        let before = state.clone();
        assert_eq!(
            remember_raw(&state, snapshot(0, 60_001, 1000)),
            Err(Rejection::OutOfBounds)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn unsupported_speed_checkpoint_cannot_replace_valid_progress() {
        let state = remember_raw(&ProgressState::default(), snapshot(0, 15_000, 1000)).unwrap();
        let before = state.clone();
        assert_eq!(
            remember_raw(&state, snapshot(0, 15_000, 999)),
            Err(Rejection::UnsupportedSpeed)
        );
        assert_eq!(state, before);
    }

    #[test]
    fn every_retained_checkpoint_matches_the_latest_explicit_fact_for_its_episode() {
        let choices = [
            snapshot(0, 0, 750),
            snapshot(1, 60_000, 1250),
            snapshot(0, 15_000, 2000),
            snapshot(2, 30_000, 1000),
            snapshot(1, 1, 1500),
        ];
        // All 5^4 command sequences, including duplicates and explicit backward seeks.
        for sequence in 0..625 {
            let mut ordinal = sequence;
            let mut state = ProgressState::default();
            let mut history = Vec::new();
            for _ in 0..4 {
                let raw = choices[ordinal % choices.len()];
                ordinal /= choices.len();
                let before = state.clone();
                let next = remember_raw(&state, raw).unwrap();
                assert_eq!(state, before);
                state = next;
                history.push(raw);
                // Naive last-occurrence oracle, structurally distinct from retain+push.
                let expected: Vec<_> = history
                    .iter()
                    .enumerate()
                    .filter(|(at, item)| {
                        !history[at + 1..]
                            .iter()
                            .any(|later| later.source.episode() == item.source.episode())
                    })
                    .map(|(_, item)| *item)
                    .collect();
                let actual: Vec<_> = state
                    .records()
                    .iter()
                    .map(|item| item.snapshot.raw())
                    .collect();
                assert_eq!(actual, expected);
                assert_eq!(state.latest().map(|item| item.snapshot.raw()), Some(raw));
                assert_eq!(
                    state
                        .find(raw.source.episode())
                        .map(|item| item.snapshot.raw()),
                    Some(raw)
                );
            }
        }
    }
}

#[cfg(test)]
mod projection_tests {
    use super::*;
    use crate::core::{
        ids::WorkId,
        playback::{ResumeSnapshot, Source},
    };

    fn snapshot(work: WorkId, index: usize, position_ms: u32) -> ValidatedResumeSnapshot {
        ValidatedResumeSnapshot::try_from(ResumeSnapshot {
            source: Source::new(EpisodeId { work, index }, 60_000).unwrap(),
            position_ms,
            speed_permille: 1000,
        })
        .unwrap()
    }

    #[test]
    fn recent_projects_one_latest_episode_per_work_without_losing_episode_positions() {
        let river_first = snapshot(WorkId::River, 0, 5000);
        let mountain = snapshot(WorkId::Mountain, 1, 10_000);
        let river_latest = snapshot(WorkId::River, 2, 15_000);
        let state = ProgressState::default()
            .remember(river_first)
            .remember(mountain)
            .remember(river_latest);
        assert_eq!(
            state.recent(),
            [
                Progress {
                    snapshot: river_latest,
                },
                Progress { snapshot: mountain },
            ]
        );
        assert_eq!(state.records().len(), 3);
        assert_eq!(
            state.find(river_first.source().episode()).unwrap().snapshot,
            river_first
        );
    }

    #[test]
    fn explicit_rewind_replaces_the_same_episode_and_becomes_most_recent() {
        let original = snapshot(WorkId::River, 0, 50_000);
        let rewind = snapshot(WorkId::River, 0, 1000);
        let state = ProgressState::default()
            .remember(original)
            .remember(snapshot(WorkId::Mountain, 0, 10_000))
            .remember(rewind);
        assert_eq!(state.records().len(), 2);
        assert_eq!(state.latest(), Some(Progress { snapshot: rewind }));
        assert_eq!(state.recent()[0].snapshot, rewind);
    }
}
