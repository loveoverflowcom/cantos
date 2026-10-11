use leptos::{ev::MouseEvent, prelude::*};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ButtonVariant {
    #[default]
    Primary,
    Tonal,
    Text,
}

impl ButtonVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Primary => "primary",
            Self::Tonal => "tonal",
            Self::Text => "text",
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

/// Disabled controls always carry a visible explanation, including when keyboard focus skips them.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ButtonState {
    #[default]
    Enabled,
    Disabled(DisabledReason),
}

/// Native non-submit action. Caller supplies a unique stable ID and visible localized children.
/// Children may compose decorative Icons. State/effects remain owned by the caller.
#[component]
pub fn Button(
    #[prop(into)] id: String,
    children: Children,
    #[prop(optional)] variant: ButtonVariant,
    #[prop(into, default = ButtonState::Enabled.into())] state: Signal<ButtonState>,
    #[prop(optional)] on_press: Option<Callback<MouseEvent>>,
) -> impl IntoView {
    let reason_id = format!("{id}-reason");
    let described_by = reason_id.clone();
    view! {
        <div class="cui-button-field">
            <button id=id type="button" class="cui-button" data-variant=variant.as_str()
                disabled=move || matches!(state.get(), ButtonState::Disabled(_))
                aria-describedby=move || match state.get() {
                    ButtonState::Enabled => None,
                    ButtonState::Disabled(_) => Some(described_by.clone()),
                }
                on:click=move |event| {
                    // Guard synthetic events as well as the native disabled interaction.
                    if matches!(state.get_untracked(), ButtonState::Enabled) {
                        if let Some(callback) = on_press { callback.run(event); }
                    }
                }>
                {children()}
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || matches!(state.get(), ButtonState::Enabled)>
                {move || match state.get() {
                    ButtonState::Enabled => String::new(),
                    ButtonState::Disabled(reason) => reason.0,
                }}
            </span>
        </div>
    }
}
