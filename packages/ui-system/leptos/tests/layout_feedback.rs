use cantos_ui::{
    Announcement, Badge, Container, ContainerWidth, EmptyState, ErrorState, Grid, GridColumns,
    LoadingState, Space, Stack, StackDirection, Status, StatusTone, Surface, SurfaceTone,
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
        html.contains("<span data-size=\"standard\" aria-hidden=\"true\" class=\"cui-loading-indicator\"></span>"),
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

/// The loop must be tied to the pending mark alone and removed under reduced motion; otherwise a
/// stationary state would still wake the compositor or an unrelated rule would loop forever.
#[test]
fn the_only_endless_animation_is_the_pending_indicator_and_reduced_motion_removes_it() {
    let css = include_str!("../foundation.css");
    let endless: Vec<_> = css
        .lines()
        .filter(|line| line.contains("infinite"))
        .collect();
    assert_eq!(endless.len(), 1, "{endless:?}");
    assert!(endless[0].contains("cui-loading-turn"), "{}", endless[0]);
    assert!(css.contains("@keyframes cui-loading-turn"));
    let reduced = css
        .match_indices("@media (prefers-reduced-motion: reduce)")
        .map(|(start, _)| &css[start..])
        .map(|rest| &rest[..rest.find("\n}").map_or(rest.len(), |end| end + 2)])
        .find(|block| block.contains(".cui-loading-indicator"))
        .expect("a reduced-motion block for the indicator");
    assert!(reduced.contains("animation: none"), "{reduced}");
    assert!(css.contains("--cantos-web-loading-turn: 1000ms"));
}

/// MicroArc: 16 and 20 logical units, a 2-unit stroke, a 270 degree arc and round ends, all from
/// canonical tokens rather than component-local numbers, drawn by the stylesheet so a host that
/// mounts the bare span (Studio's `FormSubmit`) gets the same mark.
#[test]
fn micro_arc_reads_size_stroke_and_caps_from_canonical_tokens() {
    let tokens = include_str!("../tokens.css");
    assert!(tokens.contains("--cantos-size-loading-arc-compact: 1rem;"));
    assert!(tokens.contains("--cantos-size-loading-arc: 1.25rem;"));
    assert!(tokens.contains("--cantos-size-loading-arc-stroke: 0.125rem;"));
    let css = include_str!("../foundation.css");
    let start = css
        .find(":where(.cantos-ui) .cui-loading-indicator {")
        .expect("indicator rule");
    let block = &css[start..start + css[start..].find("\n}").expect("rule end")];
    assert!(
        block.contains("--cantos-web-loading-frame: var(--cantos-size-loading-arc-compact)"),
        "{block}"
    );
    assert!(
        block.contains("conic-gradient(currentColor 0 270deg, transparent 270deg 360deg)"),
        "{block}"
    );
    assert!(
        block.contains("var(--cantos-size-loading-arc-stroke)"),
        "{block}"
    );
    assert!(css.contains(".cui-loading-indicator[data-size=\"standard\"] {\n  --cantos-web-loading-frame: var(--cantos-size-loading-arc);"));
    for end in ["::before", "::after"] {
        assert!(
            css.contains(&format!(":where(.cantos-ui) .cui-loading-indicator{end}")),
            "{end}"
        );
    }
    assert!(css.contains("--cantos-web-loading-cap: var(--cantos-size-loading-arc-stroke)"));
    assert!(
        !css[start..start + 1400].contains("border: var(--cantos-size-focus-width)"),
        "the old 3-unit border ring is gone"
    );
}

/// The arc is smaller than the glyph it replaces, so the host slot (not the arc) holds the footprint.
#[test]
fn glyph_slots_keep_their_footprint_while_the_smaller_arc_replaces_the_glyph() {
    let css = include_str!("../foundation.css");
    let rule = css
        .find(":where(.cui-button-icon, .cui-chip-check, .cui-loading-slot) {")
        .expect("slot rule");
    let block = &css[rule..rule + css[rule..].find("}").expect("end")];
    assert!(
        block.contains("min-inline-size: var(--cantos-size-icon)"),
        "{block}"
    );
    assert!(
        block.contains("min-block-size: var(--cantos-size-icon)"),
        "{block}"
    );
}
