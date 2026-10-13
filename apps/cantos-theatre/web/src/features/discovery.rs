//! App-private semantic HTML compositions, not new shared UI primitives.
use super::{common::*, explore::Explore, library::ContinueCard};
use crate::{
    app::{context::TheatreContext, routes::Route},
    application::catalog::Scenario,
    core::ids::{EpisodeId, WorkId},
    messages::Key,
};
use leptos::prelude::*;

#[component]
pub(crate) fn Discover(ctx: TheatreContext) -> impl IntoView {
    view! {
        <PageHeading ctx=ctx title=Key::HomeHeading deck=Key::HomeDeck/>
        <Show when=move || ctx.catalog.scenario() == Scenario::Ready fallback=move || view! { <CatalogState ctx=ctx/> }>
            <section class="theatre-feature">
                <div class="theatre-feature-copy">
                    <p class="theatre-eyebrow">{move || ctx.text(Key::Featured)}</p>
                    <h2 lang="vi-VN">{ctx.work(WorkId::Mountain).title}</h2>
                    <p lang="vi-VN">{ctx.work(WorkId::Mountain).synopsis}</p>
                    <p class="theatre-muted">{move || ctx.text(Key::ContentLanguage)}</p>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action" on:click=move |_| ctx.open_episode(EpisodeId { work: WorkId::Mountain, index: 0 })>
                            {icon("play_arrow")}<span class="cui-control-label">{move || ctx.text(Key::Begin)}</span>
                        </button>
                        <a class="theatre-text-link" href=Route::Work(WorkId::Mountain).hash()><span class="cui-control-label">{move || ctx.text(Key::AboutWork)}</span></a>
                    </div>
                </div>
                <div class="theatre-feature-cover"><Cover ctx=ctx id=WorkId::Mountain/></div>
            </section>
            <ContinueCard ctx=ctx/>
            <Explore ctx=ctx embedded=true/>
        </Show>
    }
}
