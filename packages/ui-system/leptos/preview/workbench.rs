//! Preview-only composition root. Recipes select production controls; no services are built.
use super::{motion_probe, platform, recipe::*, Gallery};
use cantos_ui::{
    Button, ButtonState, ButtonVariant, ChoiceState, Density, Dialog, DisabledReason, FilterChip,
    IconName, Select, Space, Stack, Switch, TextField, Theme, UiRoot,
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

fn copy(locale: Locale, vi: &'static str, en: &'static str) -> &'static str {
    match locale {
        Locale::Vi => vi,
        Locale::En => en,
    }
}

fn fixture_label(recipe: Recipe) -> &'static str {
    match (recipe.locale, recipe.fixture) {
        (Locale::Vi, Fixture::Standard) => "Ngày mai, mình có diễn tiếp không?",
        (Locale::En, Fixture::Standard) => "Shall we perform again tomorrow?",
        (Locale::Vi, Fixture::LongVietnamese) => "Nguyễn Thị Ánh Nguyệt — Ánh đèn cuối sân khấu và lời hẹn dưới chân Thanh Vân Sơn. Ngày mai, mình có diễn tiếp không?",
        (Locale::En, Fixture::LongVietnamese) => "Nguyễn Thị Ánh Nguyệt — The last stage light and a promise beneath Thanh Vân Sơn. Shall we perform again tomorrow?",
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
    let facts = RwSignal::new(String::from(
        "Browser facts not captured; refresh after fonts are ready.",
    ));
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
            Ok(()) => notice.set(
                "Metadata exported. Capture/open pixels and record input separately.".to_owned(),
            ),
            Err(error) => notice.set(format!("Export unavailable: {error:?}")),
        }
    });
    view! {
        <UiRoot theme=theme density=density>
            <main class="cui-workbench" lang="en" data-preview-workbench="v1">
                <header><p>"Cantos · Leptos · local diagnostics"</p><h1>"Scenario workbench"</h1>
                    <p>"Select a recipe, copy its link, drive real input, then capture and inspect. No network, identity or audio effects."</p>
                </header>
                {fallback.map(|message| view! { <p id="scenario-fallback" role="alert">{message}</p> })}
                <section aria-label="Scenario recipe" class="cui-recipe-controls">
                    <RecipeSelect id="recipe-component" label="Component / variant" values=ComponentId::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().component.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(component) = ComponentId::from_attribute(&value) {
                            recipe.update(|recipe| { recipe.component = component; recipe.state = component.default_state(); });
                        } })/>
                    <Select id="recipe-state" label="Initial state"
                        value=Signal::derive(move || recipe.get().state.as_str().to_owned())
                        on_input=Callback::new(move |value: String| { if let Some(state) = ScenarioState::from_attribute(&value) {
                            let mut next = recipe.get_untracked(); next.state = state;
                            match next.validate() { Ok(_) => { recipe.set(next); host.restore(); }, Err(error) => notice.set(format!("Incompatible state: {error:?}")) }
                        } })>
                        {move || state_component.get().states().iter().map(|state| view! { <option value=state.as_str()>{state.as_str()}</option> }).collect_view()}
                    </Select>
                    <RecipeSelect id="recipe-mode" label="View mode" values=PreviewMode::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().mode.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(mode) = PreviewMode::from_attribute(&value) { recipe.update(|recipe| recipe.mode = mode); } })/>
                    <RecipeSelect id="recipe-fixture" label="Fixture" values=Fixture::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().fixture.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(fixture) = Fixture::from_attribute(&value) { recipe.update(|recipe| recipe.fixture = fixture); host.restore(); } })/>
                    <RecipeSelect id="recipe-theme" label="Theme" values=vec!["system", "light", "dark"]
                        value=Signal::derive(move || theme.get().as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(value) = Theme::from_attribute(&value) { theme.set(value); } })/>
                    <RecipeSelect id="recipe-density" label="Density" values=vec!["compact", "comfortable"]
                        value=Signal::derive(move || density.get().as_str().to_owned())
                        on_change=Callback::new(move |value: String| density.set(if value == "comfortable" { Density::Comfortable } else { Density::Compact }))/>
                    <RecipeSelect id="recipe-locale" label="Specimen language" values=Locale::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().locale.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(locale) = Locale::from_attribute(&value) { recipe.update(|recipe| recipe.locale = locale); } })/>
                    <RecipeSelect id="recipe-text" label="Specimen text scale (%)" values=TextScale::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().text_scale.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(scale) = TextScale::from_attribute(&value) { recipe.update(|recipe| recipe.text_scale = scale); } })/>
                    <RecipeSelect id="recipe-frame" label="Frame width (CSS px, not viewport)" values=FrameWidth::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().frame.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(frame) = FrameWidth::from_attribute(&value) { recipe.update(|recipe| recipe.frame = frame); } })/>
                    <RecipeSelect id="recipe-motion" label="Motion (force-full overrides OS: opt-in diagnostic)" values=Motion::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().motion.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(motion) = Motion::from_attribute(&value) { recipe.update(|recipe| recipe.motion = motion); } })/>
                    <RecipeSelect id="recipe-playback" label="Review: replay resets, drive real input" values=Playback::ALL.iter().map(|v| v.as_str()).collect()
                        value=Signal::derive(move || recipe.get().playback.as_str().to_owned())
                        on_change=Callback::new(move |value: String| { if let Some(playback) = Playback::from_attribute(&value) { recipe.update(|recipe| recipe.playback = playback); if playback == Playback::Replay { host.restore(); } } })/>
                </section>
                <Stack direction=cantos_ui::StackDirection::Horizontal gap=Space::Sm>
                    <Button id="recipe-reset" on_press=Callback::new(move |_| host.restore())>"Reset to selected recipe"</Button>
                    <Button id="recipe-replay" on_press=Callback::new(move |_| { host.restore(); notice.set("Replay ready: press/release/cancel or use keyboard on the actual control. No synthetic pressed state.".to_owned()); })>"Replay input"</Button>
                    <Button id="recipe-facts" on_press=refresh>"Refresh browser / font facts"</Button>
                    <Button id="recipe-export" on_press=export>"Export metadata"</Button>
                </Stack>
                <p id="recipe-notice" role="status">{move || notice.get()}</p>
                <a id="recipe-link" href=move || recipe.get().encode_query()>"Copy scenario link / reload recipe"</a>
                <details><summary>"Recipe and capture provenance"</summary><code id="recipe-query">{move || recipe.get().encode_query()}</code><pre id="recipe-facts-output">{move || facts.get()}</pre></details>
                <p>"Native activation policy: click/Enter/Space; browser-owned slop, axis and cancel. No custom drag threshold or velocity. Dialog: Escape/close, native focus containment/restore. Sheet and mini↔full player gestures have no shared consumer here and remain unsupported."</p>
                <div class="cui-specimen-scroll" tabindex="0" aria-label="Specimen frame, horizontal scroll when wider than browser">
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
                <section aria-label="Real component intents"><h2>"Intent log"</h2><ol id="recipe-events" aria-live="polite">
                    {move || host.events.get().into_iter().map(|event| view! { <li>{event}</li> }).collect_view()}
                </ol></section>
                <section aria-label="Motion observations">
                    <h2>"Trusted input frame profile"</h2>
                    <TextField id="recipe-capture-label" label="Capture label (local diagnostic text)" value=capture_label on_input=Callback::new(move |value| capture_label.set(value))/>
                    <p>"Computed styles at start/middle/end; interruptions and unavailable middle frames stay explicit. This is a bounded observation, not a video or pixel review."</p>
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
    // Same mounted component across modes/themes. Only an explicit recipe/reset restores data.
    Effect::new(move |_| {
        host.reset.get();
        selected.set(matches!(state, ScenarioState::Selected | ScenarioState::On));
        text.set(fixture_label(host.recipe.get_untracked()).to_owned());
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
        DisabledReason::new(copy(
            host.recipe.get().locale,
            "Trạng thái mẫu bị khóa; đổi recipe để tiếp tục.",
            "This sample is blocked; change its recipe to continue.",
        ))
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
        ComponentId::Button | ComponentId::AuthPrimary => view! {
            <Button id=control_id variant=if component == ComponentId::AuthPrimary { ButtonVariant::AuthPrimary } else { ButtonVariant::Primary }
                icon=IconName::ArrowForward state=button_state on_press=press>{move || label.get()}</Button>
        }.into_any(),
        ComponentId::FilterChip => view! {
            <FilterChip id=control_id selected=selected state=button_state on_change=change>{move || label.get()}</FilterChip>
        }.into_any(),
        ComponentId::Switch => view! { <Switch id=control_id label=label checked=selected state=choice_state on_change=change/> }.into_any(),
        ComponentId::Dialog => {
            let dialog_id = format!("{id}-modal");
            let draft_id = format!("{id}-draft");
            view! {
                <Button id=control_id focus_on_press=true on_press=Callback::new(move |_| { opened.set(true); host.log(format!("{} open", log_id.get_value())); })>
                    {move || copy(host.recipe.get().locale, "Mở hộp thoại mẫu", "Open sample dialog")}
                </Button>
                <Dialog id=dialog_id title="Hộp thoại mẫu / Sample dialog" close_label="Đóng mẫu / Close sample" open=opened
                    on_close_request=Callback::new(move |reason| { opened.set(false); host.log(format!("{} close={reason:?}", log_id.get_value())); })
                    on_error=Callback::new(move |error| { opened.set(false); host.log(format!("{} error={error:?}", log_id.get_value())); })>
                    <TextField id=draft_id label=Signal::derive(move || copy(host.recipe.get().locale, "Văn bản được giữ", "Preserved text").to_owned()) value=text
                        on_input=Callback::new(move |value| { text.set(value); host.log(format!("{} input", log_id.get_value())); })/>
                </Dialog>
            }.into_any()
        }
    };
    view! { <article id=id class="cui-scenario" data-scenario-component=component.as_str() data-scenario-state=state.as_str()>
        <h2>{format!("{} / {}", component.as_str(), state.as_str())}</h2>{content}
    </article> }
}
