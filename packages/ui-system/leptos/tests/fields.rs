use cantos_ui::{
    Checkbox, ChoiceState, DisabledReason, FieldError, FieldHelp, FieldLabel, FieldState,
    NumberField, Radio, RangeField, Select, SelectDensity, SelectLabelPlacement, SelectOption,
    Switch, TextArea, TextField, TextFieldKind,
};
use leptos::prelude::*;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

fn control<'a>(html: &'a str, element: &str, id: &str) -> &'a str {
    let start = html.find(&format!("<{element} id=\"{id}\"")).unwrap();
    let end = html[start..].find('>').unwrap() + start;
    &html[start..=end]
}

#[test]
fn menu_select_preserves_native_fallback_form_facts_and_named_composite_anatomy() {
    let html = view! {
        <Select id="theme" name="theme-preference" label="Giao diện" value="missing" required=true
            options=vec![SelectOption::new("system", "Theo hệ thống"), SelectOption::new("dark", "Tối <script>")]
            placeholder="Chọn giao diện" help="Giữ lựa chọn đã lưu" error="Chọn giá trị hợp lệ"
            described_by="preferences-context" on_input=Callback::new(|_: String| panic!("render dispatched input"))/>
    }.to_html();
    let fallback = control(&html, "select", "theme");
    assert!(fallback.contains("name=\"theme-preference\""), "{fallback}");
    assert!(fallback.contains("required"), "{fallback}");
    assert!(!fallback.contains("hidden"), "{fallback}");
    let trigger = control(&html, "button", "theme-trigger");
    for attribute in [
        "type=\"button\"",
        "hidden",
        "role=\"combobox\"",
        "aria-controls=\"theme-listbox\"",
        "aria-expanded=\"false\"",
        "aria-required=\"true\"",
        "aria-invalid=\"true\"",
        "aria-describedby=\"preferences-context theme-help theme-error\"",
    ] {
        assert!(
            trigger.contains(attribute),
            "missing {attribute}: {trigger}"
        );
    }
    assert!(html.contains("popover=\"auto\""), "{html}");
    assert!(html.contains("role=\"listbox\""), "{html}");
    assert_eq!(html.matches("role=\"option\"").count(), 2);
    assert!(!html.contains("aria-selected=\"true\""), "{html}");
    assert!(!html.contains("<script>"), "{html}");
    assert!(html.contains("Chọn giao diện"), "{html}");
}

#[test]
fn menu_select_projects_disabled_loading_and_language_without_selecting_a_default() {
    for state in [
        ChoiceState::Disabled(DisabledReason::new("Không thể đổi").unwrap()),
        ChoiceState::Loading(DisabledReason::new("Đang đọc").unwrap()),
    ] {
        let busy = state.is_loading();
        let html = view! {
            <Select id="locale" label="Ngôn ngữ" value="en" state=state
                options=vec![SelectOption { lang: Some("vi-VN".to_owned()), ..SelectOption::new("vi-VN", "Tiếng Việt") },
                    SelectOption { lang: Some("en".to_owned()), disabled: true, ..SelectOption::new("en", "English") }]
                on_input=Callback::new(|_: String| panic!("render dispatched input"))/>
        }.to_html();
        for element in [
            control(&html, "select", "locale"),
            control(&html, "button", "locale-trigger"),
        ] {
            assert!(element.contains("disabled"), "{element}");
            assert!(
                element.contains(&format!("aria-busy=\"{busy}\"")),
                "{element}"
            );
            assert!(
                element.contains("aria-describedby=\"locale-reason\""),
                "{element}"
            );
        }
        assert!(
            html.contains("aria-selected=\"true\" aria-disabled=\"true\""),
            "{html}"
        );
        assert!(html.contains("lang=\"vi-VN\""), "{html}");
        assert!(html.contains("lang=\"en\""), "{html}");
    }
}

#[test]
fn labeled_input_preserves_raw_text_and_associates_all_visible_descriptions() {
    let html = view! {
        <TextField id="episode-title" label="Tên tập Sơn Hà Ký — Ngày mai, mình có diễn tiếp không?"
            value="<script>nhân vật & thoại</script>" on_input=Callback::new(|_: String| {})
            help="Giữ dấu tiếng Việt & tên gốc" error="Cần tên <b>đầy đủ</b>"
            described_by="revision-context" class="caller-layout"
            state=FieldState::Disabled(DisabledReason::new("Chỉ chủ tác phẩm được sửa").unwrap())/>
    }
    .to_html();
    let input = control(&html, "input", "episode-title");
    assert!(html.contains("for=\"episode-title\""), "{html}");
    assert!(input.contains("disabled"), "{input}");
    assert!(input.contains("aria-invalid=\"true\""), "{input}");
    assert!(
        input.contains("aria-errormessage=\"episode-title-error\""),
        "{input}"
    );
    assert!(input.contains("aria-describedby=\"revision-context episode-title-help episode-title-error episode-title-reason\""), "{input}");
    assert!(
        input.contains("value=\"&lt;script&gt;nhân vật &amp; thoại&lt;/script&gt;\""),
        "{input}"
    );
    for suffix in ["help", "error", "reason"] {
        assert!(
            html.contains(&format!("id=\"episode-title-{suffix}\"")),
            "{html}"
        );
    }
    assert!(html.contains("cui-field caller-layout"), "{html}");
    assert!(html.contains("Cần tên &lt;b&gt;đầy đủ&lt;/b&gt;"), "{html}");
    assert!(html.contains("Chỉ chủ tác phẩm được sửa"), "{html}");
    assert!(!html.contains("<script>"), "{html}");
}

#[test]
fn optional_and_blank_messages_do_not_create_invalid_or_dangling_descriptions() {
    let html = view! {
        <TextField id="name" label="Tên nhân vật" value="Đức Trí" on_input=Callback::new(|_: String| {})
            help=" " error="\n"/>
    }.to_html();
    let input = control(&html, "input", "name");
    assert!(input.contains("aria-invalid=\"false\""), "{input}");
    assert!(!input.contains("aria-describedby="), "{input}");
    assert!(!input.contains("aria-errormessage="), "{input}");
    assert!(!input.contains("disabled"), "{input}");
    assert!(!input.contains("readonly"), "{input}");
}

#[test]
fn numeric_zero_and_empty_editing_value_are_not_replaced_by_a_default() {
    for raw in ["0", "", "0.00", "-0", "1.25"] {
        let html = view! {
            <NumberField id="intensity" label="Cường độ" value=raw on_input=Callback::new(|_: String| {})
                min="0" max="2" step="0.01"/>
        }.to_html();
        let input = control(&html, "input", "intensity");
        assert!(input.contains(&format!("value=\"{raw}\"")), "{input}");
        assert!(input.contains("type=\"number\""), "{input}");
        assert!(input.contains("min=\"0\""), "{input}");
        assert!(input.contains("max=\"2\""), "{input}");
        assert!(input.contains("step=\"0.01\""), "{input}");
    }
    let html = view! {
        <RangeField id="position" label="Vị trí" value="0" on_input=Callback::new(|_: String| {})
            min="0" max="100" step="1" value_text="0 phút trên 47 phút 30 giây"/>
    }
    .to_html();
    let range = control(&html, "input", "position");
    assert!(range.contains("value=\"0\""), "{range}");
    assert!(
        range.contains("aria-valuetext=\"0 phút trên 47 phút 30 giây\""),
        "{range}"
    );
}

#[test]
fn native_readonly_keeps_text_copyable_and_loading_has_busy_reason() {
    let html = view! {
        <TextField id="source" label="Nguồn" value="Sơn Hà Ký" on_input=Callback::new(|_: String| {})
            state=FieldState::ReadOnly/>
        <NumberField id="cost" label="Chi phí" value="0" on_input=Callback::new(|_: String| {})
            state=FieldState::Loading(DisabledReason::new("Đang đọc thông tin đã lưu").unwrap())/>
    }.to_html();
    let readonly = control(&html, "input", "source");
    assert!(readonly.contains("readonly"), "{readonly}");
    assert!(!readonly.contains("disabled"), "{readonly}");
    let loading = control(&html, "input", "cost");
    assert!(loading.contains("disabled"), "{loading}");
    assert!(loading.contains("aria-busy=\"true\""), "{loading}");
    assert!(
        loading.contains("aria-describedby=\"cost-reason\""),
        "{loading}"
    );
    assert!(html.contains("Đang đọc thông tin đã lưu"), "{html}");
}

#[test]
fn textarea_escapes_closing_tag_without_inserting_markup_or_automatic_announcements() {
    let html = view! {
        <TextArea id="dialogue" label="Lời thoại Người dẫn chuyện" rows=6
            value="Ngày mai, mình có diễn tiếp không?\n</textarea><script>không</script>"
            on_input=Callback::new(|_: String| {}) error="Giữ nguyên bản nháp & sửa tên nhân vật"/>
    }
    .to_html();
    let textarea = control(&html, "textarea", "dialogue");
    assert!(textarea.contains("rows=\"6\""), "{textarea}");
    assert!(
        textarea.contains("aria-errormessage=\"dialogue-error\""),
        "{textarea}"
    );
    assert!(
        html.contains("&lt;/textarea&gt;&lt;script&gt;không&lt;/script&gt;"),
        "{html}"
    );
    assert!(!html.contains("<script>"), "{html}");
    assert!(!html.contains("role=\"alert\""), "{html}");
    assert!(!html.contains("aria-live"), "{html}");
}

#[test]
fn select_renders_native_options_and_checked_controls_keep_native_grouping() {
    let html = view! {
        <Select id="voice" label="Giọng đọc" value="my-duyen" on_input=Callback::new(|_: String| {})
            state=ChoiceState::Loading(DisabledReason::new("Đang đọc danh sách giọng").unwrap())>
            <option value="duc-tri">"Đức Trí"</option>
            <option value="my-duyen">"Mỹ Duyên"</option>
        </Select>
        <Checkbox id="rights" label="Tôi có quyền sử dụng nguồn" checked=true on_change=Callback::new(|_: bool| {})/>
        <Radio id="voice-a" name="preset" value="a" label="Đức Trí" checked=true on_change=Callback::new(|_: bool| {})/>
        <Radio id="voice-b" name="preset" value="b" label="Mỹ Duyên" checked=false on_change=Callback::new(|_: bool| {})/>
        <Switch id="autoplay" label="Tự động chuyển tập" checked=false on_change=Callback::new(|_: bool| {})/>
    }.to_html();
    let select = control(&html, "select", "voice");
    // Safari's native menulist ignores the token min-height. Keep a styled native
    // select with its decorative, non-focusable indicator, not a custom listbox.
    assert!(html.contains("class=\"cui-select\""), "{html}");
    assert!(
        html.contains("aria-hidden=\"true\" focusable=\"false\""),
        "{html}"
    );
    assert_eq!(html.matches("<select ").count(), 1, "{html}");
    assert!(!html.contains("role=\"listbox\""), "{html}");
    assert!(select.contains("disabled"), "{select}");
    assert!(select.contains("aria-busy=\"true\""), "{select}");
    assert!(
        html.contains("<option value=\"my-duyen\">Mỹ Duyên</option>"),
        "{html}"
    );
    let checkbox = control(&html, "input", "rights");
    assert!(checkbox.contains("type=\"checkbox\""), "{checkbox}");
    assert!(checkbox.contains("checked"), "{checkbox}");
    for id in ["voice-a", "voice-b"] {
        let radio = control(&html, "input", id);
        assert!(radio.contains("type=\"radio\""), "{radio}");
        assert!(radio.contains("name=\"preset\""), "{radio}");
    }
    let switch = control(&html, "input", "autoplay");
    assert!(switch.contains("role=\"switch\""), "{switch}");
    assert!(switch.contains("aria-checked=\"false\""), "{switch}");
    assert!(html.contains("for=\"autoplay\""), "{html}");
}

#[test]
fn field_state_guards_only_allow_editable_input_and_enabled_choice() {
    let reason = DisabledReason::new("Ce champ est indisponible").unwrap();
    for (state, allowed, disabled, loading, name) in [
        (FieldState::Editable, true, false, false, "editable"),
        (FieldState::ReadOnly, false, false, false, "readonly"),
        (
            FieldState::Disabled(reason.clone()),
            false,
            true,
            false,
            "disabled",
        ),
        (
            FieldState::Loading(reason.clone()),
            false,
            true,
            true,
            "loading",
        ),
    ] {
        assert_eq!(state.accepts_input(), allowed);
        assert_eq!(state.is_disabled(), disabled);
        assert_eq!(state.is_loading(), loading);
        assert_eq!(state.as_str(), name);
        assert_eq!(state.reason().is_some(), disabled);
    }
    for (state, allowed, loading) in [
        (ChoiceState::Enabled, true, false),
        (ChoiceState::Disabled(reason.clone()), false, false),
        (ChoiceState::Loading(reason), false, true),
    ] {
        assert_eq!(state.accepts_input(), allowed);
        assert_eq!(state.is_loading(), loading);
        assert_eq!(state.reason().is_some(), !allowed);
    }
}

#[test]
fn rendering_does_not_call_input_callbacks_and_kind_is_a_closed_native_mapping() {
    let calls = Arc::new(AtomicUsize::new(0));
    let recorded = calls.clone();
    let callback = Callback::new(move |_: String| {
        recorded.fetch_add(1, Ordering::SeqCst);
    });
    for (kind, native) in [
        (TextFieldKind::Text, "text"),
        (TextFieldKind::Search, "search"),
        (TextFieldKind::Email, "email"),
        (TextFieldKind::Password, "password"),
        (TextFieldKind::Url, "url"),
        (TextFieldKind::Telephone, "tel"),
    ] {
        let html =
            view! { <TextField id="kind" label="Tên" kind=kind value="0" on_input=callback/> }
                .to_html();
        assert!(
            control(&html, "input", "kind").contains(&format!("type=\"{native}\"")),
            "{html}"
        );
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn standalone_label_help_and_error_are_composable_and_escape_children() {
    let html = view! {
        <FieldLabel for_id="custom">"Nhân vật & thoại"</FieldLabel>
        <FieldHelp id="custom-help">"Giữ tên <nguồn>"</FieldHelp>
        <FieldError id="custom-error">"Thiếu quyền & nguồn"</FieldError>
    }
    .to_html();
    assert!(html.contains("for=\"custom\""), "{html}");
    assert!(html.contains("id=\"custom-help\""), "{html}");
    assert!(html.contains("id=\"custom-error\""), "{html}");
    assert!(html.contains("Nhân vật &amp; thoại"), "{html}");
    assert!(html.contains("Giữ tên &lt;nguồn&gt;"), "{html}");
}

#[test]
fn select_density_and_label_placement_preserve_native_and_menu_contracts() {
    for density in [SelectDensity::Regular, SelectDensity::Compact] {
        for placement in [SelectLabelPlacement::Above, SelectLabelPlacement::Inline] {
            for enhanced in [false, true] {
                let html = if enhanced {
                    view! { <Select id="locale-density" label="Ngôn ngữ giao diện" value="vi-VN"
                    density=density label_placement=placement
                    options=vec![SelectOption::new("vi-VN", "Tiếng Việt")]
                    on_input=Callback::new(|_: String| panic!("render dispatched input"))/> }
                    .to_html()
                } else {
                    view! { <Select id="locale-density" label="Ngôn ngữ giao diện" value="vi-VN"
                        density=density label_placement=placement
                        on_input=Callback::new(|_: String| panic!("render dispatched input"))>
                        <option value="vi-VN" lang="vi-VN">"Tiếng Việt"</option>
                    </Select> }
                    .to_html()
                };
                assert!(html.contains(&format!("data-select-density=\"{}\"", density.as_str())));
                assert!(html.contains(&format!("data-label-placement=\"{}\"", placement.as_str())));
                assert!(html.contains("Ngôn ngữ giao diện"));
                assert!(html.contains("Tiếng Việt"));
                assert_eq!(html.matches("<select ").count(), 1);
                assert_eq!(
                    html.matches("role=\"combobox\"").count(),
                    usize::from(enhanced)
                );
                assert!(!control(&html, "select", "locale-density").contains("hidden"));
            }
        }
    }
    let html = view! { <Select id="regular-default" label="Theme" value="dark"
    on_input=Callback::new(|_: String| {})><option value="dark">"Dark"</option></Select> }
    .to_html();
    assert!(html.contains("data-select-density=\"regular\""));
    assert!(html.contains("data-label-placement=\"above\""));
}
