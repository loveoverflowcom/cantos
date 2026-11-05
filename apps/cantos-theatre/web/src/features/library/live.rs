//! Web-owned Library composition over the full presented read model, separate from demo data.
use super::super::common::{EmptyArtwork, EmptyArtworkKind};
use crate::contract::library::{
    AudioIdentity, AudioPresentation, Completion, ResumeStatus, WorkPresentation,
};
use crate::features::loading::{PendingRows, RequestEdge};
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountFailure, AccountViewState, SessionPhase},
        library::{LibraryProjection, PagePhase, Section},
        library_sync::{
            HistoryCommandPhase, ProgressCommandPhase, ProgressConflict, ResumeAttemptState,
            ResumeRefusal, SavedCommandPhase,
        },
    },
    core::public_ids::PublicWorkId,
    messages::{self, Key, Locale},
};
use cantos_ui::{Button, ButtonState, ButtonVariant, DisabledReason, LoadingState};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct LibraryActions {
    pub on_resume: Callback<(AudioIdentity, u64, ResumeStatus, Option<AudioPresentation>)>,
    pub on_unsave: Callback<(String, Option<WorkPresentation>)>,
    pub on_retry_saved: Callback<String>,
    pub on_prompt_clear_history: Callback<()>,
    pub on_confirm_clear_history: Callback<()>,
    pub on_cancel_clear_history: Callback<()>,
    pub on_retry_history: Callback<()>,
    pub on_resolve_progress: Callback<(String, bool)>,
    pub on_retry_progress: Callback<String>,
}

fn clock(locale: Locale, milliseconds: u64) -> String {
    messages::clock_seconds(locale, milliseconds / 1000)
}
fn heading(section: Section) -> Key {
    match section {
        Section::Continue => Key::Continue,
        Section::Saved => Key::Saved,
        Section::History => Key::History,
    }
}
fn empty(section: Section) -> Key {
    match section {
        Section::Continue => Key::LiveLibraryEmptyContinue,
        Section::Saved => Key::LiveLibraryEmptySaved,
        Section::History => Key::LiveLibraryEmptyHistory,
    }
}
fn availability(resume: ResumeStatus) -> Key {
    match resume {
        ResumeStatus::ExactAvailable => Key::LiveResumeExact,
        ResumeStatus::ReleaseReplaced => Key::LiveResumeReplaced,
        ResumeStatus::AccessUnavailable => Key::LiveResumeUnavailable,
    }
}
mod rows;

fn row_count(projection: &LibraryProjection, section: Section) -> usize {
    match section {
        Section::Continue => projection.continuing.len(),
        Section::Saved => projection.saved.len(),
        Section::History => projection.history.len(),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
struct SectionUi {
    phase: PagePhase,
    count: usize,
    has_more: bool,
    appending: bool,
}

impl SectionUi {
    fn is_empty(&self) -> bool {
        self.phase == PagePhase::Ready && self.count == 0 && !self.has_more
    }
}

fn all_empty(state: &AccountViewState) -> bool {
    Section::ALL
        .into_iter()
        .all(|section| section_ui(state, section).is_some_and(|ui| ui.is_empty()))
}

fn section_ui(state: &AccountViewState, section: Section) -> Option<SectionUi> {
    let projection = state.library()?;
    Some(SectionUi {
        phase: projection.phase(section),
        count: row_count(projection, section),
        has_more: projection.has_more(section),
        appending: projection.appending(section),
    })
}

fn status(ui: Option<&SectionUi>, section: Section, locale: Locale) -> String {
    let Some(ui) = ui else {
        return String::new();
    };
    let key = match ui.phase {
        PagePhase::Idle | PagePhase::Loading => Key::LiveLibraryLoading,
        PagePhase::Failed(AccountFailure::Diagnostic(kind)) => crate::messages::error_key(kind),
        PagePhase::Failed(AccountFailure::CursorExpired) => Key::LiveLibraryCursorExpired,
        PagePhase::Failed(AccountFailure::Protocol) => Key::LiveLibraryInvalidPage,
        PagePhase::Failed(AccountFailure::RateLimited(_)) => Key::AccountRateLimited,
        PagePhase::Failed(_) => Key::LiveLibraryPageError,
        PagePhase::Ready if ui.count == 0 && ui.has_more => Key::LiveLibraryMore,
        PagePhase::Ready if ui.is_empty() => {
            return messages::text(empty(section), locale).to_owned()
        }
        PagePhase::Ready => {
            return messages::message(
                locale,
                crate::localization::generated::Message::TheatreShellLiveLibraryLoaded {
                    arg_count: (ui.count) as u64,
                },
            )
        }
    };
    messages::text(key, locale).to_owned()
}
pub(crate) fn status_id(section: Section) -> String {
    format!("theatre-library-{}-status", section.name())
}

fn history_clear_controls(
    history: &HistoryUi,
    actions: Option<LibraryActions>,
    locale: Locale,
) -> AnyView {
    let Some(act) = actions else {
        return ().into_any();
    };
    let confirming = history.confirming;
    let cmd = history.command.clone();
    let has_rows = history.has_rows;
    if !confirming && matches!(cmd, HistoryCommandPhase::Idle) {
        return has_rows
            .then(|| {
                view! {
                    <Button id="theatre-library-history-clear" variant=ButtonVariant::Text
                        on_press=Callback::new(move |_| act.on_prompt_clear_history.run(()))>
                        {messages::text(Key::LiveClearHistory, locale)}
                    </Button>
                }
            })
            .into_any();
    }
    let pending = matches!(cmd, HistoryCommandPhase::PendingClear { .. });
    let submit_state = Signal::derive(move || {
        if pending {
            DisabledReason::new(messages::text(Key::LiveClearHistoryPending, locale))
                .map(ButtonState::Loading)
                .unwrap_or_default()
        } else {
            ButtonState::Enabled
        }
    });
    let feedback = match &cmd {
        HistoryCommandPhase::ClearConflict { .. } => Some((Key::LiveClearHistoryConflict, false)),
        HistoryCommandPhase::UnknownOutcome { .. } => Some((Key::LiveHistoryCommandUnknown, true)),
        HistoryCommandPhase::Failed { .. } => Some((Key::LiveHistoryCommandError, true)),
        HistoryCommandPhase::Idle
        | HistoryCommandPhase::PendingStart { .. }
        | HistoryCommandPhase::PendingClear { .. } => None,
    };
    view! {
        <div id="theatre-library-history-clear-confirm" class="theatre-panel" role="region" aria-labelledby="theatre-library-history-clear-title">
            <h3 id="theatre-library-history-clear-title">{messages::text(Key::LiveClearHistoryConfirmTitle, locale)}</h3>
            <p class="theatre-muted">{messages::text(Key::LiveClearHistoryHelp, locale)}</p>
            {feedback.map(|(msg_key, can_retry)| view! {
                <p id="theatre-library-history-clear-status" class="theatre-muted" role="status">{messages::text(msg_key, locale)}</p>
                {can_retry.then(|| view! {
                    <Button id="theatre-library-history-command-retry" variant=ButtonVariant::Text
                        on_press=Callback::new(move |_| act.on_retry_history.run(()))>
                        {messages::text(Key::PublicRetry, locale)}
                    </Button>
                })}
            })}
            {confirming.then(|| view! {
                <div class="theatre-actions">
                    <Button id="theatre-library-history-clear-submit" variant=ButtonVariant::Tonal state=submit_state
                        on_press=Callback::new(move |_| act.on_confirm_clear_history.run(()))>
                        {messages::text(Key::LiveConfirmClearHistory, locale)}
                    </Button>
                    <Button id="theatre-library-history-clear-cancel" variant=ButtonVariant::Text
                        on_press=Callback::new(move |_| act.on_cancel_clear_history.run(()))>
                        {messages::text(Key::Cancel, locale)}
                    </Button>
                </div>
            })}
        </div>
    }
    .into_any()
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct HistoryUi {
    confirming: bool,
    command: HistoryCommandPhase,
    has_rows: bool,
}

fn history_ui(state: &AccountViewState) -> Option<HistoryUi> {
    let projection = state.library()?;
    if projection.phase(Section::History) != PagePhase::Ready {
        return None;
    }
    let sync = state.sync()?;
    Some(HistoryUi {
        confirming: sync.clear_confirmation().is_some(),
        command: sync.history_command().clone(),
        has_rows: !projection.history.is_empty(),
    })
}

#[component]
fn LiveSection(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    section: Section,
    on_page: Callback<(Section, bool, bool)>,
    actions: Option<LibraryActions>,
) -> impl IntoView {
    let heading_id = crate::platform::library_heading_id(section);
    let list = NodeRef::<leptos::html::Ul>::new();
    let membership = Memo::new(move |_| state.with(|s| rows::membership(s, section)));
    let chrome = Memo::new(move |_| state.with(|s| section_ui(s, section)));
    let history = Memo::new(move |_| {
        if section == Section::History {
            state.with(history_ui)
        } else {
            None
        }
    });
    let phase = move || chrome.with(|s| s.as_ref().map(|s| s.phase));
    let restart = move || {
        matches!(
            phase(),
            Some(PagePhase::Failed(
                AccountFailure::CursorExpired | AccountFailure::Protocol
            ))
        )
    };
    view! {
        <section class="theatre-library-section" aria-labelledby=heading_id.clone()
            data-empty=move || (chrome.with(|ui| ui.as_ref().is_some_and(SectionUi::is_empty))
                && history.with(|h| h.as_ref().is_none_or(|h| !h.confirming && matches!(h.command, HistoryCommandPhase::Idle)))).to_string()>
            <h2 id=heading_id.clone() tabindex="-1" class="theatre-section-heading">{move || messages::text(heading(section), locale.get())}</h2>
            <RequestEdge active=Signal::derive(move || chrome.with(|ui| ui.as_ref().is_some_and(|ui| ui.phase == PagePhase::Loading && ui.count > 0 && !ui.appending))) label=Key::LiveLibraryLoading locale/>
            <Show when=move || chrome.with(|ui| ui.as_ref().is_some_and(|ui| matches!(ui.phase, PagePhase::Idle | PagePhase::Loading) && ui.count == 0))><PendingRows compact=true/></Show>
            <ul node_ref=list class="theatre-rows" aria-busy=move || (phase() == Some(PagePhase::Loading)).to_string()>
                <For each=move || {
                    let ids = membership.get();
                    crate::platform::preserve_library_row_interaction(list);
                    ids
                } key=|id| id.clone()
                    children=move |work_id| view! { <rows::LiveRow state work_id section actions locale/> }/>
            </ul>
            <Show when=move || chrome.with(|ui| ui.as_ref().is_some_and(|ui| ui.appending))><LoadingState>{move || messages::text(Key::DiscoveryLoadingMore, locale.get())}</LoadingState></Show>
            <div id=status_id(section) tabindex="-1" role="status" aria-live="polite"
                class="theatre-library-feedback" data-empty=move || chrome.with(|ui| ui.as_ref().is_some_and(SectionUi::is_empty)).to_string()>
                <div>
                    <p>{move || chrome.with(|ui| status(ui.as_ref(), section, locale.get()))}</p>
                </div>
            </div>
            {move || history.with(|h| h.as_ref().map(|h| history_clear_controls(h, actions, locale.get())))}
            <div class="theatre-actions">
                <Show when=move || matches!(phase(), Some(PagePhase::Failed(_)))>
                    <Button id=format!("theatre-library-{}-retry", section.name()) variant=ButtonVariant::Text on_press=Callback::new(move |_| on_page.run((section, restart(), true)))>
                        {move || messages::text(if restart() { Key::LiveLibraryRestart } else { Key::LiveLibraryRetry }, locale.get())}
                    </Button>
                </Show>
                <Show when=move || chrome.with(|s| s.as_ref().is_some_and(|s| s.phase == PagePhase::Ready && s.has_more))>
                    <Button id=format!("theatre-library-{}-more", section.name()) variant=ButtonVariant::Tonal on_press=Callback::new(move |_| on_page.run((section, false, true)))>
                        {move || messages::message(locale.get(), crate::localization::generated::Message::TheatreShellLiveLibraryLoadMore { arg_section: messages::text(heading(section), locale.get()) })}
                    </Button>
                </Show>

            </div>
        </section>
    }
}
#[component]
pub(crate) fn LiveLibrary(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    on_page: Callback<(Section, bool, bool)>,
    #[prop(optional)] actions: Option<LibraryActions>,
    #[prop(optional, into)] shared_section: Option<ViewFn>,
    #[prop(default = Signal::derive(|| false), into)] shared_has_rows: Signal<bool>,
) -> impl IntoView {
    let phase = Memo::new(move |_| state.with(AccountViewState::phase));
    let invitation = Memo::new(move |_| state.with(all_empty) && !shared_has_rows.get());
    view! {
        <section id="library" aria-labelledby="theatre-live-library-heading">
            <h1 id="theatre-live-library-heading">{move || messages::text(Key::Library, locale.get())}</h1>
            <Show when=move || phase.get() == SessionPhase::SignedIn fallback=move || view! {
                <p role="status">{move || messages::text(match phase.get() {
                    SessionPhase::Idle | SessionPhase::Checking => Key::AccountChecking,
                    SessionPhase::Failed(failure) => crate::features::account::account_failure_key(failure),
                    SessionPhase::LogoutPending => Key::AccountLogoutRetryHelp,
                    SessionPhase::Guest | SessionPhase::SignedIn => Key::LiveLibrarySignedOut,
                }, locale.get())}</p>
            }>
                <p class="theatre-muted theatre-library-deck">{move || messages::text(Key::LibraryDeck, locale.get())}</p>
                <Show when=move || invitation.get()>
                    <div class="theatre-panel theatre-library-invitation" aria-labelledby="theatre-library-empty-title">
                        <EmptyArtwork kind=EmptyArtworkKind::Library/>
                        <div>
                            <h2 id="theatre-library-empty-title">{move || messages::text(Key::LiveLibraryEmptyTitle, locale.get())}</h2>
                            <p class="theatre-muted">{move || messages::text(Key::LiveLibraryEmptyHelp, locale.get())}</p>
                            <a class="theatre-action" href="#discover"><cantos_ui::Icon name=cantos_ui::IconName::Search/>{move || messages::text(Key::LiveLibraryBrowse, locale.get())}</a>
                        </div>
                    </div>
                </Show>
                <div class="theatre-library-sections">
                    {Section::ALL.into_iter().map(|section| view! { <LiveSection state locale section on_page actions/> }).collect_view()}
                </div>
                {shared_section.as_ref().map(ViewFn::run)}
            </Show>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{account::AccountScope, account_view::SessionCompletion};
    pub(super) fn signed_in() -> AccountViewState {
        let mut s = AccountViewState::default();
        let issued = s.begin_refresh().unwrap();
        assert!(matches!(
            s.finish_session(
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
        s
    }
    pub(super) fn populate(s: &mut AccountViewState) {
        for (section, body) in [
            (Section::Continue, include_bytes!("../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_continue.json").as_slice()),
            (Section::Saved, include_bytes!("../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_saved.json").as_slice()),
            (Section::History, include_bytes!("../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_history.json").as_slice()),
        ] {
            let request = s.begin_library(section, false).unwrap();
            s.finish_page(&request, crate::live::library::decode_page(section, body)).unwrap();
        }
    }
    fn render(s: AccountViewState, language: Locale) -> String {
        Owner::new().with(|| view! {
            <LiveLibrary state=RwSignal::new(s).read_only() locale=RwSignal::new(language) on_page=Callback::new(|_| {})/>
        }.to_html())
    }
    #[test]
    fn append_feedback_follows_the_retained_rows_and_ends_on_failure() {
        let mut state = signed_in();
        let first = state.begin_library(Section::Saved, false).unwrap();
        let mut page: crate::contract::library::PresentedSavedPage = serde_json::from_slice(include_bytes!("../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_saved.json")).unwrap();
        page.next_cursor = Some("ab".repeat(32));
        state
            .finish_page(
                &first,
                Ok(crate::application::library::LibraryPage::Saved(page)),
            )
            .unwrap();
        let append = state.begin_library(Section::Saved, false).unwrap();
        let html = render(state.clone(), Locale::English);
        let section = html.split_once("theatre-library-saved-heading").unwrap().1;
        let list_end = section.find("</ul>").unwrap();
        let indicator = section.find("cui-loading-state").unwrap();
        assert!(
            indicator > list_end,
            "append must not insert content above the retained list: {section}"
        );
        state
            .finish_page(&append, Err(AccountFailure::Unavailable))
            .unwrap();
        let html = render(state, Locale::English);
        assert!(!html.contains("cui-loading-indicator"));
        assert!(html.contains("Ánh đèn cuối sân khấu"));
    }

    #[test]
    fn three_real_sections_render_release_metadata_and_uuid_links_without_mock_or_play_effects() {
        let mut s = signed_in();
        populate(&mut s);
        for language in Locale::supported() {
            let html = render(s.clone(), language);
            assert_eq!(html.matches("<h1").count(), 1);
            for section in Section::ALL {
                let heading = format!("theatre-library-{}-heading", section.name());
                assert!(html.contains(&format!("aria-labelledby=\"{heading}\"")));
                assert!(html.contains(&format!("id=\"{heading}\" tabindex=\"-1\"")));
            }
            assert_eq!(
                html.matches("Ánh đèn cuối sân khấu — Một lời hẹn chưa nói")
                    .count(),
                3
            );
            assert_eq!(
                html.matches("href=\"#work/00000000-0000-4000-8000-000000000001\"")
                    .count(),
                3
            );
            assert!(html.contains("0:15") && html.contains("1:00"));
            for forbidden in [
                "<audio",
                "#work/river",
                "Sơn Hà Ký",
                "Vị trí nghe tiếp 1",
                "access_token",
                "<dd>",
            ] {
                assert!(!html.contains(forbidden), "{forbidden}");
            }
        }
        s.begin_logout().unwrap();
        let html = render(s, Locale::English);
        assert!(html.contains("Private data is hidden"));
        assert!(!html.contains("Ánh đèn"));
    }
    #[test]
    fn null_and_replaced_rows_never_borrow_other_metadata_and_failures_stay_section_local() {
        let mut s = signed_in();
        let request = s.begin_library(Section::Continue, false).unwrap();
        let mut page: crate::contract::library::PresentedContinuePage = serde_json::from_slice(include_bytes!("../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_continue_unavailable.json")).unwrap();
        page.items[0].resume = ResumeStatus::ReleaseReplaced;
        s.finish_page(
            &request,
            Ok(crate::application::library::LibraryPage::Continue(page)),
        )
        .unwrap();
        let saved = s.begin_library(Section::Saved, false).unwrap();
        s.finish_page(&saved, Err(AccountFailure::Unavailable))
            .unwrap();
        let history = s.begin_library(Section::History, false).unwrap();
        s.finish_page(
            &history,
            Ok(crate::application::library::LibraryPage::History(
                crate::contract::library::PresentedHistoryPage {
                    state: crate::contract::library::HistoryState {
                        epoch: 0,
                        revision: 0,
                    },
                    items: vec![],
                    next_cursor: None,
                },
            )),
        )
        .unwrap();
        let html = render(s, Locale::English);
        assert!(html.contains("Story details are currently unavailable"));
        assert!(html.contains("listening edition was replaced"));
        assert!(html.contains("theatre-library-saved-retry"));
        assert!(html.contains("No listening history"));
        assert!(!html.contains("href=\"#work/"));
        assert!(!html.contains("Ánh đèn"));
    }

    #[test]
    fn library_actions_render_resume_unsave_conflict_resolution_and_clear_history_dialog() {
        let mut s = signed_in();
        populate(&mut s);

        let dummy_actions = LibraryActions {
            on_resume: Callback::new(|_| {}),
            on_unsave: Callback::new(|_| {}),
            on_retry_saved: Callback::new(|_| {}),
            on_resolve_progress: Callback::new(|_| {}),
            on_retry_progress: Callback::new(|_| {}),
            on_prompt_clear_history: Callback::new(|_| {}),
            on_confirm_clear_history: Callback::new(|_| {}),
            on_cancel_clear_history: Callback::new(|_| {}),
            on_retry_history: Callback::new(|_| {}),
        };

        let render_with_actions = |state: AccountViewState, locale: Locale| -> String {
            Owner::new().with(|| {
                view! {
                    <LiveLibrary
                        state=RwSignal::new(state).read_only()
                        locale=RwSignal::new(locale)
                        on_page=Callback::new(|_| {})
                        actions=dummy_actions
                    />
                }
                .to_html()
            })
        };

        // Initial populated state with actions offers resume on Continue/History, unsave on Saved, and clear history prompt.
        let html = render_with_actions(s.clone(), Locale::Vietnamese);
        assert!(html.contains("theatre-library-continue-resume-"));
        assert!(html.contains("theatre-library-saved-unsave-00000000-0000-4000-8000-000000000001"));
        assert!(html.contains("id=\"theatre-library-history-clear\""));
        assert!(html.contains("Xóa lịch sử nghe"));

        // Prompting clear history renders the confirmation region with confirm and cancel buttons.
        s.seed_sync(|sync, _| {
            sync.prompt_clear_history();
        });
        let confirm_html = render_with_actions(s, Locale::Vietnamese);
        assert!(confirm_html.contains("id=\"theatre-library-history-clear-confirm\""));
        assert!(confirm_html.contains("id=\"theatre-library-history-clear-submit\""));
        assert!(confirm_html.contains("id=\"theatre-library-history-clear-cancel\""));
    }
}

#[cfg(test)]
mod empty_tests;
