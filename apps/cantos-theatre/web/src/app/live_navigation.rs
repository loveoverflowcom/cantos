//! Auth presentation navigation owns no credentials and never decides session validity.
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthIntentState, AuthRequestFence, AuthRoute, ReturnDestination},
    },
    core::public_ids::PublicWorkId,
    platform::{self, auth_browser as browser},
};
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PendingAuthRequest {
    Outbound(AuthRequestFence),
    ConfirmingReturn(AuthRequestFence),
}

impl PendingAuthRequest {
    fn fence(self) -> AuthRequestFence {
        match self {
            Self::Outbound(fence) | Self::ConfirmingReturn(fence) => fence,
        }
    }
}

#[derive(Clone, Copy)]
pub(crate) struct LiveNavigation {
    pub route: RwSignal<Route>,
    preview: bool,
    intents: RwSignal<AuthIntentState>,
    pending: RwSignal<Option<PendingAuthRequest>>,
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
        let callback_kind = hash.split('?').next().unwrap_or_default();
        let callback = matches!(callback_kind, "#auth-return" | "#auth-cancelled");
        let restored = if callback {
            browser::take_provider_return(&hash)
        } else {
            browser::clear_provider_return();
            None
        };
        let initial = if callback {
            restored.map(Route::Auth).unwrap_or(Route::Discover)
        } else {
            Route::from_hash(&hash)
        };
        let auth = match initial {
            Route::Auth(auth) => Some(auth),
            _ => None,
        };
        let mut intents = AuthIntentState::new(auth);
        let pending = if callback_kind == "#auth-return" && restored.is_some() {
            intents
                .begin()
                .ok()
                .map(PendingAuthRequest::ConfirmingReturn)
        } else {
            None
        };
        let navigation = Self {
            route: RwSignal::new(initial),
            preview,
            intents: RwSignal::new(intents),
            pending: RwSignal::new(pending),
            returned: StoredValue::new(if restored.is_some() {
                browser::take_return_snapshot()
            } else {
                None
            }),
            work_return: StoredValue::new(None),
        };
        if callback {
            browser::go(navigation.route, initial, true);
        }
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
                browser::clear_provider_return();
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
                } else if let (Route::PublicWork(from), Route::Discover) = (old, current) {
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
        Effect::new(move |_| {
            navigation.observe_session(state.get().phase());
        });
        #[cfg(target_arch = "wasm32")]
        {
            use leptos::leptos_dom::helpers::window_event_listener_untyped;
            use web_sys::wasm_bindgen::JsCast;
            let read = move || {
                let destination = Route::from_hash(&browser::hash());
                if destination != navigation.route.get_untracked() {
                    navigation.cancel();
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
            if let Some(request) = self.pending.get_untracked() {
                let fence = request.fence();
                let mut accepted = false;
                self.intents.update(|s| {
                    accepted =
                        self.route.get_untracked() == Route::Auth(fence.route()) && s.finish(fence)
                });
                self.pending.set(None);
                browser::clear_provider_return();
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
                && match self.pending.get_untracked() {
                    Some(PendingAuthRequest::ConfirmingReturn(_)) => true,
                    Some(PendingAuthRequest::Outbound(_)) => !self.preview,
                    None => false,
                })
        {
            // Actual popup attempts enter Checking synchronously; Guest means cleanup finished.
            // The isolated preview retains its deliberately staged Outbound/Guest scenario.
            self.cancel();
        }
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
        self.intents.update(|s| {
            let _ = s.invalidate();
        });
        self.pending.set(None);
        browser::clear_provider_return();
    }
    pub fn back(self) {
        self.cancel();
        let destination = match self.route.get_untracked() {
            Route::Auth(auth) => Route::from_target(auth.back()),
            Route::Membership(member) => Route::from_target(member.back()),
            _ => Route::Discover,
        };
        browser::back(self.route, destination);
    }
    pub fn login(self, state: RwSignal<AccountViewState>) {
        if !state.get_untracked().can_login() {
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
        if self.preview {
            self.pending.set(Some(PendingAuthRequest::Outbound(fence)));
            return;
        }
        self.pending.set(Some(PendingAuthRequest::Outbound(fence)));
        #[cfg(target_arch = "wasm32")]
        crate::live::runtime::login(state);
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
        account_view::{AccountFailure, SessionCompletion},
    };

    fn returned_login() -> (LiveNavigation, AuthRequestFence) {
        let auth = AuthRoute::login(ReturnDestination::Account);
        let mut intents = AuthIntentState::new(Some(auth));
        let fence = intents.begin().unwrap();
        (
            LiveNavigation {
                route: RwSignal::new(Route::Auth(auth)),
                preview: true,
                intents: RwSignal::new(intents),
                pending: RwSignal::new(Some(PendingAuthRequest::ConfirmingReturn(fence))),
                returned: StoredValue::new(None),
                work_return: StoredValue::new(None),
            },
            fence,
        )
    }

    fn signed_out_cleanup(
        state: RwSignal<AccountViewState>,
    ) -> crate::application::account::AccountGeneration {
        let mut current = state.write();
        let lookup = current.begin_refresh().unwrap();
        let SessionCompletion::Cleanup(cleanup) = current
            .finish_session(lookup, Err(AccountFailure::SignedOut))
            .unwrap()
        else {
            panic!("a missing callback session must require cookie cleanup")
        };
        cleanup
    }

    #[test]
    fn callback_without_session_releases_its_fence_only_after_cleanup_and_allows_retry() {
        Owner::new().with(|| {
            let (nav, old) = returned_login();
            let state = RwSignal::new(AccountViewState::default());
            let cleanup = signed_out_cleanup(state);
            nav.observe_session(state.get_untracked().phase());
            assert!(!state.get_untracked().can_login());
            assert_eq!(
                nav.pending.get_untracked(),
                Some(PendingAuthRequest::ConfirmingReturn(old))
            );

            assert!(!state
                .write()
                .finish_logout(cleanup, Err(AccountFailure::Unavailable)));
            nav.observe_session(state.get_untracked().phase());
            nav.login(state);
            assert!(!state.get_untracked().can_login());
            assert_eq!(
                nav.pending.get_untracked(),
                Some(PendingAuthRequest::ConfirmingReturn(old))
            );
            assert!(nav.intents.get_untracked().accepts(old));

            let retry = state.write().begin_logout().unwrap();
            assert!(state.write().finish_logout(retry, Ok(())));
            nav.observe_session(state.get_untracked().phase());
            assert!(state.get_untracked().can_login());
            assert_eq!(
                nav.route.get_untracked(),
                Route::Auth(AuthRoute::login(ReturnDestination::Account))
            );
            assert_eq!(nav.pending.get_untracked(), None);
            assert!(!nav.intents.get_untracked().accepts(old));

            nav.login(state);
            let Some(PendingAuthRequest::Outbound(current)) = nav.pending.get_untracked() else {
                panic!("retry must start a new outbound request")
            };
            assert_ne!(current, old);
            let before = nav.intents.get_untracked();
            nav.login(state);
            assert_eq!(
                nav.pending.get_untracked(),
                Some(PendingAuthRequest::Outbound(current))
            );
            assert_eq!(nav.intents.get_untracked(), before);
        });
    }

    #[test]
    fn completed_popup_guest_releases_live_outbound_intent_for_retry() {
        Owner::new().with(|| {
            let (mut nav, fence) = returned_login();
            nav.preview = false;
            nav.pending.set(Some(PendingAuthRequest::Outbound(fence)));
            nav.observe_session(SessionPhase::Guest);
            assert_eq!(nav.pending.get_untracked(), None);
            assert!(!nav.intents.get_untracked().accepts(fence));
        });
    }
    #[test]
    fn outbound_login_is_retained_while_guest_and_during_unacknowledged_cleanup() {
        Owner::new().with(|| {
            let (nav, _) = returned_login();
            nav.cancel();
            let state = RwSignal::new(AccountViewState::default());
            let cleanup = signed_out_cleanup(state);
            assert!(state.write().finish_logout(cleanup, Ok(())));
            nav.login(state);
            let pending = nav.pending.get_untracked();
            assert!(matches!(pending, Some(PendingAuthRequest::Outbound(_))));
            for phase in [
                SessionPhase::Guest,
                SessionPhase::Checking,
                SessionPhase::LogoutPending,
            ] {
                nav.observe_session(phase);
                assert_eq!(nav.pending.get_untracked(), pending);
            }
            nav.observe_session(SessionPhase::Failed(AccountFailure::Unavailable));
            assert_eq!(nav.pending.get_untracked(), None);
        });
    }

    #[test]
    fn cancelled_callback_and_stale_session_cannot_complete_a_repeated_login() {
        Owner::new().with(|| {
            let (nav, old_auth) = returned_login();
            let state = RwSignal::new(AccountViewState::default());
            let old_lookup = state.write().begin_refresh().unwrap();
            nav.cancel();
            let cleanup = state.write().begin_logout().unwrap();
            assert!(state.write().finish_logout(cleanup, Ok(())));
            nav.login(state);
            let pending = nav.pending.get_untracked();
            assert!(matches!(pending, Some(PendingAuthRequest::Outbound(_))));
            nav.login(state);
            assert_eq!(nav.pending.get_untracked(), pending);
            assert!(!nav.intents.get_untracked().accepts(old_auth));

            let scope = AccountScope::from_session(
                "listener-a".into(),
                uuid::Uuid::from_u128(1).to_string(),
            )
            .unwrap();
            assert_eq!(
                state.write().finish_session(old_lookup, Ok(scope.clone())),
                Ok(SessionCompletion::Ignored)
            );
            nav.observe_session(state.get_untracked().phase());
            assert_eq!(nav.pending.get_untracked(), pending);
            assert!(matches!(nav.route.get_untracked(), Route::Auth(_)));

            let current_lookup = state.write().begin_refresh().unwrap();
            assert!(matches!(
                state.write().finish_session(current_lookup, Ok(scope)),
                Ok(SessionCompletion::Authenticated(_))
            ));
            nav.observe_session(state.get_untracked().phase());
            assert_eq!(nav.route.get_untracked(), Route::Account);
            assert_eq!(nav.pending.get_untracked(), None);
        });
    }
}
