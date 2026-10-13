use leptos::prelude::*;

use crate::AccessibleName;

/// Render in-place rather than generating a stored raster or asking a provider for an avatar.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarInitialMode {
    #[default]
    Single,
    Pair,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum AvatarSize {
    Small,
    #[default]
    Medium,
    Large,
}

impl AvatarSize {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Large => "large",
        }
    }
}

/// Four theme-token color pairs, with the same stable indices as CMP.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AvatarTone {
    Soft,
    Strong,
    Elevated,
    Quiet,
}

impl AvatarTone {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Soft => "soft",
            Self::Strong => "strong",
            Self::Elevated => "elevated",
            Self::Quiet => "quiet",
        }
    }
}

/// Single mode matches Google's initial avatar; pair uses first/last meaningful name words.
/// The uppercase expansion is bounded to one Unicode scalar per slot (e.g. ß → S).
pub fn avatar_initials(display_name: &str, mode: AvatarInitialMode) -> String {
    let letters: Vec<String> = display_name
        .split_whitespace()
        .filter_map(|word| {
            word.chars()
                .find(|character| character.is_alphanumeric())
                .and_then(|character| character.to_uppercase().next())
                .map(|character| character.to_string())
        })
        .collect();
    match letters.as_slice() {
        [] => "?".to_owned(),
        [first, .., last] if mode == AvatarInitialMode::Pair && letters.len() > 1 => {
            format!("{first}{last}")
        }
        [first, ..] => first.clone(),
    }
}

/// Stable across browser sessions, native devices, and Rust/Kotlin hash implementations.
pub fn avatar_tone(seed: &str) -> AvatarTone {
    let hash = seed
        .trim()
        .as_bytes()
        .iter()
        .fold(0x811c9dc5_u32, |hash, byte| {
            (hash ^ u32::from(*byte)).wrapping_mul(0x01000193)
        });
    match hash % 4 {
        0 => AvatarTone::Soft,
        1 => AvatarTone::Strong,
        2 => AvatarTone::Elevated,
        _ => AvatarTone::Quiet,
    }
}

/// Restrict the image boundary to HTTPS or same-origin root-relative paths.
/// Source URL selection, authorization and caching remain application responsibilities.
pub fn safe_avatar_image_src(raw: &str) -> Option<String> {
    let source = raw.trim();
    if source.is_empty() || source.chars().any(|character| character.is_control() || character == '\\') {
        return None;
    }
    let same_origin = source.starts_with('/') && !source.starts_with("//");
    (source.starts_with("https://") || same_origin).then(|| source.to_owned())
}

/// Purely visual account monogram with optional browser-loaded photo. A failed image
/// reveals initials without changing any account state or persisting a generated image.
/// Decorative by default; a typed accessible name opts in to image semantics.
#[component]
pub fn Avatar(
    #[prop(into)] display_name: Signal<String>,
    #[prop(optional)] stable_id: Option<String>,
    #[prop(optional)] image_url: Option<Signal<String>>,
    #[prop(optional)] size: AvatarSize,
    #[prop(optional)] initials: AvatarInitialMode,
    #[prop(optional)] accessible_name: Option<AccessibleName>,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    let image_failed = RwSignal::new(false);
    let safe_src = Signal::derive(move || image_url.and_then(|url| safe_avatar_image_src(&url.get())));
    // A new URL must not inherit the error state of a previous image.
    Effect::new(move |_| {
        let _ = safe_src.get();
        image_failed.set(false);
    });
    let tone = move || {
        let name = display_name.get();
        let key = stable_id
            .as_deref()
            .filter(|stable| !stable.trim().is_empty())
            .unwrap_or(name.as_str());
        avatar_tone(key).as_str()
    };
    let label = accessible_name.as_ref().map(|name| name.as_str().to_owned());
    let is_decorative = label.is_none();

    view! {
        <span class=move || format!("cui-avatar {}", class.get())
            data-size=size.as_str()
            data-tone=tone
            role=(!is_decorative).then_some("img")
            aria-label=label
            aria-hidden=is_decorative.then_some("true")>
            <span class="cui-avatar-initials" aria-hidden="true">
                {move || avatar_initials(&display_name.get(), initials)}
            </span>
            <Show when=move || safe_src.get().is_some() && !image_failed.get()>
                <img class="cui-avatar-image"
                    src=move || safe_src.get().unwrap_or_default()
                    alt=""
                    on:error=move |_| image_failed.set(true) />
            </Show>
        </span>
    }
}
