//! Public discovery is independent of account availability; auth is always an explicit route.
use crate::{
    app::{live_navigation::LiveNavigation, routes::Route},
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthRoute, MembershipReturn},
        public_discovery::PublicDiscoveryState,
        registration::RegistrationState,
        signup::SignupState,
    },
    features::{
        account::LiveAccount, auth::AuthPage, common::PersonGlyph, library::LiveLibrary,
        public_home::PublicHome,
    },
    messages::{self, Key, Locale},
    platform::{self, auth_browser, signup_browser},
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
    // Capture and scrub a proof before navigation construction, effects or any network request.
    let captured = signup_browser::capture_link();
    let history_blocked =
        RwSignal::new(captured == Err(signup_browser::CaptureFailure::HistoryUnavailable));
    let network_allowed = Signal::derive(move || !history_blocked.get());
    let signup = RwSignal::new(SignupState::default());
    let incoming_link_navigation = StoredValue::new(false);
    signup.update(|current| match captured {
        Ok(Some(token)) => current.capture(token),
        Ok(None) => (),
        Err(_) => current.capture_failed(),
    });
    signup_browser::bind_capture(
        Callback::new(move |captured| {
            history_blocked
                .set(captured == Err(signup_browser::CaptureFailure::HistoryUnavailable));
            incoming_link_navigation.set_value(true);
            signup.update(|current| match captured {
                Ok(Some(token)) => current.capture(token),
                Ok(None) => (),
                Err(_) => current.capture_failed(),
            })
        }),
        Callback::new(move |()| {
            // Escape cannot dismiss an unsafe URL. Real Back/Forward to a safe location can.
            if history_blocked.get_untracked()
                && auth_browser::hash().starts_with("#register-verify")
            {
                return;
            }
            history_blocked.set(false);
            crate::live::signup::leave(signup, preview);
        }),
        Callback::new(move |()| crate::live::signup::cancel(signup, preview)),
    );
    let state = RwSignal::new(AccountViewState::default());
    let discovery = RwSignal::new(PublicDiscoveryState::default());
    let registration = RwSignal::new(RegistrationState::default());
    let locale = RwSignal::new(Locale::Vietnamese);
    let theme = RwSignal::new(Theme::System);
    let main = NodeRef::<leptos::html::Main>::new();
    let nav = LiveNavigation::new(state, main, preview);
    if !network_allowed.get_untracked() {
        nav.route.set(Route::Auth(AuthRoute::register(
            crate::application::auth_navigation::ReturnDestination::Home,
        )));
    }
    let previous_route = StoredValue::new(nav.route.get_untracked());
    Effect::new(move |_| {
        let current = nav.route.get();
        if previous_route.get_value() != current {
            previous_route.set_value(current);
            let proof_destination = Route::Auth(AuthRoute::register(
                crate::application::auth_navigation::ReturnDestination::Home,
            ));
            // Keep a newly captured proof through its own sanitized route change. Every
            // subsequent leave still clears it and invalidates late request completions.
            let entering_captured_link =
                incoming_link_navigation.get_value() && current == proof_destination;
            incoming_link_navigation.set_value(false);
            if !entering_captured_link {
                crate::live::signup::leave(signup, preview);
            }
        }
    });
    #[cfg(target_arch = "wasm32")]
    if !preview && network_allowed.get_untracked() {
        crate::live::runtime::mount(state);
    }
    if preview {
        let _ = crate::features::auth_preview::guest(state);
        registration.update(|current| {
            if let Some(issued) = current.begin() {
                current.finish(
                    issued,
                    Ok(cantos_contracts::theatre_identity::Configuration {
                        login: cantos_contracts::theatre_identity::LoginMethod::Oidc,
                        registration:
                            cantos_contracts::theatre_identity::Registration::EmailVerification {},
                    }),
                );
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
    } else if network_allowed.get_untracked() {
        refresh_discovery(discovery);
        refresh_registration(registration);
    }
    Effect::new(move |_| platform::set_title(&messages::page_title(locale.get(), nav.route.get())));
    Effect::new(move |_| platform::set_document_language(locale.get().lang()));
    let on_login = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        nav.open_auth(AuthRoute::login(nav.origin()));
    });
    let on_register = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        nav.open_auth(AuthRoute::register(nav.origin()));
    });
    let on_refresh = Callback::new(move |()| {
        if preview {
            state.update(|s| {
                let _ = s.begin_refresh();
            });
        } else if network_allowed.get_untracked() {
            refresh_account(state);
        }
    });
    let on_logout = Callback::new(move |()| {
        crate::live::signup::cancel(signup, preview);
        nav.logout(state);
    });
    let on_signup_submit = Callback::new(move |()| {
        if registration.get_untracked().phase()
            != crate::application::registration::RegistrationPhase::Available
        {
            return;
        }
        if preview {
            signup.update(|current| {
                let _ = current.begin_submit();
            });
        } else if network_allowed.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            crate::live::signup::submit(signup);
        }
    });
    let on_signup_verify = Callback::new(move |()| {
        if preview {
            signup.update(|current| {
                let _ = current.begin_verify();
            });
        } else if network_allowed.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            crate::live::signup::verify(signup);
        }
    });
    let on_signup_new = Callback::new(move |()| crate::live::signup::cancel(signup, preview));
    let on_signup_login = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        nav.open_auth(AuthRoute::login(nav.origin()));
    });
    let on_membership = Callback::new(move |()| {
        auth_browser::go(
            nav.route,
            Route::Membership(MembershipReturn::Account),
            false,
        )
    });
    let on_clear_unsafe_link = Callback::new(move |()| {
        // Reload only after successful scrub. A fresh mount restores every normal lifecycle
        // listener once; failed scrub/reload keeps this owner blocked and exposes no signup IO.
        let _ = signup_browser::discard_link_and_reload();
    });
    view! {
        <super::LiveFrame theme>
            <crate::features::signup::SignupLinkSafety blocked=history_blocked locale on_retry=on_clear_unsafe_link>
            {preview.then(|| view! { <crate::features::auth_preview::AuthPreviewControls state locale on_cancel=Callback::new(move |()| { crate::live::signup::cancel(signup, preview); nav.cancel(); })/> })}
            {preview.then(|| view! { <crate::features::signup::SignupPreviewControls state=signup locale/> })}
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
                                Route::Auth(auth) => view! { <AuthPage state registration signup locale route=auth
                                    on_back=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.back(); })
                                    on_register on_login=Callback::new(move |()| { crate::live::signup::leave(signup, preview); if network_allowed.get_untracked() { nav.login(state); } })
                                    on_home=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.cancel(); auth_browser::go(nav.route, Route::Discover, false); })
                                    on_membership on_refresh on_logout on_registration_refresh=Callback::new(move |()| { if !preview && network_allowed.get_untracked() { refresh_registration(registration); } })
                                    on_signup_submit on_signup_verify on_signup_new on_signup_login on_signup_cancel=Callback::new(move |()| {
                                        crate::live::signup::leave(signup, preview); nav.open_auth(AuthRoute::login(auth.origin));
                                    })/> }.into_any(),
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
            </crate::features::signup::SignupLinkSafety>
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
