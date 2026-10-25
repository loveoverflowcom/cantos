//! Theatre expression specimens (token v0.7.0). Local values and callbacks only: nothing here
//! calls a service, stores a secret or implies an app composition. The subtree opts in with
//! `UiRoot expression=Theatre`; the rest of the gallery stays Standard.

use super::messages::{theatre_fixture as fixture, tool};

use cantos_ui::{
    AccessibleName, Avatar, AvatarSize, Button, ButtonState, ButtonVariant, Density,
    DisabledReason, Expression, FieldState, FilterChip, IconButton, IconName, Logo, LogoTone,
    LogoVariant, Select, SelectDensity, SelectOption, Space, Stack, StackDirection, TextField,
    TextFieldKind, TextFieldVariant, Theme, UiRoot,
};
use leptos::prelude::*;

const SEARCH_QUERY_NAME: &str = "q";

fn label(text: &str) -> AccessibleName {
    AccessibleName::new(text).expect("preview names are nonempty")
}

fn reason(text: &str) -> DisabledReason {
    DisabledReason::new(text).expect("preview reasons are nonempty")
}

/// One password field with its own visibility: two fields never share a toggle.
#[component]
fn PasswordSample(
    id: &'static str,
    label_text: &'static str,
    value: RwSignal<String>,
    error: Option<&'static str>,
) -> impl IntoView {
    let visible = RwSignal::new(false);
    let action_name = Signal::derive(move || {
        label(if visible.get() {
            {
                fixture::HIDE_PASSWORD
            }
        } else {
            {
                fixture::SHOW_PASSWORD
            }
        })
    });
    let glyph = Signal::derive(move || {
        if visible.get() {
            IconName::VisibilityOff
        } else {
            IconName::Visibility
        }
    });
    view! {
        <TextField id=id label=label_text variant=TextFieldVariant::Floating
            kind=Signal::derive(move || if visible.get() { TextFieldKind::Text } else { TextFieldKind::Password })
            value=value autocomplete="off"
            error=error.map(str::to_owned)
            help=Some(fixture::PASSWORD_HELP.to_owned())
            on_input=Callback::new(move |next| value.set(next))
            trailing=move || view! {
                <IconButton id=format!("{id}-eye") name=action_name icon=glyph
                    on_press=Callback::new(move |_| visible.update(|shown| *shown = !*shown))/>
            }/>
    }
}

#[component]
pub fn TheatreExpressionSamples(
    theme: RwSignal<Theme>,
    density: RwSignal<Density>,
) -> impl IntoView {
    let empty = RwSignal::new(String::new());
    let filled = RwSignal::new(fixture::EMAIL_VALUE.to_owned());
    let invalid = RwSignal::new(fixture::EMAIL_INVALID.to_owned());
    let long_value = RwSignal::new(fixture::LONG_VALUE.to_owned());
    let secret = RwSignal::new(fixture::PASSWORD_VALUE.to_owned());
    let confirm = RwSignal::new(fixture::REPEAT_VALUE.to_owned());
    let query = RwSignal::new(String::new());
    let submissions = RwSignal::new(0u32);
    let selected = RwSignal::new(true);
    let other = RwSignal::new(false);
    let language = RwSignal::new("vi".to_owned());
    let pending = Signal::derive(|| ButtonState::Loading(reason(fixture::SENDING)));
    let languages = Signal::derive(|| {
        vec![
            SelectOption::new("vi", fixture::VIETNAMESE),
            SelectOption::new("en", fixture::ENGLISH),
        ]
    });
    view! {
        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-theatre"
            data-preview-component="theatre-expression"
            data-preview-scenario="theatre-expression theatre-fields theatre-trailing-eye theatre-docked-search theatre-tonal theatre-docked theatre-selection theatre-utility theatre-group theatre-control theatre-header"
            aria-labelledby="gallery-theatre-heading">
            <UiRoot theme=theme density=density expression=Expression::Theatre>
                <div class="cui-theatre-sample">
                    <Stack gap=Space::Md>
                        <h2 id="gallery-theatre-heading" lang="en" class="cui-theatre-heading">{tool::THEATRE_TITLE}</h2>
                        <p lang="en" class="cui-gallery-note">{tool::THEATRE_INTRO}</p>

                        // theatre-header: official symbol + bold word, tonal theme action, accent avatar.
                        <div class="cui-theatre-header" data-preview-state="theatre-header">
                            <span class="cui-theatre-brand">
                                <Logo variant=LogoVariant::Symbol tone=Signal::derive(move || {
                                    if theme.get() == Theme::Dark { LogoTone::Dark } else { LogoTone::Light }
                                }) class="cui-theatre-mark"/>
                                <span class="cui-theatre-wordmark">{fixture::BRAND}</span>
                            </span>
                            <span class="cui-theatre-header-actions">
                                <IconButton id="theatre-theme" variant=ButtonVariant::Tonal
                                    name=Signal::derive(move || label(if theme.get() == Theme::Dark { fixture::SWITCH_LIGHT } else { fixture::SWITCH_DARK }))
                                    icon=Signal::derive(move || if theme.get() == Theme::Dark { IconName::LightMode } else { IconName::DarkMode })
                                    on_press=Callback::new(move |_| theme.update(|current| {
                                        *current = if *current == Theme::Dark { Theme::Light } else { Theme::Dark };
                                    }))/>
                                <span data-preview-state="theatre-control">
                                    <Avatar display_name=fixture::BRAND size=AvatarSize::Control accent=true
                                        accessible_name=label(fixture::ACCOUNT)/>
                                </span>
                            </span>
                        </div>

                        // theatre-fields: every Floating state in one matrix.
                        <div class="cui-theatre-matrix" data-preview-state="theatre-fields">
                            <TextField id="theatre-idle" label=fixture::EMAIL variant=TextFieldVariant::Floating kind=TextFieldKind::Email
                                value=empty on_input=Callback::new(move |next| empty.set(next))/>
                            <TextField id="theatre-filled" label=fixture::EMAIL variant=TextFieldVariant::Floating kind=TextFieldKind::Email
                                value=filled on_input=Callback::new(move |next| filled.set(next))/>
                            <TextField id="theatre-error" label=fixture::EMAIL variant=TextFieldVariant::Floating kind=TextFieldKind::Email
                                value=invalid error=fixture::EMAIL_ERROR on_input=Callback::new(move |next| invalid.set(next))/>
                            <TextField id="theatre-readonly" label=fixture::EMAIL variant=TextFieldVariant::Floating kind=TextFieldKind::Email
                                value=filled state=FieldState::ReadOnly on_input=Callback::new(|_: String| {})/>
                            <TextField id="theatre-disabled" label=fixture::EMAIL variant=TextFieldVariant::Floating kind=TextFieldKind::Email
                                value=filled state=FieldState::Disabled(reason(fixture::UNAVAILABLE))
                                on_input=Callback::new(|_: String| {})/>
                            <TextField id="theatre-loading" label=fixture::EMAIL variant=TextFieldVariant::Floating kind=TextFieldKind::Email
                                value=filled state=FieldState::Loading(reason(fixture::LOADING))
                                on_input=Callback::new(|_: String| {})/>
                            <TextField id="theatre-long" label=fixture::LONG_LABEL
                                variant=TextFieldVariant::Floating value=long_value
                                on_input=Callback::new(move |next| long_value.set(next))/>
                        </div>

                        // theatre-trailing-eye: independent visibility per field.
                        <div class="cui-theatre-matrix" data-preview-state="theatre-trailing-eye">
                            <PasswordSample id="theatre-password" label_text=fixture::NEW_PASSWORD value=secret error=None/>
                            <PasswordSample id="theatre-confirm" label_text=fixture::REPEAT_PASSWORD value=confirm
                                error=Some(fixture::MISMATCH)/>
                        </div>

                        // theatre-docked-search: the native form owns Enter, composition and submit.
                        <form class="cui-theatre-search" data-preview-state="theatre-docked-search"
                            on:submit=move |event| { event.prevent_default(); submissions.update(|count| *count += 1); }>
                            <TextField id="theatre-search" label=fixture::SEARCH kind=TextFieldKind::Search
                                variant=TextFieldVariant::Floating value=query name=SEARCH_QUERY_NAME
                                on_input=Callback::new(move |next| query.set(next))
                                trailing=move || view! {
                                    <IconButton id="theatre-search-submit" name=label(fixture::SEARCH)
                                        icon=IconName::Search variant=ButtonVariant::Primary submit=true/>
                                }/>
                        </form>
                        <p lang="en" class="cui-gallery-note" aria-live="off">{move || tool::theatre_submissions(submissions.get(), &query.get())}</p>

                        // theatre-selection, theatre-utility, theatre-tonal, theatre-docked, theatre-group.
                        <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                            <span data-preview-state="theatre-selection">
                                <FilterChip id="theatre-chip-on" selected=selected
                                    on_change=Callback::new(move |next| selected.set(next))>{fixture::ALL_TYPES}</FilterChip>
                            </span>
                            <FilterChip id="theatre-chip-off" selected=other
                                on_change=Callback::new(move |next| other.set(next))>{fixture::NOVEL}</FilterChip>
                            <FilterChip id="theatre-chip-long" selected=false on_change=Callback::new(|_: bool| {})
                                state=ButtonState::Disabled(reason(fixture::NO_CONTENT))>
                                {fixture::LONG_CHIP}
                            </FilterChip>
                            <span data-preview-state="theatre-utility">
                                <Select id="theatre-language" label=fixture::LANGUAGE value=language density=SelectDensity::Compact
                                    options=languages on_input=Callback::new(move |next| language.set(next))/>
                            </span>
                        </Stack>
                        <div data-preview-state="theatre-group">
                            <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                                <Button id="theatre-login" icon=IconName::Login>{fixture::SIGN_IN}</Button>
                                <Button id="theatre-tonal" variant=ButtonVariant::Tonal icon=IconName::PersonAdd>{fixture::SIGN_UP}</Button>
                                <Button id="theatre-long-button" icon=IconName::Mail>{fixture::LONG_ACTION}</Button>
                                <Button id="theatre-pending" icon=IconName::Check state=pending>{fixture::CONFIRM}</Button>
                            </Stack>
                        </div>
                        <p lang="en" class="cui-gallery-note" data-preview-state="theatre-tonal theatre-docked">
                            {tool::THEATRE_CONTROL_HELP}
                        </p>
                        <div class="cui-theatre-hero" data-preview-state="theatre-expression">
                            <p class="cui-theatre-hero-title">{fixture::HERO_TITLE}</p>
                            <p>{fixture::HERO_SUBTITLE}</p>
                        </div>
                    </Stack>
                </div>
            </UiRoot>
        </section>
    }
}
