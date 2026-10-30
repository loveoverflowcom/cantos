//! Exact saved-revision preview (ADR 0054 D03, #650): pure facts and flow decisions.
//!
//! The Server answers only for the Work revision the request names, so every fact here names
//! its own revision and the banner reads it from the fact, never from a draft, a list or a
//! newer read. Nothing here publishes, reads a public copy or fills a missing asset.
use super::{ClientError, Command, Reply};
use crate::contract::{CatalogWork, ErrorCode, PreviewUnit, PreviewWork, StudioPreview};

/// One Work revision's presentation text, built page by page from replies that each named it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreviewFacts {
    work: PreviewWork,
    units: Vec<PreviewUnit>,
    next: Option<String>,
}

impl PreviewFacts {
    pub fn first(page: StudioPreview) -> Self {
        Self {
            work: page.work,
            units: page.units,
            next: page.next_cursor,
        }
    }

    /// A continuation joins only a page of the same Work revision whose ranks continue the walk
    /// exactly. Anything else is refused so two revisions are never shown as one.
    pub fn extend(&self, page: StudioPreview) -> Option<Self> {
        let expected = self.units.last().map_or(1, |unit| unit.ordinal + 1);
        let contiguous = page
            .units
            .iter()
            .zip(expected..)
            .all(|(unit, ordinal)| unit.ordinal == ordinal);
        if page.work != self.work || !contiguous || self.next.is_none() {
            return None;
        }
        let mut units = self.units.clone();
        units.extend(page.units);
        Some(Self {
            work: self.work.clone(),
            units,
            next: page.next_cursor,
        })
    }

    pub fn work(&self) -> &PreviewWork {
        &self.work
    }

    pub fn units(&self) -> &[PreviewUnit] {
        &self.units
    }

    pub fn next(&self) -> Option<&str> {
        self.next.as_deref()
    }

    /// True while the Studio's saved head has not moved past what is shown. A different Work, or
    /// a head newer than this revision, makes the text history, which must not stay on screen.
    pub fn current_for(&self, saved: &CatalogWork) -> bool {
        self.work.id == saved.id && self.work.revision >= saved.revision
    }
}

/// The preview command for the saved head the Studio holds, if it holds one.
pub fn preview_saved(saved: Option<&CatalogWork>) -> Option<Command> {
    let saved = saved?;
    Some(Command::Preview {
        work: saved.id.clone(),
        work_revision: saved.revision,
        cursor: None,
    })
}

/// The continuation page of the revision already shown. It names that revision again, so a head
/// that moved meanwhile answers stale instead of extending the old text.
pub fn preview_more(shown: &PreviewFacts) -> Option<Command> {
    Some(Command::Preview {
        work: shown.work.id.clone(),
        work_revision: shown.work.revision,
        cursor: Some(shown.next()?.to_owned()),
    })
}

/// "Save, then preview": armed with the save's own operation, it fires only for that operation's
/// acknowledged save of the same Work. A failed or unknown save arms nothing; the draft stays.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AfterSave {
    pub operation_id: String,
}

impl AfterSave {
    pub fn of(command: &Command) -> Option<Self> {
        match command {
            Command::PatchWork { request, .. } => Some(Self {
                operation_id: request.operation_id.clone(),
            }),
            _ => None,
        }
    }

    /// The preview of exactly the revision the acknowledgement returned.
    pub fn follow_up(&self, settled: &Command, reply: &Reply) -> Option<Command> {
        let (Command::PatchWork { work, request, .. }, Reply::Mutation(saved)) = (settled, reply)
        else {
            return None;
        };
        (request.operation_id == self.operation_id && saved.work.id == *work).then(|| {
            Command::Preview {
                work: saved.work.id.clone(),
                work_revision: saved.work.revision,
                cursor: None,
            }
        })
    }
}

/// Why a preview did not appear. Each case keeps the creator's draft untouched.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Problem {
    /// The saved head moved: read it again, then preview that revision.
    Stale,
    /// An archived Work has no preview.
    Archived,
    /// The session or membership no longer permits a preview of this Work.
    Unavailable,
    /// The request failed or its outcome is unknown; nothing is shown.
    Failed,
}

impl Problem {
    pub fn of(error: &ClientError) -> Self {
        match error {
            ClientError::Http { status: 409, error } => match error.code {
                ErrorCode::StaleRevision | ErrorCode::StaleCursor => Self::Stale,
                ErrorCode::CatalogConflict => Self::Archived,
                _ => Self::Failed,
            },
            ClientError::Http {
                status: 401 | 403 | 404,
                ..
            } => Self::Unavailable,
            _ => Self::Failed,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{
        ApiError, CatalogMutationResponse, PatchField, PatchWorkRequest, WorkPatchDto,
    };

    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const OTHER: &str = "10000000-0000-4000-8000-000000000002";
    const OPERATION: &str = "30000000-0000-4000-8000-000000000001";

    fn work(revision: u64) -> PreviewWork {
        PreviewWork {
            id: WORK.into(),
            kind: "novel".into(),
            revision,
            title: "Ánh đèn cuối sân khấu".into(),
            synopsis: "Một lời hẹn dưới mái hiên.".into(),
            language: "vi-VN".into(),
        }
    }

    fn units(range: std::ops::RangeInclusive<u32>) -> Vec<PreviewUnit> {
        range
            .map(|ordinal| PreviewUnit {
                id: format!("20000000-0000-4000-8000-{ordinal:012}"),
                kind: "chapter".into(),
                revision: 1,
                ordinal,
                title: format!("Chương {ordinal}"),
                description: String::new(),
            })
            .collect()
    }

    fn page(
        revision: u64,
        ranks: std::ops::RangeInclusive<u32>,
        next: Option<&str>,
    ) -> StudioPreview {
        StudioPreview {
            work: work(revision),
            units: units(ranks),
            next_cursor: next.map(str::to_owned),
        }
    }

    fn saved(id: &str, revision: u64) -> CatalogWork {
        serde_json::from_value(serde_json::json!({
            "id": id, "kind": "novel", "owner_id": "alice",
            "details": {"title": "Ánh đèn", "synopsis": "", "language": "vi-VN",
                        "visibility": "private", "rights_refs": []},
            "lifecycle": "draft", "revision": revision, "created_at": 1, "updated_at": 2
        }))
        .unwrap()
    }

    fn patch(work: &str, operation: &str) -> Command {
        Command::PatchWork {
            work: work.into(),
            request: PatchWorkRequest {
                operation_id: operation.into(),
                expected_revision: 4,
                details: WorkPatchDto {
                    title: PatchField::Value("Tên mới".into()),
                    ..WorkPatchDto::default()
                },
            },
        }
    }

    fn mutation(id: &str, revision: u64) -> Reply {
        Reply::Mutation(CatalogMutationResponse {
            work: saved(id, revision),
            units: vec![],
        })
    }

    #[test]
    fn a_walk_joins_only_contiguous_pages_of_one_exact_revision() {
        let first = PreviewFacts::first(page(5, 1..=2, Some("aa")));
        let joined = first.extend(page(5, 3..=4, Some("bb"))).unwrap();
        assert_eq!(joined.units().len(), 4);
        assert_eq!(joined.work().revision, 5);
        assert_eq!(joined.next(), Some("bb"));
        // Another revision, a skipped or repeated rank, and a walk with no next page are refused.
        assert!(first.extend(page(6, 3..=4, None)).is_none());
        assert!(first.extend(page(5, 4..=5, None)).is_none());
        assert!(first.extend(page(5, 2..=3, None)).is_none());
        let done = joined.extend(page(5, 5..=5, None)).unwrap();
        assert!(done.extend(page(5, 6..=6, None)).is_none());
    }

    #[test]
    fn a_preview_stays_only_while_the_saved_head_has_not_moved_past_it() {
        let shown = PreviewFacts::first(page(5, 1..=1, None));
        assert!(shown.current_for(&saved(WORK, 5)));
        assert!(shown.current_for(&saved(WORK, 4)));
        assert!(!shown.current_for(&saved(WORK, 6)));
        assert!(!shown.current_for(&saved(OTHER, 5)));
    }

    #[test]
    fn preview_commands_name_the_saved_revision_and_continue_the_same_one() {
        assert!(preview_saved(None).is_none());
        let Some(Command::Preview {
            work,
            work_revision,
            cursor,
        }) = preview_saved(Some(&saved(WORK, 5)))
        else {
            panic!("expected a preview command");
        };
        assert_eq!((work.as_str(), work_revision, cursor), (WORK, 5, None));
        let shown = PreviewFacts::first(page(5, 1..=2, Some("aa")));
        let Some(Command::Preview {
            work_revision,
            cursor,
            ..
        }) = preview_more(&shown)
        else {
            panic!("expected a continuation");
        };
        assert_eq!((work_revision, cursor.as_deref()), (5, Some("aa")));
        assert!(preview_more(&PreviewFacts::first(page(5, 1..=1, None))).is_none());
    }

    #[test]
    fn save_then_preview_follows_only_its_own_acknowledged_save_with_the_returned_revision() {
        let command = patch(WORK, OPERATION);
        let armed = AfterSave::of(&command).unwrap();
        let Some(Command::Preview {
            work,
            work_revision,
            cursor,
        }) = armed.follow_up(&command, &mutation(WORK, 5))
        else {
            panic!("the acknowledged save previews");
        };
        assert_eq!((work.as_str(), work_revision, cursor), (WORK, 5, None));
        // Another save, another Work, a read or a failure never opens a preview.
        assert!(armed
            .follow_up(&patch(WORK, "other-operation"), &mutation(WORK, 5))
            .is_none());
        assert!(armed.follow_up(&command, &mutation(OTHER, 5)).is_none());
        assert!(armed
            .follow_up(
                &Command::Work { id: WORK.into() },
                &Reply::Work(saved(WORK, 5))
            )
            .is_none());
        assert!(AfterSave::of(&Command::Work { id: WORK.into() }).is_none());
    }

    #[test]
    fn problems_separate_a_moved_head_an_archive_a_lost_right_and_an_unknown_outcome() {
        let http = |status, code| ClientError::Http {
            status,
            error: ApiError {
                code,
                current_revision: Some(7),
                issues: vec![],
            },
        };
        assert_eq!(
            Problem::of(&http(409, ErrorCode::StaleRevision)),
            Problem::Stale
        );
        assert_eq!(
            Problem::of(&http(409, ErrorCode::StaleCursor)),
            Problem::Stale
        );
        assert_eq!(
            Problem::of(&http(409, ErrorCode::CatalogConflict)),
            Problem::Archived
        );
        for status in [401, 403, 404] {
            assert_eq!(
                Problem::of(&http(status, ErrorCode::NotFound)),
                Problem::Unavailable
            );
        }
        assert_eq!(
            Problem::of(&http(503, ErrorCode::Unavailable)),
            Problem::Failed
        );
        assert_eq!(Problem::of(&ClientError::Transport), Problem::Failed);
        assert_eq!(Problem::of(&ClientError::Cancelled), Problem::Failed);
    }

    #[test]
    fn the_client_asks_for_and_accepts_only_the_exact_revision() {
        let ask = Command::Preview {
            work: WORK.into(),
            work_revision: 5,
            cursor: Some("ab12".into()),
        };
        assert_eq!(
            ask.path().unwrap(),
            format!("/api/v1/studio/works/{WORK}/preview?work_revision=5&limit=20&cursor=ab12")
        );
        assert_eq!(ask.method(), "GET");
        assert!(!ask.is_write());
        assert!(ask
            .check_reply(&Reply::Preview(page(5, 1..=1, None)))
            .is_ok());
        // A banner for another revision or Work is a reply mismatch, never displayed.
        assert!(ask
            .check_reply(&Reply::Preview(page(6, 1..=1, None)))
            .is_err());
        let mut foreign = page(5, 1..=1, None);
        foreign.work.id = OTHER.into();
        assert!(ask.check_reply(&Reply::Preview(foreign)).is_err());
        for bad in [
            Command::Preview {
                work: WORK.into(),
                work_revision: 0,
                cursor: None,
            },
            Command::Preview {
                work: WORK.into(),
                work_revision: 1,
                cursor: Some("not hex".into()),
            },
            Command::Preview {
                work: "x".into(),
                work_revision: 1,
                cursor: None,
            },
        ] {
            assert!(bad.path().is_err());
        }
    }

    #[test]
    fn facts_keep_the_shown_text_for_its_head_extend_it_and_drop_it_when_the_head_moves() {
        use crate::live::Facts;
        let read = Command::Work { id: WORK.into() };
        let facts = Facts::default().apply_command(&read, Reply::Work(saved(WORK, 5)));
        let ask = preview_saved(facts.work.as_ref()).unwrap();
        let facts = facts.apply_command(&ask, Reply::Preview(page(5, 1..=2, Some("aa"))));
        assert_eq!(facts.preview.as_ref().unwrap().units().len(), 2);

        let more = preview_more(facts.preview.as_ref().unwrap()).unwrap();
        let facts = facts.apply_command(&more, Reply::Preview(page(5, 3..=3, None)));
        let shown = facts.preview.as_ref().unwrap();
        assert_eq!((shown.units().len(), shown.next()), (3, None));

        // A continuation of another revision is refused and the walk shown so far survives.
        let shown_before = facts.preview.clone();
        let wrong = Command::Preview {
            work: WORK.into(),
            work_revision: 5,
            cursor: Some("bb".into()),
        };
        let facts = facts.apply_command(&wrong, Reply::Preview(page(5, 9..=9, None)));
        assert_eq!(facts.preview, shown_before);

        // A saved metadata edit advances the head: the older text may not stay on screen.
        let save = patch(WORK, OPERATION);
        let saved_facts = facts.apply_command(&save, mutation(WORK, 6));
        assert!(saved_facts.preview.is_none());
        // Reading another Work, or the same head again, is judged by the same rule.
        let other = facts.apply_command(
            &Command::Work { id: OTHER.into() },
            Reply::Work(saved(OTHER, 1)),
        );
        assert!(other.preview.is_none());
        let same = facts.apply_command(&read, Reply::Work(saved(WORK, 5)));
        assert!(same.preview.is_some());
    }
}
