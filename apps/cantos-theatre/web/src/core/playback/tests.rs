use super::{
    reduce, CallbackIdentity, Effect, Engine, EngineCommand, Event, Failure, Intent, Interruption,
    PauseCause, PlaybackSession, PlayerState, Rejection, ResumeSnapshot, Source, Status, TrackId,
    Transition, ValidatedResumeSnapshot,
};
use crate::core::ids::{EpisodeId, WorkId};

fn source() -> Source {
    Source::new(
        EpisodeId {
            work: WorkId::Mountain,
            index: 2,
        },
        60_000,
    )
    .unwrap()
}

fn session_state(intent: Intent, engine: Engine) -> PlayerState {
    PlayerState {
        generation: 7,
        session: Some(PlaybackSession {
            source: source(),
            position_ms: 15_000,
            speed_permille: 1250,
            intent,
            engine,
            interruption: Interruption::None,
        }),
    }
}

fn command(generation: u32, position_ms: u32) -> EngineCommand {
    EngineCommand {
        identity: CallbackIdentity {
            track: source().track(),
            generation,
        },
        source: source(),
        position_ms,
        speed_permille: 1250,
    }
}

fn assert_rejected(state: PlayerState, event: Event, reason: Rejection) {
    assert_eq!(
        reduce(state.clone(), event),
        Transition {
            state,
            effects: vec![Effect::Rejected(reason)]
        }
    );
}

#[test]
fn source_constructor_refuses_zero_duration_and_unbounded_episode_index() {
    let episode = source().episode().unwrap();
    assert_eq!(Source::new(episode, 0), None);
    assert_eq!(
        Source::new(
            EpisodeId {
                index: 65_536,
                ..episode
            },
            1
        ),
        None
    );
    assert_eq!(
        Source::new(
            EpisodeId {
                index: 65_535,
                ..episode
            },
            u32::MAX
        )
        .unwrap()
        .duration_ms(),
        u32::MAX
    );
}

#[test]
fn resume_admission_checks_all_u16_speeds_and_position_edge_partitions() {
    for position_ms in [0, 1, 59_999, 60_000, 60_001, u32::MAX] {
        for speed_permille in 0..=u16::MAX {
            let raw = ResumeSnapshot {
                source: source(),
                position_ms,
                speed_permille,
            };
            // Independent authored contract, not the production speed predicate.
            let expected = if position_ms > 60_000 {
                Err(Rejection::OutOfBounds)
            } else if matches!(speed_permille, 750 | 1000 | 1250 | 1500 | 2000) {
                Ok(raw)
            } else {
                Err(Rejection::UnsupportedSpeed)
            };
            assert_eq!(
                ValidatedResumeSnapshot::try_from(raw).map(|s| s.raw()),
                expected
            );
        }
    }
    for duration_ms in [1, u32::MAX] {
        let source = Source::new(source().episode().unwrap(), duration_ms).unwrap();
        for position_ms in [0, duration_ms] {
            let raw = ResumeSnapshot {
                source,
                position_ms,
                speed_permille: 1000,
            };
            let admitted = ValidatedResumeSnapshot::try_from(raw).unwrap();
            assert_eq!(admitted.source(), source);
            assert_eq!(admitted.position_ms(), position_ms);
            assert_eq!(admitted.speed_permille(), 1000);
            assert_eq!(admitted.raw(), raw);
        }
    }
}

#[test]
fn invalid_restore_refuses_before_generation_exhaustion_and_never_changes_state() {
    let mut state = session_state(Intent::Play, Engine::Running);
    state.generation = u32::MAX;
    for (position_ms, speed_permille, reason) in [
        (60_001, 999, Rejection::OutOfBounds),
        (60_000, 999, Rejection::UnsupportedSpeed),
        (60_000, 1000, Rejection::GenerationExhausted),
    ] {
        assert_rejected(
            state.clone(),
            Event::RestoreRequested {
                snapshot: ResumeSnapshot {
                    source: source(),
                    position_ms,
                    speed_permille,
                },
            },
            reason,
        );
    }
}

#[test]
fn controls_without_a_session_reject_without_allocating_a_generation() {
    let events = [
        Event::PlayRequested,
        Event::PauseRequested,
        Event::SeekRequested { position_ms: 0 },
        Event::SkipRequested { delta_ms: 1 },
        Event::SpeedRequested {
            speed_permille: 1250,
        },
    ];
    for event in events {
        assert_rejected(PlayerState::default(), event, Rejection::NoSession);
    }
    assert_rejected(
        PlayerState::default(),
        Event::SpeedRequested {
            speed_permille: 999,
        },
        Rejection::UnsupportedSpeed,
    );
}

#[test]
fn play_during_preparation_has_a_self_contained_new_generation_path() {
    let state = session_state(Intent::Pause(PauseCause::User), Engine::Preparing);
    let stale = state.identity().unwrap();
    let mut expected = state.clone();
    expected.generation = 8;
    expected.session.as_mut().unwrap().intent = Intent::Play;
    let transition = reduce(state, Event::PlayRequested);
    assert_eq!(
        transition,
        Transition {
            state: expected.clone(),
            effects: vec![Effect::StartEngine(command(8, 15_000))]
        }
    );
    assert_eq!(transition.state.status(), Status::Loading);
    assert_eq!(
        reduce(expected.clone(), Event::EngineReady { identity: stale }),
        Transition {
            state: expected.clone(),
            effects: vec![]
        }
    );
    assert_eq!(
        reduce(expected.clone(), Event::PlayRequested),
        Transition {
            state: expected.clone(),
            effects: vec![]
        }
    );
    let identity = expected.identity().unwrap();
    expected.session.as_mut().unwrap().engine = Engine::Running;
    assert_eq!(
        reduce(transition.state, Event::EngineStarted { identity }),
        Transition {
            state: expected,
            effects: vec![]
        }
    );
}

#[test]
fn pause_during_preparation_wins_over_stale_and_current_started_callbacks() {
    let state = session_state(Intent::Play, Engine::Preparing);
    let stale = state.identity().unwrap();
    let transition = reduce(state, Event::PauseRequested);
    let mut expected = session_state(Intent::Pause(PauseCause::User), Engine::Paused);
    expected.generation = 8;
    assert_eq!(
        transition,
        Transition {
            state: expected.clone(),
            effects: vec![Effect::PauseEngine(command(8, 15_000))]
        }
    );
    for identity in [stale, expected.identity().unwrap()] {
        for event in [
            Event::EngineStarted { identity },
            Event::EngineReady { identity },
            Event::EngineBuffering { identity },
        ] {
            assert_eq!(
                reduce(expected.clone(), event),
                Transition {
                    state: expected.clone(),
                    effects: vec![]
                }
            );
        }
    }
    assert_eq!(
        reduce(expected.clone(), Event::PauseRequested),
        Transition {
            state: expected,
            effects: vec![]
        }
    );
}

#[test]
fn seek_during_preparation_renews_the_complete_command_for_each_intent() {
    for intent in [Intent::Play, Intent::Pause(PauseCause::User)] {
        let state = session_state(intent, Engine::Preparing);
        let stale = state.identity().unwrap();
        let mut expected = state.clone();
        expected.generation = 8;
        expected.session.as_mut().unwrap().position_ms = 30_000;
        let effect = match intent {
            Intent::Play => Effect::StartEngine(command(8, 30_000)),
            Intent::Pause(_) => Effect::PrepareEngine(command(8, 30_000)),
        };
        assert_eq!(
            reduce(
                state,
                Event::SeekRequested {
                    position_ms: 30_000
                }
            ),
            Transition {
                state: expected.clone(),
                effects: vec![effect]
            }
        );
        assert_eq!(
            reduce(expected.clone(), Event::EngineReady { identity: stale }),
            Transition {
                state: expected.clone(),
                effects: vec![]
            }
        );
        let identity = expected.identity().unwrap();
        let event = match intent {
            Intent::Play => Event::EngineStarted { identity },
            Intent::Pause(_) => Event::EngineReady { identity },
        };
        let mut ready = expected.clone();
        ready.session.as_mut().unwrap().engine = match intent {
            Intent::Play => Engine::Running,
            Intent::Pause(_) => Engine::Ready,
        };
        assert_eq!(
            reduce(expected, event),
            Transition {
                state: ready,
                effects: vec![]
            }
        );
    }
}

#[test]
fn every_generation_allocating_command_rejects_exhaustion_transactionally() {
    let paused = session_state(Intent::Pause(PauseCause::User), Engine::Ready);
    let playing = session_state(Intent::Play, Engine::Running);
    let mut interrupted = playing.clone();
    interrupted.session.as_mut().unwrap().interruption = Interruption::Active;
    interrupted.session.as_mut().unwrap().engine = Engine::Paused;
    let other = Source::new(
        EpisodeId {
            work: WorkId::River,
            index: 0,
        },
        90_000,
    )
    .unwrap();
    let cases = [
        (
            PlayerState::default(),
            Event::SourceSelected {
                source: other,
                position_ms: 0,
            },
        ),
        (
            paused.clone(),
            Event::SourceSelected {
                source: other,
                position_ms: 0,
            },
        ),
        (
            paused.clone(),
            Event::RestoreRequested {
                snapshot: paused.session().unwrap().snapshot(),
            },
        ),
        (paused.clone(), Event::PlayRequested),
        (playing.clone(), Event::PauseRequested),
        (
            paused.clone(),
            Event::SeekRequested {
                position_ms: 20_000,
            },
        ),
        (paused.clone(), Event::SkipRequested { delta_ms: 1 }),
        (playing, Event::InterruptionBegan),
        (
            interrupted.clone(),
            Event::InterruptionEnded {
                resume_allowed: true,
            },
        ),
        (
            interrupted,
            Event::InterruptionEnded {
                resume_allowed: false,
            },
        ),
    ];
    for (mut state, event) in cases {
        state.generation = u32::MAX;
        assert_rejected(state, event, Rejection::GenerationExhausted);
    }
}

#[test]
fn generation_exhaustion_does_not_reject_operations_that_allocate_nothing() {
    let mut state = session_state(Intent::Play, Engine::Running);
    state.generation = u32::MAX;
    for event in [
        Event::PlayRequested,
        Event::SeekRequested {
            position_ms: 15_000,
        },
        Event::SourceSelected {
            source: source(),
            position_ms: 0,
        },
    ] {
        assert_eq!(
            reduce(state.clone(), event),
            Transition {
                state: state.clone(),
                effects: vec![]
            }
        );
    }
    let mut expected = state.clone();
    expected.session.as_mut().unwrap().speed_permille = 1500;
    let mut speed_command = command(u32::MAX, 15_000);
    speed_command.speed_permille = 1500;
    assert_eq!(
        reduce(
            state,
            Event::SpeedRequested {
                speed_permille: 1500
            }
        ),
        Transition {
            state: expected,
            effects: vec![Effect::SetSpeed(speed_command)]
        }
    );
}

#[test]
fn rejected_absolute_positions_and_speeds_leave_every_root_fact_unchanged() {
    let state = session_state(Intent::Play, Engine::Running);
    let identity = state.identity().unwrap();
    for event in [
        Event::SeekRequested {
            position_ms: 60_001,
        },
        Event::EnginePosition {
            identity,
            position_ms: u32::MAX,
        },
        Event::RestoreRequested {
            snapshot: ResumeSnapshot {
                source: source(),
                position_ms: 60_001,
                speed_permille: 1250,
            },
        },
    ] {
        assert_rejected(state.clone(), event, Rejection::OutOfBounds);
    }
    for speed_permille in [0, 749, 751, 999, 2001, u16::MAX] {
        assert_rejected(
            state.clone(),
            Event::SpeedRequested { speed_permille },
            Rejection::UnsupportedSpeed,
        );
        assert_rejected(
            state.clone(),
            Event::RestoreRequested {
                snapshot: ResumeSnapshot {
                    source: source(),
                    position_ms: 15_000,
                    speed_permille,
                },
            },
            Rejection::UnsupportedSpeed,
        );
    }
}

#[test]
fn stale_out_of_bounds_callbacks_are_no_ops_before_payload_validation() {
    let state = session_state(Intent::Play, Engine::Running);
    let identities = [
        CallbackIdentity {
            generation: 6,
            ..state.identity().unwrap()
        },
        CallbackIdentity {
            track: TrackId::Fixture(EpisodeId {
                work: WorkId::River,
                index: 0,
            }),
            generation: 7,
        },
    ];
    for identity in identities {
        assert_eq!(
            reduce(
                state.clone(),
                Event::EnginePosition {
                    identity,
                    position_ms: u32::MAX
                }
            ),
            Transition {
                state: state.clone(),
                effects: vec![]
            }
        );
    }
}

#[test]
fn relative_skips_clamp_even_at_signed_integer_extremes_and_do_not_complete() {
    for (delta_ms, position_ms) in [(i32::MIN, 0), (i32::MAX, 60_000)] {
        let state = session_state(Intent::Play, Engine::Running);
        let mut expected = state.clone();
        expected.generation = 8;
        expected.session.as_mut().unwrap().position_ms = position_ms;
        let transition = reduce(state, Event::SkipRequested { delta_ms });
        assert_eq!(
            transition,
            Transition {
                state: expected,
                effects: vec![Effect::SeekEngine(command(8, position_ms))]
            }
        );
        assert_eq!(transition.state.status(), Status::Playing);
    }
}

#[test]
fn cold_restore_preserves_the_snapshot_and_never_requests_start() {
    let snapshot = ResumeSnapshot {
        source: source(),
        position_ms: 30_000,
        speed_permille: 1250,
    };
    let transition = reduce(PlayerState::default(), Event::RestoreRequested { snapshot });
    let mut expected = session_state(Intent::Pause(PauseCause::ColdRestore), Engine::Preparing);
    expected.generation = 1;
    expected.session.as_mut().unwrap().position_ms = 30_000;
    assert_eq!(
        transition,
        Transition {
            state: expected,
            effects: vec![
                Effect::PrepareEngine(command(1, 30_000)),
                Effect::OfferResume {
                    source: source(),
                    position_ms: 30_000
                }
            ],
        }
    );
    assert_eq!(transition.state.session().unwrap().snapshot(), snapshot);
}

#[test]
fn interruption_requires_explicit_permission_and_preserved_play_intent_to_resume() {
    for (intent, resume_allowed, starts) in [
        (Intent::Play, true, true),
        (Intent::Play, false, false),
        (Intent::Pause(PauseCause::User), true, false),
        (Intent::Pause(PauseCause::User), false, false),
    ] {
        let mut state = session_state(intent, Engine::Paused);
        state.session.as_mut().unwrap().interruption = Interruption::Active;
        assert_rejected(state.clone(), Event::PlayRequested, Rejection::Interrupted);
        let mut expected = state.clone();
        expected.generation = 8;
        let session = expected.session.as_mut().unwrap();
        session.interruption = Interruption::None;
        if starts {
            session.engine = Engine::Preparing;
        } else if intent == Intent::Play {
            session.intent = Intent::Pause(PauseCause::Interruption);
        }
        let effects = if starts {
            vec![Effect::StartEngine(command(8, 15_000))]
        } else {
            vec![]
        };
        assert_eq!(
            reduce(state, Event::InterruptionEnded { resume_allowed }),
            Transition {
                state: expected.clone(),
                effects
            }
        );
        assert_eq!(
            reduce(
                expected.clone(),
                Event::InterruptionEnded { resume_allowed }
            ),
            Transition {
                state: expected,
                effects: vec![]
            }
        );
    }
}

#[test]
fn end_is_an_observation_and_only_explicit_play_restarts_the_exact_end() {
    let state = session_state(Intent::Play, Engine::Running);
    let identity = state.identity().unwrap();
    let mut ended = state.clone();
    let session = ended.session.as_mut().unwrap();
    session.position_ms = 60_000;
    session.intent = Intent::Pause(PauseCause::User);
    session.engine = Engine::Ended;
    assert_eq!(
        reduce(state, Event::EngineEnded { identity }),
        Transition {
            state: ended.clone(),
            effects: vec![]
        }
    );
    assert_eq!(ended.status(), Status::Ended);
    assert_eq!(
        reduce(ended.clone(), Event::EngineReady { identity }),
        Transition {
            state: ended.clone(),
            effects: vec![]
        }
    );
    let mut restarted = ended.clone();
    restarted.generation = 8;
    let session = restarted.session.as_mut().unwrap();
    session.position_ms = 0;
    session.intent = Intent::Play;
    session.engine = Engine::Preparing;
    assert_eq!(
        reduce(ended, Event::PlayRequested),
        Transition {
            state: restarted,
            effects: vec![Effect::StartEngine(command(8, 0))]
        }
    );
}

fn assert_root_invariants(state: &PlayerState) {
    let Some(session) = state.session() else {
        assert_eq!(state.identity(), None);
        assert_eq!(state.status(), Status::Idle);
        return;
    };
    assert!(state.generation() > 0);
    assert!(session.position_ms() <= session.source().duration_ms());
    assert!([750, 1000, 1250, 1500, 2000].contains(&session.speed_permille()));
    assert_eq!(session.validated_snapshot().raw(), session.snapshot());
    assert_eq!(
        state.identity(),
        Some(CallbackIdentity {
            track: session.source().track(),
            generation: state.generation()
        })
    );
    assert!(state.accepts(state.identity().unwrap()));
    assert_ne!(state.status(), Status::Idle);
    if state.status() == Status::Playing {
        assert_eq!(session.intent(), Intent::Play);
        assert_eq!(session.engine(), Engine::Running);
        assert_eq!(session.interruption(), Interruption::None);
    }
    if session.interruption() == Interruption::Active {
        assert!(!matches!(
            state.status(),
            Status::Playing | Status::Buffering
        ));
    }
}

fn sequence_events(state: &PlayerState) -> Vec<Event> {
    let identity = state.identity().unwrap_or(CallbackIdentity {
        track: source().track(),
        generation: 0,
    });
    let stale = CallbackIdentity {
        generation: identity.generation.wrapping_add(1),
        ..identity
    };
    vec![
        Event::SourceSelected {
            source: source(),
            position_ms: 0,
        },
        Event::PlayRequested,
        Event::PauseRequested,
        Event::SeekRequested {
            position_ms: 60_000,
        },
        Event::SeekRequested {
            position_ms: 60_001,
        },
        Event::SkipRequested { delta_ms: i32::MIN },
        Event::SpeedRequested {
            speed_permille: 1500,
        },
        Event::SpeedRequested { speed_permille: 0 },
        Event::RestoreRequested {
            snapshot: ResumeSnapshot {
                source: source(),
                position_ms: 15_000,
                speed_permille: 1250,
            },
        },
        Event::InterruptionBegan,
        Event::InterruptionEnded {
            resume_allowed: true,
        },
        Event::InterruptionEnded {
            resume_allowed: false,
        },
        Event::EngineReady { identity },
        Event::EngineStarted { identity },
        Event::EnginePaused { identity },
        Event::EngineBuffering { identity },
        Event::EnginePosition {
            identity,
            position_ms: 60_001,
        },
        Event::EngineEnded { identity },
        Event::EngineStarted { identity: stale },
    ]
}

#[test]
fn bounded_event_sequences_are_deterministic_immutable_and_preserve_root_invariants() {
    let mut frontier = vec![PlayerState::default()];
    // Enumerate every sequence of length <= 3 over this 19-event partition, without randomness.
    for _ in 0..3 {
        let mut next_frontier = Vec::new();
        for state in frontier {
            let original = state.clone();
            assert_root_invariants(&state);
            for event in sequence_events(&state) {
                let transition = reduce(state.clone(), event.clone());
                assert_eq!(transition, reduce(state.clone(), event));
                assert_eq!(state, original);
                assert_root_invariants(&transition.state);
                if matches!(transition.effects.first(), Some(Effect::Rejected(_))) {
                    assert_eq!(transition.state, state);
                    assert_eq!(transition.effects.len(), 1);
                }
                for effect in &transition.effects {
                    let command = match effect {
                        Effect::PrepareEngine(command)
                        | Effect::StartEngine(command)
                        | Effect::PauseEngine(command)
                        | Effect::SeekEngine(command)
                        | Effect::SetSpeed(command) => command,
                        Effect::ReleaseEngine
                        | Effect::OfferResume { .. }
                        | Effect::Rejected(_) => continue,
                    };
                    let session = transition.state.session().unwrap();
                    assert_eq!(Some(command.identity), transition.state.identity());
                    assert_eq!(command.source, session.source());
                    assert_eq!(command.position_ms, session.position_ms());
                    assert_eq!(command.speed_permille, session.speed_permille());
                    if matches!(effect, Effect::StartEngine(_)) {
                        assert_eq!(session.intent(), Intent::Play);
                        assert_eq!(session.interruption(), Interruption::None);
                    }
                }
                next_frontier.push(transition.state);
            }
        }
        frontier = next_frontier;
    }
}

#[test]
fn seeking_to_duration_does_not_turn_repeated_play_into_a_restart() {
    for engine in [
        Engine::Preparing,
        Engine::Ready,
        Engine::Running,
        Engine::Paused,
        Engine::Buffering,
    ] {
        let playing = session_state(Intent::Play, engine);
        let sought = reduce(
            playing,
            Event::SeekRequested {
                position_ms: 60_000,
            },
        );
        let at_end = sought.state;
        assert_eq!(at_end.session().unwrap().position_ms(), 60_000);
        assert_eq!(at_end.session().unwrap().intent(), Intent::Play);
        assert_eq!(at_end.session().unwrap().engine(), engine);
        assert_eq!(
            reduce(at_end.clone(), Event::PlayRequested),
            Transition {
                state: at_end.clone(),
                effects: vec![]
            },
            "duplicate Play remains a no-op at duration with {engine:?}",
        );
        let paused = reduce(at_end.clone(), Event::PauseRequested);
        let restarted = reduce(paused.state, Event::PlayRequested);
        assert_eq!(restarted.state.generation(), 10);
        assert_eq!(restarted.state.session().unwrap().position_ms(), 0);
        assert_eq!(restarted.effects, vec![Effect::StartEngine(command(10, 0))]);
        let interrupted = reduce(at_end, Event::InterruptionBegan).state;
        assert_rejected(interrupted, Event::PlayRequested, Rejection::Interrupted);
    }
}

fn playing_public() -> (PlayerState, CallbackIdentity) {
    let source = crate::core::public_track::testing::source(1);
    let selected = reduce(
        PlayerState::default(),
        Event::SourceSelected {
            source,
            position_ms: 0,
        },
    );
    let started = reduce(selected.state, Event::PlayRequested);
    let identity = started.state.identity().unwrap();
    let running = reduce(started.state, Event::EngineStarted { identity });
    assert_eq!(running.state.status(), Status::Playing);
    (running.state, identity)
}

#[test]
fn an_engine_failure_names_its_reason_and_withdraws_the_play_intent() {
    for failure in [
        Failure::PlayRefused,
        Failure::Unavailable,
        Failure::Network,
        Failure::Decode,
        Failure::Protocol,
        Failure::IdentityChanged,
    ] {
        let (state, identity) = playing_public();
        let failed = reduce(state, Event::EngineFailed { identity, failure });
        assert_eq!(failed.state.status(), Status::Failed(failure));
        assert!(failed.effects.is_empty());
        let session = failed.state.session().unwrap();
        // The listener is never told it is playing, and a Play press is a real retry.
        assert_eq!(session.intent(), Intent::Pause(PauseCause::User));
        assert_eq!(session.engine(), Engine::Failed(failure));
    }
}

#[test]
fn a_failure_of_a_stale_identity_changes_nothing() {
    let (state, identity) = playing_public();
    let paused = reduce(state, Event::PauseRequested);
    let stale = CallbackIdentity {
        generation: identity.generation,
        ..identity
    };
    let ignored = reduce(
        paused.state.clone(),
        Event::EngineFailed {
            identity: stale,
            failure: Failure::Network,
        },
    );
    assert_eq!(ignored.state, paused.state);
    assert!(ignored.effects.is_empty());
}

#[test]
fn a_late_engine_fact_cannot_clear_a_failure_but_a_new_play_intent_can() {
    let (state, identity) = playing_public();
    let failed = reduce(
        state,
        Event::EngineFailed {
            identity,
            failure: Failure::Unavailable,
        },
    )
    .state;
    for late in [
        Event::EngineReady { identity },
        Event::EngineStarted { identity },
        Event::EnginePaused { identity },
        Event::EngineBuffering { identity },
        Event::EnginePosition {
            identity,
            position_ms: 1_000,
        },
        Event::EngineEnded { identity },
    ] {
        let after = reduce(failed.clone(), late);
        assert_eq!(after.state.status(), Status::Failed(Failure::Unavailable));
    }
    let retry = reduce(failed, Event::PlayRequested);
    assert_eq!(retry.state.status(), Status::Loading);
    assert!(retry.state.generation() > identity.generation);
    assert!(matches!(retry.effects[..], [Effect::StartEngine(_)]));
}

#[test]
fn a_failure_after_a_failure_replaces_the_reason_for_the_same_identity() {
    let (state, identity) = playing_public();
    let first = reduce(
        state,
        Event::EngineFailed {
            identity,
            failure: Failure::Network,
        },
    )
    .state;
    let second = reduce(
        first,
        Event::EngineFailed {
            identity,
            failure: Failure::Unavailable,
        },
    );
    assert_eq!(second.state.status(), Status::Failed(Failure::Unavailable));
}

#[test]
fn an_interruption_does_not_hide_a_failure() {
    let (state, identity) = playing_public();
    let failed = reduce(
        state,
        Event::EngineFailed {
            identity,
            failure: Failure::Decode,
        },
    )
    .state;
    let interrupted = reduce(failed, Event::InterruptionBegan);
    assert_eq!(
        interrupted.state.session().unwrap().engine(),
        Engine::Failed(Failure::Decode)
    );
}

#[test]
fn seeking_after_a_failure_keeps_the_failure_and_the_new_position() {
    let (state, identity) = playing_public();
    let failed = reduce(
        state,
        Event::EngineFailed {
            identity,
            failure: Failure::Network,
        },
    )
    .state;
    let seeked = reduce(failed, Event::SeekRequested { position_ms: 9_000 });
    assert_eq!(seeked.state.status(), Status::Failed(Failure::Network));
    assert_eq!(seeked.state.session().unwrap().position_ms(), 9_000);
    assert!(matches!(seeked.effects[..], [Effect::SeekEngine(_)]));
}

#[test]
fn a_public_source_selects_replays_and_fences_like_any_other_source() {
    let (state, identity) = playing_public();
    let replacement = crate::core::public_track::testing::source(2);
    let switched = reduce(
        state,
        Event::SourceSelected {
            source: replacement,
            position_ms: 0,
        },
    );
    assert_eq!(
        switched.state.session().map(|session| session.source()),
        Some(replacement)
    );
    assert!(!switched.state.accepts(identity));
    assert!(matches!(switched.effects[..], [Effect::PrepareEngine(_)]));
}
