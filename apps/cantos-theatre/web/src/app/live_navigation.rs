//! Auth presentation navigation owns no credentials and never decides session validity.
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthIntentState, AuthRequestFence, AuthRoute, ReturnDestination},
        public_discovery::DiscoveryQuery,
    },
    core::public_ids::PublicWorkId,
    platform::{self, auth_browser as browser},
};
use leptos::prelude::*;

fn observed_session_phase(state: ReadSignal<AccountViewState>) -> Memo<SessionPhase> {
    Memo::new(move |_| state.with(AccountViewState::phase))
}

/// An older discovery link may still carry the content-language filter that was retired (#424).
/// The page drops it from the query; this makes the address say so too, replacing the entry in
/// place so Back does not revisit the stale link.
fn reconcile_retired_language(route: Route, hash: &str) {
    if matches!(route, Route::Discover | Route::Explore)
        && super::routes::carries_retired_language(hash)
    {
        browser::replace_discovery(route, &super::routes::discovery_query(hash));
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LiveNavigation {
    pub route: RwSignal<Route>,
    /// The search the current discovery link carries. History owns it: Back/Forward restore it,
    /// and the discovery walk follows it, so a cursor is never reused for another search.
    pub query: RwSignal<DiscoveryQuery>,
    preview: bool,
    intents: RwSignal<AuthIntentState>,
    pending: RwSignal<Option<AuthRequestFence>>,
    returned: StoredValue<Option<(Route, browser::ReturnSnapshot)>>,
    /// Home position and card that opened a public Work, restored when that detail returns.
    work_return: StoredValue<Option<(PublicWorkId, browser::ReturnSnapshot)>>,
}

impl LiveNavigation {
    pub fn new(
        state: RwSignal<AccountViewState>,
        main: NodeRef<leptos::html::Main>,
        preview: bool,
    ) -> Self {
        let hash = browser::hash();
        let initial = Route::from_live_hash(&hash);
        reconcile_retired_language(initial, &hash);
        let auth = match initial {
            Route::Auth(auth) => Some(auth),
            _ => None,
        };
        let intents = AuthIntentState::new(auth);
        let navigation = Self {
            route: RwSignal::new(initial),
            query: RwSignal::new(match initial {
                Route::Discover | Route::Explore => super::routes::discovery_query(&hash),
                _ => DiscoveryQuery::default(),
            }),
            preview,
            intents: RwSignal::new(intents),
            pending: RwSignal::new(None),
            returned: StoredValue::new(None),
            work_return: StoredValue::new(None),
        };
        let previous = StoredValue::new(initial);
        Effect::new(move |_| {
            let current = navigation.route.get();
            if previous.get_value() != current {
                let old = previous.get_value();
                previous.set_value(current);
                navigation.intents.update(|s| {
                    let _ = s.observe_route(match current {
                        Route::Auth(a) => Some(a),
                        _ => None,
                    });
                });
                navigation.pending.set(None);
                if matches!(old, Route::Auth(_)) && !matches!(current, Route::Auth(_)) {
                    if let Some((destination, snapshot)) = navigation
                        .returned
                        .get_value()
                        .filter(|(destination, _)| *destination == current)
                    {
                        let _ = destination;
                        browser::restore_return(snapshot);
                    } else {
                        platform::focus_main(main);
                    }
                    navigation.returned.set_value(None);
                } else if let (Route::PublicWork(from), Route::Discover | Route::Explore) =
                    (old, current)
                {
                    match navigation
                        .work_return
                        .get_value()
                        .filter(|(opened, _)| *opened == from)
                    {
                        Some((_, snapshot)) => browser::restore_return(snapshot),
                        // A deep-linked detail has no opening card: start Home at its top.
                        None => {
                            #[cfg(target_arch = "wasm32")]
                            if let Some(window) = web_sys::window() {
                                window.scroll_to_with_x_and_y(0.0, 0.0);
                            }
                            leptos::leptos_dom::helpers::request_animation_frame(move || {
                                platform::focus_main(main)
                            });
                        }
                    }
                    navigation.work_return.set_value(None);
                } else {
                    #[cfg(target_arch = "wasm32")]
                    if let Some(window) = web_sys::window() {
                        window.scroll_to_with_x_and_y(0.0, 0.0);
                    }
                    leptos::leptos_dom::helpers::request_animation_frame(move || {
                        platform::focus_main(main)
                    });
                }
            }
        });
        let phase = observed_session_phase(state.read_only());
        Effect::new(move |_| {
            navigation.observe_session(phase.get());
        });
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::leptos_dom::helpers::window_event_listener_untyped;
            use web_sys::wasm_bindgen::JsCast;
            let read = move || {
                let hash = browser::hash();
                let destination = Route::from_live_hash(&hash);
                reconcile_retired_language(destination, &hash);
                if destination != navigation.route.get_untracked() {
                    navigation.cancel();
                }
                if matches!(destination, Route::Discover | Route::Explore) {
                    navigation.query.set(super::routes::discovery_query(&hash));
                }
                navigation.route.set(destination);
            };
            let hash_listener = window_event_listener_untyped("hashchange", move |_| read());
            let pop_listener = window_event_listener_untyped("popstate", move |_| read());
            let page_listener = window_event_listener_untyped("pageshow", move |event| {
                if event
                    .dyn_ref::<web_sys::PageTransitionEvent>()
                    .is_some_and(|e| e.persisted())
                {
                    navigation.cancel();
                    if !preview {
                        crate::live::runtime::refresh(state);
                    }
                }
            });
            let escape = window_event_listener_untyped("keydown", move |event| {
                if event
                    .dyn_ref::<web_sys::KeyboardEvent>()
                    .is_some_and(|e| e.key() == "Escape")
                    && matches!(navigation.route.get_untracked(), Route::Auth(_))
                {
                    event.prevent_default();
                    navigation.back();
                }
            });
            on_cleanup(move || {
                hash_listener.remove();
                pop_listener.remove();
                page_listener.remove();
                escape.remove();
            });
        }
        navigation
    }

    fn observe_session(self, phase: SessionPhase) {
        if phase == SessionPhase::SignedIn {
            if let Some(fence) = self.pending.get_untracked() {
                let mut accepted = false;
                self.intents.update(|s| {
                    accepted =
                        self.route.get_untracked() == Route::Auth(fence.route()) && s.finish(fence)
                });
                self.pending.set(None);
                if accepted {
                    browser::go(
                        self.route,
                        self.returned
                            .get_value()
                            .map(|(destination, _)| destination)
                            .unwrap_or_else(|| Route::from_target(fence.route().origin.target())),
                        true,
                    );
                }
            }
        } else if matches!(phase, SessionPhase::Failed(_))
            || (phase == SessionPhase::Guest
                && self.pending.get_untracked().is_some()
                && !self.preview)
        {
            // A sign-in attempt enters Checking synchronously. Guest afterwards means the API
            // refused it or the attempt was cleaned up: release the intent so the form can be
            // used again. The isolated preview retains its deliberately staged Outbound/Guest
            // scenario.
            self.release();
        }
    }

    /// The attempt has already ended (refused, failed or cleaned up): free the intent without
    /// signalling the session owner.
    fn release(self) {
        self.intents.update(|s| {
            let _ = s.invalidate();
        });
        self.pending.set(None);
    }

    pub fn open_auth(self, auth: AuthRoute) {
        if !matches!(self.route.get_untracked(), Route::Auth(_))
            && !(self.route.get_untracked() == Route::Account
                && self
                    .returned
                    .get_value()
                    .is_some_and(|(route, _)| matches!(route, Route::Library | Route::History)))
        {
            self.returned.set_value(Some((
                self.route.get_untracked(),
                browser::capture_return(),
            )));
        }
        self.cancel();
        browser::go(self.route, Route::Auth(auth), false);
    }
    /// Apply a search: one history entry per distinct search, so Back restores the previous one.
    /// The shell follows `query` and fences the old walk; this never fetches.
    pub fn search(self, query: DiscoveryQuery) {
        if query == self.query.get_untracked() {
            return;
        }
        let route = match self.route.get_untracked() {
            Route::Explore => Route::Explore,
            _ => Route::Discover,
        };
        browser::push_discovery(self.route, route, &query);
        self.query.set(query);
    }
    pub fn open_account(self) {
        self.returned.set_value(Some((
            self.route.get_untracked(),
            browser::capture_return(),
        )));
        browser::go(self.route, Route::Account, false);
    }
    /// Open a public Work from Home, remembering the activating card for the return trip.
    pub fn open_work(self, work: PublicWorkId, return_focus: String) {
        let mut snapshot = browser::capture_return();
        snapshot.focus_id = return_focus;
        self.work_return.set_value(Some((work, snapshot)));
        self.cancel();
        browser::go(self.route, Route::PublicWork(work), false);
    }
    /// Contextual Back from a public Work: real history when this shell pushed it, else Home.
    pub fn close_work(self) {
        self.cancel();
        browser::back(self.route, Route::Discover);
    }
    pub fn cancel(self) {
        #[cfg(target_arch = "wasm32")]
        if !self.preview && self.pending.get_untracked().is_some() {
            crate::live::runtime::cancel_login();
        }
        self.release();
    }
    pub fn back(self) {
        self.cancel();
        let destination = match self.route.get_untracked() {
            Route::Auth(auth) => self
                .returned
                .get_value()
                .map(|(destination, _)| destination)
                .unwrap_or_else(|| Route::from_target(auth.back())),
            Route::Membership(member) => Route::from_target(member.back()),
            _ => Route::Discover,
        };
        browser::back(self.route, destination);
    }
    /// Start one email/password attempt on the open login route. The credentials are moved to
    /// the account owner's typed intent and are never kept here.
    pub fn login(self, state: RwSignal<AccountViewState>, email: String, password: String) {
        if !state.with_untracked(AccountViewState::can_login) {
            return;
        }
        let Route::Auth(auth) = self.route.get_untracked() else {
            return;
        };
        self.intents.update(|s| {
            let _ = s.observe_route(Some(auth));
        });
        let mut fence = None;
        self.intents.update(|s| fence = s.begin().ok());
        let Some(fence) = fence else {
            return;
        };
        self.pending.set(Some(fence));
        if self.preview {
            return;
        }
        #[cfg(target_arch = "wasm32")]
        crate::live::runtime::login(state, email, password);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = (email, password);
    }
    pub fn logout(self, state: RwSignal<AccountViewState>) {
        self.cancel();
        if self.preview {
            state.update(|s| {
                let _ = s.begin_logout();
            });
            return;
        }
        #[cfg(target_arch = "wasm32")]
        crate::live::runtime::logout(state);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = state;
    }
    pub fn origin(self) -> ReturnDestination {
        match self.route.get_untracked() {
            Route::Account => ReturnDestination::Account,
            Route::Membership(crate::application::auth_navigation::MembershipReturn::Account) => {
                ReturnDestination::MembershipFromAccount
            }
            Route::Membership(_) => ReturnDestination::Membership,
            Route::Auth(a) => a.origin,
            _ => ReturnDestination::Home,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::{
        account::AccountScope,
        account_view::{AccountFailure, LoginRefusal, SessionCompletion},
    };

    #[test]
    fn observed_session_phase_preserves_transition_sequence_without_unrelated_notifications() {
        use crate::features::auth_preview::{guest, signed_in, PreviewActor};
        use std::sync::{Arc, Mutex};
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            guest(state).unwrap();
            let (nav, _) = staged_login();
            let phase = observed_session_phase(state.read_only());
            let seen = Arc::new(Mutex::new(Vec::new()));
            let observations = seen.clone();
            let observe = Memo::new(move |_| {
                let phase = phase.get();
                observations.lock().unwrap().push(phase);
                nav.observe_session(phase);
                phase
            });
            assert_eq!(observe.get(), SessionPhase::Guest);
            let issued = state.write().begin_refresh().unwrap();
            assert_eq!(observe.get(), SessionPhase::Checking);
            state
                .write()
                .finish_session(issued, Err(AccountFailure::Protocol), || {
                    uuid::Uuid::from_u128(100).to_string()
                })
                .unwrap();
            assert_eq!(
                observe.get(),
                SessionPhase::Failed(AccountFailure::Protocol)
            );
            signed_in(state, PreviewActor::A).unwrap();
            assert_eq!(observe.get(), SessionPhase::SignedIn);
            let fence = state.with_untracked(AccountViewState::fence).unwrap();
            for _ in 0..3 {
                state.update(|s| s.record_storage(&fence, Ok(())));
                assert_eq!(observe.get(), SessionPhase::SignedIn);
            }
            let cleanup = state.write().begin_logout().unwrap();
            assert_eq!(observe.get(), SessionPhase::LogoutPending);
            state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable));
            assert_eq!(observe.get(), SessionPhase::LogoutPending);
            let retry = state.write().begin_logout().unwrap();
            state.write().finish_logout(retry, Ok(()));
            assert_eq!(observe.get(), SessionPhase::Guest);
            assert_eq!(
                *seen.lock().unwrap(),
                vec![
                    SessionPhase::Guest,
                    SessionPhase::Checking,
                    SessionPhase::Failed(AccountFailure::Protocol),
                    SessionPhase::SignedIn,
                    SessionPhase::LogoutPending,
                    SessionPhase::Guest
                ]
            );
        });
    }

    const EMAIL: &str = "listener@example.invalid";
    const PASSWORD: &str = "correct horse battery";

    /// A guest on the open login route; the isolated preview never touches the network.
    fn guest_on_login(preview: bool) -> (LiveNavigation, RwSignal<AccountViewState>) {
        let state = RwSignal::new(AccountViewState::default());
        crate::features::auth_preview::guest(state).unwrap();
        let nav = LiveNavigation::new(state, NodeRef::new(), preview);
        nav.open_auth(AuthRoute::login(ReturnDestination::Account));
        (nav, state)
    }
    fn login(nav: LiveNavigation, state: RwSignal<AccountViewState>) {
        nav.login(state, EMAIL.to_owned(), PASSWORD.to_owned());
    }

    #[test]
    fn login_needs_a_guest_on_the_login_route_and_keeps_one_intent_per_attempt() {
        Owner::new().with(|| {
            // Not a guest (nothing observed yet): no intent.
            let idle = RwSignal::new(AccountViewState::default());
            let nav = LiveNavigation::new(idle, NodeRef::new(), true);
            nav.open_auth(AuthRoute::login(ReturnDestination::Home));
            login(nav, idle);
            assert!(nav.pending.get_untracked().is_none());
            // A guest away from the login route: no intent either.
            let state = RwSignal::new(AccountViewState::default());
            crate::features::auth_preview::guest(state).unwrap();
            let home = LiveNavigation::new(state, NodeRef::new(), true);
            login(home, state);
            assert!(home.pending.get_untracked().is_none());
            // A guest on the route reserves exactly one intent; a repeated submit keeps it.
            let (nav, state) = guest_on_login(true);
            login(nav, state);
            let first = nav.pending.get_untracked();
            assert!(first.is_some());
            let before = nav.intents.get_untracked();
            login(nav, state);
            assert_eq!(nav.pending.get_untracked(), first);
            assert_eq!(nav.intents.get_untracked(), before);
            assert_eq!(state.get_untracked().phase(), SessionPhase::Guest);
        });
    }

    fn staged_login() -> (LiveNavigation, AuthRequestFence) {
        let (nav, state) = guest_on_login(true);
        login(nav, state);
        let fence = nav.pending.get_untracked().unwrap();
        (nav, fence)
    }

    fn signed_out_cleanup(
        state: RwSignal<AccountViewState>,
    ) -> crate::application::account::AccountGeneration {
        let mut current = state.write();
        let lookup = current.begin_refresh().unwrap();
        let SessionCompletion::Cleanup(cleanup) = current
            .finish_session(lookup, Err(AccountFailure::SignedOut), || {
                uuid::Uuid::from_u128(100).to_string()
            })
            .unwrap()
        else {
            panic!("a missing callback session must require cookie cleanup")
        };
        cleanup
    }

    #[test]
    fn a_refused_attempt_releases_its_fence_and_allows_a_fresh_one() {
        Owner::new().with(|| {
            let (nav, state) = guest_on_login(false);
            login(nav, state);
            let old = nav.pending.get_untracked().unwrap();
            // The attempt enters Checking synchronously; the API then refuses it outright.
            let lookup = state.write().begin_refresh().unwrap();
            nav.observe_session(state.get_untracked().phase());
            assert_eq!(nav.pending.get_untracked(), Some(old));
            state
                .write()
                .finish_session(
                    lookup,
                    Err(AccountFailure::LoginRefused(LoginRefusal::Credentials)),
                    || uuid::Uuid::from_u128(100).to_string(),
                )
                .unwrap();
            assert_eq!(state.get_untracked().phase(), SessionPhase::Guest);
            assert_eq!(
                state.get_untracked().login_refusal(),
                Some(LoginRefusal::Credentials)
            );
            nav.observe_session(state.get_untracked().phase());
            assert_eq!(nav.pending.get_untracked(), None);
            assert!(!nav.intents.get_untracked().accepts(old));
            assert!(matches!(nav.route.get_untracked(), Route::Auth(_)));
            // The form is usable again and the next attempt has a new fence.
            assert!(state.get_untracked().can_login());
            login(nav, state);
            let current = nav.pending.get_untracked().unwrap();
            assert_ne!(current, old);
            assert!(nav.intents.get_untracked().accepts(current));
        });
    }

    #[test]
    fn unfinished_cleanup_never_enables_a_new_attempt_until_acknowledged() {
        Owner::new().with(|| {
            let (nav, _) = guest_on_login(true);
            let state = RwSignal::new(AccountViewState::default());
            let cleanup = signed_out_cleanup(state);
            login(nav, state);
            assert_eq!(nav.pending.get_untracked(), None);
            assert!(!state.get_untracked().can_login());
            assert!(!state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable)));
            login(nav, state);
            assert_eq!(nav.pending.get_untracked(), None);
            let retry = state.write().begin_logout().unwrap();
            assert!(state.write().finish_logout(retry, Ok(())));
            login(nav, state);
            assert!(nav.pending.get_untracked().is_some());
        });
    }

    #[test]
    fn a_live_attempt_is_retained_while_checking_and_released_by_failure() {
        Owner::new().with(|| {
            let (mut nav, fence) = staged_login();
            nav.preview = false;
            for phase in [SessionPhase::Checking, SessionPhase::LogoutPending] {
                nav.observe_session(phase);
                assert_eq!(nav.pending.get_untracked(), Some(fence));
            }
            nav.observe_session(SessionPhase::Failed(AccountFailure::Unavailable));
            assert_eq!(nav.pending.get_untracked(), None);
            assert!(!nav.intents.get_untracked().accepts(fence));
        });
    }

    #[test]
    fn the_isolated_preview_keeps_its_staged_guest_scenario() {
        Owner::new().with(|| {
            let (nav, fence) = staged_login();
            nav.observe_session(SessionPhase::Guest);
            assert_eq!(nav.pending.get_untracked(), Some(fence));
        });
    }

    #[test]
    fn cancelled_attempt_and_stale_session_cannot_complete_a_repeated_login() {
        Owner::new().with(|| {
            let (nav, old_auth) = staged_login();
            let state = RwSignal::new(AccountViewState::default());
            let old_lookup = state.write().begin_refresh().unwrap();
            nav.cancel();
            assert!(!nav.intents.get_untracked().accepts(old_auth));
            let cleanup = state.write().begin_logout().unwrap();
            assert!(state.write().finish_logout(cleanup, Ok(())));
            login(nav, state);
            let pending = nav.pending.get_untracked();
            assert!(pending.is_some());
            login(nav, state);
            assert_eq!(nav.pending.get_untracked(), pending);

            let scope = AccountScope::from_session(
                "listener-a".into(),
                uuid::Uuid::from_u128(1).to_string(),
            )
            .unwrap();
            assert_eq!(
                state
                    .write()
                    .finish_session(old_lookup, Ok(scope.clone()), || uuid::Uuid::from_u128(100)
                        .to_string()),
                Ok(SessionCompletion::Ignored)
            );
            nav.observe_session(state.get_untracked().phase());
            assert_eq!(nav.pending.get_untracked(), pending);
            assert!(matches!(nav.route.get_untracked(), Route::Auth(_)));

            let current_lookup = state.write().begin_refresh().unwrap();
            assert!(matches!(
                state
                    .write()
                    .finish_session(current_lookup, Ok(scope), || uuid::Uuid::from_u128(100)
                        .to_string()),
                Ok(SessionCompletion::Authenticated(_))
            ));
            nav.observe_session(state.get_untracked().phase());
            // The listener opened sign-in from Discover and returns there.
            assert_eq!(nav.route.get_untracked(), Route::Discover);
            assert_eq!(nav.pending.get_untracked(), None);
        });
    }

    #[test]
    fn guest_auth_from_public_work_returns_to_that_work_on_back_or_sign_in() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            crate::features::auth_preview::guest(state).unwrap();
            let nav = LiveNavigation::new(state, NodeRef::new(), true);
            let work = PublicWorkId::parse(&uuid::Uuid::from_u128(42).to_string()).unwrap();

            nav.open_work(work, "card-42".into());
            assert_eq!(nav.route.get_untracked(), Route::PublicWork(work));

            // Guest presses Save -> open_auth(login) -> Back returns to PublicWork(work).
            nav.open_auth(AuthRoute::login(nav.origin()));
            assert!(matches!(nav.route.get_untracked(), Route::Auth(_)));
            nav.back();
            assert_eq!(nav.route.get_untracked(), Route::PublicWork(work));

            // Guest presses Save -> open_auth(login) -> completes login -> returns to PublicWork(work).
            nav.open_auth(AuthRoute::login(nav.origin()));
            login(nav, state);
            let lookup = state.write().begin_refresh().unwrap();
            let scope = AccountScope::from_session(
                "listener-a".into(),
                uuid::Uuid::from_u128(1).to_string(),
            )
            .unwrap();
            assert!(matches!(
                state
                    .write()
                    .finish_session(lookup, Ok(scope), || uuid::Uuid::from_u128(100).to_string()),
                Ok(SessionCompletion::Authenticated(_))
            ));
            nav.observe_session(state.get_untracked().phase());
            assert_eq!(nav.route.get_untracked(), Route::PublicWork(work));
        });
    }
}
