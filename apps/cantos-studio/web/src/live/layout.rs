//! Pure composition rules for the content workspace (#571): which task area is shown and what
//! the editor status line says. It reads facts the existing owners already hold, keeps no request,
//! draft or session state and grants no capability; the Server remains the authority.
use super::{Command, Phase};
use crate::contract::ContentRevisionResponse;

/// One task area of the selected Work or chapter / episode. Every area stays mounted so a hidden
/// area keeps its drafts, focus targets and private fencing; only visibility changes.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Task {
    #[default]
    Write,
    History,
    Details,
    Resources,
    Audio,
    CreateWork,
    CreateUnit,
}

impl Task {
    /// Areas offered as tabs. Creation forms are opened from the navigation instead.
    pub const AREAS: [Task; 5] = [
        Task::Write,
        Task::History,
        Task::Details,
        Task::Resources,
        Task::Audio,
    ];

    pub fn chip_id(self) -> &'static str {
        match self {
            Task::Write => "live-task-write",
            Task::History => "live-task-history",
            Task::Details => "live-task-details",
            Task::Resources => "live-task-resources",
            Task::Audio => "live-task-audio",
            Task::CreateWork => "live-task-create-work",
            Task::CreateUnit => "live-task-create-unit",
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Task::Write => "write",
            Task::History => "history",
            Task::Details => "details",
            Task::Resources => "resources",
            Task::Audio => "audio",
            Task::CreateWork => "create-work",
            Task::CreateUnit => "create-unit",
        }
    }

    /// Whether the area exists for the current selection. Chips for absent areas are hidden.
    pub fn available(self, has_work: bool, has_unit: bool) -> bool {
        match self {
            Task::CreateWork => true,
            Task::CreateUnit | Task::Details | Task::Resources => has_work,
            Task::Write | Task::History | Task::Audio => has_unit,
        }
    }
}

/// The area actually shown for a requested task. A unit-bound request without a selected
/// chapter / episode falls back to the Work's details; `None` means nothing is selected yet.
pub fn resolve(requested: Task, has_work: bool, has_unit: bool) -> Option<Task> {
    if requested == Task::CreateWork {
        return Some(Task::CreateWork);
    }
    if !has_work {
        return None;
    }
    if requested.available(has_work, has_unit) {
        Some(requested)
    } else {
        Some(Task::Details)
    }
}

/// Narrow layouts show one pane at a time; wide layouts show both and ignore this value.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Pane {
    #[default]
    Browse,
    Work,
}

impl Pane {
    pub fn as_str(self) -> &'static str {
        match self {
            Pane::Browse => "browse",
            Pane::Work => "work",
        }
    }
}

/// Editor status shown beside the writing area. It combines the draft owner's save base and
/// dirtiness with the lifecycle and request facts; it never claims a save the Server has not
/// acknowledged (`base` advances only on a head read or an acknowledged append).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorStatus {
    NotRead,
    NoSavedContent,
    Saved(u64),
    Unsaved(u64),
    Saving,
    ArchivedReadOnly,
}

impl EditorStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            EditorStatus::NotRead => "not-read",
            EditorStatus::NoSavedContent => "empty",
            EditorStatus::Saved(_) => "saved",
            EditorStatus::Unsaved(_) => "unsaved",
            EditorStatus::Saving => "saving",
            EditorStatus::ArchivedReadOnly => "read-only",
        }
    }
}

pub fn editor_status(
    base: Option<u64>,
    dirty: bool,
    saving: bool,
    read_only: bool,
) -> EditorStatus {
    if saving {
        return EditorStatus::Saving;
    }
    if read_only {
        return EditorStatus::ArchivedReadOnly;
    }
    match (base, dirty) {
        (None, _) => EditorStatus::NotRead,
        (Some(revision), true) => EditorStatus::Unsaved(revision),
        (Some(0), false) => EditorStatus::NoSavedContent,
        (Some(revision), false) => EditorStatus::Saved(revision),
    }
}

/// A content append for the selected chapter / episode is in flight (not ambiguous, not settled).
pub fn saving_selected(pending: Option<&Command>, phase: Phase, unit: &str) -> bool {
    phase == Phase::Pending
        && matches!(pending, Some(Command::AppendContent { unit: target, .. }) if target == unit)
}

/// What the opened revision is, relative to the current head.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Viewing {
    Nothing,
    Current,
    Historical,
}

pub fn viewing(
    opened: Option<&ContentRevisionResponse>,
    head: Option<&ContentRevisionResponse>,
    unit: &str,
) -> Viewing {
    let Some(opened) = opened.filter(|opened| opened.unit_id == unit) else {
        return Viewing::Nothing;
    };
    match head {
        Some(head) if head.unit_id == unit && head.id == opened.id => Viewing::Current,
        _ => Viewing::Historical,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ContentBody, ContentDetails};

    fn revision(id: &str, unit_id: &str, revision: u64) -> ContentRevisionResponse {
        ContentRevisionResponse {
            id: id.into(),
            work_id: "work".into(),
            unit_id: unit_id.into(),
            revision,
            details: ContentDetails {
                language: "vi-VN".into(),
                body: ContentBody::Text {
                    text: "Mở đầu".into(),
                    source: None,
                },
                rights_refs: vec![],
                provenance_note: String::new(),
            },
            content_digest: "c".into(),
            revision_digest: "r".into(),
            recorded_by: "owner".into(),
            recorded_at: 0,
        }
    }

    #[test]
    fn a_unit_bound_task_without_a_unit_shows_the_work_details_and_nothing_without_a_work() {
        for task in [Task::Write, Task::History, Task::Audio] {
            assert_eq!(resolve(task, true, false), Some(Task::Details));
            assert_eq!(resolve(task, true, true), Some(task));
            assert_eq!(resolve(task, false, false), None);
        }
        assert_eq!(resolve(Task::Resources, true, false), Some(Task::Resources));
        assert_eq!(resolve(Task::CreateUnit, false, false), None);
        assert_eq!(
            resolve(Task::CreateUnit, true, false),
            Some(Task::CreateUnit)
        );
    }

    #[test]
    fn creating_a_work_needs_no_selection() {
        assert_eq!(
            resolve(Task::CreateWork, false, false),
            Some(Task::CreateWork)
        );
        assert_eq!(
            resolve(Task::CreateWork, true, true),
            Some(Task::CreateWork)
        );
    }

    #[test]
    fn exactly_one_area_is_shown_and_every_area_has_a_distinct_chip_id() {
        let all = [
            Task::Write,
            Task::History,
            Task::Details,
            Task::Resources,
            Task::Audio,
            Task::CreateWork,
            Task::CreateUnit,
        ];
        for (has_work, has_unit) in [(false, false), (true, false), (true, true)] {
            for requested in all {
                let shown = resolve(requested, has_work, has_unit);
                assert_eq!(
                    all.iter().filter(|task| Some(**task) == shown).count(),
                    usize::from(shown.is_some())
                );
            }
        }
        let mut ids = all.map(Task::chip_id).to_vec();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), all.len());
    }

    #[test]
    fn data_attribute_values_are_the_contract_the_stylesheet_selects_on() {
        // studio.css hides panes and colours the status chip by these exact values.
        assert_eq!(Pane::Browse.as_str(), "browse");
        assert_eq!(Pane::Work.as_str(), "work");
        assert_eq!(Pane::default(), Pane::Browse);
        assert_eq!(Task::default(), Task::Write);
        let states = [
            (EditorStatus::NotRead, "not-read"),
            (EditorStatus::NoSavedContent, "empty"),
            (EditorStatus::Saved(2), "saved"),
            (EditorStatus::Unsaved(2), "unsaved"),
            (EditorStatus::Saving, "saving"),
            (EditorStatus::ArchivedReadOnly, "read-only"),
        ];
        for (status, value) in states {
            assert_eq!(status.as_str(), value);
        }
        assert_eq!(
            Task::AREAS.map(Task::as_str),
            ["write", "history", "details", "resources", "audio"]
        );
    }

    #[test]
    fn tabs_never_offer_an_area_without_its_parent_selection() {
        for area in Task::AREAS {
            assert!(!area.available(false, false), "{area:?} needs a Work");
            assert_eq!(
                area.available(true, false),
                matches!(area, Task::Details | Task::Resources)
            );
            assert!(area.available(true, true));
        }
        assert!(Task::CreateWork.available(false, false));
        assert!(!Task::CreateUnit.available(false, false));
    }

    #[test]
    fn missing_head_is_not_read_zero_base_is_no_saved_content_and_neither_is_saved() {
        assert_eq!(
            editor_status(None, false, false, false),
            EditorStatus::NotRead
        );
        assert_eq!(
            editor_status(Some(0), false, false, false),
            EditorStatus::NoSavedContent
        );
        assert_eq!(
            editor_status(Some(3), false, false, false),
            EditorStatus::Saved(3)
        );
        assert_eq!(
            editor_status(Some(0), true, false, false),
            EditorStatus::Unsaved(0)
        );
        assert_eq!(
            editor_status(Some(3), true, false, false),
            EditorStatus::Unsaved(3)
        );
    }

    #[test]
    fn saving_and_archived_take_precedence_over_dirty_and_never_report_saved() {
        assert_eq!(
            editor_status(Some(3), true, true, false),
            EditorStatus::Saving
        );
        assert_eq!(
            editor_status(Some(3), true, true, true),
            EditorStatus::Saving
        );
        assert_eq!(
            editor_status(Some(3), false, false, true),
            EditorStatus::ArchivedReadOnly
        );
        assert_eq!(
            editor_status(Some(3), true, false, true),
            EditorStatus::ArchivedReadOnly
        );
    }

    #[test]
    fn only_a_pending_append_for_the_selected_unit_is_saving() {
        let append = |target: &str| Command::AppendContent {
            unit: target.into(),
            request: crate::contract::AppendContentRequest {
                operation_id: "op".into(),
                id: "id".into(),
                expected_revision: 1,
                expected_work_revision: 1,
                details: ContentDetails {
                    language: "vi-VN".into(),
                    body: ContentBody::Recording {},
                    rights_refs: vec![],
                    provenance_note: String::new(),
                },
            },
        };
        assert!(saving_selected(
            Some(&append("unit")),
            Phase::Pending,
            "unit"
        ));
        assert!(!saving_selected(
            Some(&append("other")),
            Phase::Pending,
            "unit"
        ));
        assert!(!saving_selected(
            Some(&append("unit")),
            Phase::Ambiguous,
            "unit"
        ));
        assert!(!saving_selected(
            Some(&append("unit")),
            Phase::Ready,
            "unit"
        ));
        let read = Command::ContentHead {
            unit: "unit".into(),
        };
        assert!(!saving_selected(Some(&read), Phase::Pending, "unit"));
        assert!(!saving_selected(None, Phase::Pending, "unit"));
    }

    #[test]
    fn the_viewer_separates_nothing_current_head_and_a_historical_revision() {
        let head = revision("r3", "unit", 3);
        let older = revision("r1", "unit", 1);
        assert_eq!(viewing(None, Some(&head), "unit"), Viewing::Nothing);
        assert_eq!(viewing(Some(&head), Some(&head), "unit"), Viewing::Current);
        assert_eq!(
            viewing(Some(&older), Some(&head), "unit"),
            Viewing::Historical
        );
        assert_eq!(viewing(Some(&older), None, "unit"), Viewing::Historical);
        // A reply that belongs to another chapter / episode is never shown as this one's.
        let foreign = revision("r1", "other", 1);
        assert_eq!(
            viewing(Some(&foreign), Some(&head), "unit"),
            Viewing::Nothing
        );
        assert_eq!(
            viewing(Some(&head), Some(&revision("r3", "other", 3)), "unit"),
            Viewing::Historical
        );
    }
}
