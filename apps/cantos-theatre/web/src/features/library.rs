//! App-private semantic HTML compositions, not new shared UI primitives.
use super::common::*;
use crate::{
    app::{context::TheatreContext, routes::Route},
    core::{catalog::Episode, ids::EpisodeId, listening::library::Bookmark},
    messages::Key,
    platform,
};
use leptos::prelude::*;

/// Progress as a value, not a control: the bar carries the same facts as the visible time.
#[component]
fn ProgressMeter(ctx: TheatreContext, position_ms: Signal<u32>, duration: u32) -> impl IntoView {
    let percent = move || progress_percent(position_ms.get(), duration);
    view! {
        <div class="theatre-progress-row">
            <div class="theatre-progress" role="progressbar" aria-valuemin="0" aria-valuemax="100"
                aria-valuenow=move || percent().to_string()
                aria-label=move || ctx.format(Key::ProgressName, &[
                    ("time", clock(position_ms.get() / 1000)),
                    ("total", clock(duration)),
                ])>
                <span style=move || format!("--theatre-progress: {}%", percent())></span>
            </div>
            <p class="theatre-time">{move || clock(position_ms.get() / 1000)}<span aria-hidden="true">" / "</span>{clock(duration)}</p>
        </div>
    }
}

/// Reopening restores the saved position paused, so the label says "open", never "continue".
#[component]
fn OpenPositionButton(
    ctx: TheatreContext,
    id: EpisodeId,
    episode: Episode,
    position_ms: Signal<u32>,
    #[prop(optional)] tonal: bool,
    #[prop(optional)] from_history: bool,
) -> impl IntoView {
    view! {
        <button type="button" class=if tonal { "theatre-action theatre-action-tonal" } else { "theatre-action" }
            aria-label=move || ctx.format(Key::ResumeSampleName, &[("title", episode.title.to_owned()), ("time", clock(position_ms.get() / 1000))])
            on:click=move |_| { if from_history { ctx.resume_episode(id); } else { ctx.resume_progress(id); } }>
            {icon("replay")}<span class="cui-control-label">{move || ctx.text(Key::ResumeSample)}</span>
        </button>
    }
}

/// Artwork, names, position and the action as one cluster; the width is bounded by the content.
#[component]
fn ContinuePanel(ctx: TheatreContext, id: EpisodeId) -> impl IntoView {
    let Some(episode) = ctx.episode(id) else {
        return ().into_any();
    };
    let position_ms = Signal::derive(move || {
        ctx.progress
            .get()
            .find(id)
            .map(|progress| progress.snapshot.position_ms())
            .unwrap_or_default()
    });
    view! {
        <div class="theatre-panel theatre-continue">
            <div class="theatre-continue-art"><Cover ctx=ctx id=id.work/></div>
            <div class="theatre-continue-body">
                <h3 lang="vi-VN" class="theatre-continue-title"><a href=Route::Work(id.work).hash()>{ctx.work(id.work).title}</a></h3>
                <p lang="vi-VN" class="theatre-continue-episode">{episode.title}</p>
                <ProgressMeter ctx=ctx position_ms=position_ms duration=episode.duration/>
                <OpenPositionButton ctx=ctx id=id episode=episode position_ms=position_ms/>
                <p class="theatre-muted theatre-hint">{move || ctx.text(Key::ResumeSampleHelp)}</p>
            </div>
        </div>
    }
    .into_any()
}

#[component]
pub(crate) fn ContinueCard(ctx: TheatreContext) -> impl IntoView {
    let latest = Memo::new(move |_| {
        ctx.progress
            .get()
            .latest()
            .map(|progress| progress.snapshot.source().episode())
    });
    view! {
        <Show when=move || latest.get().is_some()>
            <section class="theatre-library-section" aria-labelledby="theatre-discover-continue">
                <h2 id="theatre-discover-continue" class="theatre-section-heading">{move || ctx.text(Key::ContinueHeading)}</h2>
                {move || latest.get().map(|id| view! { <ContinuePanel ctx=ctx id=id/> })}
            </section>
        </Show>
    }
}

/// A short reason and a way forward. `compact` drops the icon block and the action for
/// secondary sections that should not repeat the primary call to action.
#[component]
fn SectionEmpty(
    ctx: TheatreContext,
    title: Key,
    help: Key,
    #[prop(optional)] compact: bool,
    #[prop(optional)] asset: &'static str,
) -> impl IntoView {
    if compact {
        return view! { <p class="theatre-muted theatre-quiet-empty">{move || ctx.text(help)}</p> }
            .into_any();
    }
    view! {
        <div class="theatre-panel theatre-empty">
            {icon(asset)}
            <div>
                <h3>{move || ctx.text(title)}</h3>
                <p class="theatre-muted">{move || ctx.text(help)}</p>
                <a class="theatre-action" href=Route::Discover.hash()><span class="cui-control-label">{move || ctx.text(Key::BrowseStories)}</span></a>
            </div>
        </div>
    }
    .into_any()
}

#[component]
pub(crate) fn Library(ctx: TheatreContext) -> impl IntoView {
    let recent = move || {
        ctx.progress
            .get()
            .recent()
            .into_iter()
            .map(|progress| progress.snapshot.source().episode())
            .collect::<Vec<_>>()
    };
    let earlier = move || recent().into_iter().skip(1).collect::<Vec<_>>();
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
        <section class="theatre-library-section" aria-labelledby="theatre-continue-heading">
            <h2 id="theatre-continue-heading" class="theatre-section-heading">{move || ctx.text(Key::Continue)}</h2>
            <Show when=move || !recent().is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoContinue help=Key::NoContinueHelp asset="headphones"/> }>
                {move || recent().first().copied().map(|id| view! { <ContinuePanel ctx=ctx id=id/> })}
                <ul class="theatre-history-list"><For each=earlier key=|id| *id children=move |id| view! { <ListeningRow ctx=ctx id=id/> }/></ul>
            </Show>
        </section>
        <section class="theatre-library-section" aria-labelledby="theatre-saved-heading">
            <h2 id="theatre-saved-heading" class="theatre-section-heading">{move || ctx.text(Key::Saved)}</h2>
            <Show when=move || !ctx.library.get().saved.is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoSaved help=Key::NoSavedHelp asset="bookmark_border"/> }>
                <div class="theatre-shelf"><For each=saved_works key=|id| *id children=move |id| view! { <WorkCard ctx=ctx id=id/> }/></div>
            </Show>
        </section>
        <section class="theatre-library-section" aria-labelledby="theatre-history-heading">
            <div class="theatre-section-header">
                <h2 id="theatre-history-heading" class="theatre-section-heading">{move || ctx.text(Key::History)}</h2>
                <a class="theatre-text-link" href=Route::History.hash() aria-label=move || ctx.text(Key::ViewAllHistory)>{move || ctx.text(Key::ViewAll)}</a>
            </div>
            <Show when=move || !ctx.history.get().records().is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoHistory help=Key::NoHistoryHelp compact=true/> }>
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
        <Show when=move || !ctx.history.get().records().is_empty() fallback=move || view! { <SectionEmpty ctx=ctx title=Key::NoHistory help=Key::NoHistoryHelp asset="replay"/> }>
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
    let position_ms = Signal::derive(move || {
        ctx.progress
            .get()
            .find(id)
            .map(|progress| progress.snapshot.position_ms())
            .unwrap_or_default()
    });
    view! {
        <li class="theatre-listening-row">
            <div class="theatre-row-art"><Cover ctx=ctx id=id.work/></div>
            <div class="theatre-row-copy">
                <h3 lang="vi-VN"><a href=Route::Work(id.work).hash()>{ctx.work(id.work).title}</a></h3>
                <p lang="vi-VN">{episode.title}</p>
                <ProgressMeter ctx=ctx position_ms=position_ms duration=episode.duration/>
            </div>
            <OpenPositionButton ctx=ctx id=id episode=episode position_ms=position_ms tonal=true from_history=from_history/>
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
            assert!(html.contains("Lịch sử ghi nhận"));
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
                "Chưa có gì để nghe tiếp",
                "Chưa lưu tác phẩm nào",
                "Lịch sử ghi nhận",
            ] {
                assert!(html.contains(label), "{label}");
            }
            // Continue and Saved explain themselves with an icon and a route; History stays quiet.
            assert_eq!(html.matches("theatre-empty").count(), 2);
            assert!(html.matches("theatre-icon").count() >= 2);
            assert_eq!(html.matches("Khám phá tác phẩm").count(), 2);
            assert_eq!(html.matches("href=\"#discover\"").count(), 2);
            // The global mock notice owns the persistence limits; the page does not repeat them.
            assert!(!html.contains("Chưa có tài khoản hoặc đồng bộ"));
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
            assert!(populated.contains("Mở vị trí nghe"));
            assert!(!populated.contains("Nghe tiếp"));
            assert!(populated.contains("giữ tạm dừng"));
            assert!(!populated.contains("role=\"dialog\""));
        });
    }

    #[test]
    fn continue_cluster_keeps_artwork_names_progress_and_the_action_together() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            let html = view! { <Library ctx=ctx/> }.to_html();
            let (_, cluster) = html
                .split_once("theatre-continue\"")
                .expect("continue cluster");
            let cluster = cluster.split("theatre-saved-heading").next().unwrap();
            assert!(cluster.contains("assets/artwork/son-ha-ky.webp"));
            assert!(cluster.contains("Sơn Hà Ký"));
            assert!(cluster.contains("Người giữ tiếng chuông"));
            assert!(cluster.contains("role=\"progressbar\""));
            assert!(cluster.contains("aria-valuenow=\"44\""));
            assert!(cluster.contains("Đã nghe 12:48 trên 28:42"));
            assert!(cluster.contains("12:48"));
            assert!(cluster.contains("28:42"));
            // The action sits in the same cluster, and it names what really happens: open paused.
            assert!(
                cluster.contains("aria-label=\"Mở vị trí nghe Người giữ tiếng chuông tại 12:48\"")
            );
            assert!(cluster.contains(">Mở vị trí nghe<"));
            assert!(!cluster.contains(">Nghe tiếp<"));
        });
    }

    #[test]
    fn progress_percent_is_clamped_and_zero_for_unknown_duration() {
        assert_eq!(progress_percent(0, 1000), 0);
        assert_eq!(progress_percent(768_000, 1722), 44);
        assert_eq!(progress_percent(500_000, 1000), 50);
        assert_eq!(progress_percent(2_000_000, 1000), 100);
        assert_eq!(progress_percent(u32::MAX, 1), 100);
        assert_eq!(progress_percent(5_000, 0), 0);
    }

    #[test]
    fn artwork_is_mapped_per_work_and_unmapped_works_keep_the_typographic_cover() {
        Owner::new().with(|| {
            let ctx = TheatreContext::new(NodeRef::new());
            assert!(
                artwork_src(WorkId::Mountain).is_some_and(|src| src.ends_with("son-ha-ky.webp"))
            );
            assert!(artwork_src(WorkId::River).is_some_and(|src| src.ends_with("den-khuya.webp")));
            assert_eq!(artwork_src(WorkId::Dream), None);
            let art = view! { <Cover ctx=ctx id=WorkId::River/> }.to_html();
            assert!(art.contains("data-art=\"true\"") && art.contains("alt=\"\""));
            assert!(art.contains("chưa xác minh quyền"));
            let typographic = view! { <Cover ctx=ctx id=WorkId::Dream/> }.to_html();
            assert!(typographic.contains("theatre-cover-moon") && !typographic.contains("<img"));
            assert!(typographic.contains("chưa có ảnh được cấp quyền"));
        });
    }
}
