use cantos_ui::{
    avatar_initials, avatar_tone, safe_avatar_image_src, AccessibleName, Avatar, AvatarInitialMode,
    AvatarSize, AvatarTone, UiRoot,
};
use leptos::prelude::*;

#[test]
fn unicode_initials_match_cmp_contract() {
    assert_eq!(avatar_initials("  Nguyễn Thị Ánh  ", AvatarInitialMode::Single), "N");
    assert_eq!(avatar_initials("__Đỗ Văn An", AvatarInitialMode::Single), "Đ");
    assert_eq!(avatar_initials("岑 昕", AvatarInitialMode::Pair), "岑昕");
    assert_eq!(avatar_initials("Nguyễn Thị Ánh", AvatarInitialMode::Pair), "NÁ");
    assert_eq!(avatar_initials("__Đỗ Văn An", AvatarInitialMode::Pair), "ĐA");
    assert_eq!(avatar_initials("  😀 \n \t ", AvatarInitialMode::Single), "?");
    assert_eq!(avatar_initials("ß", AvatarInitialMode::Single), "S");
}

#[test]
fn fnv_palette_vectors_are_equal_across_rust_and_kotlin() {
    assert_eq!(avatar_tone("user-42"), AvatarTone::Quiet);
    assert_eq!(avatar_tone(" user-42 "), AvatarTone::Quiet);
    assert_eq!(avatar_tone("user-1"), AvatarTone::Soft);
    assert_eq!(avatar_tone("alice@example.com"), AvatarTone::Elevated);
    assert_eq!(avatar_tone(""), AvatarTone::Strong);
    assert_eq!(avatar_tone("Nguyễn Thị Ánh"), AvatarTone::Soft);
}

#[test]
fn image_src_never_accepts_scripts_unsafe_schemes_or_protocol_relative_urls() {
    assert_eq!(safe_avatar_image_src(" https://cdn.example.test/me.png "), Some("https://cdn.example.test/me.png".into()));
    assert_eq!(safe_avatar_image_src("/media/me.png"), Some("/media/me.png".into()));
    for invalid in [" ", "javascript:alert(1)", "data:image/svg+xml,<svg/>", "//other.example/me.png", "/\\unsafe", "https://bad.test/\nheader", "http://example.test/me.png"] {
        assert_eq!(safe_avatar_image_src(invalid), None, "{invalid}");
    }
}

#[test]
fn decorative_and_named_avatars_keep_visible_fallback_and_safe_semantics() {
    let html = view! {
        <UiRoot>
            <Avatar display_name="Nguyễn Thị Ánh" initials=AvatarInitialMode::Pair size=AvatarSize::Large/>
            <Avatar display_name="Đỗ An" stable_id="user-42".to_owned()
                accessible_name=AccessibleName::new("Ảnh đại diện của Đỗ An").unwrap()
                image_url=Signal::derive(|| "/missing.png".to_owned())/>
        </UiRoot>
    }.to_html();
    assert!(html.contains("cui-avatar"), "{html}");
    assert!(html.contains("NÁ"), "{html}");
    assert!(html.contains("data-size=\"large\""), "{html}");
    assert!(html.contains("data-tone=\"quiet\""), "{html}");
    assert!(html.contains("aria-hidden=\"true\""), "{html}");
    assert!(html.contains("role=\"img\""), "{html}");
    assert!(html.contains("Ảnh đại diện của Đỗ An"), "{html}");
    assert!(html.contains("cui-avatar-image"), "{html}");
}

#[test]
fn html_escapes_user_supplied_name_and_never_inserts_raw_markup() {
    let html = view! { <Avatar display_name="<script>alert('x')</script>"/> }.to_html();
    assert!(!html.contains("<script>"), "{html}");
    assert!(html.contains("cui-avatar"), "{html}");
}
