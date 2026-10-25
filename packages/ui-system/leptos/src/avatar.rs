use leptos::prelude::*;

use crate::AccessibleName;

/// Single is the Google/YouTube account default; Pair takes the first and last named words,
/// so a Vietnamese name keeps its given name ("Nguyễn Thị Ánh" → "NÁ").
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarInitials {
    #[default]
    Single,
    Pair,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarSize {
    Small,
    #[default]
    Medium,
    /// A 48-unit header control: the same hit area as an icon action, never smaller.
    Control,
    Large,
}

impl AvatarSize {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Control => "control",
            Self::Large => "large",
        }
    }
}

/// Stable identity hue, never a status. Order is part of the shared contract: the index is
/// FNV-1a(seed) mod 6 on both renderers (see `packages/ui-system/shared/README.md#avatar`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarTone {
    Lavender,
    Gold,
    Sage,
    Rose,
    Sky,
    Clay,
}

impl AvatarTone {
    pub const ALL: [Self; 6] = [
        Self::Lavender,
        Self::Gold,
        Self::Sage,
        Self::Rose,
        Self::Sky,
        Self::Clay,
    ];

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Lavender => "lavender",
            Self::Gold => "gold",
            Self::Sage => "sage",
            Self::Rose => "rose",
            Self::Sky => "sky",
            Self::Clay => "clay",
        }
    }
}

/// Text fallback for a person or creator. Returns "" when no word starts with a letter or
/// digit; the avatar then shows its plain tone, as Google does, instead of a "?" that reads as
/// an error. Combining marks after the initial are kept, so decomposed Vietnamese stays intact.
pub fn avatar_initials(display_name: &str, mode: AvatarInitials) -> String {
    let letters: Vec<String> = display_name
        .split_whitespace()
        .filter_map(word_initial)
        .collect();
    match (letters.first(), letters.last()) {
        (None, _) => String::new(),
        (Some(first), Some(last)) if mode == AvatarInitials::Pair && letters.len() > 1 => {
            format!("{first}{last}")
        }
        (Some(first), _) => first.clone(),
    }
}

/// Colour seed: a caller-owned stable ID (account or creator ID) when present, so a rename
/// keeps the colour; otherwise the display name with whitespace runs collapsed.
pub fn avatar_seed(display_name: &str, stable_id: Option<&str>) -> String {
    match stable_id.map(str::trim).filter(|id| !id.is_empty()) {
        Some(id) => id.to_owned(),
        None => display_name
            .split_whitespace()
            .collect::<Vec<_>>()
            .join(" "),
    }
}

/// FNV-1a over UTF-8; never `DefaultHasher` or other randomized hashing.
pub fn avatar_tone(seed: &str) -> AvatarTone {
    let hash = seed.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    AvatarTone::ALL[(hash % AvatarTone::ALL.len() as u32) as usize]
}

// Supplementary-plane characters are skipped to match Kotlin common, which classifies UTF-16
// units only. `is_alphabetic` minus numeric and marks approximates Unicode letters (`L*`).
fn starts_initial(character: char) -> bool {
    u32::from(character) <= 0xFFFF
        && (character.is_ascii_digit()
            || character.is_alphabetic()
                && !character.is_numeric()
                && !is_combining_mark(character))
}

fn is_combining_mark(character: char) -> bool {
    matches!(u32::from(character),
        0x0300..=0x036F | 0x1AB0..=0x1AFF | 0x1DC0..=0x1DFF | 0x20D0..=0x20FF | 0xFE20..=0xFE2F)
}

fn word_initial(word: &str) -> Option<String> {
    let mut characters = word
        .chars()
        .skip_while(|character| !starts_initial(*character));
    // Uppercasing may expand one character ("ß" → "SS"); one avatar slot keeps the first.
    let first = characters.next()?.to_uppercase().next()?;
    Some(
        std::iter::once(first)
            .chain(characters.take_while(|character| is_combining_mark(*character)))
            .collect(),
    )
}

/// Image boundary: HTTPS, same-origin root-relative paths, object URLs for private media and
/// inline raster/SVG data. Plain HTTP (mixed content), protocol-relative and other schemes are
/// refused, and the initials show instead.
pub fn avatar_image_src(raw: &str) -> Option<String> {
    let source = raw.trim();
    let lower = source.to_ascii_lowercase();
    let same_origin = source.starts_with('/') && !source.starts_with("//");
    let allowed = same_origin
        || lower.starts_with("https://")
        || lower.starts_with("blob:")
        || lower.starts_with("data:image/");
    allowed.then(|| source.to_owned())
}

/// Person or creator avatar: a caller-authorized picture over a stable tone, or initials.
///
/// The host owns which picture is authorized, size limits and caching; [`avatar_image_src`]
/// only refuses unsafe URL forms. Initials stay underneath while the picture loads and return when it
/// fails, so there is no blank frame. Decorative by default because adjacent text supplies the
/// name; pass a localized `accessible_name` only when the avatar stands alone.
#[component]
pub fn Avatar(
    #[prop(into)] display_name: Signal<String>,
    #[prop(optional, into)] stable_id: Signal<Option<String>>,
    #[prop(optional, into)] image_src: Signal<Option<String>>,
    #[prop(optional)] size: AvatarSize,
    #[prop(optional)] initials: AvatarInitials,
    /// Pins the Theatre accent pair (`color.theatrePink` / `color.theatreOnPink`) instead of the
    /// identity hue. The hue stays the stable identity default; the host decides when to pin.
    #[prop(optional)]
    accent: bool,
    #[prop(optional)] accessible_name: Option<AccessibleName>,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    let failed_src = RwSignal::new(None::<String>);
    let tone = move || {
        if accent {
            return "accent";
        }
        let stable_id = stable_id.get();
        avatar_tone(&avatar_seed(&display_name.get(), stable_id.as_deref())).as_str()
    };
    // A failure is remembered per URL, so a new picture is tried again.
    let visible_src = move || {
        image_src
            .get()
            .and_then(|src| avatar_image_src(&src))
            .filter(|src| failed_src.get().as_ref() != Some(src))
    };
    let label = accessible_name.map(|name| name.as_str().to_owned());
    let decorative = label.is_none();
    view! {
        <span class=move || format!("cui-avatar {}", class.get())
            data-size=size.as_str() data-tone=tone
            data-initials=move || if initials == AvatarInitials::Pair { "pair" } else { "single" }
            role=(!decorative).then_some("img") aria-label=label
            aria-hidden=decorative.then_some("true")>
            <span class="cui-avatar-initials" aria-hidden="true">
                {move || avatar_initials(&display_name.get(), initials)}
            </span>
            {move || visible_src().map(|src| {
                let failed = src.clone();
                view! {
                    <img class="cui-avatar-image" src=src alt="" decoding="async" loading="lazy"
                        referrerpolicy="no-referrer"
                        on:error=move |_| failed_src.set(Some(failed.clone()))/>
                }
            })}
        </span>
    }
}
