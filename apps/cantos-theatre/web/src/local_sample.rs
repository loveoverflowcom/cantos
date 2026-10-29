//! Opt-in local sample catalog preview (`?mode=local-db`, `preview` feature only).
//!
//! Connects Theatre Web on localhost to the loopback `--sample-serve` host (`127.0.0.1:8080`)
//! through Trunk's same-origin `/api/` proxy. Reads the 6 private Works, 12 draft Chapters and
//! 6 original PNG covers from Server-owned Turso (`server.db`) without changing public discovery
//! policy, fabricating release metadata or exposing Studio credentials.
use crate::{
    app::routes::Route,
    application::{
        errors::ErrorKind,
        public_discovery::kind_name,
        public_work::{CatalogReadError, ReadFailure},
    },
    contract::discovery::{
        CoverImage, CoverMime, ListenerError, ListenerErrorCode, UnitKind, WorkKind,
    },
    core::public_ids::{canonical_uuid, PublicWorkId},
    features::{
        auth_copy::{self, CopyKey},
        common::{icon, EmptyArtwork, EmptyArtworkKind},
        discovery_language,
        public_cover::PublicCover,
        public_home::work_link_id,
        public_work::{unit_heading_id, HEADING_ID},
    },
    messages::{self, text, Key, Locale},
    platform,
    view::{
        LiveBrand, LiveContainer, LiveFrame, LiveLocaleSelect, LivePreferences, LiveRecovery,
        NAV_ICON_SELECTED,
    },
};
use cantos_ui::{Theme, ThemeCycleButton};
use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

pub const SAMPLE_PREVIEW_SCHEMA: &str = "cantos.sample-preview.v1";
pub const SAMPLE_DATASET_ID: &str = "cantos-sample-catalog-v1";
pub const SAMPLE_WORKS_PATH: &str = "/api/v1/sample-preview/works";

const MAX_LIST_BYTES: usize = 524_288;
const MAX_DETAIL_BYTES: usize = 524_288;
const MAX_ERROR_BYTES: usize = 4_096;
const MAX_TITLE_BYTES: usize = 512;
const MAX_DESCRIPTION_BYTES: usize = 16_384;
const MAX_ALT_BYTES: usize = 1024;
const MAX_LANGUAGE_BYTES: usize = 63;

const NAV_ICON_DISCOVER: &str = "home";
const NAV_ICON_LIBRARY: &str = "library_books";
const ICON_REPLAY: &str = "replay";
const ICON_PLAY: &str = "play_arrow";
const ICON_BACK: &str = "arrow_back";

/// Same-origin relative path for one sample Work detail read.
pub fn sample_work_path(work: PublicWorkId) -> String {
    format!("{SAMPLE_WORKS_PATH}/{work}")
}

/// Same-origin relative path for one sample Work's original cover bytes.
pub fn sample_cover_path(work: PublicWorkId, sha256: &str) -> String {
    format!("{SAMPLE_WORKS_PATH}/{work}/cover/{sha256}")
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleWorkList {
    pub schema: String,
    pub dataset_id: String,
    pub items: Vec<SampleWorkSummary>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleWorkSummary {
    pub work_id: String,
    pub kind: WorkKind,
    pub title: String,
    pub description: String,
    pub language: String,
    pub chapter_count: u64,
    pub cover: Option<SampleCover>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleCover {
    pub path: String,
    pub sha256: String,
    pub content_type: CoverMime,
    pub width_px: u64,
    pub height_px: u64,
    pub byte_length: u64,
    pub alt_text: String,
}

impl SampleCover {
    /// Converts an admitted sample cover to the presentation `CoverImage` consumed by `<PublicCover>`.
    pub fn to_cover_image(&self) -> CoverImage {
        CoverImage {
            url: self.path.clone(),
            sha256: self.sha256.clone(),
            content_type: self.content_type,
            width_px: self.width_px,
            height_px: self.height_px,
            byte_length: self.byte_length,
            alt_text: self.alt_text.clone(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleWorkDetail {
    pub schema: String,
    pub work: SampleWorkSummary,
    pub chapters: Vec<SampleChapter>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SampleAudioAvailability {
    Unavailable,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SampleChapter {
    pub unit_id: String,
    pub kind: UnitKind,
    pub position: u32,
    pub title: String,
    pub description: String,
    pub audio: SampleAudioAvailability,
}

fn is_sha256(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn is_language_tag(value: &str) -> bool {
    let trimmed = value.trim();
    !trimmed.is_empty()
        && trimmed.len() <= MAX_LANGUAGE_BYTES
        && trimmed == value
        && trimmed
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
}

/// Admits only the exact same-origin relative cover path bound to `work` and its 64-hex digest.
pub fn is_admitted_sample_cover(work: PublicWorkId, cover: &SampleCover) -> bool {
    is_sha256(&cover.sha256)
        && !cover.alt_text.trim().is_empty()
        && cover.alt_text.len() <= MAX_ALT_BYTES
        && cover.width_px > 0
        && cover.height_px > 0
        && cover.byte_length > 0
        && cover.path == sample_cover_path(work, &cover.sha256)
}

pub fn validate_work_summary(work: &SampleWorkSummary) -> Option<PublicWorkId> {
    let id = PublicWorkId::parse(&work.work_id)?;
    let valid = !work.title.trim().is_empty()
        && work.title.len() <= MAX_TITLE_BYTES
        && work.description.len() <= MAX_DESCRIPTION_BYTES
        && is_language_tag(&work.language)
        && work.chapter_count > 0
        && work
            .cover
            .as_ref()
            .is_some_and(|cover| is_admitted_sample_cover(id, cover));
    valid.then_some(id)
}

pub fn validate_list(list: &SampleWorkList) -> bool {
    if list.schema != SAMPLE_PREVIEW_SCHEMA || list.dataset_id != SAMPLE_DATASET_ID {
        return false;
    }
    let mut ids = BTreeSet::new();
    list.items
        .iter()
        .all(|item| validate_work_summary(item).is_some_and(|id| ids.insert(id)))
}

pub fn validate_detail(detail: &SampleWorkDetail, expected: PublicWorkId) -> bool {
    if detail.schema != SAMPLE_PREVIEW_SCHEMA {
        return false;
    }
    if validate_work_summary(&detail.work) != Some(expected) {
        return false;
    }
    if u64::try_from(detail.chapters.len()).ok() != Some(detail.work.chapter_count) {
        return false;
    }
    let mut unit_ids = BTreeSet::new();
    detail.chapters.iter().enumerate().all(|(index, chapter)| {
        let expected_pos = u32::try_from(index).ok().and_then(|i| i.checked_add(1));
        canonical_uuid(&chapter.unit_id).is_some()
            && unit_ids.insert(chapter.unit_id.as_str())
            && chapter.kind == UnitKind::Chapter
            && Some(chapter.position) == expected_pos
            && !chapter.title.trim().is_empty()
            && chapter.title.len() <= MAX_TITLE_BYTES
            && chapter.description.len() <= MAX_DESCRIPTION_BYTES
            && chapter.audio == SampleAudioAvailability::Unavailable
    })
}

fn is_json(content_type: Option<&str>) -> bool {
    content_type.is_some_and(|value| {
        value
            .split(';')
            .next()
            .is_some_and(|kind| kind.trim().eq_ignore_ascii_case("application/json"))
    })
}

#[derive(Deserialize)]
struct SampleErrorEnvelope {
    code: String,
    #[serde(default)]
    retryable: Option<bool>,
}

fn classify_error(status: u16, content_type: Option<&str>, body: &[u8]) -> CatalogReadError {
    match status {
        401 => return CatalogReadError::Diagnostic(ErrorKind::Unauthenticated),
        403 => return CatalogReadError::Diagnostic(ErrorKind::Forbidden),
        408 | 504 => return CatalogReadError::Diagnostic(ErrorKind::Timeout),
        _ => (),
    }
    if is_json(content_type) && body.len() <= MAX_ERROR_BYTES {
        if let Ok(listener) = serde_json::from_slice::<ListenerError>(body) {
            return match (status, listener.code, listener.retryable) {
                (404, ListenerErrorCode::NotFound, false) => CatalogReadError::NotFound,
                (
                    503,
                    ListenerErrorCode::Unavailable | ListenerErrorCode::RenditionUnavailable,
                    true,
                ) => CatalogReadError::Unavailable,
                _ => CatalogReadError::Protocol,
            };
        }
        if let Ok(sample_err) = serde_json::from_slice::<SampleErrorEnvelope>(body) {
            return match (status, sample_err.code.as_str(), sample_err.retryable) {
                (404, "not_found", None | Some(false)) => CatalogReadError::NotFound,
                (503, "unavailable", None | Some(true)) => CatalogReadError::Unavailable,
                _ => CatalogReadError::Protocol,
            };
        }
    }
    CatalogReadError::Unavailable
}

pub fn decode_list(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
) -> Result<SampleWorkList, CatalogReadError> {
    if status != 200 {
        return Err(match classify_error(status, content_type, body) {
            CatalogReadError::NotFound | CatalogReadError::InvalidCursor => {
                CatalogReadError::Protocol
            }
            error => error,
        });
    }
    if body.len() > MAX_LIST_BYTES || !is_json(content_type) {
        return Err(CatalogReadError::Protocol);
    }
    let list: SampleWorkList =
        serde_json::from_slice(body).map_err(|_| CatalogReadError::Protocol)?;
    if !validate_list(&list) {
        return Err(CatalogReadError::Protocol);
    }
    Ok(list)
}

pub fn decode_detail(
    status: u16,
    content_type: Option<&str>,
    body: &[u8],
    expected: PublicWorkId,
) -> Result<SampleWorkDetail, CatalogReadError> {
    if status != 200 {
        return Err(match classify_error(status, content_type, body) {
            CatalogReadError::InvalidCursor => CatalogReadError::Protocol,
            error => error,
        });
    }
    if body.len() > MAX_DETAIL_BYTES || !is_json(content_type) {
        return Err(CatalogReadError::Protocol);
    }
    let detail: SampleWorkDetail =
        serde_json::from_slice(body).map_err(|_| CatalogReadError::Protocol)?;
    if !validate_detail(&detail, expected) {
        return Err(CatalogReadError::Protocol);
    }
    Ok(detail)
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ListTicket {
    generation: u64,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SampleListPhase {
    #[default]
    Idle,
    Loading,
    Ready(Vec<SampleWorkSummary>),
    Empty,
    Failed(ReadFailure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SampleListState {
    generation: u64,
    phase: SampleListPhase,
}

impl SampleListState {
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn phase(&self) -> &SampleListPhase {
        &self.phase
    }

    /// Starts the initial load only when `Idle`. Returning from a Work detail keeps `Ready`.
    pub fn ensure_loaded(&mut self) -> Option<ListTicket> {
        if matches!(self.phase, SampleListPhase::Idle) {
            self.begin()
        } else {
            None
        }
    }

    pub fn begin(&mut self) -> Option<ListTicket> {
        let next = self.generation.checked_add(1)?;
        self.generation = next;
        self.phase = SampleListPhase::Loading;
        Some(ListTicket { generation: next })
    }

    pub fn retry(&mut self) -> Option<ListTicket> {
        if matches!(self.phase, SampleListPhase::Failed(_)) {
            self.begin()
        } else {
            None
        }
    }

    pub fn finish(
        &mut self,
        ticket: &ListTicket,
        result: Result<SampleWorkList, CatalogReadError>,
    ) -> bool {
        if self.generation != ticket.generation || !matches!(self.phase, SampleListPhase::Loading) {
            return false;
        }
        self.phase = match result {
            Ok(list) if !validate_list(&list) => SampleListPhase::Failed(ReadFailure::Protocol),
            Ok(list) if list.items.is_empty() => SampleListPhase::Empty,
            Ok(list) => SampleListPhase::Ready(list.items),
            Err(error) => SampleListPhase::Failed(error.into()),
        };
        true
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DetailTicket {
    generation: u64,
    work: PublicWorkId,
}

impl DetailTicket {
    pub const fn work(&self) -> PublicWorkId {
        self.work
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SampleDetailPhase {
    #[default]
    Idle,
    Loading,
    Ready(Box<SampleWorkDetail>),
    NotFound,
    Failed(ReadFailure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SampleDetailState {
    generation: u64,
    work: Option<PublicWorkId>,
    phase: SampleDetailPhase,
}

impl SampleDetailState {
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    pub const fn work_id(&self) -> Option<PublicWorkId> {
        self.work
    }

    pub const fn phase(&self) -> &SampleDetailPhase {
        &self.phase
    }

    pub fn open(&mut self, work: PublicWorkId) -> Option<DetailTicket> {
        if self.work == Some(work)
            && matches!(
                self.phase,
                SampleDetailPhase::Loading | SampleDetailPhase::Ready(_)
            )
        {
            return None;
        }
        let next = self.generation.checked_add(1)?;
        self.generation = next;
        self.work = Some(work);
        self.phase = SampleDetailPhase::Loading;
        Some(DetailTicket {
            generation: next,
            work,
        })
    }

    pub fn retry(&mut self) -> Option<DetailTicket> {
        let work = self.work?;
        if !matches!(self.phase, SampleDetailPhase::Failed(_)) {
            return None;
        }
        let next = self.generation.checked_add(1)?;
        self.generation = next;
        self.phase = SampleDetailPhase::Loading;
        Some(DetailTicket {
            generation: next,
            work,
        })
    }

    pub fn leave(&mut self) -> bool {
        let Some(next) = self.generation.checked_add(1) else {
            return false;
        };
        self.generation = next;
        self.work = None;
        self.phase = SampleDetailPhase::Idle;
        true
    }

    pub fn finish(
        &mut self,
        ticket: &DetailTicket,
        result: Result<SampleWorkDetail, CatalogReadError>,
    ) -> bool {
        if self.generation != ticket.generation
            || self.work != Some(ticket.work)
            || !matches!(self.phase, SampleDetailPhase::Loading)
        {
            return false;
        }
        self.phase = match result {
            Ok(detail) if !validate_detail(&detail, ticket.work) => {
                SampleDetailPhase::Failed(ReadFailure::Protocol)
            }
            Ok(detail) => SampleDetailPhase::Ready(Box::new(detail)),
            Err(CatalogReadError::NotFound) => SampleDetailPhase::NotFound,
            Err(error) => SampleDetailPhase::Failed(error.into()),
        };
        true
    }
}

#[cfg(target_arch = "wasm32")]
async fn get_bytes(
    path: &str,
    limit: usize,
) -> Result<(u16, Option<String>, Vec<u8>), CatalogReadError> {
    use gloo_net::http::Request;
    use web_sys::{AbortController, RequestCredentials};

    let controller =
        AbortController::new().map_err(|_| CatalogReadError::Diagnostic(ErrorKind::Network))?;
    let deadline = crate::live::browser::deadline(controller)
        .map_err(|_| CatalogReadError::Diagnostic(ErrorKind::Network))?;
    let signal = deadline.controller.signal();
    let response = Request::get(path)
        .credentials(RequestCredentials::Omit)
        .header("Accept", "application/json")
        .abort_signal(Some(&signal))
        .send()
        .await
        .map_err(|_| CatalogReadError::Diagnostic(deadline.failure_kind()))?;
    let declared = response
        .headers()
        .get("content-length")
        .and_then(|value| value.trim().parse::<usize>().ok());
    if declared.is_some_and(|length| length > limit) {
        return Err(CatalogReadError::Protocol);
    }
    let content_type = response.headers().get("content-type");
    let body = response
        .binary()
        .await
        .map_err(|_| CatalogReadError::Diagnostic(deadline.failure_kind()))?;
    Ok((response.status(), content_type, body))
}

#[cfg(target_arch = "wasm32")]
pub async fn fetch_list() -> Result<SampleWorkList, CatalogReadError> {
    let (status, content_type, body) = get_bytes(SAMPLE_WORKS_PATH, MAX_LIST_BYTES).await?;
    decode_list(status, content_type.as_deref(), &body)
}

#[cfg(target_arch = "wasm32")]
pub async fn fetch_detail(work: PublicWorkId) -> Result<SampleWorkDetail, CatalogReadError> {
    let path = sample_work_path(work);
    let (status, content_type, body) = get_bytes(&path, MAX_DETAIL_BYTES).await?;
    decode_detail(status, content_type.as_deref(), &body, work)
}

fn spawn_list(state: RwSignal<SampleListState>, ticket: ListTicket) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        let result = fetch_list().await;
        let _ = state.try_update(|s| s.finish(&ticket, result));
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, ticket);
}

fn focus_id(id: String) {
    #[cfg(target_arch = "wasm32")]
    platform::focus_by_id(id);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = id;
}

fn spawn_detail(state: RwSignal<SampleDetailState>, ticket: DetailTicket, focus_heading: bool) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        let result = fetch_detail(ticket.work()).await;
        let settled = state
            .try_update(|s| s.finish(&ticket, result))
            .unwrap_or(false);
        if settled && focus_heading {
            focus_id(HEADING_ID.to_owned());
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, ticket, focus_heading);
}

fn in_app_activation(event: &leptos::ev::MouseEvent) -> bool {
    event.button() == 0
        && !event.default_prevented()
        && !(event.ctrl_key() || event.meta_key() || event.shift_key() || event.alt_key())
}

fn sample_unit_count_label(kind: WorkKind, count: u64, locale: Locale) -> String {
    let label = match kind {
        WorkKind::Podcast => text(Key::PublicWorkEpisodes, locale),
        WorkKind::Novel | WorkKind::Story => text(Key::PublicWorkChapters, locale),
    };
    format!("{label}: {count}")
}

fn sample_feature_meta(kind: WorkKind, language: &str, count: u64, locale: Locale) -> String {
    let units = sample_unit_count_label(kind, count, locale);
    messages::message(
        locale,
        crate::localization::generated::Message::TheatreShellPublicHomeFeatureMeta {
            arg_kind: text(messages::work_kind_key(kind), locale),
            arg_language: language,
            arg_units: &units,
        },
    )
}

#[component]
fn SampleCoverFrame(
    kind: WorkKind,
    cover: Option<SampleCover>,
    lang: String,
    class: &'static str,
) -> impl IntoView {
    let cover_image = cover.as_ref().map(SampleCover::to_cover_image);
    view! {
        <PublicCover cover=cover_image lang kind=kind_name(kind) class=class/>
    }
}

#[component]
fn SampleWorkCard(
    work: SampleWorkSummary,
    locale: RwSignal<Locale>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    let kind = work.kind;
    let count = work.chapter_count;
    let language = discovery_language::display_name(&work.language);
    let action_id = format!("theatre-public-work-action-{}", work.work_id);
    let title_id = format!("theatre-public-work-title-{}", work.work_id);
    let labelled_by = format!("{action_id} {title_id}");
    let arrow = icon("arrow_back");
    view! {
        <article class="theatre-public-work theatre-work-card theatre-panel" data-work-id=work.work_id.clone()>
            <SampleCoverFrame kind cover=work.cover.clone() lang=work.language.clone() class="theatre-public-cover"/>
            <div class="theatre-public-work-body">
                <p class="theatre-eyebrow">{move || text(messages::work_kind_key(kind), locale.get())}</p>
                <h3>{match PublicWorkId::parse(&work.work_id) {
                    Some(id) => view! {
                        <a id=work_link_id(id) class="theatre-public-work-link" href=Route::PublicWork(id).hash()
                            lang=move || locale.get().lang()
                            aria-labelledby=labelled_by
                            on:click=move |event| if in_app_activation(&event) {
                                event.prevent_default();
                                on_open_work.run(id);
                            }>
                            <span id=action_id class="cui-sr-only" lang=move || locale.get().lang()>{move || text(Key::PublicHomeOpenWork, locale.get())}</span>
                            <span id=title_id class="theatre-public-work-title" lang=work.language.clone()>{work.title}</span>
                            <span class="theatre-public-work-arrow" aria-hidden="true">{arrow}</span>
                        </a>
                    }.into_any(),
                    None => view! { <span lang=work.language.clone()>{work.title}</span> }.into_any(),
                }}</h3>
                {(!work.description.trim().is_empty()).then(|| view! {
                    <p lang=work.language class="theatre-muted theatre-public-work-description">{work.description}</p>
                })}
                <p class="theatre-muted theatre-public-work-count" data-chapter-count=count.to_string()>
                    {move || sample_feature_meta(kind, &language, count, locale.get())}
                </p>
            </div>
        </article>
    }
}

#[component]
pub fn SampleHome(
    state: RwSignal<SampleListState>,
    locale: RwSignal<Locale>,
    on_retry: Callback<()>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    let is_loading = move || {
        state.with(|s| matches!(s.phase(), SampleListPhase::Idle | SampleListPhase::Loading))
    };
    view! {
        <section class="theatre-public-home" aria-labelledby="theatre-public-home-heading">
            <header class="theatre-discover-intro">
                <div class="theatre-discover-intro-copy">
                    <p class="theatre-eyebrow">{move || text(Key::PublicHomeEyebrow, locale.get())}</p>
                    <h1 id="theatre-public-home-heading">{move || text(Key::HomeHeading, locale.get())}</h1>
                    <p class="theatre-muted">{move || text(Key::PublicHomeIntroduction, locale.get())}</p>
                </div>
            </header>
            <section class="theatre-public-catalog" aria-labelledby="theatre-public-shelf-heading"
                aria-busy=move || is_loading().then_some("true")>
                <h2 id="theatre-public-shelf-heading">{move || text(Key::StoryShelf, locale.get())}</h2>
                {move || state.with(|current| match current.phase() {
                    SampleListPhase::Idle | SampleListPhase::Loading => view! {
                        <div class="cui-loading-state theatre-public-state" role="status" aria-live="polite">
                            {move || text(Key::PublicHomeLoading, locale.get())}
                        </div>
                        <div class="theatre-shelf theatre-public-skeletons" aria-hidden="true">
                            {(0..3).map(|_| view! { <div class="theatre-public-skeleton"></div> }).collect_view()}
                        </div>
                    }.into_any(),
                    SampleListPhase::Ready(items) => {
                        let rows = items.clone();
                        let count = rows.len() as u64;
                        view! {
                            <div class="theatre-shelf theatre-public-work-grid">
                                <For each=move || rows.clone() key=|work| work.work_id.clone()
                                    children=move |work| view! { <SampleWorkCard work locale on_open_work/> }/>
                            </div>
                            <p id="theatre-discovery-status" class="theatre-muted theatre-public-status" role="status" tabindex="-1">
                                {move || messages::message(
                                    locale.get(),
                                    crate::localization::generated::Message::TheatreShellDiscoveryComplete {
                                        arg_count: count,
                                    },
                                )}
                            </p>
                        }.into_any()
                    }
                    SampleListPhase::Empty => view! {
                        <div class="theatre-public-state theatre-public-empty theatre-panel" role="status">
                            <EmptyArtwork kind=EmptyArtworkKind::Works/>
                            <h3>{move || text(Key::PublicHomeEmpty, locale.get())}</h3>
                            <p>{move || text(Key::PublicHomeEmptyHelp, locale.get())}</p>
                        </div>
                    }.into_any(),
                    SampleListPhase::Failed(failure) => {
                        let failure = *failure;
                        let (heading, help) = match failure {
                            ReadFailure::Diagnostic(kind) => (Key::ErrorDialogTitle, crate::messages::error_key(kind)),
                            ReadFailure::Unavailable => (Key::PublicHomeUnavailable, Key::PublicConnectionUnavailableHelp),
                            ReadFailure::Protocol => (Key::PublicHomeProtocol, Key::PublicHomeProtocolHelp),
                        };
                        let retry_icon = icon(ICON_REPLAY);
                        view! {
                            <div class="theatre-public-state theatre-panel" role="status">
                                <h3>{move || text(heading, locale.get())}</h3>
                                <p>{move || text(help, locale.get())}</p>
                                <button id="theatre-home-retry" type="button" class="cui-button cui-button-tonal theatre-action theatre-action-tonal"
                                    on:click=move |_| on_retry.run(())>
                                    {retry_icon}
                                    <span>{move || text(Key::PublicRetry, locale.get())}</span>
                                </button>
                            </div>
                        }.into_any()
                    }
                })}
            </section>
        </section>
    }
}

#[component]
fn SampleChapterRow(
    chapter: SampleChapter,
    lang: String,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let unit_id = chapter.unit_id.clone();
    let position = chapter.position;
    let kind = chapter.kind;
    let title = chapter.title.clone();
    let title_for_aria = chapter.title.clone();
    let description = (!chapter.description.trim().is_empty()).then(|| chapter.description.clone());
    let heading_id = unit_heading_id(&unit_id);
    let reason_id = format!("theatre-sample-unit-unavailable-{unit_id}");
    let reason_id_for_btn = reason_id.clone();
    let button_id = format!("theatre-public-unit-listen-{unit_id}");
    let unit_label_key = match kind {
        UnitKind::Chapter => Key::PublicWorkChapter,
        UnitKind::Episode => Key::PublicWorkEpisode,
    };
    let play_icon = icon(ICON_PLAY);
    view! {
        <li class="theatre-episode-row theatre-public-unit" data-unit-id=unit_id data-audio="unavailable">
            <div class="theatre-episode-copy">
                <p class="theatre-eyebrow" data-ordinal=position.to_string()>
                    {move || messages::message(
                        locale.get(),
                        crate::localization::generated::Message::TheatreShellPublicWorkUnitLabel {
                            arg_kind: text(unit_label_key, locale.get()),
                            arg_ordinal: u64::from(position),
                        },
                    )}
                </p>
                <h3 id=heading_id tabindex="-1" lang=lang.clone()>{title}</h3>
                {description.map(|desc| view! {
                    <p class="theatre-muted" lang=lang.clone()>{desc}</p>
                })}
                <p id=reason_id class="theatre-muted">
                    {move || text(Key::UnavailableHelp, locale.get())}
                </p>
            </div>
            <div class="theatre-episode-actions">
                <button id=button_id type="button" class="theatre-action theatre-action-tonal" disabled=true
                    aria-label=move || messages::message(
                        locale.get(),
                        crate::localization::generated::Message::TheatreShellPlayName {
                            arg_title: &title_for_aria,
                        },
                    )
                    aria-describedby=reason_id_for_btn.clone()>
                    {play_icon}
                    <span class="cui-control-label">{move || text(Key::Unavailable, locale.get())}</span>
                </button>
            </div>
        </li>
    }
}

#[component]
pub fn SampleWorkView(
    state: RwSignal<SampleDetailState>,
    locale: RwSignal<Locale>,
    on_back: Callback<()>,
    on_retry: Callback<()>,
) -> impl IntoView {
    let is_loading = move || {
        state.with(|s| {
            matches!(
                s.phase(),
                SampleDetailPhase::Idle | SampleDetailPhase::Loading
            )
        })
    };
    let back_icon = icon(ICON_BACK);
    view! {
        <section class="theatre-public-work-detail" aria-labelledby=HEADING_ID
            aria-busy=move || is_loading().then_some("true")>
            <button id="theatre-public-work-back" type="button" class="cui-back-button theatre-back"
                aria-label=move || auth_copy::text(CopyKey::ReturnHome, locale.get())
                on:click=move |_| on_back.run(())>
                {back_icon}
                <span>{move || auth_copy::text(CopyKey::ReturnHome, locale.get())}</span>
            </button>
            {move || state.with(|current| match current.phase() {
                SampleDetailPhase::Idle | SampleDetailPhase::Loading => view! {
                    <h1 id=HEADING_ID tabindex="-1">{move || text(Key::AboutWork, locale.get())}</h1>
                    <div class="cui-loading-state theatre-public-state" role="status" aria-live="polite">
                        {move || text(Key::PublicWorkLoading, locale.get())}
                    </div>
                }.into_any(),
                SampleDetailPhase::NotFound => view! {
                    <div class="theatre-public-state theatre-panel">
                        <h1 id=HEADING_ID tabindex="-1">{move || text(Key::PublicWorkNotFound, locale.get())}</h1>
                        <p>{move || text(Key::PublicWorkNotFoundHelp, locale.get())}</p>
                        <a class="theatre-text-link" href="#discover">{move || auth_copy::text(CopyKey::ReturnHome, locale.get())}</a>
                    </div>
                }.into_any(),
                SampleDetailPhase::Failed(failure) => {
                    let failure = *failure;
                    let (heading, help) = match failure {
                        ReadFailure::Diagnostic(kind) => (Key::ErrorDialogTitle, crate::messages::error_key(kind)),
                        ReadFailure::Unavailable => (Key::PublicWorkUnavailable, Key::PublicConnectionUnavailableHelp),
                        ReadFailure::Protocol => (Key::PublicWorkProtocol, Key::PublicWorkProtocolHelp),
                    };
                    let retry_icon = icon(ICON_REPLAY);
                    view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h1 id=HEADING_ID tabindex="-1">{move || text(heading, locale.get())}</h1>
                            <p>{move || text(help, locale.get())}</p>
                            <button id="theatre-public-work-retry" type="button" class="cui-button cui-button-tonal theatre-action theatre-action-tonal"
                                on:click=move |_| on_retry.run(())>
                                {retry_icon}
                                <span>{move || text(Key::PublicRetry, locale.get())}</span>
                            </button>
                        </div>
                    }.into_any()
                }
                SampleDetailPhase::Ready(detail) => {
                    let work = detail.work.clone();
                    let chapters = detail.chapters.clone();
                    let kind = work.kind;
                    let lang = work.language.clone();
                    let chapter_lang = work.language.clone();
                    let chapter_count = work.chapter_count;
                    let description = (!work.description.trim().is_empty()).then(|| work.description.clone());
                    let list_heading = match kind {
                        WorkKind::Podcast => Key::PublicWorkEpisodes,
                        WorkKind::Novel | WorkKind::Story => Key::PublicWorkChapters,
                    };
                    view! {
                        <header class="theatre-work-intro theatre-public-work-intro">
                            <SampleCoverFrame kind cover=work.cover.clone() lang=lang.clone() class="theatre-public-work-cover"/>
                            <div>
                                <p class="theatre-eyebrow">{move || text(messages::work_kind_key(kind), locale.get())}</p>
                                <h1 id=HEADING_ID tabindex="-1" lang=lang.clone()>{work.title.clone()}</h1>
                                {description.map(|synopsis| view! { <p class="theatre-synopsis" lang=lang.clone()>{synopsis}</p> })}
                                <p class="theatre-muted theatre-public-work-count" data-unit-count=chapter_count.to_string()>
                                    {move || sample_unit_count_label(kind, chapter_count, locale.get())}
                                </p>
                                <div class="theatre-actions">
                                    <button id="theatre-public-work-listen" type="button" class="cui-button theatre-action" disabled=true
                                        aria-describedby="theatre-public-work-listen-unavailable">
                                        {move || text(Key::Unavailable, locale.get())}
                                    </button>
                                    <p id="theatre-public-work-listen-unavailable" class="theatre-muted">
                                        {move || text(Key::UnavailableHelp, locale.get())}
                                    </p>
                                </div>
                            </div>
                        </header>
                        <section class="theatre-public-units" aria-labelledby="theatre-public-units-heading">
                            <h2 id="theatre-public-units-heading" class="theatre-section-heading">{move || text(list_heading, locale.get())}</h2>
                            <ol class="theatre-episode-list theatre-public-unit-list">
                                <For each=move || chapters.clone() key=|ch| ch.unit_id.clone()
                                    children=move |ch| view! { <SampleChapterRow chapter=ch lang=chapter_lang.clone() locale/> }/>
                            </ol>
                            <p id="theatre-public-units-status" class="theatre-muted theatre-public-units-status" role="status" tabindex="-1">
                                {move || text(Key::PublicWorkUnitsComplete, locale.get())}
                            </p>
                        </section>
                    }.into_any()
                }
            })}
        </section>
    }
}

fn current_hash_route() -> Route {
    #[cfg(target_arch = "wasm32")]
    {
        let hash = leptos::web_sys::window()
            .and_then(|w| w.location().hash().ok())
            .unwrap_or_default();
        Route::from_live_hash(&hash)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Route::Discover
    }
}

#[cfg(target_arch = "wasm32")]
fn set_location_hash(hash: &str) {
    if let Some(window) = leptos::web_sys::window() {
        if window.location().hash().ok().as_deref() != Some(hash) {
            let _ = window.location().set_hash(hash);
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn set_location_hash(_: &str) {}

#[cfg(target_arch = "wasm32")]
fn bind_sample_navigation(route: RwSignal<Route>, main: NodeRef<leptos::html::Main>) {
    use leptos::leptos_dom::helpers::window_event_listener_untyped;

    let sync = move || {
        let next = current_hash_route();
        if route.get_untracked() != next {
            route.set(next);
            platform::focus_main(main);
        }
    };
    let on_hash = window_event_listener_untyped("hashchange", move |_| sync());
    let on_pop = window_event_listener_untyped("popstate", move |_| sync());
    on_cleanup(move || {
        on_hash.remove();
        on_pop.remove();
    });
}

#[cfg(not(target_arch = "wasm32"))]
fn bind_sample_navigation(_: RwSignal<Route>, _: NodeRef<leptos::html::Main>) {}

#[component]
pub fn LocalSampleApp() -> impl IntoView {
    let preference = crate::localization::load_preference();
    let locale = RwSignal::new(preference.locale);
    let locale_warning = RwSignal::new(preference.warning);
    let theme = RwSignal::new(Theme::System);
    let on_locale = Callback::new(move |value: String| {
        if let Some(selected) = Locale::from_lang(&value) {
            locale.set(selected);
            locale_warning.set(crate::localization::save_preference(selected).err());
        } else {
            locale_warning.set(Some(cantos_localization::PreferenceWarning::Unsupported));
        }
    });
    let main = NodeRef::<leptos::html::Main>::new();
    let route = RwSignal::new(current_hash_route());
    let last_work_link = StoredValue::new(None::<String>);
    let list_state = RwSignal::new(SampleListState::default());
    let detail_state = RwSignal::new(SampleDetailState::default());

    bind_sample_navigation(route, main);

    Effect::new(move |_| {
        if matches!(route.get(), Route::Discover | Route::Explore) {
            let mut issued = None;
            list_state.update(|s| issued = s.ensure_loaded());
            if let Some(ticket) = issued {
                spawn_list(list_state, ticket);
            }
        }
    });

    Effect::new(move |_| match route.get() {
        Route::PublicWork(id) => {
            let mut issued = None;
            detail_state.update(|s| issued = s.open(id));
            if let Some(ticket) = issued {
                spawn_detail(detail_state, ticket, false);
            }
        }
        _ => {
            if detail_state.with_untracked(|s| s.work_id().is_some()) {
                detail_state.update(|s| {
                    let _ = s.leave();
                });
            }
        }
    });

    Effect::new(move |_| {
        let current = route.get();
        let title = match current {
            Route::PublicWork(id) => detail_state.with(|s| match s.phase() {
                SampleDetailPhase::Ready(detail) if s.work_id() == Some(id) => {
                    Some(messages::message(
                        locale.get(),
                        crate::localization::generated::Message::TheatreShellPageTitle {
                            arg_page: &detail.work.title,
                        },
                    ))
                }
                _ => None,
            }),
            Route::NotFound | Route::Work(_) => Some(messages::message(
                locale.get(),
                crate::localization::generated::Message::TheatreShellPageTitle {
                    arg_page: messages::text(Key::LiveNotFound, locale.get()),
                },
            )),
            Route::Library
            | Route::History
            | Route::Account
            | Route::Auth(_)
            | Route::Membership(_)
            | Route::Player
            | Route::Queue => Some(messages::message(
                locale.get(),
                crate::localization::generated::Message::TheatreShellPageTitle {
                    arg_page: messages::text(Key::LiveRouteUnavailable, locale.get()),
                },
            )),
            _ => None,
        };
        platform::set_title(&title.unwrap_or_else(|| messages::page_title(locale.get(), current)));
    });

    Effect::new(move |_| platform::set_document_locale(locale.get().lang(), locale.get().dir()));

    let on_open_work = Callback::new(move |id: PublicWorkId| {
        last_work_link.set_value(Some(work_link_id(id)));
        let hash = Route::PublicWork(id).hash();
        set_location_hash(&hash);
        route.set(Route::PublicWork(id));
        platform::focus_main(main);
    });

    let on_work_back = Callback::new(move |()| {
        set_location_hash("#discover");
        route.set(Route::Discover);
        if let Some(link_id) = last_work_link.get_value() {
            focus_id(link_id);
        } else {
            platform::focus_main(main);
        }
    });

    let on_list_retry = Callback::new(move |()| {
        let mut issued = None;
        list_state.update(|s| issued = s.retry());
        if let Some(ticket) = issued {
            spawn_list(list_state, ticket);
        }
    });

    let on_detail_retry = Callback::new(move |()| {
        let mut issued = None;
        detail_state.update(|s| issued = s.retry());
        if let Some(ticket) = issued {
            spawn_detail(detail_state, ticket, true);
        }
    });

    view! {
        <LiveFrame theme>
            <div class="theatre-app theatre-live-app" data-sample-preview="local-db" lang=move || locale.get().lang() dir=move || locale.get().dir()>
                <a class="theatre-skip" href="#theatre-main" on:click=move |e| { e.prevent_default(); platform::focus_main(main); }>
                    {move || messages::text(Key::SkipContent, locale.get())}
                </a>
                <aside class="theatre-rail">
                    <LiveBrand/>
                    <nav class="theatre-nav" aria-label=move || messages::text(Key::MainNavigation, locale.get())>
                        <a class="theatre-nav-link" href="#discover"
                            aria-current=move || (route.get().primary_navigation() == Some(Route::Discover)).then_some("page")>
                            {icon(NAV_ICON_DISCOVER)}
                            <span>{move || messages::text(Key::Discover, locale.get())}</span>
                            <span class="theatre-nav-selected" aria-hidden="true">{icon(NAV_ICON_SELECTED)}</span>
                        </a>
                        <a class="theatre-nav-link" href="#library"
                            aria-current=move || (route.get().primary_navigation() == Some(Route::Library)).then_some("page")>
                            {icon(NAV_ICON_LIBRARY)}
                            <span>{move || messages::text(Key::Library, locale.get())}</span>
                            <span class="theatre-nav-selected" aria-hidden="true">{icon(NAV_ICON_SELECTED)}</span>
                        </a>
                    </nav>
                </aside>
                <div class="theatre-content">
                    <LiveContainer>
                        <header class="theatre-topbar">
                            <p class="theatre-topbar-context">{move || messages::text(messages::route_key(route.get()), locale.get())}</p>
                            <div class="theatre-topbar-actions">
                                <LiveLocaleSelect locale on_locale/>
                                <ThemeCycleButton
                                    id="theatre-topbar-theme"
                                    theme=theme
                                    labels=Signal::derive(move || crate::localization::theme_labels(locale.get()))
                                />
                            </div>
                        </header>
                        <main id="theatre-main" node_ref=main tabindex="-1" class="theatre-main">
                            <p role="status" hidden=move || locale_warning.get().is_none()>
                                {move || crate::localization::preference_warning(locale_warning.get(), locale.get())}
                            </p>
                            {move || match route.get() {
                                Route::Discover | Route::Explore => view! {
                                    <SampleHome state=list_state locale on_retry=on_list_retry on_open_work/>
                                }.into_any(),
                                Route::PublicWork(_) => view! {
                                    <SampleWorkView state=detail_state locale on_back=on_work_back on_retry=on_detail_retry/>
                                }.into_any(),
                                Route::Settings => view! { <LivePreferences locale theme/> }.into_any(),
                                Route::Library
                                | Route::History
                                | Route::Account
                                | Route::Auth(_)
                                | Route::Membership(_)
                                | Route::Player
                                | Route::Queue => view! {
                                    <LiveRecovery locale heading=Key::LiveRouteUnavailable help=Key::LiveRouteUnavailableHelp/>
                                }.into_any(),
                                Route::NotFound | Route::Work(_) => view! {
                                    <LiveRecovery locale heading=Key::LiveNotFound help=Key::LiveNotFoundHelp/>
                                }.into_any(),
                            }}
                        </main>
                    </LiveContainer>
                </div>
            </div>
        </LiveFrame>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const JSON: Option<&str> = Some("application/json; charset=utf-8");

    fn work_uuid(index: u128) -> PublicWorkId {
        let raw =
            uuid::Uuid::from_u128(0x1000_0000_0000_4000_8000_0000_0000_0000 + index).to_string();
        PublicWorkId::parse(&raw).unwrap()
    }

    fn unit_uuid(work_index: u128, pos: u128) -> String {
        uuid::Uuid::from_u128(0x2000_0000_0000_4000_8000_0000_0000_0000 + (work_index * 16) + pos)
            .to_string()
    }

    fn digest_for(index: u8) -> String {
        format!("{:02x}", index).repeat(32)
    }

    fn sample_summary(index: u128, title: &str) -> SampleWorkSummary {
        let id = work_uuid(index);
        let sha256 = digest_for(index as u8);
        SampleWorkSummary {
            work_id: id.to_string(),
            kind: if index % 2 == 1 {
                WorkKind::Story
            } else {
                WorkKind::Novel
            },
            title: title.to_owned(),
            description: format!("Mô tả bản thảo cho {title}."),
            language: "vi-VN".to_owned(),
            chapter_count: 2,
            cover: Some(SampleCover {
                path: sample_cover_path(id, &sha256),
                sha256,
                content_type: CoverMime::Avif,
                width_px: 1024,
                height_px: 1536,
                byte_length: 4096,
                alt_text: format!("Bìa gốc của {title}"),
            }),
        }
    }

    fn sample_list_fixture() -> SampleWorkList {
        let titles = [
            "Bến Sương Đầu Ngõ",
            "Ngọn Đèn Trước Gió",
            "Mùa Trăng Cũ",
            "Tiếng Chuông Đêm Mưa",
            "Lối Về Bên Sông",
            "Khúc Hát Ngày Xa",
        ];
        SampleWorkList {
            schema: SAMPLE_PREVIEW_SCHEMA.to_owned(),
            dataset_id: SAMPLE_DATASET_ID.to_owned(),
            items: titles
                .into_iter()
                .enumerate()
                .map(|(idx, title)| sample_summary((idx + 1) as u128, title))
                .collect(),
        }
    }

    fn sample_detail_fixture(index: u128, title: &str) -> SampleWorkDetail {
        let work = sample_summary(index, title);
        SampleWorkDetail {
            schema: SAMPLE_PREVIEW_SCHEMA.to_owned(),
            work,
            chapters: vec![
                SampleChapter {
                    unit_id: unit_uuid(index, 1),
                    kind: UnitKind::Chapter,
                    position: 1,
                    title: "Chương 1: Khởi đầu trong sương".to_owned(),
                    description: "Đoạn mở đầu của chương 1.".to_owned(),
                    audio: SampleAudioAvailability::Unavailable,
                },
                SampleChapter {
                    unit_id: unit_uuid(index, 2),
                    kind: UnitKind::Chapter,
                    position: 2,
                    title: "Chương 2: Dư âm cuối phố".to_owned(),
                    description: "Đoạn kết của chương 2.".to_owned(),
                    audio: SampleAudioAvailability::Unavailable,
                },
            ],
        }
    }

    #[test]
    fn sample_cover_admission_requires_exact_same_origin_work_and_digest_and_leaves_public_cover_untouched(
    ) {
        let work = work_uuid(1);
        let summary = sample_summary(1, "Bến Sương Đầu Ngõ");
        let cover = summary.cover.unwrap();
        assert!(is_admitted_sample_cover(work, &cover));

        // Public cover validator still rejects relative HTTP sample paths.
        assert!(!crate::application::public_cover::is_admitted(
            work,
            &cover.to_cover_image()
        ));

        let other = work_uuid(2);
        assert!(!is_admitted_sample_cover(other, &cover));

        for invalid_path in [
            format!("http://127.0.0.1:8080{}", cover.path),
            format!(
                "https://api.cantos.cc/api/v1/theatre/works/{work}/cover/{}",
                cover.sha256
            ),
            format!("/api/v1/studio/works/{work}/artwork"),
            format!("{}?token=secret", cover.path),
            String::new(),
        ] {
            let mut bad = cover.clone();
            bad.path = invalid_path.clone();
            assert!(!is_admitted_sample_cover(work, &bad), "{invalid_path}");
        }
    }

    #[test]
    fn decode_list_and_detail_accept_valid_six_work_and_two_chapter_payloads_and_reject_tampering()
    {
        let list = sample_list_fixture();
        let list_bytes = serde_json::to_vec(&list).unwrap();
        let decoded_list = decode_list(200, JSON, &list_bytes).unwrap();
        assert_eq!(decoded_list.items.len(), 6);

        let work = work_uuid(1);
        let detail = sample_detail_fixture(1, "Bến Sương Đầu Ngõ");
        let detail_bytes = serde_json::to_vec(&detail).unwrap();
        let decoded_detail = decode_detail(200, JSON, &detail_bytes, work).unwrap();
        assert_eq!(decoded_detail.chapters.len(), 2);
        assert_eq!(
            decoded_detail.chapters[0].audio,
            SampleAudioAvailability::Unavailable
        );

        // Wrong schema or dataset is rejected as Protocol.
        let mut wrong_schema = list.clone();
        wrong_schema.schema = "cantos.other.v1".to_owned();
        assert_eq!(
            decode_list(200, JSON, &serde_json::to_vec(&wrong_schema).unwrap()),
            Err(CatalogReadError::Protocol)
        );

        // Missing cover is rejected in the sample preview contract.
        let mut missing_cover = list.clone();
        missing_cover.items[0].cover = None;
        assert_eq!(
            decode_list(200, JSON, &serde_json::to_vec(&missing_cover).unwrap()),
            Err(CatalogReadError::Protocol)
        );

        // Wrong route Work ID or chapter count mismatch is rejected as Protocol.
        assert_eq!(
            decode_detail(200, JSON, &detail_bytes, work_uuid(2)),
            Err(CatalogReadError::Protocol)
        );
        let mut bad_chapters = detail.clone();
        bad_chapters.chapters[1].position = 3;
        assert_eq!(
            decode_detail(200, JSON, &serde_json::to_vec(&bad_chapters).unwrap(), work),
            Err(CatalogReadError::Protocol)
        );

        // Error classifications distinguish 404 NotFound, 503 Unavailable, and 500 Protocol.
        assert_eq!(
            decode_detail(404, JSON, br#"{"code":"not_found"}"#, work),
            Err(CatalogReadError::NotFound)
        );
        assert_eq!(
            decode_detail(
                503,
                JSON,
                br#"{"code":"unavailable","retryable":true}"#,
                work
            ),
            Err(CatalogReadError::Unavailable)
        );
        assert_eq!(
            decode_detail(500, JSON, br#"{"code":"sample_mismatch"}"#, work),
            Err(CatalogReadError::Protocol)
        );
    }

    #[test]
    fn sample_home_and_work_view_render_six_works_two_chapters_original_covers_and_disabled_play_without_audio(
    ) {
        Owner::new().with(|| {
            let locale = RwSignal::new(Locale::Vietnamese);
            let list_state = RwSignal::new(SampleListState::default());

            // 1. Loading state is distinct from Empty and Failed.
            let loading_html = view! {
                <SampleHome state=list_state locale on_retry=Callback::new(|()| ()) on_open_work=Callback::new(|_| ())/>
            }
            .to_html();
            assert!(loading_html.contains("Đang mở những câu chuyện đã công bố…"));
            assert!(!loading_html.contains("theatre-public-work-grid"));

            // 2. Ready state renders all 6 Works and their exact same-origin cover URLs.
            let ticket = list_state.write().ensure_loaded().unwrap();
            assert!(list_state.write().finish(&ticket, Ok(sample_list_fixture())));
            let ready_html = view! {
                <SampleHome state=list_state locale on_retry=Callback::new(|()| ()) on_open_work=Callback::new(|_| ())/>
            }
            .to_html();
            assert_eq!(ready_html.matches("class=\"theatre-public-work theatre-work-card theatre-panel\"").count(), 6);
            for idx in 1..=6 {
                let id = work_uuid(idx);
                let sha = digest_for(idx as u8);
                let expected_cover = sample_cover_path(id, &sha);
                assert!(ready_html.contains(&expected_cover), "missing cover {expected_cover}");
                assert!(ready_html.contains(&format!("href=\"#work/{id}\"")));
            }
            assert!(ready_html.contains("Các chương: 2"));
            assert!(!ready_html.contains("Chương đã công bố"));

            // 3. Work detail renders cover, synopsis, 2 ordered chapters, and disabled Play controls.
            let detail_state = RwSignal::new(SampleDetailState::default());
            let work = work_uuid(1);
            let detail_ticket = detail_state.write().open(work).unwrap();
            assert!(detail_state
                .write()
                .finish(&detail_ticket, Ok(sample_detail_fixture(1, "Bến Sương Đầu Ngõ"))));
            let detail_html = view! {
                <SampleWorkView state=detail_state locale on_back=Callback::new(|()| ()) on_retry=Callback::new(|()| ())/>
            }
            .to_html();
            assert!(detail_html.contains("Bến Sương Đầu Ngõ"));
            assert!(detail_html.contains("Chương 1: Khởi đầu trong sương"));
            assert!(detail_html.contains("Chương 2: Dư âm cuối phố"));
            assert_eq!(detail_html.matches("data-audio=\"unavailable\"").count(), 2);
            assert!(detail_html.contains("Chưa có bản nghe mẫu"));
            assert!(detail_html.contains("Tập minh họa chưa có bản nghe; không thể mở hoặc thêm vào tiếp theo."));
            for pos in [1, 2] {
                let uid = unit_uuid(1, pos);
                let btn_id = format!("id=\"theatre-public-unit-listen-{uid}\"");
                let at = detail_html.find(&btn_id).expect("chapter listen button");
                let tag_end = detail_html[at..].find('>').unwrap();
                let button_open_tag = &detail_html[at..at + tag_end];
                assert!(button_open_tag.contains("disabled"), "{button_open_tag}");
            }
            for forbidden in [
                "<audio",
                "<video",
                "/api/v1/theatre/publications",
                "/api/v1/studio",
                "studio-token",
            ] {
                assert!(!ready_html.contains(forbidden), "{forbidden}");
                assert!(!detail_html.contains(forbidden), "{forbidden}");
            }
        });
    }

    #[test]
    fn sample_states_distinguish_empty_not_found_unavailable_and_protocol_and_support_retry() {
        Owner::new().with(|| {
            let locale = RwSignal::new(Locale::Vietnamese);
            let list_state = RwSignal::new(SampleListState::default());
            let t1 = list_state.write().begin().unwrap();
            list_state
                .write()
                .finish(&t1, Err(CatalogReadError::Unavailable));
            let unavailable_html = view! {
                <SampleHome state=list_state locale on_retry=Callback::new(|()| ()) on_open_work=Callback::new(|_| ())/>
            }
            .to_html();
            assert!(unavailable_html.contains("Chưa mở được danh mục công khai."));
            assert!(unavailable_html.contains("id=\"theatre-home-retry\""));

            let t2 = list_state.write().retry().unwrap();
            list_state.write().finish(
                &t2,
                Ok(SampleWorkList {
                    schema: SAMPLE_PREVIEW_SCHEMA.to_owned(),
                    dataset_id: SAMPLE_DATASET_ID.to_owned(),
                    items: vec![],
                }),
            );
            let empty_html = view! {
                <SampleHome state=list_state locale on_retry=Callback::new(|()| ()) on_open_work=Callback::new(|_| ())/>
            }
            .to_html();
            assert!(empty_html.contains("Chưa có câu chuyện nào"));

            let detail_state = RwSignal::new(SampleDetailState::default());
            let work = work_uuid(1);
            let dt1 = detail_state.write().open(work).unwrap();
            detail_state
                .write()
                .finish(&dt1, Err(CatalogReadError::NotFound));
            let not_found_html = view! {
                <SampleWorkView state=detail_state locale on_back=Callback::new(|()| ()) on_retry=Callback::new(|()| ())/>
            }
            .to_html();
            assert!(not_found_html.contains("Tác phẩm này hiện không mở công khai."));

            let dt2 = detail_state.write().open(work).unwrap();
            detail_state
                .write()
                .finish(&dt2, Err(CatalogReadError::Protocol));
            let protocol_html = view! {
                <SampleWorkView state=detail_state locale on_back=Callback::new(|()| ()) on_retry=Callback::new(|()| ())/>
            }
            .to_html();
            assert!(protocol_html.contains("Chưa đọc được thông tin tác phẩm."));
            assert!(protocol_html.contains("id=\"theatre-public-work-retry\""));
        });
    }
}
