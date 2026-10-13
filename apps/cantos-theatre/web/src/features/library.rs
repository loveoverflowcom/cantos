//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::{context::TheatreContext, routes::Route},
    core::{ids::EpisodeId, listening::library::Bookmark},
    messages::Key,
    platform,
};
use leptos::prelude::*;

#[component]
pub(crate) fn ContinueCard(ctx: TheatreContext) -> impl IntoView {
    let latest = Memo::new(move |_| ctx.progress.get().latest());
    view! {
        <Show when=move || latest.get().is_some()>
            <section class="theatre-continue" aria-labelledby="theatre-discover-continue">
                <div><p id="theatre-discover-continue" class="theatre-eyebrow">{move || ctx.text(Key::ContinueHeading)}</p>
                    <h2 lang="vi-VN">{move || latest.get().and_then(|progress| ctx.episode(progress.snapshot.source().episode()).map(|episode| episode.title)).unwrap_or_default()}</h2>
                    <p class="theatre-muted" lang="vi-VN">{move || latest.get().map(|progress| ctx.work(progress.snapshot.source().episode().work).title).unwrap_or_default()}</p>
                    <p class="theatre-muted">{move || ctx.text(Key::ResumeSampleHelp)}</p>
                </div>
                <span class="theatre-time">{move || latest.get().map(|progress| clock(progress.snapshot.position_ms() / 1000)).unwrap_or_default()}</span>
                <button type="button" class="theatre-action"
                    aria-label=move || latest.get().map(|progress| ctx.format(Key::ResumeSampleName, &[
                        ("title", ctx.episode(progress.snapshot.source().episode()).map(|episode| episode.title).unwrap_or_default().to_owned()),
                        ("time", clock(progress.snapshot.position_ms() / 1000)),
                    ])).unwrap_or_default()
                    on:click=move |_| ctx.continue_progress()><span class="cui-control-label">{move || ctx.text(Key::ResumeSample)}</span></button>
            </section>
        </Show>
    }
}

#[component]
fn SectionEmpty(ctx: TheatreContext, title: Key, help: Key) -> impl IntoView {
    view! {
        <div class="theatre-section-empty">
            <h3>{move || ctx.text(title)}</h3>
            <p class="theatre-muted">{move || ctx.text(help)}</p>
            <a class="theatre-text-link" href=Route::Discover.hash()>{move || ctx.text(Key::BrowseStories)}</a>
        </div>
    }
}

#[component]
pub(crate) fn Library(ctx: TheatreContext) -> impl IntoView {
    let recent_progress = move || {
        ctx.progress
            .get()
            .recent()
            .into_iter()
            .map(|progress| progress.snapshot.source().episode())
            .collect::<Vec<_>>()
    };
    let saved_works = move || {
        ctx.library
            .get()
            .saved
            .into_iter()
            .map(|saved| saved.work)
            .collect::<Vec<_>>()
    };
    let recent_history = move || {
        ctx.history
            .get()
            .records()
            .iter()
            .take(3)
            .map(|entry| entry.episode)
            .collect::<Vec<_>>()
    };
    view! {
        <PageHeading ctx=ctx title=Key::Library deck=Key::LibraryDeck/>
        <p class="theatre-library-notice">{move || ctx.text(Key::LibrarySession)}</p>
        <section class="theatre-library-section" aria-labelledby="theatre-continue-heading">
            <h2 id="theatre-continue-heading" class="theatre-section-heading">{move || ctx.text(Key::Continue)}</h2>
            <Show when=move || !ctx.progress.get().recent().is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoContinue help=Key::NoContinueHelp/> }>
                <p class="theatre-muted">{move || ctx.text(Key::ResumeSampleHelp)}</p>
                <ul class="theatre-history-list"><For each=recent_progress key=|id| *id children=move |id| view! { <ListeningRow ctx=ctx id=id/> }/></ul>
            </Show>
        </section>
        <section class="theatre-library-section" aria-labelledby="theatre-saved-heading">
            <h2 id="theatre-saved-heading" class="theatre-section-heading">{move || ctx.text(Key::Saved)}</h2>
            <Show when=move || !ctx.library.get().saved.is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoSaved help=Key::NoSavedHelp/> }>
                <div class="theatre-shelf"><For each=saved_works key=|id| *id children=move |id| view! { <WorkCard ctx=ctx id=id/> }/></div>
            </Show>
        </section>
        <section class="theatre-library-section" aria-labelledby="theatre-history-heading">
            <div class="theatre-section-header">
                <h2 id="theatre-history-heading" class="theatre-section-heading">{move || ctx.text(Key::History)}</h2>
                <a class="theatre-text-link" href=Route::History.hash() aria-label=move || ctx.text(Key::ViewAllHistory)>{move || ctx.text(Key::ViewAll)}</a>
            </div>
            <Show when=move || !ctx.history.get().records().is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoHistory help=Key::NoHistoryHelp/> }>
                <ul class="theatre-history-list"><For each=recent_history key=|id| *id children=move |id| view! { <ListeningRow ctx=ctx id=id from_history=true/> }/></ul>
            </Show>
        </section>
    }
}

#[component]
pub(crate) fn History(ctx: TheatreContext) -> impl IntoView {
    let confirming = RwSignal::new(false);
    let clear_button = NodeRef::<leptos::html::Button>::new();
    let history = move || {
        ctx.history
            .get()
            .records()
            .iter()
            .map(|entry| entry.episode)
            .collect::<Vec<_>>()
    };
    view! {
        <a class="theatre-back" href=Route::Library.hash()>{icon("arrow_back")}<span class="cui-control-label">{move || ctx.text(Key::BackLibrary)}</span></a>
        <PageHeading ctx=ctx title=Key::History deck=Key::HistoryHelp/>
        <p class="theatre-library-notice">{move || ctx.text(Key::LibrarySession)}</p>
        <Show when=move || !ctx.history.get().records().is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoHistory help=Key::NoHistoryHelp/> }>
            <button type="button" class="theatre-action theatre-action-text" node_ref=clear_button
                aria-expanded=move || confirming.get().to_string() aria-controls="theatre-history-confirm"
                on:click=move |_| confirming.set(!confirming.get_untracked())>
                {icon("delete_outline")}<span class="cui-control-label">{move || ctx.text(Key::ClearHistory)}</span>
            </button>
            <Show when=move || confirming.get()>
                <section id="theatre-history-confirm" class="theatre-history-confirm"
                    aria-labelledby="theatre-history-confirm-title" aria-describedby="theatre-history-confirm-help">
                    <h2 id="theatre-history-confirm-title">{move || ctx.text(Key::ClearHistoryConfirmTitle)}</h2>
                    <p id="theatre-history-confirm-help">{move || ctx.text(Key::ClearHistoryHelp)}</p>
                    <div class="theatre-actions">
                        <button type="button" class="theatre-action theatre-action-tonal" on:click=move |_| {
                            confirming.set(false);
                            platform::focus_button(clear_button);
                        }><span class="cui-control-label">{move || ctx.text(Key::Cancel)}</span></button>
                        <button type="button" class="theatre-action" aria-describedby="theatre-history-confirm-help" on:click=move |_| {
                            ctx.clear_history();
                            confirming.set(false);
                            platform::focus_main(ctx.main);
                        }><span class="cui-control-label">{move || ctx.text(Key::ConfirmClearHistory)}</span></button>
                    </div>
                </section>
            </Show>
            <ul class="theatre-history-list"><For each=history key=|id| *id children=move |id| view! { <ListeningRow ctx=ctx id=id from_history=true/> }/></ul>
        </Show>
    }
}

#[component]
fn ListeningRow(
    ctx: TheatreContext,
    id: EpisodeId,
    #[prop(optional)] from_history: bool,
) -> impl IntoView {
    let Some(episode) = ctx.episode(id) else {
        return ().into_any();
    };
    let position = move || {
        ctx.progress
            .get()
            .find(id)
            .map(|progress| progress.snapshot.position_ms() / 1000)
            .unwrap_or_default()
    };
    view! {
        <li class="theatre-listening-row">
            <div>
                <h3 lang="vi-VN"><a href=Route::Work(id.work).hash()>{ctx.work(id.work).title}</a></h3>
                <p lang="vi-VN">{episode.title}</p>
                <p class="theatre-time">{move || clock(position())}<span aria-hidden="true">" / "</span>{clock(episode.duration)}</p>
            </div>
            <button type="button" class="theatre-action theatre-action-tonal"
                aria-label=move || ctx.format(Key::ResumeSampleName, &[("title", episode.title.to_owned()), ("time", clock(position()))])
                on:click=move |_| { if from_history { ctx.resume_episode(id); } else { ctx.resume_progress(id); } }>
                {icon("replay")}<span class="cui-control-label">{move || ctx.text(Key::ResumeSample)}</span>
            </button>
        </li>
    }.into_any()
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        core::{ids::WorkId, playback::Event},
        features::{common::SaveAction, discovery::Discover},
    };

    #[test]
    fn library_renders_three_sections_together_without_subtabs() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let html = view! { <Library ctx=ctx/> }.to_html();
            for heading in [
                "theatre-continue-heading",
                "theatre-saved-heading",
                "theatre-history-heading",
            ] {
                assert!(html.contains(heading), "{heading}");
            }
            assert!(html.contains("Của tôi"));
            assert!(html.contains("Chưa có lịch sử nghe"));
            assert!(html.contains("href=\"#history\""));
            assert!(!html.contains("role=\"tab"));
            assert!(!html.contains("aria-pressed"));
            assert!(!html.contains("Dấu trang"));
        });
    }

    #[test]
    fn empty_library_sections_each_offer_discovery_and_keep_session_limits_visible() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            ctx.clear_library();
            ctx.clear_history();
            let html = view! { <Library ctx=ctx/> }.to_html();
            for label in [
                "Chưa có câu chuyện để nghe tiếp",
                "Chưa lưu câu chuyện nào",
                "Chưa có lịch sử nghe",
                "Chưa có tài khoản hoặc đồng bộ",
            ] {
                assert!(html.contains(label), "{label}");
            }
            assert_eq!(html.matches("Khám phá câu chuyện").count(), 3);
        });
    }

    #[test]
    fn saving_labels_name_the_actual_action_and_story_in_both_states() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let title = ctx.work(WorkId::River).title;
            let unsaved = view! { <SaveAction ctx=ctx id=WorkId::River/> }.to_html();
            assert!(unsaved.contains(&format!("aria-label=\"Lưu tác phẩm {title}\"")));
            assert!(unsaved.contains(">Lưu<"));
            ctx.set_saved(WorkId::River, true);
            let saved = view! { <SaveAction ctx=ctx id=WorkId::River/> }.to_html();
            assert!(saved.contains(&format!("aria-label=\"Bỏ lưu tác phẩm {title}\"")));
            assert!(saved.contains(">Bỏ lưu<"));
            assert!(!saved.contains("aria-pressed"));
        });
    }

    #[test]
    fn discovery_embeds_search_with_a_single_page_heading() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let html = view! { <Discover ctx=ctx/> }.to_html();
            assert_eq!(html.matches("<h1").count(), 1);
            assert!(html.contains("id=\"theatre-search\""));
            assert!(html.contains("aria-label=\"Lọc thể loại\""));
        });
    }

    #[test]
    fn history_renders_only_accepted_play_and_names_the_paused_reopen_action() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let empty = view! { <History ctx=ctx/> }.to_html();
            assert!(empty.contains("Chưa có lịch sử nghe"));
            ctx.playback(Event::PlayRequested);
            let populated = view! { <History ctx=ctx/> }.to_html();
            assert!(populated.contains("Xóa lịch sử nghe mẫu"));
            assert!(populated.contains("aria-expanded=\"false\""));
            assert!(populated.contains("Mở vị trí nghe mẫu"));
            assert!(populated.contains("giữ tạm dừng"));
            assert!(!populated.contains("role=\"dialog\""));
        });
    }
}
