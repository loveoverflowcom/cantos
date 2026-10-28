//! Host-driven listening specimens. These states never claim an engine or start audio.
use super::recipe::Locale;
use cantos_ui::{Button, ButtonState, ButtonVariant, DisabledReason, IconName, Space, Stack};
use leptos::prelude::*;

#[component]
pub fn ListenHeroSample(
    #[prop(into)] id: String,
    initial: &'static str,
    #[prop(into)] locale: Signal<Locale>,
) -> impl IntoView {
    let phase = RwSignal::new(initial);
    let presses = RwSignal::new(0u32);
    let copy = move || super::messages::listen::copy(locale.get(), phase.get());
    let state = Signal::derive(move || match phase.get() {
        "resolving" => ButtonState::Loading(DisabledReason::new(copy().1).expect("fixture copy")),
        "unavailable" => {
            ButtonState::Disabled(DisabledReason::new(copy().1).expect("fixture copy"))
        }
        _ => ButtonState::Enabled,
    });
    let icon = Signal::derive(move || match phase.get() {
        "buffering" | "playing" => IconName::Pause,
        "failed" => IconName::Refresh,
        _ => IconName::PlayArrow,
    });
    view! {
        <Stack gap=Space::Md attr:data-preview-scenario="listen-ready listen-resolving listen-buffering listen-playing listen-paused listen-failed listen-unavailable">
            <Button id=id variant=ButtonVariant::ListenHero icon_signal=icon state=state
                on_press=Callback::new(move |_| {
                    presses.update(|count| *count += 1);
                    phase.set(super::messages::listen::after_press(phase.get_untracked()));
                })>{move || copy().0}</Button>
            <p role="status">{move || if super::messages::listen::has_reason(phase.get()) { String::new() } else { copy().1.to_owned() }}</p>
            <output data-listen-presses>{move || presses.get()}</output>
            <div class="cui-gallery-controls">
                {super::messages::listen::STATES.map(|next| view! {
                    <Button id=format!("listen-fixture-{initial}-{next}") variant=ButtonVariant::Text
                        on_press=Callback::new(move |_| phase.set(next))>{next}</Button>
                }).collect_view()}
            </div>
        </Stack>
    }
}
