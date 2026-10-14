use cantos_ui::{
    AccessibleName, BackButton, Button, ButtonState, ButtonVariant, DisabledReason,
    EmptyAccessibleName, FieldState, Surface, SurfaceTone, SurfaceVariant, TextField,
    TextFieldKind, TextFieldVariant,
};
use leptos::prelude::*;

#[allow(dead_code)]
#[path = "../preview/auth_samples.rs"]
mod samples;

fn opening_tag<'a>(html: &'a str, element: &str, id: &str) -> &'a str {
    let marker = format!("<{element} id=\"{id}\"");
    let start = html
        .find(&marker)
        .unwrap_or_else(|| panic!("missing {marker}: {html}"));
    let end = html[start..].find('>').expect("complete native element") + start;
    &html[start..=end]
}

#[test]
fn auth_actions_keep_one_native_intent_label_and_explained_blocked_states() {
    for variant in [
        ButtonVariant::AuthPrimary,
        ButtonVariant::AuthSecondary,
        ButtonVariant::AuthOutlined,
    ] {
        for state in [
            ButtonState::Enabled,
            ButtonState::Disabled(DisabledReason::new("Chưa thể tiếp tục").unwrap()),
            ButtonState::Loading(DisabledReason::new("Đang chờ, giữ nhãn gốc").unwrap()),
        ] {
            let html = view! {
                <Button id="auth-action" variant=variant state=state.clone()
                    icon=cantos_ui::IconName::ArrowForward on_press=Callback::new(|_| {})>
                    "Đăng nhập với Cantos ID cho Nguyễn Thị Ánh Nguyệt"
                </Button>
            }
            .to_html();
            let button = opening_tag(&html, "button", "auth-action");
            assert!(button.contains("type=\"button\""), "{button}");
            assert_eq!(button.contains("disabled"), !state.can_press(), "{button}");
            assert_eq!(
                button.contains("aria-busy=\"true\""),
                state.is_busy(),
                "{button}"
            );
            assert_eq!(
                button.contains("aria-describedby=\"auth-action-reason\""),
                !state.can_press(),
                "{button}"
            );
            assert_eq!(
                html.matches("Đăng nhập với Cantos ID cho Nguyễn Thị Ánh Nguyệt")
                    .count(),
                1
            );
            assert!(
                html.contains("aria-hidden=\"true\""),
                "decorative icon: {html}"
            );
            assert!(
                !button.contains("aria-label="),
                "visible label must own the name: {button}"
            );
            if let Some(reason) = state.reason() {
                assert!(html.contains(reason.as_str()), "{html}");
            }
        }
    }
}

#[test]
fn back_is_a_named_native_action_not_a_link_submit_or_implicit_navigation() {
    assert_eq!(AccessibleName::new(" \n "), Err(EmptyAccessibleName));
    for icon_only in [false, true] {
        let html = view! {
            <BackButton id="back" name=AccessibleName::new("Về Tài khoản <giữ ý định>").unwrap()
                icon_only=icon_only on_back=Callback::new(|_| {})
                state=ButtonState::Loading(DisabledReason::new("Host đang chờ; không phát intent lần hai").unwrap())
                described_by="origin-context"/>
        }.to_html();
        let button = opening_tag(&html, "button", "back");
        assert!(button.contains("type=\"button\""));
        assert!(button.contains("disabled"));
        assert!(button.contains("aria-busy=\"true\""));
        assert!(button.contains("aria-describedby=\"origin-context back-reason\""));
        assert_eq!(
            html.matches("Về Tài khoản &lt;giữ ý định&gt;").count(),
            1,
            "{html}"
        );
        assert_eq!(html.contains("class=\"cui-sr-only\""), icon_only, "{html}");
        assert!(!html.contains("href="));
        assert!(!button.contains("aria-label="));
        assert!(html.contains("aria-hidden=\"true\""));
        assert!(html.contains("Host đang chờ; không phát intent lần hai"));
    }
}

#[test]
fn auth_tonal_field_retains_raw_value_native_type_and_all_visible_descriptions() {
    let raw = "nguyễn-thị-ánh-nguyệt+<draft>@example.test";
    let html = view! {
        <TextField id="auth-email" variant=TextFieldVariant::AuthTonal kind=TextFieldKind::Email
            label="Email của Nguyễn Thị Ánh Nguyệt" value=raw autocomplete="email"
            state=FieldState::Loading(DisabledReason::new("Đang chờ, giá trị vẫn còn").unwrap())
            help="Giữ dấu và giá trị gốc" error="Kiểm tra email, không xóa bản nháp"
            described_by="context" on_input=Callback::new(|_: String| {})/>
    }
    .to_html();
    let input = opening_tag(&html, "input", "auth-email");
    assert!(html.contains("for=\"auth-email\""));
    assert!(input.contains("type=\"email\""));
    assert!(input.contains("autocomplete=\"email\""));
    assert!(input.contains("value=\"nguyễn-thị-ánh-nguyệt+&lt;draft&gt;@example.test\""));
    assert!(input.contains("disabled"));
    assert!(input.contains("aria-busy=\"true\""));
    assert!(input.contains("aria-invalid=\"true\""));
    assert!(input.contains("aria-errormessage=\"auth-email-error\""));
    assert!(input.contains(
        "aria-describedby=\"context auth-email-help auth-email-error auth-email-reason\""
    ));
    assert!(html.contains("Kiểm tra email, không xóa bản nháp"));
    assert!(html.contains("Đang chờ, giá trị vẫn còn"));
}

#[test]
fn surface_geometry_does_not_replace_host_content_or_impose_auth_behavior() {
    for variant in [
        SurfaceVariant::Auth,
        SurfaceVariant::Profile,
        SurfaceVariant::Membership,
        SurfaceVariant::AccountGroup,
    ] {
        let html = view! {
            <Surface variant=variant tone=SurfaceTone::PrimaryContainer>
                <section aria-labelledby="profile-name"><h2 id="profile-name">"Nguyễn Thị Ánh Nguyệt"</h2><p>"Thông tin phụ dùng foreground được ghép đúng."</p></section>
            </Surface>
        }.to_html();
        assert!(html.contains("aria-labelledby=\"profile-name\""));
        assert!(html.contains("<h2 id=\"profile-name\""));
        assert!(html.contains("Nguyễn Thị Ánh Nguyệt"));
        assert!(!html.contains("role=\"button\""));
        assert!(!html.contains("tabindex="));
        assert!(!html.contains("aria-live="));
    }
}

#[test]
fn package_samples_expose_held_values_counts_and_recovery_without_service_effects() {
    let html = Owner::new().with(|| {
        view! {
            <samples::AuthActionSamples initial_state="loading"/>
            <samples::AuthFieldSamples initial_state="readonly" initial_error=true/>
            <samples::AuthSurfaceSamples/>
            <samples::BackButtonSamples initial_state="disabled"/>
        }
        .to_html()
    });
    assert!(opening_tag(&html, "button", "auth-primary").contains("disabled"));
    assert!(opening_tag(&html, "input", "auth-email").contains("readonly"));
    assert!(opening_tag(&html, "input", "auth-email").contains("aria-invalid=\"true\""));
    assert!(opening_tag(&html, "button", "auth-back-visible").contains("disabled"));
    for id in [
        "auth-action-output",
        "auth-field-output",
        "auth-back-output",
        "auth-regular-control",
        "auth-regular-field",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "missing observable {id}"
        );
    }
    assert!(html.contains("nguyen-thi-anh-nguyet@example.test"));
    assert!(html.contains("Số intent được nhận: 0"));
    assert!(html.contains("Host nhận 0 intent Back"));
    assert!(html.contains("Giữ những câu chuyện đã chọn"));
}
