//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::context::TheatreContext,
    application::catalog::Scenario,
    core::ids::{EpisodeId, WorkId},
    messages::{self, Key},
};
use leptos::prelude::*;

#[component]
pub(crate) fn WorkDetail(ctx: TheatreContext, id: WorkId) -> impl IntoView {
    view! {
        <a class="theatre-back" href="#discover">{icon("arrow_back")}<span class="cui-control-label">{move || ctx.text(Key::BackDiscover)}</span></a>
        <Show when=move || ctx.catalog.scenario() == Scenario::Ready fallback=move || view! { <CatalogState ctx=ctx/> }>
            <section class="theatre-work-intro">
                <Cover ctx=ctx id=id/>
                <div>
                    <p class="theatre-eyebrow">{move || ctx.text(messages::genre_key(ctx.work(id).genre))}</p>
                    <h1 lang="vi-VN">{ctx.work(id).title}</h1>
                    <p class="theatre-synopsis" lang="vi-VN">{ctx.work(id).synopsis}</p>
                    <p class="theatre-muted">{move || ctx.text(Key::ContentLanguage)}<span aria-hidden="true">" · "</span>
                        {move || messages::episode_count(ctx.locale.get(), ctx.work(id).episodes.len())}</p>
                    <SaveAction ctx=ctx id=id/>
                </div>
            </section>
            <section aria-labelledby="theatre-episodes-heading">
                <h2 id="theatre-episodes-heading" class="theatre-section-heading">{move || ctx.text(Key::Episodes)}</h2>
                <ol class="theatre-episode-list">
                    {(0..ctx.work(id).episodes.len()).map(move |index| view! { <EpisodeRow ctx=ctx id=EpisodeId { work: id, index }/> }).collect_view()}
                </ol>
            </section>
        </Show>
    }
}

#[component]
pub(crate) fn EpisodeRow(ctx: TheatreContext, id: EpisodeId) -> impl IntoView {
    let Some(episode) = ctx.episode(id) else {
        return ().into_any();
    };
    view! {
        <li class="theatre-episode-row" data-current=move || ctx.player.view.get().is_some_and(|player| player.episode == id)>
            <span class="theatre-episode-number">{format!("{:02}", id.index + 1)}</span>
            <div class="theatre-episode-copy">
                <h3 lang="vi-VN">{episode.title}</h3>
                <p class="theatre-muted">{if episode.available { clock(episode.duration) } else { String::new() }}
                    <Show when=move || ctx.player.view.get().is_some_and(|player| player.episode == id)>
                        <span>{move || ctx.text(Key::CurrentEpisode)}</span>
                    </Show>
                </p>
                {(!episode.available).then(|| view! { <p id=format!("unavailable-{}-{}", id.work.slug(), id.index)>{move || ctx.text(Key::UnavailableHelp)}</p> })}
            </div>
            <div class="theatre-episode-actions">
                <button type="button" class="theatre-action theatre-action-tonal" disabled=!episode.available
                    aria-label=move || ctx.format(Key::PlayName, &[("title", episode.title.to_owned())])
                    aria-describedby=(!episode.available).then(|| format!("unavailable-{}-{}", id.work.slug(), id.index))
                    on:click=move |_| ctx.open_episode(id)>
                    {icon("play_arrow")}<span class="cui-control-label">{move || ctx.text(if episode.available { Key::Begin } else { Key::Unavailable })}</span>
                </button>
                {episode.available.then(|| view! {
                    <div class="theatre-queue-action">
                        <button type="button" class="theatre-action theatre-action-text"
                            disabled=move || (ctx.queue.with(|state| state.items.contains(&id)) || ctx.player.view.get().is_some_and(|player| player.episode == id))
                            aria-label=move || ctx.format(Key::QueueName, &[("title", episode.title.to_owned())])
                            aria-describedby=format!("queue-reason-{}-{}", id.work.slug(), id.index)
                            on:click=move |_| ctx.enqueue(id)>
                            {icon("add")}<span class="cui-control-label">{move || ctx.text(Key::QueueAdd)}</span>
                        </button>
                        <p id=format!("queue-reason-{}-{}", id.work.slug(), id.index) class="theatre-muted"
                            hidden=move || (!ctx.queue.with(|state| state.items.contains(&id)) && ctx.player.view.get().is_none_or(|player| player.episode != id))>
                            {move || if ctx.player.view.get().is_some_and(|player| player.episode == id) { ctx.text(Key::CurrentEpisode) } else { ctx.text(Key::QueueAdded) }}
                        </p>
                    </div>
                })}
            </div>
        </li>
    }.into_any()
}
