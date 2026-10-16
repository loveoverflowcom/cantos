//! Public discovery is independent of account availability; auth is always an explicit route.
use crate::{
    app::{live_navigation::LiveNavigation, routes::Route},
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthRoute, MembershipReturn},
        public_discovery::PublicDiscoveryState,
        registration::RegistrationState,
    },
    features::{
        account::LiveAccount, auth::AuthPage, common::PersonGlyph, library::LiveLibrary,
        public_home::PublicHome,
    },
    messages::{self, Key, Locale},
    platform::{self, auth_browser},
};
use cantos_ui::{AccessibleName, BackButton, Button, ButtonVariant, Logo, LogoTone, Theme};
use leptos::prelude::*;

fn refresh_account(state: RwSignal<AccountViewState>) {
    #[cfg(target_arch = "wasm32")]
    crate::live::runtime::refresh(state);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = state;
}
fn refresh_discovery(state: RwSignal<PublicDiscoveryState>) {
    #[cfg(target_arch = "wasm32")]
    {
        let mut issued = None;
        state.update(|s| issued = s.begin_refresh());
        if let Some(issued) = issued {
            leptos::task::spawn_local(async move {
                let result = crate::live::discovery::load().await;
                let _ = state.try_update(|s| {
                    s.finish(issued, result);
                });
            });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = state;
}

fn refresh_registration(state: RwSignal<RegistrationState>) {
    #[cfg(target_arch = "wasm32")]
    {
        let mut issued = None;
        state.update(|current| issued = current.begin());
        if let Some(issued) = issued {
            leptos::task::spawn_local(async move {
                let result = crate::live::identity_configuration::load().await;
                let _ = state.try_update(|current| {
                    current.finish(issued, result);
                });
            });
        }
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = state;
}

#[component]
pub fn LiveBoundary(#[prop(optional)] preview: bool) -> impl IntoView {
    let state = RwSignal::new(AccountViewState::default());
    let discovery = RwSignal::new(PublicDiscoveryState::default());
    let registration = RwSignal::new(RegistrationState::default());
    let locale = RwSignal::new(Locale::Vietnamese);
    let theme = RwSignal::new(Theme::System);
    let main = NodeRef::<leptos::html::Main>::new();
    let nav = LiveNavigation::new(state, main, preview);
    #[cfg(target_arch = "wasm32")]
    if !preview {
        crate::live::runtime::mount(state);
    }
    if preview {
        let _ = crate::features::auth_preview::guest(state);
        registration.update(|current| {
            if let Some(issued) = current.begin() {
                current.finish(issued, Ok(cantos_contracts::theatre_identity::Configuration {
                    login: cantos_contracts::theatre_identity::LoginMethod::Oidc,
                    registration: cantos_contracts::theatre_identity::Registration::Unavailable {
                        reason: cantos_contracts::theatre_identity::RegistrationUnavailableReason::ProviderUnsupported,
                    },
                }));
            }
        });
        discovery.update(|s| {
            if let Some(issued) = s.begin_refresh() {
                s.finish(
                    issued,
                    Ok(cantos_contracts::theatre::WorkPage {
                        items: vec![],
                        next_cursor: None,
                    }),
                );
            }
        });
    } else {
        refresh_discovery(discovery);
        refresh_registration(registration);
    }
    Effect::new(move |_| platform::set_title(&messages::page_title(locale.get(), nav.route.get())));
    Effect::new(move |_| platform::set_document_language(locale.get().lang()));
    let on_login = Callback::new(move |()| nav.open_auth(AuthRoute::login(nav.origin())));
    let on_register = Callback::new(move |()| nav.open_auth(AuthRoute::register(nav.origin())));
    let on_refresh = Callback::new(move |()| {
        if preview {
            state.update(|s| {
                let _ = s.begin_refresh();
            });
        } else {
            refresh_account(state);
        }
    });
    let on_logout = Callback::new(move |()| nav.logout(state));
    let on_membership = Callback::new(move |()| {
        auth_browser::go(
            nav.route,
            Route::Membership(MembershipReturn::Account),
            false,
        )
    });
    view! {
        <super::LiveFrame theme>
            {preview.then(|| view! { <crate::features::auth_preview::AuthPreviewControls state locale on_cancel=Callback::new(move |()| nav.cancel())/> })}
            <div class="theatre-app theatre-live-app" data-auth=move || matches!(nav.route.get(), Route::Auth(_)).to_string() lang=move || locale.get().lang()>
                <a class="theatre-skip" href="#theatre-main" on:click=move |e| { e.prevent_default(); platform::focus_main(main); }>{move || messages::text(Key::SkipContent, locale.get())}</a>
                <aside class="theatre-rail">
                    <a class="theatre-brand" href="#discover" aria-label="Cantos"><Logo tone=LogoTone::Light class="theatre-logo-light"/><Logo tone=LogoTone::Dark class="theatre-logo-dark"/></a>
                    <nav class="theatre-nav" aria-label=move || messages::text(Key::MainNavigation, locale.get())>
                        <a class="theatre-nav-link" href="#discover" aria-current=move || (nav.route.get().primary_navigation() == Some(Route::Discover)).then_some("page")>{move || messages::text(Key::Discover, locale.get())}</a>
                        <a class="theatre-nav-link" href="#library" aria-current=move || (nav.route.get().primary_navigation() == Some(Route::Library)).then_some("page")>{move || messages::text(Key::Library, locale.get())}</a>
                    </nav>
                </aside>
                <div class="theatre-content">
                    <header class="theatre-topbar">
                        <a class="theatre-brand-name" href="#discover">"Cantos"</a>
                        <div class="theatre-topbar-actions">
                            <label class="theatre-locale-label" for="theatre-live-locale">{move || messages::text(Key::Locale, locale.get())}</label>
                            <select id="theatre-live-locale" prop:value=move || locale.get().lang() on:change=move |e| { if let Some(selected) = Locale::from_lang(&event_target_value(&e)) { locale.set(selected); } }>{Locale::ALL.into_iter().map(|option| view! { <option value=option.lang() lang=option.lang()>{option.autonym()}</option> }).collect_view()}</select>
                            <a id="theatre-account-entry" class="theatre-avatar" href="#account" aria-label=move || messages::text(Key::Account, locale.get()) aria-current=move || (nav.route.get() == Route::Account).then_some("page")><PersonGlyph/></a>
                        </div>
                    </header>
                    <super::LiveContainer>
                        <main id="theatre-main" node_ref=main tabindex="-1" class="theatre-main">
                            {move || match nav.route.get() {
                                Route::Auth(auth) => view! { <AuthPage state registration locale route=auth on_back=Callback::new(move |()| nav.back()) on_register on_login=Callback::new(move |()| nav.login(state)) on_home=Callback::new(move |()| { nav.cancel(); auth_browser::go(nav.route, Route::Discover, false); }) on_membership on_refresh on_logout on_registration_refresh=Callback::new(move |()| { if !preview { refresh_registration(registration); } })/> }.into_any(),
                                Route::Account => view! { <LiveAccount state locale theme on_login on_register on_logout on_refresh on_membership/> }.into_any(),
                                Route::Settings => view! { <LivePreferences locale theme/> }.into_any(),
                                Route::Library | Route::History => view! {
                                    <LiveLibrary state locale/>
                                    <LibraryAccountEntry state locale/>
                                }.into_any(),
                                Route::Membership(member) => view! {
                                    <section class="theatre-membership-information" aria-labelledby="theatre-membership-heading">
                                        {move || AccessibleName::new(crate::features::auth_copy::text(membership_back_key(member),locale.get())).ok().map(|name| view! { <BackButton id="theatre-membership-back" name on_back=Callback::new(move |_| nav.back())/> })}
                                        <h1 id="theatre-membership-heading">"Membership"</h1>
                                        <p>{move || messages::text(Key::MembershipUnavailable,locale.get())}</p>
                                        <Button id="theatre-membership-login" variant=ButtonVariant::Tonal focus_on_press=true on_press=Callback::new(move |_| nav.open_auth(AuthRoute::login(nav.origin())))>{move || messages::text(Key::SignIn,locale.get())}</Button>
                                    </section>
                                }.into_any(),
                                _ => view! { <PublicHome state=discovery locale on_login on_register on_refresh=Callback::new(move |()| { if !preview { refresh_discovery(discovery); } })/> }.into_any(),
                            }}
                        </main>
                    </super::LiveContainer>
                </div>
            </div>
        </super::LiveFrame>
    }
}

#[component]
fn LibraryAccountEntry(
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    view! {
        <Show when=move || state.get().phase() != SessionPhase::SignedIn>
            <a id="theatre-library-account" class="theatre-action theatre-action-tonal" href="#account">
                {move || crate::features::auth_copy::text(crate::features::auth_copy::CopyKey::OpenAccount, locale.get())}
            </a>
        </Show>
    }
}

#[component]
fn LivePreferences(locale: RwSignal<Locale>, theme: RwSignal<Theme>) -> impl IntoView {
    view! {
        <section class="theatre-account-preferences" aria-labelledby="theatre-preferences-heading">
            <h1 id="theatre-preferences-heading">{move || messages::text(Key::Settings, locale.get())}</h1>
            <div class="theatre-account-theme">
                <label for="theatre-settings-theme">{move || messages::text(Key::Theme, locale.get())}</label>
                <select id="theatre-settings-theme" prop:value=move || theme.get().as_str() on:change=move |e| { if let Some(value) = Theme::from_attribute(&event_target_value(&e)) { theme.set(value); } }>
                    <option value="system">{move || messages::text(Key::System,locale.get())}</option>
                    <option value="light">{move || messages::text(Key::Light,locale.get())}</option>
                    <option value="dark">{move || messages::text(Key::Dark,locale.get())}</option>
                </select>
            </div>
            <a class="theatre-text-link" href="#account">{move || messages::text(Key::Account,locale.get())}</a>
        </section>
    }
}

#[component]
pub fn AuthPreview() -> impl IntoView {
    view! { <LiveBoundary preview=true/> }
}

fn membership_back_key(member: MembershipReturn) -> crate::features::auth_copy::CopyKey {
    use crate::features::auth_copy::CopyKey;
    match member {
        MembershipReturn::Home => CopyKey::ReturnHome,
        MembershipReturn::Account => CopyKey::ReturnAccount,
        MembershipReturn::RegistrationComplete(_) => CopyKey::ReturnRegistration,
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn guest_library_entry_opens_account_before_any_login_and_hides_for_signed_in_listener() {
        use crate::features::auth_preview::{guest, signed_in, PreviewActor};

        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            guest(state).unwrap();
            for (language, copy) in [
                (Locale::Vietnamese, "Mở Tài khoản"),
                (Locale::English, "Open Account"),
            ] {
                let locale = RwSignal::new(language);
                let html = view! { <LibraryAccountEntry state locale/> }.to_html();
                assert!(html.contains("href=\"#account\""));
                assert!(html.contains(copy));
                for absent in ["<button", "#login", "Đăng nhập", "Sign in"] {
                    assert!(!html.contains(absent), "{absent}");
                }
            }
            signed_in(state, PreviewActor::A).unwrap();
            let locale = RwSignal::new(Locale::English);
            let html = view! { <LibraryAccountEntry state locale/> }.to_html();
            assert!(!html.contains("theatre-library-account"));
        });
    }

    #[test]
    fn membership_back_names_the_actual_context_and_never_claims_verified_registration() {
        use crate::application::auth_navigation::{NavigationTarget, ReturnDestination};
        use crate::features::auth_copy::text;
        for (member, target, vi, en) in [
            (
                MembershipReturn::Home,
                NavigationTarget::Home,
                "Về Trang chủ",
                "Back to Home",
            ),
            (
                MembershipReturn::Account,
                NavigationTarget::Account,
                "Về Tài khoản",
                "Back to Account",
            ),
            (
                MembershipReturn::RegistrationComplete(ReturnDestination::Home),
                NavigationTarget::Auth(AuthRoute::registration_complete(ReturnDestination::Home)),
                "Về đăng ký tài khoản",
                "Back to account registration",
            ),
        ] {
            assert_eq!(member.back(), target);
            assert_eq!(text(membership_back_key(member), Locale::Vietnamese), vi);
            assert_eq!(text(membership_back_key(member), Locale::English), en);
        }
    }
}
