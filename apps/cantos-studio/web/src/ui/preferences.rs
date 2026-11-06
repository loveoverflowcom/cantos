//! Studio-owned composition of qualified Select controls over shell-owned preferences.
use crate::{localization::Locale, messages};
use cantos_ui::{Select, SelectDensity, SelectOption, Theme};
use leptos::prelude::*;

#[component]
pub fn Preferences(
    scope: &'static str,
    theme: RwSignal<Theme>,
    locale: Signal<Locale>,
    on_locale: Callback<String>,
    /// Entry values can wrap; modal footer controls retain their native presentation.
    #[prop(optional)]
    wrap_values: bool,
) -> impl IntoView {
    let theme_id = format!("studio-{scope}-theme");
    let locale_id = format!("studio-{scope}-language");
    let theme_options = Signal::derive(move || {
        [Theme::Light, Theme::Dark, Theme::System]
            .map(|choice| {
                SelectOption::new(
                    choice.as_str(),
                    match choice {
                        Theme::Light => messages::copy(locale.get()).light,
                        Theme::Dark => messages::copy(locale.get()).dark,
                        Theme::System => messages::copy(locale.get()).system,
                    },
                )
            })
            .to_vec()
    });
    let locale_options = Signal::stored(
        Locale::supported()
            .map(|option| SelectOption {
                lang: Some(option.lang().to_owned()),
                ..SelectOption::new(option.lang(), option.autonym())
            })
            .collect::<Vec<_>>(),
    );
    view! {
        <div class="studio-preferences">
            <Select id=theme_id density=SelectDensity::Compact
                label=Signal::derive(move || messages::copy(locale.get()).theme.to_owned())
                value=Signal::derive(move || theme.get().as_str().to_owned())
                nostrip:options=wrap_values.then_some(theme_options)
                on_input=Callback::new(move |value: String| {
                    if let Some(choice) = Theme::from_attribute(&value) { theme.set(choice); }
                })>
                {[Theme::Light, Theme::Dark, Theme::System].map(|choice| view! {
                    <option value=choice.as_str() selected=move || theme.get()==choice>
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
                nostrip:options=wrap_values.then_some(locale_options)
                on_input=on_locale>
                {Locale::supported().map(|option| view! {
                    <option value=option.lang() lang=option.lang() selected=move || locale.get()==option>
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
                        let wrap_values = scope == "entry";
                        let html = view! { <Preferences scope theme locale wrap_values on_locale=Callback::new(|_| {})/> }.to_html();
                        assert!(html.contains(&format!("id=\"studio-{scope}-theme\"")), "{html}");
                        let selected = html.split("<option").filter_map(|option| option.split('>').next())
                            .filter(|attrs| attrs.contains(" selected")).collect::<Vec<_>>();
                        assert!(selected.iter().any(|attrs| attrs.contains(&format!("value=\"{}\"", mode.as_str()))), "{html}");
                        assert!(selected.iter().any(|attrs| attrs.contains(&format!("value=\"{}\"", language.lang()))), "{html}");
                        for supported in Locale::supported() {
                            assert!(html.contains(supported.autonym()), "{html}");
                        }
                        assert_eq!(html.matches(" selected").count(), 2, "{html}");
                        // Entry enhancement preserves the native fallback; rail/drawer keep
                        // their intentional native recipe and original controlled choices.
                        assert_eq!(html.matches("role=\"combobox\"").count(), if wrap_values { 2 } else { 0 }, "{html}");
                        if wrap_values {
                            assert!(html.contains(&format!("aria-labelledby=\"studio-{scope}-language-label studio-{scope}-language-value\"")), "{html}");
                        }
                        assert!(!html.contains("cui-select-short"), "{html}");
                        assert!(!html.contains("cui-theme-cycle"), "{html}");
                    }
                }
            }
        });
    }
}
