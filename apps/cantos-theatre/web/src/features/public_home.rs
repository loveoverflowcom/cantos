//! Public Home composition. Session lookup and auth are separate, explicit user intents.
//!
//! This is the Khám phá screen: the intro, the search/filter form and the public Work list. The
//! list is only a projection of [`PublicDiscoveryState`]; every search, retry and continuation is
//! a callback the shell turns into a request, so nothing here parses a cursor or fetches.
use crate::contract::discovery::{PresentedWorkSummary, WorkKind};
use crate::{
    app::routes::Route,
    application::public_discovery::{
        kind_name, DiscoveryQuery, PublicDiscoveryFailure, PublicDiscoveryPhase,
        PublicDiscoveryState, QueryError, Tail,
    },
    core::public_ids::PublicWorkId,
    features::{
        auth_copy::{self, CopyKey},
        common::{icon, EmptyArtwork, EmptyArtworkKind},
        discovery_language,
    },
    messages::{self, text, Key, Locale},
};
use cantos_ui::{
    Button, ButtonState, ButtonVariant, DisabledReason, IconName, TextField, TextFieldKind,
    TextFieldVariant,
};
use leptos::prelude::*;

pub(crate) const MORE_ID: &str = "theatre-discovery-more";
pub(crate) const STATUS_ID: &str = "theatre-discovery-status";
const KINDS: [Option<WorkKind>; 4] = [
    None,
    Some(WorkKind::Novel),
    Some(WorkKind::Story),
    Some(WorkKind::Podcast),
];

/// Stable focus target for returning from that Work's detail.
pub(crate) fn work_link_id(work: PublicWorkId) -> String {
    format!("theatre-public-work-link-{work}")
}

/// Where focus goes once a listener-initiated page request settles: the first row it added,
/// else the polite status line.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn page_focus_target(state: &PublicDiscoveryState) -> String {
    state
        .first_new_row()
        .and_then(PublicWorkId::parse)
        .map(work_link_id)
        .unwrap_or_else(|| STATUS_ID.to_owned())
}

/// Plain primary activation is routed in-app; modified clicks keep native new-tab behavior.
fn in_app_activation(event: &leptos::ev::MouseEvent) -> bool {
    event.button() == 0
        && !event.default_prevented()
        && !(event.ctrl_key() || event.meta_key() || event.shift_key() || event.alt_key())
}

/// Whether the key was pressed in an `<input>`, not on a chip or a button inside the form.
fn from_text_field(event: &leptos::ev::KeyboardEvent) -> bool {
    use leptos::wasm_bindgen::JsCast;
    event.target().is_some_and(|target| {
        target
            .dyn_ref::<leptos::web_sys::HtmlInputElement>()
            .is_some()
    })
}

/// The kind of the region, so a page of rows never rebuilds the list (and the focus in it).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Region {
    Loading,
    Ready,
    Empty,
    Incomplete,
    Expired,
    Failed(PublicDiscoveryFailure),
}

fn region(phase: PublicDiscoveryPhase) -> Region {
    match phase {
        PublicDiscoveryPhase::Idle | PublicDiscoveryPhase::Loading => Region::Loading,
        PublicDiscoveryPhase::Ready => Region::Ready,
        PublicDiscoveryPhase::Empty => Region::Empty,
        PublicDiscoveryPhase::Incomplete => Region::Incomplete,
        PublicDiscoveryPhase::Expired => Region::Expired,
        PublicDiscoveryPhase::Failed(failure) => Region::Failed(failure),
    }
}

const fn unit_count_key(kind: WorkKind) -> Key {
    match kind {
        WorkKind::Podcast => Key::PublicWorkUnitCountEpisodes,
        WorkKind::Novel | WorkKind::Story => Key::PublicWorkUnitCountChapters,
    }
}

const fn query_error_key(error: QueryError) -> Key {
    match error {
        QueryError::SearchTooLong => Key::DiscoverySearchTooLong,
        QueryError::SearchControlCharacter => Key::DiscoverySearchInvalid,
        QueryError::LanguageSyntax => Key::DiscoveryLanguageInvalid,
    }
}

/// "Truyện · Tiếng Việt · Chương đã công bố: 12", the card's server-supplied facts.
fn feature_meta(kind: WorkKind, language: &str, count: u64, locale: Locale) -> String {
    let units = messages::format(
        unit_count_key(kind),
        locale,
        &[("count", count.to_string())],
    );
    messages::format(
        Key::PublicHomeFeatureMeta,
        locale,
        &[
            (
                "kind",
                text(messages::work_kind_key(kind), locale).to_owned(),
            ),
            ("language", language.to_owned()),
            ("units", units),
        ],
    )
}

/// The public contract carries no cover yet (#157/#171), so every Work shows this deliberate
/// fallback: decoration plus the kind. It invents no artwork and borrows no other Work's image.
#[component]
fn WorkCover(kind: WorkKind, locale: RwSignal<Locale>) -> impl IntoView {
    let glyph = match kind {
        WorkKind::Podcast => icon("headphones"),
        WorkKind::Novel | WorkKind::Story => icon("library_books"),
    };
    view! {
        <div class="theatre-cover theatre-public-cover" data-kind=kind_name(kind) aria-hidden="true">
            <span class="theatre-cover-moon"></span>
            <span class="theatre-public-cover-glyph">{glyph}</span>
            <small>{move || text(messages::work_kind_key(kind), locale.get())}</small>
        </div>
    }
}

#[component]
fn SearchForm(
    applied: RwSignal<DiscoveryQuery>,
    locale: RwSignal<Locale>,
    on_search: Callback<DiscoveryQuery>,
) -> impl IntoView {
    let panel = NodeRef::<leptos::html::Div>::new();
    let initial = applied.get_untracked();
    let search = RwSignal::new(initial.text().to_owned());
    let kind = RwSignal::new(initial.kind());
    let error = RwSignal::new(None::<QueryError>);
    // Back/Forward and a cleared search replace the applied query; the fields follow it.
    Effect::new(move |_| {
        let query = applied.get();
        search.set(query.text().to_owned());
        kind.set(query.kind());
        error.set(None);
    });
    // Discovery never narrows by content language (#424). The listener's earlier language choice
    // is the interface locale, not a catalogue filter; no content-language preference exists yet.
    let submit =
        move || match DiscoveryQuery::new(&search.get_untracked(), kind.get_untracked(), "") {
            Ok(query) => {
                error.set(None);
                on_search.run(query);
            }
            Err(refused) => error.set(Some(refused)),
        };
    let search_error = Signal::derive(move || {
        error
            .get()
            .map(|e| text(query_error_key(e), locale.get()).to_owned())
    });
    let filtered = move || !applied.get().is_unfiltered();
    view! {
        <div node_ref=panel class="theatre-public-search" role="search"
            on:keydown=move |event| {
                // Enter in a text field applies the search, except while an input method is
                // composing. On a chip or button it keeps its own activation.
                if event.key() == "Enter" && !event.is_composing() && from_text_field(&event) {
                    event.prevent_default();
                    submit();
                }
            }>
            <div class="theatre-public-search-main">
                <TextField id="theatre-discovery-search" kind=TextFieldKind::Search
                    variant=TextFieldVariant::Floating
                    label=Signal::derive(move || text(Key::DiscoverySearchLabel, locale.get()).to_owned())
                    value=search on_input=Callback::new(move |value| search.set(value))
                    error=search_error autocomplete="off"/>
                <Button id="theatre-discovery-submit" variant=ButtonVariant::Tonal icon=IconName::Search
                    on_press=Callback::new(move |_| submit())>
                    {move || text(Key::DiscoverySearchSubmit, locale.get())}
                </Button>
            </div>
            <div class="theatre-public-search-filters">
                <div class="theatre-filters" role="group" aria-label=move || text(Key::DiscoveryKindLabel, locale.get())>
                    {KINDS.into_iter().map(|choice| view! {
                        <button type="button" class="theatre-chip" data-kind=choice.map_or("all", kind_name)
                            aria-pressed=move || (kind.get() == choice).to_string()
                            on:click=move |_| { kind.set(choice); submit(); }>
                            <span class="cui-control-label">{move || text(
                                choice.map_or(Key::DiscoveryKindAll, messages::work_kind_key), locale.get())}</span>
                        </button>
                    }).collect_view()}
                </div>
                <Show when=filtered>
                    <Button id="theatre-discovery-clear" icon=IconName::Close variant=ButtonVariant::Text
                        on_press=Callback::new(move |_| {
                            on_search.run(DiscoveryQuery::default());
                            crate::platform::restore_disappearing_action_focus(panel, "theatre-discovery-search");
                        })>
                        {move || text(Key::DiscoveryClear, locale.get())}
                    </Button>
                </Show>
            </div>
        </div>
    }
}

#[component]
fn WorkCard(
    work: PresentedWorkSummary,
    locale: RwSignal<Locale>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    let kind = work.kind;
    let count = work.unit_count;
    let language = discovery_language::display_name(&work.language);
    view! {
        <article class="theatre-public-work theatre-work-card theatre-panel" data-work-id=work.work_id.clone()>
            <WorkCover kind locale/>
            <div class="theatre-public-work-body">
                <p class="theatre-eyebrow">{move || text(messages::work_kind_key(kind), locale.get())}</p>
                <h3>{match PublicWorkId::parse(&work.work_id) {
                    Some(id) => {
                        let action_id = format!("theatre-public-work-action-{id}");
                        let title_id = format!("theatre-public-work-title-{id}");
                        let labelled_by = format!("{action_id} {title_id}");
                        view! {
                        <a id=work_link_id(id) class="theatre-public-work-link" href=Route::PublicWork(id).hash()
                            lang=move || locale.get().lang()
                            aria-labelledby=labelled_by
                            on:click=move |event| if in_app_activation(&event) {
                                event.prevent_default();
                                on_open_work.run(id);
                            }>
                            <span id=action_id class="cui-sr-only" lang=move || locale.get().lang()>{move || text(Key::PublicHomeOpenWork, locale.get())}</span>
                            <span id=title_id class="theatre-public-work-title" lang=work.language.clone()>{work.title}</span>
                            <span class="theatre-public-work-arrow" aria-hidden="true">{icon("arrow_back")}</span>
                        </a>
                    }.into_any()
                    },
                    // The decoder admits only canonical IDs; never route a guessed one.
                    None => view! { <span lang=work.language.clone()>{work.title}</span> }.into_any(),
                }}</h3>
                {(!work.description.trim().is_empty()).then(|| view! {
                    <p lang=work.language class="theatre-muted theatre-public-work-description">{work.description}</p>
                })}
                <p class="theatre-muted theatre-public-work-count">
                    {move || feature_meta(kind, &language, count, locale.get())}
                </p>
            </div>
        </article>
    }
}

#[component]
pub(crate) fn PublicHome(
    state: RwSignal<PublicDiscoveryState>,
    locale: RwSignal<Locale>,
    /// The search the link carries; the fields mirror it and a submit asks the shell to apply one.
    applied: RwSignal<DiscoveryQuery>,
    on_search: Callback<DiscoveryQuery>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    /// Retry the failed read (same cursor for a later page) or reload page one.
    on_retry: Callback<()>,
    on_load_more: Callback<()>,
    /// A fresh walk of the current search, for an expired cursor or an explicit refresh.
    on_restart: Callback<()>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    let current = Memo::new(move |_| state.with(|s| region(s.phase())));
    // Every returned Work stays in one keyed shelf, including the first result. This keeps
    // continuation and Back focus identities independent of visual rank or card reflow.
    let rows = move || state.with(|s| s.rows().to_vec());
    let tail = Memo::new(move |_| state.with(|s| s.tail()));
    let count = move || state.with(|s| s.rows().len());
    let status_line = move || match tail.get() {
        Tail::End => messages::format(
            Key::DiscoveryComplete,
            locale.get(),
            &[("count", count().to_string())],
        ),
        Tail::More | Tail::Loading => messages::format(
            Key::DiscoveryShowing,
            locale.get(),
            &[("count", count().to_string())],
        ),
        Tail::Failed(PublicDiscoveryFailure::Diagnostic(kind)) => {
            text(crate::messages::error_key(kind), locale.get()).to_owned()
        }
        Tail::Failed(PublicDiscoveryFailure::Unavailable) => {
            text(Key::DiscoveryPageUnavailable, locale.get()).to_owned()
        }
        Tail::Failed(PublicDiscoveryFailure::Protocol) => {
            text(Key::DiscoveryPageProtocol, locale.get()).to_owned()
        }
        Tail::Expired => text(Key::DiscoveryExpired, locale.get()).to_owned(),
    };
    let more_state = Signal::derive(move || match tail.get() {
        Tail::Loading => DisabledReason::new(text(Key::DiscoveryLoadingMore, locale.get()))
            .map(ButtonState::Loading)
            .unwrap_or_default(),
        _ => ButtonState::Enabled,
    });
    let more_label = move || match tail.get() {
        Tail::Failed(_) => Key::PublicWorkRetryPage,
        Tail::Expired => Key::DiscoveryRestart,
        _ => Key::DiscoveryLoadMore,
    };
    let more_press = Callback::new(move |_| match tail.get_untracked() {
        Tail::Failed(_) => on_retry.run(()),
        Tail::Expired => on_restart.run(()),
        Tail::More | Tail::End | Tail::Loading => on_load_more.run(()),
    });
    view! {
        <section class="theatre-public-home" aria-labelledby="theatre-public-home-heading">
            <header class="theatre-discover-intro">
                <div class="theatre-discover-intro-copy">
                    <p class="theatre-eyebrow">{move || text(Key::PublicHomeEyebrow, locale.get())}</p>
                    <h1 id="theatre-public-home-heading">{move || text(Key::HomeHeading, locale.get())}</h1>
                    <p class="theatre-muted">{move || text(Key::PublicHomeIntroduction, locale.get())}</p>
                </div>
                // The one guest cluster: it sits beside the intro and wraps below it when narrow.
                // Create account leads as the filled action; Sign in is the tonal alternative.
                <div class="theatre-home-auth-actions">
                    <Button id="theatre-home-login" icon=IconName::Login variant=ButtonVariant::AuthSecondary
                        focus_on_press=true on_press=Callback::new(move |_| on_login.run(()))>
                        {move || text(Key::SignIn, locale.get())}
                    </Button>
                    <Button id="theatre-home-register" icon=IconName::PersonAdd variant=ButtonVariant::AuthPrimary
                        focus_on_press=true on_press=Callback::new(move |_| on_register.run(()))>
                        {move || auth_copy::text(CopyKey::CreateAccount, locale.get())}
                    </Button>
                </div>
            </header>
            <section class="theatre-public-catalog" aria-labelledby="theatre-public-shelf-heading"
                aria-busy=move || (current.get() == Region::Loading).then_some("true")>
                <h2 id="theatre-public-shelf-heading">{move || text(Key::StoryShelf, locale.get())}</h2>
                <SearchForm applied locale on_search/>
                {move || match current.get() {
                    Region::Loading => view! {
                        <p class="theatre-public-state" role="status">{move || text(Key::PublicHomeLoading, locale.get())}</p>
                        <div class="theatre-shelf theatre-public-skeletons" aria-hidden="true">
                            {(0..3).map(|_| view! { <div class="theatre-public-skeleton"></div> }).collect_view()}
                        </div>
                    }.into_any(),
                    Region::Ready => view! {
                        <div class="theatre-shelf theatre-public-work-grid">
                            <For each=rows key=|work| work.work_id.clone()
                                children=move |work| view! { <WorkCard work locale on_open_work/> }/>
                        </div>
                        <p id=STATUS_ID class="theatre-muted theatre-public-status" role="status" tabindex="-1">{status_line}</p>
                        <Show when=move || !matches!(tail.get(), Tail::End)>
                            <Button id=MORE_ID variant=ButtonVariant::Tonal state=more_state on_press=more_press>
                                {move || text(more_label(), locale.get())}
                            </Button>
                        </Show>
                    }.into_any(),
                    Region::Empty => {
                        let unfiltered = applied.with(|q| q.is_unfiltered());
                        let (heading, help) = if unfiltered {
                            (Key::PublicHomeEmpty, Key::PublicHomeEmptyHelp)
                        } else {
                            (Key::DiscoveryNoMatches, Key::DiscoveryNoMatchesHelp)
                        };
                        view! {
                            <div class="theatre-public-state theatre-public-empty theatre-panel" role="status">
                                <EmptyArtwork kind=EmptyArtworkKind::Works/>
                                <h3>{move || text(heading, locale.get())}</h3>
                                <p>{move || text(help, locale.get())}</p>
                                <Button id="theatre-home-refresh" icon=IconName::Refresh variant=ButtonVariant::Tonal
                                    on_press=Callback::new(move |_| on_restart.run(()))>
                                    {move || text(Key::PublicHomeRefresh, locale.get())}
                                </Button>
                            </div>
                        }.into_any()
                    }
                    Region::Incomplete => view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h3>{move || text(Key::DiscoveryIncomplete, locale.get())}</h3>
                            <p>{move || text(Key::DiscoveryIncompleteHelp, locale.get())}</p>
                            <Button id=MORE_ID variant=ButtonVariant::Tonal state=more_state on_press=Callback::new(move |_| on_load_more.run(()))>
                                {move || text(Key::DiscoveryKeepSearching, locale.get())}
                            </Button>
                        </div>
                    }.into_any(),
                    Region::Expired => view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h3>{move || text(Key::DiscoveryExpired, locale.get())}</h3>
                            <Button id=MORE_ID variant=ButtonVariant::Tonal on_press=Callback::new(move |_| on_restart.run(()))>
                                {move || text(Key::DiscoveryRestart, locale.get())}
                            </Button>
                        </div>
                    }.into_any(),
                    Region::Failed(failure) => {
                        let (heading, help) = match failure {
                            PublicDiscoveryFailure::Diagnostic(kind) => (Key::ErrorDialogTitle, crate::messages::error_key(kind)),
                            PublicDiscoveryFailure::Unavailable => (Key::PublicHomeUnavailable, Key::PublicConnectionUnavailableHelp),
                            PublicDiscoveryFailure::Protocol => (Key::PublicHomeProtocol, Key::PublicHomeProtocolHelp),
                        };
                        view! {
                            <div class="theatre-public-state theatre-panel" role="status">
                                <h3>{move || text(heading, locale.get())}</h3>
                                <p>{move || text(help, locale.get())}</p>
                                <Button id="theatre-home-retry" icon=IconName::Refresh variant=ButtonVariant::Tonal
                                    on_press=Callback::new(move |_| on_retry.run(()))>
                                    {move || text(Key::PublicRetry, locale.get())}
                                </Button>
                            </div>
                        }.into_any()
                    }
                }}
            </section>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::public_discovery::DiscoveryReadError;
    use crate::contract::discovery::PresentedWorkPage;

    fn render_with(state: PublicDiscoveryState, applied: DiscoveryQuery, locale: Locale) -> String {
        Owner::new().with(|| {
            view! {
                <PublicHome state=RwSignal::new(state) locale=RwSignal::new(locale)
                    applied=RwSignal::new(applied) on_search=Callback::new(|_| ())
                    on_login=Callback::new(|()| ()) on_register=Callback::new(|()| ())
                    on_retry=Callback::new(|()| ()) on_load_more=Callback::new(|()| ())
                    on_restart=Callback::new(|()| ()) on_open_work=Callback::new(|_| ()) />
            }
            .to_html()
        })
    }
    fn render(state: PublicDiscoveryState, locale: Locale) -> String {
        render_with(state, DiscoveryQuery::default(), locale)
    }
    fn work(title: &str) -> PresentedWorkSummary {
        PresentedWorkSummary {
            work_id: "00000000-0000-0000-0000-000000000001".into(),
            kind: WorkKind::Story,
            title: title.into(),
            description: "Ngày mai, mình có diễn tiếp không?".into(),
            language: "vi-VN".into(),
            first_published_at_ms: 1,
            unit_count: 3,
        }
    }
    fn answered(
        query: DiscoveryQuery,
        result: Result<PresentedWorkPage, DiscoveryReadError>,
    ) -> PublicDiscoveryState {
        let mut state = PublicDiscoveryState::default();
        let ticket = state.begin(query).unwrap();
        state.finish(&ticket, result);
        state
    }
    fn page(items: Vec<PresentedWorkSummary>, next: Option<&str>) -> PresentedWorkPage {
        PresentedWorkPage {
            items,
            next_cursor: next.map(str::to_owned),
        }
    }

    #[test]
    fn outage_keeps_public_home_and_auth_intents_visible_without_mock_fallback() {
        let state = answered(
            DiscoveryQuery::default(),
            Err(DiscoveryReadError::Unavailable),
        );
        let html = render(state, Locale::Vietnamese);
        for expected in [
            "Mở một miền tưởng tượng.",
            "không cần đợi bạn đăng nhập",
            "theatre-home-login",
            "theatre-home-register",
            "theatre-home-retry",
            "theatre-discovery-search",
        ] {
            assert!(html.contains(expected), "{expected}");
        }
        for absent in [
            "Sơn Hà Ký",
            "Minh An",
            "Đang hiển thị",
            "access_token",
            "<audio",
            "theatre-public-work-grid",
        ] {
            assert!(!html.contains(absent), "{absent}");
        }
    }

    #[test]
    fn published_story_uses_server_text_safely_and_never_invents_art_or_playback() {
        let state = answered(
            DiscoveryQuery::default(),
            Ok(page(
                vec![work("Ánh đèn <cuối> sân khấu")],
                Some("opaque-page"),
            )),
        );
        let html = render(state, Locale::English);
        assert!(html.contains("Ánh đèn &lt;cuối&gt; sân khấu"));
        assert!(html.contains("href=\"#work/00000000-0000-0000-0000-000000000001\""));
        assert!(
            html.contains("id=\"theatre-public-work-link-00000000-0000-0000-0000-000000000001\"")
        );
        assert!(html.contains("Ngày mai, mình có diễn tiếp không?"));
        assert!(html.contains("Every story, a world"));
        // The count is the server's public unit_count, never the length of the page.
        assert!(html.contains("Published chapters: 3"));
        assert!(html.contains("Showing 1 stories; the catalog has more."));
        assert!(html.contains("theatre-discovery-more") && html.contains("Load more stories"));
        for absent in [
            "theatre-home-retry",
            "<img",
            "<audio",
            "publication_id",
            "opaque-page",
        ] {
            assert!(!html.contains(absent), "{absent}");
        }
    }

    #[test]
    fn a_podcast_counts_episodes_and_the_end_is_only_a_null_cursor() {
        let mut podcast = work("Đêm khuya");
        podcast.kind = WorkKind::Podcast;
        podcast.unit_count = 12;
        let state = answered(DiscoveryQuery::default(), Ok(page(vec![podcast], None)));
        let html = render(state, Locale::Vietnamese);
        assert!(html.contains("Tập đã công bố: 12"));
        assert!(html.contains("Đã hiển thị hết các tác phẩm: 1."));
        assert!(!html.contains("theatre-discovery-more"));
    }

    #[test]
    fn public_home_renders_each_typed_state_in_both_locales() {
        let filtered = DiscoveryQuery::new("đèn", None, "").unwrap();
        for (locale, loading, empty, no_match, unavailable, protocol, retry, refresh) in [
            (
                Locale::Vietnamese,
                "Đang mở những câu chuyện đã công bố…",
                "Những câu chuyện đang được chuẩn bị.",
                "Không có tác phẩm nào khớp.",
                "Chưa mở được danh mục công khai.",
                "Chưa đọc được danh mục công khai.",
                "Thử lại",
                "Cập nhật danh mục",
            ),
            (
                Locale::English,
                "Opening published stories…",
                "Stories are being prepared.",
                "No stories match.",
                "The public catalog could not be opened.",
                "The public catalog response could not be read.",
                "Try again",
                "Refresh catalog",
            ),
        ] {
            assert!(render(PublicDiscoveryState::default(), locale).contains(loading));
            // An unfiltered empty catalog and a search that matched nothing are different facts.
            let html = render(
                answered(DiscoveryQuery::default(), Ok(page(vec![], None))),
                locale,
            );
            assert!(html.contains(empty) && html.contains(refresh), "{html}");
            let html = render_with(
                answered(filtered.clone(), Ok(page(vec![], None))),
                filtered.clone(),
                locale,
            );
            assert!(html.contains(no_match) && !html.contains(empty), "{html}");
            for (failure, heading) in [
                (DiscoveryReadError::Unavailable, unavailable),
                (DiscoveryReadError::Protocol, protocol),
            ] {
                let html = render(answered(DiscoveryQuery::default(), Err(failure)), locale);
                assert!(html.contains(heading) && html.contains(retry), "{html}");
                assert!(html.contains("role=\"status\""));
            }
        }
    }

    #[test]
    fn an_empty_page_with_a_cursor_is_never_presented_as_no_results() {
        let mut state = PublicDiscoveryState::default();
        let first = state.begin(DiscoveryQuery::default()).unwrap();
        // Exhaust the automatic follow-ups with empty pages that keep a cursor.
        let mut ticket = first;
        while let crate::application::public_discovery::Step::Continue(next) =
            state.finish(&ticket, Ok(page(vec![], Some("more"))))
        {
            ticket = next;
        }
        for (locale, heading, keep, absent) in [
            (
                Locale::Vietnamese,
                "Chưa thấy kết quả trong phần đã duyệt.",
                "Tìm tiếp",
                "Không có tác phẩm nào khớp.",
            ),
            (
                Locale::English,
                "No match yet in the part searched so far.",
                "Keep searching",
                "No stories match.",
            ),
        ] {
            let html = render(state.clone(), locale);
            assert!(html.contains(heading) && html.contains(keep), "{html}");
            assert!(
                !html.contains(absent) && !html.contains("Những câu chuyện đang được chuẩn bị")
            );
        }
    }

    #[test]
    fn a_failed_later_page_and_an_expired_cursor_keep_the_rows_and_name_the_recovery() {
        let mut state = answered(
            DiscoveryQuery::default(),
            Ok(page(vec![work("A")], Some("c1"))),
        );
        let more = state.load_more().unwrap();
        state.finish(&more, Err(DiscoveryReadError::Unavailable));
        let html = render(state.clone(), Locale::English);
        assert!(html.contains("data-work-id=\"00000000-0000-0000-0000-000000000001\""));
        assert!(html.contains("Stories already shown are kept."));
        assert!(html.contains("Retry this part"));
        let retry = state.retry().unwrap();
        state.finish(&retry, Err(DiscoveryReadError::InvalidCursor));
        let html = render(state, Locale::Vietnamese);
        assert!(html.contains("data-work-id=\"00000000-0000-0000-0000-000000000001\""));
        assert!(html.contains("Phiên duyệt danh mục đã hết hạn"));
        assert!(html.contains("Tải lại từ đầu"));
    }

    #[test]
    fn the_form_mirrors_the_applied_search_and_can_always_clear_it() {
        let applied = DiscoveryQuery::new("lời hẹn", Some(WorkKind::Podcast), "").unwrap();
        let html = render_with(PublicDiscoveryState::default(), applied, Locale::Vietnamese);
        assert!(html.contains("value=\"lời hẹn\""), "{html}");
        assert!(
            html.contains("theatre-discovery-clear") && html.contains("Xóa tìm kiếm và bộ lọc")
        );
        let pressed: Vec<&str> = html
            .match_indices("aria-pressed=\"true\"")
            .map(|(i, _)| &html[i.saturating_sub(80)..i])
            .collect();
        assert_eq!(pressed.len(), 1);
        assert!(pressed[0].contains("data-kind=\"podcast\""), "{pressed:?}");
        let plain = render(PublicDiscoveryState::default(), Locale::Vietnamese);
        assert!(!plain.contains("theatre-discovery-clear"));
        for kind in ["all", "novel", "story", "podcast"] {
            assert!(plain.contains(&format!("data-kind=\"{kind}\"")), "{kind}");
        }
    }

    #[test]
    fn locale_projection_keeps_public_content_language_and_page_identity() {
        Owner::new().with(|| {
            let locale = RwSignal::new(Locale::Vietnamese);
            let initial = answered(
                DiscoveryQuery::default(),
                Ok(page(vec![work("Lời hẹn {time}")], Some("opaque-page"))),
            );
            let state = RwSignal::new(initial.clone());
            let applied = RwSignal::new(DiscoveryQuery::default());
            for (next, label, notice) in [
                (Locale::Vietnamese, "Đăng nhập", "Đang hiển thị 1 tác phẩm"),
                (Locale::English, "Sign in", "Showing 1 stories"),
            ] {
                locale.set(next);
                let html = view! { <PublicHome state locale applied on_search=Callback::new(|_| ())
                on_login=Callback::new(|()| ()) on_register=Callback::new(|()| ())
                on_retry=Callback::new(|()| ()) on_load_more=Callback::new(|()| ())
                on_restart=Callback::new(|()| ()) on_open_work=Callback::new(|_| ())/ > }
                .to_html();
                assert!(html.contains(label) && html.contains(notice), "{html}");
                assert!(html.contains("Lời hẹn {time}") && html.contains("lang=\"vi-VN\""));
                assert_eq!(state.get_untracked(), initial);
            }
        });
    }

    fn named_span<'a>(html: &'a str, id: &str) -> (&'a str, &'a str) {
        let id_at = html.find(&format!("id=\"{id}\"")).unwrap();
        let start = html[..id_at].rfind("<span").unwrap();
        let end = id_at + html[id_at..].find('>').unwrap();
        let content_end = end + 1 + html[end + 1..].find("</span>").unwrap();
        (&html[start..=end], &html[end + 1..content_end])
    }

    fn work_with(id: u8, title: &str) -> PresentedWorkSummary {
        PresentedWorkSummary {
            work_id: format!("00000000-0000-0000-0000-{id:012}"),
            ..work(title)
        }
    }

    #[test]
    fn every_work_has_one_compact_card_title_arrow_and_stable_named_link() {
        let state = answered(
            DiscoveryQuery::default(),
            Ok(page(
                vec![work_with(1, "Một"), work_with(2, "Hai"), work_with(3, "Ba")],
                None,
            )),
        );
        let html = render(state, Locale::Vietnamese);
        assert!(!html.contains("theatre-discover-feature"));
        assert_eq!(html.matches("<article").count(), 3);
        assert_eq!(html.matches("theatre-public-work-arrow").count(), 3);
        for (id, title) in [(1u8, "Một"), (2, "Hai"), (3, "Ba")] {
            let link = format!("id=\"theatre-public-work-link-00000000-0000-0000-0000-{id:012}\"");
            assert_eq!(html.matches(&link).count(), 1, "{link}");
            let action = format!("theatre-public-work-action-00000000-0000-0000-0000-{id:012}");
            let title_id = format!("theatre-public-work-title-00000000-0000-0000-0000-{id:012}");
            assert!(html.contains(&format!("aria-labelledby=\"{action} {title_id}\"")));
            let (tag, content) = named_span(&html, &title_id);
            assert!(tag.contains("class=\"theatre-public-work-title\""));
            assert!(tag.contains("lang=\"vi-VN\""));
            assert_eq!(content, title);
        }
        for card in html.split("<article").skip(1) {
            let card = card.split("</article>").next().unwrap();
            let title = card
                .split("<h3")
                .nth(1)
                .unwrap()
                .split("</h3>")
                .next()
                .unwrap();
            assert!(title.contains("theatre-public-work-arrow"));
            assert_eq!(card.matches("<a ").count(), 1);
        }
        for absent in ["Tuyển chọn", "Thịnh hành", "<img", "Nghe ", "<audio"] {
            assert!(!html.contains(absent), "{absent}");
        }
        let narrowed = DiscoveryQuery::new("Một", None, "").unwrap();
        let html = render_with(
            answered(narrowed.clone(), Ok(page(vec![work_with(1, "Một")], None))),
            narrowed,
            Locale::English,
        );
        assert_eq!(html.matches("<article").count(), 1);
        let (tag, content) = named_span(
            &html,
            "theatre-public-work-action-00000000-0000-0000-0000-000000000001",
        );
        assert!(tag.contains("class=\"cui-sr-only\"") && tag.contains("lang=\"en\""));
        assert_eq!(content, "Open story");
    }

    #[test]
    fn completed_empty_catalog_and_no_results_have_decorative_art_without_self_navigation() {
        for locale in Locale::ALL {
            let query = DiscoveryQuery::new("không khớp", None, "").unwrap();
            for applied in [DiscoveryQuery::default(), query] {
                let html = render_with(
                    answered(applied.clone(), Ok(page(vec![], None))),
                    applied,
                    locale,
                );
                assert_eq!(html.matches("data-artwork=\"works\"").count(), 1);
                assert!(html.contains("assets/empty/light/cantos-empty-works.svg"));
                assert!(html.contains("assets/empty/dark/cantos-empty-works.svg"));
                assert!(!html.contains("href=\"#discover\""));
                assert!(html.contains("id=\"theatre-home-refresh\""));
            }
            for state in [
                PublicDiscoveryState::default(),
                answered(
                    DiscoveryQuery::default(),
                    Err(DiscoveryReadError::Unavailable),
                ),
            ] {
                assert!(!render(state, locale).contains("data-artwork="));
            }
        }
    }

    #[test]
    fn work_action_uses_interface_language_and_only_the_visible_title_uses_content_language() {
        for (locale, content_language, title, action) in [
            (
                Locale::English,
                "vi-VN",
                "Ánh đèn cuối sân khấu",
                "Open story",
            ),
            (
                Locale::Vietnamese,
                "en",
                "The last light on stage",
                "Mở tác phẩm",
            ),
        ] {
            let mut story = work(title);
            story.language = content_language.to_owned();
            let html = render(
                answered(DiscoveryQuery::default(), Ok(page(vec![story], None))),
                locale,
            );
            let card = html
                .split("<article")
                .nth(1)
                .unwrap()
                .split("</article>")
                .next()
                .unwrap();
            let heading = card.split("<h3").nth(1).unwrap().split('>').next().unwrap();
            assert!(!heading.contains("lang="), "{heading}");
            let link = card.split("<a ").nth(1).unwrap().split('>').next().unwrap();
            assert!(
                link.contains(&format!("lang=\"{}\"", locale.lang())),
                "{link}"
            );
            assert!(
                !link.contains("aria-label="),
                "aria-label would erase mixed-language text: {link}"
            );
            let prefix_id = "theatre-public-work-action-00000000-0000-0000-0000-000000000001";
            let title_id = "theatre-public-work-title-00000000-0000-0000-0000-000000000001";
            assert!(
                link.contains(&format!("aria-labelledby=\"{prefix_id} {title_id}\"")),
                "{link}"
            );
            let (tag, content) = named_span(card, prefix_id);
            assert!(tag.contains("class=\"cui-sr-only\""), "{tag}");
            assert!(
                tag.contains(&format!("lang=\"{}\"", locale.lang())),
                "{tag}"
            );
            assert_eq!(content, action);
            let (tag, content) = named_span(card, title_id);
            assert!(tag.contains("class=\"theatre-public-work-title\""), "{tag}");
            assert!(
                tag.contains(&format!("lang=\"{content_language}\"")),
                "{tag}"
            );
            assert_eq!(content, title);
        }
    }

    #[test]
    fn discovery_has_no_content_language_control_and_keeps_work_language_metadata() {
        for locale in [Locale::Vietnamese, Locale::English] {
            let html = render(PublicDiscoveryState::default(), locale);
            for absent in [
                "theatre-discovery-language",
                "Ngôn ngữ nội dung",
                "Content language",
                "Mọi ngôn ngữ",
                "All languages",
            ] {
                assert!(!html.contains(absent), "{absent}");
            }
            // The one language choice left is the interface locale in the topbar (shell-owned).
            assert_eq!(html.matches("<select").count(), 0, "{html}");
        }
        // A Work still reads its own language by name and carries it as `lang`.
        let state = answered(DiscoveryQuery::default(), Ok(page(vec![work("Một")], None)));
        let html = render(state, Locale::English);
        assert!(
            html.contains("Tiếng Việt") && html.contains("lang=\"vi-VN\""),
            "{html}"
        );
    }

    #[test]
    fn search_is_a_floating_field_and_guest_actions_are_one_ranked_cluster() {
        let html = render(PublicDiscoveryState::default(), Locale::Vietnamese);
        assert!(html.contains("data-variant=\"floating\""), "{html}");
        assert_eq!(html.matches("id=\"theatre-discovery-submit\"").count(), 1);
        assert_eq!(html.matches("id=\"theatre-home-login\"").count(), 1);
        assert_eq!(html.matches("id=\"theatre-home-register\"").count(), 1);
        // Only the element's own opening tag, so one button's variant never vouches for another.
        let tag = |id: &str| {
            let at = html.find(&format!("id=\"{id}\"")).unwrap();
            let start = html[..at].rfind('<').unwrap();
            let end = at + html[at..].find('>').unwrap();
            html[start..=end].to_owned()
        };
        let register = tag("theatre-home-register");
        let login = tag("theatre-home-login");
        assert!(
            register.contains("data-variant=\"auth-primary\""),
            "{register}"
        );
        assert!(login.contains("data-variant=\"auth-secondary\""), "{login}");
        // One cluster, in the unchanged DOM (and tab) order: Sign in, then Create account.
        assert_eq!(html.matches("theatre-home-auth-actions").count(), 1);
        let cluster = html.find("theatre-home-auth-actions").unwrap();
        let login_at = html.find("id=\"theatre-home-login\"").unwrap();
        let register_at = html.find("id=\"theatre-home-register\"").unwrap();
        assert!(cluster < login_at && login_at < register_at);
        assert!(html.contains("Đăng nhập") && html.contains("Tạo tài khoản"));
    }

    #[test]
    fn an_outage_keeps_search_and_one_retry_in_the_same_shell_without_a_hero() {
        let state = answered(
            DiscoveryQuery::default(),
            Err(DiscoveryReadError::Unavailable),
        );
        let html = render(state, Locale::Vietnamese);
        assert_eq!(html.matches("id=\"theatre-home-retry\"").count(), 1);
        for present in [
            "theatre-discovery-search",
            "theatre-discovery-submit",
            "theatre-public-catalog",
        ] {
            assert!(html.contains(present), "{present}");
        }
        assert!(!html.contains("theatre-discover-feature") && !html.contains("<article"));
    }

    #[test]
    fn focus_after_a_page_follows_the_first_added_row_else_the_status() {
        let mut state = answered(
            DiscoveryQuery::default(),
            Ok(page(vec![work("A")], Some("c1"))),
        );
        assert_eq!(
            page_focus_target(&state),
            "theatre-public-work-link-00000000-0000-0000-0000-000000000001"
        );
        let more = state.load_more().unwrap();
        state.finish(&more, Ok(page(vec![], None)));
        assert_eq!(page_focus_target(&state), STATUS_ID);
    }
}
