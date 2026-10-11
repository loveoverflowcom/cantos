use cantos_ui::{
    tokens::Token, Button, ButtonState, ButtonVariant, Density, DisabledReason,
    EmptyDisabledReason, Icon, IconName, Theme, UiRoot,
};
use leptos::prelude::*;

#[test]
fn theme_attributes_are_closed_and_unknown_input_is_rejected() {
    for theme in [Theme::System, Theme::Light, Theme::Dark] {
        assert_eq!(Theme::from_attribute(theme.as_str()), Some(theme));
    }
    assert_eq!(Theme::from_attribute("unknown"), None);
    assert_eq!(Theme::from_attribute("Dark"), None);
}

#[test]
fn disabled_action_cannot_hide_its_reason_and_text_is_escaped() {
    assert_eq!(DisabledReason::new(" \n "), Err(EmptyDisabledReason));
    let reason = DisabledReason::new("Cần duyệt <b>đúng phiên bản</b>").unwrap();
    let html = view! {
        <UiRoot theme=Theme::Dark density=Density::Comfortable>
            <Button id="preview" variant=ButtonVariant::Tonal state=ButtonState::Disabled(reason)>
                <Icon name=IconName::Check/>"Duyệt bản diễn Sơn Hà Ký"
            </Button>
        </UiRoot>
    }
    .to_html();
    assert!(html.contains("data-theme=\"dark\""), "{html}");
    assert!(html.contains("data-density=\"comfortable\""), "{html}");
    assert!(html.contains("type=\"button\""), "{html}");
    assert!(html.contains("disabled"), "{html}");
    assert!(
        html.contains("aria-describedby=\"preview-reason\""),
        "{html}"
    );
    assert!(
        html.contains("Cần duyệt &lt;b&gt;đúng phiên bản&lt;/b&gt;"),
        "{html}"
    );
    assert!(!html.contains("<b>"), "{html}");
}

#[test]
fn enabled_action_has_visible_children_and_no_disabled_description() {
    let html = view! { <Button id="sample">"Thêm lời thoại"</Button> }.to_html();
    assert!(html.contains("Thêm lời thoại"), "{html}");
    assert!(!html.contains("aria-describedby="), "{html}");
    assert!(!html.contains("disabled"), "{html}");
}

#[test]
fn every_offline_icon_is_decorative_and_never_uses_html_injection() {
    for &name in IconName::ALL {
        let html = view! { <Icon name=name/> }.to_html();
        assert!(html.contains("aria-hidden=\"true\""), "{html}");
        assert!(html.contains("focusable=\"false\""), "{html}");
        assert!(html.contains("viewBox=\"0 0 24 24\""), "{html}");
        assert!(html.contains("<path"), "{html}");
        assert!(!html.contains("<script"), "{html}");
    }
}

#[test]
fn typed_tokens_point_to_the_runtime_css_contract() {
    for &token in Token::ALL {
        assert!(cantos_ui::TOKEN_CSS.contains(token.css_variable()));
        for theme in [Theme::Light, Theme::Dark] {
            assert!(!token.css_value(theme).is_empty());
        }
    }
}
