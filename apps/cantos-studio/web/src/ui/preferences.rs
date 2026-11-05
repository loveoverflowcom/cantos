//! Studio-owned composition of qualified Select controls over shell-owned preferences.
use crate::{localization::Locale, messages};
use cantos_ui::{Select, SelectDensity, Theme};
use leptos::prelude::*;

#[component]
pub fn Preferences(
    scope: &'static str,
    theme: RwSignal<Theme>,
    locale: Signal<Locale>,
    on_locale: Callback<String>,
) -> impl IntoView {
    let theme_id = format!("studio-{scope}-theme");
    let locale_id = format!("studio-{scope}-language");
    view! {
        <div class="studio-preferences">
            <Select id=theme_id density=SelectDensity::Compact
                label=Signal::derive(move || messages::copy(locale.get()).theme.to_owned())
                value=Signal::derive(move || theme.get().as_str().to_owned())
                on_input=Callback::new(move |value: String| {
                    if let Some(choice) = Theme::from_attribute(&value) { theme.set(choice); }
                })>
                {[Theme::Light, Theme::Dark, Theme::System].map(|choice| view! {
                    <option value=choice.as_str() selected=move ||theme.get()==choice>
                        {move || match choice {
                            Theme::Light => messages::copy(locale.get()).light,
                            Theme::Dark => messages::copy(locale.get()).dark,
                            Theme::System => messages::copy(locale.get()).system,
                        }}
                    </option>
                }).collect_view()}
            </Select>
            <Select id=locale_id density=SelectDensity::Compact
                label=Signal::derive(move || messages::copy(locale.get()).language.to_owned())
                value=Signal::derive(move || locale.get().lang().to_owned())
                on_input=on_locale>
                {Locale::supported().map(|option| view! {
                    <option value=option.lang() lang=option.lang() selected=move ||locale.get()==option>
                        {option.autonym()}
                    </option>
                }).collect_view()}
            </Select>
        </div>
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presentations_keep_one_selection_and_canonical_full_locale_names() {
        Owner::new().with(|| {
            let theme = RwSignal::new(Theme::System);
            let choice = RwSignal::new(Locale::Vietnamese);
            let locale = Signal::derive(move || choice.get());
            for language in Locale::supported() {
                choice.set(language);
                for mode in [Theme::Light, Theme::Dark, Theme::System] {
                    theme.set(mode);
                    for scope in ["rail", "drawer", "entry"] {
                        let html = view! { <Preferences scope theme locale on_locale=Callback::new(|_| {})/> }.to_html();
                        assert!(html.contains(&format!("id=\"studio-{scope}-theme\"")), "{html}");
                        let selected = html.split("<option").filter_map(|option| option.split('>').next())
                            .filter(|attrs| attrs.contains(" selected")).collect::<Vec<_>>();
                        assert!(selected.iter().any(|attrs| attrs.contains(&format!("value=\"{}\"", mode.as_str()))), "{html}");
                        assert!(selected.iter().any(|attrs| attrs.contains(&format!("value=\"{}\"", language.lang()))), "{html}");
                        for supported in Locale::supported() {
                            assert!(html.contains(supported.autonym()), "{html}");
                        }
                        assert_eq!(html.matches(" selected").count(), 2, "{html}");
                        assert!(!html.contains("cui-theme-cycle"), "{html}");
                    }
                }
            }
        });
    }
}
