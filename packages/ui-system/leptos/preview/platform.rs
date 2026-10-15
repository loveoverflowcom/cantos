//! Browser adapters for the local gallery. No service or persistent storage is used.

/// A snapshot of observed browser facts, independent of the gallery's injected settings.
#[derive(Clone, Debug, PartialEq)]
pub struct BrowserFacts {
    pub captured_at_iso: String,
    pub user_agent: String,
    pub viewport_width_css_px: f64,
    pub viewport_height_css_px: f64,
    pub device_pixel_ratio: f64,
    pub visibility: String,
    pub prefers_reduced_motion: bool,
    pub font_readiness: String,
}

/// Browser capabilities may be unavailable in native rendering or restricted contexts.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BrowserError {
    Unsupported,
    WindowUnavailable,
    DocumentUnavailable,
    LocationUnavailable,
    HistoryUnavailable,
    QueryReplacementRejected,
    ViewportUnavailable,
    MotionPreferenceUnavailable,
    BrowserIdentityUnavailable,
    DownloadUnavailable,
}

impl std::fmt::Display for BrowserError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Unsupported => "Browser capability unavailable in native rendering",
            Self::WindowUnavailable => "Browser window unavailable",
            Self::DocumentUnavailable => "Browser document unavailable",
            Self::LocationUnavailable => "Current browser location unavailable",
            Self::HistoryUnavailable => "Browser history unavailable",
            Self::QueryReplacementRejected => "Browser rejected the shareable query update",
            Self::ViewportUnavailable => "Browser viewport dimensions unavailable",
            Self::MotionPreferenceUnavailable => "Browser motion preference unavailable",
            Self::BrowserIdentityUnavailable => "Browser identity unavailable",
            Self::DownloadUnavailable => "Browser could not prepare the local metadata download",
        })
    }
}

impl std::error::Error for BrowserError {}

/// Missing browser state is the normal native-test case.
pub fn current_query() -> String {
    #[cfg(target_arch = "wasm32")]
    {
        leptos::web_sys::window()
            .and_then(|window| window.location().search().ok())
            .unwrap_or_default()
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        String::new()
    }
}

/// Replace only the gallery query, keeping its current path and anchor without navigation.
pub fn replace_query(query: &str) -> Result<(), BrowserError> {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::wasm_bindgen::JsValue;

        let window = leptos::web_sys::window().ok_or(BrowserError::WindowUnavailable)?;
        let location = window.location();
        let path = location
            .pathname()
            .map_err(|_| BrowserError::LocationUnavailable)?;
        let anchor = location
            .hash()
            .map_err(|_| BrowserError::LocationUnavailable)?;
        let query = query.trim_start_matches('?');
        let separator = if query.is_empty() { "" } else { "?" };
        let url = format!("{path}{separator}{query}{anchor}");
        let history = window
            .history()
            .map_err(|_| BrowserError::HistoryUnavailable)?;
        // Preserve any host-owned history state; this gallery does not introduce a router.
        let state = history.state().unwrap_or(JsValue::NULL);
        history
            .replace_state_with_url(&state, "", Some(&url))
            .map_err(|_| BrowserError::QueryReplacementRejected)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = query;
        Err(BrowserError::Unsupported)
    }
}

/// Read on demand; callers decide when to refresh and no browser listener is registered.
pub fn read_browser_facts() -> Result<BrowserFacts, BrowserError> {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::web_sys::{FontFaceSetLoadStatus, VisibilityState};

        let window = leptos::web_sys::window().ok_or(BrowserError::WindowUnavailable)?;
        let document = window.document().ok_or(BrowserError::DocumentUnavailable)?;
        let dimension = |value: Result<leptos::wasm_bindgen::JsValue, _>| {
            value
                .ok()
                .and_then(|value| value.as_f64())
                .filter(|value| value.is_finite() && *value >= 0.0)
                .ok_or(BrowserError::ViewportUnavailable)
        };
        let motion = window
            .match_media("(prefers-reduced-motion: reduce)")
            .map_err(|_| BrowserError::MotionPreferenceUnavailable)?
            .ok_or(BrowserError::MotionPreferenceUnavailable)?;

        Ok(BrowserFacts {
            captured_at_iso: leptos::web_sys::js_sys::Date::new_0()
                .to_iso_string()
                .into(),
            user_agent: window
                .navigator()
                .user_agent()
                .map_err(|_| BrowserError::BrowserIdentityUnavailable)?,
            viewport_width_css_px: dimension(window.inner_width())?,
            viewport_height_css_px: dimension(window.inner_height())?,
            device_pixel_ratio: window.device_pixel_ratio(),
            visibility: match document.visibility_state() {
                VisibilityState::Visible => "visible",
                VisibilityState::Hidden => "hidden",
                _ => "unknown",
            }
            .to_owned(),
            prefers_reduced_motion: motion.matches(),
            font_readiness: match document.fonts().status() {
                FontFaceSetLoadStatus::Loaded => "loaded",
                FontFaceSetLoadStatus::Loading => "loading",
                _ => "unknown",
            }
            .to_owned(),
        })
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        Err(BrowserError::Unsupported)
    }
}

/// Prepare a local text file from an explicit user action. Object URLs never escape this helper.
pub fn download_text(filename: &str, content: &str) -> Result<(), BrowserError> {
    #[cfg(target_arch = "wasm32")]
    {
        use leptos::{
            wasm_bindgen::{JsCast, JsValue},
            web_sys::{js_sys::Array, Blob, BlobPropertyBag, HtmlAnchorElement, Url},
        };

        let document = leptos::web_sys::window()
            .ok_or(BrowserError::WindowUnavailable)?
            .document()
            .ok_or(BrowserError::DocumentUnavailable)?;
        let body = document.body().ok_or(BrowserError::DocumentUnavailable)?;
        let anchor = document
            .create_element("a")
            .map_err(|_| BrowserError::DownloadUnavailable)?
            .dyn_into::<HtmlAnchorElement>()
            .map_err(|_| BrowserError::DownloadUnavailable)?;
        anchor.set_download(filename);
        anchor
            .set_attribute("hidden", "")
            .map_err(|_| BrowserError::DownloadUnavailable)?;

        let parts = Array::new();
        parts.push(&JsValue::from_str(content));
        let options = BlobPropertyBag::new();
        options.set_type("text/plain;charset=utf-8");
        let blob = Blob::new_with_str_sequence_and_options(&parts, &options)
            .map_err(|_| BrowserError::DownloadUnavailable)?;
        let object_url = Url::create_object_url_with_blob(&blob)
            .map_err(|_| BrowserError::DownloadUnavailable)?;
        anchor.set_href(&object_url);

        let started = body
            .append_child(&anchor)
            .map(|_| {
                anchor.click();
                anchor.remove();
            })
            .map_err(|_| BrowserError::DownloadUnavailable);
        // Revoke on both the successful and failed append paths; no ongoing resource survives.
        let revoked =
            Url::revoke_object_url(&object_url).map_err(|_| BrowserError::DownloadUnavailable);
        started.and(revoked)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let _ = (filename, content);
        Err(BrowserError::Unsupported)
    }
}
