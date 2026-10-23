//! Browser-memory authoring snapshots. Navigating between units never moves a draft to a
//! different target; acknowledgements make only the submitted bytes clean.
use super::{Command, Reply, DEFAULT_WORK_KIND};
use crate::contract::*;
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WorkForm {
    pub title: String,
    pub synopsis: String,
    pub kind: String,
    pub rights: Vec<String>,
}
impl Default for WorkForm {
    fn default() -> Self {
        Self {
            title: String::new(),
            synopsis: String::new(),
            kind: DEFAULT_WORK_KIND.into(),
            rights: vec![],
        }
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UnitForm {
    pub title: String,
    pub description: String,
}
impl From<&UnitDetailsDto> for UnitForm {
    fn from(value: &UnitDetailsDto) -> Self {
        Self {
            title: value.title.clone(),
            description: value.description.clone(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ContentForm {
    pub kind: String,
    pub text: String,
    pub provenance: String,
    pub rights: Vec<String>,
    pub language: String,
    pub source: Option<CatalogSourceRef>,
    pub script: Option<CatalogScriptRef>,
}
impl Default for ContentForm {
    fn default() -> Self {
        Self {
            kind: "text".into(),
            text: String::new(),
            provenance: String::new(),
            rights: vec![],
            language: "vi-VN".into(),
            source: None,
            script: None,
        }
    }
}
impl From<&ContentDetails> for ContentForm {
    fn from(value: &ContentDetails) -> Self {
        let (kind, text, source) = match &value.body {
            ContentBody::Text { text, source } => ("text", text.clone(), source.clone()),
            ContentBody::Recording {} => ("recording", String::new(), None),
            ContentBody::Source { source } => ("source", String::new(), Some(source.clone())),
            ContentBody::Script { source, .. } => ("script", String::new(), source.clone()),
        };
        Self {
            kind: kind.into(),
            text,
            provenance: value.provenance_note.clone(),
            rights: value.rights_refs.clone(),
            language: value.language.clone(),
            source,
            script: match &value.body {
                ContentBody::Script { script, .. } => Some(script.clone()),
                _ => None,
            },
        }
    }
}
impl ContentForm {
    /// The text/recording form does not silently convert linked Source or Script content.
    pub fn details(&self) -> Option<ContentDetails> {
        let body = match self.kind.as_str() {
            "text" => ContentBody::Text {
                text: self.text.clone(),
                source: self.source.clone(),
            },
            "recording" => ContentBody::Recording {},
            "editor-script" => ContentBody::Script {
                script: self.script.clone()?,
                source: self.source.clone(),
            },
            _ => return None,
        };
        Some(ContentDetails {
            language: self.language.clone(),
            body,
            rights_refs: self.rights.clone(),
            provenance_note: self.provenance.clone(),
        })
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct Draft<T> {
    current: T,
    baseline: T,
}
impl<T: Clone + PartialEq> Draft<T> {
    fn dirty(&self) -> bool {
        self.current != self.baseline
    }
    fn stored(value: T) -> Self {
        Self {
            current: value.clone(),
            baseline: value,
        }
    }
    fn acknowledge(&mut self, submitted: T) {
        self.baseline = submitted;
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ContentDraft {
    form: Draft<ContentForm>,
    base: Option<u64>,
}
impl ContentDraft {
    fn head(&mut self, value: ContentForm, revision: u64, replace: Option<&ContentForm>) {
        if !self.form.dirty() || replace == Some(&self.form.current) {
            self.form = Draft::stored(value);
            self.base = Some(revision);
        } else if self.base.is_none() {
            // Typing during the first read stays local; the read establishes its initial base.
            self.form.baseline = value;
            self.base = Some(revision);
        }
        // A refresh while dirty does not silently rebase an already-open draft.
    }
}
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct AuthoringDrafts {
    new_work: Draft<WorkForm>,
    new_units: BTreeMap<String, Draft<UnitForm>>,
    metadata: BTreeMap<String, Draft<UnitForm>>,
    metadata_bases: BTreeMap<String, u64>,
    content: BTreeMap<String, ContentDraft>,
    content_submissions: BTreeMap<String, (String, ContentForm)>,
}
impl AuthoringDrafts {
    pub fn has_unsaved(&self) -> bool {
        self.new_work.dirty()
            || self.new_units.values().any(Draft::dirty)
            || self.metadata.values().any(Draft::dirty)
            || self.content.values().any(|d| d.form.dirty())
    }
    pub fn edit_work(&mut self, value: WorkForm) {
        self.new_work.current = value;
    }
    pub fn edit_new_unit(&mut self, work: &str, value: UnitForm) {
        if !work.is_empty() {
            self.new_units.entry(work.into()).or_default().current = value;
        }
    }
    pub fn new_unit(&self, work: &str) -> UnitForm {
        self.new_units
            .get(work)
            .map(|d| d.current.clone())
            .unwrap_or_default()
    }
    pub fn open_unit(&mut self, unit: &CatalogContentUnit) -> (UnitForm, ContentForm) {
        let metadata = self
            .metadata
            .entry(unit.id.clone())
            .or_insert_with(|| Draft::stored(UnitForm::from(&unit.details)));
        if !metadata.dirty() {
            *metadata = Draft::stored(UnitForm::from(&unit.details));
            self.metadata_bases.insert(unit.id.clone(), unit.revision);
        }
        (
            metadata.current.clone(),
            self.content
                .entry(unit.id.clone())
                .or_default()
                .form
                .current
                .clone(),
        )
    }
    pub fn metadata_base(&self, unit: &str) -> Option<u64> {
        self.metadata_bases.get(unit).copied()
    }
    pub fn metadata_dirty(&self, unit: &str) -> bool {
        self.metadata.get(unit).is_some_and(Draft::dirty)
    }
    pub fn reload_metadata(&mut self, unit: &CatalogContentUnit) -> UnitForm {
        let value = UnitForm::from(&unit.details);
        self.metadata
            .insert(unit.id.clone(), Draft::stored(value.clone()));
        self.metadata_bases.insert(unit.id.clone(), unit.revision);
        value
    }
    pub fn metadata(&self, unit: &str) -> UnitForm {
        self.metadata
            .get(unit)
            .map(|d| d.current.clone())
            .unwrap_or_default()
    }
    pub fn edit_metadata(&mut self, unit: &str, value: UnitForm) {
        if !unit.is_empty() {
            self.metadata.entry(unit.into()).or_default().current = value;
        }
    }
    pub fn edit_content(&mut self, unit: &str, value: ContentForm) {
        if !unit.is_empty() {
            self.content.entry(unit.into()).or_default().form.current = value;
        }
    }
    pub fn content(&self, unit: &str) -> ContentForm {
        self.content
            .get(unit)
            .map(|d| d.form.current.clone())
            .unwrap_or_default()
    }
    pub fn content_base(&self, unit: &str) -> Option<u64> {
        self.content.get(unit).and_then(|d| d.base)
    }
    pub fn content_dirty(&self, unit: &str) -> bool {
        self.content.get(unit).is_some_and(|d| d.form.dirty())
    }
    pub fn head(&mut self, value: &ContentRevisionResponse, replace: Option<&ContentForm>) {
        self.content.entry(value.unit_id.clone()).or_default().head(
            ContentForm::from(&value.details),
            value.revision,
            replace,
        );
    }
    pub fn empty_head(&mut self, unit: &str, replace: Option<&ContentForm>) {
        self.content
            .entry(unit.into())
            .or_default()
            .head(ContentForm::default(), 0, replace);
    }
    pub fn submitted_content(&mut self, unit: &str, request: &AppendContentRequest) {
        let current = self.content(unit);
        if current.details().as_ref() == Some(&request.details) {
            self.content_submissions
                .insert(request.operation_id.clone(), (unit.into(), current));
        }
    }
    pub fn acknowledge(&mut self, command: &Command, reply: &Reply) {
        match (command, reply) {
            (Command::CreateWork(request), Reply::Mutation(_)) => {
                self.new_work.acknowledge(WorkForm {
                    title: request.details.title.clone(),
                    synopsis: request.details.synopsis.clone(),
                    kind: request.kind.clone(),
                    rights: request.details.rights_refs.clone(),
                })
            }
            (Command::CreateUnit { work, request }, Reply::Mutation(_)) => {
                self.new_units
                    .entry(work.clone())
                    .or_default()
                    .acknowledge(UnitForm::from(&request.details));
                self.content.entry(request.id.clone()).or_default().base = Some(0);
            }
            (Command::PatchUnit { unit, request }, Reply::Mutation(reply)) => {
                if let Some(draft) = self.metadata.get_mut(unit) {
                    let mut submitted = draft.baseline.clone();
                    if let PatchField::Value(title) = &request.details.title {
                        submitted.title = title.clone();
                    }
                    if let PatchField::Value(description) = &request.details.description {
                        submitted.description = description.clone();
                    }
                    draft.acknowledge(submitted);
                    if let Some(accepted) = reply.units.iter().find(|accepted| &accepted.id == unit)
                    {
                        self.metadata_bases.insert(unit.clone(), accepted.revision);
                    }
                }
            }
            (Command::AppendContent { unit, request }, Reply::ContentSaved(saved)) => {
                let submitted = self
                    .content_submissions
                    .remove(&request.operation_id)
                    .filter(|(target, form)| {
                        target == unit && form.details().as_ref() == Some(&request.details)
                    })
                    .map(|(_, form)| form)
                    .unwrap_or_else(|| {
                        let mut form = ContentForm::from(&request.details);
                        if form.kind == "script" {
                            form.kind = "editor-script".into();
                        }
                        form
                    });
                let draft = self.content.entry(unit.clone()).or_default();
                draft.form.acknowledge(submitted);
                draft.base = Some(saved.content.revision);
            }
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn unit(id: &str) -> CatalogContentUnit {
        CatalogContentUnit {
            id: id.into(),
            work_id: "work".into(),
            kind: "chapter".into(),
            details: UnitDetailsDto {
                title: format!("Chương {id}"),
                description: String::new(),
                source: None,
                script: None,
                content_digest: None,
            },
            lifecycle: "draft".into(),
            revision: 1,
            position: 0,
            created_at: 0,
            updated_at: 0,
        }
    }
    fn head(unit: &str, revision: u64, text: &str) -> ContentRevisionResponse {
        ContentRevisionResponse {
            id: "revision".into(),
            work_id: "work".into(),
            unit_id: unit.into(),
            revision,
            details: ContentDetails {
                language: "vi-VN".into(),
                body: ContentBody::Text {
                    text: text.into(),
                    source: None,
                },
                rights_refs: vec!["author".into()],
                provenance_note: "Tác giả".into(),
            },
            content_digest: "digest".into(),
            revision_digest: "digest".into(),
            recorded_by: "actor".into(),
            recorded_at: 0,
        }
    }
    #[test]
    fn unit_navigation_retains_independent_metadata_and_content_and_reverting_is_clean() {
        let mut drafts = AuthoringDrafts::default();
        let (original, _) = drafts.open_unit(&unit("a"));
        drafts.head(&head("a", 2, "Ánh đèn"), None);
        let baseline = drafts.content("a");
        drafts.edit_metadata(
            "a",
            UnitForm {
                title: "Tiêu đề nháp".into(),
                ..original.clone()
            },
        );
        drafts.edit_content(
            "a",
            ContentForm {
                text: "Bản nháp chưa lưu".into(),
                ..baseline.clone()
            },
        );
        let (_, b) = drafts.open_unit(&unit("b"));
        assert_eq!(b, ContentForm::default());
        assert!(drafts.has_unsaved());
        let (a, content) = drafts.open_unit(&unit("a"));
        assert_eq!(a.title, "Tiêu đề nháp");
        assert_eq!(content.text, "Bản nháp chưa lưu");
        drafts.edit_metadata("a", original);
        drafts.edit_content("a", baseline);
        assert!(!drafts.has_unsaved());
    }
    #[test]
    fn refresh_preserves_dirty_bytes_and_base_until_exact_confirmed_replacement() {
        let mut drafts = AuthoringDrafts::default();
        drafts.open_unit(&unit("a"));
        drafts.head(&head("a", 1, "Bản gốc"), None);
        let snapshot = ContentForm {
            text: "Bản nháp".into(),
            ..drafts.content("a")
        };
        drafts.edit_content("a", snapshot.clone());
        drafts.head(&head("a", 2, "Bản ở server"), None);
        assert_eq!(drafts.content_base("a"), Some(1));
        assert_eq!(drafts.content("a").text, "Bản nháp");
        drafts.edit_content(
            "a",
            ContentForm {
                text: "Gõ sau xác nhận".into(),
                ..snapshot.clone()
            },
        );
        drafts.head(&head("a", 2, "Bản ở server"), Some(&snapshot));
        assert_eq!(drafts.content("a").text, "Gõ sau xác nhận");
        let current = drafts.content("a");
        drafts.head(&head("a", 2, "Bản ở server"), Some(&current));
        assert_eq!(drafts.content("a").text, "Bản ở server");
        assert_eq!(drafts.content_base("a"), Some(2));
        assert!(!drafts.has_unsaved());
    }
    #[test]
    fn typing_during_initial_read_or_save_survives_and_only_submitted_snapshot_is_clean() {
        let mut drafts = AuthoringDrafts::default();
        drafts.open_unit(&unit("a"));
        drafts.edit_content(
            "a",
            ContentForm {
                text: "Đang gõ".into(),
                ..ContentForm::default()
            },
        );
        drafts.head(&head("a", 3, "Đã lưu"), None);
        assert_eq!(drafts.content("a").text, "Đang gõ");
        assert_eq!(drafts.content_base("a"), Some(3));
        let submitted = drafts.content("a");
        let command = Command::AppendContent {
            unit: "a".into(),
            request: AppendContentRequest {
                operation_id: "op".into(),
                id: "revision".into(),
                expected_revision: 3,
                expected_work_revision: 1,
                details: submitted.details().unwrap(),
            },
        };
        drafts.edit_content(
            "a",
            ContentForm {
                text: "Gõ tiếp".into(),
                ..submitted.clone()
            },
        );
        let reply = Reply::ContentSaved(AppendContentResponse {
            content: head("a", 4, "Đang gõ"),
            work_revision: 2,
        });
        drafts.acknowledge(&command, &reply);
        assert_eq!(drafts.content("a").text, "Gõ tiếp");
        assert!(drafts.has_unsaved());
        assert_eq!(drafts.content_base("a"), Some(4));
        drafts.edit_content("a", submitted);
        assert!(!drafts.has_unsaved());
    }
    #[test]
    fn linked_content_does_not_implicitly_become_empty_text_and_text_keeps_source() {
        let source = CatalogSourceRef {
            id: "source".into(),
            sha256: "digest".into(),
        };
        let mut details = head("a", 1, "Văn bản").details;
        details.body = ContentBody::Source {
            source: source.clone(),
        };
        assert!(ContentForm::from(&details).details().is_none());
        details.body = ContentBody::Text {
            text: "Văn bản".into(),
            source: Some(source),
        };
        assert_eq!(ContentForm::from(&details).details(), Some(details));
    }
    #[test]
    fn script_save_acknowledges_the_pinned_snapshot_and_preserves_later_edits() {
        let first = CatalogScriptRef {
            id: "script".into(),
            revision: 3,
            export_digest: "digest-3".into(),
        };
        let selected = ContentForm {
            kind: "editor-script".into(),
            script: Some(first.clone()),
            text: "Retained text".into(),
            ..ContentForm::default()
        };
        let mut drafts = AuthoringDrafts::default();
        drafts.open_unit(&unit("a"));
        drafts.edit_content("a", selected.clone());
        let request = AppendContentRequest {
            operation_id: "save".into(),
            id: "new".into(),
            expected_revision: 0,
            expected_work_revision: 1,
            details: selected.details().unwrap(),
        };
        drafts.submitted_content("a", &request);
        let command = Command::AppendContent {
            unit: "a".into(),
            request: request.clone(),
        };
        let reply = Reply::ContentSaved(AppendContentResponse {
            content: ContentRevisionResponse {
                details: request.details.clone(),
                ..head("a", 1, "")
            },
            work_revision: 2,
        });
        let mut later = selected.clone();
        later.script = Some(CatalogScriptRef {
            revision: 4,
            export_digest: "digest-4".into(),
            ..first
        });
        drafts.edit_content("a", later.clone());
        drafts.acknowledge(&command, &reply);
        assert_eq!(drafts.content("a"), later);
        assert!(drafts.has_unsaved());
        drafts.edit_content("a", selected);
        assert!(!drafts.has_unsaved());
    }
    #[test]
    fn refreshed_catalog_cannot_silently_rebase_unsaved_unit_metadata() {
        let mut drafts = AuthoringDrafts::default();
        let original = unit("a");
        drafts.open_unit(&original);
        drafts.edit_metadata(
            "a",
            UnitForm {
                title: "Bản nháp riêng".into(),
                description: String::new(),
            },
        );
        let mut changed = original.clone();
        changed.revision = 2;
        changed.details.title = "Đã đổi ở cửa sổ khác".into();
        let (local, _) = drafts.open_unit(&changed);
        assert_eq!(local.title, "Bản nháp riêng");
        assert_eq!(drafts.metadata_base("a"), Some(1));
        let restored = drafts.reload_metadata(&changed);
        assert_eq!(restored.title, "Đã đổi ở cửa sổ khác");
        assert_eq!(drafts.metadata_base("a"), Some(2));
        assert!(!drafts.has_unsaved());
    }
}
