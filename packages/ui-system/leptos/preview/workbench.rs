//! Preview-only composition root. Recipes select production controls; no services are built.
use super::messages::{
    fixture_label, specimen, tool, SpecimenKey, ENGLISH_AUTONYM, VIETNAMESE_AUTONYM,
};
use super::{motion_probe, platform, recipe::*, Gallery};
use cantos_ui::{
    AccessibleName, BackButton, BackButtonVariant, Button, ButtonState, ButtonVariant, ChoiceState,
    Density, Dialog, DisabledReason, FieldState, FilterChip, IconName, Select, SelectDensity,
    SelectLabelPlacement, SelectOption, Space, Stack, Switch, TextField, TextFieldKind,
    TextFieldVariant, Theme, UiRoot,
};
use leptos::prelude::*;

/// Diagnostic log is bounded even during repeated input; each callback is recorded immediately.
const MAX_EVENTS: usize = 40;

#[derive(Clone, Copy)]
pub struct Host {
    recipe: RwSignal<Recipe>,
    reset: RwSignal<u32>,
    events: RwSignal<Vec<String>>,
}
impl Host {
    fn log(self, event: String) {
        self.events.update(|events| {
            if events.len() == MAX_EVENTS {
                events.remove(0);
            }
            events.push(event);
        });
    }
    fn restore(self) {
        self.reset.update(|epoch| *epoch = epoch.wrapping_add(1));
        self.events.set(Vec::new());
    }
}

#[component]
fn RecipeSelect(
    id: &'static str,
    label: &'static str,
    value: Signal<String>,
    values: Vec<&'static str>,
    on_change: Callback<String>,
) -> impl IntoView {
    view! { <Select id=id label=label value=value on_input=on_change>
        {values.into_iter().map(|value| view! { <option value=value>{value}</option> }).collect_view()}
    </Select> }
}

#[component]
pub fn Workbench(#[prop(optional)] initial_query: Option<String>) -> impl IntoView {
    let parsed = Recipe::decode_query(&initial_query.unwrap_or_else(platform::current_query));
    let fallback = parsed
        .as_ref()
        .err()
        .map(|error| format!("Invalid scenario; using the default recipe: {error:?}"));
    let initial = parsed.unwrap_or_default();
    let recipe = RwSignal::new(initial);
    let state_component = Memo::new(move |_| recipe.get().component);
    let host = Host {
        recipe,
        reset: RwSignal::new(0),
        events: RwSignal::new(Vec::new()),
    };
    let theme = RwSignal::new(initial.theme);
    let density = RwSignal::new(initial.density);
    let facts = RwSignal::new(String::from(tool::BROWSER_PENDING));
    let notice = RwSignal::new(String::new());
    let frame_ref = NodeRef::<leptos::html::Section>::new();
    let motion_frames = RwSignal::new(Vec::<String>::new());
    let capture_label = RwSignal::new("review-1".to_owned());
    motion_probe::bind(
        frame_ref,
        motion_frames,
        Signal::derive(move || format!("{}#{}", recipe.get().encode_query(), host.reset.get())),
    );
    // Theme controls in the retained catalog share this root; neither path resets fixtures.
    Effect::new(move |_| {
        recipe.update(|value| {
            value.theme = theme.get();
            value.density = density.get();
        })
    });
    Effect::new(move |_| {
        if let Err(error) = platform::replace_query(&recipe.get().encode_query()) {
            notice.set(format!("Scenario URL unavailable: {error:?}"));
        }
    });
    let refresh = Callback::new(move |_| {
        facts.set(match platform::read_browser_facts() {
            Ok(facts) => format!("{facts:#?}"),
            Err(error) => format!("Browser facts unavailable: {error:?}"),
        });
    });
    let export = Callback::new(move |_| {
        let browser = platform::read_browser_facts();
        let report = format!(
            "Cantos preview evidence metadata v1\nrecipe={}\nsource_sha={}\nsource_dirty={}\nsource_identity=build-supplied; unrecorded is not attribution\ntoken_header={}\ncatalog_header={}\nfont_source=local Noto Sans/Serif; shared/assets/noto-fonts/provenance.json\nbrowser={browser:#?}\nevents={:#?}\ncapture_label={}\nmotion_frames={:#?}\nclaims=metadata and computed-style frame observations only; captures, inspected pixels, trusted input and video recordings are separate\n",
            recipe.get_untracked().encode_query(),
            option_env!("CANTOS_PREVIEW_SOURCE_SHA").unwrap_or("unrecorded"),
            option_env!("CANTOS_PREVIEW_SOURCE_DIRTY").unwrap_or("unrecorded"),
            include_str!("../src/tokens.rs").lines().next().unwrap_or("unrecorded"),
            include_str!("../src/preview_catalog.rs").lines().next().unwrap_or("unrecorded"),
            host.events.get_untracked(),
            capture_label.get_untracked(),
            motion_frames.get_untracked(),
        );
        match platform::download_text("cantos-preview-evidence.txt", &report) {
            Ok(()) => notice.set(tool::EXPORTED.to_owned()),
            Err(error) => notice.set(format!("Export unavailable: {error:?}")),
        }
    });
    view! {
        <UiRoot theme=theme density=density>
            <main class="cui-workbench" lang=tool::LANGUAGE_TAG data-preview-workbench="v1">
                <header><p>{tool::WORKBENCH_BRAND}</p><h1>{tool::WORKBENCH_TITLE}</h1>
                    <p>{tool::WORKBENCH_INTRO}</p>
                </header>
                {fallback.map(|message| view! { <p id="scenario-fallback" role="alert">{message}</p> })}
                <section aria-label=tool::RECIPE class="cui-recipe-controls">
                    <RecipeSelect id="recipe-component" label=tool::COMPONENT values=ComponentId::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().component.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(component) = ComponentId::from_attribute(&value) {
                            recipe.update(|recipe| { recipe.component = component; recipe.state = component.default_state(); });
                        } })/>
                    <Select id="recipe-state" label=tool::INITIAL_STATE
                        value=Signal::derive(move || recipe.get().state.as_str().to_owned())
                        on_input=Callback::new(move |value: String| { if let Some(state) = ScenarioState::from_attribute(&value) {
                            let mut next = recipe.get_untracked(); next.state = state;
                            match next.validate() { Ok(_) => { recipe.set(next); host.restore(); }, Err(error) => notice.set(format!("Incompatible state: {error:?}")) }
                        } })>
                        {move || state_component.get().states().iter().map(|state| view! { <option value=state.as_str()>{state.as_str()}</option> }).collect_view()}
                    </Select>
                    <RecipeSelect id="recipe-mode" label=tool::MODE values=PreviewMode::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().mode.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(mode) = PreviewMode::from_attribute(&value) { recipe.update(|recipe| recipe.mode = mode); } })/>
                    <RecipeSelect id="recipe-fixture" label=tool::FIXTURE values=Fixture::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().fixture.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(fixture) = Fixture::from_attribute(&value) { recipe.update(|recipe| recipe.fixture = fixture); host.restore(); } })/>
                    <RecipeSelect id="recipe-theme" label=tool::THEME values=vec!["system", "light", "dark"]
                        value=Signal::derive(move || theme.get().as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(value) = Theme::from_attribute(&value) { theme.set(value); } })/>
                    <RecipeSelect id="recipe-density" label=tool::DENSITY values=vec!["compact", "comfortable"]
                        value=Signal::derive(move || density.get().as_str().to_owned())
                        on_change=Callback::new(move |value: String| density.set(if value == "comfortable" { Density::Comfortable } else { Density::Compact }))/>
                    <RecipeSelect id="recipe-locale" label=tool::SPECIMEN_LANGUAGE values=Locale::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().locale.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(locale) = Locale::from_attribute(&value) { recipe.update(|recipe| recipe.locale = locale); } })/>
                    <RecipeSelect id="recipe-text" label=tool::SPECIMEN_TEXT values=TextScale::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().text_scale.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(scale) = TextScale::from_attribute(&value) { recipe.update(|recipe| recipe.text_scale = scale); } })/>
                    <RecipeSelect id="recipe-frame" label=tool::FRAME values=FrameWidth::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().frame.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(frame) = FrameWidth::from_attribute(&value) { recipe.update(|recipe| recipe.frame = frame); } })/>
                    <RecipeSelect id="recipe-motion" label=tool::MOTION values=Motion::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().motion.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(motion) = Motion::from_attribute(&value) { recipe.update(|recipe| recipe.motion = motion); } })/>
                    <RecipeSelect id="recipe-playback" label=tool::PLAYBACK values=Playback::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().playback.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(playback) = Playback::from_attribute(&value) { recipe.update(|recipe| recipe.playback = playback); if playback == Playback::Replay { host.restore(); } } })/>
                </section>
                <Stack direction=cantos_ui::StackDirection::Horizontal gap=Space::Sm>
                    <Button id="recipe-reset" on_press=Callback::new(move |_| host.restore())>{tool::RESET}</Button>
                    <Button id="recipe-replay" on_press=Callback::new(move |_| { host.restore(); notice.set(tool::REPLAY_NOTICE.to_owned()); })>{tool::REPLAY}</Button>
                    <Button id="recipe-facts" on_press=refresh>{tool::REFRESH}</Button>
                    <Button id="recipe-export" on_press=export>{tool::EXPORT}</Button>
                </Stack>
                <p id="recipe-notice" role="status">{move || notice.get()}</p>
                <a id="recipe-link" href=move || recipe.get().encode_query()>{tool::LINK}</a>
                <details><summary>{tool::PROVENANCE}</summary><code id="recipe-query">{move || recipe.get().encode_query()}</code><pre id="recipe-facts-output">{move || facts.get()}</pre></details>
                <p>{tool::NATIVE_POLICY}</p>
                <div class="cui-specimen-scroll" tabindex="0" aria-label=tool::FRAME_NAME>
                    <section class="cui-specimen-frame" id="recipe-frame-content" node_ref=frame_ref
                        lang=move || recipe.get().locale.language_tag()
                        style=move || format!("--cui-frame-width: {}px", recipe.get().frame.as_str())
                        data-motion=move || recipe.get().motion.as_str()
                        data-playback=move || recipe.get().playback.as_str()
                        data-text-scale=move || recipe.get().text_scale.as_str()>
                        {ComponentId::ALL.iter().copied().map(|component| view! {
                            <div hidden=move || recipe.get().component != component>
                                {component.states().iter().copied().map(|state| view! {
                                    <div hidden=move || recipe.get().mode != PreviewMode::Matrix && recipe.get().state != state>
                                        <Specimen host=host component=component state=state/>
                                    </div>
                                }).collect_view()}
                            </div>
                        }).collect_view()}
                    </section>
                </div>
                <section aria-label=tool::INTENTS_NAME><h2>{tool::INTENTS}</h2><ol id="recipe-events" aria-live="polite">
                    {move || host.events.get().into_iter().map(|event| view! { <li>{event}</li> }).collect_view()}
                </ol></section>
                <section aria-label=tool::MOTION_NAME>
                    <h2>{tool::MOTION_PROFILE}</h2>
                    <TextField id="recipe-capture-label" label=tool::CAPTURE_LABEL value=capture_label on_input=Callback::new(move |value| capture_label.set(value))/>
                    <p>{tool::MOTION_DESCRIPTION}</p>
                    <pre id="recipe-motion-frames">{move || motion_frames.get().join("\n")}</pre>
                </section>
            </main>
        </UiRoot>
        <div hidden=move || recipe.get().mode != PreviewMode::Catalog>
            <Gallery host_theme=theme host_density=density/>
        </div>
    }
}

#[component]
fn Specimen(host: Host, component: ComponentId, state: ScenarioState) -> impl IntoView {
    let id = format!("scenario-{}-{}", component.as_str(), state.as_str());
    let control_id = format!("{id}-control");
    let selected = RwSignal::new(matches!(state, ScenarioState::Selected | ScenarioState::On));
    let opened = RwSignal::new(false);
    let text = RwSignal::new(String::new());
    let language_value = RwSignal::new("vi-VN".to_owned());
    let select_value = RwSignal::new(
        if state == ScenarioState::MissingValue {
            "missing"
        } else {
            "system"
        }
        .to_owned(),
    );
    // Same mounted component across modes/themes. Only an explicit recipe/reset restores data.
    Effect::new(move |_| {
        host.reset.get();
        selected.set(matches!(state, ScenarioState::Selected | ScenarioState::On));
        text.set(if component == ComponentId::AuthTonal {
            "nguyen-thi-anh-nguyet+truyen-dai@example.test".to_owned()
        } else {
            fixture_label(host.recipe.get_untracked()).to_owned()
        });
        language_value.set("vi-VN".to_owned());
        select_value.set(
            if state == ScenarioState::MissingValue {
                "missing"
            } else {
                "system"
            }
            .to_owned(),
        );
    });
    let active = Memo::new(move |_| {
        let recipe = host.recipe.get();
        recipe.component == component
            && (recipe.mode == PreviewMode::Matrix || recipe.state == state)
    });
    Effect::new(move |_| {
        host.reset.get();
        opened
            .set(component == ComponentId::Dialog && state == ScenarioState::Open && active.get());
    });
    let blocked_reason = Signal::derive(move || {
        DisabledReason::new(specimen(host.recipe.get().locale, SpecimenKey::Blocked))
            .expect("nonempty sample copy")
    });
    let button_state = Signal::derive(move || match state {
        ScenarioState::Disabled => ButtonState::Disabled(blocked_reason.get()),
        ScenarioState::Loading => ButtonState::Loading(blocked_reason.get()),
        _ => ButtonState::Enabled,
    });
    let choice_state = Signal::derive(move || match state {
        ScenarioState::Disabled => ChoiceState::Disabled(blocked_reason.get()),
        ScenarioState::Loading => ChoiceState::Loading(blocked_reason.get()),
        _ => ChoiceState::Enabled,
    });
    let log_id = StoredValue::new(id.clone());
    let change = Callback::new(move |value| {
        selected.set(value);
        host.log(format!("{} change={value}", log_id.get_value()));
    });
    let press = Callback::new(move |_| host.log(format!("{} press", log_id.get_value())));
    let label = Signal::derive(move || fixture_label(host.recipe.get()).to_owned());
    let content = match component {
        ComponentId::AuthTonal => {
            let field_state = Signal::derive(move || match state {
                ScenarioState::Disabled => FieldState::Disabled(blocked_reason.get()),
                ScenarioState::Loading => FieldState::Loading(blocked_reason.get()),
                ScenarioState::ReadOnly => FieldState::ReadOnly,
                _ => FieldState::Editable,
            });
            let email_label = Signal::derive(move || {
                let recipe = host.recipe.get();
                specimen(recipe.locale, if recipe.fixture == Fixture::LongVietnamese {
                    SpecimenKey::EmailLong
                } else { SpecimenKey::Email }).to_owned()
            });
            let help = Signal::derive(move || Some(specimen(host.recipe.get().locale, SpecimenKey::EmailHelp).to_owned()));
            let error = Signal::derive(move || (state == ScenarioState::Error).then(|| specimen(host.recipe.get().locale, SpecimenKey::EmailError).to_owned()));
            let baseline_id = format!("{control_id}-baseline");
            let cta_id = format!("{control_id}-cta");
            let baseline_cta_id = format!("{control_id}-baseline-cta");
            let edit = Callback::new(move |value: String| {
                text.set(value);
                // Do not export entered text in diagnostics.
                host.log(format!("{} input", log_id.get_value()));
            });
            let cta = Signal::derive(move || specimen(host.recipe.get().locale, SpecimenKey::Continue).to_owned());
            view! {
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::AUTH_RHYTHM}</p>
                <Stack gap=Space::Lg class="cui-auth-field-rhythm" attr:data-preview-scenario="auth-tonal-spacious auth-field-focus auth-field-error auth-field-cta autofill forced-colors">
                    <TextField id=control_id variant=TextFieldVariant::AuthTonalSpacious kind=TextFieldKind::Email
                        name="email" required=true autocomplete="email" label=email_label value=text
                        state=field_state help=help error=error on_input=edit/>
                    <Button id=cta_id variant=ButtonVariant::AuthPrimary state=button_state on_press=press>{move || cta.get()}</Button>
                </Stack>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::AUTH_RHYTHM_BASELINE}</p>
                <Stack gap=Space::Md>
                    <TextField id=baseline_id variant=TextFieldVariant::AuthTonal kind=TextFieldKind::Email
                        name="baseline-email" required=true autocomplete="off" label=email_label value=text
                        state=field_state help=help error=error on_input=edit/>
                    <Button id=baseline_cta_id variant=ButtonVariant::AuthPrimary state=button_state on_press=press>{move || cta.get()}</Button>
                </Stack>
            }.into_any()
        }
        ComponentId::BackButton => {
            let name = Signal::derive(move || {
                let recipe = host.recipe.get();
                AccessibleName::new(specimen(recipe.locale, if recipe.fixture == Fixture::LongVietnamese {
                    SpecimenKey::AccountLong
                } else { SpecimenKey::Account })).expect("nonblank localized preview name")
            });
            let login_name = Signal::derive(move || AccessibleName::new(specimen(host.recipe.get().locale, SpecimenKey::SignIn)).expect("nonblank localized preview name"));
            let login_id = format!("{control_id}-login");
            let icon_id = format!("{control_id}-icon");
            let baseline_id = format!("{control_id}-baseline");
            view! {
                <BackButton id=control_id variant=BackButtonVariant::Unified name=name state=button_state on_back=press/>
                <BackButton id=login_id variant=BackButtonVariant::Unified name=login_name state=button_state on_back=press/>
                <BackButton id=icon_id variant=BackButtonVariant::Unified name=name icon_only=true state=button_state on_back=press/>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::BACK_CANDIDATE}</p>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::ICON_BASELINE}</p>
                <BackButton id=baseline_id name=name state=button_state on_back=press/>
            }.into_any()
        }
        ComponentId::Button | ComponentId::AuthPrimary => view! {
            <Button id=control_id variant=if component == ComponentId::AuthPrimary { ButtonVariant::AuthPrimary } else { ButtonVariant::Primary }
                icon=if component == ComponentId::AuthPrimary { IconName::Login } else { IconName::Save }
                state=button_state on_press=press>{move || super::messages::format_specimen(host.recipe.get().locale,
                    if component == ComponentId::AuthPrimary { SpecimenKey::SignInTitle } else { SpecimenKey::SaveDraft },
                    &[("title", label.get().as_str())])}</Button>
        }.into_any(),
        ComponentId::FilterChip => view! {
            <FilterChip id=control_id selected=selected state=button_state on_change=change>{move || label.get()}</FilterChip>
        }.into_any(),
        ComponentId::Switch => view! { <Switch id=control_id label=label checked=selected state=choice_state on_change=change/> }.into_any(),
        ComponentId::Select => {
            let language_id = format!("{control_id}-language");
            let native_id = format!("{control_id}-native");
            let options = Signal::derive(move || {
                if state == ScenarioState::EmptyOptions { return Vec::new(); }
                let recipe = host.recipe.get();
                vec![
                    SelectOption::new("system", if recipe.fixture == Fixture::LongVietnamese {
                        fixture_label(recipe)
                    } else { specimen(recipe.locale, SpecimenKey::System) }),
                    SelectOption::new("light", specimen(recipe.locale, SpecimenKey::Light)),
                    SelectOption { disabled: true, ..SelectOption::new("unavailable", specimen(recipe.locale, SpecimenKey::Unavailable)) },
                    SelectOption::new("dark", specimen(recipe.locale, SpecimenKey::Dark)),
                ]
            });
            view! {
                <Select id=control_id label=Signal::derive(move || specimen(host.recipe.get().locale, SpecimenKey::Theme).to_owned())
                    value=select_value options=options state=choice_state
                    placeholder=Signal::derive(move || specimen(host.recipe.get().locale, SpecimenKey::ThemePlaceholder).to_owned())
                    help=Signal::derive(move || Some(specimen(host.recipe.get().locale, SpecimenKey::ThemeHelp).to_owned()))
                    error=Signal::derive(move || (state == ScenarioState::Error).then(|| specimen(host.recipe.get().locale, SpecimenKey::ThemeError).to_owned()))
                    on_input=Callback::new(move |value: String| { select_value.set(value.clone()); host.log(format!("{} input={value}", log_id.get_value())); })/>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{move || tool::controlled_value(&select_value.get())}</p>
                <Select id=language_id density=SelectDensity::Compact label_placement=SelectLabelPlacement::Inline label=Signal::derive(move || specimen(host.recipe.get().locale, SpecimenKey::Language).to_owned())
                    value=language_value state=choice_state
                    options=vec![SelectOption { lang: Some("vi-VN".to_owned()), ..SelectOption::new("vi-VN", VIETNAMESE_AUTONYM) },
                        SelectOption { lang: Some("en".to_owned()), ..SelectOption::new("en", ENGLISH_AUTONYM) }]
                    on_input=Callback::new(move |value: String| { language_value.set(value.clone()); host.log(format!("{} locale={value}", log_id.get_value())); })/>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::SELECT_DENSITIES}</p>
                <Select id=native_id density=SelectDensity::Compact label_placement=SelectLabelPlacement::Inline
                    label=Signal::derive(move || specimen(host.recipe.get().locale, SpecimenKey::Language).to_owned())
                    value=language_value state=choice_state
                    on_input=Callback::new(move |value: String| { language_value.set(value.clone()); host.log(format!("{} native={value}", log_id.get_value())); })>
                    <option value="vi-VN" lang="vi-VN">{VIETNAMESE_AUTONYM}</option>
                    <option value="en" lang="en">{ENGLISH_AUTONYM}</option>
                </Select>
                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::MENU_CANDIDATE}</p>
            }.into_any()
        }
        ComponentId::Dialog => {
            let dialog_id = format!("{id}-modal");
            let draft_id = format!("{id}-draft");
            view! {
                <Button id=control_id focus_on_press=true on_press=Callback::new(move |_| { opened.set(true); host.log(format!("{} open", log_id.get_value())); })>
                    {move || specimen(host.recipe.get().locale, SpecimenKey::OpenDialog)}
                </Button>
                <div lang=tool::LANGUAGE_TAG><Dialog id=dialog_id title=tool::SAMPLE_DIALOG close_label=tool::CLOSE_SAMPLE open=opened
                    on_close_request=Callback::new(move |reason| { opened.set(false); host.log(format!("{} close={reason:?}", log_id.get_value())); })
                    on_error=Callback::new(move |error| { opened.set(false); host.log(format!("{} error={error:?}", log_id.get_value())); })>
                    <div lang=move || host.recipe.get().locale.language_tag()><TextField id=draft_id label=Signal::derive(move || specimen(host.recipe.get().locale, SpecimenKey::PreservedText).to_owned()) value=text
                        on_input=Callback::new(move |value| { text.set(value); host.log(format!("{} input", log_id.get_value())); })/></div>
                </Dialog></div>
            }.into_any()
        }
    };
    view! { <article id=id class="cui-scenario" data-scenario-component=component.as_str() data-scenario-state=state.as_str()>
        <h2>{format!("{} / {}", component.as_str(), state.as_str())}</h2>{content}
    </article> }
}
