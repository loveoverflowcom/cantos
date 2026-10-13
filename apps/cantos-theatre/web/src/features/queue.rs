//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{app::context::TheatreContext, messages::Key, platform, view::Recovery};
use leptos::prelude::*;

#[component]
pub(crate) fn Queue(ctx: TheatreContext) -> impl IntoView {
    view! {
        <a class="theatre-back" href="#player">{icon("arrow_back")}<span class="cui-control-label">{move || ctx.text(Key::CloseQueue)}</span></a>
        <PageHeading ctx=ctx title=Key::Queue/>
        <p class="theatre-muted">{move || ctx.text(Key::QueueEmptyHelp)}</p>
        <button type="button" class="theatre-action" disabled=move || ctx.queue.get().items.is_empty()
            aria-describedby="theatre-next-reason" on:click=move |_| ctx.next()>
            {icon("play_arrow")}<span class="cui-control-label">{move || ctx.text(Key::Next)}</span>
        </button>
        <p id="theatre-next-reason" hidden=move || !ctx.queue.get().items.is_empty()>{move || ctx.text(Key::NoNext)}</p>
        <Show when=move || !ctx.queue.get().items.is_empty() fallback=move || view! { <Recovery ctx=ctx title=Key::QueueEmpty help=Key::QueueEmptyHelp/> }>
            <ol class="theatre-episode-list"><For each=move || ctx.queue.get().items key=|id| *id children=move |id| {
                let title = ctx.episode(id).map(|episode| episode.title).unwrap_or_default();
                view! {
                    <li class="theatre-bookmark-row"><div><h2 lang="vi-VN">{title}</h2><p lang="vi-VN">{ctx.work(id.work).title}</p></div>
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| ctx.open_episode(id)
                            aria-label=move || ctx.format(Key::PlayName, &[("title", title.to_owned())])>{icon("play_arrow")}<span class="cui-control-label">{move || ctx.text(Key::Begin)}</span></button>
                        <button type="button" class="theatre-action theatre-action-text" on:click=move |_| { ctx.remove_queue(id); platform::focus_main(ctx.main); }
                            aria-label=move || ctx.format(Key::QueueRemoveName, &[("title", title.to_owned())])>{icon("delete_outline")}<span class="cui-control-label">{move || ctx.text(Key::QueueRemove)}</span></button>
                    </li>
                }
            }/></ol>
        </Show>
    }
}
