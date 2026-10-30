//! Read-only projections of Catalog facts: lifecycle capability and stable row identity.
//!
//! One predicate answers "may a new mutation start?" for the field/button projection and for
//! event-time admission. The Server stays the authority; this only refuses known-invalid *new*
//! intents. Retry and receipt reconciliation of an intent that already exists never pass here.
use super::{copy::Key, Command};
use crate::contract::{CatalogContentUnit, CatalogWork};

/// Why a new mutation is unavailable. Closed, so the reason shown is the reason enforced.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    NoWork,
    NoUnit,
    WorkArchived,
    UnitArchived,
    /// The command names an entity other than the current selection.
    OtherTarget,
}

impl Refusal {
    /// The specific lifecycle sentence; a missing selection or foreign target has no lifecycle
    /// reason to give and keeps the generic control wording.
    pub fn reason_key(self) -> Option<Key> {
        match self {
            Self::WorkArchived => Some(Key::ReasonWorkArchived),
            Self::UnitArchived => Some(Key::ReasonUnitArchived),
            Self::NoWork | Self::NoUnit | Self::OtherTarget => None,
        }
    }
}

const DRAFT: &str = "draft";

/// A Work accepts new child, metadata, archive and reorder mutations only while it is a draft.
pub fn edit_work(work: Option<&CatalogWork>) -> Result<(), Refusal> {
    match work {
        None => Err(Refusal::NoWork),
        Some(work) if work.lifecycle != DRAFT => Err(Refusal::WorkArchived),
        Some(_) => Ok(()),
    }
}

/// A chapter / episode accepts new metadata, content and archive mutations only while both it
/// and its Work are drafts. Reading the head, history and revisions is never gated here.
pub fn edit_unit(
    work: Option<&CatalogWork>,
    unit: Option<&CatalogContentUnit>,
) -> Result<(), Refusal> {
    edit_work(work)?;
    match unit {
        None => Err(Refusal::NoUnit),
        Some(unit) if unit.lifecycle != DRAFT => Err(Refusal::UnitArchived),
        Some(_) => Ok(()),
    }
}

/// Reason the selected chapter's editing is unavailable, `None` while it is open (or nothing is
/// selected, which is a different state and not a lifecycle reason).
pub fn lifecycle_reason(
    work: Option<&CatalogWork>,
    unit: Option<&CatalogContentUnit>,
) -> Option<Refusal> {
    match (edit_work(work), edit_unit(work, unit)) {
        (Err(reason @ Refusal::WorkArchived), _) | (_, Err(reason @ Refusal::UnitArchived)) => {
            Some(reason)
        }
        _ => None,
    }
}

/// Admission of a *new* command against the current selection. Commands without a lifecycle
/// rule here (reads, audio, publication) are admitted; the Server still decides everything else.
pub fn admit(
    command: &Command,
    work: Option<&CatalogWork>,
    unit: Option<&CatalogContentUnit>,
) -> Result<(), Refusal> {
    let work_matches = |id: &str| work.is_some_and(|work| work.id == id);
    let unit_matches = |id: &str| unit.is_some_and(|unit| unit.id == id);
    match command {
        Command::CreateUnit { work: id, .. }
        | Command::PatchWork { work: id, .. }
        | Command::ArchiveWork { work: id, .. }
        | Command::Reorder { work: id, .. } => {
            edit_work(work)?;
            if work_matches(id) {
                Ok(())
            } else {
                Err(Refusal::OtherTarget)
            }
        }
        Command::PatchUnit { unit: id, .. }
        | Command::ArchiveUnit { unit: id, .. }
        | Command::AppendContent { unit: id, .. } => {
            edit_unit(work, unit)?;
            if unit_matches(id) {
                Ok(())
            } else {
                Err(Refusal::OtherTarget)
            }
        }
        // A read, so an archived Work is still asked (the Server answers it); but only for the
        // Work on screen, never another one.
        Command::Preview { work: id, .. }
        | Command::MetadataArtwork { work: id, .. }
        | Command::MetadataState { work: id }
        | Command::MetadataCommand { work: id, .. } => {
            if work_matches(id) {
                Ok(())
            } else {
                Err(Refusal::OtherTarget)
            }
        }
        _ => Ok(()),
    }
}

/// Stable, unique row keys in display order. A repeated identity keeps its first position so a
/// keyed list can never mount two rows for one entity; the key is never an index or a revision.
pub fn row_ids<'a>(ids: impl IntoIterator<Item = &'a str>) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    ids.into_iter()
        .filter(|id| seen.insert(*id))
        .map(str::to_owned)
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{
        AppendContentRequest, ArchiveContentUnitRequest, ContentBody, ContentDetails,
        CreateContentUnitRequest, PatchContentUnitRequest, UnitDetailsDto, UnitPatchDto,
        WorkDetailsDto,
    };

    fn work(id: &str, lifecycle: &str) -> CatalogWork {
        CatalogWork {
            id: id.into(),
            kind: "story".into(),
            owner_id: "owner".into(),
            details: WorkDetailsDto {
                title: "Ánh đèn cuối sân khấu".into(),
                synopsis: String::new(),
                language: "vi-VN".into(),
                visibility: "private".into(),
                rights_refs: vec![],
            },
            lifecycle: lifecycle.into(),
            revision: 1,
            created_at: 0,
            updated_at: 0,
        }
    }
    fn unit(id: &str, lifecycle: &str) -> CatalogContentUnit {
        CatalogContentUnit {
            id: id.into(),
            work_id: "work".into(),
            kind: "chapter".into(),
            details: UnitDetailsDto {
                title: "Chương một".into(),
                description: String::new(),
                source: None,
                script: None,
                content_digest: None,
            },
            position: 0,
            lifecycle: lifecycle.into(),
            revision: 1,
            created_at: 0,
            updated_at: 0,
        }
    }
    fn create_unit(work_id: &str) -> Command {
        Command::CreateUnit {
            work: work_id.into(),
            request: CreateContentUnitRequest {
                operation_id: "op".into(),
                id: "new".into(),
                kind: "chapter".into(),
                expected_work_revision: 1,
                details: UnitDetailsDto {
                    title: "Mới".into(),
                    description: String::new(),
                    source: None,
                    script: None,
                    content_digest: None,
                },
            },
        }
    }
    fn patch_unit(unit_id: &str) -> Command {
        Command::PatchUnit {
            unit: unit_id.into(),
            request: PatchContentUnitRequest {
                operation_id: "op".into(),
                expected_revision: 1,
                expected_work_revision: 1,
                details: UnitPatchDto::default(),
            },
        }
    }
    fn append(unit_id: &str) -> Command {
        Command::AppendContent {
            unit: unit_id.into(),
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
        }
    }

    #[test]
    fn archived_work_refuses_create_patch_and_append_with_the_work_reason() {
        let archived = work("work", "archived");
        let open_unit = unit("unit", "draft");
        for command in [create_unit("work"), patch_unit("unit"), append("unit")] {
            assert_eq!(
                admit(&command, Some(&archived), Some(&open_unit)),
                Err(Refusal::WorkArchived)
            );
        }
        assert_eq!(
            lifecycle_reason(Some(&archived), Some(&open_unit)),
            Some(Refusal::WorkArchived)
        );
    }

    #[test]
    fn archived_unit_refuses_patch_and_append_but_not_a_sibling_create() {
        let open_work = work("work", "draft");
        let archived = unit("unit", "archived");
        for command in [patch_unit("unit"), append("unit")] {
            assert_eq!(
                admit(&command, Some(&open_work), Some(&archived)),
                Err(Refusal::UnitArchived)
            );
        }
        assert_eq!(
            admit(&create_unit("work"), Some(&open_work), Some(&archived)),
            Ok(())
        );
        assert_eq!(
            lifecycle_reason(Some(&open_work), Some(&archived)),
            Some(Refusal::UnitArchived)
        );
    }

    #[test]
    fn unknown_lifecycle_is_not_writable() {
        assert_eq!(
            edit_unit(Some(&work("work", "draft")), Some(&unit("unit", "future"))),
            Err(Refusal::UnitArchived)
        );
    }

    #[test]
    fn nothing_selected_is_a_missing_selection_not_a_lifecycle_reason() {
        assert_eq!(edit_work(None), Err(Refusal::NoWork));
        assert_eq!(
            edit_unit(Some(&work("work", "draft")), None),
            Err(Refusal::NoUnit)
        );
        assert_eq!(lifecycle_reason(None, None), None);
        assert_eq!(lifecycle_reason(Some(&work("work", "draft")), None), None);
    }

    #[test]
    fn a_command_for_another_entity_is_never_admitted_by_the_current_selection() {
        let open_work = work("work", "draft");
        let open_unit = unit("unit", "draft");
        assert_eq!(
            admit(&patch_unit("unit"), Some(&open_work), Some(&open_unit)),
            Ok(())
        );
        assert_eq!(
            admit(&patch_unit("other"), Some(&open_work), Some(&open_unit)),
            Err(Refusal::OtherTarget)
        );
        assert_eq!(
            admit(&create_unit("elsewhere"), Some(&open_work), None),
            Err(Refusal::OtherTarget)
        );
    }

    #[test]
    fn archive_of_an_archived_unit_is_refused_and_reads_are_never_gated() {
        let archived_work = work("work", "archived");
        let archived_unit = unit("unit", "archived");
        let archive = Command::ArchiveUnit {
            unit: "unit".into(),
            request: ArchiveContentUnitRequest {
                operation_id: "op".into(),
                expected_revision: 1,
                expected_work_revision: 1,
            },
        };
        assert!(admit(&archive, Some(&archived_work), Some(&archived_unit)).is_err());
        for read in [
            Command::ContentHead {
                unit: "unit".into(),
            },
            Command::ContentHistory {
                unit: "unit".into(),
                after: 0,
            },
            Command::Units {
                work: "work".into(),
                cursor: None,
            },
        ] {
            assert_eq!(
                admit(&read, Some(&archived_work), Some(&archived_unit)),
                Ok(())
            );
        }
        let preview = |work: &str| Command::Preview {
            work: work.into(),
            work_revision: 1,
            cursor: None,
        };
        assert_eq!(
            admit(&preview(&archived_work.id), Some(&archived_work), None),
            Ok(())
        );
        assert_eq!(
            admit(&preview("another"), Some(&archived_work), None),
            Err(Refusal::OtherTarget)
        );
        assert_eq!(
            admit(&preview(&archived_work.id), None, None),
            Err(Refusal::OtherTarget)
        );
    }

    #[test]
    fn only_lifecycle_refusals_carry_a_specific_reason() {
        assert_eq!(
            Refusal::WorkArchived.reason_key(),
            Some(Key::ReasonWorkArchived)
        );
        assert_eq!(
            Refusal::UnitArchived.reason_key(),
            Some(Key::ReasonUnitArchived)
        );
        for other in [Refusal::NoWork, Refusal::NoUnit, Refusal::OtherTarget] {
            assert_eq!(other.reason_key(), None);
        }
    }

    #[test]
    fn row_keys_are_identities_in_order_without_duplicates() {
        assert_eq!(row_ids(["b", "a", "b", "c", "a"]), vec!["b", "a", "c"]);
        assert!(row_ids(std::iter::empty::<&str>()).is_empty());
    }
}
