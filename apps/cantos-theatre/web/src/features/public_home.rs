//! Public Home composition. Session lookup and auth are separate, explicit user intents.
//!
//! This is the Khám phá screen: the intro, the search/filter form and the public Work list. The
//! list is only a projection of [`PublicDiscoveryState`]; every search, retry and continuation is
//! a callback the shell turns into a request, so nothing here parses a cursor or fetches. The
//! intro's account slot reads the same mounted [`AccountViewState`] as the header avatar.
use super::loading::{PendingRows, RequestEdge};
use crate::contract::discovery::{CoverImage, MetadataWorkSummary, WorkKind};
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountViewState, SessionPhase},
        public_discovery::{
            kind_name, DiscoveryQuery, PublicDiscoveryFailure, PublicDiscoveryPhase,
            PublicDiscoveryState, QueryError, Tail,
        },
    },
    core::public_ids::PublicWorkId,
    features::{
        account::live_status_key,
        account_ui::{account_ui, session_tone, AccountUi},
        auth::SessionRecovery,
        auth_copy::{self, CopyKey},
        common::{icon, EmptyArtwork, EmptyArtworkKind},
        discovery_language,
        public_cover::PublicCover,
    },
    messages::{self, text, Key, Locale},
};
use cantos_ui::{
    AccessibleName, Button, ButtonState, ButtonVariant, DisabledReason, FilterChip, IconButton,
    IconName, LoadingState, TextField, TextFieldKind, TextFieldVariant,
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

/// What the displayed rows were narrowed by. An unfiltered catalogue has nothing to say: the
/// copy would only repeat itself. A kind alone has no text to name, so it reads as the kind.
fn displayed_context(state: &PublicDiscoveryState, locale: Locale) -> Option<String> {
    state
        .applied_query()
        .filter(|q| !q.is_unfiltered())
        .map(|q| {
            let kind = q
                .kind()
                .map_or(Key::DiscoveryKindAll, messages::work_kind_key);
            if q.text().is_empty() {
                return text(kind, locale).to_owned();
            }
            messages::message(
                locale,
                crate::localization::generated::Message::TheatreShellDiscoveryDisplayedContext {
                    arg_query: q.text(),
                    arg_kind: text(kind, locale),
                },
            )
        })
}

fn pending_chip(state: &PublicDiscoveryState, choice: Option<WorkKind>) -> bool {
    state.replacing()
        && !state.rows().is_empty()
        && state.query().kind() == choice
        && state
            .applied_query()
            .is_some_and(|applied| applied.kind() != choice)
}

const fn query_error_key(error: QueryError) -> Key {
    match error {
        QueryError::SearchTooLong => Key::DiscoverySearchTooLong,
        QueryError::SearchControlCharacter => Key::DiscoverySearchInvalid,
        QueryError::LanguageSyntax => Key::DiscoveryLanguageInvalid,
    }
}

/// What the intro's account slot shows. It is derived from the mounted account, never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HomeSession {
    /// A healthy guest: the one Sign in / Register cluster.
    GuestActions,
    /// API-confirmed: no guest intents and no extra text; the header avatar carries the account.
    SignedIn,
    /// Checking, failed or cleaning up: the owner's own status and permitted recovery. None of
    /// these is presented as a guest or as signed in.
    Status,
}

/// Only the phase the owner admits for sign-in is offered the guest intents, so a confirmed
/// session, a lookup in flight and an unresolved logout never show them.
fn home_session(ui: &AccountUi) -> HomeSession {
    match ui.phase {
        _ if ui.can_authenticate => HomeSession::GuestActions,
        SessionPhase::SignedIn => HomeSession::SignedIn,
        SessionPhase::Idle
        | SessionPhase::Checking
        | SessionPhase::Guest
        | SessionPhase::Failed(_)
        | SessionPhase::LogoutPending => HomeSession::Status,
    }
}

/// The intro's account slot (#538): the same authority as the header, projected for Discovery.
#[component]
fn HomeAccount(
    account: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
) -> impl IntoView {
    let ui = account_ui(account);
    let slot = Memo::new(move |_| home_session(&ui.get()));
    view! {
        {move || match slot.get() {
            // The one guest cluster: it sits beside the intro and wraps below it when narrow.
            // Register leads as the filled action; Sign in is the tonal alternative.
            HomeSession::GuestActions => view! {
                <div class="theatre-home-auth-actions">
                    <Button id="theatre-home-login" icon=IconName::Login variant=ButtonVariant::AuthSecondary
                        focus_on_press=true on_press=Callback::new(move |_| on_login.run(()))>
                        {move || text(Key::SignIn, locale.get())}
                    </Button>
                    <Button id="theatre-home-register" icon=IconName::PersonAdd variant=ButtonVariant::AuthPrimary
                        focus_on_press=true on_press=Callback::new(move |_| on_register.run(()))>
                        {move || auth_copy::text(CopyKey::RegisterAction, locale.get())}
                    </Button>
                </div>
            }.into_any(),
            HomeSession::SignedIn => ().into_any(),
            HomeSession::Status => view! {
                <div class="theatre-home-session">
                    <p id="theatre-home-session-status" class="theatre-auth-feedback"
                        data-tone=move || session_tone(ui.get().phase).as_str()
                        role="status" aria-live="polite" aria-atomic="true">
                        {move || text(live_status_key(&ui.get()), locale.get())}
                    </p>
                    <SessionRecovery state=account locale scope="theatre-home-session" on_refresh on_logout/>
                </div>
            }.into_any(),
        }}
    }
}

/// "Truyện · Tiếng Việt · Chương đã công bố: 12", the card's server-supplied facts.
fn feature_meta(kind: WorkKind, language: &str, count: u64, locale: Locale) -> String {
    let units = messages::public_unit_count(kind, locale, count);
    messages::message(
        locale,
        crate::localization::generated::Message::TheatreShellPublicHomeFeatureMeta {
            arg_kind: text(messages::work_kind_key(kind), locale),
            arg_language: language,
            arg_units: &(units),
        },
    )
}

/// The Work's authorized public cover over the textless 2:3 celestial fallback. The fallback
/// invents no story artwork and borrows no other Work's image; it stays for `cover: null` and
/// for any image that cannot be shown.
#[component]
fn WorkCover(kind: WorkKind, cover: Option<CoverImage>, lang: String) -> impl IntoView {
    view! {
        <PublicCover cover lang kind=kind_name(kind) class="theatre-public-cover"/>
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SearchKeyAction {
    Ignore,
    Collapse,
    FirstResult,
    LastResult,
    Submit,
}

fn search_key_action(key: &str, composing: bool, code: u32, text_field: bool) -> SearchKeyAction {
    if composing || code == 229 {
        return SearchKeyAction::Ignore;
    }
    match (key, text_field) {
        ("Escape", _) => SearchKeyAction::Collapse,
        ("ArrowDown", true) => SearchKeyAction::FirstResult,
        ("ArrowUp", true) => SearchKeyAction::LastResult,
        ("Enter", true) => SearchKeyAction::Submit,
        _ => SearchKeyAction::Ignore,
    }
}

/// Real currently applied Discovery data; editing does not invent autocomplete results.
#[component]
fn SearchDockResults(
    state: RwSignal<PublicDiscoveryState>,
    applied: RwSignal<DiscoveryQuery>,
    locale: RwSignal<Locale>,
    on_retry: Callback<()>,
    on_restart: Callback<()>,
    on_load_more: Callback<()>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    let current = Memo::new(move |_| state.with(|s| region(s.phase())));
    view! {
        <div class="theatre-search-dock-results" role="region" aria-label=move || text(Key::DiscoverySearchLabel, locale.get())>
            {move || match current.get() {
                Region::Loading => view! { <p>{move || text(Key::PublicHomeLoading, locale.get())}</p> }.into_any(),
                Region::Failed(failure) => {
                    let message = match failure {
                        PublicDiscoveryFailure::Diagnostic(kind) => crate::messages::error_key(kind),
                        PublicDiscoveryFailure::Unavailable => Key::PublicHomeUnavailable,
                        PublicDiscoveryFailure::Protocol => Key::PublicHomeProtocol,
                    };
                    view! {
                        <p role="status">{move || text(message, locale.get())}</p>
                        <Button id="theatre-search-dock-retry" on_press=Callback::new(move |_| on_retry.run(()))>{move || text(Key::PublicRetry, locale.get())}</Button>
                    }.into_any()
                },
                Region::Expired => view! {
                    <p role="status">{move || text(Key::DiscoveryExpired, locale.get())}</p>
                    <Button id="theatre-search-dock-restart" on_press=Callback::new(move |_| on_restart.run(()))>{move || text(Key::DiscoveryRestart, locale.get())}</Button>
                }.into_any(),
                Region::Empty => view! { <p role="status">{move || text(if applied.with(|q| q.is_unfiltered()) { Key::PublicHomeEmpty } else { Key::DiscoveryNoMatches }, locale.get())}</p> }.into_any(),
                Region::Incomplete | Region::Ready => view! {
                    {move || state.with(|s| match s.tail() {
                        Tail::Failed(PublicDiscoveryFailure::Diagnostic(kind)) => Some(text(crate::messages::error_key(kind), locale.get())),
                        Tail::Failed(PublicDiscoveryFailure::Unavailable) => Some(text(if s.replacement_failed() { Key::PublicHomeUnavailable } else { Key::DiscoveryPageUnavailable }, locale.get())),
                        Tail::Failed(PublicDiscoveryFailure::Protocol) => Some(text(if s.replacement_failed() { Key::PublicHomeProtocol } else { Key::DiscoveryPageProtocol }, locale.get())),
                        _ => None,
                    }).map(|message| view! { <p role="status">{message}</p> })}
                    <ul class="theatre-search-result-list">
                        <For each=move || state.with(|s| s.rows().iter().map(|w| w.work_id.clone()).collect::<Vec<_>>()) key=|id| id.clone() children=move |work_id| {
                            let id = PublicWorkId::parse(&work_id);
                            let row = Memo::new(move |_| state.with(|s| s.rows().iter().find(|w| w.work_id == work_id).map(|w| (w.title.clone(), w.kind, w.language.clone()))));
                            view! { <li><button type="button" on:click=move |_| { if let Some(id) = id { on_open_work.run(id); } }>
                                <span class="theatre-search-result-title" lang=move || row.get().map(|(_, _, lang)| lang)>{move || row.get().map(|(title, _, _)| title)}</span>
                                <span class="theatre-search-result-kind">{move || row.get().map(|(_, kind, _)| text(messages::work_kind_key(kind), locale.get()))}</span>
                            </button></li> }
                        }/>
                    </ul>
                    <Show when=move || state.with(|s| region(s.phase())) == Region::Incomplete>
                        <p role="status">{move || text(Key::DiscoveryIncomplete, locale.get())}</p>
                        <Button id="theatre-search-dock-more" on_press=Callback::new(move |_| on_load_more.run(()))>{move || text(Key::DiscoveryKeepSearching, locale.get())}</Button>
                    </Show>
                }.into_any(),
            }}
        </div>
    }
}

#[component]
fn SearchForm(
    applied: RwSignal<DiscoveryQuery>,
    locale: RwSignal<Locale>,
    on_search: Callback<DiscoveryQuery>,
    state: RwSignal<PublicDiscoveryState>,
    on_retry: Callback<()>,
    on_restart: Callback<()>,
    on_load_more: Callback<()>,
    on_open_work: Callback<PublicWorkId>,
    children: Children,
) -> impl IntoView {
    let panel = NodeRef::<leptos::html::Div>::new();
    let expanded = RwSignal::new(false);
    let composing = RwSignal::new(false);
    let restoring = RwSignal::new(false);
    crate::platform::bind_search_dock(panel, expanded);
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
    let submit = move || {
        if composing.get_untracked() {
            return;
        }
        expanded.set(true);
        match DiscoveryQuery::new(&search.get_untracked(), kind.get_untracked(), "") {
            Ok(query) => {
                error.set(None);
                on_search.run(query);
            }
            Err(refused) => error.set(Some(refused)),
        }
    };
    let search_error = Signal::derive(move || {
        error
            .get()
            .map(|e| text(query_error_key(e), locale.get()).to_owned())
    });
    let filtered = move || !applied.get().is_unfiltered();
    view! {
        <div node_ref=panel class="theatre-public-search"
            on:focusin=move |event| { if !restoring.get_untracked() && crate::platform::focus_is_search_input(&event) { expanded.set(true); } }
            on:focusout=move |event| { if crate::platform::focus_left_search_field(&event) { expanded.set(false); } }
            // Observe input clicks before a dock result can navigate and dispose this panel.
            // A delegated bubble handler would be visited after its callback was dropped.
            on:click:capture=move |event| { if crate::platform::click_is_search_input(&event) { expanded.set(true); } }
            on:keydown=move |event| {
                // Enter in a text field applies the search, except while an input method is
                // composing. On a chip or button it keeps its own activation.
                match search_key_action(&event.key(), event.is_composing() || composing.get_untracked(), event.key_code(), from_text_field(&event)) {
                    SearchKeyAction::Ignore => {},
                    SearchKeyAction::Collapse => {
                        event.prevent_default(); restoring.set(true);
                        crate::platform::focus_search_input(panel);
                        expanded.set(false); restoring.set(false);
                    }
                    SearchKeyAction::FirstResult | SearchKeyAction::LastResult => {
                        event.prevent_default(); expanded.set(true);
                        crate::platform::focus_search_result(panel, event.key() == "ArrowUp");
                    }
                    SearchKeyAction::Submit => { event.prevent_default(); submit(); }
                }
            }>
            <div class="theatre-discovery-search-hero">
                {children()}
                <div class="theatre-public-search-main" role="search">
                    <TextField id="theatre-discovery-search" kind=TextFieldKind::Search
                        variant=TextFieldVariant::Floating
                        label=Signal::derive(move || text(Key::DiscoverySearchLabel, locale.get()).to_owned())
                        value=search on_input=Callback::new(move |value| { search.set(value); expanded.set(true); })
                        on_composition_start=Callback::new(move |_| composing.set(true))
                        on_composition_end=Callback::new(move |_| composing.set(false))
                        dock=cantos_ui::SearchDock { id: "theatre-search-dock".into(), expanded: expanded.into(),
                            content: (move || view! {
                                // Closing is the field's own behaviour (Escape, an outside press,
                                // focus leaving it): no control here. An unfiltered catalogue has no
                                // context, so no element is rendered for it.
                                {move || state.with(|s| displayed_context(s, locale.get()))
                                    .map(|context| view! { <p class="theatre-search-dock-context">{context}</p> })}
                                <SearchDockResults state applied locale on_retry on_restart on_load_more on_open_work/>
                            }).into() }

                        error=search_error autocomplete="off"
                        trailing=move || view! {
                            <span class="theatre-search-clear-slot">
                            <Show when=move || !search.get().is_empty()>
                                {move || AccessibleName::new(text(Key::SearchClearInput, locale.get())).ok().map(|name| view! {
                                    <IconButton id="theatre-search-clear-input" variant=ButtonVariant::Text icon=IconName::Close name
                                        on_press=Callback::new(move |_| {
                                            search.set(String::new()); error.set(None); expanded.set(true);
                                            crate::platform::focus_search_input(panel);
                                        })/>
                                })}
                            </Show>
                            </span>
                            // The one magnifier (Search v4.3): an icon-only action whose localized
                            // name is its accessible text; Enter and this press share `submit`.
                            {move || AccessibleName::new(text(Key::DiscoverySearchSubmit, locale.get())).ok().map(|name| view! {
                                <IconButton id="theatre-discovery-submit" variant=ButtonVariant::Primary
                                    icon=IconName::Search name
                                    on_press=Callback::new(move |_| submit())/>
                            })}
                        }/>
                </div>
            </div>
            <RequestEdge active=Signal::derive(move || state.with(|s| s.replacing() && !s.rows().is_empty()
                && !KINDS.into_iter().any(|choice| pending_chip(s, choice)))) label=Key::PublicHomeLoading locale/>
            <div class="theatre-public-search-filters">
                <div class="theatre-filters" role="group" aria-label=move || text(Key::DiscoveryKindLabel, locale.get())>
                    {KINDS.into_iter().map(|choice| view! {
                        <FilterChip id=format!("theatre-filter-{}", choice.map_or("all", kind_name))
                            selected=Signal::derive(move || state.with(|s| s.applied_query().is_some_and(|q| q.kind() == choice)))
                            state=Signal::derive(move || {
                                if state.with(|s| pending_chip(s, choice)) {
                                    DisabledReason::new(text(Key::PublicHomeLoading, locale.get())).ok().map(ButtonState::Loading).unwrap_or_default()
                                } else { ButtonState::Enabled }
                            }) on_change=Callback::new(move |_| { kind.set(choice); submit(); })>
                            {move || text(choice.map_or(Key::DiscoveryKindAll, messages::work_kind_key), locale.get())}
                        </FilterChip>
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
    work: MetadataWorkSummary,
    state: RwSignal<PublicDiscoveryState>,
    locale: RwSignal<Locale>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    let identity = work.work_id.clone();
    let lookup_id = identity.clone();
    let data = Memo::new(move |_| {
        state.with(|s| s.rows().iter().find(|w| w.work_id == lookup_id).cloned())
    });
    // Retaining a keyed row retains its link/focus, not a stale cover authorization or metadata.
    let cover = Memo::new(move |_| {
        data.with(|w| {
            w.as_ref()
                .map(|w| (w.kind, w.cover.clone(), w.language.clone()))
        })
    });
    let action_id = format!("theatre-public-work-action-{identity}");
    let title_id = format!("theatre-public-work-title-{identity}");
    let labelled_by = format!("{action_id} {title_id}");
    let arrow = icon("arrow_back");
    let language =
        move || data.with(|w| w.as_ref().map(|w| w.language.clone()).unwrap_or_default());
    let title = move || data.with(|w| w.as_ref().map(|w| w.title.clone()).unwrap_or_default());
    view! {
        <article class="theatre-public-work theatre-work-card theatre-panel" data-work-id=identity.clone()>
            {move || cover.get().map(|(kind, cover, lang)| view! { <WorkCover kind cover lang/> })}
            <div class="theatre-public-work-body">
                <p class="theatre-eyebrow">{move || data.with(|w| w.as_ref().map(|w| text(messages::work_kind_key(w.kind), locale.get())))}</p>
                <h3>{match PublicWorkId::parse(&identity) {
                    Some(id) => view! {
                        <a id=work_link_id(id) class="theatre-public-work-link" href=Route::PublicWork(id).hash()
                            lang=move || locale.get().lang() aria-labelledby=labelled_by
                            on:click=move |event| if in_app_activation(&event) { event.prevent_default(); on_open_work.run(id); }>
                            <span id=action_id class="cui-sr-only" lang=move || locale.get().lang()>{move || text(Key::PublicHomeOpenWork, locale.get())}</span>
                            <span id=title_id class="theatre-public-work-title" lang=language>{title}</span>
                            <span class="theatre-public-work-arrow" aria-hidden="true">{arrow}</span>
                        </a>
                    }.into_any(),
                    None => view! { <span lang=language>{title}</span> }.into_any(),
                }}</h3>
                <p lang=language class="theatre-muted theatre-public-work-description">{move || data.with(|w| w.as_ref().map(|w| w.description.clone()).unwrap_or_default())}</p>
                <p class="theatre-muted theatre-public-work-count">{move || data.with(|w| w.as_ref().map(|w| feature_meta(w.kind, &discovery_language::display_name(&w.language), w.unit_count, locale.get())))}</p>
            </div>
        </article>
    }
}

#[component]
pub(crate) fn PublicHome(
    state: RwSignal<PublicDiscoveryState>,
    /// The mounted session owner, read-only: it decides whether guest intents may appear.
    account: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    /// The search the link carries; the fields mirror it and a submit asks the shell to apply one.
    applied: RwSignal<DiscoveryQuery>,
    on_search: Callback<DiscoveryQuery>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    /// Re-read the session after a failed lookup; the owner refuses it while a hold applies.
    on_refresh: Callback<()>,
    /// Retry an unconfirmed logout, offered only while the owner admits it.
    on_logout: Callback<()>,
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
    let status_line = move || {
        if state.with(|s| s.appending()) {
            text(Key::DiscoveryLoadingMore, locale.get()).to_owned()
        } else if state
            .with(|s| s.replacing() && !KINDS.into_iter().any(|choice| pending_chip(s, choice)))
        {
            text(Key::PublicHomeLoading, locale.get()).to_owned()
        } else {
            match tail.get() {
                Tail::End => messages::message(
                    locale.get(),
                    crate::localization::generated::Message::TheatreShellDiscoveryComplete {
                        arg_count: (count()) as u64,
                    },
                ),
                Tail::More | Tail::Loading => messages::message(
                    locale.get(),
                    crate::localization::generated::Message::TheatreShellDiscoveryShowing {
                        arg_count: (count()) as u64,
                    },
                ),
                Tail::Failed(PublicDiscoveryFailure::Diagnostic(kind)) => {
                    text(crate::messages::error_key(kind), locale.get()).to_owned()
                }
                Tail::Failed(PublicDiscoveryFailure::Unavailable) => text(
                    if state.with(|s| s.replacement_failed()) {
                        Key::PublicHomeUnavailable
                    } else {
                        Key::DiscoveryPageUnavailable
                    },
                    locale.get(),
                )
                .to_owned(),
                Tail::Failed(PublicDiscoveryFailure::Protocol) => text(
                    if state.with(|s| s.replacement_failed()) {
                        Key::PublicHomeProtocol
                    } else {
                        Key::DiscoveryPageProtocol
                    },
                    locale.get(),
                )
                .to_owned(),
                Tail::Expired => text(Key::DiscoveryExpired, locale.get()).to_owned(),
            }
        }
    };
    let more_state = Signal::derive(move || match tail.get() {
        Tail::Loading => DisabledReason::new(text(
            if state.with(|s| s.appending()) {
                Key::DiscoveryLoadingMore
            } else {
                Key::PublicHomeLoading
            },
            locale.get(),
        ))
        .map(ButtonState::Disabled)
        .unwrap_or_default(),
        _ => ButtonState::Enabled,
    });
    let more_label = move || match tail.get() {
        Tail::Failed(_) if state.with(|s| s.replacement_failed()) => Key::PublicRetry,
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
            <SearchForm applied locale on_search state on_retry on_restart on_load_more on_open_work>
            <header class="theatre-discover-intro">
                <div class="theatre-discover-intro-copy">
                    <p class="theatre-eyebrow">{move || text(Key::PublicHomeEyebrow, locale.get())}</p>
                    <h1 id="theatre-public-home-heading">{move || text(Key::HomeHeading, locale.get())}</h1>
                    <p class="theatre-muted">{move || text(Key::PublicHomeIntroduction, locale.get())}</p>
                </div>
                <HomeAccount account locale on_login on_register on_refresh on_logout/>
            </header>
            </SearchForm>
            <section class="theatre-public-catalog" aria-labelledby="theatre-public-shelf-heading">
                <h2 id="theatre-public-shelf-heading">{move || text(Key::StoryShelf, locale.get())}</h2>
                {move || match current.get() {
                    Region::Loading => view! {
                        <p class="theatre-public-state" role="status">{move || text(Key::PublicHomeLoading, locale.get())}</p>
                        <PendingRows/>
                    }.into_any(),
                    Region::Ready => view! {
                        <div class="theatre-shelf theatre-public-work-grid" aria-busy=move || state.with(|s| s.replacing() || s.appending()).to_string()>
                            <For each=rows key=|work| work.work_id.clone()
                                children=move |work| view! { <WorkCard work state locale on_open_work/> }/>
                        </div>
                        {move || state.with(|s| displayed_context(s, locale.get())).map(|context| view! { <p class="theatre-muted theatre-displayed-context">{context}</p> })}
                        <p id=STATUS_ID class="theatre-muted theatre-public-status" role="status" tabindex="-1">{status_line}</p>
                        <Show when=move || state.with(|s| s.appending())>
                            <LoadingState>{move || text(Key::DiscoveryLoadingMore, locale.get())}</LoadingState>
                        </Show>
                        <Show when=move || !matches!(tail.get(), Tail::End)>
                            <Button id=MORE_ID variant=ButtonVariant::Tonal state=more_state on_press=more_press>
                                {move || text(more_label(), locale.get())}
                            </Button>
                        </Show>
                    }.into_any(),
                    Region::Empty => {
                        let unfiltered = state.with(|s| s.applied_query().is_none_or(|q| q.is_unfiltered()));
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
    use crate::application::account_view::AccountFailure;
    use crate::application::public_discovery::DiscoveryReadError;
    use crate::contract::discovery::MetadataWorkPage;
    use crate::features::auth_preview::{guest, logout_pending, signed_in, PreviewActor};

    #[test]
    fn search_keyboard_never_commits_or_dismisses_an_active_ime_and_native_buttons_keep_enter() {
        for key in ["Enter", "Escape", "ArrowDown", "ArrowUp"] {
            assert_eq!(
                search_key_action(key, true, 13, true),
                SearchKeyAction::Ignore
            );
            assert_eq!(
                search_key_action(key, false, 229, true),
                SearchKeyAction::Ignore
            );
        }
        assert_eq!(
            search_key_action("Enter", false, 13, true),
            SearchKeyAction::Submit
        );
        assert_eq!(
            search_key_action("Enter", false, 13, false),
            SearchKeyAction::Ignore
        );
        assert_eq!(
            search_key_action("Escape", false, 27, false),
            SearchKeyAction::Collapse
        );
        assert_eq!(
            search_key_action("Tab", false, 9, true),
            SearchKeyAction::Ignore
        );
    }

    fn guest_account() -> RwSignal<AccountViewState> {
        let account = RwSignal::new(AccountViewState::default());
        guest(account).unwrap();
        account
    }

    fn home(
        state: RwSignal<PublicDiscoveryState>,
        account: RwSignal<AccountViewState>,
        locale: RwSignal<Locale>,
        applied: RwSignal<DiscoveryQuery>,
    ) -> String {
        view! {
            <PublicHome state account=account.read_only() locale applied on_search=Callback::new(|_| ())
                on_login=Callback::new(|()| ()) on_register=Callback::new(|()| ())
                on_refresh=Callback::new(|()| ()) on_logout=Callback::new(|()| ())
                on_retry=Callback::new(|()| ()) on_load_more=Callback::new(|()| ())
                on_restart=Callback::new(|()| ()) on_open_work=Callback::new(|_| ()) />
        }
        .to_html()
    }

    fn render_with(state: PublicDiscoveryState, applied: DiscoveryQuery, locale: Locale) -> String {
        Owner::new().with(|| {
            home(
                RwSignal::new(state),
                guest_account(),
                RwSignal::new(locale),
                RwSignal::new(applied),
            )
        })
    }
    fn render(state: PublicDiscoveryState, locale: Locale) -> String {
        render_with(state, DiscoveryQuery::default(), locale)
    }
    fn work(title: &str) -> MetadataWorkSummary {
        MetadataWorkSummary {
            work_id: "00000000-0000-0000-0000-000000000001".into(),
            kind: WorkKind::Story,
            title: title.into(),
            description: "Ngày mai, mình có diễn tiếp không?".into(),
            language: "vi-VN".into(),
            first_visible_at_ms: 1,
            unit_count: 3,
            cover: None,
        }
    }
    fn answered(
        query: DiscoveryQuery,
        result: Result<MetadataWorkPage, DiscoveryReadError>,
    ) -> PublicDiscoveryState {
        let mut state = PublicDiscoveryState::default();
        let ticket = state.begin(query).unwrap();
        state.finish(&ticket, result);
        state
    }
    fn page(items: Vec<MetadataWorkSummary>, next: Option<&str>) -> MetadataWorkPage {
        MetadataWorkPage {
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
            "theatre-cover-art",
            "<audio",
            "publication_id",
            "opaque-page",
        ] {
            assert!(!html.contains(absent), "{absent}");
        }
        assert!(html.contains("theatre-cover-fallback-art"), "{html}");
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
    fn the_search_layer_repeats_nothing_and_leaves_no_empty_line_for_an_unfiltered_catalogue() {
        let state = answered(
            DiscoveryQuery::default(),
            Ok(page(vec![work("Ánh đèn cuối sân khấu")], None)),
        );
        for locale in [Locale::Vietnamese, Locale::English] {
            let html = render(state.clone(), locale);
            for absent in [
                "Kết quả đang hiển thị",
                "Displayed results",
                "Tất cả loại · Tất cả loại",
                "All types · All types",
                "<p></p>",
                "theatre-search-dock-heading",
                "theatre-search-dock-context",
            ] {
                assert!(!html.contains(absent), "{absent}: {html}");
            }
            assert!(html.contains("theatre-search-result-title"), "{html}");
        }
    }

    #[test]
    fn the_search_layer_has_no_collapse_control_in_any_state_or_locale() {
        let narrowed = DiscoveryQuery::new("đèn", Some(WorkKind::Story), "").unwrap();
        let narrowed_state = answered(narrowed.clone(), Ok(page(vec![work("Ánh đèn")], None)));
        let unfiltered = answered(
            DiscoveryQuery::default(),
            Ok(page(vec![work("Ánh đèn cuối sân khấu")], None)),
        );
        for locale in Locale::supported() {
            for html in [
                render(PublicDiscoveryState::default(), locale),
                render(unfiltered.clone(), locale),
                render_with(narrowed_state.clone(), narrowed.clone(), locale),
            ] {
                // Escape, an outside press and focus leaving the field close the layer; a text
                // button would only spend its first row.
                assert!(!html.contains("theatre-search-collapse"), "{html}");
                assert!(!html.contains(text(Key::SearchCollapse, locale)), "{html}");
            }
        }
    }

    #[test]
    fn a_kind_filter_alone_reads_as_the_kind_and_a_text_keeps_its_one_line() {
        let narrowed = DiscoveryQuery::new("", Some(WorkKind::Podcast), "").unwrap();
        let state = answered(narrowed.clone(), Ok(page(vec![work("Đêm khuya")], None)));
        let en = render_with(state, narrowed, Locale::English);
        assert!(
            en.contains("theatre-search-dock-context\">Podcast</p>"),
            "{en}"
        );
        assert!(!en.contains("All types · All types"), "{en}");
        let typed = DiscoveryQuery::new("đèn", None, "").unwrap();
        let state = answered(typed.clone(), Ok(page(vec![work("Ánh đèn")], None)));
        let vi = render_with(state, typed, Locale::Vietnamese);
        assert!(
            vi.contains("Kết quả đang hiển thị: đèn · Tất cả loại"),
            "{vi}"
        );
    }

    #[test]
    fn public_home_renders_each_typed_state_in_both_locales() {
        let filtered = DiscoveryQuery::new("đèn", None, "").unwrap();
        for (locale, loading, empty, no_match, unavailable, protocol, retry, refresh) in [
            (
                Locale::Vietnamese,
                "Đang mở những câu chuyện đã công bố…",
                "Chưa có câu chuyện nào",
                "Không có tác phẩm nào khớp.",
                "Chưa mở được danh mục công khai.",
                "Chưa đọc được danh mục công khai.",
                "Thử lại",
                "Cập nhật danh mục",
            ),
            (
                Locale::English,
                "Opening published stories…",
                "No stories yet",
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
            assert!(html.contains(empty) && !html.contains(refresh), "{html}");
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
        let html = render_with(
            answered(applied.clone(), Ok(page(vec![], None))),
            applied,
            Locale::Vietnamese,
        );
        assert!(html.contains("value=\"lời hẹn\""), "{html}");
        assert!(
            html.contains("theatre-discovery-clear") && html.contains("Xóa tìm kiếm và bộ lọc")
        );
        let pressed: Vec<&str> = html
            .match_indices("aria-pressed=\"true\"")
            .map(|(i, _)| &html[i.saturating_sub(80)..i])
            .collect();
        assert_eq!(pressed.len(), 1);
        assert!(pressed[0].contains("theatre-filter-podcast"), "{pressed:?}");
        let plain = render(PublicDiscoveryState::default(), Locale::Vietnamese);
        assert!(!plain.contains("theatre-discovery-clear"));
        for kind in ["all", "novel", "story", "podcast"] {
            assert!(plain.contains(&format!("theatre-filter-{kind}")), "{kind}");
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
            let account = guest_account();
            for (next, label, notice) in [
                (Locale::Vietnamese, "Đăng nhập", "Đang hiển thị 1 tác phẩm"),
                (Locale::English, "Sign in", "Showing 1 stories"),
            ] {
                locale.set(next);
                let html = home(state, account, locale, applied);
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

    fn work_with(id: u8, title: &str) -> MetadataWorkSummary {
        MetadataWorkSummary {
            work_id: format!("00000000-0000-0000-0000-{id:012}"),
            ..work(title)
        }
    }

    #[test]
    fn a_card_shows_its_own_authorized_cover_and_a_null_cover_keeps_the_kind_fallback() {
        let covered: MetadataWorkPage = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/covered_works.json"
        ))
        .unwrap();
        let cover = covered.items[0].cover.clone().unwrap();
        let state = answered(DiscoveryQuery::default(), Ok(covered));
        let html = render(state, Locale::Vietnamese);
        assert_eq!(
            html.matches("class=\"theatre-cover-art\"").count(),
            1,
            "{html}"
        );
        assert_eq!(
            html.matches("class=\"theatre-cover-fallback-art\"").count(),
            1,
            "{html}"
        );
        assert!(html.contains(&format!("src=\"{}\"", cover.url)), "{html}");
        assert!(
            html.contains(&format!("alt=\"{}\"", cover.alt_text)),
            "{html}"
        );
        assert_eq!(html.matches("data-cover=\"loading\"").count(), 1, "{html}");
        assert_eq!(html.matches("data-cover=\"none\"").count(), 1, "{html}");
        // The covered card's image is not the link: navigation stays on the one named title link,
        // and the card without a cover shows the textless celestial AVIF fallback.
        assert!(html.matches("theatre-public-work-link").count() >= 2);
        assert!(
            html.contains("assets/artwork/cantos-cover-fallback.avif"),
            "{html}"
        );
        assert!(!html.contains("theatre-public-cover-glyph"), "{html}");
        assert!(!html.contains("theatre-cover-moon"), "{html}");
        // An image never carries another card's text or a private field.
        for private in ["asset_id", "rights", "provenance", "storage_key"] {
            assert!(!html.contains(private), "{private}");
        }
    }

    #[test]
    fn a_page_without_any_cover_renders_no_image_and_never_mock_artwork() {
        let state = answered(
            DiscoveryQuery::default(),
            Ok(page(vec![work_with(1, "Một"), work_with(2, "Hai")], None)),
        );
        let html = render(state, Locale::Vietnamese);
        assert!(!html.contains("theatre-cover-art"), "{html}");
        assert_eq!(
            html.matches("class=\"theatre-cover-fallback-art\"").count(),
            2,
            "{html}"
        );
        assert!(
            !html.contains("son-ha-ky.webp") && !html.contains("den-khuya.webp"),
            "{html}"
        );
        assert_eq!(html.matches("data-cover=\"none\"").count(), 2, "{html}");
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
        for absent in [
            "Tuyển chọn",
            "Thịnh hành",
            "theatre-cover-art",
            "Nghe ",
            "<audio",
        ] {
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
        for locale in Locale::supported() {
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
                // A confirmed-empty catalog has nothing to refresh: no control, no copy that names one.
                assert!(
                    !html.contains("theatre-home-refresh") && !html.contains("Refresh catalog")
                );
                assert!(!html.contains("Cập nhật danh mục") && !html.contains("cập nhật sau"));
                assert!(!html.contains("refresh later"));
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

    /// The completed-empty and no-results regions are one centered visual group; the other
    /// regions never borrow the empty composition.
    /// The hero eyebrow is gold through the dedicated hero-accent role. A blanket `inherit` on it
    /// would return it to the headline's color, and a hex literal would bypass the token owner.
    #[test]
    fn hero_eyebrow_uses_the_hero_accent_role_and_never_inherits_the_headline_color() {
        let css = include_str!("../styles/theatre.css");
        let rule = |selector: &str| {
            css.split(&format!("{selector} {{"))
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing rule {selector}"))
        };
        let eyebrow = rule(".theatre-discovery-search-hero .theatre-eyebrow");
        assert_eq!(
            eyebrow.trim(),
            "color: var(--cantos-color-theatre-hero-accent);"
        );
        assert!(!css
            .contains(".theatre-discovery-search-hero :where(.theatre-muted, .theatre-eyebrow)"));
        // Supporting copy keeps the hero's high-contrast ink; hierarchy comes from size and weight.
        assert!(rule(".theatre-discovery-search-hero .theatre-muted").contains("color: inherit"));
        assert!(rule(".theatre-auth-story-copy .theatre-eyebrow")
            .contains("var(--cantos-color-theatre-hero-accent)"));
        // The auth art panel is the hero surface its Light/Dark Symbol tone is paired with.
        assert!(rule(".theatre-auth-art").contains("background: var(--cantos-color-theatre-hero)"));
        assert!(!rule(".theatre-auth-art").contains("primary-container"));
    }

    #[test]
    fn empty_regions_render_one_group_that_the_stylesheet_centers_without_a_fixed_height() {
        let css = include_str!("../styles/theatre.css");
        let rule = |selector: &str| {
            css.split(&format!("{selector} {{"))
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing rule {selector}"))
        };
        let group = rule(".theatre-public-empty");
        for declaration in [
            "display: flex",
            "flex-direction: column",
            "align-items: center",
            "justify-content: center",
            "text-align: center",
        ] {
            assert!(group.contains(declaration), "{declaration}");
        }
        assert!(!group.contains("block-size") && !group.contains("height"));
        // The shared artwork basis (16rem) would otherwise become a column height.
        assert!(rule(".theatre-public-empty > .theatre-empty-artwork").contains("flex: 0 0 auto"));
        let query = DiscoveryQuery::new("không khớp", None, "").unwrap();
        for applied in [DiscoveryQuery::default(), query] {
            let html = render_with(
                answered(applied.clone(), Ok(page(vec![], None))),
                applied,
                Locale::Vietnamese,
            );
            let start = html.find("theatre-public-empty").unwrap();
            let group = &html[start..];
            let end = group.find("</div></div>").unwrap_or(group.len());
            let group = &group[..end];
            for part in ["data-artwork=\"works\"", "<h3>", "<p>"] {
                assert_eq!(group.matches(part).count(), 1, "{part}");
            }
        }
        for state in [
            PublicDiscoveryState::default(),
            answered(
                DiscoveryQuery::default(),
                Err(DiscoveryReadError::Unavailable),
            ),
        ] {
            assert!(!render(state, Locale::Vietnamese).contains("theatre-public-empty"));
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
        // One cluster, in the unchanged DOM (and tab) order: Sign in, then Register.
        assert_eq!(html.matches("theatre-home-auth-actions").count(), 1);
        let cluster = html.find("theatre-home-auth-actions").unwrap();
        let login_at = html.find("id=\"theatre-home-login\"").unwrap();
        let register_at = html.find("id=\"theatre-home-register\"").unwrap();
        assert!(cluster < login_at && login_at < register_at);
        assert!(html.contains("Đăng nhập") && html.contains("Đăng ký"));
    }

    #[test]
    fn bounded_hero_keeps_one_docked_visible_search_action_and_filters_before_results() {
        for locale in Locale::supported() {
            let html = render(PublicDiscoveryState::default(), locale);
            let hero = html.find("theatre-discovery-search-hero").unwrap();
            let intro = html.find("theatre-discover-intro").unwrap();
            let search = html.find("id=\"theatre-discovery-search\"").unwrap();
            let trailing = html.find("class=\"cui-field-trailing\"").unwrap();
            let submit = html.find("id=\"theatre-discovery-submit\"").unwrap();
            let filters = html.find("theatre-public-search-filters").unwrap();
            let results = html.find("class=\"theatre-public-catalog\"").unwrap();
            assert!(hero < intro && intro < search && search < trailing && trailing < submit);
            assert!(submit < filters && filters < results);
            assert_eq!(html.matches("role=\"search\"").count(), 1);
            assert_eq!(html.matches("id=\"theatre-discovery-search\"").count(), 1);
            assert_eq!(html.matches("id=\"theatre-discovery-submit\"").count(), 1);
            assert!(html.contains("data-trailing=\"true\""));
            // One icon-only magnifier: its localized name is screen-reader text, never a caption,
            // and no arrow, voice or microphone action exists beside it.
            let button = html[submit..].split("</button>").next().unwrap();
            assert!(button.contains(text(Key::DiscoverySearchSubmit, locale)));
            assert!(button.contains("cui-sr-only"));
            assert_eq!(html.matches("cui-field-trailing").count(), 1);
            for forbidden in ["arrow_forward", "mic", "voice"] {
                assert!(
                    !html.contains(&format!("name=\"{forbidden}\"")),
                    "{forbidden}"
                );
            }
        }
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

    const GUEST_INTENTS: [&str; 3] = [
        "theatre-home-auth-actions",
        "id=\"theatre-home-login\"",
        "id=\"theatre-home-register\"",
    ];

    fn ui(phase: SessionPhase, can_authenticate: bool) -> AccountUi {
        AccountUi {
            phase,
            can_authenticate,
            can_retry_logout: false,
            cleanup_in_flight: false,
            login_refusal: None,
        }
    }

    /// Exhaustive over the six phases: only the owner's sign-in admission yields guest intents.
    #[test]
    fn only_an_admitted_guest_is_offered_sign_in_and_every_other_phase_keeps_its_own_status() {
        assert_eq!(
            home_session(&ui(SessionPhase::Guest, true)),
            HomeSession::GuestActions
        );
        assert_eq!(
            home_session(&ui(SessionPhase::SignedIn, false)),
            HomeSession::SignedIn
        );
        for phase in [
            SessionPhase::Idle,
            SessionPhase::Checking,
            // A guest whose own logout cleanup is still unresolved is not a healthy guest.
            SessionPhase::Guest,
            SessionPhase::Failed(AccountFailure::Unavailable),
            SessionPhase::Failed(AccountFailure::Protocol),
            SessionPhase::LogoutPending,
        ] {
            assert_eq!(
                home_session(&ui(phase, false)),
                HomeSession::Status,
                "{phase:?}"
            );
        }
    }

    /// The regression in #538: a signed-in listener still saw Sign in / Register on Discovery.
    #[test]
    fn signed_in_discovery_renders_no_guest_intents_and_no_session_status() {
        Owner::new().with(|| {
            let account = RwSignal::new(AccountViewState::default());
            signed_in(account, PreviewActor::A).unwrap();
            for locale in Locale::supported() {
                let html = home(
                    RwSignal::new(PublicDiscoveryState::default()),
                    account,
                    RwSignal::new(locale),
                    RwSignal::new(DiscoveryQuery::default()),
                );
                for absent in GUEST_INTENTS.into_iter().chain([
                    "theatre-home-session-status",
                    "theatre-home-session-refresh",
                ]) {
                    assert!(!html.contains(absent), "{absent}: {html}");
                }
                // The public catalog and search stay exactly as for a guest.
                assert!(html.contains("id=\"theatre-discovery-search\""));
                assert!(html.contains("theatre-public-catalog"));
            }
        });
    }

    #[test]
    fn checking_failed_and_logout_pending_show_the_owner_status_never_guest_intents() {
        Owner::new().with(|| {
            let locale = RwSignal::new(Locale::Vietnamese);
            let account = RwSignal::new(AccountViewState::default());
            let render = || {
                home(
                    RwSignal::new(PublicDiscoveryState::default()),
                    account,
                    locale,
                    RwSignal::new(DiscoveryQuery::default()),
                )
            };
            // Before the first lookup settles, nothing claims guest or signed in.
            let issued = account.write().begin_refresh().unwrap();
            let html = render();
            assert!(html.contains("Đang kiểm tra phiên người nghe…"), "{html}");
            assert!(html.contains("data-tone=\"info\""), "{html}");
            for absent in GUEST_INTENTS {
                assert!(!html.contains(absent), "{absent}: {html}");
            }
            // An unavailable lookup is not a sign-out: its failure and Retry stay visible.
            account
                .write()
                .finish_session(issued, Err(AccountFailure::Unavailable), String::new)
                .unwrap();
            let html = render();
            assert!(
                html.contains("id=\"theatre-home-session-status\""),
                "{html}"
            );
            assert!(html.contains("data-tone=\"error\""), "{html}");
            assert!(
                html.contains("id=\"theatre-home-session-refresh\""),
                "{html}"
            );
            for absent in GUEST_INTENTS {
                assert!(!html.contains(absent), "{absent}: {html}");
            }
            // An unresolved logout hides the account and still offers no sign-in.
            signed_in(account, PreviewActor::A).unwrap();
            logout_pending(account).unwrap();
            let html = render();
            assert!(html.contains("data-tone=\"warning\""), "{html}");
            for absent in GUEST_INTENTS {
                assert!(!html.contains(absent), "{absent}: {html}");
            }
        });
    }

    /// One mounted owner drives the slot: sign-in removes the guest intents in place, and a
    /// confirmed logout brings them back, without remounting Discovery.
    #[test]
    fn the_guest_cluster_follows_login_and_logout_of_the_same_mounted_owner() {
        Owner::new().with(|| {
            let account = guest_account();
            let ui = account_ui(account.read_only());
            let slot = Memo::new(move |_| home_session(&ui.get()));
            assert_eq!(slot.get_untracked(), HomeSession::GuestActions);
            signed_in(account, PreviewActor::A).unwrap();
            assert_eq!(slot.get_untracked(), HomeSession::SignedIn);
            logout_pending(account).unwrap();
            assert_eq!(slot.get_untracked(), HomeSession::Status);
            assert!(crate::features::auth_preview::logout_ack(account, true));
            assert_eq!(slot.get_untracked(), HomeSession::GuestActions);
        });
    }
}
