//! Vectors mirror `packages/ui-system/shared/README.md#avatar` and the CMP AvatarContractTest.

use cantos_ui::{
    avatar_image_src, avatar_initials, avatar_seed, avatar_tone, AccessibleName, Avatar,
    AvatarInitials, AvatarSize, AvatarTone, TOKEN_CSS,
};
use leptos::prelude::*;

use AvatarInitials::{Pair, Single};

#[test]
fn single_initial_follows_google_and_preserves_vietnamese() {
    for (name, expected) in [
        ("  Nguyễn Thị Ánh  ", "N"),
        ("  __Đỗ Văn An", "Đ"),
        ("Người dẫn chuyện", "N"),
        ("岑 昕", "岑"),
        ("6 Nguyễn", "6"),
        ("@cantos.studio", "C"),
        ("ß", "S"),
    ] {
        assert_eq!(avatar_initials(name, Single), expected, "{name}");
    }
}

#[test]
fn decomposed_vietnamese_keeps_its_combining_marks() {
    assert_eq!(avatar_initials("ánh  Dương", Single), "Á");
    assert_eq!(avatar_initials("a\u{301}nh Dương", Single), "A\u{301}");
    assert_eq!(avatar_initials("A\u{301}nh Dương", Pair), "A\u{301}D");
}

#[test]
fn no_letter_gives_a_plain_avatar_instead_of_an_error_glyph() {
    assert_eq!(avatar_initials("  😀 \n \t ", Single), "");
    assert_eq!(avatar_initials("", Single), "");
    assert_eq!(avatar_initials("", Pair), "");
    assert_eq!(avatar_initials("\u{20000} Lan", Single), "L");
}

#[test]
fn pair_mode_uses_first_and_last_named_words() {
    for (name, expected) in [
        ("Nguyễn Thị Ánh", "NÁ"),
        ("__Đỗ Văn An", "ĐA"),
        ("Người dẫn chuyện", "NC"),
        ("岑 昕", "岑昕"),
        ("6 Nguyễn", "6N"),
        ("Nguyễn", "N"),
    ] {
        assert_eq!(avatar_initials(name, Pair), expected, "{name}");
    }
}

#[test]
fn tones_are_fnv1a_vectors_shared_with_cmp() {
    for (seed, expected) in [
        ("user-42", AvatarTone::Rose),
        ("user-1", AvatarTone::Sky),
        ("alice@example.com", AvatarTone::Sage),
        ("", AvatarTone::Gold),
        ("creator-7", AvatarTone::Gold),
        ("Nguyễn Thị Ánh", AvatarTone::Lavender),
        ("Người dẫn chuyện", AvatarTone::Lavender),
    ] {
        assert_eq!(avatar_tone(seed), expected, "{seed}");
    }
}

#[test]
fn stable_id_wins_and_name_seed_ignores_whitespace_noise() {
    assert_eq!(avatar_seed("Nguyễn Thị Ánh", Some(" user-42 ")), "user-42");
    assert_eq!(
        avatar_seed("  Nguyễn   Thị Ánh ", Some("  ")),
        "Nguyễn Thị Ánh"
    );
    assert_eq!(avatar_seed("Nguyễn\tThị\nÁnh", None), "Nguyễn Thị Ánh");
}

#[test]
fn every_tone_and_size_has_a_shared_token() {
    for tone in AvatarTone::ALL {
        for role in ["avatar", "on-avatar"] {
            let variable = format!("--cantos-color-{role}-{}:", tone.as_str());
            assert!(TOKEN_CSS.contains(&variable), "{variable}");
        }
    }
    for size in ["small", "medium", "large"] {
        assert!(TOKEN_CSS.contains(&format!("--cantos-size-avatar-{size}:")));
    }
}

#[test]
fn decorative_avatar_renders_initials_tone_and_no_image_without_a_source() {
    let html = view! {
        <Avatar display_name="Nguyễn Thị Ánh" stable_id=Some("user-42".to_owned())
            size=AvatarSize::Large class="caller-avatar"/>
    }
    .to_html();
    for fragment in [
        "cui-avatar caller-avatar",
        "data-size=\"large\"",
        "data-tone=\"rose\"",
        "data-initials=\"single\"",
        "aria-hidden=\"true\"",
        ">N<",
    ] {
        assert!(html.contains(fragment), "{fragment}: {html}");
    }
    assert!(!html.contains("<img"), "{html}");
    assert!(!html.contains("role="), "{html}");
}

#[test]
fn picture_overlays_initials_with_empty_alt_and_no_referrer() {
    let html = view! {
        <Avatar display_name="Đỗ Văn An" image_src=Some("/media/avatar.webp".to_owned())
            initials=AvatarInitials::Pair/>
    }
    .to_html();
    for fragment in [
        ">ĐA<",
        "src=\"/media/avatar.webp\"",
        "alt=\"\"",
        "referrerpolicy=\"no-referrer\"",
        "data-initials=\"pair\"",
    ] {
        assert!(html.contains(fragment), "{fragment}: {html}");
    }
    let blank = view! { <Avatar display_name="An" image_src=Some("  ".to_owned())/> }.to_html();
    assert!(!blank.contains("<img"), "{blank}");
}

#[test]
fn named_standalone_avatar_is_one_image_for_assistive_technology() {
    let name = AccessibleName::new("Ảnh đại diện của Nguyễn Thị Ánh").expect("name");
    let html = view! { <Avatar display_name="Nguyễn Thị Ánh" accessible_name=name/> }.to_html();
    assert!(html.contains("role=\"img\""), "{html}");
    assert!(
        html.contains("aria-label=\"Ảnh đại diện của Nguyễn Thị Ánh\""),
        "{html}"
    );
    // Only the inner initials are hidden; the named root stays exposed.
    assert_eq!(html.matches("aria-hidden=\"true\"").count(), 1, "{html}");
}

#[test]
fn only_safe_picture_urls_reach_the_image_element() {
    for allowed in [
        "https://cdn.example/a.webp",
        " /media/avatar.webp ",
        "blob:https://studio.example/1f2e",
        "data:image/png;base64,AAAA",
    ] {
        assert!(avatar_image_src(allowed).is_some(), "{allowed}");
    }
    for refused in [
        "http://cdn.example/a.webp",
        "//cdn.example/a.webp",
        "javascript:alert(1)",
        "data:text/html,<p>x</p>",
        "media/relative.webp",
        "",
    ] {
        assert_eq!(avatar_image_src(refused), None, "{refused}");
    }
    let html = view! {
        <Avatar display_name="An" image_src=Some("http://cdn.example/a.webp".to_owned())/>
    }
    .to_html();
    assert!(!html.contains("<img"), "{html}");
    assert!(html.contains(">A<"), "{html}");
}
