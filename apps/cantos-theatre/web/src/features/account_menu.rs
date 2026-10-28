//! Header account entry: a signed-in disclosure and one logout confirmation.
//!
//! App-private composition over the shared Dialog/Button. The mounted account (`AccountViewState`)
//! stays the only auth owner: this file projects `AccountUi`, opens/closes overlays and calls the
//! existing logout callback exactly once per confirmation.
use super::{
    account_ui::{account_ui, AccountUi},
    common::PersonGlyph,
};
use crate::{
    app::routes::Route,
    application::{
        account_view::{AccountViewState, SessionPhase},
        auth_navigation::MembershipReturn,
    },
    messages::{self, Key, Locale},
};
use cantos_ui::{
    Button, ButtonState, ButtonVariant, Dialog, DialogCloseReason, DisabledReason, Icon, IconName,
};
use leptos::{ev, html, prelude::*};

/// What the confirmation shows. It is derived from the account owner, never stored.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LogoutStep {
    /// Nothing has been dispatched; confirming sends the one logout intent.
    Ask,
    /// Dispatched and cleanup is running; a repeat is refused.
    InProgress,
    /// Logout is not confirmed; the owner allows a retry. Neither success nor "all saved".
    Unconfirmed,
    /// Guest, checking or failed: no sign-out question is left to ask.
    Settled,
}

impl LogoutStep {
    pub(crate) fn of(ui: &AccountUi) -> Self {
        match ui.phase {
            SessionPhase::SignedIn => Self::Ask,
            SessionPhase::LogoutPending if ui.cleanup_in_flight => Self::InProgress,
            SessionPhase::LogoutPending if ui.can_retry_logout => Self::Unconfirmed,
            _ => Self::Settled,
        }
    }

    pub(crate) const fn can_confirm(self) -> bool {
        matches!(self, Self::Ask | Self::Unconfirmed)
    }
}

#[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
/// A click on `::backdrop` is reported on the dialog element with coordinates outside its box.
pub(crate) fn outside_box(x: f64, y: f64, left: f64, top: f64, right: f64, bottom: f64) -> bool {
    x < left || x > right || y < top || y > bottom
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::outside_box;
    use leptos::{
        ev, html,
        prelude::*,
        wasm_bindgen::JsCast,
        web_sys::{HtmlElement, Node},
    };

    pub(super) fn focus(element: NodeRef<html::Button>) {
        if let Some(element) = element.get_untracked() {
            let _ = element.unchecked_into::<HtmlElement>().focus();
        }
    }

    /// After a dismissal that left focus on nothing (the avatar was replaced while a logout was
    /// pending), land on the entry that now exists rather than at the top of the document.
    pub(super) fn focus_entry_if_lost(id: &str) {
        let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) else {
            return;
        };
        let lost = document
            .active_element()
            .is_none_or(|active| active.tag_name() == "BODY");
        if lost {
            focus_by_id(id);
        }
    }

    pub(super) fn focus_by_id(id: &str) -> Option<HtmlElement> {
        let element = leptos::web_sys::window()?
            .document()?
            .get_element_by_id(id)?
            .dyn_into::<HtmlElement>()
            .ok()?;
        element.focus().ok()?;
        Some(element)
    }

    /// Only a focus move to a real element outside the disclosure closes it; a click on
    /// non-focusable padding reports no related target and must not.
    pub(super) fn focus_left(event: &ev::FocusEvent, wrapper: NodeRef<html::Div>) -> bool {
        let (Some(wrapper), Some(related)) = (wrapper.get_untracked(), event.related_target())
        else {
            return false;
        };
        related
            .dyn_ref::<Node>()
            .is_some_and(|node| !wrapper.contains(Some(node)))
    }

    pub(super) fn backdrop_hit(event: &ev::MouseEvent) -> bool {
        let (Some(target), Some(current)) = (event.target(), event.current_target()) else {
            return false;
        };
        if target != current {
            return false;
        }
        let Ok(dialog) = current.dyn_into::<HtmlElement>() else {
            return false;
        };
        let rect = dialog.get_bounding_client_rect();
        outside_box(
            f64::from(event.client_x()),
            f64::from(event.client_y()),
            rect.left(),
            rect.top(),
            rect.right(),
            rect.bottom(),
        )
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod browser {
    use leptos::{ev, html, prelude::*};

    pub(super) fn focus(_: NodeRef<html::Button>) {}
    pub(super) fn focus_by_id(_: &str) -> Option<leptos::web_sys::HtmlElement> {
        None
    }
    pub(super) fn focus_entry_if_lost(_: &str) {}
    pub(super) fn focus_left(_: &ev::FocusEvent, _: NodeRef<html::Div>) -> bool {
        false
    }
    pub(super) fn backdrop_hit(_: &ev::MouseEvent) -> bool {
        false
    }
}

/// The same "focus moved to a real element outside this wrapper" rule the account disclosure uses.
pub(super) fn focus_left(event: &ev::FocusEvent, wrapper: NodeRef<html::Div>) -> bool {
    browser::focus_left(event, wrapper)
}

const MENU_ID: &str = "theatre-account-menu";
const ENTRY_ID: &str = "theatre-account-entry";
const CANCEL_ID: &str = "theatre-logout-cancel";

fn text(key: Key, locale: RwSignal<Locale>) -> impl Fn() -> &'static str + Copy {
    move || messages::text(key, locale.get())
}

/// Presentational rows only. Each destination is a route that exists today; edit-profile and a
/// support contact have no verified capability, so support is information and edit is absent.
#[component]
fn AccountMenuPanel(
    locale: RwSignal<Locale>,
    on_pick: Callback<()>,
    on_logout: Callback<()>,
) -> impl IntoView {
    let membership = Route::Membership(MembershipReturn::Account).hash();
    view! {
        <div id=MENU_ID class="theatre-account-menu" role="group"
            aria-label=text(Key::Account, locale)>
            <div class="theatre-account-menu-identity">
                <span class="theatre-avatar" data-size="large" aria-hidden="true"><PersonGlyph/></span>
                <div class="theatre-account-menu-name">
                    <p class="theatre-account-menu-title">{text(Key::AccountMenuListener, locale)}</p>
                </div>
                <p class="theatre-account-menu-tagline">{text(Key::AccountMenuTagline, locale)}</p>
            </div>
            <ul class="theatre-account-menu-list">
                <li><a id="theatre-account-menu-membership" class="theatre-account-menu-row" href=membership
                    on:click=move |_| on_pick.run(())>
                    <Icon name=IconName::Star/>
                    <span>{text(Key::AccountMenuMembership, locale)}</span>
                </a></li>
                <li><a id="theatre-account-menu-library" class="theatre-account-menu-row" href=Route::Library.hash()
                    on:click=move |_| on_pick.run(())>
                    <Icon name=IconName::MenuBook/>
                    <span>{text(Key::AccountMenuLibrary, locale)}</span>
                </a></li>
                <li><a id="theatre-account-menu-settings" class="theatre-account-menu-row" href=Route::Settings.hash()
                    on:click=move |_| on_pick.run(())>
                    <Icon name=IconName::Settings/>
                    <span>{text(Key::AccountMenuSettings, locale)}</span>
                </a></li>
            </ul>
            <p id="theatre-account-menu-support" class="theatre-account-menu-info">
                {text(Key::AccountMenuSupportPending, locale)}
            </p>
            <button id="theatre-account-menu-logout" type="button"
                class="theatre-account-menu-row theatre-account-menu-logout"
                on:click=move |_| on_logout.run(())>
                <Icon name=IconName::Logout/>
                <span>{text(Key::SignOut, locale)}</span>
            </button>
        </div>
    }
}

fn confirm_state(step: LogoutStep, locale: Locale) -> ButtonState {
    if step != LogoutStep::InProgress {
        return ButtonState::Enabled;
    }
    DisabledReason::new(messages::text(Key::LogoutInProgress, locale))
        .map(ButtonState::Loading)
        .unwrap_or(ButtonState::Enabled)
}

#[component]
fn LogoutDialog(
    locale: RwSignal<Locale>,
    open: RwSignal<bool>,
    step: Memo<LogoutStep>,
    on_confirm: Callback<()>,
) -> impl IntoView {
    let close = Callback::new(move |_| open.set(false));
    // The locale is read inside the closure so a language change remounts the closed dialog.
    view! {
        {move || {
            let language = locale.get();
            view! {
                <Dialog id="theatre-logout" class="theatre-logout-dialog"
                    title=messages::text(Key::LogoutTitle, language)
                    close_label=messages::text(Key::LogoutDismiss, language)
                    description=messages::text(Key::LogoutReassurance, language)
                    open=Signal::from(open)
                    on_close_request=Callback::new(move |_: DialogCloseReason| open.set(false))
                    on_error=Callback::new(move |_| open.set(false))
                    initial_focus=Callback::new(|()| browser::focus_by_id(CANCEL_ID))
                    actions=ViewFn::from(move || view! {
                        // Back leads with the arrow; while sign-out runs the same action only closes.
                        {move || if step.get() == LogoutStep::InProgress {
                            view! {
                                <Button id=CANCEL_ID variant=ButtonVariant::AuthSecondary
                                    on_press=Callback::new(move |_| close.run(()))>
                                    {messages::text(Key::LogoutDismiss, language)}
                                </Button>
                            }.into_any()
                        } else {
                            view! {
                                <Button id=CANCEL_ID variant=ButtonVariant::AuthSecondary icon=IconName::ArrowBack
                                    on_press=Callback::new(move |_| close.run(()))>
                                    {messages::text(Key::LogoutCancel, language)}
                                </Button>
                            }.into_any()
                        }}
                        <Button id="theatre-logout-confirm" variant=ButtonVariant::AuthPrimary icon=IconName::Logout
                            state=Signal::derive(move || confirm_state(step.get(), language))
                            on_press=Callback::new(move |_| on_confirm.run(()))>
                            {messages::text(Key::SignOut, language)}
                        </Button>
                    })
                    on:click=move |event| {
                        if browser::backdrop_hit(&event) {
                            open.set(false);
                        }
                    }>
                    <p id="theatre-logout-status" class="theatre-logout-status theatre-auth-feedback" data-tone="warning" role="status" aria-live="polite" aria-atomic="true">
                        {move || (step.get() == LogoutStep::Unconfirmed)
                            .then(|| messages::text(Key::LogoutUnconfirmed, language))}
                    </p>
                </Dialog>
            }
        }}
    }
}

/// The header avatar. Signed in: a disclosure button; otherwise the existing link to Account.
#[component]
pub(crate) fn AccountEntry(
    state: ReadSignal<AccountViewState>,
    locale: RwSignal<Locale>,
    route: RwSignal<Route>,
    on_logout: Callback<()>,
) -> impl IntoView {
    let ui = account_ui(state);
    let signed_in = Memo::new(move |_| ui.get().phase == SessionPhase::SignedIn);
    let step = Memo::new(move |_| LogoutStep::of(&ui.get()));
    let menu_open = RwSignal::new(false);
    let dialog_open = RwSignal::new(false);
    let avatar = NodeRef::<html::Button>::new();
    let wrapper = NodeRef::<html::Div>::new();

    let close_menu = move || {
        if menu_open.get_untracked() {
            menu_open.set(false);
        }
    };
    // A route change or a lost session ends the disclosure; the dialog follows the owner's step.
    Effect::new(move |_| {
        route.track();
        signed_in.track();
        close_menu();
    });
    Effect::new(move |_| {
        if step.get() == LogoutStep::Settled && dialog_open.get_untracked() {
            dialog_open.set(false);
        }
    });
    // The shared Dialog restores focus to the avatar that opened it; this covers only the case
    // where that element was replaced meanwhile. Wait one tick so the Dialog restores first.
    Effect::new(move |was_open: Option<bool>| {
        let is_open = dialog_open.get();
        if was_open == Some(true) && !is_open {
            set_timeout(
                || browser::focus_entry_if_lost(ENTRY_ID),
                std::time::Duration::ZERO,
            );
        }
        is_open
    });
    let request_logout = Callback::new(move |()| {
        close_menu();
        // The dialog returns focus to the element active when it opens: make that the avatar.
        browser::focus(avatar);
        dialog_open.set(true);
    });
    let confirm = Callback::new(move |()| {
        // Cancel never reaches here; a pending or settled owner refuses a repeat.
        if step.get_untracked().can_confirm() {
            on_logout.run(());
        }
    });
    let label = text(Key::Account, locale);
    view! {
        <div class="theatre-account-disclosure" node_ref=wrapper
            on:keydown=move |event: ev::KeyboardEvent| {
                if event.key() == "Escape" && menu_open.get_untracked() {
                    event.prevent_default();
                    menu_open.set(false);
                    browser::focus(avatar);
                }
            }
            on:focusout=move |event: ev::FocusEvent| {
                if browser::focus_left(&event, wrapper) {
                    close_menu();
                }
            }>
            {move || if signed_in.get() {
                view! {
                    <button id=ENTRY_ID node_ref=avatar type="button" class="theatre-avatar" data-account="signed-in"
                        aria-label=label aria-controls=MENU_ID
                        aria-expanded=move || menu_open.get().to_string()
                        aria-current=move || (route.get() == Route::Account).then_some("page")
                        on:click=move |_| menu_open.update(|open| *open = !*open)>
                        <PersonGlyph/>
                    </button>
                }.into_any()
            } else {
                view! {
                    <a id=ENTRY_ID class="theatre-avatar" href=Route::Account.hash() aria-label=label
                        aria-current=move || (route.get() == Route::Account).then_some("page")>
                        <PersonGlyph/>
                    </a>
                }.into_any()
            }}
            <Show when=move || signed_in.get() && menu_open.get()>
                <div class="theatre-account-scrim" aria-hidden="true" on:click=move |_| close_menu()></div>
                <AccountMenuPanel locale on_pick=Callback::new(move |()| close_menu()) on_logout=request_logout/>
            </Show>
        </div>
        // Keep the native closed dialog in the header's lifetime. A session check can change
        // SignedIn → Checking while its control effects are queued; disposing the dialog then
        // invalidates their arena-backed signals and traps release WASM. Only user intent opens
        // the modal, and the existing step guard still rejects confirmation outside admission.
        <LogoutDialog locale open=dialog_open step on_confirm=confirm/>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::features::auth_preview::{guest, signed_in, PreviewActor};

    fn ui_of(state: RwSignal<AccountViewState>) -> AccountUi {
        account_ui(state.read_only()).get_untracked()
    }

    #[test]
    fn logout_step_follows_the_owner_and_only_ask_or_retry_may_confirm() {
        Owner::new().with(|| {
            let state = RwSignal::new(AccountViewState::default());
            guest(state).unwrap();
            assert_eq!(LogoutStep::of(&ui_of(state)), LogoutStep::Settled);
            signed_in(state, PreviewActor::A).unwrap();
            assert_eq!(LogoutStep::of(&ui_of(state)), LogoutStep::Ask);
            let cleanup = state.write().begin_logout().unwrap();
            let pending = LogoutStep::of(&ui_of(state));
            assert_eq!(pending, LogoutStep::InProgress);
            assert!(!pending.can_confirm(), "a pending logout is not replayed");
            state.write().finish_logout(
                cleanup,
                Err(crate::application::account_view::AccountFailure::Unavailable),
            );
            let unconfirmed = LogoutStep::of(&ui_of(state));
            assert_eq!(unconfirmed, LogoutStep::Unconfirmed);
            assert!(unconfirmed.can_confirm());
            let retry = state.write().begin_logout().unwrap();
            state.write().finish_logout(retry, Ok(()));
            assert_eq!(LogoutStep::of(&ui_of(state)), LogoutStep::Settled);
            assert!(!LogoutStep::Settled.can_confirm());
        });
    }

    #[test]
    fn backdrop_hits_are_clicks_outside_the_dialog_box_only() {
        let (l, t, r, b) = (100.0, 50.0, 580.0, 382.0);
        assert!(outside_box(10.0, 60.0, l, t, r, b));
        assert!(outside_box(200.0, 400.0, l, t, r, b));
        assert!(!outside_box(100.0, 50.0, l, t, r, b), "the edge is inside");
        assert!(!outside_box(300.0, 200.0, l, t, r, b), "padding is inside");
    }

    fn render_entry(signed: bool, language: Locale) -> String {
        let state = RwSignal::new(AccountViewState::default());
        guest(state).unwrap();
        if signed {
            signed_in(state, PreviewActor::A).unwrap();
        }
        let locale = RwSignal::new(language);
        let route = RwSignal::new(Route::Discover);
        view! { <AccountEntry state=state.read_only() locale route on_logout=Callback::new(|()| {})/> }
            .to_html()
    }

    #[test]
    fn logout_dialog_lifetime_is_independent_of_session_admission() {
        Owner::new().with(|| {
            for signed in [false, true] {
                let html = render_entry(signed, Locale::Vietnamese);
                assert_eq!(html.matches("<dialog").count(), 1);
                assert!(!html.contains("<dialog open"));
                assert_eq!(html.matches("id=\"theatre-logout-confirm\"").count(), 1);
            }
            assert!(!LogoutStep::Settled.can_confirm());
        });
    }

    #[test]
    fn guest_keeps_the_account_link_and_no_disclosure_or_open_dialog() {
        Owner::new().with(|| {
            let html = render_entry(false, Locale::Vietnamese);
            assert!(html.contains("<a") && html.contains("href=\"#account\""));
            assert!(html.contains("aria-label=\"Tài khoản\""));
            assert!(!html.contains("data-account=\"signed-in\""));
            assert_eq!(html.matches("<dialog").count(), 1);
            assert!(!html.contains("<dialog open"));
            for absent in ["aria-expanded", "theatre-account-menu"] {
                assert!(!html.contains(absent), "{absent}");
            }
        });
    }

    #[test]
    fn signed_in_avatar_is_a_collapsed_named_disclosure_with_one_closed_logout_dialog() {
        Owner::new().with(|| {
            for (language, name, title, reassurance, cancel, confirm) in [
                (
                    Locale::Vietnamese,
                    "Tài khoản",
                    "Đăng xuất khỏi Cantos?",
                    "Bạn luôn có thể quay lại Cantos để tiếp tục câu chuyện của mình.",
                    "Quay lại",
                    "Đăng xuất",
                ),
                (
                    Locale::English,
                    "Account",
                    "Sign out of Cantos?",
                    "You can always return to Cantos to continue your story.",
                    "Back",
                    "Sign out",
                ),
            ] {
                let html = render_entry(true, language);
                assert!(html.contains("id=\"theatre-account-entry\""));
                assert!(html.contains("data-account=\"signed-in\""));
                assert!(html.contains(&format!("aria-label=\"{name}\"")));
                assert!(
                    html.contains("aria-expanded=\"false\"")
                        && html.contains("aria-controls=\"theatre-account-menu\"")
                );
                assert!(
                    !html.contains("id=\"theatre-account-menu\""),
                    "collapsed by default"
                );
                assert_eq!(html.matches("<dialog").count(), 1);
                assert!(
                    !html.contains("<dialog open"),
                    "no logout question before the user asks"
                );
                for required in [title, reassurance, cancel, confirm] {
                    assert!(html.contains(required), "{required}");
                }
                assert_eq!(html.matches("id=\"theatre-logout-confirm\"").count(), 1);
                assert!(html.contains("id=\"theatre-logout-cancel\""));
                let cancel = html.split("id=\"theatre-logout-cancel\"").nth(1).unwrap();
                let cancel = &cancel[..cancel.find("</button>").unwrap()];
                assert!(
                    cancel.find("cui-button-icon").unwrap()
                        < cancel.find(cancel_label(language)).unwrap(),
                    "the arrow leads the label"
                );
                assert!(
                    cancel.contains(ARROW_BACK_PATH) && !cancel.contains("aria-label"),
                    "a decorative arrow_back, not the logout icon"
                );
            }
        });
    }

    const ARROW_BACK_PATH: &str = "M20 11H7.83l5.59-5.59";

    fn cancel_label(language: Locale) -> &'static str {
        messages::text(Key::LogoutCancel, language)
    }

    #[test]
    fn the_back_button_is_named_back() {
        assert_eq!(cancel_label(Locale::Vietnamese), "Quay lại");
        assert_eq!(cancel_label(Locale::English), "Back");
    }

    #[test]
    fn menu_rows_are_real_routes_support_is_information_and_identity_is_never_invented() {
        Owner::new().with(|| {
            for (language, rows) in [
                (
                    Locale::Vietnamese,
                    [
                        "Tìm hiểu hội viên",
                        "Thư viện &amp; Lịch sử nghe",
                        "Cài đặt &amp; Giao diện",
                        "Thông tin hỗ trợ đang cập nhật",
                        "Người nghe Cantos",
                    ],
                ),
                (
                    Locale::English,
                    [
                        "Explore membership",
                        "Library &amp; listening history",
                        "Settings &amp; appearance",
                        "Support information is being updated",
                        "Cantos listener",
                    ],
                ),
            ] {
                let locale = RwSignal::new(language);
                let noop = Callback::new(|()| {});
                let html =
                    view! { <AccountMenuPanel locale on_pick=noop on_logout=noop/> }.to_html();
                for row in rows {
                    assert!(html.contains(row), "{row}");
                }
                for href in [
                    "href=\"#library\"",
                    "href=\"#settings\"",
                    "href=\"#membership?from=account\"",
                ] {
                    assert!(html.contains(href), "{href}");
                }
                // Support has no verified destination; edit-profile has no capability.
                assert!(
                    !html.contains("mailto:")
                        && !html.contains("Sửa hồ sơ")
                        && !html.contains("Edit profile")
                );
                assert!(!html.contains("example.com") && !html.contains("Minh Anh"));
                // A healthy signed-in session is not announced under the name.
                for obsolete in ["Đã đăng nhập", "Signed in", "theatre-account-menu-status"] {
                    assert!(!html.contains(obsolete), "{obsolete}");
                }
                assert_eq!(
                    html.matches("<button").count(),
                    1,
                    "only logout is a button"
                );
                assert_eq!(html.matches("<a ").count(), 3, "support is not a link");
            }
        });
    }

    #[test]
    fn pending_confirm_is_loading_with_a_visible_reason_and_other_steps_are_enabled() {
        for locale in [Locale::Vietnamese, Locale::English] {
            let state = confirm_state(LogoutStep::InProgress, locale);
            assert!(!state.can_press() && state.is_busy());
            assert_eq!(
                state.reason().unwrap().as_str(),
                messages::text(Key::LogoutInProgress, locale)
            );
            for step in [
                LogoutStep::Ask,
                LogoutStep::Unconfirmed,
                LogoutStep::Settled,
            ] {
                assert!(confirm_state(step, locale).can_press());
            }
        }
    }
}
