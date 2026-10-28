//! Operation-bound feedback: what the settled exchange answers, never a generic "ready".
//!
//! `RequestState` stays the only request authority. This projection reads the command it kept
//! with the exchange, so success for one task cannot be shown as success for another, and a
//! missing head (nothing saved yet) is neither a load failure nor an existing revision.
use super::{copy::Key, ClientError, Command, Phase, RequestState};
use crate::contract::ErrorCode;

/// The Catalog/Content task a command performs. Commands owned by other panels are `Other`
/// and keep the generic phase wording; their panels own their feedback.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    Read,
    CreateWork,
    SaveWork,
    ArchiveWork,
    CreateUnit,
    SaveUnit,
    ArchiveUnit,
    Reorder,
    SaveContent,
    Other,
}

impl Operation {
    pub fn of(command: &Command) -> Self {
        match command {
            Command::Works { .. }
            | Command::Work { .. }
            | Command::Units { .. }
            | Command::ContentHead { .. }
            | Command::ContentHistory { .. }
            | Command::ContentRevision { .. } => Self::Read,
            Command::CreateWork(_) => Self::CreateWork,
            Command::PatchWork { .. } => Self::SaveWork,
            Command::ArchiveWork { .. } => Self::ArchiveWork,
            Command::CreateUnit { .. } => Self::CreateUnit,
            Command::PatchUnit { .. } => Self::SaveUnit,
            Command::ArchiveUnit { .. } => Self::ArchiveUnit,
            Command::Reorder { .. } => Self::Reorder,
            Command::AppendContent { .. } => Self::SaveContent,
            _ => Self::Other,
        }
    }
}

impl Operation {
    /// Success wording is specific to the task; reads and foreign commands keep the generic one.
    pub fn done_key(self) -> Key {
        match self {
            Self::CreateWork => Key::DoneCreateWork,
            Self::SaveWork => Key::DoneSaveWork,
            Self::ArchiveWork => Key::DoneArchiveWork,
            Self::CreateUnit => Key::DoneCreateUnit,
            Self::SaveUnit => Key::DoneSaveUnit,
            Self::ArchiveUnit => Key::DoneArchiveUnit,
            Self::Reorder => Key::DoneReorder,
            Self::SaveContent => Key::DoneSaveContent,
            Self::Read | Self::Other => Key::Ready,
        }
    }

    /// The task's own name, shown with a failed or unknown outcome so it names what is at risk.
    pub fn label_key(self) -> Option<Key> {
        match self {
            Self::CreateWork => Some(Key::CreateWork),
            Self::SaveWork | Self::SaveUnit => Some(Key::SaveMetadata),
            Self::ArchiveWork => Some(Key::ArchiveWork),
            Self::CreateUnit => Some(Key::CreateUnit),
            Self::ArchiveUnit => Some(Key::ArchiveUnit),
            Self::Reorder => Some(Key::OrderZone),
            Self::SaveContent => Some(Key::SaveContent),
            Self::Read | Self::Other => None,
        }
    }
}

/// The entity a command acts on; a create targets the entity it introduces.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Target {
    None,
    Work(String),
    Unit(String),
}

impl Target {
    pub fn of(command: &Command) -> Self {
        match command {
            Command::CreateWork(request) => Self::Work(request.id.clone()),
            Command::Work { id: work }
            | Command::PatchWork { work, .. }
            | Command::ArchiveWork { work, .. }
            | Command::Units { work, .. }
            | Command::Reorder { work, .. } => Self::Work(work.clone()),
            Command::CreateUnit { request, .. } => Self::Unit(request.id.clone()),
            Command::PatchUnit { unit, .. }
            | Command::ArchiveUnit { unit, .. }
            | Command::AppendContent { unit, .. }
            | Command::ContentHead { unit }
            | Command::ContentHistory { unit, .. }
            | Command::ContentRevision { unit, .. } => Self::Unit(unit.clone()),
            _ => Self::None,
        }
    }

    /// Whether the feedback is about what the person is looking at right now.
    pub fn is_selected(&self, work: Option<&str>, unit: Option<&str>) -> bool {
        match self {
            Self::None => true,
            Self::Work(id) => work == Some(id.as_str()),
            Self::Unit(id) => unit == Some(id.as_str()),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Idle,
    Working,
    /// A valid server acknowledgement for exactly this operation.
    Done,
    /// The head read found no saved content: an empty chapter, not a failure.
    MissingHead,
    Failed,
    /// The write may or may not have happened; only an exact retry or read settles it.
    Unknown,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Feedback {
    pub operation: Operation,
    pub target: Target,
    pub outcome: Outcome,
}

/// A head read answered "not found" for the chapter: nothing has been saved yet.
pub fn is_missing_head(command: &Command, error: &ClientError) -> bool {
    matches!(command, Command::ContentHead { .. })
        && matches!(
            error,
            ClientError::Http { status: 404, error } if error.code == ErrorCode::NotFound
        )
}

pub fn feedback(state: &RequestState) -> Feedback {
    let command = state.command();
    let outcome = match state.phase() {
        Phase::Idle => Outcome::Idle,
        Phase::Pending => Outcome::Working,
        Phase::Ready => Outcome::Done,
        Phase::Ambiguous => Outcome::Unknown,
        Phase::Failed => match (command, state.error()) {
            (Some(command), Some(error)) if is_missing_head(command, error) => Outcome::MissingHead,
            _ => Outcome::Failed,
        },
    };
    Feedback {
        operation: command.map_or(Operation::Other, Operation::of),
        target: command.map_or(Target::None, Target::of),
        outcome,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ContentRevisionResponse, PatchContentUnitRequest, UnitPatchDto};

    const UNIT: &str = "10000000-0000-4000-8000-000000000001";
    const OTHER: &str = "10000000-0000-4000-8000-000000000002";

    fn state() -> RequestState {
        RequestState::default()
            .context("owner", "workspace:1")
            .unwrap()
    }
    fn revision() -> ContentRevisionResponse {
        serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/content/v1/fixtures/text-revision.json"
        ))
        .unwrap()
    }
    fn patch(unit: &str) -> Command {
        Command::PatchUnit {
            unit: unit.into(),
            request: PatchContentUnitRequest {
                operation_id: "op".into(),
                expected_revision: 1,
                expected_work_revision: 1,
                details: UnitPatchDto::default(),
            },
        }
    }

    #[test]
    fn an_acknowledged_read_reports_its_own_operation_and_target() {
        let content = revision();
        let command = Command::ContentRevision {
            unit: content.unit_id.clone(),
            revision: content.id.clone(),
        };
        let (sending, ticket) = state().begin(command).unwrap();
        let done = sending.finish(ticket, Ok(super::super::Reply::Content(content.clone())));
        assert_eq!(
            feedback(&done),
            Feedback {
                operation: Operation::Read,
                target: Target::Unit(content.unit_id),
                outcome: Outcome::Done,
            }
        );
    }

    #[test]
    fn a_wrong_acknowledgement_is_a_failure_of_that_operation_never_a_success() {
        let content = revision();
        let command = Command::ContentHead { unit: OTHER.into() };
        let (sending, ticket) = state().begin(command).unwrap();
        let failed = sending.finish(ticket, Ok(super::super::Reply::Content(content)));
        let report = feedback(&failed);
        assert_eq!(report.outcome, Outcome::Failed);
        assert_eq!(report.target, Target::Unit(OTHER.into()));
    }

    #[test]
    fn missing_head_is_neither_a_load_failure_nor_an_existing_revision() {
        let missing = ClientError::http(
            404,
            br#"{"code":"not_found","current_revision":null,"issues":[]}"#,
        );
        let head = Command::ContentHead { unit: UNIT.into() };
        let (sending, ticket) = state().begin(head.clone()).unwrap();
        let settled = sending.finish(ticket, Err(missing.clone()));
        assert_eq!(feedback(&settled).outcome, Outcome::MissingHead);
        // The same answer to any other read, or an unavailable server, stays a failure.
        let (sending, ticket) = state().begin(Command::Work { id: UNIT.into() }).unwrap();
        assert_eq!(
            feedback(&sending.finish(ticket, Err(missing))).outcome,
            Outcome::Failed
        );
        let (sending, ticket) = state().begin(head).unwrap();
        let unavailable = ClientError::http(
            500,
            br#"{"code":"unavailable","current_revision":null,"issues":[]}"#,
        );
        assert_eq!(
            feedback(&sending.finish(ticket, Err(unavailable))).outcome,
            Outcome::Failed
        );
    }

    #[test]
    fn an_interrupted_write_is_unknown_and_stays_bound_to_its_target() {
        let (sending, ticket) = state().begin(patch(UNIT)).unwrap();
        assert_eq!(feedback(&sending).outcome, Outcome::Working);
        let unknown = sending.finish(ticket, Err(ClientError::Transport));
        assert_eq!(
            feedback(&unknown),
            Feedback {
                operation: Operation::SaveUnit,
                target: Target::Unit(UNIT.into()),
                outcome: Outcome::Unknown,
            }
        );
    }

    #[test]
    fn a_conflict_is_a_failure_of_the_write_that_was_refused() {
        let conflict = ClientError::http(
            409,
            br#"{"code":"stale_revision","current_revision":8,"issues":[]}"#,
        );
        let (sending, ticket) = state().begin(patch(UNIT)).unwrap();
        let failed = sending.finish(ticket, Err(conflict));
        assert_eq!(feedback(&failed).outcome, Outcome::Failed);
        assert_eq!(feedback(&failed).operation, Operation::SaveUnit);
    }

    #[test]
    fn every_write_has_its_own_success_wording_and_reads_do_not() {
        let writes = [
            Operation::CreateWork,
            Operation::SaveWork,
            Operation::ArchiveWork,
            Operation::CreateUnit,
            Operation::SaveUnit,
            Operation::ArchiveUnit,
            Operation::Reorder,
            Operation::SaveContent,
        ];
        let keys = writes.map(Operation::done_key);
        assert!(!keys.contains(&Key::Ready));
        for (index, key) in keys.iter().enumerate() {
            assert!(!keys[..index].contains(key), "{key:?} is shared");
            assert!(writes[index].label_key().is_some());
        }
        assert_eq!(Operation::Read.done_key(), Key::Ready);
        assert_eq!(Operation::Other.label_key(), None);
    }

    #[test]
    fn locally_rejected_input_has_no_operation_to_claim() {
        let report = feedback(&state().input_failed());
        assert_eq!(report.outcome, Outcome::Failed);
        assert_eq!(report.operation, Operation::Other);
        assert_eq!(report.target, Target::None);
    }

    #[test]
    fn feedback_for_one_chapter_is_not_about_another_selection() {
        let target = Target::Unit(UNIT.into());
        assert!(target.is_selected(Some("work"), Some(UNIT)));
        assert!(!target.is_selected(Some("work"), Some(OTHER)));
        assert!(!target.is_selected(Some("work"), None));
        assert!(Target::Work("work".into()).is_selected(Some("work"), None));
        assert!(!Target::Work("work".into()).is_selected(Some("elsewhere"), Some(UNIT)));
        assert!(Target::None.is_selected(None, None));
    }

    #[test]
    fn exact_retry_of_an_unknown_append_survives_archive_but_a_new_append_is_refused() {
        use super::super::projection::{admit, Refusal};
        use crate::contract::{
            AppendContentRequest, CatalogContentUnit, CatalogWork, ContentBody, ContentDetails,
            UnitDetailsDto, WorkDetailsDto,
        };
        let command = Command::AppendContent {
            unit: UNIT.into(),
            request: AppendContentRequest {
                operation_id: "op".into(),
                id: "rev".into(),
                expected_revision: 0,
                expected_work_revision: 1,
                details: ContentDetails {
                    language: "vi-VN".into(),
                    body: ContentBody::Text {
                        text: "Xin chào".into(),
                        source: None,
                    },
                    rights_refs: vec![],
                    provenance_note: String::new(),
                },
            },
        };
        let (sending, ticket) = state().begin(command.clone()).unwrap();
        let unknown = sending.finish(ticket, Err(ClientError::Transport));
        let archived = CatalogWork {
            id: "work".into(),
            kind: "story".into(),
            owner_id: "owner".into(),
            details: WorkDetailsDto {
                title: "Ánh đèn".into(),
                synopsis: String::new(),
                language: "vi-VN".into(),
                visibility: "private".into(),
                rights_refs: vec![],
            },
            lifecycle: "archived".into(),
            revision: 2,
            created_at: 0,
            updated_at: 0,
        };
        let unit = CatalogContentUnit {
            id: UNIT.into(),
            work_id: "work".into(),
            kind: "chapter".into(),
            details: UnitDetailsDto {
                title: "Chương".into(),
                description: String::new(),
                source: None,
                script: None,
                content_digest: None,
            },
            position: 0,
            lifecycle: "draft".into(),
            revision: 1,
            created_at: 0,
            updated_at: 0,
        };
        // A new append is refused, so no new operation ID can be minted to dodge the guard...
        assert_eq!(
            admit(&command, Some(&archived), Some(&unit)),
            Err(Refusal::WorkArchived)
        );
        // ...while the existing intent is retried unchanged: same operation ID, same payload.
        let (retrying, _) = unknown.retry().unwrap();
        assert_eq!(
            retrying.pending().unwrap().json().unwrap(),
            command.json().unwrap()
        );
        assert!(unknown.can_retry());
    }

    #[test]
    fn a_new_command_replaces_the_previous_operations_feedback() {
        let (sending, ticket) = state().begin(patch(UNIT)).unwrap();
        let unknown = sending.finish(ticket, Err(ClientError::Transport));
        // Only the exact retry may continue it; a different fresh command is refused outright.
        assert!(unknown.begin(patch(OTHER)).is_none());
        let (retrying, _) = unknown.retry().unwrap();
        assert_eq!(feedback(&retrying).target, Target::Unit(UNIT.into()));
        assert_eq!(feedback(&retrying).outcome, Outcome::Working);
    }
}
