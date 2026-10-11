use leptos::{ev::MouseEvent, prelude::*};

use crate::{
    button::{component_class, description_ids},
    AccessibleName, ButtonState, Icon, IconName,
};

/// A native navigation destination. The caller owns routes and unsaved-navigation guards.
/// Relative routes, fragments and HTTP(S), mailto or tel links are supported.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkHref(String);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InvalidLinkHref {
    Empty,
    ControlCharacter,
    UnsupportedScheme,
}

impl LinkHref {
    pub fn new(value: impl Into<String>) -> Result<Self, InvalidLinkHref> {
        let value = value.into();
        if value.chars().any(char::is_control) {
            return Err(InvalidLinkHref::ControlCharacter);
        }
        let value = value.trim();
        if value.is_empty() {
            return Err(InvalidLinkHref::Empty);
        }
        if let Some(colon) = value.find(':') {
            let path_start = value.find(['/', '?', '#']);
            if path_start.is_none_or(|path_start| colon < path_start) {
                let scheme = &value[..colon];
                if !["http", "https", "mailto", "tel"]
                    .iter()
                    .any(|supported| scheme.eq_ignore_ascii_case(supported))
                {
                    return Err(InvalidLinkHref::UnsupportedScheme);
                }
            }
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// A native link: browser focus, activation, copy-link and modifier-key navigation remain intact.
/// `on_press` may prevent default for an app-owned navigation guard; it does not replace `href`.
/// There is no label override: visible children provide the accessible name.
#[component]
pub fn Link(
    #[prop(into)] id: String,
    #[prop(into)] href: Signal<LinkHref>,
    children: Children,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
    #[prop(optional)] on_press: Option<Callback<MouseEvent>>,
) -> impl IntoView {
    view! {
        <a id=id href=move || href.get().0
            class=move || component_class("cui-link", &class.get())
            aria-describedby=move || description_ids(&described_by.get(), None)
            on:click=move |event| {
                if let Some(callback) = on_press { callback.run(event); }
            }>
            {children()}
        </a>
    }
}

/// An app-owned navigation destination with an explicit current-page state.
#[component]
pub fn NavItem(
    #[prop(into)] id: String,
    #[prop(into)] href: Signal<LinkHref>,
    children: Children,
    #[prop(into, default = false.into())] current: Signal<bool>,
    #[prop(optional)] icon: Option<IconName>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
    #[prop(optional)] on_press: Option<Callback<MouseEvent>>,
) -> impl IntoView {
    view! {
        <a id=id href=move || href.get().0
            class=move || component_class("cui-nav-item", &class.get())
            aria-current=move || current.get().then_some("page")
            aria-describedby=move || description_ids(&described_by.get(), None)
            on:click=move |event| {
                if let Some(callback) = on_press { callback.run(event); }
            }>
            {icon.map(|name| view! { <Icon name=name/> })}
            <span class="cui-nav-label">{children()}</span>
        </a>
    }
}

/// A labelled navigation strip, composed with `NavItem` links.
/// Despite the visual tab shape, this is ordinary page navigation, not an ARIA tabs widget.
/// It uses native Tab/Enter navigation and does not promise arrow-key panel selection.
#[component]
pub fn NavTabs(
    label: AccessibleName,
    children: Children,
    #[prop(into, default = String::new().into())] class: Signal<String>,
) -> impl IntoView {
    view! {
        <nav aria-label=label.as_str().to_owned() class=move || component_class("cui-nav-tabs", &class.get())>
            {children()}
        </nav>
    }
}

fn requested_selection(state: &ButtonState, current: bool) -> Option<bool> {
    state.can_press().then_some(!current)
}

/// A controlled toggle button. The callback requests a new selection; the caller updates it.
/// The visible label stays stable while `aria-pressed` describes the selected state.
#[component]
pub fn FilterChip(
    #[prop(into)] id: String,
    children: Children,
    #[prop(into)] selected: Signal<bool>,
    on_change: Callback<bool>,
    #[prop(into, default = ButtonState::Enabled.into())] state: Signal<ButtonState>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, default = String::new().into())] described_by: Signal<String>,
) -> impl IntoView {
    let reason_id = format!("{id}-reason");
    let reason_id_for_description = reason_id.clone();
    view! {
        <div class="cui-button-field">
            <button id=id type="button" class=move || component_class("cui-filter-chip", &class.get())
                aria-pressed=move || selected.get().to_string()
                data-state=move || state.get().as_str()
                disabled=move || !state.get().can_press()
                aria-busy=move || state.get().is_busy().then_some("true")
                aria-describedby=move || {
                    description_ids(&described_by.get(), state.get().reason().map(|_| reason_id_for_description.as_str()))
                }
                on:click=move |_| {
                    if let Some(next) = requested_selection(&state.get_untracked(), selected.get_untracked()) {
                        on_change.run(next);
                    }
                }>
                <span class="cui-chip-check" aria-hidden="true"><Icon name=IconName::Check/></span>
                {children()}
            </button>
            <span id=reason_id class="cui-button-reason" hidden=move || state.get().can_press()
                role=move || state.get().is_busy().then_some("status")>
                {move || state.get().reason().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
            </span>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::requested_selection;
    use crate::{ButtonState, DisabledReason};

    #[test]
    fn chip_requests_opposite_value_only_when_enabled() {
        let reason = DisabledReason::new("Không thể thay đổi bộ lọc lúc này").unwrap();
        for current in [false, true] {
            assert_eq!(
                requested_selection(&ButtonState::Enabled, current),
                Some(!current)
            );
            assert_eq!(
                requested_selection(&ButtonState::Disabled(reason.clone()), current),
                None
            );
            assert_eq!(
                requested_selection(&ButtonState::Loading(reason.clone()), current),
                None
            );
        }
    }
}
