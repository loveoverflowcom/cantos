//! Web presentation of the existing Select contract. Native option children remain unchanged.
mod model;
#[cfg(target_arch = "wasm32")]
mod platform;

use crate::{
    fields::{descriptions, error_reference, has_text, FieldMessages},
    ChoiceState, FieldLabel, Icon, IconName, SelectDensity, SelectLabelPlacement,
};
use leptos::{ev, html, prelude::*};
use model::{Intent, MenuState};

/// Localized immutable option facts. Values stay opaque and caller-owned.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SelectOption {
    pub value: String,
    pub label: String,
    pub disabled: bool,
    pub lang: Option<String>,
    /// Optional decorative semantic glyph from the shared registry; never part of the value.
    pub icon: Option<IconName>,
}

impl SelectOption {
    pub fn new(value: impl Into<String>, label: impl Into<String>) -> Self {
        Self {
            value: value.into(),
            label: label.into(),
            disabled: false,
            lang: None,
            icon: None,
        }
    }

    pub fn with_icon(self, icon: IconName) -> Self {
        Self {
            icon: Some(icon),
            ..self
        }
    }
}

#[component]
pub(super) fn MenuSelect(
    id: String,
    label: Signal<String>,
    value: Signal<String>,
    on_input: Callback<String>,
    options: Signal<Vec<SelectOption>>,
    placeholder: Signal<String>,
    state: Signal<ChoiceState>,
    help: Signal<Option<String>>,
    error: Signal<Option<String>>,
    described_by: Signal<Option<String>>,
    class: Signal<String>,
    name: Option<String>,
    required: bool,
    density: SelectDensity,
    label_placement: SelectLabelPlacement,
) -> impl IntoView {
    let reason = Signal::derive(move || state.get().reason().cloned());
    let descriptions = descriptions(id.clone(), described_by, help, error, reason);
    let error_reference = error_reference(id.clone(), error);
    let native_ref = NodeRef::<html::Select>::new();
    let trigger_ref = NodeRef::<html::Button>::new();
    let popup_ref = NodeRef::<html::Div>::new();
    // False for SSR and unsupported browsers. Only a successful feature probe enhances the field.
    let enhanced = RwSignal::new(false);
    let menu = RwSignal::new(MenuState::default());
    let ids = StoredValue::new(id.clone());
    let trigger_id = format!("{id}-trigger");
    let popup_id = format!("{id}-listbox");
    let label_id = format!("{id}-label");
    let value_id = format!("{id}-value");
    let trigger_label = format!("{label_id} {value_id}");
    let label_for = Signal::derive(move || {
        let id = ids.get_value();
        if enhanced.get() {
            format!("{id}-trigger")
        } else {
            id
        }
    });
    let current_label = Signal::derive(move || {
        options
            .get()
            .into_iter()
            .find(|option| option.value == value.get())
            .map(|option| option.label)
            .unwrap_or_else(|| placeholder.get())
    });
    let current_lang = Signal::derive(move || {
        options
            .get()
            .into_iter()
            .find(|option| option.value == value.get())
            .and_then(|option| option.lang)
    });
    let has_icons =
        Signal::derive(move || options.get().iter().any(|option| option.icon.is_some()));
    let current_icon = Signal::derive(move || {
        options
            .get()
            .into_iter()
            .find(|option| option.value == value.get())
            .and_then(|option| option.icon)
    });
    let dispatch = Callback::new(move |intent| {
        if !state.get_untracked().accepts_input() {
            return;
        }
        let mut next = menu.get_untracked();
        let committed = next.apply(intent, &options.get_untracked(), &value.get_untracked());
        menu.set(next);
        if let Some(selected) = committed.filter(|selected| *selected != value.get_untracked()) {
            on_input.run(selected);
        }
    });
    Effect::new(move |_| {
        let current = value.get();
        options.track();
        state.track();
        // External facts can invalidate an active index. Cancel browsing, never acknowledge it.
        menu.set(MenuState::default());
        if let Some(native) = native_ref.get() {
            native.set_value(&current);
        }
    });
    #[cfg(target_arch = "wasm32")]
    platform::bind(trigger_ref, popup_ref, menu, enhanced);

    let popup = view! {
        <div id=popup_id.clone() node_ref=popup_ref class="cui-select-menu" popover="auto"
            role="listbox" aria-labelledby=label_id.clone()
            on:mousedown=move |event| event.prevent_default()>
            {move || options.get().into_iter().enumerate().map(|(index, option)| {
                let option_id = format!("{}-option-{index}", ids.get_value());
                let selected_value = option.value.clone();
                let clicked_value = option.value.clone();
                view! {
                    <div id=option_id class="cui-select-option" role="option"
                        aria-selected=move || (value.get() == selected_value).to_string()
                        aria-disabled=option.disabled.to_string()
                        data-active=move || (menu.get().active == Some(index)).to_string()
                        lang=option.lang
                        on:click=move |_| {
                            if state.get_untracked().accepts_input() && !option.disabled {
                                menu.set(MenuState::default());
                                #[cfg(target_arch = "wasm32")]
                                platform::focus(trigger_ref);
                                if value.get_untracked() != clicked_value { on_input.run(clicked_value.clone()); }
                            }
                        }>
                        <Show when=move || has_icons.get()>
                            <span class="cui-select-leading" aria-hidden="true">{option.icon.map(|name| view! { <Icon name/> })}</span>
                        </Show>
                        <span class="cui-select-option-label">{option.label}</span>
                        <span class="cui-select-option-check" aria-hidden="true"><Icon name=IconName::Check/></span>
                    </div>
                }
            }).collect_view()}
        </div>
    }.add_any_attr(ev::on(ev::Custom::<leptos::web_sys::Event>::new("toggle"), move |_| {
        #[cfg(target_arch = "wasm32")]
        if !platform::is_open(popup_ref) { menu.set(MenuState::default()); }
    }));

    view! {
        <div class=move || format!("cui-field cui-select-field {}", class.get()) data-select-density=density.as_str()
            data-label-placement=label_placement.as_str() data-state=move || state.get().as_str()
            data-select-presentation=move || if enhanced.get() { "menu" } else { "native-fallback" }>
            <FieldLabel for_id=label_for><span id=label_id.clone()>{move || label.get()}</span></FieldLabel>
            <div class="cui-select">
                <select id=id.clone() node_ref=native_ref class="cui-field-control" name=name required=required
                    hidden=move || enhanced.get() tabindex=move || enhanced.get().then_some(-1)
                    aria-hidden=move || enhanced.get().then_some("true")
                    disabled=move || !state.get().accepts_input()
                    aria-busy=move || state.get().is_loading().to_string()
                    aria-invalid=move || has_text(&error.get()).to_string()
                    aria-describedby=move || descriptions.get()
                    aria-errormessage=move || error_reference.get()
                    prop:value=move || value.get()
                    on:invalid=move |event| {
                        if enhanced.get_untracked() {
                            event.prevent_default();
                            #[cfg(target_arch = "wasm32")]
                            platform::focus(trigger_ref);
                        }
                    }
                    on:change=move |event| {
                        if state.get_untracked().accepts_input() { on_input.run(event_target_value(&event)); }
                    }>
                    {move || options.get().into_iter().map(|option| {
                        let selected = option.value == value.get();
                        view! { <option value=option.value disabled=option.disabled selected=selected lang=option.lang>{option.label}</option> }
                    }).collect_view()}
                </select>
                <button id=trigger_id node_ref=trigger_ref type="button" class="cui-field-control cui-select-trigger"
                    role="combobox" aria-haspopup="listbox" aria-controls=popup_id
                    aria-labelledby=trigger_label
                    aria-expanded=move || menu.get().open.to_string()
                    aria-activedescendant=move || {
                        let current = menu.get();
                        current.open.then_some(current.active).flatten()
                            .map(|index| format!("{}-option-{index}", ids.get_value()))
                    }
                    aria-required=required.to_string()
                    aria-busy=move || state.get().is_loading().to_string()
                    aria-invalid=move || has_text(&error.get()).to_string()
                    aria-describedby=move || descriptions.get()
                    aria-errormessage=move || error_reference.get()
                    hidden=move || !enhanced.get()
                    disabled=move || !state.get().accepts_input()
                    on:click=move |_| {
                        #[cfg(target_arch = "wasm32")]
                        platform::focus(trigger_ref);
                        dispatch.run(Intent::Toggle);
                    }
                    on:blur=move |_| { if menu.get_untracked().open { dispatch.run(Intent::Accept); } }
                    on:keydown=move |event| {
                        if !state.get_untracked().accepts_input() || event.is_composing() || event.ctrl_key() || event.meta_key() { return; }
                        let key = event.key();
                        if event.alt_key() && !matches!(key.as_str(), "ArrowDown" | "ArrowUp") { return; }
                        let intent = match key.as_str() {
                            "ArrowDown" => Some(Intent::Next),
                            "ArrowUp" if event.alt_key() && menu.get_untracked().open => Some(Intent::Accept),
                            "ArrowUp" => Some(Intent::Previous),
                            "Home" => Some(Intent::First),
                            "End" => Some(Intent::Last),
                            "Enter" | " " => Some(if menu.get_untracked().open { Intent::Accept } else { Intent::Toggle }),
                            "Escape" if menu.get_untracked().open => Some(Intent::Dismiss),
                            "Tab" if menu.get_untracked().open => { dispatch.run(Intent::Accept); return; }
                            _ => None,
                        };
                        if let Some(intent) = intent {
                            event.prevent_default();
                            dispatch.run(intent);
                        } else if !event.alt_key() && key.chars().count() == 1 {
                            event.prevent_default();
                            if let Some(character) = key.chars().next() {
                                let mut next = menu.get_untracked();
                                next.type_character(character, event.time_stamp(), &options.get_untracked(), &value.get_untracked());
                                menu.set(next);
                            }
                        }
                    }>
                    <Show when=move || has_icons.get()>
                        <span class="cui-select-leading" aria-hidden="true">{move || current_icon.get().map(|name| view! { <Icon name/> })}</span>
                    </Show>
                    <span id=value_id class="cui-select-value" lang=move || current_lang.get()>{move || current_label.get()}</span>
                    {move || if state.get().is_loading() {
                        view! { <span class="cui-action-progress" aria-hidden="true"></span> }.into_any()
                    } else { view! { <Icon name=IconName::ExpandMore/> }.into_any() }}
                </button>
                <span class="cui-select-native-indicator" hidden=move || enhanced.get()>
                    {move || if state.get().is_loading() {
                        view! { <span class="cui-action-progress" aria-hidden="true"></span> }.into_any()
                    } else { view! { <Icon name=IconName::ExpandMore/> }.into_any() }}
                </span>
            </div>
            {popup}
            <FieldMessages id help error reason/>
        </div>
    }
}
