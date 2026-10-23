//! App-local transport bindings for the saved current Content and exact immutable Brief.
//! These checks prevent accidental form retargeting; the Server still owns admission.
use super::{ClientError, Facts, UploadCredential};
use crate::contract::*;

pub fn brief_content(facts: &Facts, content_dirty: bool) -> Option<&ContentRevisionResponse> {
    if content_dirty {
        return None;
    }
    let work = facts.work.as_ref()?;
    let unit = facts.unit.as_ref()?;
    if unit.work_id != work.id {
        return None;
    }
    facts
        .content_head
        .as_ref()
        .filter(|content| content.work_id == work.id && content.unit_id == unit.id)
}

pub fn production_mode(body: &ContentBody) -> (ProductionMode, ProducerCapability) {
    match body {
        ContentBody::Text { .. } | ContentBody::Source { .. } => {
            (ProductionMode::Narration, ProducerCapability::Narration)
        }
        ContentBody::Recording {} => (ProductionMode::Recording, ProducerCapability::Recording),
        ContentBody::Script { .. } => (ProductionMode::AudioDrama, ProducerCapability::ScriptIr),
    }
}

pub fn submission_binding(
    brief: &ProductionBriefResponse,
    work: &str,
    unit: &str,
) -> Result<AudioBinding, ClientError> {
    let document = &brief.brief;
    if brief.freshness != BriefFreshness::Current
        || document.work.id != work
        || document.content.work_id != work
        || document.content.unit_id != unit
    {
        return Err(ClientError::InvalidInput);
    }
    Ok(AudioBinding {
        work_id: work.into(),
        unit_id: unit.into(),
        content_revision_id: document.content.id.clone(),
        content_revision_digest: document.content.revision_digest.clone(),
        brief_id: document.id.clone(),
        brief_version: document.version,
        brief_digest: document.digest.clone(),
        profile_id: document.guidance.delivery.profile_id.clone(),
    })
}

/// A retained bearer can be used only for the submission whose prepare receipt established it.
#[derive(Clone)]
pub struct LocalUploadPermission {
    submission: String,
    credential: UploadCredential,
}
impl LocalUploadPermission {
    pub fn new(submission: String, credential: UploadCredential) -> Self {
        Self {
            submission,
            credential,
        }
    }
    pub fn for_submission(&self, submission: &str) -> Option<&UploadCredential> {
        (self.submission == submission).then_some(&self.credential)
    }
}

#[cfg(target_arch = "wasm32")]
pub async fn copy_brief(value: &str) -> Result<(), ClientError> {
    let window = web_sys::window().ok_or(ClientError::Transport)?;
    use wasm_bindgen::{JsCast, JsValue};
    let clipboard = js_sys::Reflect::get(&window.navigator(), &JsValue::from_str("clipboard"))
        .map_err(|_| ClientError::Transport)?;
    if clipboard.is_undefined() || clipboard.is_null() {
        return Err(ClientError::Transport);
    }
    let clipboard: web_sys::Clipboard = clipboard.unchecked_into();
    wasm_bindgen_futures::JsFuture::from(clipboard.write_text(value))
        .await
        .map_err(|_| ClientError::Transport)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    fn brief() -> ProductionBriefResponse {
        ProductionBriefResponse {
            brief: serde_json::from_str(include_str!(
                "../../../../../services/cantos-server/contracts/studio/briefs/v1/fixtures/narration-document.json"
            )).unwrap(),
            freshness: BriefFreshness::Current,
        }
    }
    fn facts() -> Facts {
        let brief = brief();
        let work: CatalogWork = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/work.json"
        ))
        .unwrap();
        let unit: CatalogContentUnit = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/catalog/v1/fixtures/unit.json"
        ))
        .unwrap();
        Facts {
            work: Some(CatalogWork {
                id: brief.brief.work.id.clone(),
                ..work
            }),
            unit: Some(CatalogContentUnit {
                id: brief.brief.content.unit_id.clone(),
                ..unit
            }),
            content_head: Some(brief.brief.content),
            ..Facts::default()
        }
    }
    #[test]
    fn historical_inspection_never_becomes_the_brief_source() {
        let mut facts = facts();
        let current = facts.content_head.clone().unwrap();
        let mut historical = current.clone();
        historical.id = "older-content".into();
        historical.revision = 0;
        facts.content = Some(historical);
        assert_eq!(brief_content(&facts, false), Some(&current));
        assert_eq!(brief_content(&facts, true), None);
        facts.unit.as_mut().unwrap().id = "another-unit".into();
        assert_eq!(brief_content(&facts, false), None);
    }
    #[test]
    fn all_content_bodies_keep_their_explicit_production_capability() {
        let source = CatalogSourceRef {
            id: "source".into(),
            sha256: "a".repeat(64),
        };
        let script = CatalogScriptRef {
            id: "script".into(),
            revision: 7,
            export_digest: "exact".into(),
        };
        for (body, expected) in [
            (
                ContentBody::Text {
                    text: "text".into(),
                    source: None,
                },
                (ProductionMode::Narration, ProducerCapability::Narration),
            ),
            (
                ContentBody::Source {
                    source: source.clone(),
                },
                (ProductionMode::Narration, ProducerCapability::Narration),
            ),
            (
                ContentBody::Recording {},
                (ProductionMode::Recording, ProducerCapability::Recording),
            ),
            (
                ContentBody::Script {
                    script,
                    source: Some(source),
                },
                (ProductionMode::AudioDrama, ProducerCapability::ScriptIr),
            ),
        ] {
            assert_eq!(production_mode(&body), expected);
        }
    }
    #[test]
    fn prepare_pins_the_read_brief_and_refuses_other_units_or_stale_snapshots() {
        let mut response = brief();
        let work = response.brief.work.id.clone();
        let unit = response.brief.content.unit_id.clone();
        let binding = submission_binding(&response, &work, &unit).unwrap();
        assert_eq!(binding.content_revision_id, response.brief.content.id);
        assert_eq!(binding.brief_digest, response.brief.digest);
        assert_eq!(binding.profile_id, "opus-mono-24-v1");
        assert_eq!(
            submission_binding(&response, "other", &unit),
            Err(ClientError::InvalidInput)
        );
        assert_eq!(
            submission_binding(&response, &work, "other"),
            Err(ClientError::InvalidInput)
        );
        response.freshness = BriefFreshness::ContentChanged;
        assert_eq!(
            submission_binding(&response, &work, &unit),
            Err(ClientError::InvalidInput)
        );
    }
    #[test]
    fn retained_upload_permission_cannot_move_to_another_submission() {
        let token = UploadCredential::new("a".repeat(64)).unwrap();
        let permission = LocalUploadPermission::new("prepared".into(), token.clone());
        assert_eq!(permission.for_submission("prepared"), Some(&token));
        assert_eq!(permission.for_submission("another"), None);
    }
}

#[cfg(target_arch = "wasm32")]
mod view {
    use super::*;
    use crate::live::copy::{self, text, Key};
    use crate::ui::{ActionButton, BlockReason};
    use leptos::{prelude::*, task::spawn_local};
    use wasm_bindgen::JsCast;

    #[component]
    pub fn BriefDocument(
        brief: Signal<Option<ProductionBriefResponse>>,
        english: Signal<bool>,
        blocked: Signal<bool>,
    ) -> impl IntoView {
        let copied = RwSignal::new(None::<bool>);
        let copying = RwSignal::new(false);
        Effect::new(move |_| {
            let _ = brief.with(|b| b.as_ref().map(|b| b.brief.digest.clone()));
            copied.set(None);
        });
        view! {
            <p role="status">{move ||brief.with(|b|b.as_ref().map(|b|copy::brief_freshness(b.freshness.clone(),english.get())).unwrap_or_default())}</p>
            <p><code>{move ||brief.with(|b|b.as_ref().map(|b|copy::brief_binding(&b.brief)).unwrap_or_default())}</code></p>
            <p class="help">{move ||text(Key::CopyBriefHelp,english.get())}</p>
            <ActionButton id="live-copy-brief" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||blocked.get()||copying.get()||brief.with(|b|b.is_none())) on_press=Callback::new(move |_| {
                if blocked.get_untracked() || copying.get_untracked() { return; }
                let Some(snapshot)=brief.get_untracked() else {return;};
                let Ok(value)=serde_json::to_string_pretty(&snapshot.brief) else {copied.set(Some(false));return;};
                copying.set(true);
                spawn_local(async move {
                    let result=copy_brief(&value).await.is_ok();
                    if brief.try_with_untracked(|b|b.as_ref().is_some_and(|b|b.brief.id==snapshot.brief.id&&b.brief.digest==snapshot.brief.digest)).unwrap_or(false) {
                        let _=copied.try_set(Some(result));
                        if !result {
                            leptos::leptos_dom::helpers::request_animation_frame(move || {
                                if !brief.try_with_untracked(|b|b.as_ref().is_some_and(|b|b.brief.id==snapshot.brief.id&&b.brief.digest==snapshot.brief.digest)).unwrap_or(false) { return; }
                                if let Some(field)=web_sys::window().and_then(|w|w.document()).and_then(|d|d.get_element_by_id("live-brief-document")).and_then(|e|e.dyn_into::<web_sys::HtmlTextAreaElement>().ok()) {
                                    let _=field.focus(); field.select();
                                }
                            });
                        }
                    }
                    let _=copying.try_set(false);
                });
            })>{move ||text(Key::CopyBrief,english.get())}</ActionButton>
            <p role="status">{move ||copied.get().map(|success|text(if success{Key::CopiedBrief}else{Key::CopyBriefFailed},english.get())).unwrap_or_default()}</p>
            <details open=move ||copied.get()==Some(false)><summary>{move ||text(Key::InspectDocument,english.get())}</summary><textarea id="live-brief-document" class="cui-field-control" readonly aria-label=move ||text(Key::Brief,english.get()) prop:value=move ||brief.with(|b|b.as_ref().and_then(|b|serde_json::to_string_pretty(&b.brief).ok()).unwrap_or_default())/></details>
        }
    }
}
#[cfg(target_arch = "wasm32")]
pub use view::BriefDocument;
