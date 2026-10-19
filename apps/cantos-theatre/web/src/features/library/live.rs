//! Web-owned Library composition over the full presented read model, separate from demo data.
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountFailure, AccountViewState, SessionPhase},
        library::{LibraryProjection, PagePhase, Section},
    },
    core::public_ids::PublicWorkId,
    messages::{self, Key, Locale},
};
use cantos_contracts::listener_library::{
    AudioIdentity, AudioPresentation, ResumeStatus, WorkPresentation,
};
use cantos_ui::{Button, ButtonVariant};
use leptos::prelude::*;

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
fn audio_row(
    audio: &AudioIdentity,
    position: Option<u64>,
    resume: ResumeStatus,
    presentation: Option<AudioPresentation>,
    locale: Locale,
) -> AnyView {
    let unit = presentation
        .as_ref()
        .map(|p| (p.unit_title.clone(), p.work.language.clone()));
    let work = presentation.map(|p| p.work);
    let time = position.map_or_else(
        || messages::text(Key::LiveLibraryNoPosition, locale).to_owned(),
        |position| {
            messages::format(
                Key::LivePosition,
                locale,
                &[
                    ("position", clock(position)),
                    ("total", clock(audio.duration_ms)),
                ],
            )
        },
    );
    view! {
        <li class="theatre-row"><div class="theatre-row-copy">
            {title(work, locale)}
            {unit.map(|(title, language)| view! { <p lang=language>{title}</p> })}
            <p>{time}</p>
            <p class="theatre-muted">{messages::text(availability(resume), locale)}</p>
        </div></li>
    }
    .into_any()
}
fn rows(projection: &LibraryProjection, section: Section, locale: Locale) -> AnyView {
    match section {
        Section::Continue => projection.continuing.iter().map(|i| audio_row(&i.progress.audio, Some(i.progress.position_ms), i.resume, i.presentation.clone(), locale)).collect_view().into_any(),
        Section::Saved => projection.saved.iter().map(|i| view! {
            <li class="theatre-row"><div class="theatre-row-copy">{title(i.presentation.clone(), locale)}</div></li>
        }).collect_view().into_any(),
        Section::History => projection.history.iter().map(|i| audio_row(&i.audio, i.progress.as_ref().map(|p| p.position_ms), i.resume, i.presentation.clone(), locale)).collect_view().into_any(),
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

#[component]
fn LiveSection(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    section: Section,
    on_page: Callback<(Section, bool)>,
) -> impl IntoView {
    let heading_id = format!("theatre-library-{}-heading", section.name());
    view! {
        <section class="theatre-library-section" aria-labelledby=heading_id.clone()>
            <h2 id=heading_id.clone() class="theatre-section-heading">{move || messages::text(heading(section), locale.get())}</h2>
            {move || state.with(|s| s.library().map(|projection| {
                let locale = locale.get();
                let phase = projection.phase(section);
                let restart = matches!(phase, PagePhase::Failed(AccountFailure::CursorExpired | AccountFailure::Protocol));
                let more_label = messages::format(Key::LiveLibraryLoadMore, locale, &[("section", messages::text(heading(section), locale).to_owned())]);
                view! {
                    <ul class="theatre-rows" aria-busy=(phase == PagePhase::Loading).to_string()>{rows(projection, section, locale)}</ul>
                    <p id=status_id(section) tabindex="-1" role="status" aria-live="polite">{status(projection, section, locale)}</p>
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
                {Section::ALL.into_iter().map(|section| view! { <LiveSection state locale section on_page/> }).collect_view()}
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
}
