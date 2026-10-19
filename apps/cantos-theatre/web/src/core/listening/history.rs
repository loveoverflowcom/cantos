//! Session-only history stores identity, while positions stay in the progress owner.
use crate::core::{
    ids::EpisodeId,
    playback::{PlayerState, Status},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryEntry {
    pub episode: EpisodeId,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct HistoryState {
    records: Vec<HistoryEntry>,
}

impl HistoryState {
    /// Most recent simulated start first, with one latest episode per work.
    pub fn records(&self) -> &[HistoryEntry] {
        &self.records
    }

    pub fn contains(&self, episode: EpisodeId) -> bool {
        self.records.iter().any(|entry| entry.episode == episode)
    }

    /// Observe committed engine facts, never a page visit or unacknowledged play intent.
    /// Unchanged and stale callbacks cannot create or reorder history.
    pub fn observe_playback(&self, before: &PlayerState, after: &PlayerState) -> Self {
        if before.status() == Status::Playing || after.status() != Status::Playing {
            return self.clone();
        }
        let Some(session) = after.session() else {
            return self.clone();
        };
        // Simulated history is mock-only; a public release has no entry here.
        let Some(episode) = session.source().episode() else {
            return self.clone();
        };
        let mut next = self.clone();
        next.records
            .retain(|entry| entry.episode.work != episode.work);
        next.records.insert(0, HistoryEntry { episode });
        next
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::core::{
        ids::WorkId,
        playback::{reduce, Event, Source},
    };

    fn selected(work: WorkId, index: usize) -> PlayerState {
        let source = Source::new(EpisodeId { work, index }, 60_000).unwrap();
        reduce(
            PlayerState::default(),
            Event::SourceSelected {
                source,
                position_ms: 0,
            },
        )
        .state
    }

    fn started(work: WorkId, index: usize) -> (PlayerState, PlayerState) {
        let pending = reduce(selected(work, index), Event::PlayRequested).state;
        let running = reduce(
            pending.clone(),
            Event::EngineStarted {
                identity: pending.identity().unwrap(),
            },
        )
        .state;
        (pending, running)
    }

    #[test]
    fn selection_play_intent_and_seek_do_not_create_history() {
        let empty = HistoryState::default();
        let selected = selected(WorkId::River, 0);
        assert_eq!(
            empty.observe_playback(&PlayerState::default(), &selected),
            empty
        );
        let requested = reduce(selected.clone(), Event::PlayRequested).state;
        assert_eq!(empty.observe_playback(&selected, &requested), empty);
        let sought = reduce(selected.clone(), Event::SeekRequested { position_ms: 1000 }).state;
        assert_eq!(empty.observe_playback(&selected, &sought), empty);
    }

    #[test]
    fn accepted_starts_keep_only_the_latest_episode_per_work_in_recency_order() {
        let (pending, running) = started(WorkId::River, 0);
        let history = HistoryState::default().observe_playback(&pending, &running);
        let (pending, running) = started(WorkId::Mountain, 1);
        let history = history.observe_playback(&pending, &running);
        let (pending, running) = started(WorkId::River, 2);
        let history = history.observe_playback(&pending, &running);
        assert_eq!(
            history.records(),
            [
                HistoryEntry {
                    episode: EpisodeId {
                        work: WorkId::River,
                        index: 2,
                    },
                },
                HistoryEntry {
                    episode: EpisodeId {
                        work: WorkId::Mountain,
                        index: 1,
                    },
                },
            ]
        );
        assert!(!history.contains(EpisodeId {
            work: WorkId::River,
            index: 0,
        }));
    }

    #[test]
    fn stale_duplicate_and_paused_start_callbacks_cannot_revive_cleared_history() {
        let (pending, running) = started(WorkId::River, 0);
        let stale = reduce(
            pending.clone(),
            Event::EngineStarted {
                identity: selected(WorkId::River, 0).identity().unwrap(),
            },
        )
        .state;
        let cleared = HistoryState::default();
        assert_eq!(cleared.observe_playback(&pending, &stale), cleared);
        let duplicate = reduce(
            running.clone(),
            Event::EngineStarted {
                identity: running.identity().unwrap(),
            },
        )
        .state;
        assert_eq!(cleared.observe_playback(&running, &duplicate), cleared);
        let paused = reduce(running, Event::PauseRequested).state;
        let ignored = reduce(
            paused.clone(),
            Event::EngineStarted {
                identity: paused.identity().unwrap(),
            },
        )
        .state;
        assert_eq!(cleared.observe_playback(&paused, &ignored), cleared);
    }

    #[test]
    fn ended_simulation_remains_in_history_without_a_completion_claim() {
        let (pending, running) = started(WorkId::River, 0);
        let history = HistoryState::default().observe_playback(&pending, &running);
        let ended = reduce(
            running.clone(),
            Event::EngineEnded {
                identity: running.identity().unwrap(),
            },
        )
        .state;
        assert_eq!(history.observe_playback(&running, &ended), history);
        assert_eq!(history.records().len(), 1);
    }
}
