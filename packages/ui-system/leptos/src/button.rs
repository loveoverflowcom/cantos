use leptos::{ev, ev::MouseEvent, prelude::*};

use crate::{feedback::LoadingIndicator, Icon, IconName};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Tonal,
    Secondary,
    Text,
    Danger,
    /// Capsule CTA for auth/account only; ordinary actions keep their existing geometry.
    AuthPrimary,
    AuthSecondary,
    AuthOutlined,
}

impl ButtonVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Tonal => "tonal",
            Self::Secondary => "secondary",
            Self::Text => "text",
            Self::Danger => "danger",
            Self::AuthPrimary => "auth-primary",
            Self::AuthSecondary => "auth-secondary",
            Self::AuthOutlined => "auth-outlined",
        }
    }
}

/// A blocked action cannot be constructed from a raw/blank string without admission.
/// ```compile_fail,E0423
/// use cantos_ui::DisabledReason;
/// let reason = DisabledReason(String::new());
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DisabledReason(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyDisabledReason;

impl DisabledReason {
    pub fn new(text: impl Into<String>) -> Result<Self, EmptyDisabledReason> {
        let text = text.into();
        if text.trim().is_empty() {
            Err(EmptyDisabledReason)
        } else {
            Ok(Self(text))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A localized accessible name for a control that has no visible text label.
/// ```compile_fail,E0277
/// use cantos_ui::AccessibleName;
/// let name: AccessibleName = Default::default();
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccessibleName(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EmptyAccessibleName;

impl AccessibleName {
    pub fn new(text: impl Into<String>) -> Result<Self, EmptyAccessibleName> {
        let text = text.into();
        if text.trim().is_empty() {
            Err(EmptyAccessibleName)
        } else {
            Ok(Self(text))
        }
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Disabled controls always carry a visible explanation, including when keyboard focus skips them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ButtonState {
    #[default]
    Enabled,
    Disabled(DisabledReason),
    /// The localized explanation remains visible while repeated activation is blocked.
    Loading(DisabledReason),
}

impl ButtonState {
    /// Shared by native disabled attributes and event dispatch, including synthetic events.
    pub const fn can_press(&self) -> bool {
        match self {
            Self::Enabled => true,
            Self::Disabled(_) | Self::Loading(_) => false,
        }
    }

    pub const fn is_busy(&self) -> bool {
        match self {
            Self::Loading(_) => true,
            Self::Enabled | Self::Disabled(_) => false,
        }
    }

    pub const fn reason(&self) -> Option<&DisabledReason> {
        match self {
            Self::Enabled => None,
            Self::Disabled(reason) | Self::Loading(reason) => Some(reason),
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled(_) => "disabled",
            Self::Loading(_) => "loading",
        }
    }
}

pub(crate) fn component_class(base: &str, extra: &str) -> String {
    if extra.trim().is_empty() {
        base.to_owned()
    } else {
        format!("{base} {}", extra.trim())
    }
}

pub(crate) fn description_ids(extra: &str, reason_id: Option<&str>) -> Option<String> {
    let mut ids: Vec<&str> = extra.split_whitespace().collect();
    if let Some(reason_id) = reason_id {
        if !ids.contains(&reason_id) {
            ids.push(reason_id);
        }
    }
    (!ids.is_empty()).then(|| ids.join(" "))
}

/// Native non-submit action. Caller supplies a unique stable ID and visible localized children.
/// Children provide the label; use `icon` for a decorative leading icon outside its text box.
/// State/effects remain owned by the caller.
#[component]
pub fn Button(
    #[prop(into)] id: String,
    children: Children,
    #[prop(optional)] variant: ButtonVariant,
    #[prop(optional)] icon: Option<IconName>,
    #[prop(into, default = ButtonState::Enabled.into())] state: Signal<ButtonState>,
    #[prop(optional)] on_press: Option<Callback<MouseEvent>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    /// Additional description IDs. The visible reason is appended automatically when blocked.
    #[prop(into, default = String::new().into())]
    described_by: Signal<String>,
    #[prop(optional, into)] aria_controls: Option<String>,
    #[prop(optional)] aria_expanded: Option<Signal<bool>>,
    /// Opt in for a dialog trigger: Safari pointer activation does not focus buttons.
    /// Focus before dispatch lets the native modal capture a real return target.
    #[prop(optional)]
    focus_on_press: bool,
) -> impl IntoView {
    let reason_id = format!("{id}-reason");
    let reason_id_for_description = reason_id.clone();
    let button_ref = NodeRef::<leptos::html::Button>::new();
    #[cfg(target_arch = "wasm32")]
    if matches!(
        variant,
        ButtonVariant::AuthPrimary | ButtonVariant::AuthSecondary | ButtonVariant::AuthOutlined
    ) {
        auth_geometry::bind(button_ref);
    }
    view! {
        <div class="cui-button-field">
            <button id=id node_ref=button_ref type="button" class=move || component_class("cui-button", &class.get())
                data-variant=variant.as_str() data-state=move || state.get().as_str()
                disabled=move || !state.get().can_press()
                aria-busy=move || state.get().is_busy().then_some("true")
                aria-controls=aria_controls
                aria-expanded=move || aria_expanded.map(|expanded| expanded.get().to_string())
                aria-describedby=move || {
                    description_ids(&described_by.get(), state.get().reason().map(|_| reason_id_for_description.as_str()))
                }
                on:click=move |event| {
                    // Guard synthetic events as well as the native disabled interaction.
                    if state.get_untracked().can_press() {
                        #[cfg(target_arch = "wasm32")]
                        if focus_on_press {
                            use leptos::wasm_bindgen::JsCast;
                            if let Some(button) = event.current_target().and_then(|target| target.dyn_into::<leptos::web_sys::HtmlElement>().ok()) {
                                let _ = button.focus();
                            }
                        }
                        #[cfg(not(target_arch = "wasm32"))]
                        let _ = focus_on_press;
                        if let Some(callback) = on_press { callback.run(event); }
                    }
                }>
                {move || if state.get().is_busy() {
                    Some(view! { <span class="cui-button-icon" aria-hidden="true"><LoadingIndicator/></span> }.into_any())
                } else {
                    icon.map(|name| view! { <span class="cui-button-icon" aria-hidden="true"><Icon name=name/></span> }.into_any())
                }}
                <span class="cui-control-label">{children()}</span>
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || state.get().can_press()
                role=move || state.get().is_busy().then_some("status")>
                {move || state.get().reason().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
            </span>
        </div>
    }
}

#[cfg(target_arch = "wasm32")]
mod auth_geometry {
    use leptos::{
        html,
        prelude::*,
        wasm_bindgen::{closure::Closure, JsCast},
    };
    use web_sys::ResizeObserver;

    /// CSS clamps an oversized pill radius only when painting, after interpolation.
    /// Measure the actual capsule so every transition frame responds to input. Resize
    /// observation also covers wrapping, font scaling and hidden gallery sections.
    pub(super) fn bind(button_ref: NodeRef<html::Button>) {
        let binding = StoredValue::new_local(None::<(ResizeObserver, Closure<dyn FnMut()>)>);
        Effect::new(move |_| {
            let Some(button) = button_ref.get() else {
                return;
            };
            binding.update_value(|current| {
                if let Some((observer, _)) = current.take() {
                    observer.disconnect();
                }
            });
            let measured_button = button.clone();
            let measure = move || {
                let bounds = measured_button.get_bounding_client_rect();
                let radius = bounds.width().min(bounds.height()) / 2.0;
                // Hidden nodes retain their last usable value until they are visible.
                if radius > 0.0 {
                    let _ = web_sys::HtmlElement::style(&measured_button)
                        .set_property("--cantos-web-auth-capsule", &format!("{radius}px"));
                }
            };
            measure();
            let callback = Closure::wrap(Box::new(measure) as Box<dyn FnMut()>);
            if let Ok(observer) = ResizeObserver::new(callback.as_ref().unchecked_ref()) {
                observer.observe(&button);
                binding.set_value(Some((observer, callback)));
            }
        });
        on_cleanup(move || {
            binding.try_update_value(|current| {
                if let Some((observer, _)) = current.take() {
                    observer.disconnect();
                }
            });
        });
    }
}

/// An icon-only native action. A non-empty localized name is required and can change reactively.
/// The icon remains decorative; the name is text in the button's accessible content.
#[component]
pub fn IconButton(
    #[prop(into)] id: String,
    #[prop(into)] name: Signal<AccessibleName>,
    /// A signal lets a stateful action (the password eye) swap its glyph in place: the native
    /// button, its focus and its caller's listeners are never remounted.
    #[prop(into)]
    icon: Signal<IconName>,
    #[prop(default = ButtonVariant::Text)] variant: ButtonVariant,
    #[prop(into, default = ButtonState::Enabled.into())] state: Signal<ButtonState>,
    #[prop(optional)] on_press: Option<Callback<MouseEvent>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
    #[prop(optional, into)] aria_controls: Option<String>,
    #[prop(optional)] aria_expanded: Option<Signal<bool>>,
    /// Native submit for an app-owned `<form>`, such as a docked search action: the form keeps
    /// Enter, composition, validation and history. A blocked state still prevents submission.
    #[prop(optional)]
    submit: bool,
    /// Explicit tooltip text. If omitted, defaults to the button's accessible name.
    #[prop(optional, into)]
    tooltip: Option<Signal<String>>,
) -> impl IntoView {
    let reason_id = format!("{id}-reason");
    let reason_id_for_description = reason_id.clone();
    let tooltip_id = format!("{id}-tooltip");
    let tooltip_id_for_description = tooltip_id.clone();
    let tooltip_id_for_span = tooltip_id.clone();
    let tooltip_open = RwSignal::new(false);
    let tooltip_text =
        tooltip.unwrap_or_else(|| Signal::derive(move || name.get().as_str().to_owned()));

    view! {
        <div class="cui-button-field">
            <button id=id type=if submit { "submit" } else { "button" } class=move || component_class("cui-button cui-icon-button", &class.get())
                title=move || name.get().as_str().to_owned()
                data-variant=variant.as_str() data-state=move || state.get().as_str()
                disabled=move || !state.get().can_press()
                aria-busy=move || state.get().is_busy().then_some("true")
                aria-controls=aria_controls
                aria-expanded=move || aria_expanded.map(|expanded| expanded.get().to_string())
                aria-describedby=move || {
                    let desc = description_ids(&described_by.get(), state.get().reason().map(|_| reason_id_for_description.as_str()));
                    if tooltip_open.get() {
                        match desc {
                            Some(d) => Some(format!("{d} {tooltip_id_for_description}")),
                            None => Some(tooltip_id_for_description.clone()),
                        }
                    } else {
                        desc
                    }
                }
                on:mouseenter=move |_| tooltip_open.set(true)
                on:mouseleave=move |_| tooltip_open.set(false)
                on:focus=move |_| tooltip_open.set(true)
                on:blur=move |_| tooltip_open.set(false)
                on:keydown=move |event: ev::KeyboardEvent| {
                    if event.key() == "Escape" && tooltip_open.get_untracked() {
                        event.stop_propagation();
                        tooltip_open.set(false);
                    }
                }
                on:click=move |event| {
                    if state.get_untracked().can_press() {
                        if let Some(callback) = on_press { callback.run(event); }
                    } else {
                        // A synthetic click must not submit a form from a blocked action.
                        event.prevent_default();
                    }
                }>
                {move || if state.get().is_busy() {
                    view! { <LoadingIndicator/> }.into_any()
                } else {
                    view! { <Icon name=icon.get()/> }.into_any()
                }}
                <span class="cui-sr-only">{move || name.get().as_str().to_owned()}</span>
            </button>
            <span id=tooltip_id_for_span
                role="tooltip"
                class="cui-tooltip"
                data-state=move || if tooltip_open.get() { "visible" } else { "hidden" }
                hidden=move || !tooltip_open.get()
                on:mouseenter=move |_| tooltip_open.set(true)
                on:mouseleave=move |_| tooltip_open.set(false)>
                {move || tooltip_text.get()}
            </span>
            <span id=reason_id class="cui-button-reason" hidden=move || state.get().can_press()
                role=move || state.get().is_busy().then_some("status")>
                {move || state.get().reason().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
            </span>
        </div>
    }
}
