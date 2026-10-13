//! State permission and HTML projections share the same exhaustive production predicates.
use cantos_ui::{ChoiceState, DisabledReason, FieldState, TextField};
use leptos::prelude::*;

#[test]
fn every_text_state_has_one_input_readonly_disabled_busy_and_reason_contract() {
    let reason = DisabledReason::new("Đang lưu bản nháp").unwrap();
    for (state, accepts, readonly, disabled, busy, description, name) in [
        (
            FieldState::Editable,
            true,
            false,
            false,
            false,
            None,
            "editable",
        ),
        (
            FieldState::ReadOnly,
            false,
            true,
            false,
            false,
            None,
            "readonly",
        ),
        (
            FieldState::Disabled(reason.clone()),
            false,
            false,
            true,
            false,
            Some(reason.as_str()),
            "disabled",
        ),
        (
            FieldState::Loading(reason.clone()),
            false,
            false,
            true,
            true,
            Some(reason.as_str()),
            "loading",
        ),
    ] {
        assert_eq!(state.accepts_input(), accepts);
        assert_eq!(state.is_read_only(), readonly);
        assert_eq!(state.is_disabled(), disabled);
        assert_eq!(state.is_loading(), busy);
        assert_eq!(state.reason().map(DisabledReason::as_str), description);
        assert_eq!(state.as_str(), name);
        let html = view! { <TextField id="draft" label="Bản nháp" value="Ngày mai"
        state=state on_input=Callback::new(|_: String| {})/> }
        .to_html();
        let start = html.find("<input ").unwrap();
        let input = &html[start..start + html[start..].find('>').unwrap()];
        assert_eq!(input.contains("readonly"), readonly, "{html}");
        assert_eq!(input.contains("disabled"), disabled, "{html}");
        assert!(input.contains(&format!("aria-busy=\"{busy}\"")), "{html}");
        assert_eq!(
            input.contains("aria-describedby=\"draft-reason\""),
            description.is_some(),
            "{html}"
        );
    }
}

#[test]
fn every_choice_state_has_one_activation_busy_and_reason_contract() {
    let reason = DisabledReason::new("Đang đọc lựa chọn").unwrap();
    for (state, accepts, busy, description, name) in [
        (ChoiceState::Enabled, true, false, None, "enabled"),
        (
            ChoiceState::Disabled(reason.clone()),
            false,
            false,
            Some(reason.as_str()),
            "disabled",
        ),
        (
            ChoiceState::Loading(reason.clone()),
            false,
            true,
            Some(reason.as_str()),
            "loading",
        ),
    ] {
        assert_eq!(state.accepts_input(), accepts);
        assert_eq!(state.is_loading(), busy);
        assert_eq!(state.reason().map(DisabledReason::as_str), description);
        assert_eq!(state.as_str(), name);
    }
}
