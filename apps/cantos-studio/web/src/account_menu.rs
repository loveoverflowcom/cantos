//! Studio account presentation; session actions and typed credentials stay shell-owned.
use cantos_ui::{Button, ButtonVariant, Dialog, DialogError, IconName};
use leptos::prelude::*;

pub const MENU_ID: &str = "studio-account-menu";

/// One footer trigger per presentation; the shell closes navigation before opening Account.
/// The label names the Account action, never a person. The neutral shared Person icon needs
/// no actor, initials, tone or picture; the former ActorBadge was retired by the footer layout.
#[component]
pub fn AccountTrigger(
    scope: &'static str,
    label: Signal<String>,
    open: Signal<bool>,
    on_open: Callback<&'static str>,
) -> impl IntoView {
    let id = format!("studio-{scope}-account");
    view! {
        <Button id class="studio-account-trigger" variant=ButtonVariant::Text icon=IconName::Person
            focus_on_press=true aria_controls=MENU_ID aria_expanded=open
            on_press=Callback::new(move |_| on_open.run(scope))>
            {move || label.get()}
        </Button>
    }
}

/// A single shared modal lives outside both navigation presentations. It never derives a name
/// or image from the opaque actor key. Dialog owns focus containment, dismissal and restoration.
#[component]
pub fn AccountEntry(
    #[prop(into)] available: Signal<bool>,
    open: RwSignal<bool>,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] close_label: Signal<String>,
    #[prop(into)] failure_label: Signal<String>,
    children: ChildrenFn,
) -> impl IntoView {
    let failure = RwSignal::new(false);
    let children = StoredValue::new(children);
    Effect::new(move |_| {
        if !available.get() {
            open.set(false);
        }
        if open.get() {
            failure.set(false);
        }
    });
    view! {
        <p role="alert" hidden=move || !failure.get()>{move || failure_label.get()}</p>
        {move || {
            let children = children.get_value();
            view! {
                <Dialog id=MENU_ID class="studio-account-dialog"
                    title=label.get() close_label=close_label.get()
                    open=Signal::derive(move || available.get() && open.get())
                    on_close_request=Callback::new(move |_| open.set(false))
                    on_error=Callback::new(move |error: DialogError| {
                        open.set(false);
                        // A resize/logout can hide the original opener; prefer the currently
                        // visible Account trigger or navigation Menu instead of losing focus.
                        if (error != DialogError::ReturnFocusRejected || !restore_visible_opener())
                            && available.get_untracked() {
                            failure.set(true);
                        }
                    })>
                    <div class="studio-account-actions">{children()}</div>
                </Dialog>
            }
        }}
    }
}

#[cfg(target_arch = "wasm32")]
fn restore_visible_opener() -> bool {
    use leptos::wasm_bindgen::JsCast;
    let Some(document) = web_sys::window().and_then(|window| window.document()) else {
        return false;
    };
    [
        "studio-rail-account",
        "studio-navigation-menu",
        "studio-entry-title",
    ]
    .into_iter()
    .filter_map(|id| document.get_element_by_id(id))
    .filter_map(|node| node.dyn_into::<web_sys::HtmlElement>().ok())
    .filter(|node| node.get_bounding_client_rect().width() > 0.0)
    .any(|node| node.focus().is_ok())
}

#[cfg(not(target_arch = "wasm32"))]
fn restore_visible_opener() -> bool {
    false
}

/// Delay until the controlled navigation Dialog has closed and restored Menu focus. Its modal
/// must never overlap Account. Check live availability again before accepting the queued intent.
#[cfg(target_arch = "wasm32")]
pub fn open_after_navigation(
    navigation: RwSignal<bool>,
    account: RwSignal<bool>,
    available: Signal<bool>,
) {
    navigation.set(false);
    leptos::leptos_dom::helpers::request_animation_frame(move || {
        if available.try_get_untracked() == Some(true)
            && navigation.try_get_untracked() == Some(false)
        {
            let _ = account.try_set(true);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::messages::{
        entry::{text, Key},
        Locale,
    };

    #[test]
    fn scoped_triggers_control_exactly_one_named_dialog() {
        Owner::new().with(|| {
            let open = RwSignal::new(false);
            let label = Signal::derive(|| "Tài khoản".to_owned());
            let html = view! {
                <AccountTrigger scope="rail" label open=open.into() on_open=Callback::new(|_| {})/>
                <AccountTrigger scope="drawer" label open=open.into() on_open=Callback::new(|_| {})/>
                <AccountEntry available=true open label close_label="Đóng" failure_label="Không thể mở">
                    <button id="probe-action" type="button">"Đăng xuất"</button>
                </AccountEntry>
            }.to_html();
            assert_eq!(html.matches("id=\"studio-account-menu\"").count(), 1, "{html}");
            for scope in ["rail", "drawer"] {
                assert!(html.contains(&format!("id=\"studio-{scope}-account\"")), "{html}");
            }
            assert_eq!(html.matches("aria-controls=\"studio-account-menu\"").count(), 2, "{html}");
            assert!(html.contains("<dialog"), "{html}");
            assert!(html.contains("probe-action"), "{html}");
            assert!(!html.contains("studio-avatar"), "{html}");
        });
    }

    #[test]
    fn account_chrome_names_the_action_in_both_locales_without_a_profile() {
        for (locale, account, close) in [
            (Locale::Vietnamese, "Tài khoản", "Đóng menu tài khoản"),
            (Locale::English, "Account", "Close account menu"),
        ] {
            for expanded in [false, true] {
                Owner::new().with(|| {
                    let open = RwSignal::new(expanded);
                    let label = Signal::derive(move || text(Key::Account, locale).to_owned());
                    let close_label = text(Key::CloseAccount, locale);
                    assert_eq!(label.get(), account);
                    assert_eq!(close_label, close);
                    let html = view! {
                        <AccountTrigger scope="rail" label open=open.into()
                            on_open=Callback::new(|_| {})/>
                        <AccountEntry available=true open label close_label
                            failure_label="dialog failure">
                            <span>"action slot"</span>
                        </AccountEntry>
                    }
                    .to_html();
                    assert!(
                        html.contains(&format!("aria-expanded=\"{expanded}\"")),
                        "{html}"
                    );
                    assert!(
                        html.contains("aria-controls=\"studio-account-menu\""),
                        "{html}"
                    );
                    assert!(
                        html.contains("aria-labelledby=\"studio-account-menu-title\""),
                        "{html}"
                    );
                    assert!(html.contains(&format!(">{account}</h2>")), "{html}");
                    assert!(html.contains(&format!("{close}</button>")), "{html}");
                    assert!(html.contains("aria-hidden=\"true\""), "{html}");
                    for profile_markup in [
                        "studio-avatar",
                        "cui-avatar",
                        "data-tone",
                        "data-initials",
                        "<img",
                        "role=\"img\"",
                    ] {
                        assert!(!html.contains(profile_markup), "{profile_markup}: {html}");
                    }
                });
            }
        }
    }
}
