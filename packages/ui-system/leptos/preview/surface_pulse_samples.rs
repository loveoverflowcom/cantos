//! SurfacePulse specimens: a cover with two lines, a title bar and a list row. Each keeps the
//! geometry of its loaded content through the reveal. No service effect; the stage is host-driven.
use super::messages::pulse;
use cantos_ui::{Icon, IconName, SurfacePulse};
use leptos::prelude::*;

/// Which side of the reveal a specimen shows. The host drives it; no timer lives here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PulseStage {
    Running,
    Idle,
    Ready,
}

impl PulseStage {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Running => "running",
            Self::Idle => "idle",
            Self::Ready => "ready",
        }
    }
}

/// One placeholder box per text line; the caller's line box (its font size and line height) is the
/// geometry. The last line is shorter, as a paragraph's last line is.
#[component]
fn PulseLines(count: usize, running: bool, last: &'static str) -> impl IntoView {
    (0..count)
        .map(|index| {
            let width = if index + 1 == count { last } else { "100%" };
            view! { <div class="cui-pulse-line" style:inline-size=width><SurfacePulse active=running/></div> }
        })
        .collect_view()
}

/// A cover frame and text lines; the loaded content sits in the same grid cell as its placeholder, so
/// the row is as tall as the taller of the two and the reveal moves nothing.
#[component]
fn PulseRow(
    stage: PulseStage,
    title: &'static str,
    meta: &'static str,
    placeholder_title_lines: usize,
    #[prop(optional)] compact: bool,
    #[prop(default = "vi-VN")] lang: &'static str,
) -> impl IntoView {
    let running = stage == PulseStage::Running;
    view! {
        <div class="cui-pulse-row" data-stage=stage.as_str() data-compact=compact.then_some("true") lang=lang>
            <div class="cui-pulse-slot cui-pulse-cover">
                <div class="cui-pulse-layer cui-pulse-placeholder" aria-hidden="true"><SurfacePulse active=running/></div>
                <div class="cui-pulse-layer cui-pulse-loaded" aria-hidden=(stage != PulseStage::Ready).then_some("true")>
                    <Icon name=IconName::MenuBook/>
                </div>
            </div>
            <div class="cui-pulse-slot">
                <div class="cui-pulse-layer cui-pulse-placeholder cui-pulse-lines" aria-hidden="true">
                    <div class="cui-pulse-title"><PulseLines count=placeholder_title_lines running=running last="80%"/></div>
                    <div class="cui-pulse-meta"><PulseLines count=1 running=running last="50%"/></div>
                </div>
                <div class="cui-pulse-layer cui-pulse-loaded" aria-hidden=(stage != PulseStage::Ready).then_some("true")>
                    <p class="cui-pulse-title">{title}</p>
                    <p class="cui-pulse-meta">{meta}</p>
                </div>
            </div>
        </div>
    }
}

/// The host names the whole region once and marks it busy; the placeholders are never read as content.
#[component]
pub fn SurfacePulseSamples(stage: PulseStage) -> impl IntoView {
    let busy = (stage != PulseStage::Ready).then_some("true");
    view! {
        <div class="cui-pulse-specimen" data-preview-scenario="surface-pulse" role="group"
            aria-label=if stage == PulseStage::Ready { pulse::REGION_READY } else { pulse::REGION_LOADING }
            aria-busy=busy>
            <PulseRow stage=stage title=pulse::COVER_TITLE meta=pulse::COVER_META placeholder_title_lines=1/>
            <PulseRow stage=stage compact=true placeholder_title_lines=2
                title=pulse::ROW_TITLE
                meta=pulse::ROW_META/>
            <div class="cui-pulse-bar" data-stage=stage.as_str()>
                <div class="cui-pulse-slot">
                    <div class="cui-pulse-layer cui-pulse-placeholder" aria-hidden="true">
                        <SurfacePulse active=stage == PulseStage::Running/>
                    </div>
                    <div class="cui-pulse-layer cui-pulse-loaded" aria-hidden=(stage != PulseStage::Ready).then_some("true")>
                        <p class="cui-pulse-title">{pulse::BAR_TITLE}</p>
                    </div>
                </div>
            </div>
            <PulseRow stage=stage lang="en" title=pulse::EN_TITLE
                meta=pulse::EN_META placeholder_title_lines=2/>
        </div>
    }
}
