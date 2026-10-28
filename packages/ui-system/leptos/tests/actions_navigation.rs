use cantos_ui::{
    AccessibleName, Button, ButtonState, ButtonVariant, DisabledReason, EmptyAccessibleName,
    FilterChip, IconButton, IconName, InvalidLinkHref, Link, LinkHref, NavItem, NavTabs, Theme,
    ThemeCycleButton,
};
use leptos::prelude::*;

#[test]
fn listen_hero_retains_one_native_action_and_pending_focus_without_claiming_playback() {
    let owner = Owner::new();
    owner.with(|| {
        let icon = RwSignal::new(IconName::PlayArrow);
        let html = view! {
            <Button id="listen-hero" variant=ButtonVariant::ListenHero icon_signal=icon
                state=ButtonState::Loading(DisabledReason::new("Đang chuẩn bị bản nghe…").unwrap())>
                "Đang tải…"
            </Button>
        }.to_html();
        assert_eq!(html.matches("<button ").count(), 1);
        let control = html.split("</button>").next().unwrap();
        assert!(control.contains("data-variant=\"listen-hero\""));
        assert!(control.contains("aria-disabled=\"true\""));
        assert!(!control.contains(" disabled"), "pending retains its focus node: {html}");
        assert!(control.contains("cui-loading-indicator"));
        assert!(!html.contains("Đang phát"));
        let html = view! {
            <Button id="listen-pause" variant=ButtonVariant::ListenHero icon=IconName::Pause>"Tạm dừng"</Button>
        }.to_html();
        assert!(html.contains("cui-icon"));
        assert!(html.contains("aria-hidden=\"true\""));
        assert!(!html.contains("aria-disabled=\"true\""));
    });
}

#[test]
fn pending_actions_replace_decorative_glyphs_with_progress_and_keep_names_and_reasons() {
    let owner = Owner::new();
    owner.with(|| {
        for icon in [None, Some(IconName::Login), Some(IconName::Check)] {
            let html = view! {
                <Button id="pending-action" nostrip:icon=icon state=ButtonState::Loading(
                    DisabledReason::new("Đang chờ phản hồi; không gửi lại.").unwrap()
                )>"Lưu bản nháp của Nguyễn Thị Ánh Nguyệt"</Button>
            }
            .to_html();
            assert!(html.contains("cui-loading-indicator"), "{html}");
            assert!(
                !html.contains("<svg"),
                "pending is neither disclosure nor success: {html}"
            );
            assert!(html.contains("aria-busy=\"true\""), "{html}");
            assert!(html.contains("role=\"status\""), "{html}");
            assert!(html.contains("Lưu bản nháp của Nguyễn Thị Ánh Nguyệt"));
            assert!(html.contains("Đang chờ phản hồi; không gửi lại."));
            assert_eq!(html.matches("<button ").count(), 1);
        }
        let html = view! {
            <IconButton id="pending-icon" icon=IconName::Refresh
                name=AccessibleName::new("Thử lại bản nháp / Retry draft").unwrap()
                state=ButtonState::Loading(DisabledReason::new("Đang chờ / Waiting").unwrap())/>
        }
        .to_html();
        assert!(html.contains("title=\"Thử lại bản nháp / Retry draft\""));
        assert!(html.contains("cui-loading-indicator"));
        assert!(!html.contains("<svg"));
    });
}

#[test]
fn the_action_registry_is_closed_named_and_every_icon_is_decorative() {
    let names: Vec<_> = IconName::ALL.iter().map(|icon| icon.as_str()).collect();
    for expected in [
        "login",
        "logout",
        "home",
        "mail",
        "person_add",
        "save",
        "refresh",
        "archive",
        "delete_outline",
        "mic",
        "play_arrow",
        "pause",
        "cloud_sync",
        "settings",
    ] {
        assert!(names.contains(&expected), "{expected}");
    }
    let mut deduplicated = names.clone();
    deduplicated.sort();
    deduplicated.dedup();
    assert_eq!(names.len(), deduplicated.len());
    for icon in IconName::ALL {
        let html = view! { <cantos_ui::Icon name=*icon/> }.to_html();
        assert!(html.contains("aria-hidden=\"true\""), "{html}");
        assert!(html.contains("focusable=\"false\""), "{html}");
        assert!(!html.contains("<title"));
    }
}

#[test]
fn each_action_state_has_one_activation_and_busy_contract() {
    let reason = DisabledReason::new("Đang lưu bản nháp Sơn Hà Ký").unwrap();
    for (state, can_press, busy, description, attribute) in [
        (ButtonState::Enabled, true, false, None, "enabled"),
        (
            ButtonState::Disabled(reason.clone()),
            false,
            false,
            Some(reason.as_str()),
            "disabled",
        ),
        (
            ButtonState::Loading(reason.clone()),
            false,
            true,
            Some(reason.as_str()),
            "loading",
        ),
    ] {
        assert_eq!(state.can_press(), can_press);
        assert_eq!(state.is_busy(), busy);
        assert_eq!(state.reason().map(DisabledReason::as_str), description);
        assert_eq!(state.as_str(), attribute);
    }
}

#[test]
fn new_variants_and_loading_keep_the_original_visible_action() {
    for variant in [ButtonVariant::Secondary, ButtonVariant::Danger] {
        let html = view! {
            <Button id="save" variant=variant
                state=ButtonState::Loading(DisabledReason::new("Đang lưu <b>bản nháp</b>").unwrap())
                class="app-save" described_by="save-context  save-context-two"
                aria_controls="draft-panel" aria_expanded=Signal::from(true)>
                "Lưu bản nháp"
            </Button>
        }
        .to_html();
        assert!(
            html.contains(&format!("data-variant=\"{}\"", variant.as_str())),
            "{html}"
        );
        assert!(html.contains("data-state=\"loading\""), "{html}");
        assert!(html.contains("aria-busy=\"true\""), "{html}");
        assert!(html.contains("disabled"), "{html}");
        assert!(html.contains("type=\"button\""), "{html}");
        assert!(html.contains("class=\"cui-button app-save\""), "{html}");
        assert!(
            html.contains("aria-describedby=\"save-context save-context-two save-reason\""),
            "{html}"
        );
        assert!(html.contains("aria-controls=\"draft-panel\""), "{html}");
        assert!(html.contains("aria-expanded=\"true\""), "{html}");
        assert!(html.contains("Lưu bản nháp"), "{html}");
        assert!(
            html.contains("Đang lưu &lt;b&gt;bản nháp&lt;/b&gt;"),
            "{html}"
        );
        assert!(!html.contains("<b>"), "{html}");
        assert!(!html.contains("aria-label="), "{html}");
    }
}

#[test]
fn blocked_descriptions_do_not_duplicate_the_generated_reason_id() {
    let html = view! {
        <Button id="publish"
            state=ButtonState::Disabled(DisabledReason::new("Cần duyệt bản diễn").unwrap())
            described_by="publish-reason release-context">
            "Xuất bản tập"
        </Button>
    }
    .to_html();
    assert!(
        html.contains("aria-describedby=\"publish-reason release-context\""),
        "{html}"
    );
}

#[test]
fn icon_only_controls_require_text_names_and_keep_icons_decorative() {
    assert_eq!(AccessibleName::new(" \n "), Err(EmptyAccessibleName));
    let html = view! {
        <IconButton id="close" name=AccessibleName::new("Đóng <b>bảng điều khiển</b>").unwrap()
            icon=IconName::Close class="app-close"/>
    }
    .to_html();
    assert!(
        html.contains("class=\"cui-button cui-icon-button app-close\""),
        "{html}"
    );
    assert!(html.contains("class=\"cui-sr-only\""), "{html}");
    assert!(
        html.contains("Đóng &lt;b&gt;bảng điều khiển&lt;/b&gt;"),
        "{html}"
    );
    assert!(html.contains("aria-hidden=\"true\""), "{html}");
    assert!(html.contains("focusable=\"false\""), "{html}");
    assert!(!html.contains("<b>"), "{html}");
    assert!(!html.contains("aria-label="), "{html}");
}

#[test]
fn destinations_refuse_active_document_schemes_and_control_obfuscation() {
    for invalid in [
        "javascript:alert(1)",
        "JaVaScRiPt:alert(1)",
        "data:text/html,<script>",
        "file:/tmp/private",
    ] {
        assert_eq!(
            LinkHref::new(invalid),
            Err(InvalidLinkHref::UnsupportedScheme)
        );
    }
    assert_eq!(
        LinkHref::new(" \t "),
        Err(InvalidLinkHref::ControlCharacter)
    );
    assert_eq!(
        LinkHref::new("java\nscript:alert(1)"),
        Err(InvalidLinkHref::ControlCharacter)
    );
    assert_eq!(LinkHref::new("  "), Err(InvalidLinkHref::Empty));
    for valid in [
        "#scene:2",
        "/studio/works/1",
        "../episodes/2",
        "?q=Sơn+Hà+Ký",
        "HTTPS://example.test/episode",
        "mailto:hello@example.test",
        "tel:+84123456789",
    ] {
        assert_eq!(LinkHref::new(valid).unwrap().as_str(), valid);
    }
    assert_eq!(LinkHref::new("  #scene  ").unwrap().as_str(), "#scene");
}

#[test]
fn native_link_destinations_and_class_hooks_are_html_escaped() {
    let href = LinkHref::new("#cảnh\" onmouseover=\"alert(1)").unwrap();
    let html = view! {
        <Link id="scene" href=href class="scene\" onclick=\"alert(2)"
            described_by="scene-context">
            "Cảnh <b>Ánh đèn cuối sân khấu</b>"
        </Link>
    }
    .to_html();
    assert!(
        html.contains("href=\"#cảnh&quot; onmouseover=&quot;alert(1)\""),
        "{html}"
    );
    assert!(
        html.contains("class=\"cui-link scene&quot; onclick=&quot;alert(2)\""),
        "{html}"
    );
    assert!(
        html.contains("aria-describedby=\"scene-context\""),
        "{html}"
    );
    assert!(
        html.contains("Cảnh &lt;b&gt;Ánh đèn cuối sân khấu&lt;/b&gt;"),
        "{html}"
    );
    assert!(!html.contains(" onmouseover=\""), "{html}");
    assert!(!html.contains(" onclick=\""), "{html}");
    assert!(!html.contains("role=\"button\""), "{html}");
}

#[test]
fn visual_navigation_tabs_remain_native_links_with_one_current_destination() {
    let html = view! {
        <NavTabs label=AccessibleName::new("Điều hướng thư viện").unwrap()>
            <NavItem id="library" href=LinkHref::new("#library").unwrap() current=true
                icon=IconName::MenuBook>"Thư viện"</NavItem>
            <NavItem id="search" href=LinkHref::new("#search").unwrap()>"Tìm kiếm"</NavItem>
        </NavTabs>
    }
    .to_html();
    assert!(
        html.contains("<nav aria-label=\"Điều hướng thư viện\""),
        "{html}"
    );
    assert_eq!(html.matches("aria-current=\"page\"").count(), 1, "{html}");
    assert!(html.contains("href=\"#library\""), "{html}");
    assert!(html.contains("href=\"#search\""), "{html}");
    assert!(!html.contains("role=\"tab"), "{html}");
    assert!(!html.contains("tabindex="), "{html}");
    assert!(html.contains("Thư viện"), "{html}");
    assert!(html.contains("Tìm kiếm"), "{html}");
}

#[test]
fn chip_selection_is_controlled_and_blocking_does_not_erase_selection() {
    let owner = Owner::new();
    owner.with(|| {
        let selected = RwSignal::new(false);
        let first = view! {
            <FilterChip id="favorite" selected=selected on_change=Callback::new(|_| ())>
                "Truyện đã đánh dấu"
            </FilterChip>
        }.to_html();
        assert!(first.contains("aria-pressed=\"false\""), "{first}");
        selected.set(true);
        let second = view! {
            <FilterChip id="favorite" selected=selected on_change=Callback::new(|_| ())
                state=ButtonState::Disabled(DisabledReason::new("Tải dữ liệu trước khi đổi bộ lọc").unwrap())>
                "Truyện đã đánh dấu"
            </FilterChip>
        }.to_html();
        assert!(second.contains("aria-pressed=\"true\""), "{second}");
        assert!(second.contains("disabled"), "{second}");
        assert!(second.contains("aria-describedby=\"favorite-reason\""), "{second}");
        assert!(second.contains("Tải dữ liệu trước khi đổi bộ lọc"), "{second}");
        assert!(second.contains("Truyện đã đánh dấu"), "{second}");
        assert!(!second.contains("aria-label="), "{second}");
    });
}

#[test]
fn pending_chip_replaces_the_check_in_the_same_slot_and_keeps_selection_meaning() {
    let owner = Owner::new();
    owner.with(|| {
        for selected in [false, true] {
            let ready = view! {
                <FilterChip id="mine" selected=selected on_change=Callback::new(|_| ())>
                    "Truyện của Nguyễn Thị Ánh Nguyệt"
                </FilterChip>
            }
            .to_html();
            assert!(ready.contains("cui-chip-check") && ready.contains("<svg"), "{ready}");
            assert!(!ready.contains("cui-loading-indicator"), "{ready}");

            let pending = view! {
                <FilterChip id="mine" selected=selected on_change=Callback::new(|_| ())
                    state=ButtonState::Loading(DisabledReason::new("Đang lọc; không gửi lại.").unwrap())>
                    "Truyện của Nguyễn Thị Ánh Nguyệt"
                </FilterChip>
            }
            .to_html();
            assert_eq!(pending.matches("cui-chip-check").count(), 1, "same slot: {pending}");
            assert_eq!(pending.matches("cui-loading-indicator").count(), 1, "{pending}");
            assert!(!pending.contains("<svg"), "the check is replaced, not stacked: {pending}");
            assert!(pending.contains(&format!("aria-pressed=\"{selected}\"")), "selection survives: {pending}");
            assert!(pending.contains("aria-busy=\"true\"") && pending.contains("role=\"status\""), "{pending}");
            let control = pending.split_once("<button ").unwrap().1.split('>').next().unwrap();
            assert!(!control.contains(" disabled"), "pending must preserve native focus: {control}");
            assert!(control.contains("aria-disabled=\"true\""), "pending remains non-activatable: {control}");
            assert!(pending.contains("Đang lọc; không gửi lại."), "{pending}");
            assert!(pending.contains("Truyện của Nguyễn Thị Ánh Nguyệt"), "{pending}");
            assert!(!pending.contains("aria-label="), "the visible label is the name: {pending}");
            assert_eq!(pending.matches("<button ").count(), 1, "{pending}");
        }
    });
}

#[test]
fn theme_cycle_shows_the_short_mode_name_only_when_asked_and_keeps_the_full_accessible_name() {
    let owner = Owner::new();
    owner.with(|| {
        let theme = RwSignal::new(Theme::Light);
        let labels = RwSignal::new(cantos_ui::ThemeCycleLabels {
            dark_mode: "Tối",
            dark_next: "Chuyển sang Sáng",
            light_mode: "Sáng",
            light_next: "Chuyển sang Hệ thống",
            system_mode: "Hệ thống",
            system_next: "Chuyển sang Tối",
        });
        assert_eq!(labels.get().mode(Theme::System), "Hệ thống");

        let icon_only = view! { <ThemeCycleButton id="theme-btn" theme=theme labels=labels.into()/> }.to_html();
        assert!(!icon_only.contains("cui-theme-cycle-label"), "{icon_only}");
        assert!(!icon_only.contains("data-label"), "{icon_only}");

        let labelled = view! { <ThemeCycleButton id="theme-btn" theme=theme show_label=true labels=labels.into()/> }.to_html();
        assert!(labelled.contains("data-label=\"visible\""), "{labelled}");
        assert!(
            labelled.contains("<span aria-hidden=\"true\" class=\"cui-theme-cycle-label\">Sáng</span>"),
            "{labelled}"
        );
        // The accessible name still starts with the visible text (WCAG label in name).
        assert!(labelled.contains("aria-label=\"Sáng · Chuyển sang Hệ thống\""), "{labelled}");
    });
}

#[test]
fn theme_cycle_steps_dark_light_system_dark_and_renders_mode_next_tooltip() {
    assert_eq!(Theme::Dark.cycle(), Theme::Light);
    assert_eq!(Theme::Light.cycle(), Theme::System);
    assert_eq!(Theme::System.cycle(), Theme::Dark);

    let owner = Owner::new();
    owner.with(|| {
        let theme = RwSignal::new(Theme::Dark);
        let labels = RwSignal::new(cantos_ui::ThemeCycleLabels {
            dark_mode: "Tối",
            dark_next: "Chuyển sang Sáng",
            light_mode: "Sáng",
            light_next: "Chuyển sang Hệ thống",
            system_mode: "Hệ thống",
            system_next: "Chuyển sang Tối",
        });

        let dark_html = view! {
            <ThemeCycleButton id="theme-btn" theme=theme labels=labels.into()/>
        }
        .to_html();
        assert!(dark_html.contains("id=\"theme-btn\""), "{dark_html}");
        assert!(dark_html.contains("cui-theme-cycle-button"), "{dark_html}");
        assert!(dark_html.contains("data-theme=\"dark\""), "{dark_html}");
        assert!(
            dark_html.contains("aria-label=\"Tối · Chuyển sang Sáng\""),
            "{dark_html}"
        );
        assert!(dark_html.contains("role=\"tooltip\""), "{dark_html}");
        assert!(dark_html.contains("Tối · Chuyển sang Sáng"), "{dark_html}");
        assert!(dark_html.contains("M9.37"), "{dark_html}");

        theme.set(Theme::Light);
        let light_html = view! {
            <ThemeCycleButton id="theme-btn" theme=theme labels=labels.into()/>
        }
        .to_html();
        assert!(light_html.contains("data-theme=\"light\""), "{light_html}");
        assert!(
            light_html.contains("aria-label=\"Sáng · Chuyển sang Hệ thống\""),
            "{light_html}"
        );
        assert!(
            light_html.contains("Sáng · Chuyển sang Hệ thống"),
            "{light_html}"
        );
        assert!(light_html.contains("M12,9"), "{light_html}");

        theme.set(Theme::System);
        let system_html = view! {
            <ThemeCycleButton id="theme-btn" theme=theme labels=labels.into()/>
        }
        .to_html();
        assert!(
            system_html.contains("data-theme=\"system\""),
            "{system_html}"
        );
        assert!(
            system_html.contains("aria-label=\"Hệ thống · Chuyển sang Tối\""),
            "{system_html}"
        );
        assert!(
            system_html.contains("Hệ thống · Chuyển sang Tối"),
            "{system_html}"
        );
        assert!(
            system_html.contains("cui-theme-icon-system"),
            "{system_html}"
        );

        labels.set(cantos_ui::ThemeCycleLabels {
            dark_mode: "Dark",
            dark_next: "Switch to Light",
            light_mode: "Light",
            light_next: "Switch to System",
            system_mode: "System",
            system_next: "Switch to Dark",
        });
        let en_html = view! {
            <ThemeCycleButton id="theme-btn-en" theme=theme labels=labels.into()/>
        }
        .to_html();
        assert!(
            en_html.contains("aria-label=\"System · Switch to Dark\""),
            "{en_html}"
        );
        assert!(en_html.contains("System · Switch to Dark"), "{en_html}");
    });
}

#[test]
fn icon_button_renders_accessible_tooltip_and_keeps_accessible_name() {
    let owner = Owner::new();
    owner.with(|| {
        let name = AccessibleName::new("Đóng bảng").unwrap();
        let html = view! {
            <IconButton id="test-close" name=name icon=IconName::Close/>
        }
        .to_html();
        assert!(html.contains("id=\"test-close\""), "{html}");
        assert!(html.contains("id=\"test-close-tooltip\""), "{html}");
        assert!(html.contains("role=\"tooltip\""), "{html}");
        assert!(html.contains("class=\"cui-tooltip\""), "{html}");
        assert!(html.contains("hidden"), "{html}");
        assert!(html.contains("cui-sr-only"), "{html}");
        assert!(html.contains("Đóng bảng"), "{html}");
    });
}
