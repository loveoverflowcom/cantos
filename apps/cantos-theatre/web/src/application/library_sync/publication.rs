//! Closed presentation boundary. Journal/session bookkeeping is interpreted by the shell,
//! independently of publication. Adding a sync field requires classifying it here.
use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(in crate::application) struct ProgressPublication {
    record: Option<ProgressRecord>,
    resolved: bool,
    reading: bool,
    command: ProgressCommandPhase,
    presentation: Option<AudioPresentation>,
}

impl From<&PublicationProgressState> for ProgressPublication {
    fn from(value: &PublicationProgressState) -> Self {
        let PublicationProgressState {
            record,
            point_read_resolved,
            point_read_in_flight,
            command,
            presentation,
            // Closed transient progress exception: neither field feeds a view or projection.
            // `unsent` survives coalescing until mint; timestamp controls the next checkpoint.
            last_checkpoint_at_ms: _,
            unsent: _,
        } = value;
        Self {
            record: record.clone(),
            resolved: *point_read_resolved,
            reading: *point_read_in_flight,
            command: command.clone(),
            presentation: presentation.clone(),
        }
    }
}

#[derive(PartialEq, Eq)]
pub(in crate::application) struct SyncPublication {
    saved: BTreeMap<String, SavedWorkState>,
    progress: BTreeMap<String, ProgressPublication>,
    history: Option<HistoryState>,
    command: HistoryCommandPhase,
    confirmation: Option<HistoryState>,
    resume: ResumeAttemptState,
}

impl LibrarySyncState {
    pub(in crate::application) fn publication_snapshot(&self) -> SyncPublication {
        let Self {
            saved,
            progress,
            history_state,
            history_command,
            clear_confirmation,
            resume_attempt,
            // Closed non-presentation bookkeeping. These remain part of full state equality
            // and durable/effect outcomes; suppressing notify never suppresses their effects.
            actor_id: _,
            device_id: _,
            sequences: _,
            pending: _,
            active_track_session: _,
            acknowledged_sessions: _,
            unsent_starts: _,
            resume_generation: _,
        } = self;
        SyncPublication {
            saved: saved.clone(),
            progress: progress
                .iter()
                .map(|(k, v)| (k.clone(), v.into()))
                .collect(),
            history: history_state.clone(),
            command: history_command.clone(),
            confirmation: clear_confirmation.clone(),
            resume: resume_attempt.clone(),
        }
    }
}

/// Playback touches at most the source publication plus the supplied presentation's keys.
/// Never copy the whole account, journal, register maps or Library on the timeupdate path.
#[derive(PartialEq, Eq)]
pub(in crate::application) struct PlaybackPublication {
    progress: Vec<(String, Option<ProgressPublication>)>,
    work: Option<(String, Option<SavedWorkState>)>,
    history: HistoryCommandPhase,
}

impl PlaybackPublication {
    pub(in crate::application) fn new(
        sync: &LibrarySyncState,
        observation: super::super::player_session::PlaybackObservation,
        presentation: Option<&AudioPresentation>,
    ) -> Self {
        use super::super::player_session::PlaybackObservation;
        let source = match observation {
            PlaybackObservation::SessionReset => None,
            PlaybackObservation::EngineStarted { source, .. }
            | PlaybackObservation::PositionTick { source, .. }
            | PlaybackObservation::SeekConfirmed { source, .. }
            | PlaybackObservation::FlushCheckpoint { source, .. } => Some(source),
        };
        let mut keys = Vec::new();
        if let Some(audio) = source.and_then(audio_identity_from_source) {
            keys.push(audio.publication_id);
        }
        if let Some(presentation) = presentation {
            if !keys.contains(&presentation.work.publication_id) {
                keys.push(presentation.work.publication_id.clone());
            }
        }
        Self {
            progress: keys
                .into_iter()
                .map(|k| {
                    let v = sync.progress_for(&k).map(ProgressPublication::from);
                    (k, v)
                })
                .collect(),
            work: presentation.map(|p| {
                (
                    p.work.work_id.clone(),
                    sync.saved_work(&p.work.work_id).cloned(),
                )
            }),
            history: sync.history_command().clone(),
        }
    }

    pub(in crate::application) fn after(&self, sync: &LibrarySyncState) -> Self {
        Self {
            progress: self
                .progress
                .iter()
                .map(|(k, _)| {
                    (
                        k.clone(),
                        sync.progress_for(k).map(ProgressPublication::from),
                    )
                })
                .collect(),
            work: self
                .work
                .as_ref()
                .map(|(k, _)| (k.clone(), sync.saved_work(k).cloned())),
            history: sync.history_command().clone(),
        }
    }
}
