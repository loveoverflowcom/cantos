//! Anonymous discovery request ownership; identity lookup never gates public content.
use cantos_contracts::theatre::WorkPage;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PublicDiscoveryFailure {
    Unavailable,
    Protocol,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum PublicDiscoveryPhase {
    #[default]
    Idle,
    Loading,
    Empty,
    Ready(WorkPage),
    Failed(PublicDiscoveryFailure),
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicDiscoveryState {
    generation: u64,
    phase: PublicDiscoveryPhase,
}

impl PublicDiscoveryState {
    pub fn phase(&self) -> &PublicDiscoveryPhase {
        &self.phase
    }

    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// A mounted request owns one terminal response. Repeated activation while loading is inert.
    pub fn begin_refresh(&mut self) -> Option<u64> {
        if self.phase == PublicDiscoveryPhase::Loading {
            return None;
        }
        let generation = self.generation.checked_add(1)?;
        self.generation = generation;
        self.phase = PublicDiscoveryPhase::Loading;
        Some(generation)
    }

    /// Unmount/cancel invalidates the old callback without resetting another surface's state.
    pub fn cancel(&mut self) -> bool {
        let Some(generation) = self.generation.checked_add(1) else {
            return false;
        };
        self.generation = generation;
        self.phase = PublicDiscoveryPhase::Idle;
        true
    }

    pub fn finish(
        &mut self,
        issued: u64,
        result: Result<WorkPage, PublicDiscoveryFailure>,
    ) -> bool {
        if issued != self.generation || self.phase != PublicDiscoveryPhase::Loading {
            return false;
        }
        self.phase = match result {
            Ok(page) if page.items.is_empty() => PublicDiscoveryPhase::Empty,
            Ok(page) => PublicDiscoveryPhase::Ready(page),
            Err(failure) => PublicDiscoveryPhase::Failed(failure),
        };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cantos_contracts::theatre::{WorkKind, WorkSummary};

    fn page(title: &str) -> WorkPage {
        WorkPage {
            items: vec![WorkSummary {
                work_id: "00000000-0000-0000-0000-000000000001".into(),
                kind: WorkKind::Story,
                title: title.into(),
                description: "Nội dung đã được công bố.".into(),
                language: "vi-VN".into(),
                first_published_at_ms: 1,
            }],
            next_cursor: None,
        }
    }

    #[test]
    fn duplicate_read_is_blocked_and_late_results_cannot_reenter_after_cancel() {
        let mut state = PublicDiscoveryState::default();
        let first = state.begin_refresh().unwrap();
        assert_eq!(state.begin_refresh(), None);
        assert!(state.cancel());
        assert!(!state.finish(first, Ok(page("Cũ"))));
        let current = state.begin_refresh().unwrap();
        assert!(!state.finish(first, Err(PublicDiscoveryFailure::Unavailable)));
        assert!(state.finish(current, Ok(page("Hiện tại"))));
        assert!(!state.finish(current, Err(PublicDiscoveryFailure::Protocol)));
        assert_eq!(
            state.phase(),
            &PublicDiscoveryPhase::Ready(page("Hiện tại"))
        );
    }

    #[test]
    fn empty_is_only_an_actual_empty_success_and_failure_remains_retryable() {
        let mut state = PublicDiscoveryState::default();
        let issued = state.begin_refresh().unwrap();
        assert!(state.finish(issued, Err(PublicDiscoveryFailure::Unavailable)));
        assert_eq!(
            state.phase(),
            &PublicDiscoveryPhase::Failed(PublicDiscoveryFailure::Unavailable)
        );
        let retry = state.begin_refresh().unwrap();
        assert!(state.finish(
            retry,
            Ok(WorkPage {
                items: vec![],
                next_cursor: None
            })
        ));
        assert_eq!(state.phase(), &PublicDiscoveryPhase::Empty);
    }

    #[test]
    fn generation_exhaustion_preserves_the_last_state_without_reusing_identity() {
        let mut state = PublicDiscoveryState {
            generation: u64::MAX,
            phase: PublicDiscoveryPhase::Ready(page("Đang hiển thị")),
        };
        let previous = state.clone();
        assert_eq!(state.begin_refresh(), None);
        assert!(!state.cancel());
        assert_eq!(state, previous);
    }
}
