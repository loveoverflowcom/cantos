//! The public cover of a Work, shown only when the Server authorized it (Discovery v1.2).
//!
//! The image sits over the existing accessible fallback, which stays exactly what it is without a
//! cover: no artwork is invented and no other Work's image is borrowed. The element is used with
//! the URL the Server issued (see `application::public_cover`). A load error (revoked, expired,
//! withdrawn, unavailable, corrupt) removes the image and leaves the fallback; the title, link and
//! playback never depend on it. The cover is a cross-origin `<img>`: it needs no CORS because the
//! page never reads its pixels, sends no credentials or referrer and runs no script from it.
use crate::contract::discovery::CoverImage;
use leptos::prelude::*;

/// What the listener currently sees; the markup mirrors it in `data-cover` for styling and tests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum CoverPhase {
    /// The Work has no authorized cover.
    None,
    /// An authorized cover is loading; the fallback stays visible.
    Loading,
    Shown,
    /// The image failed to load; the fallback stays for the rest of this view.
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

/// One cover frame. `children` is the decorative fallback; it is hidden from assistive technology
/// because the image (or the surrounding title link) names the Work, and never read twice.
#[component]
pub(crate) fn PublicCover(
    cover: Option<CoverImage>,
    /// The Work's content language, which also applies to its alternative text.
    lang: String,
    /// The Work kind, kept as the frame's `data-kind` hook.
    kind: &'static str,
    /// Extra frame class beyond `theatre-cover`.
    class: &'static str,
    children: Children,
) -> impl IntoView {
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
        <div class=format!("theatre-cover {class}") data-kind=kind
            data-cover=move || phase.get().name() style=ratio>
            <div class="theatre-cover-fallback" aria-hidden="true">{children()}</div>
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
            <PublicCover cover lang="vi-VN".to_owned() kind="story" class="theatre-public-cover">
                <small>"Truyện"</small>
            </PublicCover>
        }
        .to_html()
    }

    #[test]
    fn an_authorized_cover_renders_the_issued_url_with_its_alt_text_over_the_fallback() {
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
        // The fallback stays and stays decorative; the image alone names the cover.
        assert!(
            html.contains("aria-hidden=\"true\" class=\"theatre-cover-fallback\""),
            "{html}"
        );
        assert!(html.contains("Truyện"), "{html}");
    }

    #[test]
    fn a_work_without_a_cover_renders_only_the_fallback() {
        let html = render(None);
        assert!(html.contains("data-cover=\"none\""), "{html}");
        assert!(!html.contains("<img"), "{html}");
        assert!(!html.contains("style="), "{html}");
        assert!(html.contains("Truyện"));
    }
}
