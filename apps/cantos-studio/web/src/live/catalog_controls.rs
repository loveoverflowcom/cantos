//! Catalog metadata drafts and explicit soft-archive/reorder intents.
use crate::contract::{CatalogContentUnit, CatalogWork};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WorkDraft {
    id: String,
    revision: u64,
    pub title: String,
    pub synopsis: String,
    baseline: (String, String),
    submitted: Option<(String, String)>,
}

impl WorkDraft {
    pub fn is_dirty(&self) -> bool {
        (&self.title, &self.synopsis) != (&self.baseline.0, &self.baseline.1)
    }

    pub fn base(&self) -> u64 {
        self.revision
    }

    fn loaded(work: &CatalogWork) -> Self {
        Self {
            id: work.id.clone(),
            revision: work.revision,
            title: work.details.title.clone(),
            synopsis: work.details.synopsis.clone(),
            baseline: (work.details.title.clone(), work.details.synopsis.clone()),
            submitted: None,
        }
    }

    /// Reads may refresh clean metadata, but never advance an unsaved draft's CAS base.
    pub fn observe(&self, work: Option<&CatalogWork>) -> Self {
        let Some(work) = work else {
            return Self::default();
        };
        if self.id != work.id || !self.is_dirty() {
            Self::loaded(work)
        } else {
            self.clone()
        }
    }

    /// Only the dispatcher's acknowledged mutation settles a submitted metadata snapshot.
    pub fn acknowledged(&self, work: &CatalogWork) -> Self {
        let Some(submitted) = &self.submitted else {
            return self.observe(Some(work));
        };
        if self.id != work.id || work.revision <= self.revision {
            return self.clone();
        }
        let accepted = Self::loaded(work);
        if submitted == &(self.title.clone(), self.synopsis.clone()) {
            accepted
        } else {
            Self {
                title: self.title.clone(),
                synopsis: self.synopsis.clone(),
                ..accepted
            }
        }
    }

    /// A separate confirmed intent chooses this exact already-read snapshot.
    pub fn reload(&self, work: &CatalogWork) -> Self {
        Self::loaded(work)
    }

    pub fn save_command(&self, operation_id: String) -> Option<super::Command> {
        use crate::contract::{PatchField, PatchWorkRequest, WorkPatchDto};
        if self.id.is_empty() || !self.is_dirty() {
            return None;
        }
        Some(super::Command::PatchWork {
            work: self.id.clone(),
            request: PatchWorkRequest {
                operation_id,
                expected_revision: self.base(),
                details: WorkPatchDto {
                    title: PatchField::Value(self.title.clone()),
                    synopsis: PatchField::Value(self.synopsis.clone()),
                    ..WorkPatchDto::default()
                },
            },
        })
    }

    pub fn failed(&self) -> Self {
        Self {
            submitted: None,
            ..self.clone()
        }
    }

    pub fn submitted(&self) -> Self {
        Self {
            submitted: Some((self.title.clone(), self.synopsis.clone())),
            ..self.clone()
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Direction {
    Earlier,
    Later,
}

/// A bounded page cannot describe a reorder unless it is the complete initial page.
/// The server still decides ownership, exact active-set admission and compare-and-swap.
pub fn reordered_ids(
    work: &str,
    units: &[CatalogContentUnit],
    complete: bool,
    selected: &str,
    direction: Direction,
) -> Option<Vec<String>> {
    if !complete || units.iter().any(|unit| unit.work_id != work) {
        return None;
    }
    let mut active = units
        .iter()
        .filter(|unit| unit.lifecycle == "draft")
        .collect::<Vec<_>>();
    active.sort_by_key(|unit| (unit.position, &unit.id));
    let mut ordered = active
        .iter()
        .map(|unit| unit.id.clone())
        .collect::<Vec<_>>();
    if ordered
        .iter()
        .collect::<std::collections::BTreeSet<_>>()
        .len()
        != ordered.len()
    {
        return None;
    }
    let index = ordered.iter().position(|id| id == selected)?;
    let adjacent = match direction {
        Direction::Earlier => index.checked_sub(1)?,
        Direction::Later => index.checked_add(1).filter(|next| *next < ordered.len())?,
    };
    ordered.swap(index, adjacent);
    Some(ordered)
}

#[cfg(target_arch = "wasm32")]
mod browser {
    use super::{reordered_ids, Direction, WorkDraft};
    use crate::{
        contract::{ArchiveContentUnitRequest, ArchiveWorkRequest, ReorderContentUnitsRequest},
        live::{
            copy::{text, Key},
            workspace::{uuid, Dispatch},
            ClientError, Command, Facts, Reply,
        },
        ui::{ActionButton, BlockReason},
    };
    use cantos_ui::{ButtonVariant, FieldState, IconName, TextArea, TextField, TextFieldVariant};
    use leptos::prelude::*;

    fn confirm(key: Key, title: &str, revision: u64, english: bool) -> bool {
        web_sys::window().is_some_and(|window| {
            window
                .confirm_with_message(&format!(
                    "{}\n{} · r{}",
                    text(key, english),
                    title,
                    revision
                ))
                .unwrap_or(false)
        })
    }

    #[component]
    pub(in crate::live) fn WorkControls(
        facts: RwSignal<Facts>,
        dispatch: Dispatch,
        english: Signal<bool>,
        blocked: Signal<bool>,
        unsaved: Signal<bool>,
        dirty: RwSignal<bool>,
    ) -> impl IntoView {
        let draft = RwSignal::new(WorkDraft::default());
        Effect::new(move |_| {
            let observed = facts.with(|facts| facts.work.clone());
            let acknowledgement = dispatch.state.with(|state| match state.result() {
                Some(Reply::Mutation(reply)) => Some(reply.work.clone()),
                _ => None,
            });
            draft.update(|draft| {
                *draft = match (&observed, &acknowledgement) {
                    (Some(work), Some(accepted)) if work == accepted => draft.acknowledged(work),
                    _ => draft.observe(observed.as_ref()),
                };
            });
        });
        Effect::new(move |_| {
            if dispatch
                .state
                .with(|state| state.phase() == crate::live::Phase::Failed)
            {
                draft.update(|draft| *draft = draft.failed());
            }
        });
        Effect::new(move |_| dirty.set(draft.with(WorkDraft::is_dirty)));
        on_cleanup(move || {
            let _ = dirty.try_set(false);
        });
        let cannot_edit = Signal::derive(move || {
            blocked.get()
                || facts.with(|facts| {
                    facts
                        .work
                        .as_ref()
                        .is_none_or(|work| work.lifecycle != "draft")
                })
        });
        let Ok(reason) = crate::ui::LocalizedReason::new(BlockReason::ReadOnly) else {
            return ().into_any();
        };
        let reason = StoredValue::new(reason);
        let field_state = Signal::derive(move || {
            if cannot_edit.get() {
                FieldState::Disabled(reason.with_value(|reason| reason.get(english.get())))
            } else {
                FieldState::Editable
            }
        });
        view! {
            <details id="live-work-metadata"><summary>{move || text(Key::EditWork, english.get())}</summary>
                <div class="live-grid">
                    <TextField id="live-edit-work-title" label=Signal::derive(move ||text(Key::Title,english.get()).to_owned())
                        variant=TextFieldVariant::Floating state=field_state class="live-field"
                        value=Signal::derive(move ||draft.with(|draft|draft.title.clone()))
                        on_input=Callback::new(move |value|draft.update(|draft|draft.title=value))/>
                    <TextArea id="live-edit-work-synopsis" label=Signal::derive(move ||text(Key::Synopsis,english.get()).to_owned())
                        state=field_state class="live-field" value=Signal::derive(move ||draft.with(|draft|draft.synopsis.clone()))
                        on_input=Callback::new(move |value|draft.update(|draft|draft.synopsis=value))/>

                </div>
                <ActionButton id="live-patch-work" english=english reason=BlockReason::ContextReady blocked=Signal::derive(move ||cannot_edit.get()||!draft.with(WorkDraft::is_dirty)) on_press=Callback::new(move |_| {
                    if cannot_edit.get_untracked() { return; }
                    dispatch.build((|| {
                        let work=facts.with_untracked(|facts|facts.work.clone()).ok_or(ClientError::InvalidInput)?;
                        let snapshot=draft.get_untracked();
                        if work.id!=snapshot.id { return Err(ClientError::InvalidInput); }
                        let command=snapshot.save_command(uuid()?).ok_or(ClientError::InvalidInput)?;
                        draft.set(snapshot.submitted());
                        Ok(command)
                    })());
                })>{move ||text(Key::SaveMetadata,english.get())}</ActionButton>
            </details>
            <ActionButton id="live-read-work" english=english variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                if blocked.get_untracked() { return; }
                if let Some(work)=facts.with_untracked(|facts|facts.work.clone()) { dispatch.send(Command::Work {id:work.id}); }
            })>{move ||text(Key::ReadWork,english.get())}</ActionButton>
            <ActionButton id="live-reload-work-metadata" english=english variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=blocked on_press=Callback::new(move |_| {
                if blocked.get_untracked() { return; }
                if let Some(work)=facts.with_untracked(|facts|facts.work.clone()) {
                    if !draft.with_untracked(WorkDraft::is_dirty) || confirm(Key::ReloadWorkConfirm,&work.details.title,work.revision,english.get_untracked()) {
                        draft.update(|draft| *draft=draft.reload(&work));
                    }
                }
            })>{move ||text(Key::ReloadWorkMetadata,english.get())}</ActionButton>
            <ActionButton id="live-archive-work" english=english variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=Signal::derive(move ||cannot_edit.get()||unsaved.get()) on_press=Callback::new(move |_| {
                if cannot_edit.get_untracked() || unsaved.get_untracked() { return; }
                if let Some(work)=facts.with_untracked(|facts|facts.work.clone()) {
                    if confirm(Key::ArchiveWorkConfirm,&work.details.title,work.revision,english.get_untracked()) {
                        dispatch.build(uuid().map(|operation_id|Command::ArchiveWork {work:work.id,request:ArchiveWorkRequest {operation_id,expected_revision:work.revision}}));
                    }
                }
            })>{move ||text(Key::ArchiveWork,english.get())}</ActionButton>
            <p class="help">{move ||text(Key::ArchiveHelp,english.get())}</p>
        }.into_any()
    }

    #[component]
    pub(in crate::live) fn UnitControls(
        facts: RwSignal<Facts>,
        dispatch: Dispatch,
        english: Signal<bool>,
        blocked: Signal<bool>,
        unsaved: Signal<bool>,
        units_complete: Signal<bool>,
    ) -> impl IntoView {
        let cannot_edit = Signal::derive(move || {
            blocked.get()
                || facts.with(|facts| {
                    facts
                        .work
                        .as_ref()
                        .is_none_or(|work| work.lifecycle != "draft")
                        || facts
                            .unit
                            .as_ref()
                            .is_none_or(|unit| unit.lifecycle != "draft")
                })
        });
        let order = move |direction| {
            facts.with_untracked(|facts| {
                let work = facts.work.as_ref()?;
                let unit = facts.unit.as_ref()?;
                reordered_ids(
                    &work.id,
                    &facts.units,
                    units_complete.get_untracked(),
                    &unit.id,
                    direction,
                )
            })
        };
        let reorder = Callback::new(move |direction| {
            if cannot_edit.get_untracked() || unsaved.get_untracked() {
                return;
            }
            dispatch.build((|| {
                let ordered = order(direction).ok_or(ClientError::InvalidInput)?;
                let work = facts
                    .with_untracked(|facts| facts.work.clone())
                    .ok_or(ClientError::InvalidInput)?;
                Ok(Command::Reorder {
                    work: work.id,
                    request: ReorderContentUnitsRequest {
                        operation_id: uuid()?,
                        expected_revision: work.revision,
                        ordered,
                    },
                })
            })());
        });
        view! {
            <div class="toolbar">
                <ActionButton id="live-unit-earlier" icon=IconName::ArrowUpward english=english reason=BlockReason::ContextReady blocked=Signal::derive(move || {
                    facts.with(|_| ()); let _=units_complete.get(); cannot_edit.get()||unsaved.get()||order(Direction::Earlier).is_none()
                }) on_press=Callback::new(move |_|reorder.run(Direction::Earlier))>{move ||text(Key::MoveEarlier,english.get())}</ActionButton>
                <ActionButton id="live-unit-later" icon=IconName::ArrowDownward english=english reason=BlockReason::ContextReady blocked=Signal::derive(move || {
                    facts.with(|_| ()); let _=units_complete.get(); cannot_edit.get()||unsaved.get()||order(Direction::Later).is_none()
                }) on_press=Callback::new(move |_|reorder.run(Direction::Later))>{move ||text(Key::MoveLater,english.get())}</ActionButton>
                <ActionButton id="live-archive-unit" icon=IconName::Archive english=english variant=ButtonVariant::Text reason=BlockReason::ContextReady blocked=Signal::derive(move ||cannot_edit.get()||unsaved.get()) on_press=Callback::new(move |_| {
                    if cannot_edit.get_untracked() || unsaved.get_untracked() { return; }
                    if let Some((work,unit))=facts.with_untracked(|facts|Some((facts.work.clone()?,facts.unit.clone()?))) {
                        if confirm(Key::ArchiveUnitConfirm,&unit.details.title,unit.revision,english.get_untracked()) {
                            dispatch.build(uuid().map(|operation_id|Command::ArchiveUnit {unit:unit.id,request:ArchiveContentUnitRequest {operation_id,expected_revision:unit.revision,expected_work_revision:work.revision}}));
                        }
                    }
                })>{move ||text(Key::ArchiveUnit,english.get())}</ActionButton>
            </div>
            <p class="help">{move ||text(Key::ReorderHelp,english.get())}</p>
        }
    }
}

#[cfg(target_arch = "wasm32")]
pub(in crate::live) use browser::{UnitControls, WorkControls};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{UnitDetailsDto, WorkDetailsDto};

    fn work() -> CatalogWork {
        CatalogWork {
            id: "work".into(),
            kind: "story".into(),
            owner_id: "owner".into(),
            details: WorkDetailsDto {
                title: "Ánh đèn cuối sân khấu".into(),
                synopsis: "Bản đã lưu".into(),
                language: "vi-VN".into(),
                visibility: "private".into(),
                rights_refs: vec![],
            },
            lifecycle: "draft".into(),
            revision: 1,
            created_at: 0,
            updated_at: 0,
        }
    }
    fn unit(id: &str, position: u32) -> CatalogContentUnit {
        CatalogContentUnit {
            id: id.into(),
            work_id: "work".into(),
            kind: "chapter".into(),
            details: UnitDetailsDto {
                title: id.into(),
                description: String::new(),
                source: None,
                script: None,
                content_digest: None,
            },
            position,
            lifecycle: "draft".into(),
            revision: 1,
            created_at: 0,
            updated_at: 0,
        }
    }
    #[test]
    fn an_acknowledged_work_snapshot_becomes_clean_without_losing_newer_typing() {
        let mut remote = work();
        let mut draft = WorkDraft::default().observe(Some(&remote));
        draft.title = "Tên mới".into();
        draft = draft.submitted();
        remote.details.title = "Tên mới".into();
        remote.revision = 2;
        let clean = draft.acknowledged(&remote);
        assert!(!clean.is_dirty());
        draft.synopsis = "Đoạn viết trong lúc chờ".into();
        let retained = draft.acknowledged(&remote);
        assert!(retained.is_dirty());
        assert_eq!(retained.synopsis, "Đoạn viết trong lúc chờ");
        assert_eq!(retained.title, "Tên mới");
    }
    #[test]
    fn a_failed_write_never_acknowledges_a_later_unrelated_revision() {
        let mut remote = work();
        let mut draft = WorkDraft::default().observe(Some(&remote));
        draft.title = "Chưa lưu".into();
        draft = draft.submitted().failed();
        remote.revision = 2;
        let retained = draft.observe(Some(&remote));
        assert!(retained.is_dirty());
        assert_eq!(retained.title, "Chưa lưu");
    }
    #[test]
    fn child_revision_changes_preserve_a_dirty_work_form_and_owner_reset_clears_it() {
        let mut remote = work();
        let mut draft = WorkDraft::default().observe(Some(&remote));
        draft.synopsis = "Chưa lưu".into();
        remote.revision = 2;
        assert_eq!(draft.observe(Some(&remote)).synopsis, "Chưa lưu");
        assert_eq!(draft.observe(None), WorkDraft::default());
        remote.id = "another".into();
        assert_eq!(draft.observe(Some(&remote)).synopsis, "Bản đã lưu");
    }
    #[test]
    fn refreshing_newer_work_never_rebases_an_unsaved_metadata_patch() {
        let mut remote = work();
        let mut draft = WorkDraft::default().observe(Some(&remote));
        draft.synopsis = "Mô tả chưa lưu".into();
        remote.revision = 2;
        remote.details.title = "Tiêu đề do người khác sửa".into();
        let retained = draft.observe(Some(&remote));
        assert_eq!(retained.base(), 1);
        assert_eq!(retained.title, "Ánh đèn cuối sân khấu");
        assert_eq!(retained.synopsis, "Mô tả chưa lưu");
        let Some(super::super::Command::PatchWork { request, .. }) =
            retained.save_command("operation".into())
        else {
            panic!("dirty metadata command");
        };
        assert_eq!(request.expected_revision, 1);
        // Server r2 must reject that r1 CAS instead of accepting stale title replacement.
        assert_ne!(request.expected_revision, remote.revision);
        let reloaded = retained.reload(&remote);
        assert_eq!(reloaded.base(), 2);
        assert_eq!(reloaded.title, "Tiêu đề do người khác sửa");
        assert_eq!(reloaded.synopsis, "Bản đã lưu");
        assert!(!reloaded.is_dirty());
    }

    #[test]
    fn reorder_requires_a_complete_owned_page_and_preserves_every_active_identity() {
        let units = vec![unit("b", 2), unit("a", 0), unit("c", 3)];
        assert_eq!(
            reordered_ids("work", &units, false, "b", Direction::Earlier),
            None
        );
        assert_eq!(
            reordered_ids("work", &units, true, "b", Direction::Earlier),
            Some(vec!["b".into(), "a".into(), "c".into()])
        );
        assert_eq!(
            reordered_ids("work", &units, true, "b", Direction::Later),
            Some(vec!["a".into(), "c".into(), "b".into()])
        );
        assert_eq!(
            reordered_ids("work", &units, true, "a", Direction::Earlier),
            None
        );
        assert_eq!(
            reordered_ids("work", &units, true, "c", Direction::Later),
            None
        );
        assert_eq!(
            reordered_ids("foreign", &units, true, "b", Direction::Earlier),
            None
        );
    }
    #[test]
    fn reorder_excludes_archive_tombstones_and_refuses_duplicate_identity() {
        let mut units = vec![unit("a", 0), unit("tombstone", 1), unit("b", 2)];
        units[1].lifecycle = "archived".into();
        assert_eq!(
            reordered_ids("work", &units, true, "b", Direction::Earlier),
            Some(vec!["b".into(), "a".into()])
        );
        units.push(unit("a", 3));
        assert_eq!(
            reordered_ids("work", &units, true, "b", Direction::Earlier),
            None
        );
    }
}
