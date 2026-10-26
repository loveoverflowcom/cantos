//! Import metadata presentation; the editor remains the sole value and request owner.
use super::{optional_metadata, ImportEditor};
use crate::{contract::ImportFormat, messages};
use cantos_ui::{Select, SelectOption, TextField, TextFieldVariant};
use leptos::prelude::*;

const FORMAT_PATH: &str = "/metadata/format";
const REFERENCE_PATH: &str = "/metadata/reference";
const RIGHTS_HOLDER_PATH: &str = "/metadata/rights_holder";
const PERMISSION_EVIDENCE_PATH: &str = "/metadata/permission_evidence";
const USAGE_SCOPE_PATH: &str = "/metadata/usage_scope";

fn format_value(format: ImportFormat) -> &'static str {
    match format {
        ImportFormat::Txt => "txt",
        ImportFormat::Markdown => "markdown",
        ImportFormat::Docx => "docx",
        ImportFormat::ScriptIr => "script_ir",
    }
}

fn selected_format(value: &str) -> ImportFormat {
    match value {
        "markdown" => ImportFormat::Markdown,
        "docx" => ImportFormat::Docx,
        "script_ir" => ImportFormat::ScriptIr,
        _ => ImportFormat::Txt,
    }
}

fn field_error(
    state: RwSignal<ImportEditor>,
    path: &'static str,
    english: Signal<bool>,
) -> Signal<Option<String>> {
    Signal::derive(move || {
        state.with(|editor| {
            let messages = editor
                .issues
                .iter()
                .filter(|issue| issue.path == path)
                .map(|issue| messages::import_field_rule(&issue.rule, english.get()))
                .collect::<Vec<_>>();
            (!messages.is_empty()).then(|| messages.join(" "))
        })
    })
}

#[component]
pub(super) fn MetadataFields(
    state: RwSignal<ImportEditor>,
    english: Signal<bool>,
    session_pending: Signal<bool>,
) -> impl IntoView {
    let copy = move || messages::import_copy(english.get());
    let blocked = move || session_pending.get() || state.with(ImportEditor::blocked);
    let reason = crate::ui::LocalizedReason::new(crate::ui::BlockReason::Pending);
    let Ok(reason) = reason else {
        return ().into_any();
    };
    let reason = StoredValue::new(reason);
    let field_state = Signal::derive(move || {
        if blocked() {
            cantos_ui::FieldState::Disabled(reason.with_value(|reason| reason.get(english.get())))
        } else {
            cantos_ui::FieldState::Editable
        }
    });
    let choice_state = Signal::derive(move || {
        if blocked() {
            cantos_ui::ChoiceState::Disabled(reason.with_value(|reason| reason.get(english.get())))
        } else {
            cantos_ui::ChoiceState::Enabled
        }
    });
    view! {
        <div class="import-fields">
        <Select id="import-format" error=field_error(state,FORMAT_PATH,english) label=Signal::derive(move ||copy().format.to_owned())
            value=Signal::derive(move ||state.with(|editor|editor.visible_metadata().map(|metadata|format_value(metadata.format)).unwrap_or("txt").to_owned()))
            state=choice_state described_by=Some("import-field-findings".to_owned())
            options=Signal::derive(move ||vec![SelectOption::new("txt",copy().txt),SelectOption::new("markdown",copy().markdown),SelectOption::new("docx",copy().docx),SelectOption::new("script_ir",copy().script_ir)])
            on_input=Callback::new(move |value:String|state.update(|editor|*editor=editor.edit_metadata(|metadata|metadata.format=selected_format(&value))))/>
        <TextField id="import-reference" error=field_error(state,REFERENCE_PATH,english) label=Signal::derive(move ||copy().reference.to_owned()) variant=TextFieldVariant::Floating
            state=field_state required=true maxlength=2048 autocomplete="off" described_by=Some("import-field-findings".to_owned())
            value=Signal::derive(move ||state.with(|editor|editor.visible_metadata().map(|metadata|metadata.reference.clone()).unwrap_or_default()))
            on_input=Callback::new(move |value|state.update(|editor|*editor=editor.edit_metadata(|metadata|metadata.reference=value)))/>
        <div class="import-metadata">
            <TextField id="import-rights-holder" error=field_error(state,RIGHTS_HOLDER_PATH,english) label=Signal::derive(move ||copy().rights_holder.to_owned()) variant=TextFieldVariant::Floating
                state=field_state maxlength=2048 autocomplete="off" described_by=Some("import-rights-help import-field-findings".to_owned())
                value=Signal::derive(move ||state.with(|editor|editor.visible_metadata().and_then(|metadata|metadata.rights_holder.clone()).unwrap_or_default()))
                on_input=Callback::new(move |value|state.update(|editor|*editor=editor.edit_metadata(|metadata|metadata.rights_holder=optional_metadata(value))))/>
            <TextField id="import-permission" error=field_error(state,PERMISSION_EVIDENCE_PATH,english) label=Signal::derive(move ||copy().permission_evidence.to_owned()) variant=TextFieldVariant::Floating
                state=field_state maxlength=2048 autocomplete="off" described_by=Some("import-rights-help import-field-findings".to_owned())
                value=Signal::derive(move ||state.with(|editor|editor.visible_metadata().and_then(|metadata|metadata.permission_evidence.clone()).unwrap_or_default()))
                on_input=Callback::new(move |value|state.update(|editor|*editor=editor.edit_metadata(|metadata|metadata.permission_evidence=optional_metadata(value))))/>
            <TextField id="import-usage" error=field_error(state,USAGE_SCOPE_PATH,english) label=Signal::derive(move ||copy().usage_scope.to_owned()) variant=TextFieldVariant::Floating
                state=field_state maxlength=2048 autocomplete="off" described_by=Some("import-rights-help import-field-findings".to_owned())
                value=Signal::derive(move ||state.with(|editor|editor.visible_metadata().and_then(|metadata|metadata.usage_scope.clone()).unwrap_or_default()))
                on_input=Callback::new(move |value|state.update(|editor|*editor=editor.edit_metadata(|metadata|metadata.usage_scope=optional_metadata(value))))/>
        </div>
        <p id="import-rights-help" class="help">{move ||copy().rights_help}</p>
        </div>
    }.into_any()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metadata_uses_shared_controls_without_losing_native_constraints_or_values() {
        Owner::new().with(|| {
            let mut editor = ImportEditor::default().signed_in_as("owner".into());
            editor = editor.edit_metadata(|m| {
                m.format = ImportFormat::Docx;
                m.reference = "Ánh đèn cuối sân khấu".into();
                m.rights_holder = Some("Người dẫn chuyện".into());
                m.permission_evidence = Some("Cho phép diễn tiếp".into());
                m.usage_scope = Some("Bản thử nội bộ".into());
            });
            let state = RwSignal::new(editor.clone());
            let english = RwSignal::new(false);
            let pending = RwSignal::new(false);
            for en in [false, true] {
                english.set(en);
                let html = view! { <MetadataFields state english=english.into() session_pending=pending.into()/> }.to_html();
                assert!(html.contains("cui-select-trigger"), "{html}");
                assert_eq!(html.matches(r#"data-variant="floating""#).count(), 4, "{html}");
                assert_eq!(html.matches(r#"maxlength="2048""#).count(), 4, "{html}");
                for value in ["Ánh đèn cuối sân khấu", "Người dẫn chuyện", "Cho phép diễn tiếp", "Bản thử nội bộ"] {
                    assert!(html.contains(value), "{html}");
                }
                assert!(html.contains("import-rights-help import-field-findings"), "{html}");
                assert!(html.contains(r#"aria-invalid="false""#), "{html}");
                assert_eq!(state.get_untracked().metadata, editor.metadata);
            }
        });
    }

    #[test]
    fn metadata_projects_server_findings_and_session_block_without_editing_the_draft() {
        Owner::new().with(|| {
            let mut editor = ImportEditor::default().signed_in_as("owner".into());
            editor = editor.edit_metadata(|m|m.reference="Bản gốc".into());
            editor.issues.push(crate::contract::FieldIssue {
                path:"/metadata/reference".into(),rule:"byte_length".into(),
            });
            let state=RwSignal::new(editor.clone());
            let pending=RwSignal::new(true);
            let english=RwSignal::new(false);
            for en in [false,true] {
                english.set(en);
                let html=view!{<MetadataFields state english=english.into() session_pending=pending.into()/>}.to_html();
                let start=html.find("<input id=\"import-reference\"").unwrap();
                let control=&html[start..start+html[start..].find('>').unwrap()];
                assert!(control.contains(" disabled"),"{control}");
                assert!(control.contains(r#"aria-invalid="true""#),"{control}");
                assert!(control.contains(r#"aria-errormessage="import-reference-error""#),"{control}");
                assert!(control.contains("import-field-findings import-reference-error import-reference-reason"),"{control}");
                assert!(html.contains(messages::import_field_rule("byte_length",en)),"{html}");
                assert_eq!(state.get_untracked().metadata,editor.metadata);
                assert_eq!(state.get_untracked().issues,editor.issues);
            }
            pending.set(false);
            let html=view!{<MetadataFields state english=english.into() session_pending=pending.into()/>}.to_html();
            assert!(!html.contains(" disabled"),"{html}");
            assert!(state.get_untracked().has_unsaved());
        });
    }
}
