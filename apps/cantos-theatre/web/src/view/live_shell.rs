//! Public discovery is independent of account availability; auth is always an explicit route.
use crate::application::error_writer::update_errors;
use crate::application::errors::{
    ErrorAction, ErrorCenter, ErrorKind, ErrorNotice, ErrorOperation,
};
use crate::contract::library::{AudioPresentation, WorkPresentation};
use crate::{
    app::{live_navigation::LiveNavigation, routes::Route},
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthRoute, MembershipReturn},
        library_sync::audio_identity_from_source,
        player_session::PlayerSession,
        public_discovery::{DiscoveryQuery, DiscoveryTicket, PublicDiscoveryState},
        public_listen::{ListenLabel, ListenOutcome, ListenState, NowPlaying},
        public_work::{Request as WorkRequest, WorkDetailPhase, WorkDetailState},
        signup::SignupState,
    },
    core::{playback::Event as PlayerEvent, public_track::ReleaseRequest},
    features::{
        account::LiveAccount,
        auth::AuthPage,
        common::icon,
        library::{LibraryActions, LiveLibrary},
        live_player::{LiveMiniPlayer, LivePlayer, LivePlayerScreen, PlayerSyncControls},
        public_home::{self, PublicHome},
        public_work::{ListenControls, ListenTarget, PublicWorkDetail, WorkSyncControls},
    },
    messages::{self, Key, Locale},
    platform::{self, auth_browser, signup_browser},
};
use cantos_ui::{
    AccessibleName, BackButton, Button, ButtonVariant, Icon, IconName, Logo, LogoTone, LogoVariant,
    Theme,
};
use leptos::prelude::*;

// The existing app Home glyph and the admitted canonical Library outline.
const NAV_ICON_DISCOVER: &str = "home";

const CANTOS_BRAND: &str = "Cantos";

/// One app-owned brand link; the admitted shared Logo supplies the original decorative symbol.
#[component]
pub(crate) fn LiveBrand() -> impl IntoView {
    view! {
        <a class="theatre-brand theatre-live-brand" href="#discover" aria-label=CANTOS_BRAND>
            <Logo variant=LogoVariant::Symbol tone=LogoTone::Light class="theatre-logo-light"/>
            <Logo variant=LogoVariant::Symbol tone=LogoTone::Dark class="theatre-logo-dark"/>
            <strong class="theatre-live-brand-name">{CANTOS_BRAND}</strong>
        </a>
    }
}

/// One route projection reused in the wide rail and the browser-native modal drawer.
/// Me and navigation borrow the same session-bound generated identity.
#[component]
fn PrimaryNavigation(
    route: RwSignal<Route>,
    locale: RwSignal<Locale>,
    state: ReadSignal<AccountViewState>,
) -> impl IntoView {
    view! {
        <nav class="theatre-nav" aria-label=move || messages::text(Key::MainNavigation, locale.get())>
            <a class="theatre-nav-link" href="#discover" aria-current=move || (route.get().primary_navigation() == Some(Route::Discover)).then_some("page")>{icon(NAV_ICON_DISCOVER)}<span class="theatre-nav-label">{move || messages::text(Key::Discover, locale.get())}</span><span class="theatre-nav-selected" aria-hidden="true">{icon(super::NAV_ICON_SELECTED)}</span></a>
            <a class="theatre-nav-link" href="#library" aria-current=move || (route.get().primary_navigation() == Some(Route::Library)).then_some("page")><Icon name=IconName::LibrarySpines/><span class="theatre-nav-label">{move || messages::text(Key::Library, locale.get())}</span><span class="theatre-nav-selected" aria-hidden="true">{icon(super::NAV_ICON_SELECTED)}</span></a>
            <a class="theatre-nav-link" href="#account" aria-current=move || (route.get().primary_navigation() == Some(Route::Account)).then_some("page")><crate::features::guest_identity::ProfileAvatar navigation=true identity=Signal::derive(move || state.with(|s| s.presentation().cloned()))/><span class="theatre-nav-label">{move || state.with(|s| s.presentation().map(|p| p.alias.clone())).unwrap_or_else(|| messages::text(Key::Me, locale.get()).to_owned())}</span><span class="theatre-nav-selected" aria-hidden="true">{icon(super::NAV_ICON_SELECTED)}</span></a>
        </nav>
    }
}

fn sync_admission(
    state: ReadSignal<AccountViewState>,
) -> Memo<Option<crate::application::account::RequestFence>> {
    // Point reads mutate account projections. Only a changed admission may retrigger their
    // effect; observing the whole signal would let even an unchanged read guard reschedule it.
    Memo::new(move |_| state.with(AccountViewState::fence))
}

fn refresh_account(state: RwSignal<AccountViewState>) {
    #[cfg(target_arch = "wasm32")]
    crate::live::runtime::refresh(state);
    #[cfg(not(target_arch = "wasm32"))]
    let _ = state;
}
/// Issue one discovery page request and the automatic page requests it implies (an empty page
/// that still has a cursor). Focus follows only the listener intent that issued it: the first
/// row a continuation added, else the polite list status. A search never moves focus.
fn run_discovery(state: RwSignal<PublicDiscoveryState>, ticket: DiscoveryTicket, focus: bool) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        use crate::application::public_discovery::Step;
        let mut ticket = ticket;
        loop {
            let result = crate::live::discovery::load(&ticket).await;
            let Some(step) = state.try_update(|s| s.finish(&ticket, result)) else {
                return;
            };
            match step {
                Step::Continue(next) => ticket = next,
                Step::Settled => {
                    if focus {
                        if let Some(target) =
                            state.try_with_untracked(public_home::page_focus_target)
                        {
                            platform::focus_by_id(target);
                        }
                    }
                    return;
                }
                Step::Ignored => return,
            }
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, ticket, focus);
}

/// Issue one public Work request and the automatic page requests it implies. Focus follows only
/// the listener intent that issued it: a detail retry focuses the heading, a page intent its
/// first new row, its failure or the list status. Automatic page one never moves focus.
fn run_work_request(
    state: RwSignal<WorkDetailState>,
    request: WorkRequest,
    focus: bool,
    account: RwSignal<AccountViewState>,
    fence: Option<crate::application::account::RequestFence>,
    requests: super::shared_runtime::Requests,
) {
    #[cfg(target_arch = "wasm32")]
    {
        use crate::application::public_work::Step;
        leptos::task::spawn_local(async move {
            let (mut step, page_focus) = match request {
                WorkRequest::Work(ticket) => {
                    let result = match &fence {
                        Some(fence) => match requests.with_value(|r| r.start()) {
                            Ok(controller) => {
                                crate::live::shared::browser::work(ticket.work(), fence, controller)
                                    .await
                            }
                            Err(error) => Err(error),
                        },
                        None => crate::live::public_work::load_work(ticket.work()).await,
                    };
                    if fence
                        .as_ref()
                        .is_some_and(|f| !account.with_untracked(|a| a.accepts(f)))
                    {
                        return;
                    }
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
                let result = match &fence {
                    Some(fence) => match requests.with_value(|r| r.start()) {
                        Ok(controller) => {
                            crate::live::shared::browser::units(&ticket, fence, controller).await
                        }
                        Err(error) => Err(error),
                    },
                    None => crate::live::public_work::load_units(&ticket).await,
                };
                if fence
                    .as_ref()
                    .is_some_and(|f| !account.with_untracked(|a| a.accepts(f)))
                {
                    return;
                }
                let Some(next) = state.try_update(|s| s.finish_page(&ticket, result)) else {
                    return;
                };
                if next == Step::Settled && page_focus {
                    if let Some(target) =
                        state.try_with_untracked(crate::features::public_work::page_focus_target)
                    {
                        platform::focus_after_unit_page(target);
                    }
                }
                step = next;
            }
        });
    }
    #[cfg(not(target_arch = "wasm32"))]
    let _ = (state, request, focus, account, fence, requests);
}

/// The single player of the live shell. Native builds have no audio element and use an inert
/// port; the browser build owns one `HTMLAudioElement` for the page's life.
fn live_player(account: RwSignal<AccountViewState>) -> PlayerSession {
    #[cfg(target_arch = "wasm32")]
    {
        PlayerSession::with_port(move |session| {
            crate::platform::audio::BrowserAudio::new(session, account)
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = account;
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
    account: RwSignal<AccountViewState>,
    fence: Option<crate::application::account::RequestFence>,
    requests: super::shared_runtime::Requests,
) {
    #[cfg(target_arch = "wasm32")]
    leptos::task::spawn_local(async move {
        let result = match web_sys::AbortController::new() {
            Ok(controller) => match ticket.delivery() {
                crate::core::public_track::DeliveryScope::Public => {
                    crate::live::public_media::load_manifest(ticket.publication(), controller).await
                }
                crate::core::public_track::DeliveryScope::Shared(session) => {
                    match fence.as_ref().filter(|f| {
                        f.session() == session.to_string()
                            && account.with_untracked(|a| a.accepts(f))
                    }) {
                        Some(fence) => match requests.with_value(|r| r.start()) {
                            Ok(controller) => {
                                crate::live::shared::browser::manifest(
                                    ticket.publication(),
                                    fence,
                                    controller,
                                )
                                .await
                            }
                            Err(error) => Err(error),
                        },
                        None => return,
                    }
                }
            },
            Err(_) => Err(crate::application::public_work::CatalogReadError::Unavailable),
        };
        if fence
            .as_ref()
            .is_some_and(|f| !account.with_untracked(|a| a.accepts(f)))
        {
            return;
        }
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
    let _ = (
        listen,
        player,
        now_playing,
        ticket,
        ListenOutcome::Failed,
        account,
        fence,
        requests,
    );
}

/// The diagnostic guest and an empty catalog of the explicit auth preview. It reads no network
/// and exists only in a build with the `preview` feature.
#[cfg(feature = "preview")]
fn seed_preview(state: RwSignal<AccountViewState>, discovery: RwSignal<PublicDiscoveryState>) {
    let _ = crate::features::auth_preview::guest(state);
    discovery.update(|s| {
        if let Some(issued) = s.begin(DiscoveryQuery::default()) {
            s.finish(
                &issued,
                Ok(crate::contract::discovery::MetadataWorkPage {
                    items: vec![],
                    next_cursor: None,
                }),
            );
        }
    });
}
#[cfg(not(feature = "preview"))]
fn seed_preview(_: RwSignal<AccountViewState>, _: RwSignal<PublicDiscoveryState>) {}

/// Fixture controls of the explicit auth preview; a production build draws nothing.
#[cfg(feature = "preview")]
#[component]
fn PreviewControls(
    preview: bool,
    state: RwSignal<AccountViewState>,
    signup: RwSignal<SignupState>,
    locale: RwSignal<Locale>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    view! {
        {preview.then(|| view! { <crate::features::auth_preview::AuthPreviewControls state locale on_cancel/> })}
        {preview.then(|| view! { <crate::features::signup::SignupPreviewControls state=signup locale/> })}
    }
}
#[cfg(not(feature = "preview"))]
#[component]
fn PreviewControls(
    preview: bool,
    state: RwSignal<AccountViewState>,
    signup: RwSignal<SignupState>,
    locale: RwSignal<Locale>,
    on_cancel: Callback<()>,
) -> impl IntoView {
    let _ = (preview, state, signup, locale, on_cancel);
}

#[component]
pub fn LiveBoundary(#[prop(optional)] preview: bool) -> impl IntoView {
    // Only a build with the `preview` feature can run the diagnostic shell; a production build
    // ignores the property, so no caller can switch fixtures on.
    #[cfg(not(feature = "preview"))]
    let preview = {
        let _ = preview;
        false
    };
    // Capture and scrub a proof before navigation construction, effects or any network request.
    let captured = signup_browser::capture_link();
    let history_blocked =
        RwSignal::new(captured == Err(signup_browser::CaptureFailure::HistoryUnavailable));
    let configuration_valid = preview || crate::api_endpoint::validate().is_ok();
    let network_allowed = Signal::derive(move || !history_blocked.get() && configuration_valid);
    let errors = RwSignal::new(ErrorCenter::default());
    if !configuration_valid {
        update_errors(errors, |s| {
            s.report(
                ErrorNotice::read(ErrorKind::Configuration, ErrorOperation::LoadCatalog),
                false,
            )
        });
    }
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
    // Own this handle at shell lifetime. Creating it inside the Library slot lets the route
    // render owner dispose it while a shared-row reaction is still queued during navigation.
    let shared_account = state.read_only();
    let discovery = RwSignal::new(PublicDiscoveryState::default());
    // RAM-only notice for the sign-in page after a finished signup. It is never a session.
    let account_created = RwSignal::new(false);
    let work_detail = RwSignal::new(WorkDetailState::default());
    let work_scope = RwSignal::new(None::<crate::application::account::RequestFence>);
    let shared = RwSignal::new(crate::application::shared_access::SharedAccess::default());
    let shared_requests = StoredValue::new_local(crate::live::shared::Requests::default());
    let admitted_sync = sync_admission(state.read_only());
    on_cleanup(move || {
        #[cfg(target_arch = "wasm32")]
        shared_requests.with_value(|r| r.abort());
    });
    let preference = crate::localization::load_preference();
    let locale = RwSignal::new(preference.locale);
    let locale_warning = RwSignal::new(preference.warning);
    let theme = RwSignal::new(Theme::System);
    // The only writer of the locale preference: the header and Account selectors both call it.
    let on_locale = Callback::new(move |value: String| {
        if let Some(selected) = Locale::from_lang(&value) {
            locale.set(selected);
            locale_warning.set(crate::localization::save_preference(selected).err());
        } else {
            locale_warning.set(Some(cantos_localization::PreferenceWarning::Unsupported));
        }
    });
    let main = NodeRef::<leptos::html::Main>::new();
    // One owner for the page: the player, the listener's intent and the display facts of what
    // plays. Routes and Work details only read them and send intents.
    let player = live_player(state);
    let listen = RwSignal::new(ListenState::default());
    let now_playing = RwSignal::new(None::<NowPlaying>);
    let live = LivePlayer {
        player,
        now_playing,
        locale,
    };
    player.observe_playback(move |obs| {
        if !preview && network_allowed.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            crate::live::runtime::handle_playback_observation(state, now_playing, obs);
            #[cfg(not(target_arch = "wasm32"))]
            let _ = (state, now_playing, obs);
        }
    });
    let nav = LiveNavigation::new(state, main, preview);
    if history_blocked.get_untracked() {
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
            if !matches!(current, Route::Auth(auth) if auth.step == crate::application::auth_navigation::AuthStep::Login)
            {
                account_created.set(false);
            }
        }
    });
    #[cfg(target_arch = "wasm32")]
    if !preview && network_allowed.get_untracked() {
        crate::live::runtime::mount(state, player, now_playing, network_allowed);
    }
    if preview {
        seed_preview(state, discovery);
    }
    // The discovery link owns the search: the walk follows it, and a state that already answers
    // this search (back from a Work) is kept with its rows and cursor.
    Effect::new(move |_| {
        if !matches!(nav.route.get(), Route::Discover | Route::Explore) {
            return;
        }
        let query = nav.query.get();
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        discovery.update(|s| issued = s.open(query));
        if let Some(ticket) = issued {
            run_discovery(discovery, ticket, false);
        }
    });
    // Route and admission changes cancel outstanding private I/O before publishing another scope.
    Effect::new(
        move |previous: Option<(Route, Option<crate::application::account::RequestFence>)>| {
            let route = nav.route.get();
            let fence = admitted_sync.get();
            if previous.as_ref() == Some(&(route, fence.clone())) {
                return (route, fence);
            }
            #[cfg(target_arch = "wasm32")]
            shared_requests.with_value(|r| r.abort());
            let account_changed = previous.as_ref().is_some_and(|(_, old)| *old != fence);
            if account_changed {
                listen.update_untracked(ListenState::clear);
                if player.source.get_untracked().is_some_and(|source|matches!(source.track(),crate::core::playback::TrackId::Public(track) if track.is_shared())) {
                player.dispatch(PlayerEvent::AccessCleared); now_playing.set(None);
            }
            } else if previous.as_ref().is_some_and(|(old, _)| *old != route) {
                listen.update_untracked(ListenState::clear);
            }
            shared.update(|s| {
                s.observe(fence.clone());
                if !matches!(route, Route::Library | Route::History) {
                    s.catalog.clear_shared();
                    if let crate::application::shared_access::ClaimPhase::Pending(op) = &s.claim {
                        s.claim =
                            crate::application::shared_access::ClaimPhase::Unknown(op.clone());
                    }
                }
            });
            match route {
                Route::PublicWork(id) | Route::SharedWork(id) => {
                    let is_shared = matches!(route, Route::SharedWork(_));
                    if is_shared || work_scope.with_untracked(Option::is_some) {
                        work_detail.update_untracked(|s| {
                            s.leave();
                        });
                    }
                    let scope = if is_shared { fence.clone() } else { None };
                    work_scope.set(scope.clone());
                    if !is_shared || scope.is_some() {
                        // The route/scope signal mounts the next view. Do not notify the old
                        // row controls while their owner is being torn down by that navigation.
                        if let Some(ticket) =
                            work_detail.try_update_untracked(|s| s.open(id)).flatten()
                        {
                            if !preview && network_allowed.get_untracked() {
                                run_work_request(
                                    work_detail,
                                    WorkRequest::Work(ticket),
                                    false,
                                    state,
                                    scope,
                                    shared_requests,
                                );
                            } else {
                                work_detail.update(|s|{s.finish_work(&ticket,Err(crate::application::public_work::CatalogReadError::Unavailable));});
                            }
                        }
                    } else {
                        work_detail.update_untracked(|s| {
                            s.leave();
                        });
                    }
                }
                _ => {
                    // Purge authority immediately without waking controls of the route which
                    // just unmounted. Late network answers still fail the generation fence.
                    work_detail.update_untracked(|s| {
                        s.leave();
                    });
                    work_scope.set(None);
                }
            }
            if matches!(route, Route::Library | Route::History)
                && !preview
                && network_allowed.get_untracked()
            {
                if let Some(fence) = fence.clone() {
                    if let Some(ticket) = shared
                        .try_update(|s| s.catalog.begin(DiscoveryQuery::default()))
                        .flatten()
                    {
                        super::shared_runtime::run_catalog(
                            shared,
                            state,
                            ticket,
                            fence,
                            shared_requests,
                        );
                    }
                }
            }
            (route, fence)
        },
    );
    Effect::new(move |_| {
        let media_denied=matches!(player.status.get(),crate::core::playback::Status::Failed(crate::core::playback::Failure::Unavailable|crate::core::playback::Failure::Unauthenticated|crate::core::playback::Failure::Forbidden)) && player.source.get().is_some_and(|source|matches!(source.track(),crate::core::playback::TrackId::Public(track) if track.is_shared()));
        let manifest_denied=listen.with(|s|matches!(s.phase(),crate::application::public_listen::ListenPhase::Failed(request,crate::application::public_listen::ListenFailure::NotFound) if matches!(request.delivery(),crate::core::public_track::DeliveryScope::Shared(_))));
        if media_denied || manifest_denied {
            player.dispatch(PlayerEvent::AccessCleared);
            now_playing.set(None);
            listen.update(ListenState::clear);
            work_detail.update(WorkDetailState::refuse_access);
            shared.update(|s| {
                s.catalog.clear_shared();
            });
        }
    });
    // When signed in on a public Work or with an active player source, load point Saved/Progress
    // registers that have not yet been observed in the current session.
    Effect::new(move |_| {
        if matches!(nav.route.get(), Route::SharedWork(_)) {
            return;
        }
        if preview || !network_allowed.get_untracked() {
            return;
        }
        if admitted_sync.get().is_none() {
            return;
        }
        let (work_info, unit_infos) = work_detail.with(|w| {
            let Some(id) = w.work_id() else {
                return (None, Vec::new());
            };
            match w.phase() {
                WorkDetailPhase::Ready { work, units } => {
                    let first_pub = units
                        .rows()
                        .iter()
                        .find_map(|u| u.playback.as_ref().map(|p| p.publication_id.clone()));
                    let work_pres = first_pub.map(|first_pub| WorkPresentation {
                        work_id: id.to_string(),
                        publication_id: first_pub,
                        title: work.title.clone(),
                        kind: work.kind,
                        language: work.language.clone(),
                    });
                    let pubs = units
                        .rows()
                        .iter()
                        .filter_map(|u| {
                            let playback = u.playback.as_ref()?;
                            Some((
                                playback.publication_id.clone(),
                                AudioPresentation {
                                    work: WorkPresentation {
                                        work_id: id.to_string(),
                                        publication_id: playback.publication_id.clone(),
                                        title: work.title.clone(),
                                        kind: work.kind,
                                        language: work.language.clone(),
                                    },
                                    unit_id: u.unit_id.clone(),
                                    unit_title: u.title.clone(),
                                },
                            ))
                        })
                        .collect::<Vec<_>>();
                    (Some((id.to_string(), work_pres)), pubs)
                }
                _ => (Some((id.to_string(), None)), Vec::new()),
            }
        });
        #[cfg(target_arch = "wasm32")]
        {
            if let Some((work_id, pres)) = work_info {
                crate::live::runtime::ensure_saved_loaded(state, work_id, pres);
            }
            for (pub_id, pres) in unit_infos {
                crate::live::runtime::ensure_progress_loaded(state, pub_id, Some(pres));
            }
            if let Some(source) = player.source.get() {
                if let Some(audio) = audio_identity_from_source(source) {
                    crate::live::runtime::ensure_progress_loaded(state, audio.publication_id, None);
                }
            }
        }
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (work_info, unit_infos, audio_identity_from_source);
    });
    let on_open_work = Callback::new(move |id| nav.open_work(id, public_home::work_link_id(id)));
    let on_work_back = Callback::new(move |()| {
        if matches!(nav.route.get_untracked(), Route::SharedWork(_)) {
            auth_browser::go(nav.route, Route::Library, false);
        } else {
            nav.close_work();
        }
    });
    let on_listen = Callback::new(move |target: ListenTarget| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        // Restricted transport source exists, but real browser teardown with a private source
        // is not qualified. Keep the control and imperative entrypoint closed until repaired.
        if matches!(nav.route.get_untracked(), Route::SharedWork(_)) {
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
        let Some(mut request) =
            ReleaseRequest::parse(work, &target.unit_id, &target.publication_id)
        else {
            return;
        };
        if matches!(nav.route.get_untracked(), Route::SharedWork(_)) {
            let Some(fence) = work_scope
                .get_untracked()
                .filter(|f| state.with_untracked(|a| a.accepts(f)))
            else {
                return;
            };
            let Some(scoped) = request.shared(fence.session()) else {
                return;
            };
            request = scoped;
        }
        let _ = crate::application::account_writer::AccountWriter::new(state)
            .reset_resume()
            .into_value();
        // The same release already in the player resumes; nothing is fetched or replaced.
        let resumes = player.source.get_untracked().is_some_and(|source| {
            matches!(source.track(), crate::core::playback::TrackId::Public(track)
                if track.publication() == request.publication() && track.unit() == request.unit() && track.delivery()==request.delivery())
        });
        if resumes {
            player.dispatch(PlayerEvent::PlayRequested);
            return;
        }
        let mut issued = None;
        listen.update(|s| issued = s.begin(request, label));
        if let Some(ticket) = issued {
            run_listen(
                listen,
                player,
                now_playing,
                ticket,
                state,
                work_scope.get_untracked(),
                shared_requests,
            );
        }
    });
    let listen_controls = ListenControls {
        player,
        listen,
        on_listen,
    };
    let work_sync_controls = WorkSyncControls {
        account: state.read_only(),
        on_login_to_save: Callback::new(move |()| {
            crate::live::signup::leave(signup, preview);
            nav.open_auth(AuthRoute::login(nav.origin()));
        }),
        on_set_saved: Callback::new(move |(work, saved, pres)| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::set_saved(state, work, saved, pres);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (work, saved, pres);
            }
        }),
        on_retry_saved: Callback::new(move |work| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::retry_saved(state, work);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = work;
            }
        }),
        on_resume: Callback::new(move |(audio, position_ms, resume, pres)| {
            if !preview && network_allowed.get_untracked() {
                listen.update(ListenState::clear);
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::resume_exact_release(
                    state,
                    player,
                    now_playing,
                    audio,
                    position_ms,
                    resume,
                    pres,
                );
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (audio, position_ms, resume, pres);
            }
        }),
    };
    let library_actions = LibraryActions {
        on_unsave: Callback::new(move |(work, pres)| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::set_saved(state, work, false, pres);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (work, pres);
            }
        }),
        on_retry_saved: Callback::new(move |work| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::retry_saved(state, work);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = work;
            }
        }),
        on_resume: Callback::new(move |(audio, position_ms, resume, pres)| {
            if !preview && network_allowed.get_untracked() {
                listen.update(ListenState::clear);
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::resume_exact_release(
                    state,
                    player,
                    now_playing,
                    audio,
                    position_ms,
                    resume,
                    pres,
                );
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (audio, position_ms, resume, pres);
            }
        }),
        on_resolve_progress: Callback::new(move |(publication, keep_local)| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::resolve_progress_conflict(
                    state,
                    player,
                    publication,
                    keep_local,
                );
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (publication, keep_local);
            }
        }),
        on_retry_progress: Callback::new(move |publication| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::retry_progress(state, publication);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = publication;
            }
        }),
        on_prompt_clear_history: Callback::new(move |()| {
            let _ = crate::application::account_writer::AccountWriter::new(state)
                .prompt_clear_history()
                .into_value();
        }),
        on_cancel_clear_history: Callback::new(move |()| {
            let _ = crate::application::account_writer::AccountWriter::new(state)
                .cancel_clear_history()
                .into_value();
        }),
        on_confirm_clear_history: Callback::new(move |()| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::confirm_clear_history(state);
            }
        }),
        on_retry_history: Callback::new(move |()| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::retry_history(state);
            }
        }),
    };
    let player_sync_controls = PlayerSyncControls {
        account: state.read_only(),
        on_mark_completed: Callback::new(move |()| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::mark_completed(state, player, now_playing);
            }
        }),
        on_restart: Callback::new(move |()| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::restart_progress(state, player, now_playing);
            }
        }),
        on_resolve_conflict: Callback::new(move |(publication, keep_local)| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::resolve_progress_conflict(
                    state,
                    player,
                    publication,
                    keep_local,
                );
                #[cfg(not(target_arch = "wasm32"))]
                let _ = (publication, keep_local);
            }
        }),
        on_retry_progress: Callback::new(move |publication| {
            if !preview && network_allowed.get_untracked() {
                #[cfg(target_arch = "wasm32")]
                crate::live::runtime::retry_progress(state, publication);
                #[cfg(not(target_arch = "wasm32"))]
                let _ = publication;
            }
        }),
    };
    let on_work_retry = Callback::new(move |()| {
        update_errors(errors, |s| {
            s.resolve(ErrorOperation::LoadWork);
            s.resolve(ErrorOperation::LoadUnits);
        });
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        work_detail.update(|s| issued = s.retry());
        if let Some(request) = issued {
            run_work_request(
                work_detail,
                request,
                true,
                state,
                work_scope.get_untracked(),
                shared_requests,
            );
        }
    });
    let on_work_more = Callback::new(move |()| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        work_detail.update(|s| issued = s.load_more());
        if let Some(ticket) = issued {
            run_work_request(
                work_detail,
                WorkRequest::Page(ticket),
                true,
                state,
                work_scope.get_untracked(),
                shared_requests,
            );
        }
    });
    Effect::new(move |_| {
        let route = nav.route.get();
        let title = match route {
            Route::PublicWork(id) | Route::SharedWork(id) => {
                work_detail.with(|s| match s.phase() {
                    WorkDetailPhase::Ready { work, .. }
                        if s.work_id() == Some(id)
                            && work_scope.with(|scope| match route {
                                Route::SharedWork(_) => {
                                    scope.as_ref().is_some_and(|f| state.with(|a| a.accepts(f)))
                                }
                                _ => scope.is_none(),
                            }) =>
                    {
                        Some(messages::message(
                            locale.get(),
                            crate::localization::generated::Message::TheatreShellPageTitle {
                                arg_page: &(work.title.clone()),
                            },
                        ))
                    }
                    _ => None,
                })
            }
            Route::NotFound | Route::Work(_) => Some(messages::message(
                locale.get(),
                crate::localization::generated::Message::TheatreShellPageTitle {
                    arg_page: messages::text(Key::LiveNotFound, locale.get()),
                },
            )),
            Route::Queue => Some(messages::message(
                locale.get(),
                crate::localization::generated::Message::TheatreShellPageTitle {
                    arg_page: messages::text(Key::LiveRouteUnavailable, locale.get()),
                },
            )),
            _ => None,
        };
        platform::set_title(&title.unwrap_or_else(|| messages::page_title(locale.get(), route)));
    });
    Effect::new(move |_| platform::set_document_locale(locale.get().lang(), locale.get().dir()));
    let on_login = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        nav.open_auth(AuthRoute::login(nav.origin()));
    });
    let on_register = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        nav.open_auth(AuthRoute::register(nav.origin()));
    });
    let on_search = Callback::new(move |query: DiscoveryQuery| {
        if query != nav.query.get_untracked() {
            nav.search(query);
        } else if !preview && network_allowed.get_untracked() {
            // The same search again is an explicit refresh of its first page.
            let mut issued = None;
            discovery.update(|s| issued = s.begin(query));
            if let Some(ticket) = issued {
                run_discovery(discovery, ticket, false);
            }
        }
    });
    let on_discovery_retry = Callback::new(move |()| {
        update_errors(errors, |s| s.resolve(ErrorOperation::LoadCatalog));
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        discovery.update(|s| issued = s.retry());
        if let Some(ticket) = issued {
            run_discovery(discovery, ticket, true);
        }
    });
    let on_discovery_more = Callback::new(move |()| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        discovery.update(|s| issued = s.load_more());
        if let Some(ticket) = issued {
            run_discovery(discovery, ticket, true);
        }
    });
    let on_discovery_restart = Callback::new(move |()| {
        if preview || !network_allowed.get_untracked() {
            return;
        }
        let mut issued = None;
        discovery.update(|s| issued = s.begin(s.query().clone()));
        if let Some(ticket) = issued {
            run_discovery(discovery, ticket, false);
        }
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
    let confirm_logout = Callback::new(move |()| {
        crate::live::signup::cancel(signup, preview);
        nav.logout(state);
    });
    let logout_open = RwSignal::new(false);
    let on_logout = Callback::new(move |()| logout_open.set(true));
    let on_signup_submit = Callback::new(move |()| {
        if preview {
            signup.update(|current| {
                let _ = current.begin_submit(0);
            });
        } else if network_allowed.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            crate::live::signup::submit(signup);
        }
    });
    // A finished signup is account history, never a session: the owner signs in next.
    let on_signup_completed = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        account_created.set(true);
        nav.open_auth(AuthRoute::login(nav.origin()));
    });
    let on_signup_confirm = Callback::new(move |()| {
        if preview {
            signup.update(|current| {
                let _ = current.begin_complete();
            });
        } else if network_allowed.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            crate::live::signup::complete(signup, on_signup_completed);
        }
    });
    let on_signup_retry = Callback::new(move |()| {
        if preview {
            signup.update(|current| {
                let _ = current.begin_retry(u64::MAX);
            });
        } else if network_allowed.get_untracked() {
            #[cfg(target_arch = "wasm32")]
            crate::live::signup::retry(signup, on_signup_completed);
        }
    });
    #[cfg(not(target_arch = "wasm32"))]
    let _ = on_signup_completed;
    let on_signup_new = Callback::new(move |()| crate::live::signup::cancel(signup, preview));
    let on_signup_login = Callback::new(move |()| {
        crate::live::signup::leave(signup, preview);
        nav.open_auth(AuthRoute::login(nav.origin()));
    });
    let on_login_submit = Callback::new(move |(email, password): (String, String)| {
        crate::live::signup::leave(signup, preview);
        account_created.set(false);
        if network_allowed.get_untracked() {
            nav.login(state, email, password);
        }
    });
    let on_membership = Callback::new(move |()| {
        auth_browser::go(
            nav.route,
            Route::Membership(MembershipReturn::Account),
            false,
        )
    });
    // Observe only admitted current states: stale callbacks have already been rejected by
    // the account/route/request owners. Loading does not resolve a dismissed failure episode.
    Effect::new(move |_| {
        use crate::application::public_discovery::{PublicDiscoveryPhase, Tail};
        if !configuration_valid {
            return;
        }
        let route = nav.route.get();
        discovery.with(|catalog| {
            update_errors(errors, |center| {
                if !matches!(route, Route::Discover | Route::Explore) {
                    center.resolve(ErrorOperation::LoadCatalog);
                } else {
                    match catalog.phase() {
                        PublicDiscoveryPhase::Failed(failure) => center.report(
                            ErrorNotice::read(failure.into(), ErrorOperation::LoadCatalog),
                            false,
                        ),
                        PublicDiscoveryPhase::Ready | PublicDiscoveryPhase::Incomplete => {
                            match catalog.tail() {
                                Tail::Failed(failure) => center.report(
                                    ErrorNotice::read(failure.into(), ErrorOperation::LoadCatalog),
                                    true,
                                ),
                                Tail::Loading => (),
                                _ => center.resolve(ErrorOperation::LoadCatalog),
                            }
                        }
                        PublicDiscoveryPhase::Empty => center.resolve(ErrorOperation::LoadCatalog),
                        _ => (),
                    }
                }
            })
        });
        work_detail.with(|work| {
            update_errors(errors, |center| match work.phase() {
                WorkDetailPhase::Failed(failure) => center.report(
                    ErrorNotice::read((*failure).into(), ErrorOperation::LoadWork),
                    false,
                ),
                WorkDetailPhase::Ready { units, .. } => {
                    center.resolve(ErrorOperation::LoadWork);
                    if let crate::application::public_work::PageStatus::Failed(failure) =
                        units.status()
                    {
                        center.report(
                            ErrorNotice::read((*failure).into(), ErrorOperation::LoadUnits),
                            !units.rows().is_empty(),
                        );
                    } else if !matches!(
                        units.status(),
                        crate::application::public_work::PageStatus::Loading
                    ) {
                        center.resolve(ErrorOperation::LoadUnits);
                    }
                }
                WorkDetailPhase::NotFound | WorkDetailPhase::Idle => {
                    center.resolve(ErrorOperation::LoadWork);
                    center.resolve(ErrorOperation::LoadUnits);
                }
                _ => (),
            })
        });
    });
    let account_observation = Memo::new(move |_| state.with(|s| (s.generation(), s.phase())));
    let observed_account_generation =
        StoredValue::new(state.with_untracked(AccountViewState::generation));
    Effect::new(move |_| {
        let (generation, phase) = account_observation.get();
        if observed_account_generation.get_value() != generation {
            observed_account_generation.set_value(generation);
            update_errors(errors, |c| {
                c.resolve(ErrorOperation::SignIn);
                c.resolve(ErrorOperation::UpdateLibrary);
            });
        }
        // Identity diagnostics belong to the explicit account flow; a guest's public Home
        // stays usable. A fresh account generation clears notices issued for the old account.
        if !matches!(
            nav.route.get(),
            Route::Account | Route::Auth(_) | Route::Library | Route::History
        ) {
            update_errors(errors, |c| c.resolve(ErrorOperation::SignIn));
            return;
        }
        update_errors(errors, |c| match phase {
            SessionPhase::Failed(crate::application::account_view::AccountFailure::Diagnostic(
                kind,
            )) => c.report(ErrorNotice::read(kind, ErrorOperation::SignIn), true),
            SessionPhase::Guest | SessionPhase::SignedIn => c.resolve(ErrorOperation::SignIn),
            _ => (),
        });
    });
    let on_error_action = Callback::new(move |action| {
        let operation = update_errors(errors, |center| center.choose(action))
            .into_value()
            .flatten();
        if let Some(operation) = operation {
            match action {
                ErrorAction::SignIn => on_login.run(()),
                ErrorAction::Retry => match operation {
                    ErrorOperation::LoadCatalog => on_discovery_retry.run(()),
                    ErrorOperation::LoadWork | ErrorOperation::LoadUnits => on_work_retry.run(()),
                    // Mutations and account cleanup keep their receipt/session owners.
                    _ => (),
                },
                ErrorAction::Close => (),
            }
        }
    });
    let on_clear_unsafe_link = Callback::new(move |()| {
        // Reload only after successful scrub. A fresh mount restores every normal lifecycle
        // listener once; failed scrub/reload keeps this owner blocked and exposes no signup IO.
        let _ = signup_browser::discard_link_and_reload();
    });
    let drawer = NodeRef::<leptos::html::Dialog>::new();
    let menu_button = NodeRef::<leptos::html::Button>::new();
    let drawer_close = NodeRef::<leptos::html::Button>::new();
    let shell = NodeRef::<leptos::html::Div>::new();
    let rail_toggle = NodeRef::<leptos::html::Button>::new();
    platform::navigation_drawer::bind(
        drawer,
        menu_button,
        rail_toggle,
        drawer_close,
        shell,
        nav.route.read_only(),
    );
    view! {
        <super::LiveFrame theme>
            <crate::features::signup::SignupLinkSafety blocked=history_blocked locale on_retry=on_clear_unsafe_link>
            <PreviewControls preview state signup locale on_cancel=Callback::new(move |()| { crate::live::signup::cancel(signup, preview); nav.cancel(); })/>
            <div class="theatre-app theatre-live-app" node_ref=shell data-navigation="drawer" data-auth=move || matches!(nav.route.get(), Route::Auth(_)).to_string() lang=move || locale.get().lang() dir=move || locale.get().dir()>
                <a class="theatre-skip" href="#theatre-main" on:click=move |e| { e.prevent_default(); platform::focus_main(main); }>{move || messages::text(Key::SkipContent, locale.get())}</a>
                <div class="theatre-header">
                    <super::LiveContainer>
                    <header class="theatre-topbar">
                        <button id="theatre-navigation-menu" node_ref=menu_button type="button" class="theatre-action theatre-menu" aria-controls="theatre-navigation-drawer" aria-expanded="false" aria-haspopup="dialog">
                            <span class="theatre-menu-glyph" aria-hidden="true"></span>
                            <span>{move || messages::text(Key::Menu, locale.get())}</span>
                        </button>
                        <div class="theatre-mobile-brand"><LiveBrand/></div>
                        <p class="theatre-topbar-context">{move || messages::text(messages::route_key(nav.route.get()), locale.get())}</p>
                    </header>
                    </super::LiveContainer>
                </div>
                <aside id="theatre-navigation-rail" class="theatre-rail">
                    <div class="theatre-rail-header">
                        <LiveBrand/>
                        <button id="theatre-rail-toggle" node_ref=rail_toggle type="button" class="theatre-action theatre-action-tonal" aria-controls="theatre-navigation-rail" aria-expanded="true" data-action="collapse">
                            <span class="theatre-menu-glyph" aria-hidden="true"></span>
                            <span class="theatre-toggle-arrow" aria-hidden="true"><Icon name=IconName::ArrowBack/></span>
                            <span data-toggle-label="menu" hidden=true>{move || messages::text(Key::Menu, locale.get())}</span>
                            <span data-toggle-label="collapse">{move || messages::text(Key::CollapseNavigation, locale.get())}</span>
                            <span data-toggle-label="expand" hidden=true>{move || messages::text(Key::ExpandNavigation, locale.get())}</span>
                        </button>
                    </div>
                    <PrimaryNavigation route=nav.route locale state=state.read_only()/>
                </aside>
                <dialog id="theatre-navigation-drawer" class="theatre-navigation-drawer" node_ref=drawer aria-labelledby="theatre-drawer-heading">
                    <header class="theatre-drawer-header">
                        <div id="theatre-drawer-heading"><LiveBrand/></div>
                        <button node_ref=drawer_close type="button" class="theatre-action theatre-action-text">{move || messages::text(Key::CloseNavigation, locale.get())}</button>
                    </header>
                    <PrimaryNavigation route=nav.route locale state=state.read_only()/>
                </dialog>
                <div class="theatre-navigation-fit" aria-hidden="true">
                    {[Key::Discover, Key::Library, Key::Me].map(|key| view! {
                        <span data-navigation-fit="compact">{move || messages::text(key, locale.get())}</span>
                        <span data-navigation-fit="expanded">{move || messages::text(key, locale.get())}</span>
                    }).collect_view()}
                </div>
                <div class="theatre-content">
                    <super::LiveContainer>

                        <main id="theatre-main" node_ref=main tabindex="-1" class="theatre-main">
                            <p role="status" hidden=move ||locale_warning.get().is_none()>{move ||crate::localization::preference_warning(locale_warning.get(),locale.get())}</p>
                            {move || if !configuration_valid {
                                view! { <LiveRecovery locale heading=Key::ErrorDialogTitle help=Key::ErrorConfiguration/> }.into_any()
                            } else { match nav.route.get() {
                                Route::Auth(auth) => view! { <AuthPage state=state.read_only() signup locale route=auth account_created
                                    on_back=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.back(); })
                                    on_register on_login=on_login_submit
                                    on_home=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.cancel(); auth_browser::go(nav.route, Route::Discover, false); })
                                    on_refresh on_logout
                                    on_signup_submit on_signup_confirm on_signup_retry on_signup_new on_signup_login/> }.into_any(),
                                Route::Account | Route::Settings => view! { <LiveAccount state=state.read_only() locale theme on_locale on_login on_register on_logout on_refresh on_membership/> }.into_any(),
                                Route::Library | Route::History => view! {
                                    <LiveLibrary state=state.read_only() locale actions=library_actions shared_section=ViewFn::from(move ||view!{<crate::features::shared_access::SharedLibrary account=shared_account state=shared locale
                                        on_reload=Callback::new(move |_| { if let Some(fence)=state.with_untracked(|a|a.fence()) { if let Some(ticket)=shared.try_update(|s|s.catalog.begin(DiscoveryQuery::default())).flatten() {super::shared_runtime::run_catalog(shared,state,ticket,fence,shared_requests);} } })
                                        on_more=Callback::new(move |_| { if let Some(fence)=state.with_untracked(|a|a.fence()) { if let Some(ticket)=shared.try_update(|s|s.catalog.load_more()).flatten() {super::shared_runtime::run_catalog(shared,state,ticket,fence,shared_requests);} } })
                                        on_claim=Callback::new(move |_|super::shared_runtime::claim(shared,state,shared_requests))/>}) on_page=Callback::new(move |(section, restart, focus)| {
                                        if preview || !network_allowed.get_untracked() { return; }
                                        #[cfg(target_arch = "wasm32")]
                                        crate::live::runtime::load_library(state, section, restart, 1, focus);
                                        #[cfg(not(target_arch = "wasm32"))]
                                        let _ = (section, restart, focus);
                                    })/>
                                    <LibraryAccess state=state.read_only() locale on_login on_register on_refresh on_logout/>
                                }.into_any(),
                                Route::Membership(member) => view! {
                                    <LiveMembership state=state.read_only() locale member
                                        on_back=Callback::new(move |_| nav.back())
                                        on_login on_account=Callback::new(move |_| nav.open_account())/>
                                }.into_any(),
                                Route::SharedWork(id) => view! { <crate::features::shared_access::SharedDetail work=id account=shared_account state=work_detail scope=work_scope locale listen=listen_controls on_login on_back=on_work_back on_retry=on_work_retry on_more=on_work_more/> }.into_any(),
                                Route::PublicWork(id) => view! { <Show when=move ||work_scope.with(Option::is_none) && work_detail.with(|s|s.work_id()==Some(id)) fallback=move ||view!{<p role="status">{move ||messages::text(Key::PublicWorkLoading,locale.get())}</p>}><PublicWorkDetail state=work_detail locale listen=listen_controls sync_controls=work_sync_controls on_back=on_work_back on_retry=on_work_retry on_load_more=on_work_more/></Show> }.into_any(),
                                Route::Player => view! { <LivePlayerScreen live sync_controls=player_sync_controls/> }.into_any(),
                                Route::Discover | Route::Explore => view! {
                                    <PublicHome state=discovery account=state.read_only() locale applied=nav.query on_search on_login on_register on_refresh on_logout
                                        on_retry=on_discovery_retry on_load_more=on_discovery_more on_restart=on_discovery_restart on_open_work/>
                                }.into_any(),
                                // A route that parses but has no live capability says so; it never
                                // borrows another screen's body.
                                Route::Queue => view! { <LiveRecovery locale heading=Key::LiveRouteUnavailable help=Key::LiveRouteUnavailableHelp/> }.into_any(),
                                // Mock fixture slugs are mapped to `NotFound` before they get here.
                                Route::NotFound | Route::Work(_) => view! { <LiveRecovery locale heading=Key::LiveNotFound help=Key::LiveNotFoundHelp/> }.into_any(),
                            } }}
                        </main>
                    </super::LiveContainer>
                </div>
                <Show when=move || state.with(|s| s.storage_fault().is_some())>
                    <div class="theatre-public-state" role="alert">
                        <p>{move || messages::text(Key::ErrorStorage, locale.get())}</p>
                        <Button id="theatre-storage-retry" variant=ButtonVariant::Tonal focus_on_press=true on_press=Callback::new(move |_| {
                            #[cfg(target_arch = "wasm32")]
                            crate::live::runtime::retry_journal_storage(state);
                        })>{move || messages::text(Key::ErrorStorageRecovery, locale.get())}</Button>
                    </div>
                </Show>
                <crate::features::account_menu::LogoutConfirmation state=state.read_only() locale open=logout_open on_logout=confirm_logout/>
                <LiveMiniPlayer live route=nav.route/>
                <crate::features::error_dialog::ErrorWindow center=errors locale on_action=on_error_action/>
            </div>
            </crate::features::signup::SignupLinkSafety>
        </super::LiveFrame>
    }
}

/// A route with nothing to show: a visible reason and one way back, with no sample content.
#[component]
pub(crate) fn LiveRecovery(locale: RwSignal<Locale>, heading: Key, help: Key) -> impl IntoView {
    view! {
        <section class="theatre-public-state theatre-panel theatre-live-recovery" aria-labelledby="theatre-live-recovery-heading">
            <h1 id="theatre-live-recovery-heading" tabindex="-1">{move || messages::text(heading, locale.get())}</h1>
            <p>{move || messages::text(help, locale.get())}</p>
            <a id="theatre-live-recovery-home" class="theatre-action theatre-action-tonal" href="#discover">
                {move || messages::text(Key::LiveBackToDiscover, locale.get())}
            </a>
        </section>
    }
}

/// Library retains its own purpose and auth origin; pending/failure cleanup never becomes Guest.
#[component]
fn LibraryAccess(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    on_login: Callback<()>,
    on_register: Callback<()>,
    on_refresh: Callback<()>,
    on_logout: Callback<()>,
) -> impl IntoView {
    view! {
        <div class="theatre-library-access">
            <Show when=move || state.with(AccountViewState::can_authenticate)>
                <crate::features::auth::AuthActions state locale scope="theatre-library" on_login on_register/>
            </Show>
            <crate::features::auth::SessionRecovery state locale scope="theatre-library" on_refresh on_logout/>
        </div>
    }
}

#[cfg(feature = "preview")]
#[component]
pub(crate) fn LivePreferences(locale: RwSignal<Locale>, theme: RwSignal<Theme>) -> impl IntoView {
    view! {
        <section class="theatre-account-preferences" aria-labelledby="theatre-preferences-heading">
            <h1 id="theatre-preferences-heading">{move || messages::text(Key::Settings, locale.get())}</h1>
            <div class="theatre-account-theme">
                <label for="theatre-settings-theme">{move || messages::text(Key::Theme, locale.get())}</label>
                <cantos_ui::ThemeCycleButton
                    id="theatre-settings-theme"
                    theme=theme
                    labels=Signal::derive(move ||crate::localization::theme_labels(locale.get()))
                />
            </div>
            <a class="theatre-text-link" href="#account">{move || messages::text(Key::Account,locale.get())}</a>
        </section>
    }
}

#[cfg(feature = "preview")]
#[component]
pub fn AuthPreview() -> impl IntoView {
    view! { <LiveBoundary preview=true/> }
}

fn membership_back_key(member: MembershipReturn) -> crate::features::auth_copy::CopyKey {
    use crate::features::auth_copy::CopyKey;
    match member {
        MembershipReturn::Home => CopyKey::ReturnHome,
        MembershipReturn::Account => CopyKey::ReturnAccount,
    }
}

/// Informational live destination. Membership intake has no accepted capability: this
/// composition owns no form, request, private contact state or success transition.
#[component]
fn LiveMembership(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    member: MembershipReturn,
    on_back: Callback<()>,
    on_login: Callback<()>,
    on_account: Callback<()>,
) -> impl IntoView {
    let can_login = Memo::new(move |_| state.with(AccountViewState::can_authenticate));
    view! {
        <section class="theatre-membership-information" aria-labelledby="theatre-membership-heading">
            {move || AccessibleName::new(crate::features::auth_copy::text(membership_back_key(member), locale.get())).ok().map(|name| view! {
                <BackButton id="theatre-membership-back" name on_back=Callback::new(move |_| on_back.run(()))/>
            })}
            <div class="theatre-membership-layout">
                <header class="theatre-membership-intro">
                    <div class="theatre-membership-symbol" aria-hidden="true">
                        <Logo variant=LogoVariant::Symbol tone=LogoTone::Light/>
                    </div>
                    <p class="theatre-eyebrow">{move || messages::text(Key::MembershipHeading, locale.get())}</p>
                    <h1 id="theatre-membership-heading">{move || messages::text(Key::MembershipIntro, locale.get())}</h1>
                    <p>{move || messages::text(Key::MembershipSummary, locale.get())}</p>
                </header>
                <section class="theatre-membership-availability" aria-labelledby="theatre-membership-status-heading">
                    <p class="theatre-membership-badge">{move || messages::text(Key::MembershipUnavailableBadge, locale.get())}</p>
                    <h2 id="theatre-membership-status-heading">{move || messages::text(Key::MembershipUnavailableTitle, locale.get())}</h2>
                    <p>{move || messages::text(Key::MembershipUnavailable, locale.get())}</p>
                    <div class="theatre-membership-actions">
                        <Show when=move || can_login.get() fallback=move || view! {
                            <Button id="theatre-membership-account" variant=ButtonVariant::Tonal icon=IconName::Person focus_on_press=true
                                on_press=Callback::new(move |_| on_account.run(()))>
                                {move || messages::text(Key::Account, locale.get())}
                            </Button>
                        }>
                            <Button id="theatre-membership-login" variant=ButtonVariant::Tonal icon=IconName::Login focus_on_press=true
                                on_press=Callback::new(move |_| {
                                    // A phase change between render and activation cannot reopen auth.
                                    if state.with_untracked(AccountViewState::can_authenticate) {
                                        on_login.run(());
                                    }
                                })>
                                {move || messages::text(Key::SignIn, locale.get())}
                            </Button>
                        </Show>
                        <a class="theatre-membership-explore" href="#discover">{move || messages::text(Key::Discover, locale.get())}</a>
                    </div>
                </section>
            </div>
        </section>
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    fn membership_html(
        state: ReadSignal<AccountViewState>,
        language: Locale,
        member: MembershipReturn,
    ) -> String {
        view! {
            <LiveMembership state locale=RwSignal::new(language) member
                on_back=Callback::new(|_| ()) on_login=Callback::new(|_| ())
                on_account=Callback::new(|_| ())/>
        }
        .to_html()
    }

    #[test]
    fn live_membership_only_offers_sign_in_to_an_admitted_guest() {
        use crate::features::auth_preview::{guest, logout_pending, signed_in, PreviewActor};
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let assert_action = |login: bool| {
                for language in [Locale::Vietnamese, Locale::English] {
                    for member in [MembershipReturn::Home, MembershipReturn::Account] {
                        let html = membership_html(state.read_only(), language, member);
                        assert_eq!(html.contains("id=\"theatre-membership-login\""), login);
                        assert_eq!(html.contains("id=\"theatre-membership-account\""), !login);
                        assert!(html.contains(messages::text(Key::MembershipUnavailable, language)));
                        assert!(html.contains(crate::features::auth_copy::text(membership_back_key(member), language)));
                        assert!(html.contains("href=\"#discover\""));
                        assert!(!html.contains("<form") && !html.contains("<input"));
                        assert_eq!(messages::route_key(Route::Membership(member)), Key::MembershipHeading);
                    }
                }
            };
            assert_action(false); // Initial session is unknown.
            state.update(|s| { s.begin_refresh().unwrap(); });
            assert_action(false); // Checking is not a guest fact.
            state.update(|s| {
                let generation = s.generation();
                s.finish_session(generation, Err(crate::application::account_view::AccountFailure::Unavailable), || uuid::Uuid::from_u128(1).to_string()).unwrap();
            });
            assert_action(false);
            guest(state).unwrap();
            assert_action(true);
            signed_in(state, PreviewActor::A).unwrap();
            assert_action(false);
            logout_pending(state).unwrap();
            assert_action(false); // Unresolved cleanup may never offer auth.
        });
    }

    #[test]
    fn live_brand_has_one_named_link_with_original_decorative_symbols_and_wordmark() {
        Owner::new().with(|| {
            let html = view! { <LiveBrand/> }.to_html();
            assert_eq!(html.matches("<a ").count(), 1);
            assert!(html.contains("href=\"#discover\""));
            assert!(html.contains("aria-label=\"Cantos\""));
            assert_eq!(html.matches("data-logo-variant=\"symbol\"").count(), 2);
            assert!(html.contains("data-logo-tone=\"light\""));
            assert!(html.contains("data-logo-tone=\"dark\""));
            assert_eq!(html.matches("aria-hidden=\"true\"").count(), 2);
            assert_eq!(html.matches("alt=\"\"").count(), 2);
            assert!(html.contains("class=\"theatre-live-brand-name\">Cantos</strong>"));
            assert!(!html.contains("<button"));
        });
    }

    #[test]
    fn topbar_locale_uses_the_shared_pill_recipe_without_an_app_level_control_override() {
        let css = include_str!("../styles/theatre.css");
        let shared = include_str!("../../../../../packages/ui-system/leptos/foundation.css.in");
        // The pill, 48-unit height and tonal fill are one shared recipe, scoped to the topbar row.
        let recipe = shared
            .split(
                ":where(.cantos-ui) .cui-topbar-actions .cui-select-field[data-select-density=\"compact\"] .cui-field-control {",
            )
            .nth(1)
            .and_then(|rest| rest.split('}').next())
            .expect("shared topbar locale recipe");
        assert!(recipe.contains("min-block-size: var(--cantos-size-control)"));
        assert!(recipe.contains("border-radius: var(--cantos-shape-pill)"));
        assert!(recipe.contains("background-color: var(--cantos-color-surface-container-high)"));
        assert!(!recipe.contains('#') && !recipe.contains("rgb"));
        // The app no longer restyles the select control, size, radius or fill.
        assert!(!css.contains(".theatre-locale-field.cui-select-field .cui-field-control"));
        assert!(!css.contains(".theatre-locale-field.cui-select-field .cui-select-trigger"));
        assert!(include_str!("live_shell.rs").contains("theatre-topbar-actions cui-topbar-actions"));
    }

    #[test]
    fn topbar_actions_wrap_instead_of_shrinking_the_theme_and_avatar_targets() {
        let css = include_str!("../styles/theatre.css");
        let rule = |selector: &str| {
            css.split(&format!("{selector} {{"))
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing rule {selector}"))
        };
        let actions = rule(".theatre-topbar-actions");
        assert!(
            actions.contains("flex-wrap: wrap") && actions.contains("justify-content: flex-end")
        );
        // No rule re-sizes the shared 44×44 theme target or the avatar from the app.
        assert!(!css.contains(".cui-theme-cycle-button"));
    }

    #[test]
    fn sync_admission_does_not_retrigger_reads_for_projection_updates() {
        use crate::features::auth_preview::{signed_in, PreviewActor};
        use std::sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        };

        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            signed_in(state, PreviewActor::A).unwrap();
            let admission = sync_admission(state.read_only());
            let runs = Arc::new(AtomicUsize::new(0));
            let observed_runs = runs.clone();
            let read_effect = Memo::new(move |_| {
                let admitted = admission.get().is_some();
                observed_runs.fetch_add(1, Ordering::SeqCst);
                admitted
            });
            assert!(read_effect.get());
            let work = uuid::Uuid::from_u128(99).to_string();
            // Both a real projection mutation and the already-in-flight no-op notify state.
            for _ in 0..3 {
                state.update(|s| {
                    s.seed_sync(|sync, _| sync.begin_saved_read(&work));
                });
                assert!(read_effect.get());
            }
            assert_eq!(runs.load(Ordering::SeqCst), 1);
            state.update(|s| {
                s.begin_logout().unwrap();
            });
            assert!(!read_effect.get());
            assert_eq!(runs.load(Ordering::SeqCst), 2);
        });
    }

    #[test]
    fn unknown_and_unwired_routes_say_so_and_offer_the_way_back_without_sample_content() {
        Owner::new().with(|| {
            for (locale, not_found, unavailable, back) in [
                (
                    Locale::Vietnamese,
                    "Không tìm thấy trang này",
                    "Tính năng này chưa mở",
                    "Về Khám phá",
                ),
                (
                    Locale::English,
                    "This page could not be found",
                    "This feature is not available yet",
                    "Back to Explore",
                ),
            ] {
                let locale = RwSignal::new(locale);
                for (heading, help, expected) in [
                    (Key::LiveNotFound, Key::LiveNotFoundHelp, not_found),
                    (
                        Key::LiveRouteUnavailable,
                        Key::LiveRouteUnavailableHelp,
                        unavailable,
                    ),
                ] {
                    let html = view! { <LiveRecovery locale heading help/> }.to_html();
                    assert!(html.contains(expected) && html.contains(back), "{html}");
                    assert!(html.contains("href=\"#discover\""));
                    // No Home body, mock title or sample catalog hides behind the heading.
                    for absent in [
                        "theatre-public-home",
                        "Sơn Hà Ký",
                        "demo",
                        "theatre-public-work",
                    ] {
                        assert!(!html.contains(absent), "{absent}");
                    }
                }
            }
        });
    }

    #[test]
    fn navigation_exposes_exactly_three_localized_native_links_and_one_current_family() {
        Owner::new().with(|| {
            for language in Locale::supported() {
                for (route, selected) in [(Route::Explore, "#discover"), (Route::History, "#library"), (Route::Settings, "#account"), (Route::Membership(MembershipReturn::Account), "#account")] {
                    let html = view! { <PrimaryNavigation route=RwSignal::new(route) locale=RwSignal::new(language) state=RwSignal::new(AccountViewState::default()).read_only()/> }.to_html();
                    assert_eq!(html.matches("<a ").count(), 3);
                    assert_eq!(html.matches("aria-current=\"page\"").count(), 1);
                    assert!(html.contains(&format!("href=\"{selected}\" aria-current=\"page\"")), "{html}");
                    assert!(html.contains(messages::text(Key::Library, language)) && html.contains(">Me</span>"));
                }
            }
        });
    }

    #[test]
    fn library_auth_is_direct_and_only_the_admitted_guest_can_start_it() {
        use crate::features::auth_preview::{guest, logout_pending, signed_in, PreviewActor};
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            let render = || view! { <LibraryAccess state=state.read_only() locale=RwSignal::new(Locale::English) on_login=Callback::new(|_|()) on_register=Callback::new(|_|()) on_refresh=Callback::new(|_|()) on_logout=Callback::new(|_|())/> }.to_html();
            assert!(!render().contains("theatre-library-login"));
            guest(state).unwrap();
            let guest_html = render();
            assert!(guest_html.contains("theatre-library-login") && guest_html.contains("theatre-library-register"));
            assert!(!guest_html.contains("href=\"#account\"") && !guest_html.contains("theatre-account-preferences"));
            signed_in(state, PreviewActor::A).unwrap();
            assert!(!render().contains("theatre-library-login"));
            logout_pending(state).unwrap();
            assert!(!render().contains("theatre-library-login"));
        });
    }

    #[test]
    fn live_preferences_renders_theme_cycle_button_with_accessible_controls() {
        Owner::new().with(|| {
            let locale = RwSignal::new(Locale::Vietnamese);
            let theme = RwSignal::new(Theme::Dark);
            let html = view! { <LivePreferences locale theme/> }.to_html();
            assert!(html.contains("id=\"theatre-settings-theme\""));
            assert!(html.contains("cui-theme-cycle-button"));
            assert!(!html.contains("<select"));
        });
    }
}
