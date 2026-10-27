//! Native controlled fields. Apps own values, validation, localization and durable decisions.

use crate::{DisabledReason, Icon, IconName};
use leptos::{ev::CompositionEvent, prelude::*};

/// Text entry supports native read-only selection/copy without disabling the field.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum FieldState {
    #[default]
    Editable,
    ReadOnly,
    Disabled(DisabledReason),
    Loading(DisabledReason),
}

impl FieldState {
    pub const fn accepts_input(&self) -> bool {
        match self {
            Self::Editable => true,
            Self::ReadOnly | Self::Disabled(_) | Self::Loading(_) => false,
        }
    }

    pub const fn is_disabled(&self) -> bool {
        match self {
            Self::Disabled(_) | Self::Loading(_) => true,
            Self::Editable | Self::ReadOnly => false,
        }
    }

    pub const fn is_loading(&self) -> bool {
        match self {
            Self::Loading(_) => true,
            Self::Editable | Self::ReadOnly | Self::Disabled(_) => false,
        }
    }

    /// Native text selection/copy remains available; this is not disabled input.
    pub const fn is_read_only(&self) -> bool {
        match self {
            Self::ReadOnly => true,
            Self::Editable | Self::Disabled(_) | Self::Loading(_) => false,
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Editable => "editable",
            Self::ReadOnly => "readonly",
            Self::Disabled(_) => "disabled",
            Self::Loading(_) => "loading",
        }
    }

    pub fn reason(&self) -> Option<&DisabledReason> {
        match self {
            Self::Disabled(reason) | Self::Loading(reason) => Some(reason),
            Self::Editable | Self::ReadOnly => None,
        }
    }
}

/// Native select, checked and range controls do not support a read-only attribute.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ChoiceState {
    #[default]
    Enabled,
    Disabled(DisabledReason),
    Loading(DisabledReason),
}

impl ChoiceState {
    pub const fn accepts_input(&self) -> bool {
        match self {
            Self::Enabled => true,
            Self::Disabled(_) | Self::Loading(_) => false,
        }
    }

    pub const fn is_loading(&self) -> bool {
        match self {
            Self::Loading(_) => true,
            Self::Enabled | Self::Disabled(_) => false,
        }
    }

    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::Enabled => "enabled",
            Self::Disabled(_) => "disabled",
            Self::Loading(_) => "loading",
        }
    }

    pub fn reason(&self) -> Option<&DisabledReason> {
        match self {
            Self::Disabled(reason) | Self::Loading(reason) => Some(reason),
            Self::Enabled => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextFieldKind {
    #[default]
    Text,
    Search,
    Email,
    Password,
    Url,
    Telephone,
}

impl TextFieldKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Search => "search",
            Self::Email => "email",
            Self::Password => "password",
            Self::Url => "url",
            Self::Telephone => "tel",
        }
    }
}

/// Geometry only; values, validation and composition behavior stay caller-controlled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TextFieldVariant {
    #[default]
    Regular,
    AuthTonal,
    /// Web spacing candidate, superseded by `Floating` (Fields v3); kept for existing previews.
    AuthTonalSpacious,
    /// Fields v3: the label lives inside one shell that owns fill and state stroke.
    /// It rests as the placeholder and floats while focused or holding any value. Use an
    /// external label (`Regular`) for editors and labels too long to share a 64-unit shell.
    Floating,
    /// Preview candidate: Floating anatomy with a tonal, borderless auth treatment.
    FloatingTonal,
}

impl TextFieldVariant {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::AuthTonal => "auth-tonal",
            Self::AuthTonalSpacious => "auth-tonal-spacious",
            Self::Floating | Self::FloatingTonal => "floating",
        }
    }

    pub const fn has_floating_label(self) -> bool {
        match self {
            Self::Floating | Self::FloatingTonal => true,
            Self::Regular | Self::AuthTonal | Self::AuthTonalSpacious => false,
        }
    }

    /// A trailing action lives inside the one shell, so only the shell variant can host it.
    /// Other variants render no slot rather than paint a control outside their painted surface.
    pub const fn supports_trailing(self) -> bool {
        self.has_floating_label()
    }
}

/// Floating labels follow the browser's real value, not the Rust signal: composition text,
/// autofill and restored values all clear `:placeholder-shown`. A one-space placeholder keeps
/// that pseudo-class meaningful when the caller supplies none; an empty string would not.
fn floating_placeholder(placeholder: Option<String>) -> String {
    placeholder
        .filter(|text| !text.is_empty())
        .unwrap_or_else(|| " ".to_owned())
}

/// Label primitive for an app-composed native control with the same stable ID.
#[component]
pub fn FieldLabel(
    #[prop(into)] for_id: Signal<String>,
    children: Children,
    #[prop(into, default = String::new().into())] class: Signal<String>,
) -> impl IntoView {
    view! { <label for=move || for_id.get() class=move || format!("cui-field-label {}", class.get())>{children()}</label> }
}

#[component]
pub fn FieldHelp(
    #[prop(into)] id: String,
    children: Children,
    #[prop(into, default = String::new().into())] class: Signal<String>,
) -> impl IntoView {
    view! { <p id=id class=move || format!("cui-field-help {}", class.get())>{children()}</p> }
}

/// Inline error text. The app chooses when to announce validation; typing is not a live region.
#[component]
pub fn FieldError(
    #[prop(into)] id: String,
    children: Children,
    #[prop(into, default = String::new().into())] class: Signal<String>,
) -> impl IntoView {
    view! { <p id=id class=move || format!("cui-field-error {}", class.get())>{children()}</p> }
}

pub(super) fn has_text(text: &Option<String>) -> bool {
    text.as_ref().is_some_and(|text| !text.trim().is_empty())
}

fn described_by(
    id: &str,
    external: &Option<String>,
    help: bool,
    error: bool,
    reason: bool,
) -> Option<String> {
    let mut ids: Vec<String> = external
        .as_deref()
        .unwrap_or_default()
        .split_whitespace()
        .map(str::to_owned)
        .collect();
    for (visible, suffix) in [(help, "help"), (error, "error"), (reason, "reason")] {
        if visible {
            let message_id = format!("{id}-{suffix}");
            if !ids.contains(&message_id) {
                ids.push(message_id);
            }
        }
    }
    if ids.is_empty() {
        None
    } else {
        Some(ids.join(" "))
    }
}

pub(super) fn descriptions(
    id: String,
    external: Signal<Option<String>>,
    help: Signal<Option<String>>,
    error: Signal<Option<String>>,
    reason: Signal<Option<DisabledReason>>,
) -> Signal<Option<String>> {
    Signal::derive(move || {
        described_by(
            &id,
            &external.get(),
            has_text(&help.get()),
            has_text(&error.get()),
            reason.get().is_some(),
        )
    })
}

pub(super) fn error_reference(id: String, error: Signal<Option<String>>) -> Signal<Option<String>> {
    Signal::derive(move || has_text(&error.get()).then(|| format!("{id}-error")))
}

#[cfg(not(target_arch = "wasm32"))]
fn textarea_initial_text(value: String) -> String {
    // Pinned tachys treats textarea children as unescaped SSR raw text. Encode only this
    // native HTML shell; the HTML parser decodes it back into the original textarea value.
    let mut encoded = value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;");
    // HTML drops the first LF immediately after a textarea start tag.
    if value.starts_with('\n') {
        encoded.insert(0, '\n');
    }
    encoded
}

#[cfg(target_arch = "wasm32")]
fn textarea_initial_text(value: String) -> String {
    // CSR creates a text node and uses the raw value property; entity-encoding here would
    // change visible text. This renderer package does not deploy WASM-side HTML serialization.
    value
}

#[component]
pub(super) fn FieldMessages(
    id: String,
    help: Signal<Option<String>>,
    error: Signal<Option<String>>,
    reason: Signal<Option<DisabledReason>>,
    #[prop(optional)] error_icon: bool,
) -> impl IntoView {
    // Keep the ordinary message's existing native markup unchanged; the decorative icon is
    // present only in the explicitly selected tonal recipe.
    let error_id = format!("{id}-error");
    let error_message = if error_icon {
        view! {
            <p id=error_id class="cui-field-error" hidden=move || !has_text(&error.get())>
                <span data-field-error-icon="true"><Icon name=IconName::WarningAmber/></span>
                {move || error.get().unwrap_or_default()}
            </p>
        }
        .into_any()
    } else {
        view! {
            <p id=error_id class="cui-field-error" hidden=move || !has_text(&error.get())>
                {move || error.get().unwrap_or_default()}
            </p>
        }
        .into_any()
    };
    view! {
        <p id=format!("{id}-help") class="cui-field-help" hidden=move || !has_text(&help.get())>
            {move || help.get().unwrap_or_default()}
        </p>
        {error_message}
        <p id=format!("{id}-reason") class="cui-field-help" hidden=move || reason.get().is_none()>
            {move || reason.get().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
        </p>
    }
}

/// A reactive `type` (the password eye) changes the attribute of the same element. Preserve its
/// selection synchronously: a delayed animation-frame restore would overwrite a newer selection
/// or edit. The element, focus, value and composition callbacks are never replaced. Types without
/// a text selection (`number`, `email`) report none, and nothing is restored for them.
#[cfg(target_arch = "wasm32")]
fn retype_preserving_selection(
    input: NodeRef<leptos::html::Input>,
    input_type: Signal<&'static str>,
) {
    fn restore(
        element: &leptos::web_sys::HtmlInputElement,
        selection: &(u32, u32, Option<String>),
    ) {
        let (start, end, direction) = selection;
        let _ = match direction {
            Some(direction) => element.set_selection_range_with_direction(*start, *end, direction),
            None => element.set_selection_range(*start, *end),
        };
    }
    Effect::new(move |previous: Option<&'static str>| {
        let next = input_type.get();
        let Some(element) = input.get() else {
            return next;
        };
        if previous.is_some_and(|previous| previous != next) {
            let start = element.selection_start().ok().flatten();
            let end = element.selection_end().ok().flatten();
            let direction = element.selection_direction().ok().flatten();
            element.set_type(next);
            if let (Some(start), Some(end)) = (start, end) {
                let selection = (start, end, direction);
                restore(&element, &selection);
            }
        }
        next
    });
}

#[component]
fn InputField(
    id: String,
    label: Signal<String>,
    value: Signal<String>,
    on_input: Callback<String>,
    on_composition_start: Option<Callback<CompositionEvent>>,
    on_composition_end: Option<Callback<CompositionEvent>>,
    input_type: Signal<&'static str>,
    variant: TextFieldVariant,
    trailing: Option<ViewFn>,
    state: Signal<FieldState>,
    help: Signal<Option<String>>,
    error: Signal<Option<String>>,
    described_by: Signal<Option<String>>,
    class: Signal<String>,
    name: Option<String>,
    required: bool,
    placeholder: Option<String>,
    autocomplete: Option<String>,
    maxlength: Option<u32>,
    min: Option<String>,
    max: Option<String>,
    step: Option<String>,
) -> impl IntoView {
    let reason = Signal::derive(move || state.get().reason().cloned());
    let descriptions = descriptions(id.clone(), described_by, help, error, reason);
    let error_reference = error_reference(id.clone(), error);
    let input_ref = NodeRef::<leptos::html::Input>::new();
    #[cfg(target_arch = "wasm32")]
    retype_preserving_selection(input_ref, input_type);
    let floating = variant.has_floating_label();
    let placeholder = if floating {
        Some(floating_placeholder(placeholder))
    } else {
        placeholder
    };
    let label_view = view! { <FieldLabel for_id=id.clone()>{move || label.get()}</FieldLabel> };
    let control = view! {
        <input id=id.clone() node_ref=input_ref class="cui-field-control" type=input_type.get_untracked() name=name required=required
            placeholder=placeholder autocomplete=autocomplete maxlength=maxlength min=min max=max step=step
            value=move || value.get() prop:value=move || value.get()
            readonly=move || state.get().is_read_only()
            disabled=move || state.get().is_disabled()
            aria-busy=move || state.get().is_loading().to_string()
            aria-invalid=move || has_text(&error.get()).to_string()
            aria-describedby=move || descriptions.get()
            aria-errormessage=move || error_reference.get()
            on:input=move |event| {
                if state.get_untracked().accepts_input() {
                    on_input.run(event_target_value(&event));
                }
            }
            on:compositionstart=move |event| {
                if let Some(callback) = on_composition_start { callback.run(event); }
            }
            on:compositionend=move |event| {
                if let Some(callback) = on_composition_end { callback.run(event); }
            }
        />
    };
    // The shell is the only painted surface. The label follows the control in the DOM so CSS can
    // read the browser's own value/focus state with sibling selectors; grid placement keeps the
    // label visually first. Messages stay outside it, below the control.
    let anatomy = if floating {
        let has_trailing = trailing.is_some();
        view! {
            <div class="cui-field-shell" data-invalid=move || has_text(&error.get()).to_string()
                data-trailing=has_trailing.then_some("true")>
                {control}
                {label_view}
                {trailing.map(|slot| view! {
                    // A pointer press on the action must not move focus out of the field: the caret,
                    // an active IME composition and a touch keyboard stay put. Keyboard focus and
                    // activation of the action itself are unchanged.
                    <div class="cui-field-trailing" on:mousedown=move |event| event.prevent_default()>{slot.run()}</div>
                })}
            </div>
        }
        .into_any()
    } else {
        view! { <>{label_view}{control}</> }.into_any()
    };
    view! {
        <div class=move || format!("cui-field {}", class.get()) data-variant=variant.as_str() data-recipe=(variant == TextFieldVariant::FloatingTonal).then_some("auth-tonal") data-kind=move || input_type.get() data-state=move || state.get().as_str()>
            {anatomy}
            <FieldMessages id=id help=help error=error reason=reason error_icon=variant == TextFieldVariant::FloatingTonal/>
        </div>
    }
}

/// Controlled single-line input; no validation, normalization or value ownership is hidden here.
/// Native input dispatch includes composition edits. The app can gate validation/autosave using
/// the composition callbacks; the library never reformats or commits text during composition.
#[component]
pub fn TextField(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(optional)] on_composition_start: Option<Callback<CompositionEvent>>,
    #[prop(optional)] on_composition_end: Option<Callback<CompositionEvent>>,
    /// A signal changes the native `type` of the same element (the password eye), so focus,
    /// selection and composition survive; nothing is remounted.
    #[prop(optional, into)]
    kind: Signal<TextFieldKind>,
    #[prop(optional)] variant: TextFieldVariant,
    #[prop(into, default = FieldState::Editable.into())] state: Signal<FieldState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(optional)] required: bool,
    #[prop(into, optional)] placeholder: Option<String>,
    #[prop(into, optional)] autocomplete: Option<String>,
    /// Native UTF-16 editing limit; callers retain their own validation policy.
    #[prop(optional)]
    maxlength: Option<u32>,
    /// One trailing action inside the shell of a `Floating` field: the password eye or a docked
    /// search submit. Pass a real native control (`IconButton`) with a localized name; the shell
    /// is never a clickable surface and owns no action or secret state. The control column
    /// reserves the action's width, so text never runs under it. Other variants ignore the slot.
    #[prop(optional, into)]
    trailing: Option<ViewFn>,
) -> impl IntoView {
    let trailing = trailing.filter(|_| variant.supports_trailing());
    let input_type = Signal::derive(move || kind.get().as_str());
    view! {
        <InputField id=id label=label value=value on_input=on_input input_type=input_type variant=variant
            trailing=trailing
            on_composition_start=on_composition_start on_composition_end=on_composition_end
            state=state help=help error=error described_by=described_by class=class name=name
            required=required placeholder=placeholder autocomplete=autocomplete maxlength=maxlength min=None max=None step=None/>
    }
}

/// Raw numeric strings include valid zero and an empty editing value. Parse in the app.
#[component]
pub fn NumberField(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(into, default = FieldState::Editable.into())] state: Signal<FieldState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(optional)] required: bool,
    #[prop(into, optional)] placeholder: Option<String>,
    #[prop(into, optional)] min: Option<String>,
    #[prop(into, optional)] max: Option<String>,
    #[prop(into, optional)] step: Option<String>,
) -> impl IntoView {
    view! {
        <InputField id=id label=label value=value on_input=on_input input_type=Signal::derive(|| "number") variant=TextFieldVariant::Regular
            trailing=None
            on_composition_start=None on_composition_end=None
            state=state help=help error=error described_by=described_by class=class name=name
            required=required placeholder=placeholder autocomplete=None maxlength=None min=min max=max step=step/>
    }
}

#[component]
pub fn TextArea(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(optional)] on_composition_start: Option<Callback<CompositionEvent>>,
    #[prop(optional)] on_composition_end: Option<Callback<CompositionEvent>>,
    #[prop(into, default = FieldState::Editable.into())] state: Signal<FieldState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(optional)] required: bool,
    #[prop(into, optional)] placeholder: Option<String>,
    #[prop(default = 4)] rows: u32,
) -> impl IntoView {
    let reason = Signal::derive(move || state.get().reason().cloned());
    let descriptions = descriptions(id.clone(), described_by, help, error, reason);
    let error_reference = error_reference(id.clone(), error);
    // Text content is the initial SSR value; the property owns subsequent browser editing.
    let initial = textarea_initial_text(value.get_untracked());
    view! {
        <div class=move || format!("cui-field {}", class.get()) data-state=move || state.get().as_str()>
            <FieldLabel for_id=id.clone()>{move || label.get()}</FieldLabel>
            <textarea id=id.clone() class="cui-field-control cui-textarea" name=name required=required
                placeholder=placeholder rows=rows prop:value=move || value.get()
                readonly=move || state.get().is_read_only()
                disabled=move || state.get().is_disabled()
                aria-busy=move || state.get().is_loading().to_string()
                aria-invalid=move || has_text(&error.get()).to_string()
                aria-describedby=move || descriptions.get()
                aria-errormessage=move || error_reference.get()
                on:input=move |event| {
                    if state.get_untracked().accepts_input() {
                        on_input.run(event_target_value(&event));
                    }
                }
                on:compositionstart=move |event| {
                    if let Some(callback) = on_composition_start { callback.run(event); }
                }
                on:compositionend=move |event| {
                    if let Some(callback) = on_composition_end { callback.run(event); }
                }>{initial}</textarea>
            <FieldMessages id=id help=help error=error reason=reason/>
        </div>
    }
}

/// Select-only visual density, independent of UiRoot density and other field geometry.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectDensity {
    #[default]
    Regular,
    /// Forty-unit painted surface inside the existing minimum interaction target.
    Compact,
}
impl SelectDensity {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Regular => "regular",
            Self::Compact => "compact",
        }
    }
}

/// Label composition does not change the control's accessible name or value ownership.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum SelectLabelPlacement {
    #[default]
    Above,
    Inline,
}
impl SelectLabelPlacement {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Above => "above",
            Self::Inline => "inline",
        }
    }
}

/// Native single-select with app-owned options. Options must carry stable values.
#[component]
pub fn Select(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(optional)] children: Option<Children>,
    /// Opt-in Web menu. Omit to retain the existing native option-child contract.
    #[prop(optional, into)]
    options: Option<Signal<Vec<crate::SelectOption>>>,
    #[prop(into, default = String::new().into())] placeholder: Signal<String>,
    #[prop(into, default = ChoiceState::Enabled.into())] state: Signal<ChoiceState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(optional)] required: bool,
    #[prop(optional)] density: SelectDensity,
    #[prop(optional)] label_placement: SelectLabelPlacement,
) -> impl IntoView {
    if let Some(options) = options {
        return view! {
            <crate::select_menu::MenuSelect id label value on_input options placeholder
                state help error described_by class name required density label_placement/>
        }
        .into_any();
    }
    let reason = Signal::derive(move || state.get().reason().cloned());
    let descriptions = descriptions(id.clone(), described_by, help, error, reason);
    let error_reference = error_reference(id.clone(), error);
    let node = NodeRef::<leptos::html::Select>::new();
    // A select property set before its options mount cannot choose a non-first option.
    // Reading the NodeRef in an owner-scoped effect applies the controlled value after mount.
    Effect::new(move |_| {
        let current = value.get();
        if let Some(select) = node.get() {
            select.set_value(&current);
        }
    });
    view! {
        <div class=move || format!("cui-field cui-select-field {}", class.get()) data-select-density=density.as_str()
            data-label-placement=label_placement.as_str() data-state=move || state.get().as_str()>
            <FieldLabel for_id=id.clone()>{move || label.get()}</FieldLabel>
            <div class="cui-select">
                <select id=id.clone() node_ref=node class="cui-field-control" name=name required=required
                    prop:value=move || value.get() disabled=move || !state.get().accepts_input()
                    aria-busy=move || state.get().is_loading().to_string()
                    aria-invalid=move || has_text(&error.get()).to_string()
                    aria-describedby=move || descriptions.get()
                    aria-errormessage=move || error_reference.get()
                    on:change=move |event| {
                        if state.get_untracked().accepts_input() {
                            on_input.run(event_target_value(&event));
                        }
                    }>{children.map(|render| render())}</select>
                <Icon name=IconName::ExpandMore/>
            </div>
            <FieldMessages id=id help=help error=error reason=reason/>
        </div>
    }.into_any()
}

#[component]
fn CheckedField(
    id: String,
    label: Signal<String>,
    checked: Signal<bool>,
    on_change: Callback<bool>,
    input_type: &'static str,
    role: Option<&'static str>,
    state: Signal<ChoiceState>,
    help: Signal<Option<String>>,
    error: Signal<Option<String>>,
    described_by: Signal<Option<String>>,
    class: Signal<String>,
    name: Option<String>,
    value: Option<String>,
    required: bool,
) -> impl IntoView {
    let reason = Signal::derive(move || state.get().reason().cloned());
    let descriptions = descriptions(id.clone(), described_by, help, error, reason);
    let error_reference = error_reference(id.clone(), error);
    view! {
        <div class=move || format!("cui-field cui-choice {}", class.get()) data-state=move || state.get().as_str()>
            <label for=id.clone() class="cui-choice-label">
                <input id=id.clone() class="cui-choice-control" type=input_type role=role name=name value=value
                    required=required checked=move || checked.get() prop:checked=move || checked.get()
                    disabled=move || !state.get().accepts_input()
                    aria-busy=move || state.get().is_loading().to_string()
                    aria-checked=move || role.map(|_| checked.get().to_string())
                    aria-invalid=move || has_text(&error.get()).to_string()
                    aria-describedby=move || descriptions.get()
                    aria-errormessage=move || error_reference.get()
                    on:change=move |event| {
                        if state.get_untracked().accepts_input() {
                            on_change.run(event_target_checked(&event));
                        }
                    }/>
                <span class="cui-field-label">{move || label.get()}</span>
            </label>
            <FieldMessages id=id help=help error=error reason=reason/>
        </div>
    }
}

#[component]
pub fn Checkbox(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] on_change: Callback<bool>,
    #[prop(into, default = ChoiceState::Enabled.into())] state: Signal<ChoiceState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(into, optional)] value: Option<String>,
    #[prop(optional)] required: bool,
) -> impl IntoView {
    view! {
        <CheckedField id=id label=label checked=checked on_change=on_change input_type="checkbox" role=None
            state=state help=help error=error described_by=described_by class=class name=name value=value required=required/>
    }
}

/// Native radios with the same name form a group; the app controls the checked signals.
/// Compose a fieldset/legend in the app for the group's shared question.
#[component]
pub fn Radio(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] name: String,
    #[prop(into)] value: String,
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] on_change: Callback<bool>,
    #[prop(into, default = ChoiceState::Enabled.into())] state: Signal<ChoiceState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(optional)] required: bool,
) -> impl IntoView {
    view! {
        <CheckedField id=id label=label checked=checked on_change=on_change input_type="radio" role=None
            state=state help=help error=error described_by=described_by class=class name=Some(name) value=Some(value) required=required/>
    }
}

/// Native checkbox input with switch semantics. Its label remains stable as it changes state.
#[component]
pub fn Switch(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] checked: Signal<bool>,
    #[prop(into)] on_change: Callback<bool>,
    #[prop(into, default = ChoiceState::Enabled.into())] state: Signal<ChoiceState>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
) -> impl IntoView {
    view! {
        <CheckedField id=id label=label checked=checked on_change=on_change input_type="checkbox" role=Some("switch")
            state=state help=help error=error described_by=described_by class=class name=name value=None required=false/>
    }
}

/// The browser owns native range normalization and keyboard steps; this component does no parsing.
/// Supply localized value_text when a number alone is not meaningful (for example elapsed time).
#[component]
pub fn RangeField(
    #[prop(into)] id: String,
    #[prop(into)] label: Signal<String>,
    #[prop(into)] value: Signal<String>,
    #[prop(into)] on_input: Callback<String>,
    #[prop(into, default = ChoiceState::Enabled.into())] state: Signal<ChoiceState>,
    #[prop(into, default = None.into())] value_text: Signal<Option<String>>,
    #[prop(into, default = None.into())] help: Signal<Option<String>>,
    #[prop(into, default = None.into())] error: Signal<Option<String>>,
    #[prop(into, default = None.into())] described_by: Signal<Option<String>>,
    #[prop(into, default = String::new().into())] class: Signal<String>,
    #[prop(into, optional)] name: Option<String>,
    #[prop(into, optional)] min: Option<String>,
    #[prop(into, optional)] max: Option<String>,
    #[prop(into, optional)] step: Option<String>,
) -> impl IntoView {
    let reason = Signal::derive(move || state.get().reason().cloned());
    let descriptions = descriptions(id.clone(), described_by, help, error, reason);
    let error_reference = error_reference(id.clone(), error);
    view! {
        <div class=move || format!("cui-field {}", class.get()) data-state=move || state.get().as_str()>
            <FieldLabel for_id=id.clone()>{move || label.get()}</FieldLabel>
            <input id=id.clone() class="cui-range-control" type="range" name=name min=min max=max step=step
                value=move || value.get() prop:value=move || value.get()
                disabled=move || !state.get().accepts_input()
                aria-busy=move || state.get().is_loading().to_string()
                aria-invalid=move || has_text(&error.get()).to_string()
                aria-valuetext=move || value_text.get()
                aria-describedby=move || descriptions.get()
                aria-errormessage=move || error_reference.get()
                on:input=move |event| {
                    if state.get_untracked().accepts_input() {
                        on_input.run(event_target_value(&event));
                    }
                }/>
            <FieldMessages id=id help=help error=error reason=reason/>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn descriptions_keep_external_ids_and_only_visible_messages() {
        assert_eq!(described_by("title", &None, false, false, false), None);
        assert_eq!(
            described_by(
                "title",
                &Some("context title-help".into()),
                true,
                true,
                true
            ),
            Some("context title-help title-error title-reason".into())
        );
        assert_eq!(
            described_by(
                "title",
                &Some("  context\nmore  ".into()),
                false,
                false,
                false
            ),
            Some("context more".into())
        );
        assert!(!has_text(&Some(" \n".into())));
        assert!(has_text(&Some("Thiếu tên nhân vật".into())));
    }

    #[cfg(not(target_arch = "wasm32"))]
    #[test]
    fn native_textarea_shell_encodes_raw_text_without_reinterpreting_entities() {
        assert_eq!(
            textarea_initial_text("&amp; < >".into()),
            "&amp;amp; &lt; &gt;"
        );
        assert_eq!(textarea_initial_text("\nLời thoại".into()), "\n\nLời thoại");
        assert_eq!(textarea_initial_text(String::new()), "");
    }
}
