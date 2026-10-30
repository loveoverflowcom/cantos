//! Live public Work detail. Server text and the Server-authorized cover only: no genre or credit.
//! The Listen
//! control and each unit's control only express an intent; the root resolves the exact release
//! and owns the one player. Opening this page never plays anything.
use crate::contract::{
    discovery::{MetadataUnitSummary, MetadataWorkSummary, UnitKind, WorkKind},
    library::{AudioIdentity, AudioPresentation, Completion, ResumeStatus, WorkPresentation},
};
use crate::{
    application::{
        account_view::{AccountViewState, SessionPhase},
        library_sync::{ResumeAttemptState, ResumeRefusal, SavedCommandPhase},
        player_session::PlayerSession,
        public_discovery::kind_name,
        public_listen::{ListenFailure, ListenState},
        public_work::{PageStatus, ReadFailure, UnitList, WorkDetailPhase, WorkDetailState},
    },
    core::{
        playback::{Event, Failure, Intent, Status, TrackId},
        public_ids::canonical_uuid,
    },
    features::{
        auth_copy::{self, CopyKey},
        common::clock,
        listen_help::ListenAbsence,
        public_cover::PublicCover,
    },
    messages::{self, text, Key, Locale},
};
use cantos_ui::{
    AccessibleName, Announcement, BackButton, Button, ButtonState, ButtonVariant, DisabledReason,
    IconName, LoadingState,
};
use leptos::prelude::*;

pub(crate) const HEADING_ID: &str = "theatre-public-work-heading";
pub(crate) const UNITS_STATUS_ID: &str = "theatre-public-units-status";
pub(crate) const UNITS_RETRY_ID: &str = "theatre-public-units-retry";
pub(crate) const UNITS_MORE_ID: &str = "theatre-public-units-more";

pub(crate) fn unit_heading_id(unit_id: &str) -> String {
    format!("theatre-public-unit-{unit_id}")
}

/// The unit a listener chose, as the list showed it. The root pins the release from it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct ListenTarget {
    pub unit_id: String,
    pub publication_id: String,
    pub title: String,
}

impl ListenTarget {
    fn of(row: &MetadataUnitSummary) -> Option<Self> {
        Some(Self {
            unit_id: row.unit_id.clone(),
            publication_id: row.playback.as_ref()?.publication_id.clone(),
            title: row.title.clone(),
        })
    }
}

/// What a unit's control offers, derived only from the listen intent and the one session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum RowListen {
    Ready,
    Preparing,
    Playing,
    Failed(ListenFailure),
}

impl RowListen {
    /// The decorative glyph mirrors the verb. `Preparing` shows the button's busy indicator
    /// instead, so its Play glyph is never painted.
    pub(crate) const fn icon(self) -> IconName {
        match self {
            Self::Playing => IconName::Pause,
            Self::Failed(_) => IconName::Refresh,
            Self::Ready | Self::Preparing => IconName::PlayArrow,
        }
    }
}

/// The action and the observed engine state stay distinct. A pending Play is cancellable.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum HeroListen {
    Ready,
    Resolving,
    Pause(Status),
    Resume,
    ResolveFailed(ListenFailure),
    Unavailable(ListenFailure),
    TransportFailed(Failure),
}

impl HeroListen {
    const fn label(self) -> Key {
        match self {
            Self::Ready | Self::Unavailable(_) => Key::WorkListen,
            Self::Resolving => Key::WorkListenLoading,
            Self::Pause(_) => Key::ListenPause,
            Self::Resume => Key::WorkListenResume,
            Self::ResolveFailed(_) | Self::TransportFailed(_) => Key::PublicRetry,
        }
    }

    const fn icon(self) -> IconName {
        match self {
            Self::Pause(_) => IconName::Pause,
            Self::ResolveFailed(_) | Self::TransportFailed(_) => IconName::Refresh,
            Self::Ready | Self::Resolving | Self::Resume | Self::Unavailable(_) => {
                IconName::PlayArrow
            }
        }
    }
}

fn hero_listen(
    listen: &ListenState,
    publication_id: &str,
    current: Option<(uuid::Uuid, Intent, Status, u32)>,
) -> HeroListen {
    let Some(publication) = canonical_uuid(publication_id) else {
        return HeroListen::Ready;
    };
    if listen.is_resolving(publication) {
        return HeroListen::Resolving;
    }
    if let Some(failure) = listen.failure_of(publication) {
        return if failure == ListenFailure::NotFound {
            HeroListen::Unavailable(failure)
        } else {
            HeroListen::ResolveFailed(failure)
        };
    }
    match current.filter(|(id, _, _, _)| *id == publication) {
        Some((_, _, Status::Failed(failure), _)) => HeroListen::TransportFailed(failure),
        Some((_, Intent::Play, status, _)) => HeroListen::Pause(status),
        Some((_, Intent::Pause(_), Status::Paused(_), position)) if position > 0 => {
            HeroListen::Resume
        }
        _ => HeroListen::Ready,
    }
}

/// `current` is the public release in the player and whether the listener intends to play it.
pub(crate) fn row_listen(
    listen: &ListenState,
    current: Option<(uuid::Uuid, bool)>,
    publication_id: &str,
) -> RowListen {
    let Some(publication) = canonical_uuid(publication_id) else {
        return RowListen::Ready;
    };
    if listen.is_resolving(publication) {
        return RowListen::Preparing;
    }
    if let Some(failure) = listen.failure_of(publication) {
        return RowListen::Failed(failure);
    }
    match current {
        Some((playing, true)) if playing == publication => RowListen::Playing,
        _ => RowListen::Ready,
    }
}

/// Everything the Work detail needs to offer listening, without owning any of it.
#[derive(Clone, Copy)]
pub(crate) struct ListenControls {
    pub player: PlayerSession,
    pub listen: RwSignal<ListenState>,
    pub on_listen: Callback<ListenTarget>,
}

impl ListenControls {
    fn hero_of(self, row: &MetadataUnitSummary) -> HeroListen {
        let Some(playback) = &row.playback else {
            return HeroListen::Ready;
        };
        let current = self.player.state.with(|state| {
            let session = state.session()?;
            let TrackId::Public(track) = session.source().track() else {
                return None;
            };
            Some((
                track.publication(),
                session.intent(),
                state.status(),
                session.position_ms(),
            ))
        });
        self.listen
            .with(|listen| hero_listen(listen, &playback.publication_id, current))
    }

    fn press_hero(self, row: &MetadataUnitSummary) {
        match self.hero_of(row) {
            HeroListen::Resolving | HeroListen::Unavailable(_) => (),
            HeroListen::Pause(_) => self.player.dispatch(Event::PauseRequested),
            HeroListen::Resume | HeroListen::TransportFailed(_) => {
                self.player.dispatch(Event::PlayRequested)
            }
            HeroListen::Ready => {
                // A loaded, paused source starts in the same root session, even at position zero.
                if self.current().is_some_and(|(id, _)| {
                    row.playback
                        .as_ref()
                        .is_some_and(|p| canonical_uuid(&p.publication_id) == Some(id))
                }) {
                    self.player.dispatch(Event::PlayRequested);
                } else {
                    self.press(row);
                }
            }
            HeroListen::ResolveFailed(_) => self.press(row),
        }
    }

    fn current(self) -> Option<(uuid::Uuid, bool)> {
        let source = self.player.source.get()?;
        let TrackId::Public(track) = source.track() else {
            return None;
        };
        let playing = self
            .player
            .transport
            .get()
            .is_some_and(|transport| transport.intent == Intent::Play);
        Some((track.publication(), playing))
    }

    fn state_of(self, row: &MetadataUnitSummary) -> RowListen {
        let Some(playback) = &row.playback else {
            return RowListen::Ready;
        };
        let current = self.current();
        self.listen
            .with(|listen| row_listen(listen, current, &playback.publication_id))
    }

    /// Pause what plays, otherwise ask the root to resolve and play this unit.
    fn press(self, row: &MetadataUnitSummary) {
        let Some(target) = ListenTarget::of(row) else {
            return;
        };
        match self.state_of(row) {
            RowListen::Playing => self.player.dispatch(Event::PauseRequested),
            RowListen::Preparing => (),
            RowListen::Ready | RowListen::Failed(_) => {
                self.on_listen.run(target);
            }
        }
    }
}

const fn resolve_failure_key(failure: ListenFailure) -> Key {
    match failure {
        ListenFailure::Diagnostic(kind) => crate::messages::error_key(kind),
        ListenFailure::NotFound => Key::ListenResolveNotFound,
        ListenFailure::Unavailable => Key::ListenResolveUnavailable,
        ListenFailure::Protocol => Key::ListenResolveProtocol,
    }
}

const fn unit_label(kind: UnitKind) -> Key {
    match kind {
        UnitKind::Chapter => Key::PublicWorkChapter,
        UnitKind::Episode => Key::PublicWorkEpisode,
    }
}

/// Coarse shape: the Ready subtree is built once per route generation, so page updates keep
/// the header, rows and the Load more control (and its focus) in place.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Shape {
    Loading,
    NotFound,
    Failed(ReadFailure),
    Ready(u64),
}

fn shape(state: &WorkDetailState) -> Shape {
    match state.phase() {
        WorkDetailPhase::Idle | WorkDetailPhase::Loading => Shape::Loading,
        WorkDetailPhase::NotFound => Shape::NotFound,
        WorkDetailPhase::Failed(failure) => Shape::Failed(*failure),
        WorkDetailPhase::Ready { .. } => Shape::Ready(state.generation()),
    }
}

fn with_units<T>(state: &WorkDetailState, read: impl FnOnce(&UnitList) -> T) -> Option<T> {
    match state.phase() {
        WorkDetailPhase::Ready { units, .. } => Some(read(units)),
        _ => None,
    }
}

/// Where focus goes once a listener-initiated page request settles.
#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
pub(crate) fn page_focus_target(state: &WorkDetailState) -> String {
    with_units(state, |units| match units.status() {
        PageStatus::Failed(_) if units.rows().is_empty() => UNITS_RETRY_ID.to_owned(),
        PageStatus::Failed(_) => UNITS_MORE_ID.to_owned(),
        PageStatus::Idle | PageStatus::Loading => units
            .first_new_row()
            .map(unit_heading_id)
            .unwrap_or_else(|| UNITS_STATUS_ID.to_owned()),
    })
    .unwrap_or_else(|| HEADING_ID.to_owned())
}

/// The polite list status. It never states a count: none is known before the walk ends.
fn status_line(units: &UnitList, work_kind: WorkKind, locale: Locale) -> String {
    let rows = !units.rows().is_empty();
    let status = match units.status() {
        PageStatus::Loading if !units.walk_started() && rows => {
            Some(Key::PublicWorkUnitsRestarting)
        }
        PageStatus::Loading if !rows => Some(Key::PublicWorkUnitsLoading),
        // The Load more control announces its own busy reason.
        PageStatus::Loading => None,
        PageStatus::Failed(_) if !rows => None,
        PageStatus::Failed(ReadFailure::Diagnostic(kind)) => {
            Some(crate::messages::error_key(*kind))
        }
        PageStatus::Failed(ReadFailure::Unavailable) => Some(Key::PublicWorkPageUnavailable),
        PageStatus::Failed(ReadFailure::Protocol) => Some(Key::PublicWorkPageProtocol),
        PageStatus::Idle if units.is_empty_terminal() => Some(match work_kind {
            WorkKind::Podcast => Key::PublicWorkEpisodesEmpty,
            WorkKind::Novel | WorkKind::Story => Key::PublicWorkChaptersEmpty,
        }),
        PageStatus::Idle if units.is_complete() => Some(Key::PublicWorkUnitsComplete),
        PageStatus::Idle if !rows && units.has_more() => Some(Key::PublicWorkUnitsMoreAvailable),
        PageStatus::Idle if !units.walk_started() => Some(Key::PublicWorkUnitsLoading),
        PageStatus::Idle => None,
    };
    let notice = units.notice().map(|_| Key::PublicWorkRestarted);
    [notice, status]
        .into_iter()
        .flatten()
        .map(|key| text(key, locale))
        .collect::<Vec<_>>()
        .join(" ")
}

#[derive(Clone, Copy)]
pub(crate) struct WorkSyncControls {
    pub account: ReadSignal<AccountViewState>,
    pub on_login_to_save: Callback<()>,
    pub on_set_saved: Callback<(String, bool, Option<WorkPresentation>)>,
    pub on_retry_saved: Callback<String>,
    pub on_resume: Callback<(AudioIdentity, u64, ResumeStatus, Option<AudioPresentation>)>,
}

#[component]
pub(crate) fn PublicWorkDetail(
    state: RwSignal<WorkDetailState>,
    locale: RwSignal<Locale>,
    listen: ListenControls,
    on_back: Callback<()>,
    on_retry: Callback<()>,
    on_load_more: Callback<()>,
    #[prop(optional)] sync_controls: Option<WorkSyncControls>,
) -> impl IntoView {
    let current = Memo::new(move |_| state.with(shape));
    view! {
        <section class="theatre-public-work-detail" aria-labelledby=HEADING_ID
            aria-busy=move || (current.get() == Shape::Loading).then_some("true")>
            {move || AccessibleName::new(auth_copy::text(CopyKey::ReturnHome, locale.get())).ok().map(|name| view! {
                <BackButton id="theatre-public-work-back" name on_back=Callback::new(move |_| on_back.run(()))/>
            })}
            {move || match current.get() {
                Shape::Loading => view! {
                    <h1 id=HEADING_ID tabindex="-1">{move || text(Key::AboutWork, locale.get())}</h1>
                    <LoadingState class="theatre-public-state" announcement=Announcement::Polite>
                        {move || text(Key::PublicWorkLoading, locale.get())}
                    </LoadingState>
                }.into_any(),
                Shape::NotFound => view! {
                    <div class="theatre-public-state theatre-panel">
                        <h1 id=HEADING_ID tabindex="-1">{move || text(Key::PublicWorkNotFound, locale.get())}</h1>
                        <p>{move || text(Key::PublicWorkNotFoundHelp, locale.get())} </p>
                        <a class="theatre-text-link" href="#discover">{move || auth_copy::text(CopyKey::ReturnHome, locale.get())}</a>
                    </div>
                }.into_any(),
                Shape::Failed(failure) => {
                    let (heading, help) = match failure {
                        ReadFailure::Diagnostic(kind) => (Key::ErrorDialogTitle, crate::messages::error_key(kind)),
                        ReadFailure::Unavailable => (Key::PublicWorkUnavailable, Key::PublicConnectionUnavailableHelp),
                        ReadFailure::Protocol => (Key::PublicWorkProtocol, Key::PublicWorkProtocolHelp),
                    };
                    view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h1 id=HEADING_ID tabindex="-1">{move || text(heading, locale.get())}</h1>
                            <p>{move || text(help, locale.get())}</p>
                            <Button id="theatre-public-work-retry" icon=IconName::Refresh variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |_| on_retry.run(()))>
                                {move || text(Key::PublicRetry, locale.get())}
                            </Button>
                        </div>
                    }.into_any()
                }
                Shape::Ready(_) => state
                    .with_untracked(|s| match s.phase() {
                        WorkDetailPhase::Ready { work, .. } => Some((**work).clone()),
                        _ => None,
                    })
                    .map(|work| view! { <ReadyWork state locale listen work on_retry on_load_more sync_controls/> }.into_any())
                    .unwrap_or_else(|| ().into_any()),
            }}
        </section>
    }
}

#[component]
fn WorkSaveControl(
    work: MetadataWorkSummary,
    first_publication_id: Signal<Option<String>>,
    locale: RwSignal<Locale>,
    controls: WorkSyncControls,
) -> impl IntoView {
    let work_id = work.work_id.clone();
    let work_id_for_state = StoredValue::new(work.work_id.clone());
    let work_id_for_retry = work.work_id.clone();
    let title = work.title.clone();
    let title_for_guest = work.title.clone();
    let kind = work.kind;
    let language = work.language.clone();
    let saved_state = move || {
        controls.account.with(|a| {
            work_id_for_state.with_value(|wid| {
                a.sync()
                    .and_then(|s| s.saved_work(wid))
                    .cloned()
                    .unwrap_or_default()
            })
        })
    };
    let button_state = Signal::derive(move || match saved_state().command {
        SavedCommandPhase::Pending { desired, .. } => {
            let reason_key = if desired {
                Key::LiveSaveSaving
            } else {
                Key::LiveSaveRemoving
            };
            DisabledReason::new(text(reason_key, locale.get()))
                .map(ButtonState::Loading)
                .unwrap_or_default()
        }
        _ => ButtonState::Enabled,
    });
    view! {
        {move || {
            if controls.account.with(|a| a.phase()) != SessionPhase::SignedIn {
                let guest_sr = title_for_guest.clone();
                view! {
                    <Button id="theatre-public-work-save-login" icon=IconName::Login variant=ButtonVariant::Tonal
                        on_press=Callback::new(move |_| controls.on_login_to_save.run(()))>
                        {move || text(Key::LiveSaveSignIn, locale.get())}
                        <span class="cui-sr-only">" "{guest_sr}</span>
                    </Button>
                }
                .into_any()
            } else {
                let current = saved_state();
                let is_saved = current.is_saved();
                let label_key = if is_saved { Key::Saved } else { Key::Save };
                let is_pending = matches!(&current.command, SavedCommandPhase::Pending { .. });
                let status_key = match &current.command {
                    SavedCommandPhase::UnknownOutcome { .. } => Some((Key::LiveSaveUnknown, true)),
                    SavedCommandPhase::Conflict { .. } => Some((Key::LiveSaveConflict, true)),
                    SavedCommandPhase::Failed { .. } => Some((Key::LiveSaveError, true)),
                    SavedCommandPhase::Idle | SavedCommandPhase::Pending { .. } => None,
                };
                let work_id_press = work_id.clone();
                let title_press = title.clone();
                let title_sr = title.clone();
                let lang_press = language.clone();
                let work_id_retry = work_id_for_retry.clone();
                view! {
                    <div class="theatre-work-save-controls" data-saved=is_saved.to_string()>
                        <Button id="theatre-public-work-save" class=if is_saved && !is_pending { "theatre-save-pill theatre-save-pill-confirmed" } else { "theatre-save-pill" }
                            icon=if is_saved { IconName::Bookmark } else { IconName::BookmarkBorder } variant=ButtonVariant::Tonal state=button_state
                            on_press=Callback::new(move |_| {
                                let presentation = first_publication_id.get_untracked().map(|publication_id| {
                                    WorkPresentation {
                                        work_id: work_id_press.clone(),
                                        publication_id,
                                        title: title_press.clone(),
                                        kind,
                                        language: lang_press.clone(),
                                    }
                                });
                                controls.on_set_saved.run((work_id_press.clone(), !is_saved, presentation));
                            })>
                            <span aria-hidden="true">{move || text(label_key, locale.get())}</span>
                            <span class="cui-sr-only">{move || messages::message(locale.get(), if is_saved {
                                crate::localization::generated::Message::TheatreShellRemoveSaveName { arg_title: &title_sr }
                            } else {
                                crate::localization::generated::Message::TheatreShellSaveName { arg_title: &title_sr }
                            })}</span>
                        </Button>
                        {status_key.map(|(msg_key, can_retry)| view! {
                            <p id="theatre-public-work-save-status" class="theatre-muted" role="status">
                                {move || text(msg_key, locale.get())}
                            </p>
                            {can_retry.then(|| {
                                let retry_id = work_id_retry.clone();
                                view! {
                                    <Button id="theatre-public-work-save-retry" icon=IconName::Refresh variant=ButtonVariant::Text
                                        on_press=Callback::new(move |_| controls.on_retry_saved.run(retry_id.clone()))>
                                        {move || text(Key::PublicRetry, locale.get())}
                                    </Button>
                                }
                            })}
                        })}
                    </div>
                }
                .into_any()
            }
        }}
    }
}

#[component]
fn ReadyWork(
    state: RwSignal<WorkDetailState>,
    locale: RwSignal<Locale>,
    listen: ListenControls,
    work: MetadataWorkSummary,
    on_retry: Callback<()>,
    on_load_more: Callback<()>,
    sync_controls: Option<WorkSyncControls>,
) -> impl IntoView {
    let kind = work.kind;
    let lang = work.language.clone();
    let row_lang = StoredValue::new(work.language.clone());
    let work_summary = StoredValue::new(work.clone());
    let (list_heading, more_label) = match kind {
        WorkKind::Podcast => (Key::PublicWorkEpisodes, Key::PublicWorkMoreEpisodes),
        WorkKind::Novel | WorkKind::Story => (Key::PublicWorkChapters, Key::PublicWorkMoreChapters),
    };
    let membership = Memo::new(move |_| state.with(unit_membership));
    let status = move || state.with(|s| with_units(s, |units| units.status().clone()));
    let first_page_failure = move || {
        state
            .with(|s| {
                with_units(s, |units| match units.status() {
                    PageStatus::Failed(failure) if units.rows().is_empty() => Some(*failure),
                    _ => None,
                })
            })
            .flatten()
    };
    let more_visible = move || {
        state
            .with(|s| {
                with_units(s, |units| {
                    units.has_more()
                        || (!units.rows().is_empty()
                            && matches!(units.status(), PageStatus::Failed(_)))
                })
            })
            .unwrap_or(false)
    };
    let more_state = Signal::derive(move || match status() {
        Some(PageStatus::Loading) => {
            DisabledReason::new(text(Key::PublicWorkLoadingMore, locale.get()))
                .map(ButtonState::Loading)
                .unwrap_or_default()
        }
        _ => ButtonState::Enabled,
    });
    // Listen starts the first unit the list still offers, in the server's order.
    let first = move || {
        state
            .with(|s| {
                with_units(s, |units| {
                    units
                        .rows()
                        .iter()
                        .find(|row| !units.is_unavailable(&row.unit_id) && row.playback.is_some())
                        .cloned()
                })
            })
            .flatten()
    };
    let first_pub =
        Signal::derive(move || first().and_then(|row| row.playback.map(|p| p.publication_id)));
    let hero = Memo::new(move |_| first().map(|row| listen.hero_of(&row)));
    let listen_absence = move || {
        ListenAbsence::of(
            status().as_ref(),
            state
                .with(|s| with_units(s, UnitList::is_complete))
                .unwrap_or(false),
        )
    };
    let listen_state = Signal::derive(move || {
        let reason = match hero.get() {
            Some(HeroListen::Resolving) => Some((true, text(Key::ListenPreparing, locale.get()))),
            Some(HeroListen::Unavailable(failure)) => {
                Some((false, text(resolve_failure_key(failure), locale.get())))
            }
            None => {
                let absence = listen_absence();
                Some((absence.is_busy(), text(absence.reason(), locale.get())))
            }
            _ => None,
        };
        reason
            .map(|(busy, reason)| {
                let reason = DisabledReason::new(reason).expect("localized listen explanation");
                if busy {
                    ButtonState::Loading(reason)
                } else {
                    ButtonState::Disabled(reason)
                }
            })
            .unwrap_or_default()
    });
    let listen_icon = Signal::derive(move || {
        hero.get()
            .map(HeroListen::icon)
            .unwrap_or(IconName::PlayArrow)
    });
    let listen_label = move || {
        hero.get().map(HeroListen::label).unwrap_or_else(|| {
            if matches!(status(), Some(PageStatus::Loading)) {
                Key::WorkListenLoading
            } else {
                Key::WorkListen
            }
        })
    };
    let hero_feedback = move || match hero.get() {
        Some(HeroListen::Pause(status)) => Some(messages::listening_status_key(status)),
        Some(HeroListen::Resume) => Some(Key::ListenPaused),
        Some(HeroListen::ResolveFailed(failure)) => Some(resolve_failure_key(failure)),
        Some(HeroListen::TransportFailed(failure)) => Some(messages::failure_key(failure)),
        _ => None,
    };
    let description = (!work.description.trim().is_empty()).then(|| work.description.clone());
    // The server's count of currently public units, not the length of the loaded page.
    let unit_count = work.unit_count;
    view! {
        <header class="theatre-work-intro theatre-public-work-intro">
            <PublicCover cover=work.cover.clone() lang=lang.clone() kind=kind_name(kind) class="theatre-public-work-cover"/>
            <div>
                <p class="theatre-eyebrow">{move || text(messages::work_kind_key(kind), locale.get())}</p>
                <h1 id=HEADING_ID tabindex="-1" lang=lang.clone()>{work.title.clone()}</h1>
                {description.map(|text| view! { <p class="theatre-synopsis" lang=lang.clone()>{text}</p> })}
                <p class="theatre-muted theatre-public-work-count" data-unit-count=unit_count.to_string()>
                    {move || messages::public_unit_count(kind, locale.get(), unit_count)}
                </p>
                <div class="theatre-actions">
                    <Button id="theatre-public-work-listen" variant=ButtonVariant::ListenHero icon_signal=listen_icon state=listen_state
                        described_by="theatre-work-listen-feedback"
                        on_press=Callback::new(move |_| { if let Some(row) = first() { listen.press_hero(&row); } })>
                        {move || text(listen_label(), locale.get())}
                        {move || first().map(|row| view! { <span class="cui-sr-only">" "{row.title}</span> })}
                    </Button>
                    {sync_controls.map(|controls| view! {
                        <WorkSaveControl work=work.clone() first_publication_id=first_pub locale controls/>
                    })}
                </div>
                <p id="theatre-work-listen-feedback" class="theatre-muted" role="status">
                    {move || hero_feedback().map(|key| text(key, locale.get())).unwrap_or_default()}
                </p>
            </div>
        </header>
        <section class="theatre-public-units" aria-labelledby="theatre-public-units-heading"
            aria-busy=move || matches!(status(), Some(PageStatus::Loading)).then_some("true")>
            <h2 id="theatre-public-units-heading" class="theatre-section-heading">{move || text(list_heading, locale.get())}</h2>
            <ol class="theatre-episode-list theatre-public-unit-list">
                <For each=move || membership.get() key=|id| id.clone()
                    children=move |unit_id| view! { <UnitRow state=state.read_only() unit_id work=work_summary.get_value() lang=row_lang.get_value() locale listen sync_controls/> }/>
            </ol>
            {move || first_page_failure().map(|failure| {
                let help = match failure {
                    ReadFailure::Diagnostic(kind) => crate::messages::error_key(kind),
                    ReadFailure::Unavailable => Key::PublicWorkUnitsUnavailableHelp,
                    ReadFailure::Protocol => Key::PublicWorkUnitsProtocolHelp,
                };
                view! {
                    <div class="theatre-public-state theatre-panel">
                        <h3>{move || text(Key::PublicWorkUnitsFailed, locale.get())}</h3>
                        <p>{move || text(help, locale.get())}</p>
                        <Button id=UNITS_RETRY_ID variant=ButtonVariant::Tonal
                            on_press=Callback::new(move |_| on_retry.run(()))>
                            {move || text(Key::PublicRetry, locale.get())}
                        </Button>
                    </div>
                }
            })}
            <p id=UNITS_STATUS_ID class="theatre-muted theatre-public-units-status" role="status" tabindex="-1">
                {move || state.with(|s| with_units(s, |units| status_line(units, kind, locale.get()))).unwrap_or_default()}
            </p>
            <Show when=more_visible>
                <Button id=UNITS_MORE_ID variant=ButtonVariant::Tonal state=more_state
                    on_press=Callback::new(move |_| on_load_more.run(()))>
                    {move || text(if matches!(status(), Some(PageStatus::Failed(_))) { Key::PublicWorkRetryPage } else { more_label }, locale.get())}
                </Button>
            </Show>
        </section>
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct UnitRowUi {
    row: MetadataUnitSummary,
    unavailable: bool,
}

fn unit_membership(state: &WorkDetailState) -> Vec<String> {
    with_units(state, |units| {
        units.rows().iter().map(|row| row.unit_id.clone()).collect()
    })
    .unwrap_or_default()
}

fn unit_row_ui(state: &WorkDetailState, unit_id: &str) -> Option<UnitRowUi> {
    with_units(state, |units| {
        units
            .rows()
            .iter()
            .find(|row| row.unit_id == unit_id)
            .map(|row| UnitRowUi {
                row: row.clone(),
                unavailable: units.is_unavailable(unit_id),
            })
    })
    .flatten()
}

fn press_current_unit(state: ReadSignal<WorkDetailState>, unit_id: &str, listen: ListenControls) {
    let current = state.with_untracked(|s| unit_row_ui(s, unit_id));
    if let Some(current) = current.filter(|r| !r.unavailable) {
        listen.press(&current.row);
    }
}

#[component]
fn UnitRow(
    state: ReadSignal<WorkDetailState>,
    unit_id: String,
    work: MetadataWorkSummary,
    lang: String,
    locale: RwSignal<Locale>,
    listen: ListenControls,
    sync_controls: Option<WorkSyncControls>,
) -> impl IntoView {
    let id = StoredValue::new(unit_id.clone());
    let row = Memo::new(move |_| state.with(|s| unit_row_ui(s, &unit_id)));
    let heading = NodeRef::<leptos::html::H3>::new();
    let actions = NodeRef::<leptos::html::Div>::new();
    let offered = Memo::new(move |_| row.with(|r| r.as_ref().is_some_and(|r| !r.unavailable)));
    let row_state = move || {
        row.with(|r| {
            r.as_ref()
                .map(|r| listen.state_of(&r.row))
                .unwrap_or(RowListen::Ready)
        })
    };
    let button_state = Signal::derive(move || {
        if row.with(|r| r.as_ref().is_some_and(|r| r.row.playback.is_none())) {
            DisabledReason::new(text(Key::PublicWorkListenUnavailable, locale.get()))
                .map(ButtonState::Disabled)
                .unwrap_or_default()
        } else {
            match row_state() {
                RowListen::Preparing => row
                    .with(|r| {
                        r.as_ref().and_then(|r| {
                            DisabledReason::new(messages::message(
                        locale.get(),
                        crate::localization::generated::Message::TheatreShellListenUnitPreparing {
                            arg_title: &(r.row.title.clone()),
                        },
                    ))
                    .ok()
                        })
                    })
                    .map(ButtonState::Loading)
                    .unwrap_or_default(),
                _ => ButtonState::Enabled,
            }
        }
    });
    let verb = move || match row_state() {
        RowListen::Playing => Key::ListenPause,
        RowListen::Failed(_) => Key::PublicRetry,
        RowListen::Ready | RowListen::Preparing => Key::PublicWorkListen,
    };
    let verb_icon = Signal::derive(move || row_state().icon());
    let unit_progress = move || {
        let controls = sync_controls?;
        row.with(|r| {
            let r = r.as_ref()?;
            let publication_id = &r.row.playback.as_ref()?.publication_id;
            controls.account.with(|a| {
                a.sync()
                    .and_then(|s| s.progress_for(publication_id))
                    .and_then(|p| p.record.clone())
            })
        })
    };
    let unit_progress_text = move || {
        let rec = unit_progress()?;
        let pos_sec = u32::try_from(rec.position_ms / 1000).unwrap_or(u32::MAX);
        let total_sec = u32::try_from(rec.audio.duration_ms / 1000).unwrap_or(u32::MAX);
        let line = messages::message(
            locale.get(),
            crate::localization::generated::Message::TheatreShellLivePosition {
                arg_position: &(clock(locale.get(), pos_sec)),
                arg_total: &(clock(locale.get(), total_sec)),
            },
        );
        Some(if rec.completion == Completion::Completed {
            format!("{line} · {}", text(Key::LiveResumeCompleted, locale.get()))
        } else {
            line
        })
    };
    let unit_resume_feedback = move || {
        let controls = sync_controls?;
        row.with(|r| {
            let r = r.as_ref()?;
            controls.account.with(|a| match a.sync()?.resume_attempt() {
                ResumeAttemptState::Restored {
                    publication_id,
                    position_ms,
                } if r
                    .row
                    .playback
                    .as_ref()
                    .is_some_and(|p| publication_id == &p.publication_id) =>
                {
                    Some(messages::message(
                        locale.get(),
                        crate::localization::generated::Message::TheatreShellLiveResumeRestored {
                            arg_time: &(clock(locale.get(), position_ms / 1000)),
                        },
                    ))
                }
                ResumeAttemptState::Refused {
                    publication_id,
                    reason,
                } if r
                    .row
                    .playback
                    .as_ref()
                    .is_some_and(|p| publication_id == &p.publication_id) =>
                {
                    Some(
                        text(
                            match reason {
                                ResumeRefusal::ReleaseReplaced => Key::LiveResumeReplaced,
                                ResumeRefusal::AccessUnavailable => Key::LiveResumeUnavailable,
                                ResumeRefusal::LoadFailed | ResumeRefusal::Protocol => {
                                    Key::LiveResumeLoadFailed
                                }
                            },
                            locale.get(),
                        )
                        .to_owned(),
                    )
                }
                _ => None,
            })
        })
    };
    let resume_visible =
        Memo::new(move |_| offered.get() && unit_progress().is_some_and(|r| r.position_ms > 0));
    let seconds = move || {
        row.with(|r| {
            r.as_ref()
                .and_then(|r| {
                    r.row
                        .playback
                        .as_ref()
                        .map(|p| u32::try_from(p.duration_ms / 1000).unwrap_or(u32::MAX))
                })
                .unwrap_or_default()
        })
    };
    let row_lang = StoredValue::new(lang);
    let work = StoredValue::new(work);
    view! {
        <Show when=move || row.with(Option::is_some)>
            <li class="theatre-episode-row theatre-public-unit" data-unit-id=id.get_value()
                data-listen=move || match row_state() {
                    RowListen::Ready => "ready", RowListen::Preparing => "preparing", RowListen::Playing => "playing", RowListen::Failed(_) => "failed",
                }
                data-unavailable=move || row.with(|r| r.as_ref().is_some_and(|r| r.unavailable)).then_some("true")>
                <div class="theatre-episode-copy">
                    <p class="theatre-eyebrow" data-ordinal=move || row.with(|r| r.as_ref().map(|r| r.row.ordinal.to_string()))>
                        {move || row.with(|r| r.as_ref().map(|r| messages::message(locale.get(), crate::localization::generated::Message::TheatreShellPublicWorkUnitLabel { arg_kind: text(unit_label(r.row.kind), locale.get()), arg_ordinal: r.row.ordinal })))}
                    </p>
                    <h3 node_ref=heading id=unit_heading_id(&id.get_value()) tabindex="-1" lang=row_lang.get_value()>{move || row.with(|r| r.as_ref().map(|r| r.row.title.clone()).unwrap_or_default())}</h3>
                    <Show when=move || row.with(|r| r.as_ref().is_some_and(|r| !r.row.description.trim().is_empty()))>
                        <p class="theatre-muted" lang=row_lang.get_value()>{move || row.with(|r| r.as_ref().map(|r| r.row.description.clone()).unwrap_or_default())}</p>
                    </Show>
                    <p class="theatre-muted theatre-public-unit-duration">
                        <Show when=move || row.with(|r| r.as_ref().is_some_and(|r| r.row.playback.as_ref().is_some_and(|p| p.duration_ms > 0))) fallback=move || text(Key::PublicWorkDurationUnknown, locale.get())>
                            <span aria-hidden="true">{move || clock(locale.get(), seconds())}</span>
                            <span class="cui-sr-only">{move || messages::message(locale.get(), crate::localization::generated::Message::TheatreShellPublicWorkDurationDescription { arg_duration: &(messages::spoken_time(locale.get(), seconds())) })}</span>
                        </Show>
                    </p>
                    {move || unit_progress_text().map(|line| view! { <p class="theatre-muted theatre-public-unit-progress">{line}</p> })}
                    {move || unit_resume_feedback().map(|feedback| view! { <p class="theatre-muted theatre-public-unit-resume-status" role="status">{feedback}</p> })}
                    <Show when=move || row.with(|r| r.as_ref().is_some_and(|r| r.unavailable))>
                        <p class="theatre-muted">{move || text(Key::PublicWorkRowUnavailable, locale.get())}</p>
                    </Show>
                    {move || match row_state() {
                        RowListen::Failed(failure) => Some(view! { <p class="theatre-muted theatre-public-unit-failure" role="status">{text(resolve_failure_key(failure), locale.get())}</p> }),
                        _ => None,
                    }}
                </div>
                <Show when=move || {
                    let offered = offered.get();
                    // Run before Show removes the focused subtree; an effect afterward would
                    // see body. Other focus is left alone, including during background walks.
                    if !offered { crate::platform::focus_before_unit_actions_removed(actions, heading); }
                    offered
                }>
                    <div node_ref=actions class="theatre-episode-actions">
                        <Button id=format!("theatre-public-unit-listen-{}", id.get_value()) variant=ButtonVariant::Tonal icon_signal=verb_icon state=button_state
                            on_press=Callback::new(move |_| id.with_value(|id| press_current_unit(state, id, listen)))>
                            {move || text(verb(), locale.get())}
                            <span class="cui-sr-only">" "{move || row.with(|r| r.as_ref().map(|r| r.row.title.clone()).unwrap_or_default())}</span>
                        </Button>
                        <Show when=move || resume_visible.get()>
                            <Button id=format!("theatre-public-unit-resume-{}", id.get_value()) variant=ButtonVariant::Text
                                on_press=Callback::new(move |_| {
                                    let Some(controls) = sync_controls else { return; };
                                    let Some(current) = state.with_untracked(|s| id.with_value(|id| unit_row_ui(s, id))).filter(|r| !r.unavailable) else { return; };
                                    let Some(publication_id) = current.row.playback.as_ref().map(|p| p.publication_id.as_str()) else { return; };
                                    let Some(rec) = controls.account.with_untracked(|a| a.sync().and_then(|s| s.progress_for(publication_id)).and_then(|p| p.record.clone())).filter(|r| r.position_ms > 0) else { return; };
                                    let w = work.get_value();
                                    let pres = AudioPresentation { work: WorkPresentation { work_id: w.work_id, publication_id: rec.audio.publication_id.clone(), title: w.title, kind: w.kind, language: w.language }, unit_id: current.row.unit_id, unit_title: current.row.title };
                                    controls.on_resume.run((rec.audio, rec.position_ms, ResumeStatus::ExactAvailable, Some(pres)));
                                })>
                                {move || text(Key::LiveResumeOpen, locale.get())}
                                <span class="cui-sr-only">" "{move || row.with(|r| r.as_ref().map(|r| r.row.title.clone()).unwrap_or_default())}</span>
                            </Button>
                        </Show>
                    </div>
                </Show>
            </li>
        </Show>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::discovery::{MetadataUnitPage, MetadataWorkPage};
    use crate::{
        application::public_work::{CatalogReadError, Step},
        core::public_ids::PublicWorkId,
        infrastructure::inert_media::InertMedia,
    };

    const WORK: &str = "10000000-0000-4000-8000-000000000001";

    fn fixture_work() -> MetadataWorkSummary {
        let page: MetadataWorkPage = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/covered_works.json"
        ))
        .unwrap();
        page.items.into_iter().next().unwrap()
    }
    fn fixture_units() -> MetadataUnitPage {
        serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/presented_units.json"
        ))
        .unwrap()
    }
    fn unit(n: u128, title: &str) -> MetadataUnitSummary {
        MetadataUnitSummary {
            unit_id: uuid::Uuid::from_u128(0x2000_0000_0000_4000_8000_0000_0000_0000 + n)
                .to_string(),
            kind: UnitKind::Chapter,
            position: 7,
            title: title.into(),
            description: "Một đoạn mô tả đã công bố.".into(),
            playback: Some(crate::contract::discovery::UnitPlayback {
                publication_id: uuid::Uuid::from_u128(
                    0x8000_0000_0000_4000_8000_0000_0000_0000 + n,
                )
                .to_string(),
                published_at_ms: 5_000,
                duration_ms: 754_000,
            }),
            ordinal: n as u64,
        }
    }
    fn fixture_manifest(
        row: &MetadataUnitSummary,
    ) -> crate::contract::discovery::PublicationManifest {
        let mut manifest: crate::contract::discovery::PublicationManifest =
            serde_json::from_str(include_str!(
                "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/manifest.json"
            ))
            .unwrap();
        manifest.work_id = WORK.into();
        manifest.unit_id = row.unit_id.clone();
        manifest.publication_id = row.playback.as_ref().unwrap().publication_id.clone();
        manifest
    }
    fn page(items: Vec<MetadataUnitSummary>, next: Option<&str>) -> MetadataUnitPage {
        MetadataUnitPage {
            work_id: WORK.into(),
            items,
            next_cursor: next.map(str::to_owned),
        }
    }
    /// A ready Work with its page-one request in flight.
    fn opened(
        work: MetadataWorkSummary,
    ) -> (WorkDetailState, crate::application::public_work::PageTicket) {
        let mut state = WorkDetailState::default();
        let ticket = state.open(PublicWorkId::parse(WORK).unwrap()).unwrap();
        let Step::Continue(first) = state.finish_work(&ticket, Ok(work)) else {
            panic!("page one follows the detail")
        };
        (state, first)
    }
    fn controls() -> ListenControls {
        ListenControls {
            player: PlayerSession::new(InertMedia),
            listen: RwSignal::new(ListenState::default()),
            on_listen: Callback::new(|_| ()),
        }
    }
    fn render(state: WorkDetailState, locale: Locale) -> String {
        Owner::new().with(|| {
            let listen = controls();
            view! {
                <PublicWorkDetail state=RwSignal::new(state) locale=RwSignal::new(locale) listen
                    on_back=Callback::new(|()| ()) on_retry=Callback::new(|()| ())
                    on_load_more=Callback::new(|()| ())/>
            }
            .to_html()
        })
    }
    /// Only the Server-authorized cover may be an image; nothing else is borrowed or invented.
    fn assert_no_fabrication(html: &str) {
        assert!(
            html.matches("class=\"theatre-cover-art\"").count() <= 1,
            "{html}"
        );
        for absent in [
            "<audio",
            "<video",
            "Tập 1",
            "Chương 1",
            "Chương 8",
            "publication_id",
            "lc1_",
            "Sơn Hà Ký",
            "Minh An",
            "Đã nghe xong",
        ] {
            assert!(!html.contains(absent), "{absent}");
        }
    }

    fn replace_walk(state: &mut WorkDetailState, items: Vec<MetadataUnitSummary>) {
        let more = state.load_more().unwrap();
        let Step::Continue(first) = state.finish_page(&more, Err(CatalogReadError::InvalidCursor))
        else {
            panic!("restart")
        };
        assert_eq!(
            state.finish_page(&first, Ok(page(items, Some("lc1_next")))),
            Step::Settled
        );
    }

    #[test]
    fn detail_shows_the_authorized_cover_with_its_alt_text_and_keeps_title_and_listen() {
        let work = fixture_work();
        let cover = work.cover.clone().unwrap();
        let (mut state, first) = opened(work);
        state.finish_page(&first, Ok(page(vec![unit(1, "Chương một")], None)));
        let html = render(state, Locale::Vietnamese);
        assert!(html.contains(&format!("src=\"{}\"", cover.url)), "{html}");
        assert!(
            html.contains(&format!("alt=\"{}\"", cover.alt_text)),
            "{html}"
        );
        assert!(html.contains("data-cover=\"loading\""), "{html}");
        assert_eq!(
            html.matches("class=\"theatre-cover-art\"").count(),
            1,
            "{html}"
        );
        assert!(!html.contains("theatre-cover-fallback-art"), "{html}");
        // Title, summary and Listen never depend on the image.
        assert!(
            html.contains("Ánh đèn cuối sân khấu") && html.contains("theatre-public-work-listen")
        );
        // Private and rights metadata never reach the page.
        for private in ["asset_id", "rights", "provenance", "storage", "owner"] {
            assert!(!html.contains(private), "{private}");
        }
    }

    #[test]
    fn detail_without_a_cover_keeps_the_typographic_fallback_and_no_image() {
        let mut work = fixture_work();
        work.cover = None;
        let (mut state, first) = opened(work);
        state.finish_page(&first, Ok(page(vec![unit(1, "Chương một")], None)));
        let html = render(state, Locale::Vietnamese);
        assert!(html.contains("data-cover=\"none\""), "{html}");
        assert!(!html.contains("theatre-cover-art"), "{html}");
        assert!(html.contains("theatre-cover-fallback-art"), "{html}");
        assert!(
            html.contains("assets/artwork/cantos-cover-fallback.avif"),
            "{html}"
        );
        assert!(!html.contains("theatre-cover-moon"), "{html}");
    }

    #[test]
    fn unit_fields_refresh_with_unchanged_membership_and_event_time_listen_uses_new_release() {
        use std::sync::{Arc, Mutex};
        Owner::new().with(|| {
            let original = unit(1, "Ban đầu");
            let (mut initial, first) = opened(fixture_work());
            initial.finish_page(&first, Ok(page(vec![original.clone()], Some("lc1_next"))));
            let state = RwSignal::new(initial);
            let id = StoredValue::new(original.unit_id.clone());
            let row = Memo::new(move |_| state.with(|s| id.with_value(|id| unit_row_ui(s, id))));
            let membership = Memo::new(move |_| state.with(unit_membership));
            let original_ids = membership.get();
            let targets = Arc::new(Mutex::new(Vec::new()));
            let received = targets.clone();
            let listen = ListenControls {
                on_listen: Callback::new(move |target| received.lock().unwrap().push(target)),
                ..controls()
            };
            for field in 0..7 {
                let mut updated = original.clone();
                match field {
                    0 => updated.title = "Tên hiện tại".into(),
                    1 => updated.description = "Mô tả hiện tại".into(),
                    2 => updated.playback.as_mut().unwrap().duration_ms = 123_000,
                    3 => updated.ordinal = 12,
                    4 => updated.position = 18,
                    5 => updated.playback.as_mut().unwrap().published_at_ms = 55_000,
                    6 => {
                        updated.playback.as_mut().unwrap().publication_id =
                            uuid::Uuid::from_u128(999).to_string()
                    }
                    _ => unreachable!(),
                }
                state.update(|s| replace_walk(s, vec![updated.clone()]));
                assert_eq!(
                    row.get().unwrap(),
                    UnitRowUi {
                        row: updated.clone(),
                        unavailable: false
                    }
                );
                assert_eq!(membership.get(), original_ids);
                // The retained handler consults the state even before a rendered row reads it.
                id.with_value(|id| press_current_unit(state.read_only(), id, listen));
                assert_eq!(
                    targets.lock().unwrap().last(),
                    ListenTarget::of(&updated).as_ref()
                );
            }
            let count = targets.lock().unwrap().len();
            state.update(|s| {
                s.leave();
            });
            id.with_value(|id| press_current_unit(state.read_only(), id, listen));
            assert_eq!(targets.lock().unwrap().len(), count);
            assert_eq!(row.get(), None);
        });
    }

    #[test]
    fn disclosed_unit_without_audio_is_visible_but_never_dispatches_listen() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        Owner::new().with(|| {
            let mut item = unit(1, "Chương chưa có audio");
            item.playback = None;
            let (mut initial, first) = opened(fixture_work());
            initial.finish_page(&first, Ok(page(vec![item.clone()], None)));
            let html = render(initial.clone(), Locale::Vietnamese);
            assert!(html.contains("Chương chưa có audio"));
            assert!(html.contains("disabled"));
            let state = RwSignal::new(initial);
            let calls = Arc::new(AtomicUsize::new(0));
            let observed = calls.clone();
            let listen = ListenControls {
                on_listen: Callback::new(move |_| {
                    observed.fetch_add(1, Ordering::SeqCst);
                }),
                ..controls()
            };
            press_current_unit(state.read_only(), &item.unit_id, listen);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        });
    }

    fn without_audio(n: u128, title: &str) -> MetadataUnitSummary {
        let mut item = unit(n, title);
        item.playback = None;
        item
    }

    #[test]
    fn absent_audio_keeps_short_reason_without_help_control() {
        for locale in Locale::supported() {
            let (mut complete, first) = opened(fixture_work());
            complete.finish_page(&first, Ok(page(vec![without_audio(1, "Một")], None)));
            let html = render(complete, locale);
            let cta = html
                .split("id=\"theatre-public-work-listen\"")
                .nth(1)
                .unwrap();
            assert!(cta.contains("aria-disabled=\"true\""), "{cta}");
            // The essential reason is visible and tied to the control with no interaction.
            assert!(
                html.contains(text(Key::WorkListenUnavailable, locale)),
                "{html}"
            );
            assert!(html.contains("aria-describedby=\"theatre-work-listen-feedback theatre-public-work-listen-reason\""));
            // The row repeats only the short phrase, never the long copy.
            assert!(html.contains(text(Key::PublicWorkListenUnavailable, locale)));
            assert!(!html.contains("theatre-work-listen-help-trigger"));
            assert!(!html.contains("theatre-work-listen-help"));
        }
    }

    #[test]
    fn a_partial_list_without_audio_is_not_reported_as_permanently_empty() {
        let (mut partial, first) = opened(fixture_work());
        partial.finish_page(
            &first,
            Ok(page(vec![without_audio(1, "Một")], Some("lc1_next"))),
        );
        let html = render(partial, Locale::English);
        let reason = html
            .split("id=\"theatre-public-work-listen-reason\"")
            .nth(1)
            .and_then(|rest| rest.split("</span>").next())
            .unwrap();
        assert!(
            reason.contains(text(Key::WorkListenNoTarget, Locale::English)),
            "{reason}"
        );
        assert!(!html.contains("theatre-work-listen-help-trigger"));
    }

    #[test]
    fn loading_and_failed_lists_keep_their_own_state_and_offer_no_audio_detail() {
        for locale in Locale::supported() {
            let (loading, _) = opened(fixture_work());
            let html = render(loading, locale);
            assert!(!html.contains("theatre-work-listen-help-trigger"), "{html}");
            let (mut failed, first) = opened(fixture_work());
            failed.finish_page(&first, Err(CatalogReadError::Unavailable));
            let html = render(failed, locale);
            assert!(html.contains(text(Key::PublicWorkUnitsFailed, locale)));
            assert!(!html.contains("theatre-work-listen-help-trigger"), "{html}");
            assert!(html.contains(UNITS_RETRY_ID));
        }
    }

    #[test]
    fn unavailable_unit_keeps_identity_but_retained_action_is_inert() {
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };
        Owner::new().with(|| {
            let original = unit(1, "Một");
            let (mut initial, first) = opened(fixture_work());
            initial.finish_page(&first, Ok(page(vec![original.clone()], Some("lc1_next"))));
            let state = RwSignal::new(initial);
            let calls = Arc::new(AtomicUsize::new(0));
            let count = calls.clone();
            let listen = ListenControls {
                on_listen: Callback::new(move |_| {
                    count.fetch_add(1, Ordering::SeqCst);
                }),
                ..controls()
            };
            let more = state
                .try_update(WorkDetailState::load_more)
                .unwrap()
                .unwrap();
            let Step::Continue(first) = state
                .try_update(|s| s.finish_page(&more, Err(CatalogReadError::InvalidCursor)))
                .unwrap()
            else {
                panic!("restart")
            };
            state.update(|s| {
                s.finish_page(&first, Ok(page(Vec::new(), None)));
            });
            assert_eq!(state.with(unit_membership), vec![original.unit_id.clone()]);
            assert!(
                state
                    .with(|s| unit_row_ui(s, &original.unit_id))
                    .unwrap()
                    .unavailable
            );
            press_current_unit(state.read_only(), &original.unit_id, listen);
            assert_eq!(calls.load(Ordering::SeqCst), 0);
        });
    }

    #[test]
    fn ready_detail_renders_server_text_rows_duration_and_enabled_listen_controls() {
        let (mut state, first) = opened(fixture_work());
        state.finish_page(&first, Ok(fixture_units()));
        let html = render(state, Locale::Vietnamese);
        for expected in [
            "Ánh đèn cuối sân khấu",
            "Một lời hẹn chưa bao giờ nói ra.",
            "Một tiếng chuông trong sương",
            "Chương mở đầu của bản thử hư cấu.",
            "Các chương",
            ">Truyện<",
            "0:01",
            "Thời lượng: ",
            "Đã hiển thị hết danh sách.",
            "theatre-public-work-back",
            "Về Trang chủ",
            "lang=\"vi-VN\"",
            "data-unit-id=\"20000000-0000-4000-8000-000000000002\"",
            "id=\"theatre-public-unit-listen-20000000-0000-4000-8000-000000000002\"",
            "data-listen=\"ready\"",
        ] {
            assert!(html.contains(expected), "{expected}");
        }
        // Listening is offered, not blocked, and the name carries the unit it starts.
        let cta = html
            .split("id=\"theatre-public-work-listen\"")
            .nth(1)
            .and_then(|rest| rest.split("</button>").next())
            .unwrap();
        assert!(!cta.contains("disabled"), "{cta}");
        assert!(cta.contains(">Nghe<") && cta.contains("Một tiếng chuông trong sương"));
        for stale in [
            "Cantos Web chưa phát được âm thanh.",
            "aria-describedby=\"theatre-public-work-listen-reason\"",
        ] {
            assert!(!html.contains(stale), "{stale}");
        }
        assert!(
            !html.contains(UNITS_MORE_ID),
            "a complete list offers no Load more"
        );
        assert_no_fabrication(&html);
    }

    #[test]
    fn unit_listen_icon_mirrors_the_verb_and_the_hero_glyphs() {
        // Same registry glyphs as the Work hero for the same intent: Play, Pause, Retry.
        for (state, icon) in [
            (RowListen::Ready, IconName::PlayArrow),
            (RowListen::Preparing, IconName::PlayArrow),
            (RowListen::Playing, IconName::Pause),
            (
                RowListen::Failed(ListenFailure::NotFound),
                IconName::Refresh,
            ),
        ] {
            assert_eq!(state.icon(), icon, "{state:?}");
        }
        assert_eq!(RowListen::Ready.icon(), HeroListen::Ready.icon());
        assert_eq!(
            RowListen::Playing.icon(),
            HeroListen::Pause(Status::Playing).icon()
        );
        assert_eq!(
            RowListen::Failed(ListenFailure::NotFound).icon(),
            HeroListen::ResolveFailed(ListenFailure::NotFound).icon()
        );
    }

    #[test]
    fn a_unit_listen_button_leads_with_the_decorative_play_glyph() {
        let (mut state, first) = opened(fixture_work());
        state.finish_page(&first, Ok(fixture_units()));
        let html = render(state, Locale::Vietnamese);
        let button_of = |id: &str| {
            html.split(id)
                .nth(1)
                .and_then(|rest| rest.split("</button>").next())
                .unwrap_or_else(|| panic!("{id} is rendered"))
                .to_owned()
        };
        let icon_of = |button: &str| {
            button
                .split("cui-button-icon")
                .nth(1)
                .and_then(|rest| rest.split("</span>").next())
                .unwrap_or_else(|| panic!("a labelled listen action carries an icon: {button}"))
                .to_owned()
        };
        let unit =
            button_of("id=\"theatre-public-unit-listen-20000000-0000-4000-8000-000000000002\"");
        let hero = button_of("id=\"theatre-public-work-listen\"");
        let glyph = icon_of(&unit);
        assert!(glyph.contains("aria-hidden=\"true\""), "{unit}");
        // Same registry glyph as the Work hero for the same intent (Ready: Play).
        assert_eq!(glyph, icon_of(&hero));
        assert!(
            unit.find("cui-button-icon").unwrap() < unit.find("cui-control-label").unwrap(),
            "the glyph leads the label: {unit}"
        );
        // The accessible name stays the visible verb plus the unit; the glyph adds nothing to it.
        assert!(unit.contains(">Nghe<") && unit.contains("Một tiếng chuông trong sương"));
    }

    #[test]
    fn total_and_position_come_from_the_presented_dtos_never_from_the_loaded_page() {
        let mut work = fixture_work();
        work.unit_count = 40;
        let (mut state, first) = opened(work);
        // Two rows of forty, ranked 7 and 8: neither the total nor the rank is a page length
        // or index, and a rank is never presented as an author-assigned chapter number.
        let mut seventh = unit(7, "Bảy");
        seventh.ordinal = 7;
        let mut eighth = unit(8, "Tám");
        eighth.ordinal = 8;
        state.finish_page(&first, Ok(page(vec![seventh, eighth], Some("lc1_opaque"))));
        let vi = render(state.clone(), Locale::Vietnamese);
        for expected in [
            "Chương đã công bố: 40",
            "data-unit-count=\"40\"",
            "Vị trí 7",
            "data-ordinal=\"7\"",
            "Vị trí 8",
        ] {
            assert!(vi.contains(expected), "{expected}");
        }
        for absent in [
            "Chương đã công bố: 2",
            "Vị trí 1",
            "Vị trí 2",
            "Chương 7",
            "Chương 8",
        ] {
            assert!(!vi.contains(absent), "{absent}");
        }
        let en = render(state, Locale::English);
        assert!(en.contains("Published chapters: 40") && en.contains("Position 7"));
    }

    #[test]
    fn listen_is_blocked_with_a_reason_until_the_list_offers_a_unit() {
        for locale in Locale::supported() {
            let (loading, _) = opened(fixture_work());
            let html = render(loading, locale);
            let cta = html
                .split("id=\"theatre-public-work-listen\"")
                .nth(1)
                .unwrap();
            assert!(cta.contains("aria-disabled=\"true\""), "{cta}");
            assert!(
                html.contains(text(Key::PublicWorkUnitsLoading, locale)),
                "{html}"
            );
            assert!(!html.contains(text(Key::WorkListenUnavailable, locale)));
            assert!(html.contains("aria-describedby=\"theatre-work-listen-feedback theatre-public-work-listen-reason\""));
        }
    }

    #[test]
    fn opening_the_detail_never_dispatches_a_player_event_or_a_listen_intent() {
        use crate::{
            application::ports::MediaPort,
            core::playback::{Effect, PlayerState},
        };
        use std::{
            cell::RefCell,
            rc::Rc,
            sync::{Arc, Mutex},
        };
        struct Recording(Rc<RefCell<Vec<Effect>>>);
        impl MediaPort for Recording {
            fn execute(&mut self, effect: &Effect) -> Vec<Event> {
                self.0.borrow_mut().push(effect.clone());
                Vec::new()
            }
        }
        Owner::new().with(|| {
            let effects = Rc::new(RefCell::new(Vec::new()));
            let intents = Arc::new(Mutex::new(0));
            let counted = intents.clone();
            let listen = ListenControls {
                player: PlayerSession::new(Recording(effects.clone())),
                listen: RwSignal::new(ListenState::default()),
                on_listen: Callback::new(move |_| *counted.lock().unwrap() += 1),
            };
            let (mut state, first) = opened(fixture_work());
            state.finish_page(&first, Ok(fixture_units()));
            let _ = view! {
                <PublicWorkDetail state=RwSignal::new(state) locale=RwSignal::new(Locale::Vietnamese) listen
                    on_back=Callback::new(|()| ()) on_retry=Callback::new(|()| ())
                    on_load_more=Callback::new(|()| ())/>
            }
            .to_html();
            assert!(effects.borrow().is_empty());
            assert_eq!(*intents.lock().unwrap(), 0);
            assert_eq!(listen.player.state.get_untracked(), PlayerState::default());
            assert_eq!(listen.listen.get_untracked(), ListenState::default());
        });
    }

    #[test]
    fn a_unit_control_reflects_the_intent_and_the_one_session_and_presses_accordingly() {
        use crate::{
            application::public_listen::{ListenLabel, ListenOutcome},
            core::{
                playback::Source,
                public_track::{PinnedRelease, ReleaseRequest},
            },
        };
        use std::sync::{Arc, Mutex};
        let row = unit(2, "Chương hai");
        let publication =
            uuid::Uuid::parse_str(&row.playback.as_ref().unwrap().publication_id).unwrap();
        Owner::new().with(|| {
            let pressed = Arc::new(Mutex::new(Vec::new()));
            let recorded = pressed.clone();
            let controls = ListenControls {
                player: PlayerSession::new(InertMedia),
                listen: RwSignal::new(ListenState::default()),
                on_listen: Callback::new(move |target: ListenTarget| {
                    recorded.lock().unwrap().push(target)
                }),
            };
            assert_eq!(controls.state_of(&row), RowListen::Ready);
            controls.press(&row);
            assert_eq!(pressed.lock().unwrap().len(), 1);
            assert_eq!(
                pressed.lock().unwrap()[0].publication_id,
                row.playback.as_ref().unwrap().publication_id
            );

            // A resolving intent shows preparing and a second press asks for nothing.
            let request = ReleaseRequest::parse(
                PublicWorkId::parse(WORK).unwrap(),
                &row.unit_id,
                &row.playback.as_ref().unwrap().publication_id,
            )
            .unwrap();
            let label = ListenLabel {
                work_title: "w".into(),
                unit_title: "u".into(),
                language: "vi-VN".into(),
            };
            let mut ticket = None;
            controls.listen.update(|s| ticket = s.begin(request, label));
            assert_eq!(controls.state_of(&row), RowListen::Preparing);
            controls.press(&row);
            assert_eq!(pressed.lock().unwrap().len(), 1);

            // Once selected and playing, the same control pauses instead of asking again.
            let mut manifest = fixture_manifest(&row);
            manifest.duration_ms = 60_000;
            let mut outcome = None;
            controls
                .listen
                .update(|s| outcome = Some(s.finish(&ticket.unwrap(), Ok(manifest))));
            let Some(ListenOutcome::Select(selection)) = outcome else {
                panic!("{outcome:?}")
            };
            let source =
                Source::public(selection.release.track(), selection.release.duration_ms()).unwrap();
            controls.player.dispatch(Event::SourceSelected {
                source,
                position_ms: 0,
            });
            assert_eq!(controls.state_of(&row), RowListen::Ready);
            controls.player.dispatch(Event::PlayRequested);
            assert_eq!(controls.state_of(&row), RowListen::Playing);
            assert_eq!(controls.hero_of(&row), HeroListen::Pause(Status::Loading));
            controls.press_hero(&row);
            assert!(matches!(
                controls.player.status.get_untracked(),
                Status::Paused(_)
            ));
            controls.press_hero(&row);
            assert_eq!(controls.hero_of(&row), HeroListen::Pause(Status::Loading));
            controls.press(&row);
            assert_eq!(controls.state_of(&row), RowListen::Ready);
            assert_eq!(
                pressed.lock().unwrap().len(),
                1,
                "pausing is not a new listen intent"
            );
            let _ = (publication, PinnedRelease::admit);
        });
    }

    #[test]
    fn hero_action_vectors_require_engine_ack_for_playing_and_the_same_release_for_resume() {
        use crate::core::playback::PauseCause;
        let release = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000001").unwrap();
        let other = uuid::Uuid::parse_str("00000000-0000-4000-8000-000000000002").unwrap();
        let listen = ListenState::default();
        for (intent, status, position, expected) in [
            (
                Intent::Play,
                Status::Loading,
                0,
                HeroListen::Pause(Status::Loading),
            ),
            (
                Intent::Play,
                Status::Buffering,
                1000,
                HeroListen::Pause(Status::Buffering),
            ),
            (
                Intent::Play,
                Status::Playing,
                1000,
                HeroListen::Pause(Status::Playing),
            ),
            (
                Intent::Pause(PauseCause::User),
                Status::Paused(PauseCause::User),
                0,
                HeroListen::Ready,
            ),
            (
                Intent::Pause(PauseCause::User),
                Status::Paused(PauseCause::User),
                1,
                HeroListen::Resume,
            ),
            (
                Intent::Pause(PauseCause::User),
                Status::Failed(Failure::Network),
                1000,
                HeroListen::TransportFailed(Failure::Network),
            ),
        ] {
            let actual = hero_listen(
                &listen,
                &release.to_string(),
                Some((release, intent, status, position)),
            );
            assert_eq!(actual, expected);
            assert_eq!(
                hero_listen(
                    &listen,
                    &other.to_string(),
                    Some((release, intent, status, position))
                ),
                HeroListen::Ready
            );
            if status != Status::Playing {
                assert_ne!(actual, HeroListen::Pause(Status::Playing));
            }
        }
    }

    #[test]
    fn row_state_is_derived_from_intent_then_failure_then_the_session_only() {
        use crate::core::public_track::ReleaseRequest;
        let row = unit(3, "Chương ba");
        let publication =
            uuid::Uuid::parse_str(&row.playback.as_ref().unwrap().publication_id).unwrap();
        let other = uuid::Uuid::from_u128(7);
        let request = ReleaseRequest::parse(
            PublicWorkId::parse(WORK).unwrap(),
            &row.unit_id,
            &row.playback.as_ref().unwrap().publication_id,
        )
        .unwrap();
        let label = crate::application::public_listen::ListenLabel {
            work_title: "w".into(),
            unit_title: "u".into(),
            language: "vi-VN".into(),
        };
        let idle = ListenState::default();
        for (current, expected) in [
            (None, RowListen::Ready),
            (Some((publication, false)), RowListen::Ready),
            (Some((publication, true)), RowListen::Playing),
            (Some((other, true)), RowListen::Ready),
        ] {
            assert_eq!(
                row_listen(
                    &idle,
                    current,
                    &row.playback.as_ref().unwrap().publication_id
                ),
                expected
            );
        }
        let mut resolving = ListenState::default();
        let ticket = resolving.begin(request, label).unwrap();
        assert_eq!(
            row_listen(
                &resolving,
                Some((publication, true)),
                &row.playback.as_ref().unwrap().publication_id
            ),
            RowListen::Preparing
        );
        resolving.finish(&ticket, Err(CatalogReadError::NotFound));
        assert_eq!(
            hero_listen(&resolving, &publication.to_string(), None),
            HeroListen::Unavailable(ListenFailure::NotFound),
            "a withdrawn release has a visible disabled reason instead of a transport retry"
        );
        assert_eq!(
            row_listen(
                &resolving,
                None,
                &row.playback.as_ref().unwrap().publication_id
            ),
            RowListen::Failed(ListenFailure::NotFound)
        );
        // Another unit's failure or progress never colors this row.
        assert_eq!(
            row_listen(&resolving, None, "80000000-0000-4000-8000-0000000000aa"),
            RowListen::Ready
        );
        assert_eq!(
            row_listen(&idle, Some((publication, true)), "not-an-id"),
            RowListen::Ready
        );
    }

    #[test]
    fn loading_not_found_and_failures_are_distinct_states_without_mock_fallback() {
        let mut loading = WorkDetailState::default();
        let ticket = loading.open(PublicWorkId::parse(WORK).unwrap()).unwrap();
        let html = render(loading.clone(), Locale::Vietnamese);
        assert!(html.contains("Đang mở tác phẩm…") && html.contains("aria-busy=\"true\""));
        assert_eq!(
            html.matches("cui-loading-indicator").count(),
            1,
            "one loader for one state: {html}"
        );
        assert!(html.contains("role=\"status\""), "{html}");

        let mut not_found = loading.clone();
        not_found.finish_work(&ticket, Err(CatalogReadError::NotFound));
        let html = render(not_found, Locale::Vietnamese);
        assert!(html.contains("Tác phẩm này hiện không mở công khai."));
        assert!(html.contains("href=\"#discover\""));
        assert!(!html.contains("theatre-public-work-retry"));
        for guessed in [
            "rút",
            "riêng tư",
            "bản nháp",
            "withdrawn",
            "private",
            "draft",
        ] {
            assert!(!html.contains(guessed), "{guessed}");
        }

        for (error, heading) in [
            (CatalogReadError::Unavailable, "Chưa mở được tác phẩm."),
            (
                CatalogReadError::Protocol,
                "Chưa đọc được thông tin tác phẩm.",
            ),
        ] {
            let mut failed = loading.clone();
            failed.finish_work(&ticket, Err(error));
            let html = render(failed, Locale::Vietnamese);
            assert!(html.contains(heading), "{heading}");
            assert!(html.contains("theatre-public-work-retry"));
            assert_no_fabrication(&html);
            assert!(!html.contains("Ánh đèn cuối sân khấu"));
        }
    }

    #[test]
    fn terminal_empty_list_is_its_own_state_and_first_page_failure_offers_retry() {
        let (mut empty, first) = opened(fixture_work());
        empty.finish_page(&first, Ok(page(vec![], None)));
        let html = render(empty, Locale::Vietnamese);
        assert!(html.contains("Tác phẩm này chưa có chương nào mở công khai."));
        assert!(!html.contains(UNITS_RETRY_ID) && !html.contains(UNITS_MORE_ID));

        let (mut failed, first) = opened(fixture_work());
        failed.finish_page(&first, Err(CatalogReadError::Unavailable));
        let html = render(failed, Locale::English);
        assert!(html.contains("The list could not be loaded."));
        assert!(html.contains(UNITS_RETRY_ID));
        assert!(!html.contains("no publicly available chapters"));
    }

    #[test]
    fn partial_list_offers_load_more_without_a_count_and_page_failure_keeps_rows() {
        let (mut state, first) = opened(fixture_work());
        state.finish_page(
            &first,
            Ok(page(
                vec![
                    unit(1, "Tiếng gõ cửa lúc nửa đêm"),
                    unit(2, "Bức thư <không> ký tên"),
                ],
                Some("lc1_opaque"),
            )),
        );
        let html = render(state.clone(), Locale::Vietnamese);
        assert!(html.contains("Tải thêm chương"));
        assert!(html.contains("Bức thư &lt;không&gt; ký tên"));
        assert!(html.contains("12:34"));
        assert!(!html.contains("Đã hiển thị hết"));
        assert_no_fabrication(&html);

        let second = state.load_more().unwrap();
        let loading = render(state.clone(), Locale::Vietnamese);
        assert!(loading.contains("Đang tải phần tiếp theo…"));
        assert!(loading.contains("Tiếng gõ cửa lúc nửa đêm"));

        state.finish_page(&second, Err(CatalogReadError::Unavailable));
        let html = render(state.clone(), Locale::Vietnamese);
        assert!(html.contains("Tiếng gõ cửa lúc nửa đêm"));
        assert!(html.contains("Chưa tải được phần tiếp theo."));
        assert!(html.contains("Thử lại phần này"));
        assert!(
            !html.contains(UNITS_RETRY_ID),
            "rows stay; only the page retries"
        );
        assert_eq!(page_focus_target(&state), UNITS_MORE_ID);
    }

    #[test]
    fn restart_notice_podcast_episodes_and_unknown_duration_render_honestly() {
        let mut podcast = fixture_work();
        podcast.kind = WorkKind::Podcast;
        let (mut state, first) = opened(podcast);
        let mut episode = unit(3, "Trò chuyện bên hiên nhà");
        episode.kind = UnitKind::Episode;
        episode.playback = None;
        state.finish_page(&first, Ok(page(vec![episode.clone()], Some("lc1_old"))));
        let second = state.load_more().unwrap();
        let Step::Continue(restart) =
            state.finish_page(&second, Err(CatalogReadError::InvalidCursor))
        else {
            panic!("restart")
        };
        let html = render(state.clone(), Locale::Vietnamese);
        assert!(html.contains("Phiên duyệt danh sách đã hết hạn"));
        assert!(html.contains("Đang tải lại danh sách từ đầu…"));
        state.finish_page(&restart, Ok(page(vec![episode], Some("lc1_new"))));
        let html = render(state.clone(), Locale::English);
        assert!(html.contains("Episodes") && html.contains("Load more episodes"));
        assert!(html.contains("Episode · Position ") && html.contains(">Podcast<"));
        assert!(html.contains("Duration not known yet") && !html.contains("0:00"));
        assert_eq!(html.matches("data-unit-id=").count(), 1);
        assert!(!html.contains("Episode 1"));
    }

    #[test]
    fn copy_is_complete_and_nonblank_in_both_locales() {
        for locale in Locale::supported() {
            for key in [
                Key::PublicWorkLoading,
                Key::AboutWork,
                Key::PublicWorkNotFound,
                Key::PublicWorkNotFoundHelp,
                Key::PublicWorkUnavailable,
                Key::PublicConnectionUnavailableHelp,
                Key::PublicWorkProtocol,
                Key::PublicWorkProtocolHelp,
                Key::PublicRetry,
                Key::PublicWorkChapter,
                Key::PublicWorkEpisode,
                Key::PublicWorkChapters,
                Key::PublicWorkEpisodes,
                Key::PublicWorkListen,
                Key::PublicWorkListenUnavailable,
                Key::PublicWorkUnitsLoading,
                Key::PublicWorkUnitsRestarting,
                Key::PublicWorkChaptersEmpty,
                Key::PublicWorkEpisodesEmpty,
                Key::PublicWorkUnitsComplete,
                Key::PublicWorkUnitsMoreAvailable,
                Key::PublicWorkUnitsFailed,
                Key::PublicWorkUnitsUnavailableHelp,
                Key::PublicWorkUnitsProtocolHelp,
                Key::PublicWorkPageUnavailable,
                Key::PublicWorkPageProtocol,
                Key::PublicWorkRestarted,
                Key::PublicWorkMoreChapters,
                Key::PublicWorkMoreEpisodes,
                Key::PublicWorkLoadingMore,
                Key::PublicWorkRetryPage,
                Key::PublicWorkDurationUnknown,
                Key::PublicWorkRowUnavailable,
            ] {
                assert!(!text(key, locale).trim().is_empty(), "{key:?}");
            }
        }
    }

    #[test]
    fn both_locales_preserve_content_and_map_each_read_failure_and_duration() {
        for (locale, kind, chapters, duration, listen_label, not_found, unavailable, protocol) in [
            (
                Locale::Vietnamese,
                ">Truyện<",
                "Các chương",
                "Thời lượng: 12 phút 34 giây",
                ">Nghe<",
                "Tác phẩm này hiện không mở công khai.",
                "Chưa mở được tác phẩm.",
                "Chưa đọc được thông tin tác phẩm.",
            ),
            (
                Locale::English,
                ">Story<",
                "Chapters",
                "Duration: 12 minutes 34 seconds",
                ">Listen<",
                "This story is not publicly available right now.",
                "The story could not be opened.",
                "The story details could not be read.",
            ),
        ] {
            let mut work = fixture_work();
            work.title = "Lời hẹn {time} <cuối>".into();
            let (mut ready, first) = opened(work);
            ready.finish_page(&first, Ok(page(vec![unit(1, "Tập {duration}")], None)));
            let html = render(ready, locale);
            for expected in [
                kind,
                chapters,
                duration,
                listen_label,
                "Lời hẹn {time} &lt;cuối&gt;",
                "Tập {duration}",
                "lang=\"vi-VN\"",
            ] {
                assert!(html.contains(expected), "{expected}: {html}");
            }
            for (failure, heading) in [
                (CatalogReadError::NotFound, not_found),
                (CatalogReadError::Unavailable, unavailable),
                (CatalogReadError::Protocol, protocol),
            ] {
                let mut failed = WorkDetailState::default();
                let ticket = failed.open(PublicWorkId::parse(WORK).unwrap()).unwrap();
                failed.finish_work(&ticket, Err(failure));
                assert!(render(failed, locale).contains(heading), "{heading}");
            }
            for (failure, help) in [
                (
                    CatalogReadError::Unavailable,
                    text(Key::PublicWorkUnitsUnavailableHelp, locale),
                ),
                (
                    CatalogReadError::Protocol,
                    text(Key::PublicWorkUnitsProtocolHelp, locale),
                ),
            ] {
                let (mut failed, first) = opened(fixture_work());
                failed.finish_page(&first, Err(failure));
                let html = render(failed, locale);
                assert!(
                    html.contains(help) && html.contains(UNITS_RETRY_ID),
                    "{html}"
                );
            }
        }
    }

    #[test]
    fn public_work_save_and_unit_resume_controls_follow_signed_in_library_sync_state() {
        use crate::application::{
            account::AccountScope,
            account_view::{AccountViewState, SessionCompletion},
            library_sync::ResumeAttemptState,
        };
        use crate::contract::library::{
            AudioIdentity, Completion, ProgressKind, ProgressRecord, SavedRecord,
        };

        let mut work = fixture_work();
        work.work_id = WORK.into();
        let row = unit(1, "Chương một: Khởi đầu");
        let (mut detail, first) = opened(work);
        detail.finish_page(&first, Ok(page(vec![row.clone()], None)));

        // 1. Guest listener: Save button prompts sign-in and shows no private progress.
        let mut guest_account = AccountViewState::default();
        let issued = guest_account.begin_refresh().unwrap();
        let _ = guest_account.finish_session(
            issued,
            Err(crate::application::account_view::AccountFailure::SignedOut),
            || uuid::Uuid::from_u128(100).to_string(),
        );

        let render_with_sync = |account_state: AccountViewState, locale: Locale| -> String {
            Owner::new().with(|| {
                let state = RwSignal::new(detail.clone());
                let account = RwSignal::new(account_state);
                let listen = controls();
                let sync_controls = WorkSyncControls {
                    account: account.read_only(),
                    on_set_saved: Callback::new(|_| {}),
                    on_retry_saved: Callback::new(|_| {}),
                    on_login_to_save: Callback::new(|_| {}),
                    on_resume: Callback::new(|_| {}),
                };
                view! {
                    <PublicWorkDetail
                        state
                        locale=RwSignal::new(locale)
                        listen
                        on_back=Callback::new(|()| ())
                        on_retry=Callback::new(|_| {})
                        on_load_more=Callback::new(|_| {})
                        sync_controls
                    />
                }
                .to_html()
            })
        };

        let guest_html = render_with_sync(guest_account, Locale::Vietnamese);
        assert!(guest_html.contains("id=\"theatre-public-work-save-login\""));
        assert!(!guest_html.contains("theatre-public-unit-resume-"));

        // 2. Signed-in listener with saved=true and in-progress unit record + restored resume status.
        let mut signed_in = AccountViewState::default();
        let issued = signed_in.begin_refresh().unwrap();
        assert!(matches!(
            signed_in.finish_session(
                issued,
                Ok(AccountScope::from_session(
                    "listener-a".into(),
                    uuid::Uuid::from_u128(10).to_string()
                )
                .unwrap()),
                || uuid::Uuid::from_u128(100).to_string()
            ),
            Ok(SessionCompletion::Authenticated(_))
        ));
        signed_in.seed_sync(|sync, _| {
            sync.observe_saved_record(SavedRecord {
                work_id: WORK.into(),
                saved: true,
                revision: 2,
                listener_order: 2,
                accepted_at_ms: 5_000,
            });
            sync.finish_progress_read(
                &row.playback.as_ref().unwrap().publication_id,
                Some(ProgressRecord {
                    audio: AudioIdentity {
                        work_id: WORK.into(),
                        unit_id: row.unit_id.clone(),
                        publication_id: row.playback.as_ref().unwrap().publication_id.clone(),
                        rendition_id: uuid::Uuid::from_u128(99).to_string(),
                        duration_ms: row.playback.as_ref().unwrap().duration_ms,
                        sha256: "a".repeat(64),
                    },
                    position_ms: 125_000,
                    completion: Completion::InProgress,
                    kind: ProgressKind::Checkpoint,
                    revision: 3,
                    listener_order: 3,
                    accepted_at_ms: 6_000,
                    written_by: uuid::Uuid::from_u128(77).to_string(),
                }),
                || uuid::Uuid::from_u128(88).to_string(),
                None,
            );
            sync.set_resume_attempt(ResumeAttemptState::Restored {
                publication_id: row.playback.as_ref().unwrap().publication_id.clone(),
                position_ms: 125_000,
            });
        });

        let signed_html = render_with_sync(signed_in, Locale::Vietnamese);
        assert!(signed_html.contains("id=\"theatre-public-work-save\""));
        assert!(signed_html.contains("data-saved=\"true\""));
        assert!(signed_html.contains("theatre-save-pill-confirmed"));
        assert!(signed_html.contains("Đã lưu"));
        assert!(signed_html.contains("Bỏ lưu tác phẩm"));
        assert!(signed_html.contains("M17 3H7"));
        assert!(signed_html.contains("Vị trí 2:05 trên 12:34"));
        assert!(signed_html.contains(&format!(
            "id=\"theatre-public-unit-resume-{}\"",
            row.unit_id
        )));
        assert!(signed_html.contains("Đã khôi phục vị trí nghe tại 2:05 (đang tạm dừng)."));
    }
}
