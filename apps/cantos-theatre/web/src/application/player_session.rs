//! Root-owned imperative shell. Pure transitions commit before fake effects execute.
use std::collections::VecDeque;
use std::cell::RefCell;
use leptos::prelude::*;
use crate::core::playback::{self, Effect, Event, PlayerState, ResumeSnapshot, Status};
use super::ports::MediaPort;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerView {
    pub episode: crate::core::ids::EpisodeId,
    pub position: u32,
    pub duration: u32,
    pub transport: Status,
    pub intent: playback::Intent,
    pub speed: u16,
}


#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerFeedback { pub resume_offered: Option<ResumeSnapshot>, pub rejection: Option<playback::Rejection> }
#[derive(Clone, Copy)]
pub struct PlayerSession {
    pub state: ReadSignal<PlayerState>,
    writer: RwSignal<PlayerState>,
    pub feedback: ReadSignal<PlayerFeedback>,
    feedback_writer: RwSignal<PlayerFeedback>,
    pub status: Memo<Status>,
    pub view: Memo<Option<PlayerView>>,
    adapter: StoredValue<RefCell<Box<dyn MediaPort>>, LocalStorage>,
}
impl PlayerSession {
    pub fn new(adapter: impl MediaPort + 'static) -> Self {
        let state = RwSignal::new(PlayerState::default());
        let view = Memo::new(move |_| state.with(|state| state.session().map(|session| PlayerView {
            episode: session.source().episode(), position: session.position_ms() / 1000,
            duration: session.source().duration_ms() / 1000, transport: state.status(),
            intent: session.intent(), speed: session.speed_permille() / 10,
        })));
        let feedback = RwSignal::new(PlayerFeedback::default());
        Self { state: state.read_only(), writer: state, view, feedback: feedback.read_only(), feedback_writer: feedback, status: Memo::new(move |_| state.get().status()), adapter: StoredValue::new_local(RefCell::new(Box::new(adapter))) }
    }
    pub fn dispatch(self, event: Event) {
        let mut pending = VecDeque::from([event]);
        while let Some(event) = pending.pop_front() {
            let transition = playback::reduce(self.state.get_untracked(), event);
            self.writer.set(transition.state);
            for effect in transition.effects {
                match &effect {
                    Effect::OfferResume { source, position_ms } => {
                        let speed_permille = self.state.get_untracked().session().map(|session| session.speed_permille()).unwrap_or(1000);
                        self.feedback_writer.set(PlayerFeedback { resume_offered: Some(ResumeSnapshot { source: *source, position_ms: *position_ms, speed_permille }), rejection: None });
                    }
                    Effect::Rejected(reason) => self.feedback_writer.set(PlayerFeedback { resume_offered: None, rejection: Some(*reason) }),
                    Effect::PrepareEngine(_) | Effect::StartEngine(_) | Effect::PauseEngine(_) | Effect::SeekEngine(_) | Effect::SetSpeed(_) => {
                        self.feedback_writer.set(PlayerFeedback::default());
                        let callbacks = self.adapter.with_value(|port| port.borrow_mut().execute(&effect));
                        pending.extend(callbacks);
                    }
                }
            }
        }
    }
    pub fn snapshot(self) -> Option<ResumeSnapshot> { self.state.get_untracked().session().map(|session| session.snapshot()) }
    /// Restore always advances generation; Reset never reuses callback identities.
    pub fn restore(self, snapshot: ResumeSnapshot) { self.dispatch(Event::RestoreRequested { snapshot }); }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::{ids::{EpisodeId, WorkId}, playback::{Intent, Source}}, infrastructure::fake_media::FakeMedia};
    struct DeferredFake;
    impl MediaPort for DeferredFake { fn execute(&mut self, _effect: &Effect) -> Vec<Event> { Vec::new() } }
    fn sample() -> ResumeSnapshot {
        ResumeSnapshot { source: Source::new(EpisodeId { work: WorkId::Mountain, index: 2 }, 1722000).unwrap(), position_ms: 768000, speed_permille: 1000 }
    }
    #[test]
    fn delayed_old_started_cannot_override_a_new_pause_or_reset() {
        Owner::new().with(|| {
            let session = PlayerSession::new(DeferredFake);
            session.restore(sample());
            session.dispatch(Event::PlayRequested);
            assert_eq!(session.state.get_untracked().status(), Status::Loading);
            let old = session.state.get_untracked().identity().unwrap();
            session.dispatch(Event::PauseRequested);
            let paused = session.state.get_untracked();
            session.dispatch(Event::EngineStarted { identity: old });
            assert_eq!(session.state.get_untracked(), paused);
            session.restore(sample());
            assert!(session.state.get_untracked().generation() > old.generation);
            session.dispatch(Event::EngineStarted { identity: old });
            assert!(matches!(session.state.get_untracked().session().unwrap().intent(), Intent::Pause(_)));
        });
    }
    #[test]
    fn restoring_with_fake_port_offers_a_resume_without_starting() {
        Owner::new().with(|| {
            let session = PlayerSession::new(FakeMedia);
            session.restore(sample());
            assert!(matches!(session.state.get_untracked().status(), Status::Paused(_)));
            assert_eq!(session.feedback.get_untracked().resume_offered, Some(sample()));
        });
    }
}
