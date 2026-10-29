//! The public cover of a Work, shown only when the Server authorized it (Discovery v1.2).
//!
//! When the Work has no authorized cover (`cover: null`) or its cover fails to load (revoked,
//! expired, withdrawn, unavailable, corrupt), the frame renders the bundled textless 2:3 celestial
//! fallback (`assets/artwork/cantos-cover-fallback.avif`): no artwork is invented from another
//! Work, no text/logo/chip is baked into the cover, and the title, link and playback never depend
//! on the image. While an authorized cover is in flight, `SurfacePulse` is shown without flashing
//! the fallback image; once loaded, only the authorized cover is shown.
use crate::contract::discovery::CoverImage;
use cantos_ui::SurfacePulse;
use leptos::prelude::*;

pub(crate) const COVER_FALLBACK_SRC: &str = "assets/artwork/cantos-cover-fallback.avif";

/// What the listener currently sees; the markup mirrors it in `data-cover` for styling and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoverPhase {
    /// The Work has no authorized cover.
    None,
    /// An authorized cover is loading; `SurfacePulse` is shown while in flight.
    Loading,
    Shown,
    /// The image failed to load; the static celestial fallback stays for the rest of this view.
    Failed,
}

impl CoverPhase {
    const fn name(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Loading => "loading",
            Self::Shown => "shown",
            Self::Failed => "failed",
        }
    }
}

/// One cover frame. The textless celestial fallback is hidden from assistive technology because
/// the surrounding title link names the Work, and an authorized cover image carries its own `alt`.
#[component]
pub(crate) fn PublicCover(
    cover: Option<CoverImage>,
    /// The Work's content language, which also applies to its alternative text.
    lang: String,
    /// The Work kind, kept as the frame's `data-kind` hook.
    kind: &'static str,
    /// Extra frame class beyond `theatre-cover`.
    class: &'static str,
) -> impl IntoView {
    let node = NodeRef::<leptos::html::Div>::new();
    let active = crate::platform::loading_presence(node);
    let phase = RwSignal::new(if cover.is_some() {
        CoverPhase::Loading
    } else {
        CoverPhase::None
    });
    let ratio = cover.as_ref().map(|cover| {
        format!(
            "--theatre-cover-ratio: {} / {}",
            cover.width_px, cover.height_px
        )
    });
    let image = cover.map(|cover| {
        let (width, height) = (cover.width_px, cover.height_px);
        view! {
            <Show when=move || phase.get() != CoverPhase::Failed>
                <img class="theatre-cover-art" src=cover.url.clone() alt=cover.alt_text.clone()
                    lang=lang.clone() width=width height=height decoding="async" loading="lazy"
                    referrerpolicy="no-referrer"
                    on:load=move |_| phase.set(CoverPhase::Shown)
                    on:error=move |_| phase.set(CoverPhase::Failed)/>
            </Show>
        }
    });
    view! {
        <div node_ref=node class=format!("theatre-cover {class}") data-kind=kind
            data-cover=move || phase.get().name() style=ratio>
            <Show when=move || matches!(phase.get(), CoverPhase::None | CoverPhase::Failed)>
                <div class="theatre-cover-fallback" aria-hidden="true">
                    <img class="theatre-cover-fallback-art" src=COVER_FALLBACK_SRC alt="" width="600" height="900" decoding="async" loading="lazy"/>
                </div>
            </Show>
            <Show when=move || phase.get() == CoverPhase::Loading><div class="theatre-cover-pending"><SurfacePulse active/></div></Show>
            {image}
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::discovery::CoveredWorkPage;

    fn covered() -> CoverImage {
        let page: CoveredWorkPage = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/cover/covered_works.json"
        ))
        .unwrap();
        page.items[0].cover.clone().unwrap()
    }

    fn render(cover: Option<CoverImage>) -> String {
        view! {
            <PublicCover cover lang="vi-VN".to_owned() kind="story" class="theatre-public-cover"/>
        }
        .to_html()
    }

    #[test]
    fn an_authorized_cover_renders_the_issued_url_with_its_alt_text_and_loading_pulse_without_flashing_fallback(
    ) {
        let cover = covered();
        let html = render(Some(cover.clone()));
        assert!(html.contains(&format!("src=\"{}\"", cover.url)), "{html}");
        assert!(
            html.contains(&format!("alt=\"{}\"", cover.alt_text)),
            "{html}"
        );
        assert!(html.contains("data-cover=\"loading\""), "{html}");
        assert!(html.contains("lang=\"vi-VN\""), "{html}");
        assert!(html.contains("referrerpolicy=\"no-referrer\""), "{html}");
        assert!(html.contains("width=\"1200\"") && html.contains("height=\"1800\""));
        assert!(
            html.contains("--theatre-cover-ratio: 1200 / 1800"),
            "{html}"
        );
        assert!(html.contains("theatre-cover-pending"), "{html}");
        assert!(!html.contains("theatre-cover-fallback-art"), "{html}");
    }

    #[test]
    fn a_work_without_a_cover_renders_only_the_textless_celestial_avif_fallback() {
        let html = render(None);
        assert!(html.contains("data-cover=\"none\""), "{html}");
        assert!(!html.contains("theatre-cover-art"), "{html}");
        assert!(!html.contains("theatre-cover-pending"), "{html}");
        assert!(!html.contains("style="), "{html}");
        assert!(
            html.contains("aria-hidden=\"true\" class=\"theatre-cover-fallback\"")
                || html.contains("class=\"theatre-cover-fallback\" aria-hidden=\"true\""),
            "{html}"
        );
        assert!(
            html.contains("class=\"theatre-cover-fallback-art\""),
            "{html}"
        );
        assert!(html.contains(COVER_FALLBACK_SRC), "{html}");
        assert!(
            html.contains("alt=\"\"")
                && html.contains("width=\"600\"")
                && html.contains("height=\"900\""),
            "{html}"
        );
    }

    #[test]
    fn an_unobserved_pending_cover_keeps_a_static_placeholder() {
        let html = render(Some(covered()));
        assert!(html.contains("data-cover=\"loading\""));
        assert!(html.contains("data-pulse=\"idle\""), "{html}");
    }

    /// An opaque image over the frame's own fill is masked twice at the rounded edge and the fill
    /// bleeds into the antialiased pixels. Every state an image covers drops the fill; `loading`
    /// keeps it for the pulse, and `shown` drops it only after the fade-in.
    #[test]
    fn a_frame_paints_no_fill_of_its_own_under_an_opaque_image() {
        let css = include_str!("../styles/theatre.css");
        let rule = |selector: &str| {
            css.split(&format!("\n{selector} {{"))
                .nth(1)
                .and_then(|rest| rest.split('}').next())
                .unwrap_or_else(|| panic!("missing rule {selector}"))
        };
        assert!(
            rule(".theatre-cover:is([data-cover=\"none\"], [data-cover=\"failed\"])")
                .contains("background: transparent")
        );
        let shown = rule(".theatre-cover[data-cover=\"shown\"]");
        assert!(shown.contains("background-color: transparent"));
        assert!(shown.contains(
            "transition: background-color 0s linear var(--cantos-motion-loading-reveal-duration)"
        ));
        assert!(!css.contains("[data-cover=\"loading\"] { background"));
        // The one mask stays the frame's own radius and overflow clip, never a second one.
        let frame = rule(".theatre-cover");
        assert!(frame.contains("border-radius: var(--cantos-web-shape-container)"));
        assert!(frame.contains("overflow: hidden"));
        assert!(!rule(".theatre-cover-fallback-art").contains("border-radius"));
        assert!(!rule(".theatre-cover-art").contains("border-radius"));
    }
}
