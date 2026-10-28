use cantos_ui::{
    Announcement, Badge, Container, ContainerWidth, EmptyState, ErrorState, Grid, GridColumns,
    LoadingState, Space, Stack, StackDirection, Status, StatusTone, Surface, SurfacePulse,
    SurfaceTone,
};
use leptos::prelude::*;

#[test]
fn layouts_compose_without_interactive_roles_and_preserve_class_and_long_text() {
    let html = view! {
        <Container width=ContainerWidth::Reading class="caller-container">
            <Surface tone=SurfaceTone::Reading inset=Space::Sm>
                <Stack direction=StackDirection::Horizontal gap=Space::Lg>
                    <Grid columns=GridColumns::Three>
                        <p>"Ánh đèn cuối sân khấu — một lời hẹn chưa bao giờ nói ra"</p>
                    </Grid>
                </Stack>
            </Surface>
        </Container>
    }
    .to_html();
    for attribute in [
        "data-width=\"reading\"",
        "data-tone=\"reading\"",
        "data-inset=\"sm\"",
        "data-direction=\"horizontal\"",
        "data-gap=\"lg\"",
        "data-columns=\"three\"",
        "caller-container",
    ] {
        assert!(html.contains(attribute), "{html}");
    }
    assert!(html.contains("chưa bao giờ nói ra"), "{html}");
    assert!(!html.contains("role="), "{html}");
}

#[test]
fn every_status_tone_has_text_and_offline_decorative_icon() {
    for tone in [
        StatusTone::Neutral,
        StatusTone::Info,
        StatusTone::Success,
        StatusTone::Warning,
        StatusTone::Error,
    ] {
        let html = view! { <Badge tone=tone>"Trạng thái có tên"</Badge> }.to_html();
        assert!(html.contains("Trạng thái có tên"), "{html}");
        assert!(html.contains("aria-hidden=\"true\""), "{html}");
        assert!(
            html.contains(&format!("data-tone=\"{}\"", tone.as_str())),
            "{html}"
        );
        assert!(!html.contains("role="), "{html}");
    }
}

#[test]
fn status_announcements_are_opt_in_and_assertive_is_explicit() {
    assert_eq!(Announcement::Off.role(), None);
    assert_eq!(Announcement::Polite.role(), Some("status"));
    assert_eq!(Announcement::Assertive.role(), Some("alert"));
    let off = view! { <Status>"Bản nháp còn nguyên"</Status> }.to_html();
    assert!(!off.contains("role="), "{off}");
    let polite = view! { <Status announcement=Announcement::Polite>"Đã lưu"</Status> }.to_html();
    assert!(polite.contains("role=\"status\""), "{polite}");
    assert!(polite.contains("aria-atomic=\"true\""), "{polite}");
}

#[test]
fn feedback_is_escaped_and_recovery_children_remain_available() {
    let html = view! {
        <EmptyState title="Chưa có <tác phẩm>"><a href="#new">"Thêm tác phẩm"</a></EmptyState>
        <ErrorState title="Lỗi <kết nối>" announcement=Announcement::Assertive>
            <button type="button">"Thử lại"</button>
        </ErrorState>
        <LoadingState>"Đang tải danh sách"</LoadingState>
    }
    .to_html();
    for text in [
        "Chưa có &lt;tác phẩm&gt;",
        "Lỗi &lt;kết nối&gt;",
        "Thêm tác phẩm",
        "Thử lại",
        "data-state=\"loading\"",
        "role=\"alert\"",
    ] {
        assert!(html.contains(text), "{html}");
    }
}

#[test]
fn loading_announcement_is_not_permanently_suppressed_by_its_own_busy_state() {
    let html = view! { <LoadingState announcement=Announcement::Polite>"Đang tải"</LoadingState> }
        .to_html();
    assert!(html.contains("role=\"status\""), "{html}");
    assert!(!html.contains("aria-busy="), "{html}");
}

#[test]
fn loading_state_shows_one_shared_indicator_that_is_decorative_and_never_a_glyph() {
    let html = view! { <LoadingState>"Đang tải danh sách"</LoadingState> }.to_html();
    assert_eq!(html.matches("cui-loading-indicator").count(), 1, "{html}");
    assert!(
        html.contains("<span aria-hidden=\"true\" class=\"cui-loading-indicator\">"),
        "{html}"
    );
    assert!(
        !html.contains("<svg") && !html.contains("cui-loading-mark"),
        "{html}"
    );
    assert!(
        html.contains("Đang tải danh sách"),
        "the label carries the status: {html}"
    );
}

/// Every endless loop is a named loading primitive tied to its own mark and removed under reduced
/// motion; otherwise a stationary state would still wake the compositor or an unrelated rule would
/// loop forever. A new loop needs its own entry here, with its lifecycle and reduced-motion check.
#[test]
fn the_only_endless_animations_are_loading_primitives_and_reduced_motion_removes_them() {
    let css = include_str!("../foundation.css");
    let endless: Vec<_> = css
        .lines()
        .filter(|line| line.contains("infinite"))
        .collect();
    assert_eq!(endless.len(), 2, "{endless:?}");
    for (animation, mark) in [
        ("cui-loading-turn", ".cui-loading-indicator"),
        ("cui-surface-pulse", ".cui-surface-pulse"),
    ] {
        assert_eq!(
            endless
                .iter()
                .filter(|line| line.contains(animation))
                .count(),
            1,
            "{animation}: {endless:?}"
        );
        assert!(
            css.contains(&format!("@keyframes {animation}")),
            "{animation}"
        );
        let reduced = css
            .match_indices("@media (prefers-reduced-motion: reduce)")
            .map(|(start, _)| &css[start..])
            .map(|rest| &rest[..rest.find("\n}").map_or(rest.len(), |end| end + 2)])
            .find(|block| block.contains(mark))
            .unwrap_or_else(|| panic!("a reduced-motion block for {mark}"));
        assert!(reduced.contains("animation: none"), "{reduced}");
    }
    assert!(css.contains("--cantos-web-loading-turn: 1000ms"));
}

#[test]
fn surface_pulse_is_one_decorative_fill_whose_geometry_belongs_to_the_caller() {
    let html = view! { <SurfacePulse class="caller-geometry"/> }.to_html();
    assert!(html.contains("aria-hidden=\"true\""), "{html}");
    assert!(html.contains("cui-surface-pulse caller-geometry"), "{html}");
    assert!(html.contains("data-pulse=\"running\""), "{html}");
    for forbidden in ["role=", "aria-label", "tabindex", "<svg", "<img", "style="] {
        assert!(!html.contains(forbidden), "{forbidden}: {html}");
    }
}

#[test]
fn surface_pulse_stops_when_inactive_without_leaving_the_layout() {
    let html = view! { <SurfacePulse active=false/> }.to_html();
    assert!(html.contains("data-pulse=\"idle\""), "{html}");
    assert!(
        html.contains("cui-surface-pulse"),
        "the fill keeps its box: {html}"
    );
}

#[test]
fn surface_pulse_css_fades_opacity_over_the_canonical_cycle_with_no_sweep() {
    let css = include_str!("../foundation.css");
    let animation = css
        .lines()
        .find(|line| line.contains("animation: cui-surface-pulse"))
        .expect("the SurfacePulse animation");
    assert!(
        animation.contains("var(--cantos-motion-loading-pulse-cycle)")
            && animation.contains("var(--cantos-motion-loading-pulse-easing)"),
        "{animation}"
    );
    let keyframes = &css[css.find("@keyframes cui-surface-pulse").unwrap()..];
    let keyframes = &keyframes[..keyframes.find("\n}").unwrap()];
    for stop in [
        "0%, 100% { opacity: var(--cantos-state-loading-pulse-low-opacity); }",
        "50% { opacity: var(--cantos-state-loading-pulse-high-opacity); }",
    ] {
        assert!(keyframes.contains(stop), "{keyframes}");
    }
    assert_eq!(keyframes.matches(" opacity:").count(), 2, "{keyframes}");
    assert!(!keyframes.contains("transform") && !keyframes.contains("gradient"));
    let rule = &css[css.find(".cui-surface-pulse {").unwrap()..];
    let rule = &rule[..rule.find('}').unwrap()];
    assert!(
        rule.contains("opacity: var(--cantos-state-loading-pulse-reduced-opacity)"),
        "the resting and reduced fill is the static middle opacity: {rule}"
    );
    assert!(
        !rule.contains("gradient") && !rule.contains("url("),
        "one solid fill, no asset: {rule}"
    );
    let tokens = include_str!("../tokens.css");
    assert!(tokens.contains("--cantos-motion-loading-pulse-cycle: 1400ms;"));
    assert!(tokens.contains("--cantos-state-loading-pulse-reduced-opacity: 0.7;"));
}

#[test]
fn the_loading_reveal_is_a_scoped_finite_duration_that_reduced_motion_zeroes() {
    let tokens = include_str!("../tokens.css");
    assert!(tokens.contains("--cantos-motion-loading-reveal-duration: 150ms;"));
    assert!(tokens.contains("--cantos-motion-loading-reveal-duration: 0ms;"));
    assert!(tokens.contains("--cantos-motion-feedback-duration: 140ms;"));
}
