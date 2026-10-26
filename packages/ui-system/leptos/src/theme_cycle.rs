//! Contextual theme cycle action: Dark → Light → System → Dark.
//!
//! A single icon button with a constant 44×44 CSS px footprint. Renders moon for Dark,
//! sun for Light, and moon+sun for System. The accessible name and tooltip state the current
//! mode and next action without pretending to be a boolean switch.
use leptos::{ev, prelude::*};

use crate::{button::component_class, ButtonVariant, Icon, IconName, Theme};

/// Accessible names and tooltips for the three theme states.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ThemeCycleLabels {
    pub dark_mode: &'static str,
    pub dark_next: &'static str,
    pub light_mode: &'static str,
    pub light_next: &'static str,
    pub system_mode: &'static str,
    pub system_next: &'static str,
}

impl ThemeCycleLabels {
    pub fn text(&self, theme: Theme) -> String {
        match theme {
            Theme::Dark => format!("{} · {}", self.dark_mode, self.dark_next),
            Theme::Light => format!("{} · {}", self.light_mode, self.light_next),
            Theme::System => format!("{} · {}", self.system_mode, self.system_next),
        }
    }
}

macro_rules! ui_primitive {
    ($($item:tt)*) => {
        #[component]
        $($item)*
    };
}

ui_primitive! {
    /// A theme cycle control that steps Dark → Light → System → Dark on each activation.
    /// Retains a constant 44×44 CSS px footprint without width changes across modes.
    pub fn ThemeCycleButton(
        #[prop(into)] theme: RwSignal<Theme>,
        #[prop(optional, into)] id: Option<String>,
        #[prop(optional, into)] class: Signal<String>,
        labels: Signal<ThemeCycleLabels>,
        #[prop(optional)] on_cycle: Option<Callback<Theme>>,
        #[prop(default = ButtonVariant::Text)] variant: ButtonVariant,
    ) -> impl IntoView {
        let button_id = id.unwrap_or_else(|| "theme-cycle-button".to_string());
        let tooltip_id = format!("{button_id}-tooltip");
        let tooltip_id_for_description = tooltip_id.clone();
        let tooltip_id_for_span = tooltip_id.clone();
        let tooltip_open = RwSignal::new(false);

        let current_text = Memo::new(move |_| labels.get().text(theme.get()));

        let button_class = Signal::derive(move || {
            let extra = class.get();
            let base = "cui-button cui-icon-button cui-theme-cycle-button";
            component_class(base, &extra)
        });

        view! {
            <div class="cui-button-field cui-theme-cycle-field">
                <button id=button_id.clone()
                    type="button"
                    class=move || button_class.get()
                    title=move || current_text.get()
                    data-variant=variant.as_str()
                    data-theme=move || theme.get().as_str()
                    aria-label=move || current_text.get()
                    aria-describedby=move || {
                        if tooltip_open.get() {
                            Some(tooltip_id_for_description.clone())
                        } else {
                            None
                        }
                    }
                    on:mouseenter=move |_| tooltip_open.set(true)
                    on:mouseleave=move |_| tooltip_open.set(false)
                    on:focus=move |_| tooltip_open.set(true)
                    on:blur=move |_| tooltip_open.set(false)
                    on:keydown=move |event: ev::KeyboardEvent| {
                        if event.key() == "Escape" && tooltip_open.get_untracked() {
                            event.stop_propagation();
                            tooltip_open.set(false);
                        }
                    }
                    on:click=move |_| {
                        let next = theme.get_untracked().cycle();
                        theme.set(next);
                        if let Some(callback) = on_cycle {
                            callback.run(next);
                        }
                    }>
                    {move || match theme.get() {
                        Theme::Dark => view! { <Icon name=IconName::DarkMode/> }.into_any(),
                        Theme::Light => view! { <Icon name=IconName::LightMode/> }.into_any(),
                        Theme::System => view! {
                            <svg class="cui-icon cui-theme-icon-system" xmlns="http://www.w3.org/2000/svg" viewBox="0 0 24 24"
                                fill="currentColor" aria-hidden="true" focusable="false">
                                <path d="M12 4a8 8 0 0 1 0 16 6 6 0 0 0 0-16zm-1-2h1v3h-1zm0 17h1v3h-1zM2 11.5h3v1H2zm4.34-6.57 1.41 1.41-1.41 1.41-1.41-1.41zm0 14.14 1.41-1.41-1.41-1.41-1.41 1.41zM12 8a4 4 0 0 0 0 8z"/>
                            </svg>
                        }.into_any(),
                    }}
                    <span class="cui-sr-only">{move || current_text.get()}</span>
                </button>
                <span id=tooltip_id_for_span
                    role="tooltip"
                    class="cui-tooltip"
                    data-state=move || if tooltip_open.get() { "visible" } else { "hidden" }
                    hidden=move || !tooltip_open.get()
                    on:mouseenter=move |_| tooltip_open.set(true)
                    on:mouseleave=move |_| tooltip_open.set(false)>
                    {move || current_text.get()}
                </span>
            </div>
        }
    }
}
