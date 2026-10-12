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
