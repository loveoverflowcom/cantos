//! Root-owned imperative shell. Pure transitions commit before fake effects execute.
use super::ports::MediaPort;
use crate::core::playback::{self, Effect, Event, PlayerState, Status, ValidatedResumeSnapshot};
use leptos::prelude::*;
use std::cell::RefCell;
use std::collections::VecDeque;

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
pub struct PlayerFeedback {
    pub resume_offered: Option<ValidatedResumeSnapshot>,
    pub rejection: Option<playback::Rejection>,
}
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
        let view = Memo::new(move |_| {
            state.with(|state| {
                state.session().map(|session| PlayerView {
                    episode: session.source().episode(),
                    position: session.position_ms() / 1000,
                    duration: session.source().duration_ms() / 1000,
                    transport: state.status(),
                    intent: session.intent(),
                    speed: session.speed_permille() / 10,
                })
            })
        });
        let feedback = RwSignal::new(PlayerFeedback::default());
        Self {
            state: state.read_only(),
            writer: state,
            view,
            feedback: feedback.read_only(),
            feedback_writer: feedback,
            status: Memo::new(move |_| state.get().status()),
            adapter: StoredValue::new_local(RefCell::new(Box::new(adapter))),
        }
    }
    pub fn dispatch(self, event: Event) {
        let mut pending = VecDeque::from([event]);
        while let Some(event) = pending.pop_front() {
            let transition = playback::reduce(self.state.get_untracked(), event);
            self.writer.set(transition.state);
            for effect in transition.effects {
                match &effect {
                    Effect::OfferResume { .. } => {
                        self.feedback_writer.set(PlayerFeedback {
                            resume_offered: self.snapshot(),
                            rejection: None,
                        });
                    }
                    Effect::Rejected(reason) => self.feedback_writer.set(PlayerFeedback {
                        resume_offered: None,
                        rejection: Some(*reason),
                    }),
                    Effect::PrepareEngine(_)
                    | Effect::StartEngine(_)
                    | Effect::PauseEngine(_)
                    | Effect::SeekEngine(_)
                    | Effect::SetSpeed(_) => {
                        self.feedback_writer.set(PlayerFeedback::default());
                        let callbacks = self
                            .adapter
                            .with_value(|port| port.borrow_mut().execute(&effect));
                        pending.extend(callbacks);
                    }
                }
            }
        }
    }
    pub fn snapshot(self) -> Option<ValidatedResumeSnapshot> {
        self.state
            .get_untracked()
            .session()
            .map(|session| session.validated_snapshot())
    }
    /// Restore always advances generation; Reset never reuses callback identities.
    pub fn restore(self, snapshot: ValidatedResumeSnapshot) {
        self.dispatch(Event::RestoreRequested {
            snapshot: snapshot.raw(),
        });
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{
            ids::{EpisodeId, WorkId},
            playback::{Intent, ResumeSnapshot, Source},
        },
        infrastructure::fake_media::FakeMedia,
    };
    struct DeferredFake;
    impl MediaPort for DeferredFake {
        fn execute(&mut self, _effect: &Effect) -> Vec<Event> {
            Vec::new()
        }
    }
    fn sample() -> ValidatedResumeSnapshot {
        ValidatedResumeSnapshot::try_from(ResumeSnapshot {
            source: Source::new(
                EpisodeId {
                    work: WorkId::Mountain,
                    index: 2,
                },
                1722000,
            )
            .unwrap(),
            position_ms: 768000,
            speed_permille: 1000,
        })
        .unwrap()
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
            assert!(matches!(
                session.state.get_untracked().session().unwrap().intent(),
                Intent::Pause(_)
            ));
        });
    }
    #[test]
    fn restoring_with_fake_port_offers_a_resume_without_starting() {
        Owner::new().with(|| {
            let session = PlayerSession::new(FakeMedia);
            session.restore(sample());
            assert!(matches!(
                session.state.get_untracked().status(),
                Status::Paused(_)
            ));
            assert_eq!(
                session.feedback.get_untracked().resume_offered,
                Some(sample())
            );
        });
    }

    #[test]
    fn admitted_restore_preserves_speed_and_ignores_every_old_engine_observation() {
        Owner::new().with(|| {
            let session = PlayerSession::new(DeferredFake);
            session.restore(sample());
            let old = session.state.get_untracked().identity().unwrap();
            let admitted = ValidatedResumeSnapshot::try_from(ResumeSnapshot {
                source: sample().source(),
                position_ms: 30_000,
                speed_permille: 1500,
            })
            .unwrap();
            session.restore(admitted);
            let restored = session.state.get_untracked();
            let feedback = session.feedback.get_untracked();
            assert_eq!(restored.generation(), old.generation + 1);
            assert_eq!(session.snapshot(), Some(admitted));
            assert_eq!(
                restored.status(),
                Status::Paused(playback::PauseCause::ColdRestore)
            );
            assert_eq!(
                restored.session().unwrap().engine(),
                playback::Engine::Preparing
            );
            assert_eq!(feedback.resume_offered, Some(admitted));
            for event in [
                Event::EngineReady { identity: old },
                Event::EngineStarted { identity: old },
                Event::EnginePaused { identity: old },
                Event::EngineBuffering { identity: old },
                Event::EnginePosition {
                    identity: old,
                    position_ms: u32::MAX,
                },
                Event::EngineEnded { identity: old },
            ] {
                session.dispatch(event);
                assert_eq!(session.state.get_untracked(), restored);
                assert_eq!(session.feedback.get_untracked(), feedback);
            }
        });
    }

    struct RecordingMedia(std::rc::Rc<RefCell<Vec<Effect>>>);
    impl MediaPort for RecordingMedia {
        fn execute(&mut self, effect: &Effect) -> Vec<Event> {
            self.0.borrow_mut().push(effect.clone());
            Vec::new()
        }
    }

    #[test]
    fn raw_restore_rejection_leaves_the_session_and_media_effects_unchanged() {
        Owner::new().with(|| {
            let effects = std::rc::Rc::new(RefCell::new(Vec::new()));
            let session = PlayerSession::new(RecordingMedia(effects.clone()));
            session.restore(sample());
            let before = session.state.get_untracked();
            let issued = effects.borrow().clone();
            for (position_ms, speed_permille, reason) in [
                (
                    sample().source().duration_ms() + 1,
                    1000,
                    playback::Rejection::OutOfBounds,
                ),
                (30_000, 999, playback::Rejection::UnsupportedSpeed),
            ] {
                session.dispatch(Event::RestoreRequested {
                    snapshot: ResumeSnapshot {
                        source: sample().source(),
                        position_ms,
                        speed_permille,
                    },
                });
                assert_eq!(session.state.get_untracked(), before);
                assert_eq!(*effects.borrow(), issued);
                assert_eq!(
                    session.feedback.get_untracked(),
                    PlayerFeedback {
                        resume_offered: None,
                        rejection: Some(reason)
                    }
                );
            }
        });
    }
}
