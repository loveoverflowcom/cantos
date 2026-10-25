//! App-private semantic HTML compositions, not new shared UI primitives.
#[cfg(feature = "preview")]
use crate::{
    app::{context::TheatreContext, routes::Route},
    application::catalog::Scenario,
    core::ids::WorkId,
    messages::{self, Key},
    platform,
};
use leptos::prelude::*;

/// App-owned decorative artwork. These bundled design assets carry no catalog or release data.
#[derive(Clone, Copy)]
pub(crate) enum EmptyArtworkKind {
    Library,
    Works,
    History,
}

#[component]
pub(crate) fn EmptyArtwork(kind: EmptyArtworkKind) -> impl IntoView {
    match kind {
        EmptyArtworkKind::Library => view! {
            <div class="theatre-empty-artwork" data-artwork="library" aria-hidden="true">
                <img src="assets/empty/library-expressive.png" alt="" width="1374" height="1145" decoding="async"/>
            </div>
        }.into_any(),
        EmptyArtworkKind::Works => paired_empty_artwork("works").into_any(),
        EmptyArtworkKind::History => paired_empty_artwork("history").into_any(),
    }
}

fn paired_empty_artwork(name: &'static str) -> impl IntoView {
    view! {
        <div class="theatre-empty-artwork theatre-empty-artwork-paired" data-artwork=name aria-hidden="true">
            <img class="theatre-empty-artwork-light" src=format!("assets/empty/light/cantos-empty-{name}.svg") alt="" width="240" height="200" decoding="async"/>
            <img class="theatre-empty-artwork-dark" src=format!("assets/empty/dark/cantos-empty-{name}.svg") alt="" width="240" height="200" decoding="async"/>
        </div>
    }
}

// Official licensed static markup, never user content. This is app chrome, not a shared API.
pub(crate) fn icon(asset: &'static str) -> impl IntoView {
    let markup = match asset {
        "home" => include_str!("../../assets/icons/home.svg"),
        "search" => include_str!("../../assets/icons/search.svg"),
        "library_books" => include_str!("../../assets/icons/library_books.svg"),
        "headphones" => include_str!("../../assets/icons/headphones.svg"),
        "settings" => include_str!("../../assets/icons/settings.svg"),
        "smartphone" => include_str!("../../assets/icons/smartphone.svg"),
        "arrow_back" => include_str!("../../assets/icons/arrow_back.svg"),
        "play_arrow" => include_str!("../../assets/icons/play_arrow.svg"),
        "pause" => include_str!("../../assets/icons/pause.svg"),
        "bookmark_border" => include_str!("../../assets/icons/bookmark_border.svg"),
        "check" => include_str!("../../assets/icons/check.svg"),
        "queue_music" => include_str!("../../assets/icons/queue_music.svg"),
        "replay" => include_str!("../../assets/icons/replay.svg"),
        "rotate_right" => include_str!("../../assets/icons/rotate_right.svg"),
        "delete_outline" => include_str!("../../assets/icons/delete_outline.svg"),
        "add" => include_str!("../../assets/icons/add.svg"),
        _ => "",
    };
    view! { <span class="theatre-icon" aria-hidden="true" inner_html=markup></span> }
}

/// The one app-private listener glyph (guest). The shared `Avatar` is not admitted for Theatre yet:
/// it still lacks both-renderer qualification evidence (see the preview catalog gate).
#[component]
pub(crate) fn PersonGlyph() -> impl IntoView {
    view! {
        <svg viewBox="0 0 24 24" aria-hidden="true" focusable="false">
            <circle cx="12" cy="8" r="4" fill="currentColor"/>
            <path d="M4 21v-2a8 8 0 0 1 16 0v2Z" fill="currentColor"/>
        </svg>
    }
}

pub(crate) fn clock(seconds: u32) -> String {
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

#[cfg(feature = "preview")]
/// Whole-percent position for a progress bar; an unknown or zero duration shows no progress.
pub(crate) fn progress_percent(position_ms: u32, duration_seconds: u32) -> u8 {
    let duration_ms = u64::from(duration_seconds) * 1000;
    if duration_ms == 0 {
        return 0;
    }
    (u64::from(position_ms) * 100 / duration_ms).min(100) as u8
}

#[cfg(feature = "preview")]
/// Mock artwork bundled with the app (see assets/provenance.json). Works without a painting
/// keep the typographic cover.
pub(crate) fn artwork_src(id: WorkId) -> Option<&'static str> {
    match id {
        WorkId::Mountain => Some("assets/artwork/son-ha-ky.webp"),
        WorkId::River => Some("assets/artwork/den-khuya.webp"),
        WorkId::Dream => None,
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn PageHeading(
    ctx: TheatreContext,
    title: Key,
    #[prop(optional)] deck: Option<Key>,
) -> impl IntoView {
    view! {
        <header class="theatre-page-heading">
            <p class="theatre-eyebrow">{move || ctx.text(Key::BrandCaption)}</p>
            <h1>{move || ctx.text(title)}</h1>
            {deck.map(|key| view! { <p class="theatre-muted">{move || ctx.text(key)}</p> })}
        </header>
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn Cover(ctx: TheatreContext, id: WorkId) -> impl IntoView {
    match artwork_src(id) {
        Some(src) => view! {
            <div class="theatre-cover" data-work=id.slug() data-art="true" role="img"
                aria-label=move || ctx.format(Key::CoverArtName, &[("title", ctx.work(id).title.to_owned()), ("description", ctx.text(Key::CoverArtPlaceholder).to_owned())])>
                <img class="theatre-cover-art" src=src alt="" decoding="async" loading="lazy"/>
            </div>
        }
        .into_any(),
        None => view! {
            <div class="theatre-cover" data-work=id.slug() role="img"
                aria-label=move || ctx.format(Key::CoverArtName, &[("title", ctx.work(id).title.to_owned()), ("description", ctx.text(Key::CoverPlaceholder).to_owned())])>
                <span class="theatre-cover-moon" aria-hidden="true"></span>
                <span class="theatre-cover-index" aria-hidden="true">"C / "{match id { WorkId::Mountain => "01", WorkId::River => "02", WorkId::Dream => "03" }}</span>
                <strong lang="vi-VN" aria-hidden="true">{ctx.work(id).title}</strong>
                <small aria-hidden="true">{move || ctx.text(messages::genre_key(ctx.work(id).genre))}</small>
            </div>
        }
        .into_any(),
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn SaveAction(ctx: TheatreContext, id: WorkId) -> impl IntoView {
    view! {
        <button type="button" class="theatre-action theatre-action-tonal"
            aria-label=move || ctx.format(if ctx.library.with(|state| state.is_saved(id)) { Key::RemoveSaveName } else { Key::SaveName }, &[("title", ctx.work(id).title.to_owned())])
            on:click=move |_| {
                let desired = !ctx.library.with_untracked(|state| state.is_saved(id));
                ctx.set_saved(id, desired);
                if ctx.route.get_untracked() == Route::Library && !desired { platform::focus_main(ctx.main); }
            }>
            {move || icon(if ctx.library.with(|state| state.is_saved(id)) { "check" } else { "bookmark_border" })}
            <span class="cui-control-label">{move || ctx.text(if ctx.library.with(|state| state.is_saved(id)) { Key::RemoveSave } else { Key::Save })}</span>
        </button>
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn WorkCard(ctx: TheatreContext, id: WorkId) -> impl IntoView {
    view! {
        <article class="theatre-work-card">
            <a class="theatre-cover-link" href=Route::Work(id).hash()><Cover ctx=ctx id=id/></a>
            <div class="theatre-work-meta">
                <h3 lang="vi-VN"><a href=Route::Work(id).hash()>{ctx.work(id).title}</a></h3>
                <p class="theatre-muted">
                    {move || ctx.text(messages::genre_key(ctx.work(id).genre))}
                    <span aria-hidden="true">" · "</span>
                    {move || messages::episode_count(ctx.locale.get(), ctx.work(id).episodes.len())}
                </p>
                <SaveAction ctx=ctx id=id/>
            </div>
        </article>
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn CatalogState(ctx: TheatreContext) -> impl IntoView {
    view! {
        <section class="theatre-state" role="status" aria-atomic="true">
            <h2>{move || ctx.text(match ctx.catalog.scenario() {
                Scenario::Ready => Key::Ready, Scenario::Loading => Key::Loading,
                Scenario::Empty => Key::EmptyCatalog, Scenario::Error => Key::CatalogError,
            })}</h2>
            <p>{move || ctx.text(match ctx.catalog.scenario() {
                Scenario::Ready => Key::DemoNotice, Scenario::Loading => Key::LoadingHelp,
                Scenario::Empty => Key::EmptyCatalogHelp, Scenario::Error => Key::CatalogErrorHelp,
            })}</p>
            <button type="button" class="theatre-action" on:click=move |_| {
                ctx.catalog.load(Scenario::Ready);
                // Recovery removes this button; keep keyboard focus in the persistent landmark.
                platform::focus_main(ctx.main);
            }>
                <span class="cui-control-label">{move || if ctx.catalog.scenario() == Scenario::Error { ctx.text(Key::Retry) } else { ctx.text(Key::LoadDemo) }}</span>
            </button>
        </section>
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn UnsupportedAction(ctx: TheatreContext, label: Key) -> impl IntoView {
    let reason_id = match label {
        Key::Download => "theatre-download-reason",
        Key::Timer => "theatre-timer-reason",
        Key::SignIn => "theatre-sign-in-reason",
        Key::Sync => "theatre-sync-reason",
        _ => "theatre-unsupported-reason",
    };
    view! {
        <div class="theatre-unsupported">
            <button type="button" class="theatre-action theatre-action-tonal" disabled aria-describedby=reason_id><span class="cui-control-label">{move || ctx.text(label)}</span></button>
            <p id=reason_id class="theatre-muted">{move || ctx.text(Key::Unsupported)}</p>
        </div>
    }
}
