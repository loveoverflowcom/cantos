//! Live public Work detail. Server text only: no cover art, genre, credit, count or playback.
//! Playback stays explicitly unavailable until a real delivery and engine exist.
use crate::{
    application::public_work::{
        PageStatus, ReadFailure, UnitList, WorkDetailPhase, WorkDetailState,
    },
    features::{
        auth_copy::{self, CopyKey},
        common::clock,
    },
    messages::{self, text, Key, Locale},
};
use cantos_contracts::theatre::{UnitKind, UnitSummary, WorkKind, WorkSummary};
use cantos_ui::{AccessibleName, BackButton, Button, ButtonState, ButtonVariant, DisabledReason};
use leptos::prelude::*;

pub(crate) const HEADING_ID: &str = "theatre-public-work-heading";
pub(crate) const UNITS_STATUS_ID: &str = "theatre-public-units-status";
pub(crate) const UNITS_RETRY_ID: &str = "theatre-public-units-retry";
pub(crate) const UNITS_MORE_ID: &str = "theatre-public-units-more";

pub(crate) fn unit_heading_id(unit_id: &str) -> String {
    format!("theatre-public-unit-{unit_id}")
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

#[component]
pub(crate) fn PublicWorkDetail(
    state: RwSignal<WorkDetailState>,
    locale: RwSignal<Locale>,
    on_back: Callback<()>,
    on_retry: Callback<()>,
    on_load_more: Callback<()>,
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
                    <p class="theatre-public-state" role="status">{move || text(Key::PublicWorkLoading, locale.get())}</p>
                }.into_any(),
                Shape::NotFound => view! {
                    <div class="theatre-public-state theatre-panel">
                        <h1 id=HEADING_ID tabindex="-1">{move || text(Key::PublicWorkNotFound, locale.get())}</h1>
                        <p>{move || text(Key::PublicWorkNotFoundHelp, locale.get())}</p>
                        <a class="theatre-text-link" href="#discover">{move || auth_copy::text(CopyKey::ReturnHome, locale.get())}</a>
                    </div>
                }.into_any(),
                Shape::Failed(failure) => {
                    let (heading, help) = match failure {
                        ReadFailure::Unavailable => (Key::PublicWorkUnavailable, Key::PublicConnectionUnavailableHelp),
                        ReadFailure::Protocol => (Key::PublicWorkProtocol, Key::PublicWorkProtocolHelp),
                    };
                    view! {
                        <div class="theatre-public-state theatre-panel" role="status">
                            <h1 id=HEADING_ID tabindex="-1">{move || text(heading, locale.get())}</h1>
                            <p>{move || text(help, locale.get())}</p>
                            <Button id="theatre-public-work-retry" variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |_| on_retry.run(()))>
                                {move || text(Key::PublicRetry, locale.get())}
                            </Button>
                        </div>
                    }.into_any()
                }
                Shape::Ready(_) => state
                    .with_untracked(|s| match s.phase() {
                        WorkDetailPhase::Ready { work, .. } => Some(work.clone()),
                        _ => None,
                    })
                    .map(|work| view! { <ReadyWork state locale work on_retry on_load_more/> }.into_any())
                    .unwrap_or_else(|| ().into_any()),
            }}
        </section>
    }
}

#[component]
fn ReadyWork(
    state: RwSignal<WorkDetailState>,
    locale: RwSignal<Locale>,
    work: WorkSummary,
    on_retry: Callback<()>,
    on_load_more: Callback<()>,
) -> impl IntoView {
    let kind = work.kind;
    let lang = work.language.clone();
    let row_lang = StoredValue::new(work.language.clone());
    let (list_heading, more_label) = match kind {
        WorkKind::Podcast => (Key::PublicWorkEpisodes, Key::PublicWorkMoreEpisodes),
        WorkKind::Novel | WorkKind::Story => (Key::PublicWorkChapters, Key::PublicWorkMoreChapters),
    };
    let rows = move || {
        state
            .with(|s| {
                with_units(s, |units| {
                    units
                        .rows()
                        .iter()
                        .map(|row| (row.clone(), units.is_unavailable(&row.unit_id)))
                        .collect::<Vec<_>>()
                })
            })
            .unwrap_or_default()
    };
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
    let listen_state = Signal::derive(move || {
        DisabledReason::new(text(Key::PublicWorkListenUnavailable, locale.get()))
            .map(ButtonState::Disabled)
            .unwrap_or_default()
    });
    let description = (!work.description.trim().is_empty()).then(|| work.description.clone());
    view! {
        <header class="theatre-work-intro theatre-public-work-intro">
            <div class="theatre-cover" aria-hidden="true">
                <span class="theatre-cover-moon"></span>
                <strong lang=lang.clone()>{work.title.clone()}</strong>
                <small>{move || text(messages::work_kind_key(kind), locale.get())}</small>
            </div>
            <div>
                <p class="theatre-eyebrow">{move || text(messages::work_kind_key(kind), locale.get())}</p>
                <h1 id=HEADING_ID tabindex="-1" lang=lang.clone()>{work.title.clone()}</h1>
                {description.map(|text| view! { <p class="theatre-synopsis" lang=lang.clone()>{text}</p> })}
                <Button id="theatre-public-work-listen" state=listen_state>
                    {move || text(Key::PublicWorkListen, locale.get())}
                </Button>
            </div>
        </header>
        <section class="theatre-public-units" aria-labelledby="theatre-public-units-heading"
            aria-busy=move || matches!(status(), Some(PageStatus::Loading)).then_some("true")>
            <h2 id="theatre-public-units-heading" class="theatre-section-heading">{move || text(list_heading, locale.get())}</h2>
            <ol class="theatre-episode-list theatre-public-unit-list">
                <For each=rows key=|(row, unavailable)| (row.unit_id.clone(), row.publication_id.clone(), *unavailable)
                    children=move |(row, unavailable)| view! { <UnitRow row unavailable lang=row_lang.get_value() locale/> }/>
            </ol>
            {move || first_page_failure().map(|failure| {
                let help = match failure {
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

#[component]
fn UnitRow(
    row: UnitSummary,
    unavailable: bool,
    lang: String,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let kind = unit_label(row.kind);
    let seconds = u32::try_from(row.duration_ms / 1000).unwrap_or(u32::MAX);
    let known = row.duration_ms > 0;
    let description = (!row.description.trim().is_empty()).then(|| row.description.clone());
    view! {
        <li class="theatre-episode-row theatre-public-unit" data-unit-id=row.unit_id.clone()
            data-unavailable=unavailable.then_some("true")>
            <div class="theatre-episode-copy">
                <p class="theatre-eyebrow">{move || text(kind, locale.get())}</p>
                <h3 id=unit_heading_id(&row.unit_id) tabindex="-1" lang=lang.clone()>{row.title.clone()}</h3>
                {description.map(|text| view! { <p class="theatre-muted" lang=lang.clone()>{text}</p> })}
                <p class="theatre-muted theatre-public-unit-duration">
                    {move || if known {
                        view! {
                            <span aria-hidden="true">{clock(seconds)}</span>
                            <span class="cui-sr-only">{messages::format(Key::PublicWorkDurationDescription, locale.get(), &[("duration", messages::spoken_time(locale.get(), seconds))])}</span>
                        }.into_any()
                    } else {
                        text(Key::PublicWorkDurationUnknown, locale.get()).into_any()
                    }}
                </p>
                {unavailable.then(|| view! { <p class="theatre-muted">{move || text(Key::PublicWorkRowUnavailable, locale.get())}</p> })}
            </div>
        </li>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::public_work::{CatalogReadError, Step},
        core::public_ids::PublicWorkId,
    };
    use cantos_contracts::theatre::{UnitPage, WorkPage};

    const WORK: &str = "10000000-0000-4000-8000-000000000001";

    fn fixture_work() -> WorkSummary {
        let page: WorkPage = serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/theatre/discovery/v1/works.json"
        ))
        .unwrap();
        page.items.into_iter().next().unwrap()
    }
    fn fixture_units() -> UnitPage {
        serde_json::from_str(include_str!(
            "../../../../../packages/contracts/fixtures/theatre/discovery/v1/units.json"
        ))
        .unwrap()
    }
    fn unit(n: u128, title: &str) -> UnitSummary {
        UnitSummary {
            unit_id: uuid::Uuid::from_u128(0x2000_0000_0000_4000_8000_0000_0000_0000 + n)
                .to_string(),
            kind: UnitKind::Chapter,
            position: 7,
            title: title.into(),
            description: "Một đoạn mô tả đã công bố.".into(),
            publication_id: uuid::Uuid::from_u128(0x8000_0000_0000_4000_8000_0000_0000_0000 + n)
                .to_string(),
            published_at_ms: 5_000,
            duration_ms: 754_000,
        }
    }
    fn page(items: Vec<UnitSummary>, next: Option<&str>) -> UnitPage {
        UnitPage {
            work_id: WORK.into(),
            items,
            next_cursor: next.map(str::to_owned),
        }
    }
    /// A ready Work with its page-one request in flight.
    fn opened(work: WorkSummary) -> (WorkDetailState, crate::application::public_work::PageTicket) {
        let mut state = WorkDetailState::default();
        let ticket = state.open(PublicWorkId::parse(WORK).unwrap()).unwrap();
        let Step::Continue(first) = state.finish_work(&ticket, Ok(work)) else {
            panic!("page one follows the detail")
        };
        (state, first)
    }
    fn render(state: WorkDetailState, locale: Locale) -> String {
        Owner::new().with(|| {
            view! {
                <PublicWorkDetail state=RwSignal::new(state) locale=RwSignal::new(locale)
                    on_back=Callback::new(|()| ()) on_retry=Callback::new(|()| ())
                    on_load_more=Callback::new(|()| ())/>
            }
            .to_html()
        })
    }
    fn assert_no_fabrication(html: &str) {
        for absent in [
            "<audio",
            "<video",
            "<img",
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

    #[test]
    fn ready_detail_renders_server_text_rows_duration_and_disabled_listen_reason() {
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
            "aria-describedby=\"theatre-public-work-listen-reason\"",
            "Cantos Web chưa phát được âm thanh.",
        ] {
            assert!(html.contains(expected), "{expected}");
        }
        assert!(html.contains("id=\"theatre-public-work-listen\"") && html.contains("disabled"));
        assert!(
            !html.contains(UNITS_MORE_ID),
            "a complete list offers no Load more"
        );
        assert_no_fabrication(&html);
    }

    #[test]
    fn loading_not_found_and_failures_are_distinct_states_without_mock_fallback() {
        let mut loading = WorkDetailState::default();
        let ticket = loading.open(PublicWorkId::parse(WORK).unwrap()).unwrap();
        let html = render(loading.clone(), Locale::Vietnamese);
        assert!(html.contains("Đang mở tác phẩm…") && html.contains("aria-busy=\"true\""));

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
        episode.duration_ms = 0;
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
        assert!(html.contains(">Episode<") && html.contains(">Podcast<"));
        assert!(html.contains("Duration not known yet") && !html.contains("0:00"));
        assert_eq!(html.matches("data-unit-id=").count(), 1);
        assert!(!html.contains("Episode 1"));
    }

    #[test]
    fn copy_is_complete_and_nonblank_in_both_locales() {
        for locale in Locale::ALL {
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
                Key::PublicWorkDurationDescription,
                Key::PublicWorkDurationUnknown,
                Key::PublicWorkRowUnavailable,
            ] {
                assert!(!text(key, locale).trim().is_empty(), "{key:?}");
            }
        }
    }

    #[test]
    fn both_locales_preserve_content_and_map_each_read_failure_and_duration() {
        for (locale, kind, chapters, duration, listen_reason, not_found, unavailable, protocol) in [
            (
                Locale::Vietnamese,
                ">Truyện<",
                "Các chương",
                "Thời lượng: 12 phút 34 giây",
                "Cantos Web chưa phát được âm thanh.",
                "Tác phẩm này hiện không mở công khai.",
                "Chưa mở được tác phẩm.",
                "Chưa đọc được thông tin tác phẩm.",
            ),
            (
                Locale::English,
                ">Story<",
                "Chapters",
                "Duration: 12 minutes 34 seconds",
                "Cantos Web cannot play audio yet.",
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
                listen_reason,
                "Lời hẹn {time} &lt;cuối&gt;",
                "Tập {duration}",
                "lang=\"vi-VN\"",
                "aria-describedby=\"theatre-public-work-listen-reason\"",
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
}
