//! The live full player and mini-player. Both read one root-owned `PlayerSession` and send the
//! same intents; neither owns audio, a timer or a request. Text comes from the display facts
//! captured when the listener chose the unit, and only while they describe the session's track.
use super::{
    common::clock,
    player_timeline::{PlayerPanel, PlayerTimeline, TimelineCopy},
};
use crate::contract::library::{Completion, ProgressKind};
use crate::{
    app::routes::Route,
    application::{
        account::RequestFence,
        account_view::{AccountViewState, SessionPhase},
        library_sync::{audio_identity_from_source, ProgressCommandPhase, ProgressConflict},
        player_session::PlayerSession,
        public_listen::NowPlaying,
    },
    core::playback::{CallbackIdentity, Event, Intent, Source, Status},
    messages::{self, Key, Locale},
};
use cantos_ui::{Button, ButtonState, ButtonVariant, DisabledReason, IconName};
use leptos::prelude::*;

/// Optional authenticated progress synchronization controls for the active source.
#[derive(Clone, Copy)]
pub(crate) struct PlayerSyncControls {
    pub account: ReadSignal<AccountViewState>,
    pub on_mark_completed: Callback<()>,
    pub on_restart: Callback<()>,
    pub on_resolve_conflict: Callback<(String, bool)>,
    pub on_retry_progress: Callback<String>,
}

/// What both players need from the root: the session, the display facts and the locale.
#[derive(Clone, Copy)]
pub(crate) struct LivePlayer {
    pub player: PlayerSession,
    pub now_playing: RwSignal<Option<NowPlaying>>,
    pub locale: RwSignal<Locale>,
}

impl LivePlayer {
    fn text(self, key: Key) -> &'static str {
        messages::text(key, self.locale.get())
    }

    fn panel(self) -> PlayerPanel {
        let player = self.player;
        PlayerPanel {
            player,
            locale: self.locale,
            dispatch: Callback::new(move |event| player.dispatch(event)),
            copy: TimelineCopy::LIVE,
        }
    }

    /// The facts describing the current source, or `None` when they belong to another track.
    fn facts(self) -> Option<NowPlaying> {
        let source = self.player.source.get()?;
        self.now_playing
            .get()
            .filter(|facts| facts.describes(source))
    }

    fn transport_key(self) -> Key {
        match self.player.transport.get() {
            Some(transport) if matches!(transport.status, Status::Failed(_)) => Key::PublicRetry,
            Some(transport) if transport.intent == Intent::Play => Key::ListenPause,
            Some(transport) if transport.replay => Key::ListenReplay,
            _ => Key::ListenPlay,
        }
    }

    /// One press: pause while the listener intends to play, otherwise ask to play.
    fn press_transport(self) {
        let playing = self
            .player
            .transport
            .get_untracked()
            .is_some_and(|transport| transport.intent == Intent::Play);
        self.player.dispatch(if playing {
            Event::PauseRequested
        } else {
            Event::PlayRequested
        });
    }

    fn position_label(self) -> String {
        self.player
            .view
            .get()
            .map(|view| {
                let shown = self
                    .player
                    .pending_seek
                    .get()
                    .map(|seek| seek.position_ms / 1000)
                    .unwrap_or(view.position);
                format!(
                    "{} / {}",
                    clock(self.locale.get(), shown),
                    clock(self.locale.get(), view.duration)
                )
            })
            .unwrap_or_default()
    }

    fn work_hash(self) -> String {
        self.facts()
            .map(|facts| {
                if facts.track().is_shared() {
                    Route::SharedWork(facts.track().work()).hash()
                } else {
                    Route::PublicWork(facts.track().work()).hash()
                }
            })
            .unwrap_or_else(|| Route::Discover.hash())
    }
}

#[component]
fn Transport(live: LivePlayer, #[prop(optional)] compact: bool) -> impl IntoView {
    let move_id = if compact {
        "theatre-mini-transport"
    } else {
        "theatre-listen-transport"
    };
    let name = move || {
        let title = live
            .facts()
            .map(|facts| facts.label().unit_title.clone())
            .unwrap_or_default();
        messages::message(
            live.locale.get(),
            crate::localization::generated::Message::TheatreShellListenTransportName {
                arg_action: live.text(live.transport_key()),
                arg_title: &(title),
            },
        )
    };
    let asset = move || {
        if live.transport_key() == Key::ListenPause {
            IconName::Pause
        } else {
            IconName::PlayArrow
        }
    };
    view! {
        <div class="theatre-transport-slot" data-compact=compact.to_string()>
            // Loading/buffering describe the engine, not a blocked action: Pause must work.
            <Button id=move_id class="theatre-transport" icon_signal=Signal::derive(asset)
                on_press=Callback::new(move |_| live.press_transport())>
                // Reserve the localized action envelope without duplicating accessible text.
                <span class="theatre-transport-label" aria-hidden="true">
                    {[Key::ListenPlay, Key::ListenPause, Key::ListenReplay, Key::PublicRetry]
                        .into_iter().map(|key| view! {
                            <span data-current=move || (live.transport_key() == key).to_string()>{move || live.text(key)}</span>
                        }).collect_view()}
                </span>
                <span class="cui-sr-only">{name}</span>
            </Button>
        </div>
    }
}

#[component]
fn Skip(live: LivePlayer, delta_ms: i32) -> impl IntoView {
    let (id, asset, label) = if delta_ms < 0 {
        (
            "theatre-listen-back",
            IconName::FastRewind,
            Key::ListenBack15,
        )
    } else {
        (
            "theatre-listen-forward",
            IconName::FastForward,
            Key::ListenForward15,
        )
    };
    view! {
        <Button id variant=ButtonVariant::Tonal icon=asset
            on_press=Callback::new(move |_| live.player.dispatch(Event::SkipRequested { delta_ms }))>
            {move || live.text(label)}
        </Button>
    }
}

#[component]
fn Speed(live: LivePlayer) -> impl IntoView {
    const SPEEDS: [u16; 5] = [750, 1000, 1250, 1500, 2000];
    let current = move || {
        live.player
            .view
            .get()
            .map(|view| view.speed * 10)
            .unwrap_or(1000)
    };
    view! {
        <div class="theatre-listen-speed">
            <label for="theatre-listen-speed">{move || live.text(Key::ListenSpeed)}</label>
            <select id="theatre-listen-speed" prop:value=move || current().to_string()
                on:change=move |event| {
                    if let Ok(speed_permille) = event_target_value(&event).parse::<u16>() {
                        live.player.dispatch(Event::SpeedRequested { speed_permille });
                    }
                }>
                {SPEEDS.into_iter().map(|speed| view! {
                    <option value=speed.to_string() selected=move || current() == speed>{move || messages::speed_label(live.locale.get(), speed / 10)}</option>
                }).collect_view()}
            </select>
        </div>
    }
}

/// `#player`: the player for the current source, or an honest empty state with a way out. It
/// never falls back to Home and never starts playback by being opened.
#[component]
pub(crate) fn LivePlayerScreen(
    live: LivePlayer,
    #[prop(optional)] sync_controls: Option<PlayerSyncControls>,
) -> impl IntoView {
    view! {
        <section class="theatre-live-player" aria-labelledby="theatre-player-heading">
            <h1 id="theatre-player-heading" tabindex="-1">{move || live.text(Key::Player)}</h1>
            <Show when=move || live.player.source.get().is_some()
                fallback=move || view! {
                    <div class="theatre-public-state theatre-panel">
                        <h2>{move || live.text(Key::ListenNothingSelected)}</h2>
                        <p>{move || live.text(Key::ListenNothingSelectedHelp)}</p>
                        <a class="theatre-text-link" href="#discover">{move || live.text(Key::Discover)}</a>
                    </div>
                }>
                <div class="theatre-player">
                    <div class="theatre-player-cover">
                        <div class="theatre-cover" aria-hidden="true">
                            <span class="theatre-cover-moon"></span>
                            <strong lang=move || live.facts().map(|facts| facts.label().language.clone()).unwrap_or_default()>
                                {move || live.facts().map(|facts| facts.label().work_title.clone()).unwrap_or_default()}
                            </strong>
                        </div>
                    </div>
                    <div class="theatre-player-body">
                        <h2 id="theatre-listen-title" lang=move || live.facts().map(|facts| facts.label().language.clone()).unwrap_or_default()>
                            {move || live.facts().map(|facts| facts.label().unit_title.clone()).unwrap_or_default()}
                        </h2>
                        <a class="theatre-text-link" href=move || live.work_hash()
                            lang=move || live.facts().map(|facts| facts.label().language.clone()).unwrap_or_default()>
                            <span class="cui-control-label">{move || live.facts().map(|facts| facts.label().work_title.clone()).unwrap_or_default()}</span>
                        </a>
                        <PlayerTimeline panel=live.panel()/>
                        <div class="theatre-actions">
                            <Skip live delta_ms=-15_000/>
                            <Transport live/>
                            <Skip live delta_ms=15_000/>
                        </div>
                        <Speed live/>
                        {move || sync_controls.map(|controls| view! {
                            <PlayerSyncPanel live controls/>
                        })}
                    </div>
                </div>
            </Show>
        </section>
    }
}

fn clock_ms(locale: Locale, ms: u64) -> String {
    messages::clock_seconds(locale, ms / 1000)
}

/// A transient recovery action describes this exact rendered command, not a later source
/// or conflict. The account and sync owners still admit and execute every intent.
#[derive(Clone)]
struct ProgressActionTarget {
    source: Source,
    identity: CallbackIdentity,
    fence: RequestFence,
    command: ProgressCommandPhase,
}

impl ProgressActionTarget {
    fn capture(live: LivePlayer, controls: PlayerSyncControls) -> Option<Self> {
        let source = live.player.source.get()?;
        let identity = live.player.seek_binding.get()?.identity;
        let audio = audio_identity_from_source(source)?;
        controls.account.with(|account| {
            Some(Self {
                source,
                identity,
                fence: account.fence()?,
                command: account
                    .sync()?
                    .progress_for(&audio.publication_id)?
                    .command
                    .clone(),
            })
        })
    }

    fn publication_if_current(
        &self,
        live: LivePlayer,
        controls: PlayerSyncControls,
    ) -> Option<String> {
        if live.player.source.get_untracked() != Some(self.source)
            || live
                .player
                .seek_binding
                .get_untracked()
                .map(|binding| binding.identity)
                != Some(self.identity)
        {
            return None;
        }
        let audio = audio_identity_from_source(self.source)?;
        controls.account.with_untracked(|account| {
            (account.accepts(&self.fence)
                && account
                    .sync()
                    .and_then(|sync| sync.progress_for(&audio.publication_id))
                    .is_some_and(|state| state.command == self.command))
            .then_some(audio.publication_id)
        })
    }
}

fn progress_action_state(
    command: &ProgressCommandPhase,
    completed: bool,
    action: ProgressKind,
    locale: Locale,
) -> ButtonState {
    let (reason, loading) = match command {
        ProgressCommandPhase::Pending { write, .. } => (
            messages::text(Key::LiveProgressPending, locale).to_owned(),
            write.kind == action,
        ),
        ProgressCommandPhase::UnknownOutcome { .. } => (
            messages::text(Key::LiveProgressUnknown, locale).to_owned(),
            false,
        ),
        ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses {
            local_position_ms,
            server,
            ..
        }) => (
            messages::message(
                locale,
                crate::localization::generated::Message::TheatreShellLiveProgressConflictPrompt {
                    arg_local: &clock_ms(locale, *local_position_ms),
                    arg_server: &clock_ms(locale, server.position_ms),
                },
            ),
            false,
        ),
        ProgressCommandPhase::Idle
        | ProgressCommandPhase::Failed { .. }
        | ProgressCommandPhase::Conflict(ProgressConflict::KeepServer { .. }) => {
            if action != ProgressKind::MarkCompleted || !completed {
                return ButtonState::Enabled;
            }
            (
                messages::text(Key::LiveResumeCompleted, locale).to_owned(),
                false,
            )
        }
    };
    DisabledReason::new(reason)
        .map(|reason| {
            if loading {
                ButtonState::Loading(reason)
            } else {
                ButtonState::Disabled(reason)
            }
        })
        .unwrap_or_default()
}

#[component]
fn PlayerSyncPanel(live: LivePlayer, controls: PlayerSyncControls) -> impl IntoView {
    let signed_in = move || {
        controls.account.with(|a| a.phase()) == SessionPhase::SignedIn
            && live
                .player
                .source
                .get()
                .and_then(audio_identity_from_source)
                .is_some()
    };
    let publication = move || {
        live.player
            .source
            .get()
            .and_then(audio_identity_from_source)
            .map(|a| a.publication_id)
    };
    let progress_state = move || {
        let pub_id = publication()?;
        controls.account.with(|a| {
            a.sync()
                .and_then(|sync| sync.progress_for(&pub_id))
                .cloned()
        })
    };
    let is_completed = move || {
        progress_state()
            .and_then(|state| state.record)
            .is_some_and(|p| p.completion == Completion::Completed)
    };
    let cmd = move || {
        progress_state()
            .map(|state| state.command)
            .unwrap_or(ProgressCommandPhase::Idle)
    };
    let restart_state = Signal::derive(move || {
        progress_action_state(
            &cmd(),
            is_completed(),
            ProgressKind::Restart,
            live.locale.get(),
        )
    });
    let completed_state = Signal::derive(move || {
        progress_action_state(
            &cmd(),
            is_completed(),
            ProgressKind::MarkCompleted,
            live.locale.get(),
        )
    });
    let conflict = move || match cmd() {
        ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses {
            local_position_ms,
            server,
            ..
        }) => Some((local_position_ms, server)),
        _ => None,
    };

    view! {
        <Show when=signed_in>
            <div class="theatre-player-sync">
                // The sync owner refuses ordinary intents until this choice is resolved.
                // Give the two admitted choices space instead of repeating their prompt.
                <Show when=move || conflict().is_none()>
                <div class="theatre-actions">
                    <Button id="theatre-player-restart" variant=ButtonVariant::Tonal icon=IconName::Refresh
                        state=restart_state
                        on_press=Callback::new(move |_| {
                            if signed_in() && restart_state.get_untracked().can_press() {
                                controls.on_restart.run(());
                            }
                        })>
                        {move || live.text(Key::LiveRestartFromBeginning)}
                    </Button>
                    <Button id="theatre-player-mark-completed" variant=ButtonVariant::Tonal icon=IconName::Check
                        state=completed_state
                        on_press=Callback::new(move |_| {
                            if signed_in() && completed_state.get_untracked().can_press() {
                                controls.on_mark_completed.run(());
                            }
                        })>
                        {move || if is_completed() {
                            live.text(Key::LiveResumeCompleted)
                        } else {
                            live.text(Key::LiveMarkCompleted)
                        }}
                    </Button>
                </div>
                </Show>
                {move || match cmd() {
                    // A direct action's shared Loading reason is the one live announcement.
                    ProgressCommandPhase::Pending { write, .. }
                        if !matches!(write.kind, ProgressKind::Restart | ProgressKind::MarkCompleted) => view! {
                        <p class="theatre-meta" role="status">{live.text(Key::LiveProgressPending)}</p>
                    }.into_any(),
                    ProgressCommandPhase::UnknownOutcome { .. } => view! {
                        <p class="theatre-meta" role="status">{live.text(Key::LiveProgressUnknown)}</p>
                    }.into_any(),
                    ProgressCommandPhase::Failed { .. } => {
                        let target = ProgressActionTarget::capture(live, controls);
                        view! {
                            <div class="theatre-inline-error">
                                <span id="theatre-player-progress-error" role="status">{move || live.text(Key::LiveProgressError)}</span>
                                <Button id="theatre-player-progress-retry" variant=ButtonVariant::Tonal
                                    described_by="theatre-player-progress-error"
                                    on_press=Callback::new(move |_| {
                                        if let Some(pub_id) = target.as_ref().and_then(|target| target.publication_if_current(live, controls)) {
                                            controls.on_retry_progress.run(pub_id);
                                        }
                                    })>
                                    {move || live.text(Key::PublicRetry)}
                                </Button>
                            </div>
                        }.into_any()
                    },
                    ProgressCommandPhase::Conflict(ProgressConflict::KeepServer { server, .. }) => view! {
                        <p class="theatre-meta" role="status">
                            {messages::message(live.locale.get(), crate::localization::generated::Message::TheatreShellLiveProgressKeepServer { arg_time: &(clock_ms(live.locale.get(), server.position_ms)) })}
                        </p>
                    }.into_any(),
                    _ => ().into_any(),
                }}
                {move || conflict().map(|(local_ms, server)| {
                    let here_target = ProgressActionTarget::capture(live, controls);
                    let other_target = here_target.clone();
                    let local_pos = clock_ms(live.locale.get(), local_ms);
                    let server_pos = clock_ms(live.locale.get(), server.position_ms);
                    let local_btn = local_pos.clone();
                    let server_btn = server_pos.clone();
                    view! {
                        <div class="theatre-conflict-card theatre-panel" role="region"
                            aria-label=move || live.text(Key::LiveProgressError)>
                            <p id="theatre-player-conflict-reason" class="theatre-meta">
                                {move || messages::message(live.locale.get(), crate::localization::generated::Message::TheatreShellLiveProgressConflictPrompt { arg_local: &(local_pos.clone()), arg_server: &(server_pos.clone()) })}
                            </p>
                            <div class="theatre-actions">
                                <Button id="theatre-player-resolve-here" variant=ButtonVariant::Tonal
                                    described_by="theatre-player-conflict-reason"
                                    on_press=Callback::new(move |_| {
                                        if let Some(pub_id) = here_target.as_ref().and_then(|target| target.publication_if_current(live, controls)) {
                                            controls.on_resolve_conflict.run((pub_id, true));
                                        }
                                    })>
                                    {move || messages::message(live.locale.get(), crate::localization::generated::Message::TheatreShellLiveProgressResolveHere { arg_time: &(local_btn.clone()) })}
                                </Button>
                                <Button id="theatre-player-resolve-other" variant=ButtonVariant::Tonal
                                    described_by="theatre-player-conflict-reason"
                                    on_press=Callback::new(move |_| {
                                        if let Some(pub_id) = other_target.as_ref().and_then(|target| target.publication_if_current(live, controls)) {
                                            controls.on_resolve_conflict.run((pub_id, false));
                                        }
                                    })>
                                    {move || messages::message(live.locale.get(), crate::localization::generated::Message::TheatreShellLiveProgressResolveOther { arg_time: &(server_btn.clone()) })}
                                </Button>
                            </div>
                        </div>
                    }
                })}
            </div>
        </Show>
    }
}

/// The compact player shown on every route except `#player`.
#[component]
pub(crate) fn LiveMiniPlayer(live: LivePlayer, route: RwSignal<Route>) -> impl IntoView {
    view! {
        <Show when=move || live.player.source.get().is_some() && route.get() != Route::Player>
            <MiniBar live/>
        </Show>
    }
}

#[component]
fn MiniBar(live: LivePlayer) -> impl IntoView {
    let bar = NodeRef::<leptos::html::Aside>::new();
    #[cfg(target_arch = "wasm32")]
    crate::platform::bind_mini_player_clearance(bar);
    let title = move || {
        live.facts()
            .map(|facts| facts.label().unit_title.clone())
            .unwrap_or_default()
    };
    let work = move || {
        live.facts()
            .map(|facts| facts.label().work_title.clone())
            .unwrap_or_default()
    };
    let language = move || {
        live.facts()
            .map(|facts| facts.label().language.clone())
            .unwrap_or_default()
    };
    let status = move || {
        live.text(if live.player.pending_seek.get().is_some() {
            Key::ListenSeekPending
        } else {
            messages::listening_status_key(live.player.status.get())
        })
    };
    view! {
        <aside class="theatre-mini-player" node_ref=bar aria-label=move || live.text(Key::ListenMiniPlayer)>
            <a class="theatre-mini-title" href="#player">
                <div class="theatre-mini-cover" aria-hidden="true">
                    <div class="theatre-cover"><span class="theatre-cover-moon"></span></div>
                </div>
                <span>
                    <strong lang=language>{title}</strong>
                    <small lang=language>{work}</small>
                    <small id="theatre-mini-status">{status}</small>
                </span>
            </a>
            <span class="theatre-time">{move || live.position_label()}</span>
            <Transport live compact=true/>
            <progress class="theatre-mini-progress" aria-hidden="true"
                max=move || live.player.source.get().map(|source| source.duration_ms()).unwrap_or(1).to_string()
                value=move || live.player.pending_seek.get().map(|seek| seek.position_ms).unwrap_or_else(|| live.player.position_ms.get().unwrap_or(0)).to_string()/>
        </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{
            ports::MediaPort,
            public_listen::{ListenLabel, NowPlaying},
        },
        core::{
            playback::{Effect, Failure, Status},
            public_track::testing,
        },
    };

    struct Deferred;
    impl MediaPort for Deferred {
        fn execute(&mut self, _: &Effect) -> Vec<Event> {
            Vec::new()
        }
    }

    fn live(locale: Locale) -> LivePlayer {
        LivePlayer {
            player: PlayerSession::new(Deferred),
            now_playing: RwSignal::new(None),
            locale: RwSignal::new(locale),
        }
    }
    fn choose(live: LivePlayer, n: u32, unit: &str) {
        let release = testing::release(n);
        live.now_playing.set(Some(NowPlaying::new(
            &release,
            ListenLabel {
                work_title: "Mùa gió chướng {x}".into(),
                unit_title: unit.into(),
                language: "vi-VN".into(),
            },
        )));
        live.player.dispatch(Event::SourceSelected {
            source: testing::source(n),
            position_ms: 0,
        });
    }
    fn screen(live: LivePlayer) -> String {
        view! { <LivePlayerScreen live/> }.to_html()
    }
    fn mini(live: LivePlayer, route: Route) -> String {
        view! { <LiveMiniPlayer live route=RwSignal::new(route)/> }.to_html()
    }
    fn identity(live: LivePlayer) -> crate::core::playback::CallbackIdentity {
        live.player.state.get_untracked().identity().unwrap()
    }

    fn sync_fixture(
        live: LivePlayer,
        kind: ProgressKind,
    ) -> (RwSignal<AccountViewState>, PlayerSyncControls, String) {
        use crate::application::{account::AccountScope, account_view::SessionCompletion};
        let mut state = AccountViewState::default();
        let refresh = state.begin_refresh().unwrap();
        assert!(matches!(
            state.finish_session(
                refresh,
                Ok(AccountScope::from_session(
                    "listener-a".into(),
                    uuid::Uuid::from_u128(10).to_string()
                )
                .unwrap()),
                || uuid::Uuid::from_u128(100).to_string()
            ),
            Ok(SessionCompletion::Authenticated(_))
        ));
        let operation = uuid::Uuid::from_u128(501).to_string();
        state.seed_sync(|sync, _| {
            assert!(sync
                .record_progress_intent(
                    audio_identity_from_source(live.player.source.get_untracked().unwrap())
                        .unwrap(),
                    45_000,
                    kind,
                    true,
                    1_000,
                    || operation.clone()
                )
                .is_some());
        });
        let account = RwSignal::new(state);
        let controls = PlayerSyncControls {
            account: account.read_only(),
            on_mark_completed: Callback::new(|_| {}),
            on_restart: Callback::new(|_| {}),
            on_resolve_conflict: Callback::new(|_| {}),
            on_retry_progress: Callback::new(|_| {}),
        };
        (account, controls, operation)
    }

    fn sync_html(live: LivePlayer, controls: PlayerSyncControls) -> String {
        view! { <PlayerSyncPanel live controls/> }.to_html()
    }

    fn button_tag<'a>(html: &'a str, id: &str) -> &'a str {
        html.split(&format!("id=\"{id}\""))
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap()
    }

    #[test]
    fn progress_pending_and_unknown_keep_their_reason_without_optimistic_completion_or_duplicate_announcements(
    ) {
        for locale in Locale::supported() {
            for kind in [
                ProgressKind::Restart,
                ProgressKind::MarkCompleted,
                ProgressKind::Checkpoint,
            ] {
                Owner::new().with(|| {
                    let live = live(locale);
                    choose(live, 1, "Chương một");
                    let (account, controls, operation) = sync_fixture(live, kind);
                    let html = sync_html(live, controls);
                    for (id, own_kind) in [
                        ("theatre-player-restart", ProgressKind::Restart),
                        ("theatre-player-mark-completed", ProgressKind::MarkCompleted),
                    ] {
                        let tag = button_tag(&html, id);
                        assert!(tag.contains("disabled"), "{tag}");
                        assert_eq!(
                            tag.contains("aria-busy=\"true\""),
                            own_kind == kind,
                            "{tag}"
                        );
                        assert!(
                            tag.contains(&format!("aria-describedby=\"{id}-reason\"")),
                            "{tag}"
                        );
                    }
                    assert_eq!(html.matches("role=\"status\"").count(), 1, "{html}");
                    assert!(html.contains(live.text(Key::LiveProgressPending)));
                    assert!(
                        !html.contains(live.text(Key::LiveResumeCompleted)),
                        "unacknowledged completion: {html}"
                    );
                    account.update(|account| {
                        account.seed_sync(|sync, _| {
                            assert!(sync.record_unknown_outcome(&operation));
                        });
                    });
                    let html = sync_html(live, controls);
                    assert!(html.contains(live.text(Key::LiveProgressUnknown)));
                    assert!(
                        !html.contains("aria-busy"),
                        "unknown is not a live request: {html}"
                    );
                    assert!(!html.contains("theatre-player-progress-retry"));
                    assert_eq!(html.matches("role=\"status\"").count(), 1, "{html}");
                    for id in ["theatre-player-restart", "theatre-player-mark-completed"] {
                        assert!(button_tag(&html, id).contains("data-state=\"disabled\""));
                    }
                });
            }
        }
    }

    #[test]
    fn progress_retry_targets_only_the_rendered_failure_source_and_session() {
        use crate::application::library_sync::CommandError;
        Owner::new().with(|| {
            let live = live(Locale::English);
            choose(live, 1, "Chapter one");
            let (account, controls, operation) = sync_fixture(live, ProgressKind::MarkCompleted);
            account.update(|account| {
                account.seed_sync(|sync, _| {
                    assert!(
                        sync.record_command_error(&operation, CommandError::Forbidden, || {
                            uuid::Uuid::from_u128(700).to_string()
                        })
                    );
                });
            });
            let target = ProgressActionTarget::capture(live, controls).unwrap();
            let publication = target.publication_if_current(live, controls).unwrap();
            let html = sync_html(live, controls);
            assert!(button_tag(&html, "theatre-player-progress-retry")
                .contains("aria-describedby=\"theatre-player-progress-error\""));
            assert!(html.contains(live.text(Key::LiveProgressError)));
            assert!(!html.contains(live.text(Key::LiveResumeCompleted)));
            choose(live, 2, "Chapter two");
            assert_eq!(target.publication_if_current(live, controls), None);
            assert!(!sync_html(live, controls).contains("theatre-player-progress-retry"));
            choose(live, 1, "Chapter one");
            assert_eq!(
                target.publication_if_current(live, controls),
                None,
                "A to B to A keeps a new playback binding"
            );
            account.update(|account| {
                account.seed_sync(|sync, _| {
                    assert!(sync
                        .retry_progress(
                            &publication,
                            || uuid::Uuid::from_u128(502).to_string(),
                            None
                        )
                        .is_some());
                });
            });
            assert_eq!(
                target.publication_if_current(live, controls),
                None,
                "old failure cannot retry a newer command"
            );
            let pending_target = ProgressActionTarget::capture(live, controls).unwrap();
            account.update(|account| {
                account.begin_logout().unwrap();
            });
            assert_eq!(pending_target.publication_if_current(live, controls), None);
            assert!(!sync_html(live, controls).contains("theatre-player-restart"));
        });
    }

    #[test]
    fn only_a_confirmed_completed_record_changes_the_completion_label() {
        use crate::contract::library::{MutationState, ProgressRecord, Receipt, WriteOutcome};
        for locale in Locale::supported() {
            Owner::new().with(|| {
                let live = live(locale);
                choose(live, 1, "Chương một");
                let (account, controls, operation) =
                    sync_fixture(live, ProgressKind::MarkCompleted);
                let audio = audio_identity_from_source(live.player.source.get_untracked().unwrap())
                    .unwrap();
                account.update(|account| {
                    account.seed_sync(|sync, _| {
                        sync.apply_receipt(
                            &operation,
                            Receipt {
                                operation_id: operation.clone(),
                                outcome: WriteOutcome::Accepted,
                                resolution: None,
                                state: MutationState::Progress {
                                    current: Some(ProgressRecord {
                                        position_ms: audio.duration_ms,
                                        audio,
                                        completion: Completion::Completed,
                                        kind: ProgressKind::MarkCompleted,
                                        revision: 1,
                                        listener_order: 1,
                                        accepted_at_ms: 2_000,
                                        written_by: uuid::Uuid::from_u128(100).to_string(),
                                    }),
                                },
                            },
                            false,
                            || uuid::Uuid::from_u128(502).to_string(),
                            None,
                        )
                        .unwrap();
                    });
                });
                let html = sync_html(live, controls);
                assert!(html.contains(live.text(Key::LiveResumeCompleted)));
                assert!(button_tag(&html, "theatre-player-mark-completed")
                    .contains("data-state=\"disabled\""));
                assert!(
                    button_tag(&html, "theatre-player-restart").contains("data-state=\"enabled\"")
                );
                assert!(!html.contains("aria-busy"));
            });
        }
    }

    #[test]
    fn shared_transport_stays_enabled_for_pause_and_retry_in_both_players() {
        for compact in [false, true] {
            for locale in Locale::supported() {
                Owner::new().with(|| {
                    let live = live(locale);
                    choose(live, 1, "Chương một");
                    let assert_action = |key| {
                        assert_eq!(live.transport_key(), key);
                        let html = view! { <Transport live compact/> }.to_html();
                        let button = html.split("<button ").nth(1).unwrap();
                        let tag = button.split('>').next().unwrap();
                        assert!(tag.contains("cui-button"), "shared anatomy: {tag}");
                        assert!(tag.contains("data-state=\"enabled\""), "{tag}");
                        assert!(
                            !tag.contains("disabled") && !tag.contains("aria-busy"),
                            "{tag}"
                        );
                        assert!(!tag.contains("aria-pressed"), "changing action, not toggle");
                        assert!(html.contains("Chương một"));
                        assert!(html.contains("aria-hidden=\"true\""));
                        assert!(
                            html.contains(&format!("data-current=\"true\">{}", live.text(key))),
                            "visible current label: {html}"
                        );
                    };
                    assert_action(Key::ListenPlay);
                    live.press_transport();
                    assert_eq!(live.player.status.get_untracked(), Status::Loading);
                    assert_action(Key::ListenPause);
                    live.press_transport();
                    assert_action(Key::ListenPlay);
                    live.press_transport();
                    let id = identity(live);
                    live.player.dispatch(Event::EngineStarted { identity: id });
                    assert_action(Key::ListenPause);
                    live.player
                        .dispatch(Event::EngineBuffering { identity: id });
                    assert_action(Key::ListenPause);
                    live.press_transport();
                    assert_action(Key::ListenPlay);
                    for failure in [Failure::Network, Failure::Unavailable, Failure::PlayRefused] {
                        live.press_transport();
                        let id = identity(live);
                        live.player.dispatch(Event::EngineFailed {
                            identity: id,
                            failure,
                        });
                        assert_action(Key::PublicRetry);
                    }
                    live.press_transport();
                    let id = identity(live);
                    live.player.dispatch(Event::EngineStarted { identity: id });
                    live.player.dispatch(Event::EngineEnded { identity: id });
                    assert_action(Key::ListenReplay);
                });
            }
        }
    }

    #[test]
    fn skip_actions_use_shared_tonal_anatomy_with_distinct_native_ids() {
        Owner::new().with(|| {
            let live = live(Locale::English);
            choose(live, 1, "Chapter one");
            for (delta_ms, id) in [
                (-15_000, "theatre-listen-back"),
                (15_000, "theatre-listen-forward"),
            ] {
                let html = view! { <Skip live delta_ms/> }.to_html();
                assert!(html.contains(&format!("id=\"{id}\"")), "{html}");
                assert!(
                    html.contains("cui-button") && html.contains("data-variant=\"tonal\""),
                    "{html}"
                );
                assert!(html.contains("data-state=\"enabled\""));
                assert!(!html.contains("theatre-action"));
            }
        });
    }

    #[test]
    fn the_player_route_without_a_source_is_an_honest_empty_state_not_home() {
        for (locale, heading, empty, help) in [
            (
                Locale::Vietnamese,
                "Đang nghe",
                "Chưa chọn nội dung để nghe",
                "Mở một tác phẩm đã công bố",
            ),
            (
                Locale::English,
                "Now listening",
                "Nothing selected to listen to",
                "Open a published story",
            ),
        ] {
            Owner::new().with(|| {
                let html = screen(live(locale));
                for expected in [heading, empty, help, "href=\"#discover\""] {
                    assert!(html.contains(expected), "{expected}: {html}");
                }
                for absent in ["<button", "type=\"range\"", "theatre-player\""] {
                    assert!(!html.contains(absent), "{absent}");
                }
            });
        }
    }

    #[test]
    fn a_chosen_release_shows_its_own_facts_and_never_starts_by_being_opened() {
        Owner::new().with(|| {
            let live = live(Locale::Vietnamese);
            choose(live, 1, "Chương một: Bến cũ");
            let html = screen(live);
            let work = testing::release(1).track().work();
            for expected in [
                "Chương một: Bến cũ",
                "Mùa gió chướng {x}",
                &format!("href=\"#work/{work}\""),
                "Phát: Chương một: Bến cũ",
                "Đang tạm dừng",
                "Vị trí nghe",
                "Lùi 15 giây",
                "Tiến 15 giây",
                "Tốc độ nghe",
                "id=\"theatre-listen-transport\"",
                "<select id=\"theatre-listen-speed\"",
                "0,75×",
                "1,5×",
                "2×",
            ] {
                assert!(html.contains(expected), "{expected}: {html}");
            }
            assert_eq!(
                live.player.status.get_untracked(),
                Status::Paused(crate::core::playback::PauseCause::User)
            );
        });
    }

    #[test]
    fn the_indicator_follows_the_engine_acknowledgement_not_the_press() {
        Owner::new().with(|| {
            let live = live(Locale::Vietnamese);
            choose(live, 1, "Chương một");
            live.player.dispatch(Event::PlayRequested);
            let pressed = screen(live);
            assert!(pressed.contains("Đang chuẩn bị âm thanh…"), "{pressed}");
            assert!(
                pressed.contains("Tạm dừng: Chương một"),
                "a press can be cancelled"
            );
            assert!(
                !pressed.contains("Đang phát"),
                "not playing before the engine says so"
            );
            let id = identity(live);
            live.player.dispatch(Event::EngineStarted { identity: id });
            assert!(screen(live).contains("Đang phát"));
            live.player
                .dispatch(Event::EngineBuffering { identity: id });
            assert!(screen(live).contains("Đang đệm âm thanh…"));
        });
    }

    #[test]
    fn a_failure_is_named_and_the_transport_becomes_a_retry() {
        for (failure, vi, en) in [
            (
                Failure::PlayRefused,
                "Trình duyệt chưa cho phát",
                "The browser did not start playback",
            ),
            (
                Failure::Unavailable,
                "không còn khả dụng",
                "no longer available",
            ),
            (Failure::Network, "Mất kết nối", "connection failed"),
            (Failure::Decode, "không giải mã được", "could not decode"),
            (
                Failure::Protocol,
                "dữ liệu phát không hợp lệ",
                "invalid playback data",
            ),
            (
                Failure::IdentityChanged,
                "Bản nghe đã đổi",
                "release changed",
            ),
        ] {
            for (locale, expected, retry) in [
                (Locale::Vietnamese, vi, "Thử lại: Chương một"),
                (Locale::English, en, "Try again: Chương một"),
            ] {
                Owner::new().with(|| {
                    let live = live(locale);
                    choose(live, 1, "Chương một");
                    live.player.dispatch(Event::PlayRequested);
                    let id = identity(live);
                    live.player.dispatch(Event::EngineFailed {
                        identity: id,
                        failure,
                    });
                    let html = screen(live);
                    assert!(html.contains(expected), "{expected}: {html}");
                    assert!(html.contains(retry), "{retry}: {html}");
                    assert!(mini(live, Route::Discover).contains(expected));
                });
            }
        }
    }

    #[test]
    fn display_facts_of_another_release_are_never_shown_under_the_current_audio() {
        Owner::new().with(|| {
            let live = live(Locale::Vietnamese);
            choose(live, 1, "Chương một");
            // The session moves to another release whose facts were never captured.
            live.player.dispatch(Event::SourceSelected {
                source: testing::source(2),
                position_ms: 0,
            });
            let html = screen(live);
            assert!(
                !html.contains("Chương một") && !html.contains("Mùa gió chướng"),
                "{html}"
            );
            assert!(mini(live, Route::Library).contains("theatre-mini-player"));
        });
    }

    #[test]
    fn the_mini_player_follows_the_same_session_everywhere_but_the_player_route() {
        Owner::new().with(|| {
            let live = live(Locale::Vietnamese);
            assert!(!mini(live, Route::Discover).contains("theatre-mini-player"));
            choose(live, 1, "Chương một");
            for route in [
                Route::Discover,
                Route::Library,
                Route::Account,
                Route::Settings,
            ] {
                let html = mini(live, route);
                for expected in [
                    "class=\"theatre-mini-player\"",
                    "href=\"#player\"",
                    "Trình phát thu gọn",
                    "Chương một",
                    "Phát: Chương một",
                    "aria-hidden=\"true\"",
                    "id=\"theatre-mini-transport\"",
                ] {
                    assert!(html.contains(expected), "{route:?} {expected}: {html}");
                }
                assert!(
                    !html.contains("role=\"status\""),
                    "the bar does not announce progress"
                );
            }
            assert!(!mini(live, Route::Player).contains("theatre-mini-player"));
            // The link and the action are separate controls.
            let html = mini(live, Route::Discover);
            assert!(html.find("</a>").unwrap() < html.find("<button").unwrap());
        });
    }

    #[test]
    fn no_live_status_or_control_is_labelled_as_a_simulation() {
        for locale in Locale::supported() {
            for status in [
                Status::Idle,
                Status::Loading,
                Status::Paused(crate::core::playback::PauseCause::User),
                Status::Paused(crate::core::playback::PauseCause::Interruption),
                Status::Paused(crate::core::playback::PauseCause::ColdRestore),
                Status::Playing,
                Status::Buffering,
                Status::Ended,
                Status::Failed(Failure::Network),
            ] {
                let copy = messages::text(messages::listening_status_key(status), locale);
                for banned in [
                    "mô phỏng",
                    "Mô phỏng",
                    "imulat",
                    "Mock",
                    "mock",
                    "demo",
                    "Demo",
                ] {
                    assert!(!copy.contains(banned), "{status:?} {locale:?}: {copy}");
                }
            }
            for key in [
                Key::ListenPlay,
                Key::ListenPause,
                Key::ListenReplay,
                Key::ListenTimeline,
                Key::ListenTimelineHelp,
                Key::ListenBack15,
                Key::ListenForward15,
                Key::ListenSpeed,
            ] {
                let copy = messages::text(key, locale);
                assert!(
                    !copy.to_lowercase().contains("mô phỏng")
                        && !copy.to_lowercase().contains("simulat"),
                    "{key:?}"
                );
            }
        }
    }

    #[test]
    fn replaying_an_ended_unit_offers_replay_and_the_timeline_uses_live_copy() {
        Owner::new().with(|| {
            let live = live(Locale::English);
            choose(live, 1, "Chapter one");
            live.player.dispatch(Event::PlayRequested);
            let id = identity(live);
            live.player.dispatch(Event::EngineStarted { identity: id });
            live.player.dispatch(Event::EngineEnded { identity: id });
            let html = screen(live);
            assert!(
                html.contains("Play again from the start: Chapter one"),
                "{html}"
            );
            assert!(html.contains("Listening position") && html.contains("Finished"));
            assert!(!html.contains("Mock"));
        });
    }

    #[test]
    fn player_sync_panel_prioritizes_qualified_conflict_choices_over_blocked_progress_intents() {
        use crate::application::{
            account::AccountScope,
            account_view::{AccountViewState, SessionCompletion},
            library_sync::audio_identity_from_source,
        };
        use crate::contract::library::{
            Completion, MutationState, ProgressKind, ProgressRecord, Receipt, Resolution,
            WriteOutcome,
        };

        Owner::new().with(|| {
            let live = live(Locale::Vietnamese);
            choose(live, 1, "Chương một");
            let source = testing::source(1);
            let audio = audio_identity_from_source(source).unwrap();

            let mut account = AccountViewState::default();
            let issued = account.begin_refresh().unwrap();
            assert!(matches!(
                account.finish_session(
                    issued,
                    Ok(AccountScope::from_session(
                        "listener-a".into(),
                        uuid::Uuid::from_u128(10).to_string()
                    )
                    .unwrap()),
                    || uuid::Uuid::from_u128(100).to_string()
                ),
                Ok(SessionCompletion::Authenticated(_))
            ));

            // Create a revision conflict on the active publication.
            let op_id = uuid::Uuid::from_u128(501).to_string();
            account.seed_sync(|sync, _| {
                sync.record_progress_intent(
                    audio.clone(),
                    45_000,
                    ProgressKind::ExplicitSeek,
                    true,
                    1_000,
                    || op_id.clone(),
                );
                let _ = sync.apply_receipt(
                    &op_id,
                    Receipt {
                        operation_id: op_id.clone(),
                        outcome: WriteOutcome::RevisionConflict,
                        resolution: Some(Resolution::ListenerChooses),
                        state: MutationState::Progress {
                            current: Some(ProgressRecord {
                                audio: audio.clone(),
                                position_ms: 90_000,
                                completion: Completion::InProgress,
                                kind: ProgressKind::Checkpoint,
                                revision: 4,
                                listener_order: 4,
                                accepted_at_ms: 2_000,
                                written_by: uuid::Uuid::from_u128(999).to_string(),
                            }),
                        },
                    },
                    false,
                    || uuid::Uuid::from_u128(502).to_string(),
                    None,
                );
            });

            let account = RwSignal::new(account);
            let sync_controls = PlayerSyncControls {
                account: account.read_only(),
                on_mark_completed: Callback::new(|_| {}),
                on_restart: Callback::new(|_| {}),
                on_resolve_conflict: Callback::new(|_| {}),
                on_retry_progress: Callback::new(|_| {}),
            };
            let html = view! { <LivePlayerScreen live sync_controls/> }.to_html();
            assert!(!html.contains("id=\"theatre-player-restart\""));
            assert!(!html.contains("id=\"theatre-player-mark-completed\""));
            assert!(html.contains("Giữ vị trí tại đây (0:45)"));
            assert!(html.contains("Theo vị trí trên máy chủ (1:30)"));
            for id in [
                "theatre-player-resolve-here",
                "theatre-player-resolve-other",
            ] {
                let button = html.split(&format!("id=\"{id}\"")).nth(1).unwrap();
                let anatomy = button.split('>').next().unwrap();
                assert!(anatomy.contains("cui-button"), "{id}: {anatomy}");
                assert!(anatomy.contains("data-variant=\"tonal\""), "{anatomy}");
                assert!(!anatomy.contains("theatre-action"), "{anatomy}");
            }
            assert_eq!(
                html.matches("Vị trí nghe khác nhau").count(),
                1,
                "one visible conflict prompt"
            );
            assert!(
                !html.contains("aria-busy"),
                "an unresolved choice is not a request"
            );
            let target = ProgressActionTarget::capture(live, sync_controls).unwrap();
            let publication = target.publication_if_current(live, sync_controls).unwrap();
            account.update(|account| {
                account.seed_sync(|sync, _| {
                    assert!(sync
                        .resolve_progress_conflict(
                            &publication,
                            false,
                            || uuid::Uuid::from_u128(503).to_string(),
                            None
                        )
                        .is_some());
                });
            });
            assert_eq!(
                target.publication_if_current(live, sync_controls),
                None,
                "the old choice cannot resolve a pending command twice"
            );
            let html = sync_html(live, sync_controls);
            assert!(!html.contains("theatre-player-resolve-here"));
            assert_eq!(html.matches("role=\"status\"").count(), 1, "{html}");
        });
    }
}
