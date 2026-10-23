//! App-local read navigation remembers acknowledged page starts, never pending requests.
use super::{Command, Reply};

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct ReadPages {
    works: Vec<Option<String>>,
    units: Vec<Option<String>>,
    history: Vec<u64>,
}

impl ReadPages {
    pub fn previous_works(&self) -> Option<Option<String>> {
        previous(&self.works)
    }

    pub fn previous_units(&self) -> Option<Option<String>> {
        previous(&self.units)
    }

    pub fn previous_history(&self) -> Option<u64> {
        previous(&self.history)
    }

    /// Work selection keeps the owner's Work page but forgets every dependent page.
    pub fn for_work(&self) -> Self {
        Self {
            works: self.works.clone(),
            ..Self::default()
        }
    }

    pub fn clear_history(&mut self) {
        self.history.clear();
    }

    pub fn acknowledge(&mut self, command: &Command, reply: &Reply) {
        match (command, reply) {
            (Command::Works { cursor }, Reply::Works(_)) => acknowledge(&mut self.works, cursor),
            (Command::Units { cursor, .. }, Reply::Units(_)) => {
                acknowledge(&mut self.units, cursor)
            }
            (Command::ContentHistory { after, .. }, Reply::History(_)) => {
                acknowledge(&mut self.history, after);
            }
            _ => {}
        }
    }
}

fn previous<T: Clone>(pages: &[T]) -> Option<T> {
    pages.len().checked_sub(2).map(|index| pages[index].clone())
}

fn acknowledge<T: Clone + Default + PartialEq>(pages: &mut Vec<T>, cursor: &T) {
    if *cursor == T::default() {
        pages.clear();
    } else if let Some(index) = pages.iter().position(|seen| seen == cursor) {
        pages.truncate(index + 1);
        return;
    }
    pages.push(cursor.clone());
}

/// Automatic reads and unrelated controls never request a focus move.
#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn focus_target(command: &Command, source: &str) -> Option<&'static str> {
    match (command, source) {
        (Command::Works { .. }, "dashboard-refresh" | "dashboard-previous" | "dashboard-next") => {
            Some("dashboard-works-title")
        }
        (Command::Works { .. }, "live-works" | "live-works-previous" | "live-works-next") => {
            Some("live-works-list")
        }
        (
            Command::Units { .. },
            "live-refresh-units" | "live-units-previous" | "live-units-next",
        ) => Some("live-units-list"),
        (
            Command::ContentHistory { .. },
            "live-content-history" | "live-history-previous" | "live-history-next",
        ) => Some("live-history-list"),
        _ => None,
    }
}

#[cfg(any(target_arch = "wasm32", test))]
pub(super) fn outcome_focus_target(results: &'static str, succeeded: bool) -> &'static str {
    if succeeded {
        results
    } else if results == "dashboard-works-title" {
        "dashboard-status"
    } else {
        "live-status"
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        contract::{CatalogPage, ContentHistoryResponse, ProductionBriefDocument},
        live::{drafts::AuthoringDrafts, ClientError, Facts, RequestState},
    };

    fn history() -> Reply {
        Reply::History(ContentHistoryResponse {
            revisions: vec![],
            next_after: None,
        })
    }

    #[test]
    fn previous_reuses_each_acknowledged_history_cursor_and_refresh_forgets_the_trail() {
        let mut pages = ReadPages::default();
        assert_eq!(pages.previous_history(), None);
        for (after, previous) in [(0, None), (20, Some(0)), (40, Some(20)), (20, Some(0))] {
            pages.acknowledge(
                &Command::ContentHistory {
                    unit: "unit".into(),
                    after,
                },
                &history(),
            );
            assert_eq!(pages.previous_history(), previous);
        }
        pages.acknowledge(
            &Command::ContentHistory {
                unit: "unit".into(),
                after: 60,
            },
            &history(),
        );
        assert_eq!(pages.previous_history(), Some(20));
        pages.acknowledge(
            &Command::ContentHistory {
                unit: "unit".into(),
                after: 0,
            },
            &history(),
        );
        assert_eq!(pages.previous_history(), None);
    }

    #[test]
    fn work_and_unit_trails_keep_opaque_cursors_and_reset_only_their_scope() {
        let mut pages = ReadPages::default();
        for cursor in [None, Some("abcd".into()), Some("ef01".into())] {
            pages.acknowledge(
                &Command::Works {
                    cursor: cursor.clone(),
                },
                &Reply::Works(CatalogPage {
                    items: vec![],
                    next_cursor: None,
                }),
            );
            pages.acknowledge(
                &Command::Units {
                    work: "work".into(),
                    cursor,
                },
                &Reply::Units(CatalogPage {
                    items: vec![],
                    next_cursor: None,
                }),
            );
        }
        pages.acknowledge(
            &Command::ContentHistory {
                unit: "unit".into(),
                after: 20,
            },
            &history(),
        );
        assert_eq!(pages.previous_works(), Some(Some("abcd".into())));
        assert_eq!(pages.previous_units(), Some(Some("abcd".into())));
        pages.clear_history();
        assert_eq!(pages.previous_history(), None);
        assert_eq!(pages.previous_units(), Some(Some("abcd".into())));
        let work = pages.for_work();
        assert_eq!(work.previous_units(), None);
        assert_eq!(work.previous_works(), Some(Some("abcd".into())));
        assert_eq!(ReadPages::default().previous_works(), None);
    }

    #[test]
    fn unrelated_or_mismatched_reads_cannot_advance_page_navigation() {
        let mut pages = ReadPages::default();
        pages.acknowledge(
            &Command::Works {
                cursor: Some("abcd".into()),
            },
            &history(),
        );
        pages.acknowledge(
            &Command::ContentHead {
                unit: "unit".into(),
            },
            &history(),
        );
        assert_eq!(pages, ReadPages::default());
    }

    #[test]
    fn failed_cancelled_or_late_history_reads_keep_the_acknowledged_page() {
        let first = Command::ContentHistory {
            unit: "unit".into(),
            after: 0,
        };
        let next = Command::ContentHistory {
            unit: "unit".into(),
            after: 20,
        };
        let facts = Facts::default().apply_command(&first, history());
        let state = RequestState::default()
            .context("owner", "workspace:1")
            .unwrap();
        let (pending, ticket) = state.begin(next.clone()).unwrap();
        assert!(!pending.settled(ticket));
        assert_eq!(facts.pages.previous_history(), None);
        let failed = pending.finish(ticket, Err(ClientError::Transport));
        assert!(failed.settled(ticket));
        assert!(!failed
            .context("other-owner", "workspace:2")
            .unwrap()
            .settled(ticket));
        assert!(!failed.begin(next.clone()).unwrap().0.settled(ticket));
        assert_eq!(failed.completion(), Some(Err(&ClientError::Transport)));
        let after_failure = match failed.completion() {
            Some(Ok(reply)) => facts.apply_command(&next, reply.clone()),
            _ => facts.clone(),
        };
        assert_eq!(after_failure.pages, facts.pages);
        assert_eq!(after_failure.history, facts.history);
        let cancelled = pending.cancel().unwrap();
        assert!(!cancelled.accepts(ticket));
        assert!(!cancelled.settled(ticket));
        assert!(cancelled.finish(ticket, Ok(history())).result().is_none());
        let switched = pending.context("other-owner", "workspace:2").unwrap();
        assert!(!switched.accepts(ticket));
        assert!(switched.finish(ticket, Ok(history())).result().is_none());
        let accepted = pending.finish(ticket, Ok(history()));
        assert!(accepted.settled(ticket));
        assert!(!accepted
            .context("other-owner", "workspace:2")
            .unwrap()
            .settled(ticket));
        assert!(!accepted.begin(first).unwrap().0.settled(ticket));
        let after_success = facts.apply_command(&next, accepted.result().unwrap().clone());
        assert_eq!(after_success.pages.previous_history(), Some(0));
    }

    #[test]
    fn history_navigation_never_replaces_current_head_save_base_or_unsaved_content() {
        let brief: ProductionBriefDocument = serde_json::from_str(include_str!(
            "../../../../../services/cantos-server/contracts/studio/briefs/v1/fixtures/narration-document.json"
        )).unwrap();
        let mut drafts = AuthoringDrafts::default();
        drafts.head(&brief.content, None);
        let mut draft = drafts.content(&brief.content.unit_id);
        draft.text = "Bản nháp chưa lưu — giữ nguyên.".into();
        drafts.edit_content(&brief.content.unit_id, draft.clone());
        let original = Facts {
            content: Some(brief.content.clone()),
            content_head: Some(brief.content.clone()),
            content_base: Some(brief.content.revision),
            brief: Some(crate::contract::ProductionBriefResponse {
                brief: brief.clone(),
                freshness: crate::contract::BriefFreshness::Current,
            }),
            ..Facts::default()
        };
        let mut facts = original.clone();
        for after in [0, 20, 40, 20, 0] {
            let command = Command::ContentHistory {
                unit: brief.content.unit_id.clone(),
                after,
            };
            let reply = history();
            drafts.acknowledge(&command, &reply);
            facts = facts.apply_command(&command, reply);
            assert_eq!(facts.content, original.content);
            assert_eq!(facts.content_head, original.content_head);
            assert_eq!(facts.content_base, original.content_base);
            assert_eq!(facts.brief, original.brief);
            assert_eq!(drafts.content(&brief.content.unit_id), draft);
            assert_eq!(
                drafts.content_base(&brief.content.unit_id),
                original.content_base
            );
            assert!(drafts.content_dirty(&brief.content.unit_id));
        }
    }

    #[test]
    fn focus_recovery_maps_only_the_control_that_started_a_matching_page_read() {
        let commands = [
            (
                Command::Works { cursor: None },
                ["dashboard-refresh", "dashboard-previous", "dashboard-next"],
                "dashboard-works-title",
            ),
            (
                Command::Works { cursor: None },
                ["live-works", "live-works-previous", "live-works-next"],
                "live-works-list",
            ),
            (
                Command::Units {
                    work: "work".into(),
                    cursor: None,
                },
                [
                    "live-refresh-units",
                    "live-units-previous",
                    "live-units-next",
                ],
                "live-units-list",
            ),
            (
                Command::ContentHistory {
                    unit: "unit".into(),
                    after: 0,
                },
                [
                    "live-content-history",
                    "live-history-previous",
                    "live-history-next",
                ],
                "live-history-list",
            ),
        ];
        for (command, sources, target) in commands {
            for source in sources {
                assert_eq!(focus_target(&command, source), Some(target));
                assert_eq!(outcome_focus_target(target, true), target);
                assert_eq!(
                    outcome_focus_target(target, false),
                    if target == "dashboard-works-title" {
                        "dashboard-status"
                    } else {
                        "live-status"
                    }
                );
                assert_eq!(
                    focus_target(
                        &Command::ContentHead {
                            unit: "unit".into()
                        },
                        source
                    ),
                    None
                );
            }
            for source in ["", "live-manuscript", "live-retry", "dashboard-continue"] {
                assert_eq!(focus_target(&command, source), None);
            }
        }
        assert_eq!(
            focus_target(&Command::Works { cursor: None }, "live-history-next"),
            None
        );
    }
}
