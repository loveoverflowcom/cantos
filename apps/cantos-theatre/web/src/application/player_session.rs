//! Root-owned imperative shell. Pure transitions commit before engine effects execute.
//! Callbacks may arrive later from the engine: an adapter built with
//! [`PlayerSession::with_port`] posts them through the same session, and every event, whether
//! synchronous or late, passes through one ordered inbox so effects never re-enter the adapter.
use super::ports::MediaPort;
use super::scrub::{PendingSeek, PositionObservation, SeekBinding};
use crate::core::playback::{
    self, Effect, Event, PlayerState, Source, Status, TrackId, ValidatedResumeSnapshot,
};
use leptos::prelude::*;
use std::cell::{Cell, RefCell};
use std::collections::VecDeque;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerView {
    pub track: TrackId,
    pub position: u32,
    pub duration: u32,
    pub transport: Status,
    pub intent: playback::Intent,
    pub speed: u16,
}

impl PlayerView {
    /// Whether the session plays this mock episode.
    pub fn is_episode(&self, episode: crate::core::ids::EpisodeId) -> bool {
        self.track == TrackId::Fixture(episode)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TransportView {
    pub intent: playback::Intent,
    pub status: Status,
    pub replay: bool,
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
    /// Stable metadata consumers must not subscribe to position-only updates.
    pub source: Memo<Option<Source>>,
    pub view: Memo<Option<PlayerView>>,
    pub transport: Memo<Option<TransportView>>,
    pub(crate) position_ms: Memo<Option<u32>>,
    pub(crate) seek_binding: Memo<Option<SeekBinding>>,
    pub(crate) observed_position: ReadSignal<Option<PositionObservation>>,
    observation_writer: RwSignal<Option<PositionObservation>>,
    pub(crate) pending_seek: ReadSignal<Option<PendingSeek>>,
    seek_writer: RwSignal<Option<PendingSeek>>,
    adapter: StoredValue<RefCell<Box<dyn MediaPort>>, LocalStorage>,
    inbox: StoredValue<RefCell<VecDeque<Event>>, LocalStorage>,
    draining: StoredValue<Cell<bool>, LocalStorage>,
}

/// Placeholder while an adapter that needs the session is still being built.
struct Unattached;
impl MediaPort for Unattached {
    fn execute(&mut self, _effect: &Effect) -> Vec<Event> {
        Vec::new()
    }
}

impl PlayerSession {
    pub fn new(adapter: impl MediaPort + 'static) -> Self {
        Self::with_port(|_| adapter)
    }
    /// Build the one session, then the adapter that may post late callbacks back to it.
    pub fn with_port<P: MediaPort + 'static>(make: impl FnOnce(PlayerSession) -> P) -> Self {
        let session = Self::unattached();
        let port = make(session);
        session
            .adapter
            .update_value(|slot| *slot.borrow_mut() = Box::new(port));
        session
    }
    fn unattached() -> Self {
        let state = RwSignal::new(PlayerState::default());
        let view = Memo::new(move |_| {
            state.with(|state| {
                state.session().map(|session| PlayerView {
                    track: session.source().track(),
                    position: session.position_ms() / 1000,
                    duration: session.source().duration_ms() / 1000,
                    transport: state.status(),
                    intent: session.intent(),
                    speed: session.speed_permille() / 10,
                })
            })
        });
        let feedback = RwSignal::new(PlayerFeedback::default());
        let observation = RwSignal::new(None);
        let pending_seek = RwSignal::new(None);
        Self {
            state: state.read_only(),
            writer: state,
            view,
            feedback: feedback.read_only(),
            feedback_writer: feedback,
            status: Memo::new(move |_| state.with(PlayerState::status)),
            source: Memo::new(move |_| {
                state.with(|state| state.session().map(|session| session.source()))
            }),
            transport: Memo::new(move |_| {
                state.with(|state| {
                    state.session().map(|session| TransportView {
                        intent: session.intent(),
                        status: state.status(),
                        replay: state.status() == Status::Ended
                            && session.position_ms() == session.source().duration_ms(),
                    })
                })
            }),
            position_ms: Memo::new(move |_| {
                state.with(|state| state.session().map(|session| session.position_ms()))
            }),
            seek_binding: Memo::new(move |_| {
                state.with(|state| {
                    Some(SeekBinding {
                        identity: state.identity()?,
                        duration_ms: state.session()?.source().duration_ms(),
                    })
                })
            }),
            observed_position: observation.read_only(),
            observation_writer: observation,
            pending_seek: pending_seek.read_only(),
            seek_writer: pending_seek,
            adapter: StoredValue::new_local(RefCell::new(Box::new(Unattached))),
            inbox: StoredValue::new_local(RefCell::new(VecDeque::new())),
            draining: StoredValue::new_local(Cell::new(false)),
        }
    }
    /// Queue `event` and, unless an outer call is already draining, apply every queued event in
    /// arrival order. A callback posted while an effect executes is applied after it returns.
    pub fn dispatch(self, event: Event) {
        self.inbox
            .with_value(|inbox| inbox.borrow_mut().push_back(event));
        if self.draining.with_value(|draining| draining.replace(true)) {
            return;
        }
        while let Some(event) = self
            .inbox
            .with_value(|inbox| inbox.borrow_mut().pop_front())
        {
            self.apply(event);
        }
        self.draining.with_value(|draining| draining.set(false));
    }
    fn apply(self, event: Event) {
        let seeks = matches!(
            &event,
            Event::SeekRequested { .. } | Event::SkipRequested { .. }
        );
        let ended = matches!(&event, Event::EngineEnded { identity } if self.state.with_untracked(|state| state.accepts(*identity)));
        // Requested position is optimistic reducer state, never an engine acknowledgement.
        let observed = match &event {
            Event::EnginePosition {
                identity,
                position_ms,
            } if self.state.with_untracked(|state| {
                state.accepts(*identity)
                    && state
                        .session()
                        .is_some_and(|session| *position_ms <= session.source().duration_ms())
            }) =>
            {
                Some(PositionObservation {
                    identity: *identity,
                    position_ms: *position_ms,
                })
            }
            _ => None,
        };
        let transition = playback::reduce(self.state.get_untracked(), event);
        let requested = seeks
            .then(|| {
                transition.effects.iter().find_map(|effect| match effect {
                    Effect::SeekEngine(command)
                    | Effect::PrepareEngine(command)
                    | Effect::StartEngine(command) => Some(PendingSeek {
                        binding: SeekBinding {
                            identity: command.identity,
                            duration_ms: command.source.duration_ms(),
                        },
                        position_ms: command.position_ms,
                    }),
                    _ => None,
                })
            })
            .flatten();
        self.writer.set(transition.state);
        if let Some(observed) = observed {
            self.observation_writer.set(Some(observed));
        }
        let previous = self.pending_seek.get_untracked();
        let next = if ended {
            None
        } else {
            requested.or_else(|| {
                previous
                    .and_then(|seek| seek.reconcile(self.seek_binding.get_untracked(), observed))
            })
        };
        if next != previous {
            self.seek_writer.set(next);
        }
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
                    self.inbox
                        .with_value(|inbox| inbox.borrow_mut().extend(callbacks));
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

    #[test]
    fn position_ticks_do_not_recompute_source_metadata() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        Owner::new().with(|| {
            let session = PlayerSession::new(DeferredFake);
            session.restore(sample());
            let broad_reads = Arc::new(AtomicUsize::new(0));
            let narrow_reads = Arc::new(AtomicUsize::new(0));
            // Retain the pre-optimization dependency as an independent work-count baseline.
            let broad_metadata = Memo::new({
                let reads = broad_reads.clone();
                move |_| {
                    reads.fetch_add(1, Ordering::Relaxed);
                    session.view.get().map(|view| view.track)
                }
            });
            let source_metadata = Memo::new({
                let reads = narrow_reads.clone();
                move |_| {
                    reads.fetch_add(1, Ordering::Relaxed);
                    session.source.get().map(Source::track)
                }
            });
            assert_eq!(broad_metadata.get(), source_metadata.get());
            let identity = session.state.get_untracked().identity().unwrap();
            for tick in 1..=1000 {
                session.dispatch(Event::EnginePosition {
                    identity,
                    position_ms: tick * 1000,
                });
                assert_eq!(broad_metadata.get(), Some(sample().source().track()));
                assert_eq!(source_metadata.get(), broad_metadata.get());
                assert_eq!(session.view.get().unwrap().position, tick);
            }
            assert_eq!(broad_reads.load(Ordering::Relaxed), 1001);
            assert_eq!(narrow_reads.load(Ordering::Relaxed), 1);
            println!(
                "player metadata / 1000 position ticks: broad={} narrow={}",
                broad_reads.load(Ordering::Relaxed),
                narrow_reads.load(Ordering::Relaxed)
            );

            let replacement = Source::new(
                EpisodeId {
                    work: WorkId::River,
                    index: 0,
                },
                60_000,
            )
            .unwrap();
            session.dispatch(Event::SourceSelected {
                source: replacement,
                position_ms: 0,
            });
            assert_eq!(source_metadata.get(), Some(replacement.track()));
            assert_eq!(narrow_reads.load(Ordering::Relaxed), 2);
            session.dispatch(Event::EnginePosition {
                identity,
                position_ms: 30_000,
            });
            assert_eq!(source_metadata.get(), Some(replacement.track()));
            assert_eq!(session.view.get().unwrap().position, 0);
            assert_eq!(narrow_reads.load(Ordering::Relaxed), 2);
        });
    }

    #[test]
    fn narrow_source_and_status_keep_seek_speed_end_and_rebinding_semantics() {
        Owner::new().with(|| {
            let session = PlayerSession::new(DeferredFake);
            assert_eq!(session.source.get(), None);
            assert_eq!(session.status.get(), Status::Idle);
            session.restore(sample());
            let source = sample().source();
            assert_eq!(session.source.get(), Some(source));
            assert_eq!(
                session.status.get(),
                Status::Paused(playback::PauseCause::ColdRestore)
            );
            session.dispatch(Event::PlayRequested);
            let identity = session.state.get_untracked().identity().unwrap();
            session.dispatch(Event::EngineStarted { identity });
            assert_eq!(session.status.get(), Status::Playing);
            session.dispatch(Event::SeekRequested {
                position_ms: 15_000,
            });
            assert_eq!(session.view.get().unwrap().position, 15);
            assert_eq!(session.source.get(), Some(source));
            session.dispatch(Event::SpeedRequested {
                speed_permille: 1500,
            });
            assert_eq!(session.view.get().unwrap().speed, 150);
            assert_eq!(session.source.get(), Some(source));
            let identity = session.state.get_untracked().identity().unwrap();
            session.dispatch(Event::EngineEnded { identity });
            assert_eq!(session.status.get(), Status::Ended);
            assert_eq!(
                session.view.get().unwrap().position,
                source.duration_ms() / 1000
            );
            assert_eq!(session.source.get(), Some(source));

            // Source equality includes duration, so an equal episode ID cannot hide a rebind.
            let rebound =
                Source::new(source.episode().unwrap(), source.duration_ms() + 1000).unwrap();
            session.dispatch(Event::SourceSelected {
                source: rebound,
                position_ms: 0,
            });
            assert_eq!(session.source.get(), Some(rebound));
            assert_eq!(
                session.view.get().unwrap().duration,
                rebound.duration_ms() / 1000
            );
        });
    }

    /// A port that remembers the session it was built for, like the browser adapter.
    struct Reentrant {
        session: PlayerSession,
        seen: std::rc::Rc<RefCell<Vec<Effect>>>,
        answer_synchronously: bool,
    }
    impl MediaPort for Reentrant {
        fn execute(&mut self, effect: &Effect) -> Vec<Event> {
            self.seen.borrow_mut().push(effect.clone());
            if self.answer_synchronously {
                if let Effect::StartEngine(command) = effect {
                    // Callbacks posted from inside the effect: queued, never re-entrant. The
                    // second one produces an effect of its own, which would re-enter this very
                    // port (and its borrow) if it ran before this call returned.
                    self.session.dispatch(Event::EngineStarted {
                        identity: command.identity,
                    });
                    self.session.dispatch(Event::PauseRequested);
                }
            }
            Vec::new()
        }
    }

    fn public_source(n: u32) -> Source {
        crate::core::public_track::testing::source(n)
    }

    #[test]
    fn a_callback_posted_while_an_effect_executes_applies_after_it_without_reentering_the_port() {
        Owner::new().with(|| {
            let seen = std::rc::Rc::new(RefCell::new(Vec::new()));
            let session = PlayerSession::with_port(|session| Reentrant {
                session,
                seen: seen.clone(),
                answer_synchronously: true,
            });
            session.dispatch(Event::SourceSelected {
                source: public_source(1),
                position_ms: 0,
            });
            session.dispatch(Event::PlayRequested);
            // Started was acknowledged first, then the queued pause ran after the effect returned.
            assert_eq!(
                session.status.get_untracked(),
                Status::Paused(playback::PauseCause::User)
            );
            let order: Vec<&str> = seen
                .borrow()
                .iter()
                .map(|effect| match effect {
                    Effect::PrepareEngine(_) => "prepare",
                    Effect::StartEngine(_) => "start",
                    Effect::PauseEngine(_) => "pause",
                    _ => "other",
                })
                .collect();
            assert_eq!(order, ["prepare", "start", "pause"]);
        });
    }

    #[test]
    fn callbacks_of_a_replaced_source_or_cancelled_intent_cannot_change_the_current_state() {
        Owner::new().with(|| {
            let seen = std::rc::Rc::new(RefCell::new(Vec::new()));
            let session = PlayerSession::with_port(|session| Reentrant {
                session,
                seen: seen.clone(),
                answer_synchronously: false,
            });
            session.dispatch(Event::SourceSelected {
                source: public_source(1),
                position_ms: 0,
            });
            session.dispatch(Event::PlayRequested);
            let first = session.state.get_untracked().identity().unwrap();
            session.dispatch(Event::SourceSelected {
                source: public_source(2),
                position_ms: 0,
            });
            let current = session.state.get_untracked();
            for late in [
                Event::EngineReady { identity: first },
                Event::EngineStarted { identity: first },
                Event::EngineBuffering { identity: first },
                Event::EnginePosition {
                    identity: first,
                    position_ms: 5_000,
                },
                Event::EngineEnded { identity: first },
                Event::EngineFailed {
                    identity: first,
                    failure: playback::Failure::Network,
                },
            ] {
                session.dispatch(late);
                assert_eq!(session.state.get_untracked(), current);
            }
            // A cancelled intent of the same source is fenced the same way.
            session.dispatch(Event::PlayRequested);
            let playing = session.state.get_untracked().identity().unwrap();
            session.dispatch(Event::PauseRequested);
            let paused = session.state.get_untracked();
            session.dispatch(Event::EngineStarted { identity: playing });
            assert_eq!(session.state.get_untracked(), paused);
            assert_eq!(
                session.status.get_untracked(),
                Status::Paused(playback::PauseCause::User)
            );
        });
    }

    #[test]
    fn a_public_source_is_a_new_source_even_with_an_equal_fixture_shape() {
        Owner::new().with(|| {
            let session = PlayerSession::new(DeferredFake);
            session.dispatch(Event::SourceSelected {
                source: public_source(1),
                position_ms: 0,
            });
            let before = session.state.get_untracked();
            // Selecting the same pinned release again is a no-op; another release replaces it.
            session.dispatch(Event::SourceSelected {
                source: public_source(1),
                position_ms: 0,
            });
            assert_eq!(session.state.get_untracked(), before);
            session.dispatch(Event::SourceSelected {
                source: public_source(2),
                position_ms: 0,
            });
            assert_eq!(session.source.get_untracked(), Some(public_source(2)));
            assert!(session.state.get_untracked().generation() > before.generation());
        });
    }
}
