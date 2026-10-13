//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::context::TheatreContext, core::listening::library::Bookmark, messages::Key, platform,
    view::Recovery,
};
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum LibraryTab {
    #[default]
    Continue,
    Saved,
    Bookmarks,
}

#[component]
pub(crate) fn ContinueCard(ctx: TheatreContext) -> impl IntoView {
    let latest = Memo::new(move |_| ctx.progress.get().latest());
    view! {
        <Show when=move || latest.get().is_some()>
            <section class="theatre-continue">
                <div><p class="theatre-eyebrow">{move || ctx.text(Key::ContinueHeading)}</p>
                    <h2 lang="vi-VN">{move || latest.get().and_then(|progress| ctx.episode(progress.snapshot.source.episode()).map(|episode| episode.title)).unwrap_or_default()}</h2>
                    <p class="theatre-muted" lang="vi-VN">{move || latest.get().map(|progress| ctx.work(progress.snapshot.source.episode().work).title).unwrap_or_default()}</p>
                </div>
                <span class="theatre-time">{move || latest.get().map(|progress| clock(progress.snapshot.position_ms / 1000)).unwrap_or_default()}</span>
                <button type="button" class="theatre-action" on:click=move |_| ctx.continue_progress()><span class="cui-control-label">{move || ctx.text(Key::Continue)}</span></button>
            </section>
        </Show>
    }
}

#[component]
pub(crate) fn Library(ctx: TheatreContext) -> impl IntoView {
    let tab = RwSignal::new(LibraryTab::Continue);
    let saved_works = move || {
        ctx.library
            .get()
            .saved
            .into_iter()
            .map(|saved| saved.work)
            .collect::<Vec<_>>()
    };
    view! {
        <PageHeading ctx=ctx title=Key::Library/>
        <div class="theatre-filters" role="group" aria-label=move || ctx.text(Key::Library)>
            {[(LibraryTab::Continue, Key::Continue), (LibraryTab::Saved, Key::Saved), (LibraryTab::Bookmarks, Key::Bookmarks)].into_iter().map(move |(choice, label)| view! {
                <button type="button" class="theatre-chip" aria-pressed=move || (tab.get() == choice).to_string()
                    on:click=move |_| tab.set(choice)><span class="cui-control-label">{move || ctx.text(label)}</span></button>
            }).collect_view()}
        </div>
        {move || match tab.get() {
            LibraryTab::Continue => view! {
                <Show when=move || ctx.progress.get().latest().is_some() fallback=move || view! { <Recovery ctx=ctx title=Key::NoLibrary help=Key::NoLibraryHelp/> }>
                    <ContinueCard ctx=ctx/>
                </Show>
            }.into_any(),
            LibraryTab::Saved => view! {
                <Show when=move || !ctx.library.get().saved.is_empty() fallback=move || view! { <Recovery ctx=ctx title=Key::NoLibrary help=Key::NoLibraryHelp/> }>
                    <div class="theatre-shelf"><For each=saved_works key=|id| *id children=move |id| view! { <WorkCard ctx=ctx id=id/> }/></div>
                </Show>
            }.into_any(),
            LibraryTab::Bookmarks => view! {
                <Show when=move || !ctx.library.get().bookmarks.is_empty() fallback=move || view! { <Recovery ctx=ctx title=Key::NoLibrary help=Key::NoLibraryHelp/> }>
                    <ul class="theatre-bookmarks"><For each=move || ctx.library.get().bookmarks key=|mark| (mark.episode, mark.position_ms) children=move |mark| view! { <BookmarkRow ctx=ctx mark=mark/> }/></ul>
                </Show>
            }.into_any(),
        }}
    }
}

#[component]
pub(crate) fn BookmarkRow(ctx: TheatreContext, mark: Bookmark) -> impl IntoView {
    let title = ctx
        .episode(mark.episode)
        .map(|episode| episode.title)
        .unwrap_or_default();
    view! {
        <li class="theatre-bookmark-row">
            <div><h3 lang="vi-VN">{title}</h3><p>{clock(mark.position_ms / 1000)}</p></div>
            <button type="button" class="theatre-action theatre-action-tonal"
                aria-label=move || ctx.format(Key::BookmarkName, &[("title", title.to_owned()), ("time", clock(mark.position_ms / 1000))])
                on:click=move |_| ctx.resume_bookmark(mark)>
                {icon("play_arrow")}<span class="cui-control-label">{move || ctx.text(Key::Continue)}</span>
            </button>
            <button type="button" class="theatre-action theatre-action-text"
                aria-label=move || ctx.format(Key::RemoveBookmarkName, &[("title", title.to_owned()), ("time", clock(mark.position_ms / 1000))])
                on:click=move |_| { ctx.remove_bookmark(mark); platform::focus_main(ctx.main); }>{icon("delete_outline")}<span class="cui-control-label">{move || ctx.text(Key::RemoveBookmark)}</span></button>
        </li>
    }
}
