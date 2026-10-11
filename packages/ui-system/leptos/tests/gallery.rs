#[allow(dead_code)]
#[path = "../preview/main.rs"]
mod preview;

use leptos::prelude::*;
use std::collections::BTreeSet;

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
