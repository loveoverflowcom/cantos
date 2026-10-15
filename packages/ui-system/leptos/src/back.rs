use leptos::{ev::MouseEvent, prelude::*};

use crate::{
    button::component_class, AccessibleName, Button, ButtonState, ButtonVariant, IconName,
};

/// Contextual native Back action, not a router, logout action or browser-history policy.
/// The host must provide both a localized nonblank name and the return intent callback.
/// `icon_only` hides only the visible text; the same name remains accessible.
///
/// ```compile_fail
/// use cantos_ui::{AccessibleName, BackButton};
/// use leptos::prelude::*;
/// // A name alone cannot silently create an inert or history-owning Back action.
/// let _ = view! { <BackButton id="back" name=AccessibleName::new("Về Trang chủ").unwrap()/> };
/// ```
#[component]
pub fn BackButton(
    #[prop(into)] id: String,
    #[prop(into)] name: Signal<AccessibleName>,
    on_back: Callback<MouseEvent>,
    #[prop(optional)] icon_only: bool,
    #[prop(into, default = ButtonState::Enabled.into())] state: Signal<ButtonState>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
) -> impl IntoView {
    let base = if icon_only {
        "cui-back-button cui-back-button-icon-only"
    } else {
        "cui-back-button"
    };
    view! {
        <Button id=id variant=ButtonVariant::Text icon=IconName::ArrowBack
            state=state on_press=on_back described_by=described_by
            class=Signal::derive(move || component_class(base, &class.get()))>
            <span class=if icon_only { "cui-sr-only" } else { "cui-back-button-label" }>
                {move || name.get().as_str().to_owned()}
            </span>
        </Button>
    }
}
