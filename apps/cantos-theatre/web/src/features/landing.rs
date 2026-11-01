//! Guest entry is a distinct page; catalog/search and session authority keep their existing owners.
use super::public_home::{HomeAccount, PublicCatalog, SearchForm};
use crate::{
    application::{
        account_view::AccountViewState,
        public_discovery::{DiscoveryQuery, PublicDiscoveryState},
    },
    core::public_ids::PublicWorkId,
    messages::{self, Key, Locale},
};
use leptos::prelude::*;

#[component]
pub(crate) fn Landing(
    state: RwSignal<PublicDiscoveryState>,
    account: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    applied: RwSignal<DiscoveryQuery>,
    on_search: Callback<DiscoveryQuery>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
    on_retry: Callback<()>,
    on_load_more: Callback<()>,
    on_restart: Callback<()>,
    on_open_work: Callback<PublicWorkId>,
) -> impl IntoView {
    view! {
        <section id="theatre-landing" class="theatre-public-home theatre-landing" aria-labelledby="theatre-landing-heading">
            <SearchForm applied locale on_search state on_retry on_restart on_load_more on_open_work>
                <header class="theatre-discover-intro">
                    <div class="theatre-discover-intro-copy">
                        <p class="theatre-eyebrow">{move || messages::text(Key::PublicHomeEyebrow, locale.get())}</p>
                        <h1 id="theatre-landing-heading">{move || messages::text(Key::HomeHeading, locale.get())}</h1>
                        <p class="theatre-muted">{move || messages::text(Key::PublicHomeIntroduction, locale.get())}</p>
                    </div>
                    <HomeAccount account locale on_login on_register on_refresh on_logout/>
                </header>
            </SearchForm>
            <PublicCatalog state locale on_retry on_load_more on_restart on_open_work/>
        </section>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        application::{account_view::AccountFailure, public_discovery::DiscoveryReadError},
        contract::discovery::MetadataWorkPage,
    };

    fn render(
        catalog: PublicDiscoveryState,
        account: ReadSignal<AccountViewState>,
        language: Locale,
    ) -> String {
        view! { <Landing state=RwSignal::new(catalog) account locale=RwSignal::new(language)
        applied=RwSignal::new(DiscoveryQuery::default()) on_search=Callback::new(|_|())
        on_login=Callback::new(|_|()) on_register=Callback::new(|_|())
        on_refresh=Callback::new(|_|()) on_logout=Callback::new(|_|())
        on_retry=Callback::new(|_|()) on_load_more=Callback::new(|_|())
        on_restart=Callback::new(|_|()) on_open_work=Callback::new(|_|())/> }
        .to_html()
    }

    // Shared scenario: cantos.presentation.guest-navigation.run.unresolved-session
    #[test]
    fn landing_keeps_real_catalog_failure_and_retry_independent_of_session_failure() {
        Owner::new().with(|| {
            for language in [Locale::Vietnamese, Locale::English] {
                let account = RwSignal::new(AccountViewState::default());
                let lookup = account.write().begin_refresh().unwrap();
                account
                    .write()
                    .finish_session(lookup, Err(AccountFailure::Unavailable), || "unused".into())
                    .unwrap();
                let mut catalog = PublicDiscoveryState::default();
                let ticket = catalog.begin(DiscoveryQuery::default()).unwrap();
                catalog.finish(&ticket, Err(DiscoveryReadError::Unavailable));
                let html = render(catalog, account.read_only(), language);
                for id in [
                    "theatre-landing",
                    "theatre-discovery-search",
                    "theatre-home-retry",
                    "theatre-home-session-status",
                ] {
                    assert!(html.contains(id), "{id}");
                }
                assert!(!html.contains("id=\"theatre-home-login\""));
                assert!(!html.contains("theatre-public-home-heading"));
                assert!(!html.contains("theatre-nav-link"));
            }
        });
    }

    #[test]
    fn guest_landing_has_loading_and_completed_empty_states_with_owner_admitted_auth() {
        Owner::new().with(|| {
            let account = RwSignal::new(AccountViewState::default());
            crate::features::auth_preview::guest(account).unwrap();
            for language in [Locale::Vietnamese, Locale::English] {
                let loading = render(
                    PublicDiscoveryState::default(),
                    account.read_only(),
                    language,
                );
                assert!(loading.contains(messages::text(Key::PublicHomeLoading, language)));
                assert!(loading.contains("id=\"theatre-home-login\""));
                assert!(loading.contains("id=\"theatre-home-register\""));
                let mut catalog = PublicDiscoveryState::default();
                let ticket = catalog.begin(DiscoveryQuery::default()).unwrap();
                catalog.finish(
                    &ticket,
                    Ok(MetadataWorkPage {
                        items: vec![],
                        next_cursor: None,
                    }),
                );
                let empty = render(catalog, account.read_only(), language);
                assert!(empty.contains(messages::text(Key::PublicHomeEmpty, language)));
                assert!(!empty.contains("theatre-home-retry"));
                assert!(!empty.contains("<audio"));
            }
        });
    }
}
