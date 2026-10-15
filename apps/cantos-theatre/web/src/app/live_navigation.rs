//! Auth presentation navigation owns no credentials and never decides session validity.
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::{AuthIntentState, AuthRequestFence, AuthRoute, ReturnDestination},
    },
    platform::{self, auth_browser as browser},
};
use leptos::prelude::*;

#[derive(Clone, Copy)]
pub(crate) struct LiveNavigation {
    pub route: RwSignal<Route>,
    preview: bool,
    intents: RwSignal<AuthIntentState>,
    pending: RwSignal<Option<AuthRequestFence>>,
    returned: StoredValue<Option<(Route, browser::ReturnSnapshot)>>,
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
            intents.begin().ok()
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
            if state.get().phase() == SessionPhase::SignedIn {
                if let Some(fence) = navigation.pending.get_untracked() {
                    let mut accepted = false;
                    navigation.intents.update(|s| {
                        accepted = navigation.route.get_untracked() == Route::Auth(fence.route())
                            && s.finish(fence)
                    });
                    navigation.pending.set(None);
                    browser::clear_provider_return();
                    if accepted {
                        browser::go(
                            navigation.route,
                            Route::from_target(fence.route().origin.target()),
                            true,
                        );
                    }
                }
            } else if matches!(state.get().phase(), SessionPhase::Failed(_)) {
                navigation.cancel();
            }
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

    pub fn open_auth(self, auth: AuthRoute) {
        if !matches!(self.route.get_untracked(), Route::Auth(_)) {
            self.returned.set_value(Some((
                self.route.get_untracked(),
                browser::capture_return(),
            )));
        }
        self.cancel();
        browser::go(self.route, Route::Auth(auth), false);
    }
    pub fn cancel(self) {
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
            self.pending.set(Some(fence));
            return;
        }
        let Some(intent) = browser::save_provider_return(auth) else {
            self.cancel();
            state.update(AccountViewState::navigation_failed);
            return;
        };
        if let Some((destination, snapshot)) = self.returned.get_value() {
            browser::save_return_snapshot(destination, snapshot);
        }
        self.pending.set(Some(fence));
        #[cfg(target_arch = "wasm32")]
        crate::live::runtime::login(state, &intent);
        #[cfg(not(target_arch = "wasm32"))]
        let _ = intent;
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
