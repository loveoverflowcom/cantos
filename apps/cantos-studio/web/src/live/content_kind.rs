//! Content format presentation. The workspace owns the revision lock and exact draft value.
use super::copy::{text, Key};
use crate::ui::{BlockReason, LocalizedReason};
use cantos_ui::{ChoiceState, Select, SelectOption};
use leptos::prelude::*;

#[component]
pub(super) fn ContentKindSelect(
    locale: Signal<crate::localization::Locale>,
    value: Signal<String>,
    locked: Signal<bool>,
    editor_script_available: Signal<bool>,
    on_change: Callback<String>,
) -> impl IntoView {
    let Ok(reason) = LocalizedReason::new(BlockReason::ContextReady) else {
        return ().into_any();
    };
    let reason = StoredValue::new(reason);
    view! {
        <Select id="live-content-kind" label=Signal::derive(move ||text(Key::Body,locale.get()).to_owned())
            value on_input=on_change class="live-field"
            state=Signal::derive(move ||if locked.get() {
                ChoiceState::Disabled(reason.with_value(|reason|reason.get(locale.get())))
            }else {ChoiceState::Enabled})
            options=Signal::derive(move ||vec![
                SelectOption::new("text",text(Key::Text,locale.get())),
                SelectOption::new("recording",text(Key::Recording,locale.get())),
                SelectOption {disabled:true,..SelectOption::new("source",text(Key::Source,locale.get()))},
                SelectOption {disabled:true,..SelectOption::new("script",text(Key::Script,locale.get()))},
                SelectOption {disabled:!editor_script_available.get(),..SelectOption::new("editor-script",text(Key::EditorScript,locale.get()))},
            ])/>
    }.into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn control<'a>(html: &'a str, id: &str) -> &'a str {
        let start = html.find(&format!("id=\"{id}\"")).unwrap();
        &html[start..start + html[start..].find('>').unwrap()]
    }

    #[test]
    fn missing_revision_disables_both_select_presentations_and_preserves_retained_source() {
        Owner::new().with(|| {
            let locked = RwSignal::new(true);
            let available = RwSignal::new(false);
            let value = RwSignal::new("source".to_owned());
            for (lock, script) in [(true,false),(false,false),(false,true),(true,true)] {
                locked.set(lock);
                available.set(script);
                let html=view! {
                    <ContentKindSelect locale=Signal::derive(||crate::localization::Locale::Vietnamese) value=value.into()
                        locked=locked.into() editor_script_available=available.into()
                        on_change=Callback::new(|_:String|panic!("render must not edit the retained source"))/>
                }.to_html();
                for id in ["live-content-kind","live-content-kind-trigger"] {
                    assert_eq!(control(&html,id).contains(" disabled"),lock,"{html}");
                }
                assert!(html.contains(&format!("data-state=\"{}\"",if lock {"disabled"}else {"enabled"})),"{html}");
                let selected_source=html.split("<option value=\"source\"").nth(1).unwrap().split('>').next().unwrap();
                assert!(selected_source.contains("disabled") && selected_source.contains("selected"),"{html}");
                let editor_option=html.split("<option value=\"editor-script\"").nth(1).unwrap().split('>').next().unwrap();
                assert_eq!(editor_option.contains("disabled"),!script,"{html}");
                assert_eq!(value.get_untracked(),"source");
                assert!(html.contains(text(Key::Source,crate::localization::Locale::Vietnamese)));
            }
        });
    }
}
