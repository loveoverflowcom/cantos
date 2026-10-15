//! Preview-owned native editing diagnostics; no new shared Web component API.

use cantos_ui::{
    Button, ButtonState, ButtonVariant, Density, FieldHelp, FieldLabel, FieldState, Grid,
    GridColumns, IconName, NumberField, Select, Space, Stack, Surface, SurfaceTone, Switch,
    TextArea, TextField, Theme, UiRoot,
};
use leptos::{html, prelude::*};

/// Browser-only observation owned by the preview composition root, never a shared control.
#[component]
pub fn FontReadinessSample() -> impl IntoView {
    let observation = RwSignal::new("NOT_RUN: chưa kiểm tra font trong browser.".to_owned());
    view! {
        <Stack gap=Space::Sm>
            <Button id="gallery-font-check" variant=ButtonVariant::Text on_press=Callback::new(move |_| observation.set(font_observation()))>"Kiểm tra bốn font Noto đã tải"</Button>
            <output id="gallery-font-readiness" role="status">{move || observation.get()}</output>
        </Stack>
    }
}

#[cfg(target_arch = "wasm32")]
fn font_observation() -> String {
    let Some(document) = leptos::web_sys::window().and_then(|window| window.document()) else {
        return "UNSUPPORTED: browser document không có sẵn.".to_owned();
    };
    use leptos::wasm_bindgen::{closure::Closure, JsCast};
    use std::{cell::RefCell, rc::Rc};
    use web_sys::{FontFace, FontFaceLoadStatus};

    let observed = Rc::new(RefCell::new(Vec::new()));
    let collected = Rc::clone(&observed);
    let callback = Closure::wrap(Box::new(move |face: FontFace| {
        if let Ok(mut records) = collected.try_borrow_mut() {
            records.push((
                face.family().trim_matches('"').to_owned(),
                face.weight(),
                face.status(),
            ));
        }
    }) as Box<dyn FnMut(FontFace)>);
    if document
        .fonts()
        .for_each(callback.as_ref().unchecked_ref())
        .is_err()
    {
        return "UNSUPPORTED: không đọc được FontFaceSet.".to_owned();
    }
    let Ok(faces) = observed.try_borrow() else {
        return "UNSUPPORTED: không đọc được font observations.".to_owned();
    };
    let mut records = Vec::new();
    for family in ["Noto Sans", "Noto Serif"] {
        for weight in ["400", "700"] {
            let result = match faces
                .iter()
                .find(|(name, face_weight, _)| name == family && face_weight == weight)
            {
                Some((_, _, FontFaceLoadStatus::Loaded)) => "loaded",
                Some((_, _, FontFaceLoadStatus::Error)) => "FAIL: font tải lỗi",
                Some(_) => "NOT_RUN: chưa tải xong",
                None => "UNSUPPORTED: chưa khai báo face",
            };
            records.push(format!("{family} {weight}: {result}"));
        }
    }
    records.join(" · ")
}

#[cfg(not(target_arch = "wasm32"))]
fn font_observation() -> String {
    "NOT_RUN: font readiness cần browser thật.".to_owned()
}

#[component]
pub fn NestedThemeSample() -> impl IntoView {
    let dark = RwSignal::new(true);
    let value = RwSignal::new("Nguyễn Thị Ánh Nguyệt — lời hẹn còn nguyên".to_owned());
    view! {
        <div data-preview-scenario="nested-theme">
            <Stack gap=Space::Sm>
                <h3>"Theme và mật độ có phạm vi riêng"</h3>
                <Switch id="gallery-nested-theme-toggle" label="Vùng lồng dùng theme tối"
                    checked=dark on_change=Callback::new(move |next| dark.set(next))/>
                <UiRoot theme=Signal::derive(move || if dark.get() { Theme::Dark } else { Theme::Light })
                    density=Density::Comfortable class="cui-gallery-nested-root">
                    <Surface tone=SurfaceTone::Container>
                        <TextField id="gallery-nested-theme-value" label="Giá trị trong vùng theme lồng"
                            value=value on_input=Callback::new(move |next| value.set(next))
                            help=Some("Đổi theme của vùng lồng hoặc gallery; nội dung đã nhập vẫn được giữ.".to_owned())/>
                    </Surface>
                </UiRoot>
                <Surface tone=SurfaceTone::Container>
                    <p id="gallery-outer-theme-sibling">"Vùng kế tiếp tiếp tục dùng theme và mật độ của gallery."</p>
                </Surface>
            </Stack>
        </div>
    }
}

#[component]
pub fn ComposedButtonSample(#[prop(into)] state: Signal<ButtonState>) -> impl IntoView {
    let count = RwSignal::new(0u32);
    let press = composed_press(state, count);
    view! {
        <div data-preview-scenario="composed-content">
            <Stack gap=Space::Sm>
                <Button id="gallery-composed-button" variant=ButtonVariant::Tonal state=state on_press=Callback::new(move |_| press.run(())) icon=IconName::MenuBook>
                    <span>"Đọc lời hẹn dưới chân Thanh Vân Sơn"</span>
                </Button>
                <output id="gallery-composed-button-output" class="cui-gallery-output" aria-live="off">
                    {move || format!("Số lần nhận hành động có nội dung ghép: {}", count.get())}
                </output>
            </Stack>
        </div>
    }
}

pub fn composed_press(state: Signal<ButtonState>, count: RwSignal<u32>) -> Callback<()> {
    Callback::new(move |_| {
        if state.get_untracked().can_press() {
            count.update(|value| *value += 1);
        }
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NativeFieldKind {
    Text,
    Area,
    Number,
}

impl NativeFieldKind {
    fn key(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::Area => "area",
            Self::Number => "number",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Self::Text => "TextField — chọn văn bản và Enter",
            Self::Area => "TextArea — chọn văn bản và Enter giữ xuống dòng",
            Self::Number => "NumberField — Enter với hành vi số native",
        }
    }
}

/// Enter must not commit while a composition or a locked editing state is active.
pub fn accepts_ime_action(state: &FieldState, key: &str, composing: bool, key_code: u32) -> bool {
    state.accepts_input() && key == "Enter" && !composing && key_code != 229
}

#[component]
pub fn FieldFoundationSamples(#[prop(into)] state: Signal<FieldState>) -> impl IntoView {
    let error_mode = RwSignal::new("blank".to_owned());
    let error = Signal::derive(move || match error_mode.get().as_str() {
        "visible" => Some("Lỗi minh họa có nội dung; giá trị nhập vẫn được giữ.".to_owned()),
        "blank" => Some(" \n\t ".to_owned()),
        _ => None,
    });
    let text = RwSignal::new("Ánh Nguyệt chờ dưới hiên cũ".to_owned());
    let area = RwSignal::new("Lời hẹn vẫn còn.\nGió nhẹ qua Thanh Vân Sơn.".to_owned());
    let number = RwSignal::new("0".to_owned());
    view! {
        <Stack>
            <div data-preview-scenario="blank-error">
                <Stack gap=Space::Sm>
                    <h3>"Lỗi rỗng không làm trường nhập thành invalid"</h3>
                    <Select id="gallery-blank-error-state" label="Nội dung lỗi minh họa"
                        value=error_mode on_input=Callback::new(move |next| error_mode.set(next))>
                        <option value="none">"Không có lỗi"</option>
                        <option value="blank">"Chỉ có khoảng trắng"</option>
                        <option value="visible">"Có nội dung lỗi"</option>
                    </Select>
                    <Grid columns=GridColumns::Two>
                        <TextField id="gallery-blank-error-text" label="TextField với lỗi có kiểm tra nội dung"
                            value=text state=state error=error on_input=Callback::new(move |next| text.set(next))/>
                        <NumberField id="gallery-blank-error-number" label="NumberField với lỗi có kiểm tra nội dung"
                            value=number state=state error=error min="0" max="100" step="1"
                            on_input=Callback::new(move |next| number.set(next))/>
                    </Grid>
                    <TextArea id="gallery-blank-error-area" label="TextArea với lỗi có kiểm tra nội dung"
                        value=area state=state error=error rows=2 on_input=Callback::new(move |next| area.set(next))/>
                </Stack>
            </div>
            <div data-preview-scenario="selection ime-action">
                <Stack gap=Space::Sm>
                    <h3>"Lựa chọn và hành động bàn phím do bản mẫu quan sát"</h3>
                    <p class="cui-gallery-note">"Các control native dưới đây được ghép bằng FieldLabel/FieldHelp. Không thêm API lựa chọn vào component Web. Sự kiện composition và Enter chỉ ghi nhận trong tab này; không chứng minh IME hệ điều hành."</p>
                    <NativeEditingSample kind=NativeFieldKind::Text state=state/>
                    <NativeEditingSample kind=NativeFieldKind::Area state=state/>
                    <NativeEditingSample kind=NativeFieldKind::Number state=state/>
                </Stack>
            </div>
        </Stack>
    }
}

#[component]
pub fn NativeEditingSample(
    kind: NativeFieldKind,
    #[prop(into)] state: Signal<FieldState>,
) -> impl IntoView {
    let id = format!("gallery-native-{}", kind.key());
    let help_id = format!("{id}-help");
    let input = NodeRef::<html::Input>::new();
    let area = NodeRef::<html::Textarea>::new();
    let initial = if kind == NativeFieldKind::Number {
        "0"
    } else {
        "Ánh Nguyệt — Thanh Vân Sơn"
    };
    let value = RwSignal::new(initial.to_owned());
    let selection = RwSignal::new("Chưa ghi nhận lựa chọn native.".to_owned());
    let composing = RwSignal::new(false);
    let compositions = RwSignal::new(0u32);
    let actions = RwSignal::new(0u32);
    let on_input = move |event: leptos::ev::Event| {
        if state.get_untracked().accepts_input() {
            value.set(event_target_value(&event));
        }
    };
    let on_key = move |event: leptos::ev::KeyboardEvent| {
        if accepts_ime_action(
            &state.get_untracked(),
            &event.key(),
            composing.get_untracked() || event.is_composing(),
            event.key_code(),
        ) {
            actions.update(|count| *count += 1);
        }
    };
    let on_select = move |_: leptos::ev::Event| {
        selection.set(platform::read_selection(kind, input, area).message());
    };
    let on_end = move |_: leptos::ev::CompositionEvent| {
        composing.set(false);
        compositions.update(|count| *count += 1);
    };
    let select_state = Signal::derive(move || match state.get() {
        FieldState::Disabled(reason) => ButtonState::Disabled(reason),
        FieldState::Loading(reason) => ButtonState::Loading(reason),
        FieldState::Editable | FieldState::ReadOnly => ButtonState::Enabled,
    });
    let reason_id = format!("{id}-reason");
    let descriptions = Signal::derive({
        let help_id = help_id.clone();
        let reason_id = reason_id.clone();
        move || {
            if state.get().reason().is_some() {
                format!("{help_id} {reason_id}")
            } else {
                help_id.clone()
            }
        }
    });
    let select_all = Callback::new(move |_| {
        if !state.get_untracked().is_disabled() {
            selection
                .set(platform::select_all(kind, input, area, &value.get_untracked()).message());
        }
    });
    let help = match kind {
        NativeFieldKind::Number => "HTML input[type=number] không hỗ trợ selectionStart/selectionEnd hoặc setSelectionRange. min/max/step là ràng buộc Web; không suy ra bàn phím số native. Thử xóa về rỗng rồi nhập lại 0.",
        NativeFieldKind::Area => "Thử chọn và sửa văn bản, Tab, Shift+Tab, Enter và bộ gõ Việt. Enter vẫn xuống dòng; bản mẫu không preventDefault hay viết lại văn bản đang ghép.",
        NativeFieldKind::Text => "Thử chọn văn bản, chọn tất cả bằng nút rồi nhập; quan sát số lần Enter và compositionend. Trạng thái chỉ đọc vẫn giữ lựa chọn/copy native.",
    };
    view! {
        <div class="cui-field" data-native-kind=kind.key() data-state=move || state.get().as_str()>
            <FieldLabel for_id=id.clone()>{kind.label()}</FieldLabel>
            {if kind == NativeFieldKind::Area {
                view! {
                    <textarea id=id.clone() node_ref=area class="cui-field-control cui-textarea" rows=2
                        prop:value=move || value.get() readonly=move || matches!(state.get(), FieldState::ReadOnly)
                        disabled=move || state.get().is_disabled() aria-describedby=move || descriptions.get()
                        aria-busy=move || state.get().is_loading().to_string()
                        on:input=on_input on:select=on_select on:keydown=on_key
                        on:compositionstart=move |_| composing.set(true) on:compositionend=on_end>{initial}</textarea>
                }.into_any()
            } else {
                view! {
                    <input id=id.clone() node_ref=input class="cui-field-control"
                        type=if kind == NativeFieldKind::Number { "number" } else { "text" }
                        inputmode=if kind == NativeFieldKind::Number { "decimal" } else { "text" }
                        enterkeyhint="done" min=(kind == NativeFieldKind::Number).then_some("0")
                        max=(kind == NativeFieldKind::Number).then_some("100") step=(kind == NativeFieldKind::Number).then_some("1")
                        value=move || value.get() prop:value=move || value.get()
                        readonly=move || matches!(state.get(), FieldState::ReadOnly) disabled=move || state.get().is_disabled()
                        aria-describedby=move || descriptions.get() aria-busy=move || state.get().is_loading().to_string()
                        on:input=on_input on:select=on_select on:keydown=on_key
                        on:compositionstart=move |_| composing.set(true) on:compositionend=on_end/>
                }.into_any()
            }}
            <FieldHelp id=help_id>{help}</FieldHelp>
            <p id=reason_id class="cui-field-help" hidden=move || state.get().reason().is_none()>
                {move || state.get().reason().map(|reason| reason.as_str().to_owned()).unwrap_or_default()}
            </p>
            <Button id=format!("{id}-select-all") variant=ButtonVariant::Secondary state=select_state on_press=select_all>
                {if kind == NativeFieldKind::Number { "Kiểm tra giới hạn lựa chọn số" } else { "Chọn toàn bộ văn bản mẫu" }}
            </Button>
            <output id=format!("{id}-output") class="cui-gallery-output" aria-live="off">
                {move || format!("{} · Enter: {} · Đang ghép: {} · compositionend: {} · Giá trị: {:?}", selection.get(), actions.get(), composing.get(), compositions.get(), value.get())}
            </output>
        </div>
    }
}

mod platform {
    use super::{html, NativeFieldKind, NodeRef};
    #[cfg(target_arch = "wasm32")]
    use leptos::prelude::Get;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[cfg_attr(not(target_arch = "wasm32"), allow(dead_code))]
    pub enum SelectionObservation {
        Selected {
            start: u32,
            end: u32,
        },
        NumberUnsupported,
        NotMounted,
        ReadFailed,
        ApplyFailed,
        #[cfg(not(target_arch = "wasm32"))]
        BrowserNotRun,
    }

    impl SelectionObservation {
        pub fn message(self) -> String {
            match self {
                Self::Selected { start, end } => format!("Lựa chọn UTF-16 native: {start}–{end}"),
                Self::NumberUnsupported => {
                    "UNSUPPORTED: input[type=number] không có API lựa chọn văn bản.".to_owned()
                }
                Self::NotMounted => "Control chưa mount; chưa ghi nhận được lựa chọn.".to_owned(),
                Self::ReadFailed => "Không đọc được lựa chọn native của control.".to_owned(),
                Self::ApplyFailed => {
                    "Không áp dụng được lựa chọn native; giá trị vẫn được giữ.".to_owned()
                }
                #[cfg(not(target_arch = "wasm32"))]
                Self::BrowserNotRun => {
                    "NOT_RUN: cần DOM trình duyệt thật để đọc lựa chọn.".to_owned()
                }
            }
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn read_selection(
        kind: NativeFieldKind,
        input: NodeRef<html::Input>,
        area: NodeRef<html::Textarea>,
    ) -> SelectionObservation {
        let result = match kind {
            NativeFieldKind::Number => return SelectionObservation::NumberUnsupported,
            NativeFieldKind::Text => input
                .get()
                .map(|node| (node.selection_start(), node.selection_end())),
            NativeFieldKind::Area => area
                .get()
                .map(|node| (node.selection_start(), node.selection_end())),
        };
        match result {
            Some((Ok(Some(start)), Ok(Some(end)))) => SelectionObservation::Selected { start, end },
            Some(_) => SelectionObservation::ReadFailed,
            None => SelectionObservation::NotMounted,
        }
    }

    #[cfg(target_arch = "wasm32")]
    pub fn select_all(
        kind: NativeFieldKind,
        input: NodeRef<html::Input>,
        area: NodeRef<html::Textarea>,
        value: &str,
    ) -> SelectionObservation {
        let end = value.encode_utf16().count() as u32;
        let result = match kind {
            NativeFieldKind::Number => return SelectionObservation::NumberUnsupported,
            NativeFieldKind::Text => input
                .get()
                .map(|node| node.focus().and_then(|_| node.set_selection_range(0, end))),
            NativeFieldKind::Area => area
                .get()
                .map(|node| node.focus().and_then(|_| node.set_selection_range(0, end))),
        };
        match result {
            Some(Ok(())) => read_selection(kind, input, area),
            Some(Err(_)) => SelectionObservation::ApplyFailed,
            None => SelectionObservation::NotMounted,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn read_selection(
        kind: NativeFieldKind,
        _: NodeRef<html::Input>,
        _: NodeRef<html::Textarea>,
    ) -> SelectionObservation {
        if kind == NativeFieldKind::Number {
            SelectionObservation::NumberUnsupported
        } else {
            SelectionObservation::BrowserNotRun
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub fn select_all(
        kind: NativeFieldKind,
        input: NodeRef<html::Input>,
        area: NodeRef<html::Textarea>,
        _: &str,
    ) -> SelectionObservation {
        read_selection(kind, input, area)
    }
}
