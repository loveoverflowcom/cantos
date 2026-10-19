//! App-owned native range composition; no shared primitive or playback policy is introduced.
#[cfg(feature = "preview")]
use crate::app::context::TheatreContext;
use crate::{
    application::{player_session::PlayerSession, scrub::Scrub},
    core::playback::{Event, Status},
    messages::{self, Key, Locale},
    platform,
};
use leptos::prelude::*;

/// The strings a timeline shows. The isolated mock labels everything as simulated; the live
/// player does not, but both share one control and one scrub policy.
#[derive(Clone, Copy)]
pub(crate) struct TimelineCopy {
    label: Key,
    help: Key,
    preview: Key,
    pending: Key,
    status: fn(Status) -> Key,
}

impl TimelineCopy {
    #[cfg(feature = "preview")]
    pub(crate) const MOCK: Self = Self {
        label: Key::Timeline,
        help: Key::TimelineHelp,
        preview: Key::ScrubPreview,
        pending: Key::SeekPending,
        status: messages::status_key,
    };
    pub(crate) const LIVE: Self = Self {
        label: Key::ListenTimeline,
        help: Key::ListenTimelineHelp,
        preview: Key::ListenScrubPreview,
        pending: Key::ListenSeekPending,
        status: messages::listening_status_key,
    };
}

/// What a timeline needs from its owner: the one session, the locale and where seeks go.
/// The mock routes seeks through its context so simulated history can observe them.
#[derive(Clone, Copy)]
pub(crate) struct PlayerPanel {
    pub player: PlayerSession,
    pub locale: RwSignal<Locale>,
    pub dispatch: Callback<Event>,
    pub copy: TimelineCopy,
}

impl PlayerPanel {
    #[cfg(feature = "preview")]
    pub(crate) fn mock(ctx: TheatreContext) -> Self {
        Self {
            player: ctx.player,
            locale: ctx.locale,
            dispatch: Callback::new(move |event| ctx.playback(event)),
            copy: TimelineCopy::MOCK,
        }
    }
    fn text(self, key: Key) -> &'static str {
        messages::text(key, self.locale.get())
    }
}

#[derive(Clone, Copy)]
struct TimelineControl {
    panel: PlayerPanel,
    draft: RwSignal<Scrub>,
}

impl TimelineControl {
    fn new(panel: PlayerPanel) -> Self {
        Self {
            panel,
            draft: RwSignal::new(Scrub::FollowingEngine),
        }
    }

    fn current(self) -> Scrub {
        let draft = self
            .draft
            .get()
            .reconcile(self.panel.player.seek_binding.get(), None);
        match draft {
            Scrub::FollowingEngine | Scrub::Canceled => self
                .panel
                .player
                .pending_seek
                .get()
                .map(|seek| seek.presentation())
                .unwrap_or(draft),
            Scrub::Preview { .. } | Scrub::PendingSeek { .. } => draft,
        }
    }

    fn reconcile(self) {
        let next = self
            .draft
            .get()
            .reconcile(self.panel.player.seek_binding.get(), None);
        if next != self.draft.get_untracked() {
            self.draft.set(next);
        }
    }

    fn begin(self) {
        self.draft.set(Scrub::begin(
            self.panel.player.seek_binding.get_untracked(),
            self.panel
                .player
                .pending_seek
                .get_untracked()
                .map(|seek| seek.position_ms)
                .unwrap_or_else(|| self.panel.player.position_ms.get_untracked().unwrap_or(0)),
        ));
    }

    fn preview(self, position_ms: u32) {
        let binding = self.panel.player.seek_binding.get_untracked();
        // Reconcile at input time too: a delayed event cannot rebind an old gesture.
        let current = self
            .draft
            .get_untracked()
            .reconcile(binding, self.panel.player.observed_position.get_untracked());
        self.draft.set(current.preview(binding, position_ms));
    }

    fn commit(self) {
        let before = self.panel.player.seek_binding.get_untracked();
        let Some(position_ms) = self.draft.get_untracked().commit(before) else {
            return;
        };
        // Consume before dispatch: both native change and pointerup may arrive.
        self.draft.set(Scrub::FollowingEngine);
        self.panel
            .dispatch
            .run(Event::SeekRequested { position_ms });
    }

    fn cancel(self) {
        let _ = self.draft.try_update(|draft| *draft = draft.cancel());
    }

    fn release(self) {
        let _ = self.draft.try_update(|draft| *draft = draft.release());
    }

    /// Removing a focused slider (a route change) fires `blur` after its owner is disposed;
    /// touching a disposed signal would abort the whole page, so a late blur does nothing.
    fn blur(self, input: NodeRef<leptos::html::Input>, position: Memo<u32>) {
        let Some(position) = position.try_get_untracked() else {
            return;
        };
        self.cancel();
        self.release();
        platform::restore_range_value(input, position);
    }
}

#[component]
pub(super) fn PlayerTimeline(panel: PlayerPanel) -> impl IntoView {
    let control = TimelineControl::new(panel);
    let input = NodeRef::<leptos::html::Input>::new();
    Effect::new(move |_| control.reconcile());
    let phase = Memo::new(move |_| control.current());
    let position = Memo::new(move |_| {
        phase
            .get()
            .displayed_position()
            .unwrap_or_else(|| panel.player.position_ms.get().unwrap_or(0))
    });
    let duration = move || {
        panel
            .player
            .source
            .get()
            .map(|source| source.duration_ms())
            .unwrap_or(0)
    };
    let status = Memo::new(move |_| match phase.get() {
        Scrub::Preview { .. } => panel.copy.preview,
        Scrub::PendingSeek { .. } => panel.copy.pending,
        Scrub::FollowingEngine | Scrub::Canceled => (panel.copy.status)(panel.player.status.get()),
    });
    view! {
        <p role="status" aria-atomic="true">{move || panel.text(status.get())}</p>
        <label class="theatre-range-field" for="theatre-position">{move || panel.text(panel.copy.label)}
            <input id="theatre-position" node_ref=input type="range" min="0" step="1000"
                max=move || duration().to_string() disabled=move || duration() == 0
                prop:value=move || position.get().to_string()
                aria-valuetext=move || messages::position_description(panel.locale.get(), position.get() / 1000, duration() / 1000)
                aria-describedby="theatre-timeline-help"
                on:pointerdown=move |_| control.begin()
                on:input=move |event| {
                    if let Ok(value) = event_target_value(&event).parse::<u32>() { control.preview(value); }
                    // Also restore on ignored input: a canceled native drag can keep emitting.
                    platform::restore_range_value(input, position.get_untracked());
                }
                on:change=move |_| {
                    control.commit();
                    platform::restore_range_value(input, position.get_untracked());
                }
                on:pointerup=move |_| { control.commit(); control.release(); }
                on:pointercancel=move |_| {
                    control.cancel();
                    platform::restore_range_value(input, position.get_untracked());
                }
                on:keydown=move |event| {
                    if event.key() == "Escape" {
                        event.prevent_default();
                        control.cancel();
                        platform::restore_range_value(input, position.get_untracked());
                    } else if matches!(event.key().as_str(), "ArrowLeft" | "ArrowRight" | "ArrowUp" | "ArrowDown" | "Home" | "End" | "PageUp" | "PageDown") {
                        control.release();
                    }
                }
                on:blur=move |_| control.blur(input, position)/>
        </label>
        <p class="theatre-time">{move || format!("{} / {}", super::common::clock(position.get() / 1000), super::common::clock(duration() / 1000))}</p>
        <p id="theatre-timeline-help" class="theatre-muted">{move || panel.text(panel.copy.help)}</p>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{player_session::PlayerSession, ports::MediaPort},
        core::{
            ids::{EpisodeId, WorkId},
            playback::{Effect, Engine, EngineCommand, Intent, PauseCause, Source},
        },
    };
    use std::{cell::RefCell, rc::Rc};

    type Commands = Rc<RefCell<Vec<Effect>>>;
    struct DeferredMedia(Commands);
    impl MediaPort for DeferredMedia {
        fn execute(&mut self, effect: &Effect) -> Vec<Event> {
            self.0.borrow_mut().push(effect.clone());
            Vec::new()
        }
    }

    fn prepared() -> (TheatreContext, Commands) {
        let commands = Rc::new(RefCell::new(Vec::new()));
        let mut ctx = TheatreContext::new(NodeRef::new());
        ctx.player = PlayerSession::new(DeferredMedia(commands.clone()));
        ctx.open_episode(EpisodeId {
            work: WorkId::River,
            index: 0,
        });
        ctx.player.dispatch(Event::EngineReady {
            identity: ctx.player.state.get_untracked().identity().unwrap(),
        });
        commands.borrow_mut().clear();
        (ctx, commands)
    }

    #[test]
    fn drag_previews_leave_playback_untouched_and_change_plus_release_seeks_once_paused() {
        Owner::new().with(|| {
            let (baseline, baseline_commands) = prepared();
            // Replay develop's input handler through the same production session/reducer.
            // This measures host work, not native browser event ordering or latency.
            for input in 0..1000 {
                baseline.playback(Event::SeekRequested {
                    position_ms: if input % 2 == 0 { 55_000 } else { 5_000 },
                });
            }
            assert_eq!(baseline_commands.borrow().len(), 1000);
            let (ctx, commands) = prepared();
            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            let before = ctx.player.state.get_untracked();
            let progress = ctx.progress.get_untracked();
            control.begin();
            for input in 0..1000 {
                control.preview(if input % 2 == 0 { 55_000 } else { 5_000 });
            }
            assert_eq!(ctx.player.state.get_untracked(), before);
            assert_eq!(ctx.progress.get_untracked(), progress);
            assert!(commands.borrow().is_empty());
            assert_eq!(control.current().displayed_position(), Some(5_000));
            control.commit();
            control.commit(); // native change/pointerup in either order
            control.release();
            let issued = commands.borrow();
            assert_eq!(issued.len(), 1);
            assert!(
                matches!(&issued[0], Effect::SeekEngine(command) if command.position_ms == 5_000)
            );
            assert_eq!(
                ctx.player.state.get_untracked().session().unwrap().intent(),
                Intent::Pause(PauseCause::User)
            );
            assert!(matches!(control.current(), Scrub::PendingSeek { .. }));
            println!(
                "1000 drag inputs: baseline seeks={}; candidate seeks={}",
                baseline_commands.borrow().len(),
                issued.len()
            );
        });
    }

    #[test]
    fn engine_ticks_cannot_move_the_drag_thumb_and_only_matching_ack_finishes_pending() {
        Owner::new().with(|| {
            let (ctx, _) = prepared();
            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            let old = ctx.player.state.get_untracked().identity().unwrap();
            control.begin();
            control.preview(5_000);
            ctx.player.dispatch(Event::EnginePosition {
                identity: old,
                position_ms: 40_000,
            });
            assert_eq!(control.current().displayed_position(), Some(5_000));
            control.commit();
            let current = ctx.player.state.get_untracked().identity().unwrap();
            for (identity, position_ms) in [(old, 5_000), (current, 40_000)] {
                ctx.player.dispatch(Event::EnginePosition {
                    identity,
                    position_ms,
                });
                control.reconcile();
                assert_eq!(control.current().displayed_position(), Some(5_000));
            }
            let observation = ctx.player.observed_position.get_untracked();
            ctx.player.dispatch(Event::EnginePosition {
                identity: current,
                position_ms: u32::MAX,
            });
            assert_eq!(ctx.player.observed_position.get_untracked(), observation);
            ctx.player.dispatch(Event::EnginePosition {
                identity: current,
                position_ms: 5_000,
            });
            control.reconcile();
            assert_eq!(control.current(), Scrub::FollowingEngine);
            ctx.player.dispatch(Event::EnginePosition {
                identity: current,
                position_ms: 6_000,
            });
            assert_eq!(control.current(), Scrub::FollowingEngine);
        });
    }

    #[test]
    fn canceled_drag_and_source_switch_cannot_commit_late_input() {
        Owner::new().with(|| {
            let (ctx, commands) = prepared();
            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            control.begin();
            control.preview(50_000);
            control.cancel();
            control.preview(55_000);
            control.commit();
            control.release();
            assert!(commands.borrow().is_empty());
            assert_eq!(ctx.player.position_ms.get_untracked(), Some(0));
            control.begin();
            control.preview(50_000);
            ctx.open_episode(EpisodeId {
                work: WorkId::Mountain,
                index: 0,
            });
            commands.borrow_mut().clear();
            control.reconcile();
            control.preview(55_000);
            control.commit();
            control.release();
            assert!(commands.borrow().is_empty());
            assert_eq!(ctx.player.position_ms.get_untracked(), Some(0));
        });
    }

    #[test]
    fn generation_changing_controls_cancel_the_real_draft_before_or_after_reconciliation() {
        type Case = (&'static str, fn(TheatreContext));
        let cases: [Case; 8] = [
            ("play", |ctx| ctx.playback(Event::PlayRequested)),
            ("pause", |ctx| ctx.playback(Event::PauseRequested)),
            ("skip", |ctx| {
                ctx.playback(Event::SkipRequested { delta_ms: 15_000 });
            }),
            ("restore", |ctx| ctx.restore_position()),
            ("interruption", |ctx| ctx.playback(Event::InterruptionBegan)),
            ("reset", |ctx| ctx.reset_demo()),
            ("source replacement", |ctx| {
                ctx.open_episode(EpisodeId {
                    work: WorkId::Mountain,
                    index: 0,
                });
            }),
            ("same episode with new duration", |ctx| {
                let source = ctx.player.source.get_untracked().unwrap();
                ctx.playback(Event::SourceSelected {
                    source: Source::new(source.episode().unwrap(), source.duration_ms() + 1000)
                        .unwrap(),
                    position_ms: 0,
                });
            }),
        ];
        for (name, invalidate) in cases {
            for reconcile_first in [false, true] {
                Owner::new().with(|| {
                    let (ctx, commands) = prepared();
                    let control = TimelineControl::new(PlayerPanel::mock(ctx));
                    let original_binding = ctx.player.seek_binding.get_untracked();
                    control.begin();
                    control.preview(50_000);
                    assert!(matches!(
                        control.current(),
                        Scrub::Preview {
                            position_ms: 50_000,
                            ..
                        }
                    ));
                    invalidate(ctx);
                    assert_ne!(
                        ctx.player.seek_binding.get_untracked(),
                        original_binding,
                        "{name}"
                    );
                    let state = ctx.player.state.get_untracked();
                    let progress = ctx.progress.get_untracked();
                    let pending = ctx.player.pending_seek.get_untracked();
                    commands.borrow_mut().clear();
                    if reconcile_first {
                        control.reconcile();
                    }
                    // Late native input must be fenced even before the reactive effect runs.
                    control.preview(55_000);
                    assert_eq!(control.draft.get_untracked(), Scrub::Canceled, "{name}");
                    control.commit();
                    control.commit();
                    control.release();
                    assert_eq!(ctx.player.state.get_untracked(), state, "{name}");
                    assert_eq!(ctx.progress.get_untracked(), progress, "{name}");
                    assert_eq!(ctx.player.pending_seek.get_untracked(), pending, "{name}");
                    assert!(commands.borrow().is_empty(), "{name}");

                    // Cancellation fences one gesture, not every subsequent interaction.
                    control.preview(20_000);
                    control.commit();
                    assert_eq!(commands.borrow().len(), 1, "{name}");
                    assert_eq!(
                        control.current().displayed_position(),
                        Some(20_000),
                        "{name}"
                    );
                    assert_eq!(
                        ctx.player.position_ms.get_untracked(),
                        Some(20_000),
                        "{name}"
                    );
                });
            }
        }
    }

    #[test]
    fn preparing_seek_waits_for_position_not_ready_or_started_for_each_intent() {
        for play in [false, true] {
            Owner::new().with(|| {
                let (ctx, commands) = prepared();
                ctx.restore_position();
                if play {
                    ctx.playback(Event::PlayRequested);
                }
                let before = ctx.player.state.get_untracked();
                assert_eq!(before.session().unwrap().engine(), Engine::Preparing);
                let intent = before.session().unwrap().intent();
                let old = before.identity().unwrap();
                commands.borrow_mut().clear();
                let control = TimelineControl::new(PlayerPanel::mock(ctx));
                control.preview(5_000);
                control.commit();
                let current = ctx.player.state.get_untracked().identity().unwrap();
                assert_ne!(current, old);
                let command = EngineCommand {
                    identity: current,
                    source: before.session().unwrap().source(),
                    position_ms: 5_000,
                    speed_permille: before.session().unwrap().speed_permille(),
                };
                let expected_effects = vec![if play {
                    Effect::StartEngine(command)
                } else {
                    Effect::PrepareEngine(command)
                }];
                assert_eq!(*commands.borrow(), expected_effects);
                let pending = ctx.player.pending_seek.get_untracked();
                assert!(pending.is_some());
                assert!(matches!(control.current(), Scrub::PendingSeek { .. }));
                assert_eq!(ctx.player.observed_position.get_untracked(), None);
                for event in [
                    Event::EnginePosition {
                        identity: old,
                        position_ms: 5_000,
                    },
                    Event::EngineReady { identity: current },
                    Event::EngineStarted { identity: current },
                    Event::EnginePosition {
                        identity: current,
                        position_ms: 6_000,
                    },
                    Event::EnginePosition {
                        identity: current,
                        position_ms: u32::MAX,
                    },
                ] {
                    ctx.player.dispatch(event);
                    control.reconcile();
                    assert_eq!(ctx.player.pending_seek.get_untracked(), pending);
                    assert_eq!(control.current().displayed_position(), Some(5_000));
                    assert_eq!(
                        ctx.player.state.get_untracked().session().unwrap().intent(),
                        intent
                    );
                    assert_eq!(*commands.borrow(), expected_effects);
                }
                ctx.player.dispatch(Event::EnginePosition {
                    identity: current,
                    position_ms: 5_000,
                });
                control.reconcile();
                assert_eq!(ctx.player.pending_seek.get_untracked(), None);
                assert_eq!(control.current(), Scrub::FollowingEngine);
                assert_eq!(ctx.player.position_ms.get_untracked(), Some(5_000));
                assert_eq!(*commands.borrow(), expected_effects);
            });
        }
    }

    #[test]
    fn keyboard_changes_commit_individually_and_unchanged_values_issue_nothing() {
        Owner::new().with(|| {
            let (ctx, commands) = prepared();
            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            for position_ms in [
                1000,
                2000,
                0,
                ctx.player.source.get_untracked().unwrap().duration_ms(),
            ] {
                control.preview(position_ms);
                control.commit();
                control.commit();
                assert_eq!(ctx.player.position_ms.get_untracked(), Some(position_ms));
            }
            assert_eq!(commands.borrow().len(), 4);
            let same = ctx.player.position_ms.get_untracked().unwrap();
            let pending = ctx.player.pending_seek.get_untracked();
            control.preview(same);
            control.commit();
            assert_eq!(commands.borrow().len(), 4);
            assert_eq!(ctx.player.pending_seek.get_untracked(), pending);
            ctx.player.dispatch(Event::EnginePosition {
                identity: ctx.player.state.get_untracked().identity().unwrap(),
                position_ms: same,
            });
            assert_eq!(control.current(), Scrub::FollowingEngine);
        });
    }

    #[test]
    fn disposing_a_route_drops_its_draft_without_touching_the_root_player() {
        Owner::new().with(|| {
            let (ctx, commands) = prepared();
            let root = ctx.player.state.get_untracked();
            let route_owner = Owner::new();
            route_owner.with(|| {
                let control = TimelineControl::new(PlayerPanel::mock(ctx));
                control.begin();
                control.preview(55_000);
            });
            route_owner.cleanup();
            assert_eq!(ctx.player.state.get_untracked(), root);
            assert!(commands.borrow().is_empty());
            assert_eq!(
                TimelineControl::new(PlayerPanel::mock(ctx)).current(),
                Scrub::FollowingEngine
            );
        });
    }

    #[test]
    fn committed_seek_survives_route_disposal_and_mini_full_observe_the_same_pending_position() {
        Owner::new().with(|| {
            let (ctx, commands) = prepared();
            let old = ctx.player.state.get_untracked().identity().unwrap();
            let route_owner = Owner::new();
            route_owner.with(|| {
                let control = TimelineControl::new(PlayerPanel::mock(ctx));
                control.begin();
                control.preview(5_000);
                control.commit();
            });
            route_owner.cleanup();
            let current = ctx.player.state.get_untracked().identity().unwrap();
            let pending = ctx.player.pending_seek.get_untracked();
            assert!(pending.is_some());
            assert_eq!(
                TimelineControl::new(PlayerPanel::mock(ctx))
                    .current()
                    .displayed_position(),
                Some(5_000)
            );
            assert!(super::super::player::position_label(ctx).starts_with("0:05 /"));
            ctx.player.dispatch(Event::EngineEnded { identity: old });
            assert_eq!(ctx.player.pending_seek.get_untracked(), pending);
            ctx.player.dispatch(Event::EnginePosition {
                identity: current,
                position_ms: 5_000,
            });
            assert_eq!(ctx.player.pending_seek.get_untracked(), None);
            assert_eq!(commands.borrow().len(), 1);

            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            control.preview(10_000);
            control.commit();
            assert!(ctx.player.pending_seek.get_untracked().is_some());
            let current = ctx.player.state.get_untracked().identity().unwrap();
            ctx.player
                .dispatch(Event::EngineEnded { identity: current });
            assert_eq!(ctx.player.pending_seek.get_untracked(), None);
        });
    }

    #[test]
    fn rapid_second_seek_rejects_the_first_ack_and_pause_cancels_pending_without_starting() {
        Owner::new().with(|| {
            let (ctx, commands) = prepared();
            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            control.preview(5_000);
            control.commit();
            let first = ctx.player.state.get_untracked().identity().unwrap();
            control.preview(10_000);
            control.commit();
            let second = ctx.player.state.get_untracked().identity().unwrap();
            assert_ne!(first, second);
            ctx.player.dispatch(Event::EnginePosition {
                identity: first,
                position_ms: 5_000,
            });
            assert_eq!(control.current().displayed_position(), Some(10_000));
            assert_eq!(ctx.player.position_ms.get_untracked(), Some(10_000));
            ctx.playback(Event::PauseRequested);
            assert_eq!(ctx.player.pending_seek.get_untracked(), None);
            ctx.player.dispatch(Event::EnginePosition {
                identity: second,
                position_ms: 10_000,
            });
            assert_eq!(ctx.player.pending_seek.get_untracked(), None);
            assert!(!commands
                .borrow()
                .iter()
                .any(|effect| matches!(effect, Effect::StartEngine(_))));
        });
    }

    #[test]
    fn a_blur_delivered_after_the_timeline_was_disposed_does_nothing() {
        let owner = Owner::new();
        let (control, position, input) = owner.with(|| {
            let (ctx, _) = prepared();
            let control = TimelineControl::new(PlayerPanel::mock(ctx));
            let position = Memo::new(move |_| ctx.player.position_ms.get().unwrap_or(0));
            (control, position, NodeRef::<leptos::html::Input>::new())
        });
        control.blur(input, position);
        drop(owner);
        // Navigation removed the focused slider: the late blur must not touch freed state.
        control.blur(input, position);
        control.cancel();
        control.release();
        assert_eq!(position.try_get_untracked(), None);
    }
}
