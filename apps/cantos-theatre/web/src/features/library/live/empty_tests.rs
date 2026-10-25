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
fn terminal_empty_library_has_one_expressive_illustration_quiet_summaries_and_one_refresh() {
    for locale in Locale::ALL {
        let html = render(ready_empty(), locale);
        assert_eq!(html.matches("href=\"#discover\"").count(), 1);
        assert!(html.contains("theatre-library-empty-title"));
        assert_eq!(html.matches("id=\"theatre-library-refresh\"").count(), 1);
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
            "global emptiness keeps section summaries quiet"
        );
        assert_eq!(html.matches("data-artwork=\"library\"").count(), 1);
        assert_eq!(html.matches("<img ").count(), 1);
        assert!(html.contains("assets/empty/library-expressive.png"));
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
    assert_eq!(html.matches("href=\"#discover\"").count(), 1);
}

#[test]
fn contextual_history_art_requires_its_own_completed_read_and_keeps_other_rows() {
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
    for locale in Locale::ALL {
        let html = render(state.clone(), locale);
        assert!(html.contains("Ánh đèn cuối sân khấu"));
        assert_eq!(html.matches("data-artwork=\"history\"").count(), 1);
        assert!(html.contains("assets/empty/light/cantos-empty-history.svg"));
        assert!(html.contains("assets/empty/dark/cantos-empty-history.svg"));
        assert!(!html.contains("library-expressive.png"));
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
fn refresh_plans_one_restart_per_section_and_refuses_pending_or_signed_out() {
    let mut state = ready_empty();
    assert_eq!(
        refresh_intents(&state),
        Some([
            (Section::Continue, true, false),
            (Section::Saved, true, false),
            (Section::History, true, false),
        ])
    );
    let request = state.begin_library(Section::Saved, true).unwrap();
    assert_eq!(refresh_intents(&state), None);
    state
        .finish_page(&request, Ok(empty_page(Section::Saved, false)))
        .unwrap();
    assert!(refresh_intents(&state).is_some());
    state.begin_logout().unwrap();
    assert_eq!(refresh_intents(&state), None);
    assert_eq!(refresh_intents(&AccountViewState::default()), None);
    assert_eq!(
        crate::app::routes::Route::from_live_hash("#discover"),
        crate::app::routes::Route::Discover
    );
}

#[test]
fn pending_refresh_is_focusable_and_exposes_its_busy_reason() {
    let mut state = ready_empty();
    state.begin_library(Section::Continue, true).unwrap();
    for locale in Locale::ALL {
        let html = render(state.clone(), locale);
        let trigger = html
            .split("id=\"theatre-library-refresh\"")
            .nth(1)
            .unwrap()
            .split('>')
            .next()
            .unwrap();
        assert!(
            !trigger.contains(" disabled"),
            "pending native disable would discard keyboard focus: {trigger}"
        );
        assert!(trigger.contains("aria-disabled=\"true\""));
        assert!(trigger.contains("aria-busy=\"true\""));
        assert!(trigger.contains("aria-describedby=\"theatre-library-refresh-status\""));
        assert!(html.contains(messages::text(Key::LiveLibraryRefreshing, locale)));
    }
}
