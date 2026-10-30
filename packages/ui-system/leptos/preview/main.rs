//! Local component diagnostics. Values and callbacks here never call a Cantos service.

use cantos_ui::{
    AccessibleName, Announcement, Avatar, AvatarInitials, AvatarSize, AvatarTone, Badge, Button,
    ButtonState, ButtonVariant, Checkbox, ChoiceState, Container, ContainerWidth, Density, Dialog,
    DisabledReason, EmptyState, ErrorState, FieldError, FieldHelp, FieldLabel, FieldState,
    FilterChip, Grid, GridColumns, Icon, IconButton, IconName, Link, LinkHref, LoadingState,
    NumberField, Radio, RangeField, Select, Space, Stack, StackDirection, Status, StatusTone,
    Surface, SurfaceTone, Switch, TextArea, TextField, TextFieldKind, Theme, UiRoot,
};
use leptos::prelude::*;
use messages::tool;

/// Offline sample picture; real hosts pass an authorized same-origin or object URL.
const PREVIEW_AVATAR_IMAGE: &str = "data:image/svg+xml,%3Csvg xmlns='http://www.w3.org/2000/svg' viewBox='0 0 64 64'%3E%3Crect width='64' height='64' fill='%23886638'/%3E%3Ccircle cx='32' cy='26' r='12' fill='%23f6e7ca'/%3E%3Cpath d='M12 60c2-14 12-20 20-20s18 6 20 20z' fill='%23f6e7ca'/%3E%3C/svg%3E";

pub mod auth_samples;
pub mod catalog;
pub mod design_samples;
mod discovery;
pub mod edge_progress_samples;
pub mod foundation_samples;
pub mod interaction_samples;
pub mod listen_samples;
mod logo_samples;
pub mod messages;
pub mod motion_probe;
pub mod platform;
#[path = "../src/preview_catalog.rs"]
pub mod preview_catalog;
pub mod recipe;
pub mod surface_pulse_samples;
pub mod theatre_samples;
pub mod token_sample;
pub mod workbench;

use auth_samples::{AuthActionSamples, AuthFieldSamples, AuthSurfaceSamples, BackButtonSamples};
use design_samples::{DesignInventory, PrototypeCompositions};
use discovery::{ComponentCatalog, FamilyNavigation, TokenCatalog};
use foundation_samples::{
    ComposedButtonSample, FieldFoundationSamples, FontReadinessSample, NestedThemeSample,
};
use interaction_samples::{LayoutFeedbackContractSamples, SelectionContractSamples};
use surface_pulse_samples::{PulseStage, SurfacePulseSamples};
use theatre_samples::TheatreExpressionSamples;

fn name(text: &str) -> AccessibleName {
    AccessibleName::new(text).expect("gallery fixture names are nonempty")
}

fn href(anchor: &str) -> LinkHref {
    LinkHref::new(anchor).expect("gallery fixture links are local anchors")
}

fn reason(text: &str) -> DisabledReason {
    DisabledReason::new(text).expect("gallery fixture reasons are nonempty")
}

/// App-owned composition for state, keyboard and responsive checks; not a production screen.
#[component]
pub fn Gallery(
    #[prop(default = "empty")] initial_feedback: &'static str,
    #[prop(default = "all")] initial_family: &'static str,
    #[prop(default = "")] initial_query: &'static str,
    #[prop(optional)] host_theme: Option<RwSignal<Theme>>,
    #[prop(optional)] host_density: Option<RwSignal<Density>>,
) -> impl IntoView {
    let theme = host_theme.unwrap_or_else(|| RwSignal::new(Theme::Light));
    let density = host_density.unwrap_or_else(|| RwSignal::new(Density::Compact));
    let large_text = RwSignal::new(false);
    let reduced_motion = RwSignal::new(false);
    let token_query = RwSignal::new(String::new());
    let action_mode = RwSignal::new("enabled".to_owned());
    let field_mode = RwSignal::new("editable".to_owned());
    let show_errors = RwSignal::new(false);
    let action_count = RwSignal::new(0u32);
    let input_count = RwSignal::new(0u32);
    let selection_count = RwSignal::new(0u32);
    let selected_chip = RwSignal::new(true);
    let checkbox_value = RwSignal::new(true);
    let switch_value = RwSignal::new(false);
    let radio_value = RwSignal::new("serif".to_owned());
    let select_value = RwSignal::new("son-ha".to_owned());
    let title_value =
        RwSignal::new("Sơn Hà Ký — Ánh đèn cuối sân khấu và lời hẹn dưới chân núi".to_owned());
    let search_value = RwSignal::new(String::new());
    let body_value = RwSignal::new("Nguyễn Thị Ánh Nguyệt khẽ nói: “Ta vẫn nhớ lời hẹn ấy, dù mây đã phủ kín con đường về Thanh Vân Sơn.”\nMột làn gió lướt qua hiên cũ; tiếng chuông còn ngân nơi xa.".to_owned());
    let number_value = RwSignal::new("0".to_owned());
    let range_value = RwSignal::new("0".to_owned());
    let composed_value = RwSignal::new("Một lời hẹn dưới chân Thanh Vân Sơn".to_owned());
    let composition_active = RwSignal::new(false);
    let composition_count = RwSignal::new(0u32);
    let feedback_mode = RwSignal::new(initial_feedback.to_owned());
    let dialog_open = RwSignal::new(false);
    let dialog_count = RwSignal::new(0u32);
    let dialog_text = RwSignal::new("Lời hẹn vẫn còn ở đây khi đóng rồi mở lại.".to_owned());
    let dialog_message = RwSignal::new(tool::DIALOG_NOT_OPENED.to_owned());
    let current_anchor = RwSignal::new("actions".to_owned());
    let catalog_family = RwSignal::new(catalog::family_key(initial_family));
    let catalog_query = RwSignal::new(initial_query.to_owned());

    let action_state = Signal::derive(move || match action_mode.get().as_str() {
        "disabled" => ButtonState::Disabled(reason(
            "Trạng thái mẫu bị khóa; chọn “Sẵn sàng” để thao tác.",
        )),
        "loading" => {
            ButtonState::Loading(reason("Đang chờ trong ví dụ; chọn “Sẵn sàng” để kết thúc."))
        }
        _ => ButtonState::Enabled,
    });
    let field_state = Signal::derive(move || match field_mode.get().as_str() {
        "readonly" => FieldState::ReadOnly,
        "disabled" => {
            FieldState::Disabled(reason("Trường mẫu bị khóa; dữ liệu đã nhập vẫn được giữ."))
        }
        "loading" => FieldState::Loading(reason(
            "Đang chờ trong ví dụ; dữ liệu đã nhập vẫn được giữ.",
        )),
        _ => FieldState::Editable,
    });
    let choice_state = Signal::derive(move || match field_mode.get().as_str() {
        "disabled" => ChoiceState::Disabled(reason(
            "Lựa chọn mẫu bị khóa; chọn “Có thể sửa” để thay đổi.",
        )),
        "loading" => ChoiceState::Loading(reason(
            "Lựa chọn đang chờ trong ví dụ; giá trị vẫn được giữ.",
        )),
        _ => ChoiceState::Enabled,
    });
    let error = Signal::derive(move || {
        show_errors.get().then(|| "Lỗi minh họa: hãy kiểm tra nội dung rồi tắt trạng thái lỗi. Văn bản của bạn vẫn được giữ.".to_owned())
    });
    let press = Callback::new(move |_| action_count.update(|count| *count += 1));

    view! {
        <UiRoot theme=theme density=density>
            <main class="cui-preview" lang=tool::LANGUAGE_TAG id="gallery-main"
                data-text-scale=move || if large_text.get() { "200" } else { "100" }
                data-motion=move || if reduced_motion.get() { "reduced" } else { "system" }>
                <Container width=ContainerWidth::Wide class="cui-gallery-page">
                    <Stack gap=Space::Lg>
                        <header class="cui-gallery-header">
                            <p class="cui-gallery-eyebrow">"Cantos · UI system · Leptos"</p>
                            <h1 lang=tool::LANGUAGE_TAG class="cui-preview-title">{tool::GALLERY_TITLE}</h1>
                            <p lang=tool::LANGUAGE_TAG>{tool::GALLERY_INTRO}</p>
                            <nav class="cui-gallery-index" aria-label=tool::GALLERY_NAV>
                                <a href="#gallery-settings">{tool::DISPLAY}</a>
                                <a href="#gallery-design">"Design v3"</a>
                                <a href="#gallery-catalog">{tool::EXISTING_API}</a>
                                <a href="#gallery-tokens">"Tokens"</a>
                            </nav>
                        </header>

                        <Surface tone=SurfaceTone::Container>
                            <section id="gallery-settings" data-sample-family="foundation" aria-labelledby="gallery-settings-heading">
                                <Stack>
                                    <h2 lang=tool::LANGUAGE_TAG id="gallery-settings-heading">{tool::SETTINGS}</h2>
                                    <Grid columns=GridColumns::Three>
                                        <div lang=tool::LANGUAGE_TAG><Select id="gallery-theme" label=tool::THEME
                                            value=Signal::derive(move || theme.get().as_str().to_owned())
                                            on_input=Callback::new(move |value: String| {
                                                if let Some(next) = Theme::from_attribute(&value) { theme.set(next); }
                                            })>
                                            <option value="light">{tool::LIGHT}</option>
                                            <option value="dark">{tool::DARK}</option>
                                            <option value="system">{tool::SYSTEM}</option>
                                        </Select></div>
                                        <Switch id="gallery-density" label=tool::COMFORTABLE
                                            checked=Signal::derive(move || density.get() == Density::Comfortable)
                                            on_change=Callback::new(move |value| density.set(if value { Density::Comfortable } else { Density::Compact }))
                                            help=Some(tool::DENSITY_HELP.to_owned())/>
                                        <Switch id="gallery-errors" label=tool::SHOW_ERRORS
                                            checked=show_errors on_change=Callback::new(move |value| show_errors.set(value))
                                            help=Some(tool::ERRORS_HELP.to_owned())/>
                                    </Grid>
                                    <Grid columns=GridColumns::Two>
                                        <Switch id="gallery-large-text" label=tool::TEXT_SCALE checked=large_text
                                            on_change=Callback::new(move |next| large_text.set(next))
                                            help=Some(tool::TEXT_SCALE_HELP.to_owned())/>
                                        <Switch id="gallery-reduced-motion" label=tool::REDUCED_MOTION checked=reduced_motion
                                            on_change=Callback::new(move |next| reduced_motion.set(next))
                                            help=Some(tool::MOTION_HELP.to_owned())/>
                                    </Grid>
                                    <details class="cui-foundation-details">
                                    <summary lang=tool::LANGUAGE_TAG>{tool::FONT_THEME}</summary>
                                    <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::FONT_HELP}</p>
                                    <div lang="vi-VN" id="gallery-font-proof" data-preview-scenario="noto-vietnamese">
                                        <p>"Noto Sans 400 · Nguyễn Thị Ánh Nguyệt · Ỗ, Ẫ, ặ · "<strong>"Noto Sans 700 · Người dẫn chuyện"</strong></p>
                                        <p class="cui-font-editorial">"Noto Serif 400 · Lời hẹn dưới chân Thanh Vân Sơn · "<strong>"Noto Serif 700 · Ỗ, Ẫ, ặ"</strong></p>
                                    </div>
                                    <FontReadinessSample/>
                                    <NestedThemeSample/>
                                    </details>
                                </Stack>
                            </section>
                        </Surface>

                        <DesignInventory token_query=token_query on_sample=Callback::new(move |family| {
                            catalog_family.set(family); catalog_query.set(String::new()); current_anchor.set(family.to_owned());
                        })/>
                        <ComponentCatalog query=catalog_query family=catalog_family current_anchor=current_anchor/>
                        <PrototypeCompositions/>
                        <TheatreExpressionSamples theme=theme density=density/>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-navigation" data-sample-family="navigation" data-preview-scenario="overflow keyboard disabled"
                            aria-labelledby="gallery-navigation-heading">
                            <Stack gap=Space::Sm>
                                <BackButtonSamples/>
                                <h2 lang=tool::LANGUAGE_TAG id="gallery-navigation-heading">{tool::NAVIGATION}</h2>
                                <FamilyNavigation current_anchor=current_anchor family=catalog_family query=catalog_query/>
                                <div lang=tool::LANGUAGE_TAG><Link id="gallery-link" href=href("#gallery-fields")
                                    on_press=Callback::new(move |_| { catalog_family.set("fields"); catalog_query.set(String::new()); current_anchor.set("fields".to_owned()); })>{tool::GO_FIELDS}</Link></div>
                                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::NAV_HELP}</p>
                            </Stack>
                        </section>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-actions" class="cui-gallery-family" data-sample-family="actions" aria-labelledby="gallery-actions-heading"
                            hidden=move || !catalog::family_matches("actions", catalog_family.get(), &catalog_query.get())>
                            <Stack>
                                <AuthActionSamples/>
                                    <h2 lang=tool::LANGUAGE_TAG id="gallery-actions-heading">{tool::ACTIONS}</h2>
                                <div lang=tool::LANGUAGE_TAG><Select id="gallery-action-state" label=tool::ACTION_STATE
                                    value=action_mode on_input=Callback::new(move |value| action_mode.set(value))>
                                    <option value="enabled">{tool::READY}</option>
                                    <option value="disabled">{tool::DISABLED_REASON}</option>
                                    <option value="loading">{tool::LOADING_REASON}</option>
                                </Select></div>
                                <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                                    <Button id="gallery-primary" state=action_state on_press=press icon=IconName::Save>
                                        "Lưu bản nháp mẫu"
                                    </Button>
                                    <Button id="gallery-tonal" variant=ButtonVariant::Tonal icon=IconName::Mic state=action_state on_press=press>"Tạo giọng đọc mẫu"</Button>
                                    <Button id="gallery-secondary" variant=ButtonVariant::Secondary icon=IconName::CloudSync state=action_state on_press=press>"Đồng bộ bản nháp mẫu"</Button>
                                    <Button id="gallery-text" variant=ButtonVariant::Text icon=IconName::Refresh state=action_state on_press=press>"Thử lại mẫu"</Button>
                                    <Button id="gallery-danger" variant=ButtonVariant::Danger icon=IconName::DeleteOutline state=action_state on_press=press>"Xóa bản nháp mẫu"</Button>
                                </Stack>
                                <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                                    <IconButton id="gallery-icon-action" name=name("Tìm trong phòng mẫu") icon=IconName::Search state=action_state on_press=press/>
                                    <FilterChip id="gallery-filter" selected=selected_chip state=action_state
                                        on_change=Callback::new(move |value| { selected_chip.set(value); selection_count.update(|count| *count += 1); })>
                                        "Truyện huyền huyễn và lời kể dưới chân Thanh Vân Sơn"
                                    </FilterChip>
                                </Stack>
                                <Status tone=StatusTone::Info announcement=Announcement::Polite>
                                    <p lang=tool::LANGUAGE_TAG class="cui-gallery-output" id="gallery-action-output">
                                        {move || format!("Button intents: {} · Filter: {} · State: {}", action_count.get(), if selected_chip.get() { "selected" } else { "unselected" }, action_mode.get())}
                                    </p>
                                </Status>
                                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::NATIVE_HELP}</p>
                                <ComposedButtonSample state=action_state/>
                            </Stack>
                        </section>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-fields" data-preview-scenario="label-activation" class="cui-gallery-family" data-sample-family="fields" aria-labelledby="gallery-fields-heading"
                            hidden=move || !catalog::family_matches("fields", catalog_family.get(), &catalog_query.get())>
                            <Surface tone=SurfaceTone::Plain>
                                <Stack>
                                    <AuthFieldSamples/>
                                    <h2 lang=tool::LANGUAGE_TAG id="gallery-fields-heading">{tool::FIELDS}</h2>
                                    <div lang=tool::LANGUAGE_TAG><Select id="gallery-field-state" label=tool::FIELD_STATE
                                        value=field_mode on_input=Callback::new(move |value| field_mode.set(value))>
                                        <option value="editable">{tool::EDITABLE}</option>
                                        <option value="readonly">{tool::READONLY}</option>
                                        <option value="disabled">{tool::DISABLED_REASON}</option>
                                        <option value="loading">{tool::LOADING_REASON}</option>
                                    </Select></div>
                                    <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::FIELD_HELP}</p>
                                    <Grid columns=GridColumns::Two>
                                        <TextField id="gallery-title" label="Tên dài có dấu tiếng Việt" value=title_value
                                            state=field_state error=error required=true
                                            help=Some("Thử dấu tiếng Việt, ký tự < > & và văn bản dài; component giữ nguyên giá trị.".to_owned())
                                            on_composition_start=Callback::new(move |_| composition_active.set(true))
                                            on_composition_end=Callback::new(move |_| { composition_active.set(false); composition_count.update(|count| *count += 1); })
                                            on_input=Callback::new(move |value| { title_value.set(value); input_count.update(|count| *count += 1); })/>
                                        <TextField id="gallery-search" label="Tìm nội dung mẫu" kind=TextFieldKind::Search value=search_value
                                            state=field_state error=error placeholder="Ví dụ: Nguyễn Thị Ánh Nguyệt"
                                            help=Some("Trường tìm kiếm native; placeholder không thay cho nhãn.".to_owned())
                                            on_input=Callback::new(move |value| { search_value.set(value); input_count.update(|count| *count += 1); })/>
                                    </Grid>
                                    <TextArea id="gallery-textarea" label="Đoạn văn dài — chọn và sửa theo ý bạn" value=body_value
                                        state=field_state error=error rows=4
                                        help=Some("Văn bản mẫu do repository tạo; không gửi dữ liệu đến dịch vụ.".to_owned())
                                        on_input=Callback::new(move |value| { body_value.set(value); input_count.update(|count| *count += 1); })/>
                                    <Grid columns=GridColumns::Two>
                                        <NumberField id="gallery-number" label="Giá trị số — bắt đầu bằng 0" value=number_value
                                            state=field_state error=error min="0" max="100" step="1"
                                            help=Some("Thử nhập 0, xóa về rỗng rồi nhập lại. Không dùng giá trị mặc định vì chuỗi “0”.".to_owned())
                                            on_input=Callback::new(move |value| { number_value.set(value); input_count.update(|count| *count += 1); })/>
                                        <RangeField id="gallery-range" label="Range — 0 đến 100" value=range_value
                                            state=choice_state error=error min="0" max="100" step="1"
                                            value_text=Signal::derive(move || Some(format!("{} trên 100", range_value.get())))
                                            help=Some("Thử phím mũi tên, Home và End; giá trị ban đầu bằng 0.".to_owned())
                                            on_input=Callback::new(move |value| { range_value.set(value); input_count.update(|count| *count += 1); })/>
                                    </Grid>
                                    <Grid columns=GridColumns::Two>
                                        <Select id="gallery-select" label="Tác phẩm đại diện" value=select_value
                                            state=choice_state error=error
                                            on_input=Callback::new(move |value| { select_value.set(value); selection_count.update(|count| *count += 1); })>
                                            <option value="son-ha">"Sơn Hà Ký — Ánh đèn cuối sân khấu"</option>
                                            <option value="thanh-van">"Lời hẹn dưới chân Thanh Vân Sơn và hành trình trở về của Nguyễn Thị Ánh Nguyệt"</option>
                                            <option value="night">"Đêm gió qua hiên cũ"</option>
                                        </Select>
                                        <Stack gap=Space::Sm>
                                            <Checkbox id="gallery-checkbox" label="Giữ lựa chọn khi chuyển trạng thái mẫu"
                                                checked=checkbox_value state=choice_state error=error
                                                on_change=Callback::new(move |value| { checkbox_value.set(value); selection_count.update(|count| *count += 1); })/>
                                            <Switch id="gallery-switch" label="Chế độ đọc yên tĩnh"
                                                checked=switch_value state=choice_state error=error
                                                on_change=Callback::new(move |value| { switch_value.set(value); selection_count.update(|count| *count += 1); })/>
                                        </Stack>
                                    </Grid>
                                    <fieldset class="cui-gallery-fieldset">
                                        <legend>"Lựa chọn kiểu chữ mẫu — nhóm radio native"</legend>
                                        <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                                            <Radio id="gallery-radio-serif" name="gallery-typeface" value="serif"
                                                label="Chữ có chân cho vùng đọc" state=choice_state error=error
                                                checked=Signal::derive(move || radio_value.get() == "serif")
                                                on_change=Callback::new(move |checked| { if checked { radio_value.set("serif".to_owned()); selection_count.update(|count| *count += 1); } })/>
                                            <Radio id="gallery-radio-sans" name="gallery-typeface" value="sans"
                                                label="Chữ không chân cho điều khiển" state=choice_state error=error
                                                checked=Signal::derive(move || radio_value.get() == "sans")
                                                on_change=Callback::new(move |checked| { if checked { radio_value.set("sans".to_owned()); selection_count.update(|count| *count += 1); } })/>
                                        </Stack>
                                    </fieldset>
                                    <div class="cui-field">
                                        <FieldLabel for_id="gallery-composed">"Nhãn, help và error rời cho control do app ghép"</FieldLabel>
                                        <input id="gallery-composed" class="cui-field-control" type="text"
                                            value=move || composed_value.get() prop:value=move || composed_value.get()
                                            aria-describedby=move || if show_errors.get() { "gallery-composed-help gallery-composed-error" } else { "gallery-composed-help" }
                                            aria-invalid=move || show_errors.get().to_string()
                                            on:input=move |event| { composed_value.set(event_target_value(&event)); input_count.update(|count| *count += 1); }/>
                                        <FieldHelp id="gallery-composed-help">"Nhãn liên kết bằng stable ID. Ví dụ ghép này luôn cho phép sửa."</FieldHelp>
                                        <div hidden=move || !show_errors.get()>
                                            <FieldError id="gallery-composed-error">"Lỗi minh họa do app chọn thời điểm hiển thị."</FieldError>
                                        </div>
                                    </div>
                                    <output lang=tool::LANGUAGE_TAG id="gallery-input-output" class="cui-gallery-output" aria-live="off">
                                        {move || format!("Input events: {} · Selection events: {}\nNumber = {:?} · Range = {:?} · Select = {}\nCheckbox = {} · Switch = {} · Radio = {}\nIME composing = {} · compositionend count = {}\nCurrent title: {}", input_count.get(), selection_count.get(), number_value.get(), range_value.get(), select_value.get(), checkbox_value.get(), switch_value.get(), radio_value.get(), composition_active.get(), composition_count.get(), title_value.get())}
                                    </output>
                                    <FieldFoundationSamples state=field_state/>
                                    <SelectionContractSamples/>
                                </Stack>
                            </Surface>
                        </section>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-feedback" class="cui-gallery-family" data-sample-family="feedback" aria-labelledby="gallery-feedback-heading"
                            hidden=move || !catalog::family_matches("feedback", catalog_family.get(), &catalog_query.get())>
                            <Stack>
                                <h2 lang=tool::LANGUAGE_TAG id="gallery-feedback-heading">{tool::FEEDBACK}</h2>
                                <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                                    <Badge>"Trung tính"</Badge>
                                    <Badge tone=StatusTone::Info>"Thông tin"</Badge>
                                    <Badge tone=StatusTone::Success>"Thành công"</Badge>
                                    <Badge tone=StatusTone::Warning>"Cần chú ý"</Badge>
                                    <Badge tone=StatusTone::Error>"Có lỗi"</Badge>
                                </Stack>
                                <Grid columns=GridColumns::Two>
                                    <Status>"Trạng thái trung tính có văn bản và biểu tượng."</Status>
                                    <Status tone=StatusTone::Info>"Thông tin mẫu: dữ liệu ở trong tab này."</Status>
                                    <Status tone=StatusTone::Success>"Ví dụ thành công; không biểu thị đã lưu lên máy chủ."</Status>
                                    <Status tone=StatusTone::Warning>"Ví dụ cảnh báo với giải thích đủ dài để thử xuống dòng trên màn hình hẹp."</Status>
                                    <Status tone=StatusTone::Error>"Ví dụ lỗi: nội dung đã nhập vẫn được giữ."</Status>
                                </Grid>
                                <div lang=tool::LANGUAGE_TAG><Select id="gallery-feedback-state" label=tool::FEEDBACK_STATE value=feedback_mode
                                    on_input=Callback::new(move |value| feedback_mode.set(value))>
                                    <option value="empty">{tool::EMPTY}</option>
                                    <option value="loading">{tool::LOADING}</option>
                                    <option value="announcement">{tool::LOADING_ANNOUNCEMENT}</option>
                                    <option value="pulse">{tool::SURFACE_PULSE}</option>
                                    <option value="pulse-idle">{tool::SURFACE_PULSE_IDLE}</option>
                                    <option value="pulse-ready">{tool::SURFACE_PULSE_READY}</option>
                                    <option value="error">{tool::ERROR}</option>
                                    <option value="success">{tool::SUCCESS}</option>
                                </Select></div>
                                <div aria-live="polite" aria-atomic="true">
                                    {move || match feedback_mode.get().as_str() {
                                        "loading" => view! { <LoadingState>"Đang chờ trong ví dụ. Chọn trạng thái khác để kết thúc; không có yêu cầu mạng."</LoadingState> }.into_any(),
                                        "announcement" => view! {
                                            <div data-preview-scenario="announcement">
                                                <LoadingState announcement=Announcement::Polite>"Thông báo đang tải được bật rõ ràng; nội dung vẫn được giữ."</LoadingState>
                                            </div>
                                        }.into_any(),
                                        messages::pulse::MODE_RUNNING => view! { <SurfacePulseSamples stage=PulseStage::Running/> }.into_any(),
                                        messages::pulse::MODE_IDLE => view! { <SurfacePulseSamples stage=PulseStage::Idle/> }.into_any(),
                                        messages::pulse::MODE_READY => view! { <SurfacePulseSamples stage=PulseStage::Ready/> }.into_any(),
                                        "error" => view! { <ErrorState title="Không hiển thị được nội dung mẫu">
                                            <p>"Đây là lỗi minh họa. Bạn có thể chọn thử lại và giữ nguyên mọi giá trị đã nhập."</p>
                                            <Button id="gallery-retry" variant=ButtonVariant::Secondary
                                                on_press=Callback::new(move |_| feedback_mode.set("success".to_owned()))>"Thử lại trong bản mẫu"</Button>
                                        </ErrorState> }.into_any(),
                                        "success" => view! { <Status tone=StatusTone::Success>"Vùng mẫu đã đổi sang thành công. Không có thay đổi nào trên dịch vụ Cantos."</Status> }.into_any(),
                                        _ => view! { <EmptyState title="Chưa có nội dung trong vùng mẫu">
                                            <p>"Trạng thái rỗng cần giải thích và một hành động phù hợp."</p>
                                            <Button id="gallery-empty-action" variant=ButtonVariant::Tonal
                                                on_press=Callback::new(move |_| feedback_mode.set("success".to_owned()))>"Hiển thị nội dung mẫu"</Button>
                                        </EmptyState> }.into_any(),
                                    }}
                                </div>
                            </Stack>
                        </section>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-layout" data-preview-scenario="gap-lg inset-none" class="cui-gallery-family" data-sample-family="layout" aria-labelledby="gallery-layout-heading"
                            hidden=move || !catalog::family_matches("layout", catalog_family.get(), &catalog_query.get())>
                            <Stack>
                                <LayoutFeedbackContractSamples/>
                                    <AuthSurfaceSamples/>
                                    <h2 lang=tool::LANGUAGE_TAG id="gallery-layout-heading">{tool::LAYOUT}</h2>
                                <Grid columns=GridColumns::Three>
                                    <Surface tone=SurfaceTone::Plain inset=Space::Sm><p>"Surface · Plain · Sm"</p><p>"Một nhóm yên tĩnh, không mặc định thêm viền."</p></Surface>
                                    <Surface tone=SurfaceTone::Container inset=Space::Md><p>"Surface · Container · Md"</p><p>"Grid tối đa ba cột tự chuyển theo không gian khả dụng."</p></Surface>
                                    <Surface tone=SurfaceTone::High inset=Space::Lg><p>"Surface · High · Lg"</p><p>"Stack ngang có thể xuống dòng; gap dùng token."</p></Surface>
                                </Grid>
                                <Stack direction=StackDirection::Horizontal gap=Space::Sm>
                                    <Grid columns=GridColumns::Two gap=Space::Sm>
                                        <Surface tone=SurfaceTone::Container><p>"Grid trong Stack ngang: Nguyễn Thị Ánh Nguyệt — lời hẹn chưa thành dưới chân Thanh Vân Sơn."</p></Surface>
                                        <Surface tone=SurfaceTone::High><p>"Một nhóm lồng ghép vẫn nhận toàn bộ chiều rộng khả dụng; thử xuống 320px và phóng chữ."</p></Surface>
                                    </Grid>
                                </Stack>
                                <Container width=ContainerWidth::Reading>
                                    <Surface tone=SurfaceTone::Reading>
                                        <article aria-labelledby="gallery-reading-title">
                                            <h3 id="gallery-reading-title">"Sơn Hà Ký — Lời hẹn dưới chân Thanh Vân Sơn"</h3>
                                            <p>"Khi ánh đèn cuối sân khấu dịu lại, Nguyễn Thị Ánh Nguyệt dừng bước trước hiên cũ. Cô nhớ từng tiếng chuông, từng khoảng lặng và lời hẹn chưa thành giữa mùa gió năm ấy."</p>
                                            <p>"Vùng đọc giữ nhịp chữ yên tĩnh. Hãy thử theme tối, mật độ thoải mái và phóng chữ 200% để quan sát tiếng Việt có dấu, xuống dòng và khoảng cách giữa các ý."</p>
                                        </article>
                                    </Surface>
                                </Container>
                                <Container width=ContainerWidth::Fluid>
                                    <Surface inset=Space::None>
                                        <Stack gap=Space::None>
                                            <p class="cui-gallery-note">"Container · Fluid / Surface · None / Stack · None"</p>
                                            <Grid columns=GridColumns::One gap=Space::Xs>
                                                <p>"Grid · One · Xs: Thanh Vân Sơn — Nguyễn Thị Ánh Nguyệt — Trần Quốc Thiên Dương."</p>
                                            </Grid>
                                        </Stack>
                                    </Surface>
                                </Container>
                            </Stack>
                        </section>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-icons" class="cui-gallery-family" data-sample-family="icons" aria-labelledby="gallery-icons-heading"
                            hidden=move || !catalog::family_matches("icons", catalog_family.get(), &catalog_query.get())>
                            <Stack gap=Space::Sm>
                                <logo_samples::LogoSamples/>
                                <h2 lang=tool::LANGUAGE_TAG id="gallery-icons-heading">{tool::ICONS}</h2>
                                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::ICONS_HELP}</p>
                                <Stack direction=StackDirection::Horizontal gap=Space::Xs class="cui-gallery-icons">
                                    {IconName::ALL.iter().copied().map(|icon| view! {
                                        <span><Icon name=icon/>{icon.as_str()}</span>
                                    }).collect_view()}
                                </Stack>
                                <h3 lang=tool::LANGUAGE_TAG>{tool::AVATARS}</h3>
                                <p lang=tool::LANGUAGE_TAG class="cui-gallery-note">{tool::AVATARS_HELP}</p>
                                <Stack direction=StackDirection::Horizontal gap=Space::Sm class="cui-gallery-avatars">
                                    <span data-avatar-state="initials-single"><Avatar display_name="Nguyễn Thị Ánh" stable_id=Some("user-42".to_owned())/>"Nguyễn Thị Ánh"</span>
                                    <span data-avatar-state="generated-alias">
                                        <Stack gap=Space::Sm>
                                            <span>{tool::AVATAR_LAVENDER}<Avatar display_name="Mây Lam" initials=AvatarInitials::Pair size=AvatarSize::Large tone=AvatarTone::Lavender/><Avatar display_name="Mây Lam" initials=AvatarInitials::Pair size=AvatarSize::Control tone=AvatarTone::Lavender/></span>
                                            <span>{tool::AVATAR_GOLD}<Avatar display_name="Mây Lam" initials=AvatarInitials::Pair size=AvatarSize::Large tone=AvatarTone::Gold/><Avatar display_name="Mây Lam" initials=AvatarInitials::Pair size=AvatarSize::Control tone=AvatarTone::Gold/></span>
                                            <span>{tool::AVATAR_PERSONAL}<Avatar display_name="Mây Lam" stable_id=Some("6e45675a6ee0e0dd5a16f55c6fcaafd1f8737a27c0572ffc39a2a1bd22100ecb".to_owned()) initials=AvatarInitials::Pair size=AvatarSize::Large/><Avatar display_name="Mây Lam" stable_id=Some("6e45675a6ee0e0dd5a16f55c6fcaafd1f8737a27c0572ffc39a2a1bd22100ecb".to_owned()) initials=AvatarInitials::Pair size=AvatarSize::Control/></span>
                                        </Stack>
                                    </span>
                                    <span data-avatar-state="initials-pair"><Avatar display_name="Người dẫn chuyện" initials=AvatarInitials::Pair/>"Người dẫn chuyện"</span>
                                    <span data-avatar-state="no-initial"><Avatar display_name="😀"/>"Không có chữ cái"</span>
                                    <span data-avatar-state="image"><Avatar display_name="Đỗ Văn An" image_src=Some(PREVIEW_AVATAR_IMAGE.to_owned())/>"Đỗ Văn An"</span>
                                    <span data-avatar-state="image-failed"><Avatar display_name="Đỗ Văn An" image_src=Some("data:image/png;base64,AAAA".to_owned())/>"Ảnh lỗi"</span>
                                    <span data-avatar-state="sizes">
                                        <Avatar display_name="Lan" size=AvatarSize::Small/>
                                        <Avatar display_name="Lan" size=AvatarSize::Medium/>
                                        <Avatar display_name="Lan" size=AvatarSize::Large/>
                                        "Ba kích thước"
                                    </span>
                                    <span data-avatar-state="named"><Avatar display_name="岑 昕" initials=AvatarInitials::Pair
                                        accessible_name=AccessibleName::new("Ảnh đại diện của 岑 昕").expect("preview name")/></span>
                                </Stack>
                            </Stack>
                        </section>

                        <section lang="vi-VN" data-preview-fixture="vietnamese" id="gallery-dialog-samples" data-preview-scenario="retained-input" class="cui-gallery-family" data-sample-family="dialog" aria-labelledby="gallery-dialog-heading"
                            hidden=move || !catalog::family_matches("dialog", catalog_family.get(), &catalog_query.get())>
                            <Surface tone=SurfaceTone::Container inset=Space::Xs>
                                <Stack gap=Space::Sm>
                                    <h2 lang=tool::LANGUAGE_TAG id="gallery-dialog-heading">{tool::DIALOG}</h2>
                                    <p lang=tool::LANGUAGE_TAG>{tool::DIALOG_HELP}</p>
                                    <Button id="gallery-dialog-open" variant=ButtonVariant::Tonal
                                        focus_on_press=true
                                        aria_controls="gallery-dialog"
                                        on_press=Callback::new(move |_| {
                                            dialog_count.update(|count| *count += 1);
                                            dialog_message.set(tool::DIALOG_REQUEST_OPEN.to_owned());
                                            dialog_open.set(true);
                                        })>"Mở hộp thoại mẫu"</Button>
                                    <Status tone=StatusTone::Info announcement=Announcement::Polite>
                                        <p lang=tool::LANGUAGE_TAG id="gallery-dialog-output" class="cui-gallery-output">
                                            {move || tool::dialog_events(dialog_count.get(), &dialog_message.get())}
                                        </p>
                                    </Status>
                                </Stack>
                            </Surface>
                        </section>

                        <TokenCatalog query=token_query/>

                        <footer class="cui-gallery-note">
                            <p lang=tool::LANGUAGE_TAG>{tool::FOOTER}</p>
                            <Link id="gallery-back-to-top" href=href("#gallery-main")
                                on_press=Callback::new(move |_| current_anchor.set(String::new()))>{tool::TOP}</Link>
                        </footer>
                    </Stack>
                </Container>
                <div lang="vi-VN" data-preview-fixture="vietnamese"><Dialog id="gallery-dialog" title="Một lời hẹn dưới chân Thanh Vân Sơn — hộp thoại mẫu"
                    close_label="Đóng hộp thoại mẫu" open=dialog_open
                    description="Ví dụ kiểm tra focus và giữ dữ liệu nhập. Không có hành động lưu hoặc gửi đến dịch vụ."
                    on_close_request=Callback::new(move |close_reason| {
                        dialog_open.set(false);
                        dialog_message.set(tool::dialog_closed(close_reason));
                    })
                    on_error=Callback::new(move |error| {
                        dialog_open.set(false);
                        dialog_message.set(tool::dialog_error(error));
                    })
                    actions=ViewFn::from(move || view! {
                        <Button id="gallery-dialog-done" variant=ButtonVariant::Secondary
                            on_press=Callback::new(move |_| {
                                dialog_open.set(false);
                                dialog_message.set(tool::DIALOG_FINISHED.to_owned());
                            })>"Hoàn tất ví dụ"</Button>
                    })>
                    <TextField id="gallery-dialog-text" label="Văn bản được giữ sau khi đóng" value=dialog_text
                        on_input=Callback::new(move |value| dialog_text.set(value))/>
                </Dialog></div>
            </main>
        </UiRoot>
    }
}

#[cfg(target_arch = "wasm32")]
fn main() {
    leptos::mount::mount_to_body(|| view! { <workbench::Workbench/> });
}

#[cfg(not(target_arch = "wasm32"))]
fn main() {}
