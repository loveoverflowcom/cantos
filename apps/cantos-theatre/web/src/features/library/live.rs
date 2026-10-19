//! Web-owned Library composition over the full presented read model, separate from demo data.
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountFailure, AccountViewState, SessionPhase},
        library::{LibraryProjection, PagePhase, Section},
        library_sync::{
            HistoryCommandPhase, LibrarySyncState, ProgressCommandPhase, ProgressConflict,
            ResumeAttemptState, ResumeRefusal, SavedCommandPhase,
        },
    },
    core::public_ids::PublicWorkId,
    messages::{self, Key, Locale},
};
use cantos_contracts::listener_library::{
    AudioIdentity, AudioPresentation, Completion, ResumeStatus, WorkPresentation,
};
use cantos_ui::{Button, ButtonState, ButtonVariant, DisabledReason};
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

fn clock(milliseconds: u64) -> String {
    let seconds = milliseconds / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
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
fn title(presentation: Option<WorkPresentation>, locale: Locale) -> AnyView {
    match presentation.and_then(|work| PublicWorkId::parse(&work.work_id).map(|id| (work, id))) {
        Some((work, id)) => view! {
            <h3 class="theatre-row-label" lang=work.language><a href=Route::PublicWork(id).hash()>{work.title}</a></h3>
        }.into_any(),
        None => view! { <h3 class="theatre-row-label">{messages::text(Key::LiveLibraryMetadataUnavailable, locale)}</h3> }.into_any(),
    }
}
#[allow(clippy::too_many_arguments)]
fn audio_row(
    section: Section,
    audio: &AudioIdentity,
    position: Option<u64>,
    completion: Option<Completion>,
    resume: ResumeStatus,
    presentation: Option<AudioPresentation>,
    sync: Option<&LibrarySyncState>,
    actions: Option<LibraryActions>,
    locale: Locale,
) -> AnyView {
    let unit = presentation
        .as_ref()
        .map(|p| (p.unit_title.clone(), p.work.language.clone()));
    let unit_title_for_sr = presentation
        .as_ref()
        .map(|p| p.unit_title.clone())
        .unwrap_or_default();
    let work = presentation.as_ref().map(|p| p.work.clone());
    let time = position.map_or_else(
        || messages::text(Key::LiveLibraryNoPosition, locale).to_owned(),
        |position| {
            let base = messages::format(
                Key::LivePosition,
                locale,
                &[
                    ("position", clock(position)),
                    ("total", clock(audio.duration_ms)),
                ],
            );
            if completion == Some(Completion::Completed) {
                format!(
                    "{base} · {}",
                    messages::text(Key::LiveResumeCompleted, locale)
                )
            } else {
                base
            }
        },
    );
    let resume_feedback = sync.and_then(|s| match s.resume_attempt() {
        ResumeAttemptState::Restored {
            publication_id,
            position_ms,
        } if publication_id == &audio.publication_id => Some(messages::format(
            Key::LiveResumeRestored,
            locale,
            &[("time", clock(u64::from(*position_ms)))],
        )),
        ResumeAttemptState::Refused {
            publication_id,
            reason,
        } if publication_id == &audio.publication_id => {
            let key = match reason {
                ResumeRefusal::ReleaseReplaced => Key::LiveResumeReplaced,
                ResumeRefusal::AccessUnavailable => Key::LiveResumeUnavailable,
                ResumeRefusal::LoadFailed | ResumeRefusal::Protocol => Key::LiveResumeLoadFailed,
            };
            Some(messages::text(key, locale).to_owned())
        }
        _ => None,
    });
    let progress_command = sync
        .and_then(|s| s.progress_for(&audio.publication_id))
        .map(|p| p.command.clone())
        .unwrap_or_default();
    let pub_id = audio.publication_id.clone();
    let resume_id = format!("theatre-library-{}-resume-{}", section.name(), pub_id);
    let audio_owned = audio.clone();
    let pres_owned = presentation;
    let resume_pos = position.unwrap_or(0);
    view! {
        <li class="theatre-row" data-publication-id=pub_id.clone()>
            <div class="theatre-row-copy">
                {title(work, locale)}
                {unit.map(|(title, language)| view! { <p lang=language>{title}</p> })}
                <p>{time}</p>
                <p class="theatre-muted">{messages::text(availability(resume), locale)}</p>
                {resume_feedback.map(|msg| view! { <p class="theatre-muted" role="status">{msg}</p> })}
                {match progress_command {
                    ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses {
                        local_position_ms,
                        server,
                        ..
                    }) => {
                        let prompt = messages::format(
                            Key::LiveProgressConflictPrompt,
                            locale,
                            &[
                                ("local", clock(local_position_ms)),
                                ("server", clock(server.position_ms)),
                            ],
                        );
                        let keep_here = messages::format(
                            Key::LiveProgressResolveHere,
                            locale,
                            &[("time", clock(local_position_ms))],
                        );
                        let keep_other = messages::format(
                            Key::LiveProgressResolveOther,
                            locale,
                            &[("time", clock(server.position_ms))],
                        );
                        let pub_here = pub_id.clone();
                        let pub_other = pub_id.clone();
                        Some(view! {
                            <div class="theatre-conflict-panel" role="status">
                                <p>{prompt}</p>
                                {actions.map(|act| view! {
                                    <div class="theatre-actions">
                                        <Button id=format!("theatre-library-{}-resolve-here-{}", section.name(), pub_here)
                                            variant=ButtonVariant::Tonal
                                            on_press=Callback::new(move |_| act.on_resolve_progress.run((pub_here.clone(), true)))>
                                            {keep_here}
                                        </Button>
                                        <Button id=format!("theatre-library-{}-resolve-other-{}", section.name(), pub_other)
                                            variant=ButtonVariant::Text
                                            on_press=Callback::new(move |_| act.on_resolve_progress.run((pub_other.clone(), false)))>
                                            {keep_other}
                                        </Button>
                                    </div>
                                })}
                            </div>
                        }.into_any())
                    }
                    ProgressCommandPhase::Conflict(ProgressConflict::KeepServer { server, .. }) => {
                        let msg = messages::format(
                            Key::LiveProgressKeepServer,
                            locale,
                            &[("time", clock(server.position_ms))],
                        );
                        Some(view! { <p class="theatre-muted" role="status">{msg}</p> }.into_any())
                    }
                    ProgressCommandPhase::UnknownOutcome { .. } => {
                        let pub_retry = pub_id.clone();
                        Some(view! {
                            <p class="theatre-muted" role="status">{messages::text(Key::LiveProgressUnknown, locale)}</p>
                            {actions.map(|act| view! {
                                <Button id=format!("theatre-library-{}-progress-retry-{}", section.name(), pub_retry)
                                    variant=ButtonVariant::Text
                                    on_press=Callback::new(move |_| act.on_retry_progress.run(pub_retry.clone()))>
                                    {messages::text(Key::PublicRetry, locale)}
                                </Button>
                            })}
                        }.into_any())
                    }
                    ProgressCommandPhase::Failed { .. } => {
                        let pub_retry = pub_id.clone();
                        Some(view! {
                            <p class="theatre-muted" role="status">{messages::text(Key::LiveProgressError, locale)}</p>
                            {actions.map(|act| view! {
                                <Button id=format!("theatre-library-{}-progress-retry-{}", section.name(), pub_retry)
                                    variant=ButtonVariant::Text
                                    on_press=Callback::new(move |_| act.on_retry_progress.run(pub_retry.clone()))>
                                    {messages::text(Key::PublicRetry, locale)}
                                </Button>
                            })}
                        }.into_any())
                    }
                    ProgressCommandPhase::Idle | ProgressCommandPhase::Pending { .. } => None,
                }}
            </div>
            {actions.and_then(|act| {
                (resume == ResumeStatus::ExactAvailable).then(|| {
                    let sr = unit_title_for_sr.clone();
                    view! {
                        <div class="theatre-row-actions">
                            <Button id=resume_id variant=ButtonVariant::Tonal
                                on_press=Callback::new(move |_| {
                                    act.on_resume.run((
                                        audio_owned.clone(),
                                        resume_pos,
                                        resume,
                                        pres_owned.clone(),
                                    ));
                                })>
                                {messages::text(Key::LiveResumeOpen, locale)}
                                {(!sr.is_empty()).then(|| view! { <span class="cui-sr-only">" "{sr}</span> })}
                            </Button>
                        </div>
                    }
                })
            })}
        </li>
    }
    .into_any()
}

fn saved_row(
    item: &cantos_contracts::listener_library::PresentedSavedItem,
    sync: Option<&LibrarySyncState>,
    actions: Option<LibraryActions>,
    locale: Locale,
) -> AnyView {
    let work_id = item.saved.work_id.clone();
    let presentation = item.presentation.clone();
    let sr_title = presentation
        .as_ref()
        .map(|w| w.title.clone())
        .unwrap_or_default();
    let saved_cmd = sync
        .and_then(|s| s.saved_work(&work_id))
        .map(|s| s.command.clone())
        .unwrap_or_default();
    let status_info = match &saved_cmd {
        SavedCommandPhase::UnknownOutcome { .. } => Some((Key::LiveSaveUnknown, true)),
        SavedCommandPhase::Conflict { .. } => Some((Key::LiveSaveConflict, true)),
        SavedCommandPhase::Failed { .. } => Some((Key::LiveSaveError, true)),
        SavedCommandPhase::Idle | SavedCommandPhase::Pending { .. } => None,
    };
    let pending = matches!(saved_cmd, SavedCommandPhase::Pending { .. });
    let btn_state = Signal::derive(move || {
        if pending {
            DisabledReason::new(messages::text(Key::LiveSaveRemoving, locale))
                .map(ButtonState::Loading)
                .unwrap_or_default()
        } else {
            ButtonState::Enabled
        }
    });
    let work_unsave = work_id.clone();
    let work_retry = work_id.clone();
    let pres_unsave = presentation.clone();
    view! {
        <li class="theatre-row" data-work-id=work_id.clone()>
            <div class="theatre-row-copy">
                {title(presentation, locale)}
                {status_info.map(|(msg_key, can_retry)| view! {
                    <p class="theatre-muted" role="status">{messages::text(msg_key, locale)}</p>
                    {actions.zip(can_retry.then_some(())).map(|(act, ())| {
                        let retry_id = work_retry.clone();
                        view! {
                            <Button id=format!("theatre-library-saved-retry-{}", retry_id)
                                variant=ButtonVariant::Text
                                on_press=Callback::new(move |_| act.on_retry_saved.run(retry_id.clone()))>
                                {messages::text(Key::PublicRetry, locale)}
                            </Button>
                        }
                    })}
                })}
            </div>
            {actions.map(|act| view! {
                <div class="theatre-row-actions">
                    <Button id=format!("theatre-library-saved-unsave-{}", work_unsave)
                        variant=ButtonVariant::Text
                        state=btn_state
                        on_press=Callback::new(move |_| act.on_unsave.run((work_unsave.clone(), pres_unsave.clone())))>
                        {messages::text(Key::RemoveSave, locale)}
                        {(!sr_title.is_empty()).then(|| view! { <span class="cui-sr-only">" "{sr_title}</span> })}
                    </Button>
                </div>
            })}
        </li>
    }
    .into_any()
}

fn rows(
    projection: &LibraryProjection,
    sync: Option<&LibrarySyncState>,
    section: Section,
    actions: Option<LibraryActions>,
    locale: Locale,
) -> AnyView {
    match section {
        Section::Continue => projection
            .continuing
            .iter()
            .map(|i| {
                audio_row(
                    Section::Continue,
                    &i.progress.audio,
                    Some(i.progress.position_ms),
                    Some(i.progress.completion),
                    i.resume,
                    i.presentation.clone(),
                    sync,
                    actions,
                    locale,
                )
            })
            .collect_view()
            .into_any(),
        Section::Saved => projection
            .saved
            .iter()
            .map(|i| saved_row(i, sync, actions, locale))
            .collect_view()
            .into_any(),
        Section::History => projection
            .history
            .iter()
            .map(|i| {
                audio_row(
                    Section::History,
                    &i.audio,
                    i.progress.as_ref().map(|p| p.position_ms),
                    i.progress.as_ref().map(|p| p.completion),
                    i.resume,
                    i.presentation.clone(),
                    sync,
                    actions,
                    locale,
                )
            })
            .collect_view()
            .into_any(),
    }
}
fn row_count(projection: &LibraryProjection, section: Section) -> usize {
    match section {
        Section::Continue => projection.continuing.len(),
        Section::Saved => projection.saved.len(),
        Section::History => projection.history.len(),
    }
}
fn status(projection: &LibraryProjection, section: Section, locale: Locale) -> String {
    let key = match projection.phase(section) {
        PagePhase::Idle | PagePhase::Loading => Key::LiveLibraryLoading,
        PagePhase::Failed(AccountFailure::CursorExpired) => Key::LiveLibraryCursorExpired,
        PagePhase::Failed(AccountFailure::Protocol) => Key::LiveLibraryInvalidPage,
        PagePhase::Failed(_) => Key::LiveLibraryPageError,
        PagePhase::Ready if row_count(projection, section) == 0 => {
            return messages::text(empty(section), locale).to_owned()
        }
        PagePhase::Ready => {
            return messages::format(
                Key::LiveLibraryLoaded,
                locale,
                &[("count", row_count(projection, section).to_string())],
            )
        }
    };
    messages::text(key, locale).to_owned()
}
pub(crate) fn status_id(section: Section) -> String {
    format!("theatre-library-{}-status", section.name())
}

fn history_clear_controls(
    projection: &LibraryProjection,
    sync: Option<&LibrarySyncState>,
    actions: Option<LibraryActions>,
    locale: Locale,
) -> AnyView {
    let Some(act) = actions else {
        return ().into_any();
    };
    let Some(sync) = sync else {
        return ().into_any();
    };
    let confirming = sync.clear_confirmation().is_some();
    let cmd = sync.history_command().clone();
    let has_rows = !projection.history.is_empty();
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

#[component]
fn LiveSection(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    section: Section,
    on_page: Callback<(Section, bool)>,
    actions: Option<LibraryActions>,
) -> impl IntoView {
    let heading_id = format!("theatre-library-{}-heading", section.name());
    view! {
        <section class="theatre-library-section" aria-labelledby=heading_id.clone()>
            <h2 id=heading_id.clone() class="theatre-section-heading">{move || messages::text(heading(section), locale.get())}</h2>
            {move || state.with(|s| s.library().map(|projection| {
                let locale = locale.get();
                let sync = s.sync();
                let phase = projection.phase(section);
                let restart = matches!(phase, PagePhase::Failed(AccountFailure::CursorExpired | AccountFailure::Protocol));
                let more_label = messages::format(Key::LiveLibraryLoadMore, locale, &[("section", messages::text(heading(section), locale).to_owned())]);
                view! {
                    <ul class="theatre-rows" aria-busy=(phase == PagePhase::Loading).to_string()>{rows(projection, sync, section, actions, locale)}</ul>
                    <p id=status_id(section) tabindex="-1" role="status" aria-live="polite">{status(projection, section, locale)}</p>
                    {(section == Section::History && phase == PagePhase::Ready).then(|| history_clear_controls(projection, sync, actions, locale))}
                    <div class="theatre-actions">
                        {matches!(phase, PagePhase::Failed(_)).then(|| view! {
                            <Button id=format!("theatre-library-{}-retry", section.name()) variant=ButtonVariant::Tonal on_press=Callback::new(move |_| on_page.run((section, restart)))>
                                {messages::text(if restart { Key::LiveLibraryRestart } else { Key::LiveLibraryRetry }, locale)}
                            </Button>
                        })}
                        {(phase == PagePhase::Ready && projection.has_more(section)).then(|| view! {
                            <Button id=format!("theatre-library-{}-more", section.name()) variant=ButtonVariant::Tonal on_press=Callback::new(move |_| on_page.run((section, false)))>{more_label}</Button>
                        })}
                        {(phase == PagePhase::Ready).then(|| view! {
                            <Button id=format!("theatre-library-{}-reload", section.name()) variant=ButtonVariant::Text on_press=Callback::new(move |_| on_page.run((section, true)))>{messages::text(Key::LiveLibraryRestart, locale)}</Button>
                        })}
                    </div>
                }
            }))}
        </section>
    }
}
#[component]
pub(crate) fn LiveLibrary(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    on_page: Callback<(Section, bool)>,
    #[prop(optional)] actions: Option<LibraryActions>,
) -> impl IntoView {
    view! {
        <section id="library" aria-labelledby="theatre-live-library-heading">
            <h1 id="theatre-live-library-heading">{move || messages::text(Key::Library, locale.get())}</h1>
            <Show when=move || state.get().phase() == SessionPhase::SignedIn fallback=move || view! {
                <p role="status">{move || messages::text(match state.get().phase() {
                    SessionPhase::Idle | SessionPhase::Checking => Key::AccountChecking,
                    SessionPhase::Failed(failure) => crate::features::account::account_failure_key(failure),
                    SessionPhase::LogoutPending => Key::AccountLogoutRetryHelp,
                    SessionPhase::Guest | SessionPhase::SignedIn => Key::LiveLibrarySignedOut,
                }, locale.get())}</p>
            }>
                <p class="theatre-muted">{move || messages::text(Key::LiveLibraryPageHelp, locale.get())}</p>
                {Section::ALL.into_iter().map(|section| view! { <LiveSection state locale section on_page actions/> }).collect_view()}
            </Show>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{account::AccountScope, account_view::SessionCompletion};
    fn signed_in() -> AccountViewState {
        let mut s = AccountViewState::default();
        let issued = s.begin_refresh().unwrap();
        assert!(matches!(
            s.finish_session(
                issued,
                Ok(AccountScope::from_session(
                    "listener-a".into(),
                    uuid::Uuid::from_u128(10).to_string()
                )
                .unwrap())
            ),
            Ok(SessionCompletion::Authenticated(_))
        ));
        s
    }
    fn populate(s: &mut AccountViewState) {
        for (section, body) in [
            (Section::Continue, include_bytes!("../../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_continue.json").as_slice()),
            (Section::Saved, include_bytes!("../../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_saved.json").as_slice()),
            (Section::History, include_bytes!("../../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_history.json").as_slice()),
        ] {
            let request = s.begin_library(section, false).unwrap();
            s.finish_page(&request, crate::live::library::decode_page(section, body)).unwrap();
        }
    }
    fn render(s: AccountViewState, language: Locale) -> String {
        Owner::new().with(|| view! {
            <LiveLibrary state=RwSignal::new(s) locale=RwSignal::new(language) on_page=Callback::new(|_| {})/>
        }.to_html())
    }
    #[test]
    fn three_real_sections_render_release_metadata_and_uuid_links_without_mock_or_play_effects() {
        let mut s = signed_in();
        populate(&mut s);
        for language in Locale::ALL {
            let html = render(s.clone(), language);
            assert_eq!(html.matches("<h1").count(), 1);
            for section in Section::ALL {
                assert!(html.contains(&format!("theatre-library-{}-heading", section.name())));
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
        let mut page: cantos_contracts::listener_library::PresentedContinuePage = serde_json::from_slice(include_bytes!("../../../../../../packages/contracts/fixtures/theatre/library/v1/presentation/presented_continue_unavailable.json")).unwrap();
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
                cantos_contracts::listener_library::PresentedHistoryPage {
                    state: cantos_contracts::listener_library::HistoryState {
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
                        state=RwSignal::new(state)
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
        s.with_sync_mut(|sync, _| {
            sync.prompt_clear_history();
        });
        let confirm_html = render_with_actions(s, Locale::Vietnamese);
        assert!(confirm_html.contains("id=\"theatre-library-history-clear-confirm\""));
        assert!(confirm_html.contains("id=\"theatre-library-history-clear-submit\""));
        assert!(confirm_html.contains("id=\"theatre-library-history-clear-cancel\""));
    }
}
