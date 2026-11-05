use super::*;
use crate::{
    application::library::LibraryPage,
    contract::library::{
        HistoryState, PresentedContinuePage, PresentedHistoryPage, PresentedSavedPage,
    },
};

fn empty_page(section: Section, more: bool) -> LibraryPage {
    let next_cursor = more.then(|| "next-page".to_owned());
    match section {
        Section::Continue => LibraryPage::Continue(PresentedContinuePage {
            items: vec![],
            next_cursor,
        }),
        Section::Saved => LibraryPage::Saved(PresentedSavedPage {
            items: vec![],
            next_cursor,
        }),
        Section::History => LibraryPage::History(PresentedHistoryPage {
            state: HistoryState {
                epoch: 0,
                revision: 0,
            },
            items: vec![],
            next_cursor,
        }),
    }
}

fn ready_empty() -> AccountViewState {
    let mut state = super::tests::signed_in();
    for section in Section::ALL {
        let request = state.begin_library(section, false).unwrap();
        state
            .finish_page(&request, Ok(empty_page(section, false)))
            .unwrap();
    }
    state
}

fn render(state: AccountViewState, locale: Locale) -> String {
    Owner::new().with(|| {
        view! {
            <LiveLibrary state=RwSignal::new(state).read_only() locale=RwSignal::new(locale)
                on_page=Callback::new(|_| {})/>
        }
        .to_html()
    })
}

#[test]
fn terminal_empty_library_has_one_invitation_no_technical_controls_and_keeps_focus_targets() {
    for locale in Locale::supported() {
        let html = render(ready_empty(), locale);
        assert_eq!(html.matches("href=\"#discover\"").count(), 1);
        assert!(html.contains("theatre-library-empty-title"));
        assert!(!html.contains("theatre-library-refresh"));
        for section in Section::ALL {
            assert!(html.contains(&format!("id=\"theatre-library-{}-status\"", section.name())));
            assert!(html.contains(messages::text(empty(section), locale)));
            assert!(!html.contains(&format!("theatre-library-{}-reload", section.name())));
            assert!(!html.contains(&format!("theatre-library-{}-retry", section.name())));
            assert!(!html.contains(&format!("theatre-library-{}-more", section.name())));
        }
        let icons = html
            .split("<span")
            .filter_map(|part| {
                let tag = part.split('>').next()?;
                tag.contains("theatre-icon").then_some(tag)
            })
            .collect::<Vec<_>>();
        assert!(
            icons.is_empty(),
            "section summaries have no decorative icons"
        );
        assert_eq!(html.matches("data-artwork=\"library\"").count(), 1);
        assert_eq!(html.matches("<img ").count(), 1);
        assert!(html.contains("assets/empty/library-expressive.avif"));
        assert!(!html.contains("data-artwork=\"history\""));
    }
}

#[test]
fn empty_is_section_local_while_rows_and_failures_keep_their_meaning() {
    let mut state = super::tests::signed_in();
    let continuing = state.begin_library(Section::Continue, false).unwrap();
    state.finish_page(&continuing, crate::live::library::decode_page(Section::Continue,
        include_bytes!("../../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_continue.json"))).unwrap();
    let saved = state.begin_library(Section::Saved, true).unwrap();
    state
        .finish_page(&saved, Ok(empty_page(Section::Saved, false)))
        .unwrap();
    let history = state.begin_library(Section::History, true).unwrap();
    state
        .finish_page(&history, Err(AccountFailure::Unavailable))
        .unwrap();
    let html = render(state, Locale::English);
    assert!(!html.contains("theatre-library-empty-title"));
    assert!(html.contains("Ánh đèn cuối sân khấu"));
    assert!(html.contains("You have no saved stories yet."));
    assert!(html.contains("theatre-library-history-retry"));
    assert!(!html.contains("No listening history in this account yet."));
    assert_eq!(html.matches("href=\"#discover\"").count(), 0);
}

#[test]
fn terminal_history_is_quiet_and_retains_its_shortcut_heading() {
    let mut state = super::tests::signed_in();
    let continuing = state.begin_library(Section::Continue, false).unwrap();
    state.finish_page(&continuing, crate::live::library::decode_page(Section::Continue,
        include_bytes!("../../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_continue.json"))).unwrap();
    let history = state.begin_library(Section::History, true).unwrap();
    assert!(!render(state.clone(), Locale::English).contains("data-artwork=\"history\""));
    state
        .finish_page(&history, Err(AccountFailure::Unavailable))
        .unwrap();
    assert!(!render(state.clone(), Locale::English).contains("data-artwork=\"history\""));
    let retry = state.begin_library(Section::History, true).unwrap();
    state
        .finish_page(&retry, Ok(empty_page(Section::History, false)))
        .unwrap();
    for locale in Locale::supported() {
        let html = render(state.clone(), locale);
        assert!(html.contains("Ánh đèn cuối sân khấu"));
        assert_eq!(html.matches("data-artwork=\"history\"").count(), 0);
        assert!(html.contains("id=\"theatre-library-history-heading\" tabindex=\"-1\""));
        assert!(html.contains(messages::text(Key::LiveLibraryEmptyHistory, locale)));
        assert!(!html.contains("library-expressive.avif"));
    }
}

#[test]
fn idle_loading_failure_and_empty_page_with_cursor_are_never_terminal_empty() {
    for section in Section::ALL {
        let mut state = ready_empty();
        let request = state.begin_library(section, true).unwrap();
        let loading = render(state.clone(), Locale::English);
        assert!(!loading.contains("theatre-library-empty-title"));
        assert!(loading.contains("aria-busy=\"true\""));
        assert!(!loading.contains(messages::text(empty(section), Locale::English)));
        state
            .finish_page(&request, Err(AccountFailure::Unavailable))
            .unwrap();
        let failed = render(state, Locale::English);
        assert!(!failed.contains("theatre-library-empty-title"));
        assert!(failed.contains(&format!("theatre-library-{}-retry", section.name())));

        let mut state = ready_empty();
        let request = state.begin_library(section, true).unwrap();
        state
            .finish_page(&request, Ok(empty_page(section, true)))
            .unwrap();
        let more = render(state, Locale::English);
        assert!(!more.contains("theatre-library-empty-title"));
        assert!(more.contains(&format!("theatre-library-{}-more", section.name())));
        assert!(more.contains(messages::text(Key::LiveLibraryMore, Locale::English)));
        assert!(!more.contains(messages::text(empty(section), Locale::English)));
    }
    let idle = render(super::tests::signed_in(), Locale::English);
    assert!(!idle.contains("theatre-library-empty-title"));
    assert!(!idle.contains("href=\"#discover\""));
    let guest = render(AccountViewState::default(), Locale::English);
    assert!(!guest.contains("theatre-library-refresh"));
}

#[test]
fn retained_rows_survive_next_page_loading_and_failure_without_empty_feedback() {
    let mut state = super::tests::signed_in();
    let mut page: PresentedSavedPage = serde_json::from_slice(include_bytes!(
        "../../../../../../../services/cantos-server/contracts/theatre/library/v1/fixtures/presentation/presented_saved.json"
    )).unwrap();
    page.next_cursor = Some("saved-next".to_owned());
    let first = state.begin_library(Section::Saved, false).unwrap();
    state
        .finish_page(&first, Ok(LibraryPage::Saved(page)))
        .unwrap();
    let next = state.begin_library(Section::Saved, false).unwrap();
    for failed in [false, true] {
        if failed {
            state
                .finish_page(&next, Err(AccountFailure::Unavailable))
                .unwrap();
        }
        let html = render(state.clone(), Locale::English);
        assert!(html.contains("Ánh đèn cuối sân khấu"));
        assert!(!html.contains("You have no saved stories yet."));
        assert!(!html.contains("theatre-library-empty-title"));
        assert_eq!(html.contains("theatre-library-saved-retry"), failed);
    }
}

#[test]
fn background_refresh_has_section_busy_feedback_without_a_header_button() {
    let mut state = ready_empty();
    state.begin_library(Section::Continue, true).unwrap();
    for locale in Locale::supported() {
        let html = render(state.clone(), locale);
        assert!(!html.contains("theatre-library-refresh"));
        assert!(html.contains("aria-busy=\"true\""));
        assert!(html.contains(messages::text(Key::LiveLibraryLoading, locale)));
        assert!(!html.contains(messages::text(Key::LiveLibraryPageHelp, locale)));
    }
}

#[test]
fn actual_shared_rows_suppress_the_personal_hero_but_not_the_shared_content() {
    use crate::application::{public_discovery::DiscoveryQuery, shared_access::SharedAccess};
    for locale in Locale::supported() {
        Owner::new().with(|| {
            let account = RwSignal::new(ready_empty());
            let mut access = SharedAccess::default();
            access.observe(account.with_untracked(AccountViewState::fence));
            let ticket = access.catalog.begin(DiscoveryQuery::default()).unwrap();
            let page: crate::contract::discovery::MetadataWorkPage = serde_json::from_str(include_str!(
                "../../../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/metadata/covered_works.json"
            )).unwrap();
            let title = page.items[0].title.clone();
            let link = format!("href=\"#shared/{}\"", page.items[0].work_id);
            access.catalog.finish(&ticket, Ok(page));
            let shared = RwSignal::new(access);
            let locale = RwSignal::new(locale);
            let view = view! {
                <LiveLibrary state=account.read_only() locale on_page=Callback::new(|_| {})
                    shared_has_rows=Signal::derive(move || shared.with(|s| account.with(|a| !s.visible_rows(a).is_empty())))
                    shared_section=ViewFn::from(move || view! {
                        <crate::features::shared_access::SharedLibrary account=account.read_only() state=shared locale
                            on_reload=Callback::new(|_| {}) on_more=Callback::new(|_| {}) on_claim=Callback::new(|_| {})/>
                    })/>
            };
            let html = view.to_html();
            assert!(!html.contains("theatre-library-empty-title"));
            assert!(html.contains(&title) && html.contains(&link));
            assert!(!html.contains("href=\"#discover\""));
        });
    }
}

#[test]
fn shared_component_presence_does_not_suppress_personal_invitation() {
    Owner::new().with(|| {
        let html = view! {
            <LiveLibrary state=RwSignal::new(ready_empty()).read_only()
                locale=RwSignal::new(Locale::English) on_page=Callback::new(|_| {})
                shared_section=ViewFn::from(|| view! { <div id="shared-fixture"/> })/>
        }
        .to_html();
        assert!(html.contains("theatre-library-empty-title"));
        assert_eq!(html.matches("href=\"#discover\"").count(), 1);
        assert!(html.contains("shared-fixture"));
    });
}
