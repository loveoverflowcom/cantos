//! The live full player and mini-player. Both read one root-owned `PlayerSession` and send the
//! same intents; neither owns audio, a timer or a request. Text comes from the display facts
//! captured when the listener chose the unit, and only while they describe the session's track.
use super::{
    common::{clock, icon},
    player_timeline::{PlayerPanel, PlayerTimeline, TimelineCopy},
};
use crate::{
    app::routes::Route,
    application::{player_session::PlayerSession, public_listen::NowPlaying},
    core::playback::{Event, Intent, Status},
    messages::{self, Key, Locale},
};
use leptos::prelude::*;

const ICON_PLAY: &str = "play_arrow";
const ICON_PAUSE: &str = "pause";

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
                format!("{} / {}", clock(shown), clock(view.duration))
            })
            .unwrap_or_default()
    }

    fn work_hash(self) -> String {
        self.facts()
            .map(|facts| Route::PublicWork(facts.track().work()).hash())
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
        messages::format(
            Key::ListenTransportName,
            live.locale.get(),
            &[
                ("action", live.text(live.transport_key()).to_owned()),
                ("title", title),
            ],
        )
    };
    let asset = move || {
        if live.transport_key() == Key::ListenPause {
            ICON_PAUSE
        } else {
            ICON_PLAY
        }
    };
    view! {
        <button type="button" class="theatre-action theatre-transport" data-compact=compact
            id=move_id aria-label=name
            on:click=move |_| live.press_transport()>
            {move || icon(asset())}
            <span class="cui-control-label">{move || live.text(live.transport_key())}</span>
        </button>
    }
}

#[component]
fn Skip(live: LivePlayer, delta_ms: i32) -> impl IntoView {
    let (asset, label) = if delta_ms < 0 {
        ("replay", Key::ListenBack15)
    } else {
        ("rotate_right", Key::ListenForward15)
    };
    view! {
        <button type="button" class="theatre-action theatre-action-tonal"
            on:click=move |_| live.player.dispatch(Event::SkipRequested { delta_ms })>
            {icon(asset)}<span class="cui-control-label">{move || live.text(label)}</span>
        </button>
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
pub(crate) fn LivePlayerScreen(live: LivePlayer) -> impl IntoView {
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
                    </div>
                </div>
            </Show>
        </section>
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
        for locale in Locale::ALL {
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
}
