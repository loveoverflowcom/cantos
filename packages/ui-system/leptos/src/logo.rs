//! Offline, font-independent official Cantos Logo Kit v2 vector documents.
use crate::AccessibleName;
use leptos::prelude::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogoVariant {
    #[default]
    Cantos,
    CantosStudio,
    Symbol,
}

impl LogoVariant {
    pub const ALL: &'static [Self] = &[Self::Cantos, Self::CantosStudio, Self::Symbol];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Cantos => "cantos",
            Self::CantosStudio => "studio",
            Self::Symbol => "symbol",
        }
    }

    pub const fn intrinsic_size(self) -> (u16, u16) {
        match self {
            Self::Cantos | Self::CantosStudio => (283, 96),
            Self::Symbol => (64, 64),
        }
    }

    /// Original, unmodified path artwork, checksum-pinned in the adjacent shared assets.
    pub const fn svg(self, tone: LogoTone) -> &'static str {
        match (self, tone) {
            (Self::Cantos, LogoTone::Light) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-logo.svg")
            }
            (Self::Cantos, LogoTone::Dark) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-logo-dark.svg")
            }
            (Self::Cantos, LogoTone::Mono) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-logo-mono.svg")
            }
            (Self::CantosStudio, LogoTone::Light) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-studio-logo.svg")
            }
            (Self::CantosStudio, LogoTone::Dark) => include_str!(
                "../../shared/assets/cantos-logo-kit-v2/svg/cantos-studio-logo-dark.svg"
            ),
            (Self::CantosStudio, LogoTone::Mono) => include_str!(
                "../../shared/assets/cantos-logo-kit-v2/svg/cantos-studio-logo-mono.svg"
            ),
            (Self::Symbol, LogoTone::Light) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-symbol.svg")
            }
            (Self::Symbol, LogoTone::Dark) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-symbol-dark.svg")
            }
            (Self::Symbol, LogoTone::Mono) => {
                include_str!("../../shared/assets/cantos-logo-kit-v2/svg/cantos-symbol-mono.svg")
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LogoTone {
    #[default]
    Light,
    Dark,
    Mono,
}

impl LogoTone {
    pub const ALL: &'static [Self] = &[Self::Light, Self::Dark, Self::Mono];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::Mono => "mono",
        }
    }
}

fn vector_uri(svg: &str) -> String {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    let mut uri = String::with_capacity(svg.len() * 2);
    uri.push_str("data:image/svg+xml,");
    for byte in svg.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            uri.push(char::from(byte));
        } else {
            uri.push('%');
            uri.push(char::from(HEX[usize::from(byte >> 4)]));
            uri.push(char::from(HEX[usize::from(byte & 15)]));
        }
    }
    uri
}

/// Original SVG vector, not a raster image or a text reconstruction.
///
/// A self-contained image document isolates repeated monochrome mask IDs. The default empty
/// alt hides decorative artwork beside a heading or named control. Only pass a localized
/// `accessible_name` for a standalone mark. The caller selects the tone for its paired surface
/// and provides symbol clear space; CSS preserves the original aspect ratio.
/// New app adoption remains blocked until both preview renderers qualify.
#[component]
pub fn Logo(
    #[prop(optional, into)] variant: Signal<LogoVariant>,
    #[prop(optional, into)] tone: Signal<LogoTone>,
    #[prop(optional)] accessible_name: Option<AccessibleName>,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    let label = accessible_name.map(|name| name.as_str().to_owned());
    let decorative = label.is_none();
    view! {
        <img class=move || format!("cui-logo {}", class.get())
            src=move || vector_uri(variant.get().svg(tone.get()))
            width=move || variant.get().intrinsic_size().0
            height=move || variant.get().intrinsic_size().1
            data-logo-variant=move || variant.get().as_str()
            data-logo-tone=move || tone.get().as_str()
            alt=label.unwrap_or_default()
            aria-hidden=decorative.then_some("true")
            draggable="false"/>
    }
}
