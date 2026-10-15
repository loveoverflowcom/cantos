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
fn loading_preview_discovery_includes_announcing_state_with_native_status_semantics() {
    let loading = preview::preview_catalog::COMPONENTS
        .iter()
        .find(|entry| entry.id == "loading-state")
        .expect("registered LoadingState");
    assert!(loading.states.contains(&"announcement"));
    let html = Owner::new()
        .with(|| view! { <preview::Gallery initial_feedback="announcement"/> }.to_html());
    assert!(html.contains("data-preview-scenario=\"announcement\""));
    let marker = html
        .find("class=\"cui-loading-state")
        .expect("actual LoadingState sample");
    let start = html[..marker]
        .rfind('<')
        .expect("LoadingState element start");
    let end = html[start..]
        .find('>')
        .expect("complete LoadingState opening tag")
        + start;
    assert!(html[start..end].contains("role=\"status\""));
    assert!(html.contains("Thông báo đang tải được bật rõ ràng"));
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

#[test]
fn foundation_scenarios_keep_existing_public_components_and_native_web_limits() {
    let html = Owner::new().with(|| view! { <preview::Gallery/> }.to_html());
    for scenario in [
        "nested-theme",
        "composed-content",
        "blank-error",
        "selection ime-action",
    ] {
        assert!(html.contains(&format!("data-preview-scenario=\"{scenario}\"")));
    }
    let nested = html
        .find("class=\"cantos-ui cui-gallery-nested-root\"")
        .unwrap();
    let tag_start = html[..nested].rfind('<').expect("nested root opening tag");
    let tag_end = nested + html[nested..].find('>').expect("complete nested root tag");
    let nested_root = &html[tag_start..tag_end];
    assert!(nested_root.contains("class=\"cantos-ui cui-gallery-nested-root\""));
    assert!(nested_root.contains("data-theme=\"dark\""));
    assert!(nested_root.contains("data-density=\"comfortable\""));
    assert!(html.contains("id=\"gallery-outer-theme-sibling\""));
    let button = opening_tag(&html, "gallery-composed-button");
    assert!(button.contains("type=\"button\""));
    assert!(html.contains("Đọc lời hẹn dưới chân Thanh Vân Sơn"));
    assert!(html.contains("Số lần nhận hành động có nội dung ghép: 0"));
    for id in [
        "gallery-blank-error-text",
        "gallery-blank-error-area",
        "gallery-blank-error-number",
    ] {
        let control = opening_tag(&html, id);
        assert!(
            control.contains("aria-invalid=\"false\""),
            "blank error marks {id} invalid"
        );
        assert!(
            !control.contains("aria-errormessage"),
            "blank error references an invisible message for {id}"
        );
        assert!(opening_tag(&html, &format!("{id}-error")).contains("hidden"));
    }
    assert!(opening_tag(&html, "gallery-native-number").contains("type=\"number\""));
    assert!(opening_tag(&html, "gallery-native-number").contains("min=\"0\""));
    assert!(opening_tag(&html, "gallery-native-number").contains("max=\"100\""));
    assert!(opening_tag(&html, "gallery-native-number").contains("step=\"1\""));
    assert!(html.contains("không hỗ trợ selectionStart/selectionEnd hoặc setSelectionRange"));
    assert!(html.contains("không chứng minh IME hệ điều hành"));
}

#[test]
fn composed_button_callback_counts_only_enabled_dispatches() {
    use cantos_ui::{ButtonState, DisabledReason};

    Owner::new().with(|| {
        let state = RwSignal::new(ButtonState::Enabled);
        let count = RwSignal::new(0);
        let press = preview::foundation_samples::composed_press(state.into(), count);
        press.run(());
        assert_eq!(count.get_untracked(), 1);
        let reason = DisabledReason::new("Khóa trong bản mẫu").unwrap();
        state.set(ButtonState::Disabled(reason.clone()));
        press.run(());
        state.set(ButtonState::Loading(reason));
        press.run(());
        assert_eq!(count.get_untracked(), 1);
        state.set(ButtonState::Enabled);
        press.run(());
        assert_eq!(count.get_untracked(), 2);
    });
}

#[test]
fn ime_action_guard_preserves_composition_and_locked_field_states() {
    use cantos_ui::{DisabledReason, FieldState};
    use preview::foundation_samples::accepts_ime_action;

    assert!(accepts_ime_action(
        &FieldState::Editable,
        "Enter",
        false,
        13
    ));
    assert!(!accepts_ime_action(
        &FieldState::Editable,
        "Enter",
        true,
        13
    ));
    assert!(!accepts_ime_action(
        &FieldState::Editable,
        "Enter",
        false,
        229
    ));
    assert!(!accepts_ime_action(&FieldState::Editable, "Tab", false, 9));
    let reason = DisabledReason::new("Khóa trong bản mẫu").unwrap();
    for state in [
        FieldState::ReadOnly,
        FieldState::Disabled(reason.clone()),
        FieldState::Loading(reason),
    ] {
        assert!(!accepts_ime_action(&state, "Enter", false, 13));
    }
}

#[test]
fn native_editing_samples_connect_labels_readonly_selection_and_visible_block_reasons() {
    use cantos_ui::{DisabledReason, FieldState};
    use preview::foundation_samples::{NativeEditingSample, NativeFieldKind};

    for kind in [
        NativeFieldKind::Text,
        NativeFieldKind::Area,
        NativeFieldKind::Number,
    ] {
        let key = match kind {
            NativeFieldKind::Text => "text",
            NativeFieldKind::Area => "area",
            NativeFieldKind::Number => "number",
        };
        let id = format!("gallery-native-{key}");
        let html = Owner::new().with(|| {
            view! { <NativeEditingSample kind=kind state=FieldState::ReadOnly/> }.to_html()
        });
        let control = opening_tag(&html, &id);
        assert!(control.contains("readonly"));
        assert!(!control.contains(" disabled"));
        assert!(html.contains(&format!("for=\"{id}\"")));
        assert!(!opening_tag(&html, &format!("{id}-select-all")).contains("disabled"));

        let reason = DisabledReason::new("Trường bị khóa; nội dung vẫn được giữ.").unwrap();
        let html = Owner::new().with(|| {
            view! { <NativeEditingSample kind=kind state=FieldState::Loading(reason)/> }.to_html()
        });
        let control = opening_tag(&html, &id);
        assert!(control.contains("disabled"));
        assert!(control.contains("aria-busy=\"true\""));
        assert!(control.contains(&format!("aria-describedby=\"{id}-help {id}-reason\"")));
        assert!(!opening_tag(&html, &format!("{id}-reason")).contains("hidden"));
        assert!(opening_tag(&html, &format!("{id}-select-all")).contains("disabled"));
    }
}

#[test]
fn stable_selection_samples_render_empty_missing_reordered_and_unsupported_options() {
    use preview::interaction_samples::SelectionContractSamples;
    for mode in ["stable", "reordered", "empty", "missing", "interrupted"] {
        let html =
            Owner::new().with(|| view! { <SelectionContractSamples initial_mode=mode/> }.to_html());
        let select = opening_tag(&html, "gallery-stable-select");
        assert!(html.contains("callback: 0"));
        assert!(html.contains("for=\"gallery-stable-select\""));
        if mode == "empty" || mode == "interrupted" {
            assert!(
                select.contains("disabled"),
                "blocked {mode} remains interactive"
            );
        }
        if mode == "missing" {
            assert!(html.contains("requested: removed"));
        }
        if mode != "empty" {
            assert!(html.contains("value=\"unavailable\" disabled"));
            assert!(html.contains("thiếu quyền dùng giọng"));
        }
        if mode == "reordered" {
            assert!(
                html.find("value=\"minh-phuong\"").unwrap()
                    < html.find("value=\"narrator\"").unwrap()
            );
        }
        let range = opening_tag(&html, "gallery-stepped-range");
        for attribute in [
            "value=\"0\"",
            "min=\"0\"",
            "max=\"100\"",
            "step=\"25\"",
            "aria-valuetext=\"0 trên 100\"",
        ] {
            assert!(range.contains(attribute), "range omits {attribute}");
        }
    }
}

#[test]
fn layout_recovery_samples_keep_reading_insets_wrapping_and_explicit_announcement_policy() {
    let html = Owner::new().with(|| {
        view! { <preview::interaction_samples::LayoutFeedbackContractSamples/> }.to_html()
    });
    assert!(html.contains("data-tone=\"reading\""));
    assert!(html.contains("data-inset=\"none\""));
    assert!(html.contains("data-direction=\"horizontal\""));
    assert!(html.contains("data-gap=\"lg\""));
    assert!(html.contains("Hành động khôi phục: 0"));
    assert!(html.contains("role=\"status\""));
    assert!(!html.contains("role=\"alert\""));
    assert!(opening_tag(&html, "gallery-recovery-retry").contains("type=\"button\""));
    assert!(opening_tag(&html, "gallery-recovery-notes").contains("type=\"button\""));
}

#[test]
fn design_discovery_preserves_baseline_and_never_promotes_a_proposal_to_an_api() {
    let specimens = preview::preview_catalog::DESIGN_SPECIMENS;
    assert_eq!(specimens.len(), 102);
    let components = preview::preview_catalog::COMPONENTS;
    assert_eq!(components.len(), 32);
    assert_eq!(
        components.iter().map(|api| api.states.len()).sum::<usize>(),
        198
    );
    assert_eq!(
        specimens
            .iter()
            .filter(|specimen| specimen.design_status == "proposed")
            .count(),
        70
    );
    for component in components {
        let specimen = specimens
            .iter()
            .find(|entry| entry.id == component.id)
            .expect("baseline specimen");
        assert_eq!(
            preview::catalog::specimen_registration(specimen).map(|api| api.id),
            Some(component.id)
        );
    }
    let proposed = specimens
        .iter()
        .find(|entry| entry.id == "mini-player")
        .unwrap();
    assert!(preview::catalog::specimen_registration(proposed).is_none());
    assert!(preview::catalog::design_matches(
        proposed,
        "proposed",
        "all",
        " MINI-PLAYER "
    ));
    assert!(!preview::catalog::design_matches(
        proposed,
        "existing",
        "all",
        "mini-player"
    ));
    assert!(!preview::catalog::design_matches(
        proposed,
        "all",
        "all",
        "mini-player absent-state"
    ));
    assert!(!preview::catalog::design_matches(
        proposed,
        "all",
        "platform-specific",
        "mini-player"
    ));
}

#[test]
fn v3_workbench_renders_anatomy_states_tokens_and_honest_proposal_limits() {
    let html = Owner::new().with(|| view! { <preview::Gallery/> }.to_html());
    for entry in preview::preview_catalog::DESIGN_SPECIMENS {
        assert!(
            html.contains(&format!("data-design-specimen=\"{}\"", entry.id)),
            "missing {}",
            entry.id
        );
    }
    for text in [
        "Anatomy",
        "Consumer dự kiến",
        "Baseline states được giữ",
        "State matrix thiết kế",
        "Runtime Leptos / CMP",
        "planned · chưa có public API; chưa qualified",
    ] {
        assert!(html.contains(text), "missing {text}");
    }
    assert!(html.contains(preview::preview_catalog::DESIGN_SOURCE_SHA256));
    assert!(html.contains("id=\"gallery-large-text\""));
    assert!(html.contains("id=\"gallery-reduced-motion\""));
    assert!(html.contains("data-text-scale=\"100\""));
    assert!(html.contains("data-motion=\"system\""));
    assert!(!html.contains("role=\"tablist\""));
}

#[test]
fn scenario_workbench_restores_recipe_environment_and_keeps_unique_production_anatomy() {
    let html = Owner::new().with(|| view! {
        <preview::workbench::Workbench initial_query="?v=1&component=auth-primary&state=loading&theme=dark&mode=matrix&locale=en&text=200&frame=320&fixture=long-vietnamese".to_owned()/>
    }.to_html());
    assert!(html.contains("data-theme=\"dark\""));
    assert!(html.contains("data-text-scale=\"200\""));
    assert!(html.contains("--cui-frame-width: 320px"));
    assert!(html.contains("data-variant=\"auth-primary\""));
    assert!(opening_tag(&html, "scenario-auth-primary-loading-control").contains("disabled"));
    assert!(
        opening_tag(&html, "scenario-auth-primary-loading-control").contains("aria-busy=\"true\"")
    );
    assert!(html.contains("Nguyễn Thị Ánh Nguyệt"));
    assert!(!html.contains("role=\"tab\""));
    assert!(!html.contains("<audio"));
    let mut ids = BTreeSet::new();
    for attribute in html.split(" id=\"").skip(1) {
        let id = attribute.split('"').next().unwrap();
        assert!(ids.insert(id), "duplicate workbench ID {id}");
    }
}

#[test]
fn invalid_scenario_has_visible_fallback_without_echoing_untrusted_query() {
    let html = Owner::new().with(|| {
        view! {
            <preview::workbench::Workbench initial_query="?v=999&private=DO_NOT_ECHO".to_owned()/>
        }
        .to_html()
    });
    assert!(opening_tag(&html, "scenario-fallback").contains("role=\"alert\""));
    assert!(html.contains("using the default recipe"));
    assert!(!html.contains("DO_NOT_ECHO"));
    assert!(html.contains("data-scenario-component=\"button\""));
}
