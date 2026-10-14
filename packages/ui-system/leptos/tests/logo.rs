use cantos_ui::{AccessibleName, Logo, LogoTone, LogoVariant};
use leptos::prelude::*;

#[test]
fn every_variant_preserves_original_vector_geometry_and_theme_ink() {
    for variant in LogoVariant::ALL {
        let (width, height) = variant.intrinsic_size();
        for (tone, ink) in [
            (LogoTone::Light, "#675080"),
            (LogoTone::Dark, "#D9BDF1"),
            (LogoTone::Mono, "#292331"),
        ] {
            let svg = variant.svg(tone);
            assert!(svg.contains(&format!("viewBox=\"0 0 {width} {height}\"")));
            assert!(svg.contains(ink));
            assert!(svg.contains("<path"));
            for forbidden in [
                "<text",
                "<image",
                "<script",
                "@font",
                "href=",
                "<foreignObject",
            ] {
                assert!(
                    !svg.contains(forbidden),
                    "{variant:?}/{tone:?}: {forbidden}"
                );
            }
            if tone == LogoTone::Mono {
                assert!(svg.contains("<mask id=\"knockout\""));
                assert!(svg.contains("stroke=\"black\""));
                assert!(!svg.contains("#FFF4DA"));
            }
        }
    }
}

#[test]
fn decorative_logo_is_one_nameless_local_vector_image_with_original_aspect() {
    let html = view! { <Logo variant=LogoVariant::CantosStudio tone=LogoTone::Dark/> }.to_html();
    for fragment in [
        "data:image/svg+xml,",
        "%3Cpath",
        "alt=\"\"",
        "aria-hidden=\"true\"",
        "width=\"283\"",
        "height=\"96\"",
        "data-logo-variant=\"studio\"",
        "data-logo-tone=\"dark\"",
    ] {
        assert!(html.contains(fragment), "{fragment}: {html}");
    }
    assert_eq!(html.matches("<img").count(), 1);
    assert!(!html.contains("aria-label="));
    assert!(!html.contains("src=\"http"));
    assert!(!html.contains("data:image/png"));
}

#[test]
fn standalone_logo_has_exactly_one_caller_owned_localized_name() {
    let name = AccessibleName::new("Biểu tượng Cantos").expect("localized name");
    let html = view! { <Logo variant=LogoVariant::Symbol accessible_name=name/> }.to_html();
    assert!(html.contains("alt=\"Biểu tượng Cantos\""));
    assert!(html.contains("width=\"64\""));
    assert!(html.contains("height=\"64\""));
    assert!(!html.contains("aria-hidden="));
    assert!(!html.contains("aria-label="));
    assert_eq!(html.matches("Biểu tượng Cantos").count(), 1);
}

#[test]
fn repeated_mono_marks_keep_mask_ids_in_isolated_image_documents() {
    let html = view! { <Logo tone=LogoTone::Mono/><Logo tone=LogoTone::Mono/> }.to_html();
    assert_eq!(html.matches("data:image/svg+xml,").count(), 2);
    assert_eq!(html.matches("%3Cmask").count(), 2);
    assert!(!html.contains("<mask"));
    assert!(!html.contains("id=\"knockout\""));
}
