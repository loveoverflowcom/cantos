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
