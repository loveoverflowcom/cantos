//! Catalog request ownership and stale-result boundary, independent of route rendering.
use super::ports::CatalogPort;
use crate::core::{
    catalog::{Episode, Work},
    ids::{EpisodeId, WorkId},
    playback::Source,
};
use leptos::prelude::*;
use std::cell::RefCell;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenario {
    #[default]
    Ready,
    Loading,
    Empty,
    Error,
}
impl Scenario {
    pub const fn value(self) -> &'static str {
        match self {
            Self::Ready => "ready",
            Self::Loading => "loading",
            Self::Empty => "empty",
            Self::Error => "error",
        }
    }
    pub fn from_value(value: &str) -> Option<Self> {
        match value {
            "ready" => Some(Self::Ready),
            "loading" => Some(Self::Loading),
            "empty" => Some(Self::Empty),
            "error" => Some(Self::Error),
            _ => None,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogRequest {
    pub generation: u32,
    pub scenario: Scenario,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogOutcome {
    Pending,
    Ready(Vec<WorkId>),
    Empty,
    Error,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogResponse {
    pub generation: u32,
    pub outcome: CatalogOutcome,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogState {
    pub generation: u32,
    pub outcome: CatalogOutcome,
}
impl Default for CatalogState {
    fn default() -> Self {
        Self {
            generation: 0,
            outcome: CatalogOutcome::Pending,
        }
    }
}
impl CatalogState {
    pub fn begin(&self, scenario: Scenario) -> Option<(Self, CatalogRequest)> {
        let generation = self.generation.checked_add(1)?;
        Some((
            Self {
                generation,
                outcome: CatalogOutcome::Pending,
            },
            CatalogRequest {
                generation,
                scenario,
            },
        ))
    }
    /// A request settles once; refresh/retry must issue a new generation.
    pub fn accept(&self, response: CatalogResponse) -> Self {
        if self.generation == 0
            || response.generation != self.generation
            || !matches!(&self.outcome, CatalogOutcome::Pending)
        {
            return self.clone();
        }
        Self {
            generation: self.generation,
            outcome: response.outcome,
        }
    }
    pub fn scenario(&self) -> Scenario {
        match &self.outcome {
            CatalogOutcome::Pending => Scenario::Loading,
            CatalogOutcome::Ready(_) => Scenario::Ready,
            CatalogOutcome::Empty => Scenario::Empty,
            CatalogOutcome::Error => Scenario::Error,
        }
    }
    pub fn ids(&self) -> Vec<WorkId> {
        match &self.outcome {
            CatalogOutcome::Ready(ids) => ids.clone(),
            CatalogOutcome::Pending | CatalogOutcome::Empty | CatalogOutcome::Error => Vec::new(),
        }
    }
}
#[derive(Clone, Copy)]
pub struct CatalogSession {
    pub state: ReadSignal<CatalogState>,
    writer: RwSignal<CatalogState>,
    adapter: StoredValue<RefCell<Box<dyn CatalogPort>>, LocalStorage>,
}
impl CatalogSession {
    pub fn new(adapter: impl CatalogPort + 'static) -> Self {
        let state = RwSignal::new(CatalogState::default());
        let session = Self {
            state: state.read_only(),
            writer: state,
            adapter: StoredValue::new_local(RefCell::new(Box::new(adapter))),
        };
        session.load(Scenario::Ready);
        session
    }
    pub fn load(self, scenario: Scenario) {
        let Some((next, request)) = self.state.get_untracked().begin(scenario) else {
            return;
        };
        self.writer.set(next);
        let response = self
            .adapter
            .with_value(|port| port.borrow_mut().fetch(request));
        self.accept(response);
    }
    pub fn accept(self, response: CatalogResponse) {
        let next = self.state.get_untracked().accept(response);
        self.writer.set(next);
    }
    pub fn scenario(self) -> Scenario {
        self.state.get().scenario()
    }
    pub fn work(self, id: WorkId) -> Work {
        self.adapter.with_value(|port| port.borrow().work(id))
    }
    pub fn episode(self, id: EpisodeId) -> Option<Episode> {
        self.work(id.work).episodes.get(id.index).copied()
    }
    pub fn source(self, id: EpisodeId) -> Option<Source> {
        let episode = self.episode(id)?;
        if !episode.available {
            return None;
        }
        Source::new(id, episode.duration.checked_mul(1000)?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    use crate::{core::catalog::Genre, infrastructure::mock_catalog::MockCatalog};
    use std::rc::Rc;

    fn outcomes() -> [CatalogOutcome; 4] {
        [
            CatalogOutcome::Pending,
            CatalogOutcome::Ready(vec![WorkId::River]),
            CatalogOutcome::Empty,
            CatalogOutcome::Error,
        ]
    }

    #[test]
    fn no_result_is_accepted_before_the_first_request_is_issued() {
        let idle = CatalogState::default();
        for outcome in outcomes() {
            assert_eq!(
                idle.accept(CatalogResponse {
                    generation: 0,
                    outcome,
                }),
                idle
            );
        }
    }

    #[test]
    fn each_new_request_clears_its_previous_result_and_advances_identity() {
        for outcome in outcomes() {
            let previous = CatalogState {
                generation: 41,
                outcome,
            };
            for scenario in [
                Scenario::Ready,
                Scenario::Loading,
                Scenario::Empty,
                Scenario::Error,
            ] {
                let (loading, request) = previous.begin(scenario).unwrap();
                assert_eq!(
                    loading,
                    CatalogState {
                        generation: 42,
                        outcome: CatalogOutcome::Pending,
                    }
                );
                assert_eq!(
                    request,
                    CatalogRequest {
                        generation: 42,
                        scenario,
                    }
                );
            }
        }
    }

    #[test]
    fn older_and_unissued_results_never_replace_pending_or_settled_catalogs() {
        for current_outcome in outcomes() {
            let current = CatalogState {
                generation: 8,
                outcome: current_outcome,
            };
            for generation in [0, 7, 9, u32::MAX] {
                for outcome in outcomes() {
                    assert_eq!(
                        current.accept(CatalogResponse {
                            generation,
                            outcome,
                        }),
                        current
                    );
                }
            }
        }
    }

    #[test]
    fn first_terminal_result_settles_a_request_until_a_new_request_begins() {
        let loading = CatalogState {
            generation: 7,
            outcome: CatalogOutcome::Pending,
        };
        for outcome in [
            CatalogOutcome::Ready(vec![WorkId::River]),
            CatalogOutcome::Empty,
            CatalogOutcome::Error,
        ] {
            let settled = loading.accept(CatalogResponse {
                generation: 7,
                outcome: outcome.clone(),
            });
            assert_eq!(
                settled,
                CatalogState {
                    generation: 7,
                    outcome,
                }
            );
            for duplicate in outcomes() {
                assert_eq!(
                    settled.accept(CatalogResponse {
                        generation: 7,
                        outcome: duplicate,
                    }),
                    settled
                );
            }
            let (retrying, request) = settled.begin(Scenario::Ready).unwrap();
            assert_eq!(request.generation, 8);
            assert_eq!(retrying.outcome, CatalogOutcome::Pending);
        }
    }

    #[test]
    fn exhausted_generation_preserves_catalog_instead_of_reusing_callback_identity() {
        let current = CatalogState {
            generation: u32::MAX,
            outcome: CatalogOutcome::Ready(vec![WorkId::River]),
        };
        assert_eq!(current.begin(Scenario::Loading), None);
        assert_eq!(current.generation, u32::MAX);
        assert_eq!(current.ids(), vec![WorkId::River]);
    }

    /// A controllable read boundary: requests stay pending until the test delivers a result.
    struct DeferredCatalog {
        requests: Rc<RefCell<Vec<CatalogRequest>>>,
    }

    impl CatalogPort for DeferredCatalog {
        fn work(&self, id: WorkId) -> Work {
            MockCatalog.work(id)
        }

        fn fetch(&mut self, request: CatalogRequest) -> CatalogResponse {
            self.requests.borrow_mut().push(request);
            CatalogResponse {
                generation: request.generation,
                outcome: CatalogOutcome::Pending,
            }
        }
    }

    #[test]
    fn session_commits_only_the_latest_deferred_port_result_then_retries_independently() {
        Owner::new().with(|| {
            let requests = Rc::new(RefCell::new(Vec::new()));
            let session = CatalogSession::new(DeferredCatalog {
                requests: requests.clone(),
            });
            session.load(Scenario::Empty);
            assert_eq!(
                requests.borrow().as_slice(),
                [
                    CatalogRequest {
                        generation: 1,
                        scenario: Scenario::Ready,
                    },
                    CatalogRequest {
                        generation: 2,
                        scenario: Scenario::Empty,
                    },
                ]
            );
            session.accept(CatalogResponse {
                generation: 1,
                outcome: CatalogOutcome::Ready(vec![WorkId::Mountain]),
            });
            assert_eq!(
                session.state.get_untracked().outcome,
                CatalogOutcome::Pending
            );
            session.accept(CatalogResponse {
                generation: 2,
                outcome: CatalogOutcome::Empty,
            });
            assert_eq!(session.state.get_untracked().outcome, CatalogOutcome::Empty);

            session.load(Scenario::Ready);
            session.accept(CatalogResponse {
                generation: 2,
                outcome: CatalogOutcome::Error,
            });
            assert_eq!(
                session.state.get_untracked().outcome,
                CatalogOutcome::Pending
            );
            session.accept(CatalogResponse {
                generation: 3,
                outcome: CatalogOutcome::Ready(vec![WorkId::Dream]),
            });
            session.accept(CatalogResponse {
                generation: 1,
                outcome: CatalogOutcome::Error,
            });
            assert_eq!(
                session.state.get_untracked(),
                CatalogState {
                    generation: 3,
                    outcome: CatalogOutcome::Ready(vec![WorkId::Dream]),
                }
            );
        });
    }

    #[test]
    fn local_scenarios_stay_distinct_and_recovery_loads_samples_on_a_new_request() {
        Owner::new().with(|| {
            let session = CatalogSession::new(MockCatalog);
            assert_eq!(session.state.get_untracked().generation, 1);
            assert_eq!(session.state.get_untracked().ids(), WorkId::ALL.to_vec());
            for (scenario, expected) in [
                (Scenario::Loading, CatalogOutcome::Pending),
                (Scenario::Error, CatalogOutcome::Error),
                (Scenario::Empty, CatalogOutcome::Empty),
                (
                    Scenario::Ready,
                    CatalogOutcome::Ready(vec![WorkId::Mountain, WorkId::River, WorkId::Dream]),
                ),
            ] {
                let generation = session.state.get_untracked().generation + 1;
                session.load(scenario);
                assert_eq!(
                    session.state.get_untracked(),
                    CatalogState {
                        generation,
                        outcome: expected,
                    }
                );
            }
        });
    }

    struct InvalidDurationCatalog;

    impl CatalogPort for InvalidDurationCatalog {
        fn work(&self, id: WorkId) -> Work {
            Work {
                id,
                title: "Dữ liệu thử giới hạn thời lượng",
                genre: Genre::Memory,
                synopsis: "Chỉ dùng để kiểm tra ranh giới dữ liệu cục bộ.",
                episodes: &[
                    Episode {
                        title: "Thời lượng rỗng",
                        duration: 0,
                        available: true,
                    },
                    Episode {
                        title: "Thời lượng vượt giới hạn",
                        duration: u32::MAX,
                        available: true,
                    },
                ],
            }
        }

        fn fetch(&mut self, request: CatalogRequest) -> CatalogResponse {
            CatalogResponse {
                generation: request.generation,
                outcome: CatalogOutcome::Pending,
            }
        }
    }

    #[test]
    fn source_lookup_rejects_unavailable_missing_zero_and_overflowing_episodes() {
        Owner::new().with(|| {
            let session = CatalogSession::new(MockCatalog);
            let available = EpisodeId {
                work: WorkId::Mountain,
                index: 0,
            };
            assert_eq!(session.source(available), Source::new(available, 1_480_000));
            for index in [5, 6, usize::MAX] {
                assert_eq!(
                    session.source(EpisodeId {
                        work: WorkId::Mountain,
                        index,
                    }),
                    None
                );
            }
            let invalid = CatalogSession::new(InvalidDurationCatalog);
            for index in [0, 1] {
                assert_eq!(
                    invalid.source(EpisodeId {
                        work: WorkId::Mountain,
                        index,
                    }),
                    None
                );
            }
            assert_eq!(
                invalid
                    .episode(EpisodeId {
                        work: WorkId::Mountain,
                        index: usize::MAX,
                    })
                    .map(|episode| episode.title),
                None
            );
        });
    }
}
