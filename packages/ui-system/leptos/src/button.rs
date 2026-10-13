use leptos::{ev::MouseEvent, prelude::*};

use crate::{Icon, IconName};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Tonal,
    Secondary,
    Text,
    Danger,
}

impl ButtonVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Tonal => "tonal",
            Self::Secondary => "secondary",
            Self::Text => "text",
            Self::Danger => "danger",
        }
    }
}

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
        matches!(self, Self::Enabled)
    }

    pub const fn is_busy(&self) -> bool {
        matches!(self, Self::Loading(_))
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
    view! {
        <div class="cui-button-field">
            <button id=id type="button" class=move || component_class("cui-button", &class.get())
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
                {icon.map(|name| view! { <Icon name=name/> })}
                <span class="cui-control-label">{children()}</span>
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || state.get().can_press()
                role=move || state.get().is_busy().then_some("status")>
                {move || state.get().reason().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
            </span>
        </div>
    }
}

/// An icon-only native action. A non-empty localized name is required and can change reactively.
/// The icon remains decorative; the name is text in the button's accessible content.
#[component]
pub fn IconButton(
    #[prop(into)] id: String,
    #[prop(into)] name: Signal<AccessibleName>,
    icon: IconName,
    #[prop(default = ButtonVariant::Text)] variant: ButtonVariant,
    #[prop(into, default = ButtonState::Enabled.into())] state: Signal<ButtonState>,
    #[prop(optional)] on_press: Option<Callback<MouseEvent>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
    #[prop(optional, into)] aria_controls: Option<String>,
    #[prop(optional)] aria_expanded: Option<Signal<bool>>,
) -> impl IntoView {
    let reason_id = format!("{id}-reason");
    let reason_id_for_description = reason_id.clone();
    view! {
        <div class="cui-button-field">
            <button id=id type="button" class=move || component_class("cui-button cui-icon-button", &class.get())
                data-variant=variant.as_str() data-state=move || state.get().as_str()
                disabled=move || !state.get().can_press()
                aria-busy=move || state.get().is_busy().then_some("true")
                aria-controls=aria_controls
                aria-expanded=move || aria_expanded.map(|expanded| expanded.get().to_string())
                aria-describedby=move || {
                    description_ids(&described_by.get(), state.get().reason().map(|_| reason_id_for_description.as_str()))
                }
                on:click=move |event| {
                    if state.get_untracked().can_press() {
                        if let Some(callback) = on_press { callback.run(event); }
                    }
                }>
                <Icon name=icon/>
                <span class="cui-sr-only">{move || name.get().as_str().to_owned()}</span>
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || state.get().can_press()
                role=move || state.get().is_busy().then_some("status")>
                {move || state.get().reason().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
            </span>
        </div>
    }
}
