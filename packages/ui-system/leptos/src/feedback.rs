use leptos::prelude::*;

use crate::{Icon, IconName};

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum StatusTone {
    #[default]
    Neutral,
    Info,
    Success,
    Warning,
    Error,
}

impl StatusTone {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Neutral => "neutral",
            Self::Info => "info",
            Self::Success => "success",
            Self::Warning => "warning",
            Self::Error => "error",
        }
    }

    pub const fn icon(self) -> IconName {
        match self {
            Self::Neutral | Self::Info => IconName::MenuBook,
            Self::Success => IconName::Check,
            Self::Warning | Self::Error => IconName::WarningAmber,
        }
    }
}

/// Hosts opt into announcements and own deduplication/throttling of changing async facts.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Announcement {
    #[default]
    Off,
    Polite,
    Assertive,
}

impl Announcement {
    pub const fn role(self) -> Option<&'static str> {
        match self {
            Self::Off => None,
            Self::Polite => Some("status"),
            Self::Assertive => Some("alert"),
        }
    }
}

/// Noninteractive compact label. Text remains visible; tone is never its sole meaning.
#[component]
pub fn Badge(
    children: Children,
    #[prop(optional)] tone: StatusTone,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <span class=move || format!("cui-badge {}", class.get()) data-tone=tone.as_str()>
            <Icon name=tone.icon()/>{children()}
        </span>
    }
}

/// A stable region can stay mounted while its children change; no app state is inferred.
#[component]
pub fn Status(
    children: Children,
    #[prop(optional)] tone: StatusTone,
    #[prop(optional)] announcement: Announcement,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-status {}", class.get()) data-tone=tone.as_str()
            role=announcement.role() aria-atomic=announcement.role().map(|_| "true")>
            <Icon name=tone.icon()/><div class="cui-status-content">{children()}</div>
        </div>
    }
}

/// The one indeterminate indicator, internal to the package so it is consumed through the registered
/// components (Button, Select, `LoadingState`) and needs no catalog entry of its own: an arc that turns while it is mounted and stays a visible
/// stationary arc under `prefers-reduced-motion`. It is decorative: the adjacent localized text or
/// reason owns the pending announcement, so it never shows a percentage, a check or a disclosure
/// glyph. Mount it only while work is ongoing; unmounting (or a hidden ancestor) ends the animation.
#[component]
pub(crate) fn LoadingIndicator() -> impl IntoView {
    view! { <span class="cui-loading-indicator" aria-hidden="true"></span> }
}

/// Indeterminate loading with visible localized text. The indicator turns under full motion and
/// stays a stationary arc under reduced motion; the text carries the status either way.
#[component]
pub fn LoadingState(
    children: Children,
    #[prop(optional)] announcement: Announcement,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-loading-state {}", class.get())
            data-state="loading" role=announcement.role()>
            <LoadingIndicator/>
            <div>{children()}</div>
        </div>
    }
}

/// Explain an empty region and compose a caller-owned recovery action through children.
#[component]
pub fn EmptyState(
    #[prop(into)] title: String,
    children: Children,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-empty-state {}", class.get())>
            <Icon name=IconName::MenuBook/>
            <div><p class="cui-state-title">{title}</p>{children()}</div>
        </div>
    }
}

#[component]
pub fn ErrorState(
    #[prop(into)] title: String,
    children: Children,
    #[prop(optional)] announcement: Announcement,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <div class=move || format!("cui-error-state {}", class.get()) role=announcement.role()>
            <Icon name=IconName::WarningAmber/>
            <div><p class="cui-state-title">{title}</p>{children()}</div>
        </div>
    }
}
