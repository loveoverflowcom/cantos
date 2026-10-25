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
        common::{icon, PersonGlyph},
        library::{LibraryActions, LiveLibrary},
        live_player::{LiveMiniPlayer, LivePlayer, LivePlayerScreen, PlayerSyncControls},
        public_home::{self, PublicHome},
        public_work::{ListenControls, ListenTarget, PublicWorkDetail, WorkSyncControls},
    },
    messages::{self, Key, Locale},
    platform::{self, auth_browser, signup_browser},
};
use cantos_ui::{
    AccessibleName, BackButton, Button, ButtonVariant, Logo, LogoTone, LogoVariant, Select,
    SelectDensity, SelectLabelPlacement, SelectOption, Theme,
};
use leptos::prelude::*;

// Registered outline icons (assets/icons) of the two primary destinations.
const NAV_ICON_DISCOVER: &str = "home";
const NAV_ICON_LIBRARY: &str = "library_books";

const CANTOS_BRAND: &str = "Cantos";

/// One app-owned brand link; the admitted shared Logo supplies the original decorative symbol.
#[component]
fn LiveBrand() -> impl IntoView {
    view! {
        <a class="theatre-brand theatre-live-brand" href="#discover" aria-label=CANTOS_BRAND>
            <Logo variant=LogoVariant::Symbol tone=LogoTone::Light class="theatre-logo-light"/>
            <Logo variant=LogoVariant::Symbol tone=LogoTone::Dark class="theatre-logo-dark"/>
            <strong class="theatre-live-brand-name">{CANTOS_BRAND}</strong>
        </a>
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
                        platform::focus_after_unit_page(target);
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

/// The diagnostic guest and an empty catalog of the explicit auth preview. It reads no network
/// and exists only in a build with the `preview` feature.
#[cfg(feature = "preview")]
fn seed_preview(state: RwSignal<AccountViewState>, discovery: RwSignal<PublicDiscoveryState>) {
    let _ = crate::features::auth_preview::guest(state);
    discovery.update(|s| {
        if let Some(issued) = s.begin(DiscoveryQuery::default()) {
            s.finish(
                &issued,
                Ok(crate::contract::discovery::PresentedWorkPage {
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
    let discovery = RwSignal::new(PublicDiscoveryState::default());
    // RAM-only notice for the sign-in page after a finished signup. It is never a session.
    let account_created = RwSignal::new(false);
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
    // When signed in on a public Work or with an active player source, load point Saved/Progress
    // registers that have not yet been observed in the current session.
    let admitted_sync = sync_admission(state.read_only());
    Effect::new(move |_| {
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
                        .first()
                        .map(|u| u.publication_id.clone())
                        .unwrap_or_default();
                    let work_pres = WorkPresentation {
                        work_id: id.to_string(),
                        publication_id: first_pub,
                        title: work.title.clone(),
                        kind: work.kind,
                        language: work.language.clone(),
                    };
                    let pubs = units
                        .rows()
                        .iter()
                        .map(|u| {
                            (
                                u.publication_id.clone(),
                                AudioPresentation {
                                    work: WorkPresentation {
                                        work_id: id.to_string(),
                                        publication_id: u.publication_id.clone(),
                                        title: work.title.clone(),
                                        kind: work.kind,
                                        language: work.language.clone(),
                                    },
                                    unit_id: u.unit_id.clone(),
                                    unit_title: u.title.clone(),
                                },
                            )
                        })
                        .collect::<Vec<_>>();
                    (Some((id.to_string(), Some(work_pres))), pubs)
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
        let _ = crate::application::account_writer::AccountWriter::new(state)
            .reset_resume()
            .into_value();
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
            Route::NotFound | Route::Work(_) => Some(messages::format(
                Key::PageTitle,
                locale.get(),
                &[(
                    "page",
                    messages::text(Key::LiveNotFound, locale.get()).to_owned(),
                )],
            )),
            Route::Queue => Some(messages::format(
                Key::PageTitle,
                locale.get(),
                &[(
                    "page",
                    messages::text(Key::LiveRouteUnavailable, locale.get()).to_owned(),
                )],
            )),
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
    let on_logout = Callback::new(move |()| {
        crate::live::signup::cancel(signup, preview);
        nav.logout(state);
    });
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
    view! {
        <super::LiveFrame theme>
            <crate::features::signup::SignupLinkSafety blocked=history_blocked locale on_retry=on_clear_unsafe_link>
            <PreviewControls preview state signup locale on_cancel=Callback::new(move |()| { crate::live::signup::cancel(signup, preview); nav.cancel(); })/>
            <div class="theatre-app theatre-live-app" data-auth=move || matches!(nav.route.get(), Route::Auth(_)).to_string() lang=move || locale.get().lang()>
                <a class="theatre-skip" href="#theatre-main" on:click=move |e| { e.prevent_default(); platform::focus_main(main); }>{move || messages::text(Key::SkipContent, locale.get())}</a>
                <aside class="theatre-rail">
                    <LiveBrand/>
                    <nav class="theatre-nav" aria-label=move || messages::text(Key::MainNavigation, locale.get())>
                        <a class="theatre-nav-link" href="#discover" aria-current=move || (nav.route.get().primary_navigation() == Some(Route::Discover)).then_some("page")>{icon(NAV_ICON_DISCOVER)}<span>{move || messages::text(Key::Discover, locale.get())}</span></a>
                        <a class="theatre-nav-link" href="#library" aria-current=move || (nav.route.get().primary_navigation() == Some(Route::Library)).then_some("page")>{icon(NAV_ICON_LIBRARY)}<span>{move || messages::text(Key::Library, locale.get())}</span></a>
                    </nav>
                </aside>
                <div class="theatre-content">
                    <super::LiveContainer>
                    <header class="theatre-topbar">
                        // The rail owns the wordmark; the topbar says where the listener is.
                        <p class="theatre-topbar-context">{move || messages::text(messages::route_key(nav.route.get()), locale.get())}</p>
                        <div class="theatre-topbar-actions">
                            <Select id="theatre-live-locale" class="theatre-locale-field"
                                density=SelectDensity::Compact label_placement=SelectLabelPlacement::Inline
                                label=Signal::derive(move || messages::text(Key::Locale, locale.get()).to_owned())
                                value=Signal::derive(move || locale.get().lang().to_owned())
                                options={Locale::ALL.into_iter().map(|option| SelectOption {
                                    lang: Some(option.lang().to_owned()),
                                    ..SelectOption::new(option.lang(), option.autonym())
                                }).collect::<Vec<_>>()}
                                on_input=Callback::new(move |value: String| {
                                    if let Some(selected) = Locale::from_lang(&value) { locale.set(selected); }
                                })/>
                            <a id="theatre-account-entry" class="theatre-avatar" href="#account" aria-label=move || messages::text(Key::Account, locale.get()) aria-current=move || (nav.route.get() == Route::Account).then_some("page")><PersonGlyph/></a>
                        </div>
                    </header>
                        <main id="theatre-main" node_ref=main tabindex="-1" class="theatre-main">
                            {move || if !configuration_valid {
                                view! { <LiveRecovery locale heading=Key::ErrorDialogTitle help=Key::ErrorConfiguration/> }.into_any()
                            } else { match nav.route.get() {
                                Route::Auth(auth) => view! { <AuthPage state=state.read_only() signup locale route=auth account_created
                                    on_back=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.back(); })
                                    on_register on_login=on_login_submit
                                    on_home=Callback::new(move |()| { crate::live::signup::leave(signup, preview); nav.cancel(); auth_browser::go(nav.route, Route::Discover, false); })
                                    on_refresh on_logout
                                    on_signup_submit on_signup_confirm on_signup_retry on_signup_new on_signup_login/> }.into_any(),
                                Route::Account => view! { <LiveAccount state=state.read_only() locale theme on_login on_register on_logout on_refresh on_membership/> }.into_any(),
                                Route::Settings => view! { <LivePreferences locale theme/> }.into_any(),
                                Route::Library | Route::History => view! {
                                    <LiveLibrary state=state.read_only() locale actions=library_actions on_page=Callback::new(move |(section, restart, focus)| {
                                        if preview || !network_allowed.get_untracked() { return; }
                                        #[cfg(target_arch = "wasm32")]
                                        crate::live::runtime::load_library(state, section, restart, 1, focus);
                                        #[cfg(not(target_arch = "wasm32"))]
                                        let _ = (section, restart, focus);
                                    })/>
                                    <LibraryAccountEntry state=state.read_only() locale on_account=Callback::new(move |_|nav.open_account())/>
                                }.into_any(),
                                Route::Membership(member) => view! {
                                    <section class="theatre-membership-information" aria-labelledby="theatre-membership-heading">
                                        {move || AccessibleName::new(crate::features::auth_copy::text(membership_back_key(member),locale.get())).ok().map(|name| view! { <BackButton id="theatre-membership-back" name on_back=Callback::new(move |_| nav.back())/> })}
                                        <h1 id="theatre-membership-heading">{move || messages::text(Key::MembershipHeading,locale.get())}</h1>
                                        <p>{move || messages::text(Key::MembershipUnavailable,locale.get())}</p>
                                        <Button id="theatre-membership-login" variant=ButtonVariant::Tonal focus_on_press=true on_press=Callback::new(move |_| nav.open_auth(AuthRoute::login(nav.origin())))>{move || messages::text(Key::SignIn,locale.get())}</Button>
                                    </section>
                                }.into_any(),
                                Route::PublicWork(_) => view! { <PublicWorkDetail state=work_detail locale listen=listen_controls sync_controls=work_sync_controls on_back=on_work_back on_retry=on_work_retry on_load_more=on_work_more/> }.into_any(),
                                Route::Player => view! { <LivePlayerScreen live sync_controls=player_sync_controls/> }.into_any(),
                                Route::Discover | Route::Explore => view! {
                                    <PublicHome state=discovery locale applied=nav.query on_search on_login on_register
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
                <LiveMiniPlayer live route=nav.route/>
                <crate::features::error_dialog::ErrorWindow center=errors locale on_action=on_error_action/>
            </div>
            </crate::features::signup::SignupLinkSafety>
        </super::LiveFrame>
    }
}

/// A route with nothing to show: a visible reason and one way back, with no sample content.
#[component]
fn LiveRecovery(locale: RwSignal<Locale>, heading: Key, help: Key) -> impl IntoView {
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

#[component]
fn LibraryAccountEntry(
    #[prop(optional)] on_account: Option<Callback<()>>,
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
) -> impl IntoView {
    let phase = Memo::new(move |_| state.with(AccountViewState::phase));
    view! {
        <Show when=move || phase.get() != SessionPhase::SignedIn>
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
#[cfg(test)]
mod tests {
    use super::*;

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
                let html =
                    view! { <LibraryAccountEntry state=state.read_only() locale/> }.to_html();
                assert!(html.contains("href=\"#account\""));
                assert!(html.contains(copy));
                for absent in ["<button", "#login", "Đăng nhập", "Sign in"] {
                    assert!(!html.contains(absent), "{absent}");
                }
            }
            signed_in(state, PreviewActor::A).unwrap();
            let locale = RwSignal::new(Locale::English);
            let html = view! { <LibraryAccountEntry state=state.read_only() locale/> }.to_html();
            assert!(!html.contains("theatre-library-account"));
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
    fn membership_back_names_the_actual_internal_context() {
        use crate::application::auth_navigation::NavigationTarget;
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
        ] {
            assert_eq!(member.back(), target);
            assert_eq!(text(membership_back_key(member), Locale::Vietnamese), vi);
            assert_eq!(text(membership_back_key(member), Locale::English), en);
        }
    }
}
