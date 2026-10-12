//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::{context::TheatreContext, routes::Route},
    core::{
        listening::library::Bookmark,
        playback::{Event, Intent, Status},
    },
    messages::{self, Key},
    view::Recovery,
};
use leptos::prelude::*;

pub(crate) fn position_label(ctx: TheatreContext) -> String {
    ctx.player
        .view
        .get()
        .map(|player| format!("{} / {}", clock(player.position), clock(player.duration)))
        .unwrap_or_default()
}

#[component]
pub(crate) fn TransportAction(
    ctx: TheatreContext,
    #[prop(optional)] compact: bool,
) -> impl IntoView {
    view! {
        <button type="button" class="theatre-action theatre-transport" data-compact=compact
            on:click=move |_| ctx.toggle_transport() aria-describedby="theatre-demo-notice">
            {move || icon(if ctx.player.view.get().is_some_and(|player| player.intent == Intent::Play) { "pause" } else { "play_arrow" })}
            <span>{move || ctx.text(if ctx.player.view.get().is_some_and(|player| player.intent == Intent::Play) { Key::Pause }
                else if ctx.player.view.get().is_some_and(|player| player.transport == Status::Ended && player.position == player.duration) { Key::Replay } else { Key::Play })}</span>
        </button>
    }
}

#[component]
pub(crate) fn Player(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::Player/>
        <Show when=move || ctx.player.view.get().is_some() fallback=move || view! { <Recovery ctx=ctx title=Key::NoPlayer help=Key::NoPlayerHelp/> }>
            <section class="theatre-player">
                <div class="theatre-player-cover">{move || ctx.player.view.get().map(|player| view! { <Cover ctx=ctx id=player.episode.work/> })}</div>
                <div class="theatre-player-body">
                    <p class="theatre-eyebrow">{move || ctx.text(Key::DemoBadge)}</p>
                    <h2 lang="vi-VN">{move || ctx.player.view.get().and_then(|player| ctx.episode(player.episode).map(|episode| episode.title)).unwrap_or_default()}</h2>
                    <a class="theatre-text-link" lang="vi-VN" href=move || ctx.player.view.get().map(|player| Route::Work(player.episode.work).hash()).unwrap_or_else(|| Route::Discover.hash())>
                        {move || ctx.player.view.get().map(|player| ctx.work(player.episode.work).title).unwrap_or_default()}
                    </a>
                    <p role="status" aria-atomic="true">{move || ctx.player.view.get().map(|player| ctx.text(messages::status_key(player.transport))).unwrap_or_default()}</p>
                    <Show when=move || ctx.player.feedback.get().rejection.is_some()>
                        <p role="status">{move || ctx.text(Key::MockRejected)}</p>
                    </Show>
                    <label class="theatre-range-field" for="theatre-position">{move || ctx.text(Key::Timeline)}
                        <input id="theatre-position" type="range" min="0" step="1"
                            max=move || ctx.player.view.get().and_then(|player| ctx.episode(player.episode).map(|episode| episode.duration)).unwrap_or(0).to_string()
                            prop:value=move || ctx.player.view.get().map(|player| player.position).unwrap_or(0).to_string()
                            aria-valuetext=move || ctx.player.view.get().map(|player| messages::position_description(ctx.locale.get(), player.position, ctx.episode(player.episode).map(|episode| episode.duration).unwrap_or(0))).unwrap_or_default()
                            aria-describedby="theatre-timeline-help"
                            on:input=move |event| { if let Ok(position) = event_target_value(&event).parse::<u32>() { if let Some(position_ms) = position.checked_mul(1000) { ctx.playback(Event::SeekRequested { position_ms }); } } }/>
                    </label>
                    <p class="theatre-time">{move || position_label(ctx)}</p>
                    <p id="theatre-timeline-help" class="theatre-muted">{move || ctx.text(Key::TimelineHelp)}</p>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.playback(Event::SkipRequested { delta_ms: -15000 })>{icon("replay")}{move || ctx.text(Key::Back15)}</button>
                        <TransportAction ctx=ctx/>
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.playback(Event::SkipRequested { delta_ms: 15000 })>{icon("rotate_right")}{move || ctx.text(Key::Forward15)}</button>
                    </div>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-text" on:click=move |_| ctx.add_bookmark()>
                            {icon("bookmark_border")}{move || if ctx.player.state.with(|state| state.session().is_some_and(|player| ctx.library.with(|library| library.bookmarks.contains(&Bookmark { episode: player.source().episode(), position_ms: player.position_ms() })))) { ctx.text(Key::BookmarkAdded) } else { ctx.text(Key::AddBookmark) }}
                        </button>
                        <a class="theatre-text-link" href="#queue">{icon("queue_music")}{move || ctx.text(Key::Queue)}</a>
                        <a class="theatre-text-link" href="#settings">{icon("settings")}{move || ctx.text(Key::Settings)}</a>
                    </div>
                    <dl class="theatre-player-facts"><div><dt>{move || ctx.text(Key::Speed)}</dt><dd>{move || messages::speed_label(ctx.locale.get(), ctx.player.view.get().map(|player| player.speed).unwrap_or(100))}</dd></div></dl>
                    <details class="theatre-demo-tools">
                        <summary>{move || ctx.text(Key::MockEngineTools)}</summary>
                        <p id="theatre-engine-help" class="theatre-muted">{move || ctx.text(Key::MockEngineHelp)}</p>
                        <div class="theatre-actions">
                            <button type="button" class="theatre-action theatre-action-tonal"
                                aria-describedby="theatre-engine-help"
                                disabled=move || ctx.player.view.get().is_none_or(|player| player.intent != Intent::Play)
                                on:click=move |_| ctx.mock_buffer()>{move || ctx.text(Key::MockBuffer)}</button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                aria-describedby="theatre-engine-help"
                                disabled=move || ctx.player.view.get().is_none_or(|player| player.intent != Intent::Play)
                                on:click=move |_| ctx.mock_started()>{move || ctx.text(Key::MockReady)}</button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.playback(Event::InterruptionBegan)>{move || ctx.text(Key::MockInterrupt)}</button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.playback(Event::InterruptionEnded { resume_allowed: false })>{move || ctx.text(Key::MockEndInterrupt)}</button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.mock_ended()>{move || ctx.text(Key::MockEnd)}</button>
                            <button type="button" class="theatre-action theatre-action-tonal"
                                on:click=move |_| ctx.restore_position()>{move || ctx.text(Key::MockRestore)}</button>
                        </div>
                    </details>
                    <UnsupportedAction ctx=ctx label=Key::Download/>
                    <UnsupportedAction ctx=ctx label=Key::Timer/>
                </div>
            </section>
        </Show>
    }
}

#[component]
pub(crate) fn MiniPlayer(ctx: TheatreContext) -> impl IntoView {
    view! {
        <Show when=move || ctx.player.view.get().is_some() && !matches!(ctx.route.get(), Route::Player | Route::Queue)>
            <aside class="theatre-mini-player" aria-label=move || ctx.text(Key::MiniPlayer)>
                <a class="theatre-mini-title" href="#player">
                    {icon("headphones")}<span><strong lang="vi-VN">{move || ctx.player.view.get().and_then(|player| ctx.episode(player.episode).map(|episode| episode.title)).unwrap_or_default()}</strong>
                    <small>{move || ctx.player.view.get().map(|player| ctx.text(messages::status_key(player.transport))).unwrap_or_default()}</small></span>
                </a>
                <span class="theatre-time">{move || position_label(ctx)}</span>
                <TransportAction ctx=ctx compact=true/>
            </aside>
        </Show>
    }
}
