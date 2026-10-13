//! Explicit local progress facts. No completion claim, persistence or sync arbitration.
use crate::core::{ids::EpisodeId, playback::ResumeSnapshot};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Progress {
    pub snapshot: ResumeSnapshot,
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
            .filter(|progress| works.insert(progress.snapshot.source.episode().work))
            .copied()
            .collect()
    }
    pub fn remember(&self, snapshot: ResumeSnapshot) -> Self {
        let mut next = self.clone();
        next.records
            .retain(|item| item.snapshot.source.episode() != snapshot.source.episode());
        next.records.push(Progress { snapshot });
        next
    }
    pub fn find(&self, episode: EpisodeId) -> Option<Progress> {
        self.records
            .iter()
            .find(|item| item.snapshot.source.episode() == episode)
            .copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{ids::WorkId, playback::Source};

    fn snapshot(work: WorkId, index: usize, position_ms: u32) -> ResumeSnapshot {
        ResumeSnapshot {
            source: Source::new(EpisodeId { work, index }, 60_000).unwrap(),
            position_ms,
            speed_permille: 1000,
        }
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
            state.find(river_first.source.episode()).unwrap().snapshot,
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
