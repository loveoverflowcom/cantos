//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::context::TheatreContext,
    application::catalog::Scenario,
    core::catalog::Genre,
    messages::{self, Key},
};
use leptos::prelude::*;

#[component]
pub(crate) fn Explore(ctx: TheatreContext) -> impl IntoView {
    let query = RwSignal::new(String::new());
    let genre = RwSignal::new(Genre::All);
    let matches = Memo::new(move |_| {
        let query = query.get().trim().to_lowercase();
        let genre = genre.get();
        ctx.catalog
            .state
            .get()
            .ids()
            .into_iter()
            .filter(|id| {
                let item = ctx.work(*id);
                (genre == Genre::All || genre == item.genre)
                    && (query.is_empty()
                        || format!("{} {}", item.title, item.synopsis)
                            .to_lowercase()
                            .contains(&query))
            })
            .collect::<Vec<_>>()
    });
    view! {
        <PageHeading ctx=ctx title=Key::Explore/>
        <div class="theatre-search-field">
            <label for="theatre-search">{move || ctx.text(Key::SearchLabel)}</label>
            <input id="theatre-search" type="search" class="theatre-input" aria-describedby="theatre-search-help"
                prop:value=move || query.get()
                on:input=move |event| query.set(event_target_value(&event))/>
            <p id="theatre-search-help" class="theatre-muted">{move || ctx.text(Key::SearchHelp)}</p>
        </div>
        <div class="theatre-filters" role="group" aria-label=move || ctx.text(Key::Filters)>
            {[Genre::All, Genre::Fantasy, Genre::Memory, Genre::Dream].into_iter().map(move |choice| view! {
                <button type="button" class="theatre-chip" aria-pressed=move || (genre.get() == choice).to_string()
                    on:click=move |_| genre.set(choice)>{move || ctx.text(messages::genre_key(choice))}</button>
            }).collect_view()}
        </div>
        <Show when=move || ctx.catalog.scenario() == Scenario::Ready fallback=move || view! { <CatalogState ctx=ctx/> }>
            <Show when=move || !matches.get().is_empty() fallback=move || view! {
                <section class="theatre-state">
                    <h2>{move || ctx.text(Key::NoResults)}</h2><p>{move || ctx.text(Key::NoResultsHelp)}</p>
                    <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| {
                        query.set(String::new()); genre.set(Genre::All);
                    }>{move || ctx.text(Key::ClearSearch)}</button>
                </section>
            }>
                <div class="theatre-shelf">
                    <For each=move || matches.get() key=|id| *id children=move |id| view! { <WorkCard ctx=ctx id=id/> }/>
                </div>
            </Show>
        </Show>
    }
}
