//! Public discovery is independent of account availability; auth is always an explicit route.
use crate::{
    app::{live_navigation::LiveNavigation, routes::Route},
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthRoute, MembershipReturn},
        player_session::PlayerSession,
        public_discovery::PublicDiscoveryState,
        public_listen::{ListenLabel, ListenOutcome, ListenState, NowPlaying},
        public_work::{Request as WorkRequest, WorkDetailPhase, WorkDetailState},
        registration::RegistrationState,
        signup::SignupState,
    },
    core::{playback::Event as PlayerEvent, public_track::ReleaseRequest},
    features::{
        account::LiveAccount,
        auth::AuthPage,
        common::PersonGlyph,
        library::LiveLibrary,
        live_player::{LiveMiniPlayer, LivePlayer, LivePlayerScreen},
        public_home::{self, PublicHome},
        public_work::{ListenControls, ListenTarget, PublicWorkDetail},
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

/// Issue one public Work request and the automatic page requests it implies. Focus follows only
/// the listener intent that issued it: a detail retry focuses the heading, a page intent its
/// first new row, its failure or the list status. Automatic page one never moves focus.
fn run_work_request(state: RwSignal<WorkDetailState>, request: WorkRequest, focus: bool) {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::application::public_work::Step;
        leptos::task::spawn_local(async move {
            let (mut step, page_focus) = match request {
                WorkRequest::Work(ticket) => {
                    let result = crate::live::public_work::load_work(ticket.work()).await;
                    let Some(step) = state.try_update(|s| s.finish_work(&ticket, result)) else {
                        return;
                    };
                    if focus && step != Step::Ignored {
                        platform::focus_by_id(crate::features::public_work::HEADING_ID.to_owned());
                    }
                    (step, false)
                }
                WorkRequest::Page(ticket) => (Step::Continue(ticket), focus),
            };
            while let Step::Continue(ticket) = step {
                let result = crate::live::public_work::load_units(&ticket).await;
                let Some(next) = state.try_update(|s| s.finish_page(&ticket, result)) else {
                    return;
                };
                if next == Step::Settled && page_focus {
                    if let Some(target) =
                        state.try_with_untracked(crate::features::public_work::page_focus_target)
                    {
                        platform::focus_by_id(target);
                    }
                }
                step = next;
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, request, focus);
}

/// The single player of the live shell. Native builds have no audio element and use an inert
/// port; the browser build owns one `HTMLAudioElement` for the page's life.
fn live_player() -> PlayerSession {
    #[cfg(target_arch = "wasm32")]
    {
        PlayerSession::with_port(crate::platform::audio::BrowserAudio::new)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        PlayerSession::new(crate::infrastructure::inert_media::InertMedia)
    }
}

/// Resolve the chosen release's manifest, then hand the admitted source to the one player. The
/// ticket fences the answer: a newer choice makes this one a no-op.
fn run_listen(
    listen: RwSignal<ListenState>,
    player: PlayerSession,
    now_playing: RwSignal<Option<NowPlaying>>,
    ticket: crate::application::public_listen::ListenTicket,
) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        let result = match web_sys::AbortController::new() {
            Ok(controller) => {
                crate::live::public_media::load_manifest(ticket.publication(), controller).await
            }
            Err(_) => Err(crate::application::public_work::CatalogReadError::Unavailable),
        };
        let Some(outcome) = listen.try_update(|state| state.finish(&ticket, result)) else {
            return;
        };
        if let ListenOutcome::Select(selection) = outcome {
            let crate::application::public_listen::Selection { release, label } = *selection;
            let Some(source) =
                crate::core::playback::Source::public(release.track(), release.duration_ms())
            else {
                return;
            };
            now_playing.set(Some(NowPlaying::new(&release, label)));
            player.dispatch(PlayerEvent::SourceSelected {
                source,
                position_ms: 0,
            });
            player.dispatch(PlayerEvent::PlayRequested);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (listen, player, now_playing, ticket, ListenOutcome::Failed);
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
            if history_blocked.get_untracked() && signup_browser::location_contains_proof() {
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
    let work_detail = RwSignal::new(WorkDetailState::default());
    let locale = RwSignal::new(Locale::Vietnamese);
    let theme = RwSignal::new(Theme::System);
    let main = NodeRef::<leptos::html::Main>::new();
    // One owner for the page: the player, the listener's intent and the display facts of what
    // plays. Routes and Work details only read them and send intents.
    let player = live_player();
    let listen = RwSignal::new(ListenState::default());
    let now_playing = RwSignal::new(None::<NowPlaying>);
    let live = LivePlayer {
        player,
        now_playing,
        locale,
    };
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
    // The live detail is keyed by the route's server ID; leaving fences every outstanding read.
    Effect::new(move |_| match nav.route.get() {
        Route::PublicWork(id) => {
            let mut issued = None;
            work_detail.update(|s| issued = s.open(id));
            match issued {
                Some(ticket) if !preview && network_allowed.get_untracked() => {
                    run_work_request(work_detail, WorkRequest::Work(ticket), false)
                }
                // Preview mode has no catalog network, and an unsafe link blocks it until cleared:
                // both settle as the honest unavailable state, which Retry leaves once allowed.
                Some(ticket) => work_detail.update(|s| {
                    let _ = s.finish_work(
                        &ticket,
                        Err(crate::application::public_work::CatalogReadError::Unavailable),
                    );
                }),
                None => (),
            }
        }
        _ => {
            if work_detail.with_untracked(|s| s.work_id().is_some()) {
                work_detail.update(|s| {
                    let _ = s.leave();
                });
            }
        }
    });
    let on_open_work = Callback::new(move |id| nav.open_work(id, public_home::work_link_id(id)));
    let on_work_back = Callback::new(move |()| nav.close_work());
    let on_listen = Callback::new(move |target: ListenTarget| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        // The release comes from the open Work and the unit the listener chose, never a slug.
        let Some((work, label)) = work_detail.with_untracked(|s| {
            let id = s.work_id()?;
            match s.phase() {
                WorkDetailPhase::Ready { work, .. } => Some((
                    id,
                    ListenLabel {
                        work_title: work.title.clone(),
                        unit_title: target.title.clone(),
                        language: work.language.clone(),
                    },
                )),
                _ => None,
            }
        }) else {
            return;
        };
        let Some(request) = ReleaseRequest::parse(work, &target.unit_id, &target.publication_id)
        else {
            return;
        };
        // The same release already in the player resumes; nothing is fetched or replaced.
        let resumes = player.source.get_untracked().is_some_and(|source| {
            matches!(source.track(), crate::core::playback::TrackId::Public(track)
                if track.publication() == request.publication() && track.unit() == request.unit())
        });
        if resumes {
            player.dispatch(PlayerEvent::PlayRequested);
            return;
        }
        let mut issued = None;
        listen.update(|s| issued = s.begin(request, label));
        if let Some(ticket) = issued {
            run_listen(listen, player, now_playing, ticket);
        }
    });
    let listen_controls = ListenControls {
        player,
        listen,
        on_listen,
    };
    let on_work_retry = Callback::new(move |()| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        work_detail.update(|s| issued = s.retry());
        if let Some(request) = issued {
            run_work_request(work_detail, request, true);
        }
    });
    let on_work_more = Callback::new(move |()| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        work_detail.update(|s| issued = s.load_more());
        if let Some(ticket) = issued {
            run_work_request(work_detail, WorkRequest::Page(ticket), true);
        }
    });
    Effect::new(move |_| {
        let route = nav.route.get();
        let title = match route {
            Route::PublicWork(id) => work_detail.with(|s| match s.phase() {
                WorkDetailPhase::Ready { work, .. } if s.work_id() == Some(id) => {
                    Some(messages::format(
                        Key::PageTitle,
                        locale.get(),
                        &[("page", work.title.clone())],
                    ))
                }
                _ => None,
            }),
            _ => None,
        };
        platform::set_title(&title.unwrap_or_else(|| messages::page_title(locale.get(), route)));
    });
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
    let on_signup_setup = Callback::new(move |()| {
        if preview {
            signup.update(|current| {
                let _ = current.take_setup_url();
            });
        } else if network_allowed.get_untracked() {
            crate::live::signup::open_setup(signup);
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
                                Route::Auth(auth) => view! { <AuthPage state registration signup locale route=auth preview_icons=preview
                                    on_back=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.back(); })
                                    on_register on_login=Callback::new(move |()| { crate::live::signup::leave(signup, preview); if network_allowed.get_untracked() { nav.login(state, registration.get_untracked()); } })
                                    on_home=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.cancel(); auth_browser::go(nav.route, Route::Discover, false); })
                                    on_membership on_refresh on_logout on_registration_refresh=Callback::new(move |()| { if !preview && network_allowed.get_untracked() { refresh_registration(registration); } })
                                    on_signup_submit on_signup_verify on_signup_new on_signup_login on_signup_setup on_signup_cancel=Callback::new(move |()| {
                                        crate::live::signup::cancel_explicit(signup, preview); nav.open_auth(AuthRoute::login(auth.origin));
                                    })/> }.into_any(),
                                Route::Account => view! { <LiveAccount state locale theme registration on_registration_refresh=Callback::new(move |()| { if !preview && network_allowed.get_untracked() { refresh_registration(registration); } }) on_login on_register on_logout on_refresh on_membership/> }.into_any(),
                                Route::Settings => view! { <LivePreferences locale theme/> }.into_any(),
                                Route::Library | Route::History => view! {
                                    <LiveLibrary state locale on_page=Callback::new(move |(section, restart)| {
                                        if preview || !network_allowed.get_untracked() { return; }
                                        #[cfg(target_arch = "wasm32")]
                                        crate::live::runtime::load_library(state, section, restart, 1, true);
                                        #[cfg(not(target_arch = "wasm32"))]
                                        let _ = (section, restart);
                                    })/>
                                    <LibraryAccountEntry state locale on_account=Callback::new(move |_|nav.open_account())/>
                                }.into_any(),
                                Route::Membership(member) => view! {
                                    <section class="theatre-membership-information" aria-labelledby="theatre-membership-heading">
                                        {move || AccessibleName::new(crate::features::auth_copy::text(membership_back_key(member),locale.get())).ok().map(|name| view! { <BackButton id="theatre-membership-back" name on_back=Callback::new(move |_| nav.back())/> })}
                                        <h1 id="theatre-membership-heading">{move || messages::text(Key::MembershipHeading,locale.get())}</h1>
                                        <p>{move || messages::text(Key::MembershipUnavailable,locale.get())}</p>
                                        <Button id="theatre-membership-login" variant=ButtonVariant::Tonal focus_on_press=true on_press=Callback::new(move |_| nav.open_auth(AuthRoute::login(nav.origin())))>{move || messages::text(Key::SignIn,locale.get())}</Button>
                                    </section>
                                }.into_any(),
                                Route::PublicWork(_) => view! { <PublicWorkDetail state=work_detail locale listen=listen_controls on_back=on_work_back on_retry=on_work_retry on_load_more=on_work_more/> }.into_any(),
                                Route::Player => view! { <LivePlayerScreen live/> }.into_any(),
                                _ => view! { <PublicHome state=discovery locale on_login on_register on_open_work on_refresh=Callback::new(move |()| { if !preview { refresh_discovery(discovery); } })/> }.into_any(),
                            }}
                        </main>
                    </super::LiveContainer>
                </div>
                <LiveMiniPlayer live route=nav.route/>
            </div>
            </crate::features::signup::SignupLinkSafety>
        </super::LiveFrame>
    }
}

#[component]
fn LibraryAccountEntry(
    #[prop(optional)] on_account: Option<Callback<()>>,
    state: RwSignal<AccountViewState>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    view! {
        <Show when=move || state.get().phase() != SessionPhase::SignedIn>
            <a id="theatre-library-account" class="theatre-action theatre-action-tonal" href="#account" on:click=move |event|{if let Some(callback)=on_account{event.prevent_default();callback.run(());}}>
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
