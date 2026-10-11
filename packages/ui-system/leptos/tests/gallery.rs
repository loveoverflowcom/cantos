#[allow(dead_code)]
#[path = "../preview/main.rs"]
mod preview;

use leptos::prelude::*;
use std::collections::BTreeSet;

fn opening_tag<'a>(html: &'a str, id: &str) -> &'a str {
    let attribute = format!(" id=\"{id}\"");
    let start = html
        .find(&attribute)
        .unwrap_or_else(|| panic!("missing {id}"));
    let end = html[start..].find('>').expect("complete opening tag") + start;
    &html[start..end]
}

#[test]
fn gallery_renders_every_family_with_unique_reserved_ids_and_zero_fixtures() {
    let html = Owner::new().with(|| view! { <preview::Gallery/> }.to_html());
    for family in [
        "cui-button",
        "cui-icon-button",
        "cui-filter-chip",
        "cui-nav-tabs",
        "cui-field",
        "cui-choice",
        "cui-range-control",
        "cui-dialog",
        "cui-surface",
        "cui-container",
        "cui-stack",
        "cui-grid",
        "cui-badge",
        "cui-status",
        "cui-empty-state",
    ] {
        assert!(html.contains(family), "gallery omits {family}");
    }
    let mut ids = BTreeSet::new();
    for attribute in html.split(" id=\"").skip(1) {
        let id = attribute.split('"').next().unwrap();
        assert!(!id.is_empty(), "empty gallery ID");
        assert!(ids.insert(id), "duplicate gallery ID: {id}");
    }
    assert!(ids.contains("gallery-number"));
    assert!(ids.contains("gallery-range"));
    assert!(html.contains("value=\"0\""));
    assert!(html.contains("Thanh Vân Sơn"));
    assert!(!html.contains("<audio"));
    assert!(!html.contains("<iframe"));
}

#[test]
fn gallery_feedback_fixtures_cover_each_actual_conditional_state() {
    for (state, family) in [
        ("empty", "cui-empty-state"),
        ("loading", "cui-loading-state"),
        ("error", "cui-error-state"),
        ("success", "cui-status"),
    ] {
        let html =
            Owner::new().with(|| view! { <preview::Gallery initial_feedback=state/> }.to_html());
        assert!(
            html.contains(family),
            "gallery state {state} omits {family}"
        );
        if state == "success" {
            assert!(html.contains("Vùng mẫu đã đổi sang thành công."));
            assert!(!html.contains("cui-empty-state"));
        }
    }
}

#[test]
fn registrations_have_unique_ids_and_every_family_has_a_real_sample_anchor() {
    let html = Owner::new().with(|| view! { <preview::Gallery/> }.to_html());
    let mut registered = BTreeSet::new();
    for entry in preview::preview_catalog::COMPONENTS {
        assert!(
            registered.insert(entry.id),
            "duplicate registry ID {}",
            entry.id
        );
        assert!(
            !entry.states.is_empty(),
            "{} has no sample states",
            entry.id
        );
        assert!(!entry.leptos.reason.is_empty());
        assert!(!entry.cmp.reason.is_empty());
        let anchor =
            preview::catalog::sample_anchor(entry.family).expect("registered sample family");
        assert!(
            html.contains(&format!("id=\"{anchor}\"")),
            "{} omits its sample anchor",
            entry.id
        );
        assert!(html.contains(&format!("data-component=\"{}\"", entry.id)));
        assert!(html.contains(&format!("href=\"#{anchor}\"")));
        assert!(html.contains(entry.leptos.status));
        assert!(html.contains(entry.cmp.status));
    }
    for family in preview::catalog::FAMILIES {
        assert!(
            preview::catalog::family_matches(family.key, "all", ""),
            "{} has no registration",
            family.key
        );
        assert!(html.contains(&format!("data-sample-family=\"{}\"", family.key)));
    }
}

#[test]
fn discovery_is_case_insensitive_conjunctive_and_respects_family_selection() {
    let entries = preview::preview_catalog::COMPONENTS;
    let text_field = entries
        .iter()
        .find(|entry| entry.id == "text-field")
        .unwrap();
    assert!(preview::catalog::component_matches(
        text_field,
        "all",
        " TEXTFIELD  READONLY "
    ));
    assert!(preview::catalog::component_matches(
        text_field,
        "all",
        "NHẬP LIỆU TextField"
    ));
    assert!(preview::catalog::component_matches(
        text_field,
        "fields",
        "text-field"
    ));
    assert!(!preview::catalog::component_matches(
        text_field,
        "actions",
        "text-field"
    ));
    assert!(!preview::catalog::component_matches(
        text_field,
        "all",
        "TextField absent-state"
    ));
    assert!(preview::catalog::family_matches(
        "fields",
        "all",
        "TextField"
    ));
    assert!(!preview::catalog::family_matches(
        "actions",
        "all",
        "TextField"
    ));
    assert_eq!(preview::catalog::family_key("fields"), "fields");
    assert_eq!(preview::catalog::family_key("unknown"), "all");
    assert_eq!(preview::catalog::sample_anchor("unknown"), None);
}

#[test]
fn filtering_hides_samples_instead_of_removing_controlled_values_or_dialog_anatomy() {
    let html =
        Owner::new().with(|| view! { <preview::Gallery initial_family="fields"/> }.to_html());
    assert!(!opening_tag(&html, "gallery-fields").contains("hidden"));
    for id in [
        "gallery-actions",
        "gallery-feedback",
        "gallery-layout",
        "gallery-icons",
        "gallery-dialog-samples",
    ] {
        assert!(
            opening_tag(&html, id).contains("hidden"),
            "{id} should be filtered"
        );
    }
    assert!(!opening_tag(&html, "gallery-navigation").contains("hidden"));
    for id in [
        "gallery-primary",
        "gallery-title",
        "gallery-number",
        "gallery-range",
        "gallery-dialog-open",
        "gallery-dialog",
        "gallery-dialog-text",
    ] {
        assert!(
            html.contains(&format!("id=\"{id}\"")),
            "filter removed {id}"
        );
    }
    assert!(html.contains("value=\"0\""));
    assert!(html.contains("Lời hẹn vẫn còn ở đây khi đóng rồi mở lại."));
    assert!(html.contains("aria-controls=\"gallery-dialog\""));
    assert!(
        !html.contains("focus_on_press"),
        "component API must not leak as a DOM attribute"
    );
}

#[test]
fn query_filter_has_an_honest_empty_state_and_escapes_input() {
    let html = Owner::new()
        .with(|| view! { <preview::Gallery initial_query="<script>absent</script>"/> }.to_html());
    assert!(!opening_tag(&html, "gallery-catalog-empty").contains("hidden"));
    assert!(html.contains("0 / "));
    assert!(!html.contains("<script>"));
    assert!(html.contains("&lt;script&gt;absent&lt;/script&gt;"));
    assert!(html.contains("id=\"gallery-title\""));
}

#[test]
fn token_catalogue_renders_every_semantic_name_both_themes_and_density_overrides() {
    let html = Owner::new().with(|| view! { <preview::Gallery/> }.to_html());
    let mut names = BTreeSet::new();
    for token in preview::preview_catalog::TOKENS {
        assert!(names.insert(token.name), "duplicate token {}", token.name);
        assert!(html.contains(&format!("data-token=\"{}\"", token.name)));
        for value in [
            Some(token.light),
            Some(token.dark),
            token.compact,
            token.comfortable,
        ]
        .into_iter()
        .flatten()
        {
            let rendered_value = view! { <code>{value}</code> }.to_html();
            assert!(
                html.contains(&rendered_value),
                "{} omits value {value}",
                token.name
            );
        }
        assert!(preview::catalog::token_matches(token, token.name));
        for override_value in [token.compact, token.comfortable].into_iter().flatten() {
            assert!(preview::catalog::token_matches(token, override_value));
        }
        assert!(!preview::catalog::token_matches(
            token,
            "absent-token-family"
        ));
    }
    assert!(html.contains("Mật độ gọn"));
    assert!(html.contains("Mật độ thoải mái"));
    assert!(html.contains(preview::preview_catalog::TOKEN_VERSION));
}

#[test]
fn token_specimens_use_only_generated_css_roles_and_cover_supported_visual_families() {
    use preview::token_sample::SampleKind;

    let html = Owner::new().with(|| view! { <preview::Gallery/> }.to_html());
    for (name, expected) in [
        ("color.primary", SampleKind::Color),
        ("type.family.editorial", SampleKind::FontFamily),
        ("type.headline.size", SampleKind::FontSize),
        ("type.label.weight", SampleKind::FontWeight),
        ("type.body.lineHeight", SampleKind::LineHeight),
        ("space.inset", SampleKind::Spacing),
        ("shape.pill", SampleKind::Shape),
        ("size.control", SampleKind::Size),
        ("size.focusWidth", SampleKind::Stroke),
        ("elevation.overlay", SampleKind::Shadow),
        ("state.hoverOpacity", SampleKind::Opacity),
        ("breakpoint.medium", SampleKind::ValueOnly),
        ("size.focusOffset", SampleKind::ValueOnly),
        ("motion.feedback.duration", SampleKind::ValueOnly),
    ] {
        let token = preview::preview_catalog::TOKENS
            .iter()
            .find(|token| token.name == name)
            .unwrap();
        assert_eq!(
            preview::token_sample::sample_kind(token),
            expected,
            "{name}"
        );
    }
    for token in preview::preview_catalog::TOKENS {
        assert!(cantos_ui::TOKEN_CSS.contains(token.css_variable));
        assert!(html.contains(&format!("data-token-sample=\"{}\"", token.name)));
        assert_eq!(
            preview::token_sample::sample_value(token),
            format!("var({})", token.css_variable)
        );
        assert!(html.contains(&format!(
            "style=\"--cui-token-sample-value:var({});\"",
            token.css_variable
        )));
    }
    assert!(html.contains("Ánh đèn cuối sân khấu"));
    assert!(html.contains("Nguyễn Thị Ánh Nguyệt vẫn nhớ lời hẹn ấy."));
    assert!(html.contains("class=\"cui-token-mark\""));
    assert!(html.contains("aria-hidden=\"true\""));
}

#[test]
fn token_sample_geometry_stays_preview_scoped_static_and_token_driven() {
    let css = include_str!("../preview/gallery.css");
    for selector in [
        "data-preview-kind=\"color\"",
        "data-preview-kind=\"font-family\"",
        "data-preview-kind=\"font-size\"",
        "data-preview-kind=\"spacing\"",
        "data-preview-kind=\"shape\"",
    ] {
        assert!(css.contains(selector), "missing specimen rule {selector}");
    }
    assert!(css.contains("background: var(--cui-token-sample-value)"));
    assert!(css.contains("gap: var(--cui-token-sample-value)"));
    assert!(css.contains("border-radius: var(--cui-token-sample-value)"));
    assert!(!css.contains("animation:"));
    assert!(!css.contains("transition:"));
    assert!(!css.contains("#"), "preview CSS must not create a palette");
}
