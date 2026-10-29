use leptos::prelude::*;

use crate::{AccessibleName, Icon, IconName};

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

/// The two MicroArc frames, 16 and 20 logical units (`size.loadingArcCompact`/`size.loadingArc`).
/// Compact sits in a button, chip or select slot; Standard is the section/footer frame. The frame
/// never changes the hit target: the host keeps its own slot and control size.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum LoadingSize {
    #[default]
    Compact,
    Standard,
}

impl LoadingSize {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Compact => "compact",
            Self::Standard => "standard",
        }
    }
}

/// The one indeterminate indicator (the MicroArc), internal to the package so it is consumed through
/// the registered components (Button, Select, FilterChip, `LoadingState`) and needs no catalog entry
/// of its own: one fixed 270 degree monochrome arc with round ends that turns while it is mounted
/// and stays a visible stationary arc under `prefers-reduced-motion`. The arc is drawn by the
/// stylesheet alone (an empty element), so a host that mounts the bare `cui-loading-indicator` span,
/// as Studio's `FormSubmit` does, gets the same mark. It is decorative: the adjacent localized text
/// or reason owns the pending announcement, so it never shows a percentage, a check or a disclosure
/// glyph. Mount it only while work is ongoing; unmounting (or a hidden ancestor) ends the animation.
#[component]
pub(crate) fn LoadingIndicator(#[prop(optional)] size: LoadingSize) -> impl IntoView {
    view! { <span class="cui-loading-indicator" data-size=size.as_str() aria-hidden="true"></span> }
}

/// One reusable placeholder fill for an area whose content is still loading: a cover frame, a title
/// bar or a list row. It is a single monochrome fill that fades in opacity on the shared pulse beat,
/// so a group of fills breathes together. The caller owns geometry: size, aspect ratio and corner
/// radius come from the surrounding box (the fill takes its parent's box and radius), so the
/// placeholder keeps the loaded content's layout. The fill is decorative and hidden from assistive
/// technology: the host names the surrounding region and its busy state once, and never exposes
/// each block as content. `active` ends the animation without unmounting the fill; unmounting or
/// a hidden ancestor ends it too, and reduced motion holds a static fill.
#[component]
pub fn SurfacePulse(
    #[prop(default = true.into(), into)] active: Signal<bool>,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    view! {
        <span class=move || format!("cui-surface-pulse {}", class.get())
            data-pulse=move || if active.get() { "running" } else { "idle" } aria-hidden="true"></span>
    }
}

/// Controlled indeterminate edge for a retained-content region. The host supplies request,
/// viewport visibility and foreground lifecycle facts; success/error/cancel set `active=false`.
/// No request, clock, focus or field state is inferred here. Keep this mounted in a reserved slot
/// (or a host overlay); inactive/hidden/background states keep its height but paint nothing.
/// The label lives outside the decorative clip and must be outside the host's aria-busy region.
/// Announcements are opt-in and do not change on animation frames. Reduced motion keeps a static
/// quarter-width segment. A host that hides an ancestor must also clear `visible`.
#[component]
pub fn EdgeProgress(
    #[prop(into)] active: Signal<bool>,
    #[prop(into)] visible: Signal<bool>,
    #[prop(into)] foreground: Signal<bool>,
    #[prop(into)] label: Signal<AccessibleName>,
    #[prop(optional)] announcement: Announcement,
    #[prop(optional, into)] class: Signal<String>,
) -> impl IntoView {
    // Queued renders can outlive route-owner cleanup. The view owns these projections until
    // its readers are dropped; an arena-local derived signal would already be disposed.
    let active: ArcSignal<bool> = active.into();
    let visible: ArcSignal<bool> = visible.into();
    let foreground: ArcSignal<bool> = foreground.into();
    let label: ArcSignal<AccessibleName> = label.into();
    let class: ArcSignal<String> = class.into();
    let showing = ArcSignal::derive(move || active.get() && visible.get() && foreground.get());
    let show_status = showing.clone();
    view! {
        <div class=move || format!("cui-edge-progress {}", class.get())
            data-active=move || if showing.get() { "true" } else { "false" }>
            <span class="cui-edge-track" aria-hidden="true"><span class="cui-edge-segment"></span></span>
            <span class="cui-edge-status" role=announcement.role() aria-atomic="true">
                {move || show_status.get().then(|| label.get().as_str().to_owned())}
            </span>
        </div>
    }
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
            <LoadingIndicator size=LoadingSize::Standard/>
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
