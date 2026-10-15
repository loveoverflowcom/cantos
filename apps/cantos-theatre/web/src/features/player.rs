//! App-private semantic HTML compositions, not new shared UI primitives.
use super::{common::*, library::BookmarkRow};
use crate::{
    app::{context::TheatreContext, routes::Route},
    core::{
        listening::library::Bookmark,
        playback::{Event, Intent},
    },
    messages::{self, Key},
    view::Recovery,
};
use leptos::prelude::*;

fn transport_key(ctx: TheatreContext) -> Key {
    match ctx.player.transport.get() {
        Some(transport) if transport.intent == Intent::Play => Key::Pause,
        Some(transport) if transport.replay => Key::Replay,
        _ => Key::Play,
    }
}

pub(crate) fn position_label(ctx: TheatreContext) -> String {
    ctx.player
        .view
        .get()
        .map(|player| {
            format!(
                "{} / {}",
                clock(
                    ctx.player
                        .pending_seek
                        .get()
                        .map(|seek| seek.position_ms / 1000)
                        .unwrap_or(player.position)
                ),
                clock(player.duration)
            )
        })
        .unwrap_or_default()
}

#[component]
pub(crate) fn TransportAction(
    ctx: TheatreContext,
    #[prop(optional)] compact: bool,
) -> impl IntoView {
    view! {
        <button type="button" class="theatre-action theatre-transport" data-compact=compact
            on:click=move |_| ctx.toggle_transport() aria-describedby="theatre-demo-notice"
            aria-label=move || ctx.format(Key::TransportEpisode, &[("action", ctx.text(transport_key(ctx)).to_owned()),
                ("episode", ctx.player.source.get().and_then(|source| ctx.episode(source.episode()).map(|episode| episode.title)).unwrap_or_default().to_owned())])>
            {move || icon(if transport_key(ctx) == Key::Pause { "pause" } else { "play_arrow" })}
            <span class="cui-control-label">{move || ctx.text(transport_key(ctx))}</span>
        </button>
    }
}

#[component]
pub(crate) fn Player(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Player/>
        <Show when=move || ctx.player.source.get().is_some() fallback=move || view! { <Recovery ctx=ctx title=Key::NoPlayer help=Key::NoPlayerHelp/> }>
            <section class="theatre-player">
                <div class="theatre-player-cover">{move || ctx.player.source.get().map(|source| view! { <Cover ctx=ctx id=source.episode().work/> })}</div>
                <div class="theatre-player-body">
                    <p class="theatre-eyebrow">{move || ctx.text(Key::DemoBadge)}</p>
                    <h2 lang="vi-VN">{move || ctx.player.source.get().and_then(|source| ctx.episode(source.episode()).map(|episode| episode.title)).unwrap_or_default()}</h2>
                    <a class="theatre-text-link" lang="vi-VN" href=move || ctx.player.source.get().map(|source| Route::Work(source.episode().work).hash()).unwrap_or_else(|| Route::Discover.hash())>
                        <span class="cui-control-label">{move || ctx.player.source.get().map(|source| ctx.work(source.episode().work).title).unwrap_or_default()}</span>
                    </a>
                    <Show when=move || ctx.player.feedback.get().rejection.is_some()>
                        <p role="status">{move || ctx.text(Key::MockRejected)}</p>
                    </Show>
                    <super::player_timeline::PlayerTimeline ctx=ctx/>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.playback(Event::SkipRequested { delta_ms: -15000 })>{icon("replay")}<span class="cui-control-label">{move || ctx.text(Key::Back15)}</span></button>
                        <TransportAction ctx=ctx/>
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.playback(Event::SkipRequested { delta_ms: 15000 })>{icon("rotate_right")}<span class="cui-control-label">{move || ctx.text(Key::Forward15)}</span></button>
                    </div>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-text" on:click=move |_| ctx.add_bookmark()>
                            {icon("bookmark_border")}<span class="cui-control-label">{move || if ctx.player.state.with(|state| state.session().is_some_and(|player| ctx.library.with(|library| library.bookmarks.contains(&Bookmark { episode: player.source().episode(), position_ms: player.position_ms() })))) { ctx.text(Key::BookmarkAdded) } else { ctx.text(Key::AddBookmark) }}</span>
                        </button>
                        <a class="theatre-text-link" href="#queue">{icon("queue_music")}<span class="cui-control-label">{move || ctx.text(Key::Queue)}</span></a>
                        <a class="theatre-text-link" href="#settings">{icon("settings")}<span class="cui-control-label">{move || ctx.text(Key::Settings)}</span></a>
                    </div>
                    <dl class="theatre-player-facts"><div><dt>{move || ctx.text(Key::Speed)}</dt><dd>{move || messages::speed_label(ctx.locale.get(), ctx.player.view.get().map(|player| player.speed).unwrap_or(100))}</dd></div></dl>
                    <details class="theatre-demo-tools">
                        <summary>{move || ctx.text(Key::MockEngineTools)}</summary>
                        <p id="theatre-engine-help" class="theatre-muted">{move || ctx.text(Key::MockEngineHelp)}</p>
                        <div class="theatre-actions">
                            <button type="button" class="theatre-action theatre-action-tonal"
                                aria-describedby="theatre-engine-help"
                                disabled=move || ctx.player.transport.get().is_none_or(|player| player.intent != Intent::Play)
                                on:click=move |_| ctx.mock_buffer()><span class="cui-control-label">{move || ctx.text(Key::MockBuffer)}</span></button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                aria-describedby="theatre-engine-help"
                                disabled=move || ctx.player.transport.get().is_none_or(|player| player.intent != Intent::Play)
                                on:click=move |_| ctx.mock_started()><span class="cui-control-label">{move || ctx.text(Key::MockReady)}</span></button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.playback(Event::InterruptionBegan)><span class="cui-control-label">{move || ctx.text(Key::MockInterrupt)}</span></button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.playback(Event::InterruptionEnded { resume_allowed: false })><span class="cui-control-label">{move || ctx.text(Key::MockEndInterrupt)}</span></button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.mock_ended()><span class="cui-control-label">{move || ctx.text(Key::MockEnd)}</span></button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.restore_position()><span class="cui-control-label">{move || ctx.text(Key::MockRestore)}</span></button>
                        </div>
                    </details>
                    <UnsupportedAction ctx=ctx label=Key::Download/>
                    <UnsupportedAction ctx=ctx label=Key::Timer/>
                    <Show when=move || !ctx.library.get().bookmarks.is_empty()>
                        <details class="theatre-demo-tools">
                            <summary>{move || ctx.text(Key::Bookmarks)}</summary>
                            <ul class="theatre-bookmarks">
                                <For each=move || ctx.library.get().bookmarks key=|mark| (mark.episode, mark.position_ms)
                                    children=move |mark| view! { <BookmarkRow ctx=ctx mark=mark/> }/>
                            </ul>
                        </details>
                    </Show>
                </div>
            </section>
        </Show>
    }
}

#[component]
pub(crate) fn MiniPlayer(ctx: TheatreContext) -> impl IntoView {
    view! {
        <Show when=move || ctx.player.source.get().is_some() && !matches!(ctx.route.get(), Route::Player | Route::Queue)>
            <MiniPlayerBar ctx=ctx/>
        </Show>
    }
}

#[component]
fn MiniPlayerBar(ctx: TheatreContext) -> impl IntoView {
    let bar = NodeRef::<leptos::html::Aside>::new();
    crate::platform::bind_mini_player_clearance(bar);
    view! {
            <aside class="theatre-mini-player" node_ref=bar aria-label=move || ctx.text(Key::MiniPlayer)>
                <a class="theatre-mini-title" href="#player">
                    <div class="theatre-mini-cover" aria-hidden="true">{move || ctx.player.source.get().map(|source| view! { <Cover ctx=ctx id=source.episode().work/> })}</div>
                    <span><strong lang="vi-VN">{move || ctx.player.source.get().and_then(|source| ctx.episode(source.episode()).map(|episode| episode.title)).unwrap_or_default()}</strong>
                    <small lang="vi-VN">{move || ctx.player.source.get().map(|source| ctx.work(source.episode().work).title).unwrap_or_default()}</small>
                    <small>{move || ctx.text(if ctx.player.pending_seek.get().is_some() { Key::SeekPending } else { messages::status_key(ctx.player.status.get()) })}</small></span>
                </a>
                <span class="theatre-time">{move || position_label(ctx)}</span>
                <TransportAction ctx=ctx compact=true/>
                <progress class="theatre-mini-progress" aria-hidden="true"
                    max=move || ctx.player.source.get().map(|source| source.duration_ms()).unwrap_or(1).to_string()
                    value=move || ctx.player.pending_seek.get().map(|seek| seek.position_ms).unwrap_or_else(|| ctx.player.position_ms.get().unwrap_or(0)).to_string()/>
            </aside>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{player_session::PlayerSession, ports::MediaPort},
        core::{
            ids::{EpisodeId, WorkId},
            playback::{Effect, ResumeSnapshot, Source, Status},
        },
    };
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    struct DeferredMedia;
    impl MediaPort for DeferredMedia {
        fn execute(&mut self, _: &Effect) -> Vec<Event> {
            Vec::new()
        }
    }

    #[test]
    fn transport_consumer_does_not_rerun_for_an_hour_of_position_ticks() {
        Owner::new().with(|| {
            let mut ctx = TheatreContext::new(NodeRef::new());
            ctx.player = PlayerSession::new(DeferredMedia);
            let source = Source::new(
                EpisodeId {
                    work: WorkId::River,
                    index: 0,
                },
                7_200_000,
            )
            .unwrap();
            ctx.player.restore(
                ResumeSnapshot {
                    source,
                    position_ms: 0,
                    speed_permille: 1000,
                }
                .try_into()
                .unwrap(),
            );
            let broad_reads = Arc::new(AtomicUsize::new(0));
            let narrow_reads = Arc::new(AtomicUsize::new(0));
            let broad = Memo::new({
                let reads = broad_reads.clone();
                move |_| {
                    reads.fetch_add(1, Ordering::Relaxed);
                    match ctx.player.view.get() {
                        Some(view) if view.intent == Intent::Play => Key::Pause,
                        Some(view)
                            if view.transport == Status::Ended
                                && view.position == view.duration =>
                        {
                            Key::Replay
                        }
                        _ => Key::Play,
                    }
                }
            });
            let narrow = Memo::new({
                let reads = narrow_reads.clone();
                move |_| {
                    reads.fetch_add(1, Ordering::Relaxed);
                    transport_key(ctx)
                }
            });
            assert_eq!(broad.get(), narrow.get());
            let identity = ctx.player.state.get_untracked().identity().unwrap();
            for tick in 1..=3600 {
                ctx.player.dispatch(Event::EnginePosition {
                    identity,
                    position_ms: tick * 1000,
                });
                assert_eq!(broad.get(), narrow.get());
            }
            assert_eq!(broad_reads.load(Ordering::Relaxed), 3601);
            assert_eq!(narrow_reads.load(Ordering::Relaxed), 1);
            for _ in 0..1000 {
                ctx.toggle_saved(WorkId::Mountain);
                assert_eq!(narrow.get(), Key::Play);
            }
            assert_eq!(narrow_reads.load(Ordering::Relaxed), 1);
            println!(
                "transport consumer / 3600 simulated ticks: broad={} narrow={}",
                broad_reads.load(Ordering::Relaxed),
                narrow_reads.load(Ordering::Relaxed)
            );
            ctx.playback(Event::PlayRequested);
            assert_eq!(narrow.get(), Key::Pause);
            assert_eq!(ctx.player.status.get(), Status::Loading);
            assert_eq!(narrow_reads.load(Ordering::Relaxed), 2);
            let identity = ctx.player.state.get_untracked().identity().unwrap();
            ctx.player.dispatch(Event::EngineStarted { identity });
            assert_eq!(narrow.get(), Key::Pause);
            ctx.playback(Event::PauseRequested);
            assert_eq!(narrow.get(), Key::Play);
            let identity = ctx.player.state.get_untracked().identity().unwrap();
            ctx.player.dispatch(Event::EngineEnded { identity });
            assert_eq!(narrow.get(), Key::Replay);
            ctx.playback(Event::SeekRequested { position_ms: 0 });
            assert_eq!(narrow.get(), Key::Play);
            ctx.open_episode(EpisodeId {
                work: WorkId::Mountain,
                index: 0,
            });
            assert_eq!(narrow.get(), broad.get());
        });
    }

    #[test]
    fn mini_player_has_separate_link_action_artwork_and_nonannouncing_progress() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.open_episode(EpisodeId {
                work: WorkId::Dream,
                index: 0,
            });
            ctx.route.set(Route::Library);
            let before = ctx.player.state.get_untracked();
            let html = view! { <MiniPlayer ctx=ctx/> }.to_html();
            assert!(html.contains("class=\"theatre-mini-cover\""));
            let progress = html
                .split("<progress")
                .nth(1)
                .unwrap()
                .split('>')
                .next()
                .unwrap();
            assert!(progress.contains("class=\"theatre-mini-progress\""));
            assert!(progress.contains("aria-hidden=\"true\""));
            assert!(html.contains("href=\"#player\""));
            let link_end = html.find("</a>").unwrap();
            let button_start = html.find("<button").unwrap();
            assert!(button_start > link_end);
            assert!(html.contains("aria-label=\"Phát mô phỏng:"));
            assert!(!html.contains("role=\"status\""));
            assert_eq!(ctx.player.state.get_untracked(), before);
        });
    }
}
