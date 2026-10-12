//! Catalog request ownership and stale-result boundary, independent of route rendering.
use std::cell::RefCell;
use leptos::prelude::*;
use crate::core::{catalog::{Episode, Work}, ids::{EpisodeId, WorkId}, playback::Source};
use super::ports::CatalogPort;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Scenario { #[default] Ready, Loading, Empty, Error }
impl Scenario {
    pub const fn value(self) -> &'static str {
        match self { Self::Ready => "ready", Self::Loading => "loading", Self::Empty => "empty", Self::Error => "error" }
    }
    pub fn from_value(value: &str) -> Option<Self> {
        match value { "ready" => Some(Self::Ready), "loading" => Some(Self::Loading), "empty" => Some(Self::Empty), "error" => Some(Self::Error), _ => None }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CatalogRequest { pub generation: u32, pub scenario: Scenario }
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogOutcome { Pending, Ready(Vec<WorkId>), Empty, Error }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogResponse { pub generation: u32, pub outcome: CatalogOutcome }
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogState { pub generation: u32, pub outcome: CatalogOutcome }
impl Default for CatalogState {
    fn default() -> Self { Self { generation: 0, outcome: CatalogOutcome::Pending } }
}
impl CatalogState {
    pub fn begin(&self, scenario: Scenario) -> Option<(Self, CatalogRequest)> {
        let generation = self.generation.checked_add(1)?;
        Some((Self { generation, outcome: CatalogOutcome::Pending }, CatalogRequest { generation, scenario }))
    }
    pub fn accept(&self, response: CatalogResponse) -> Self {
        if response.generation != self.generation { return self.clone(); }
        Self { generation: self.generation, outcome: response.outcome }
    }
    pub fn scenario(&self) -> Scenario {
        match &self.outcome { CatalogOutcome::Pending => Scenario::Loading, CatalogOutcome::Ready(_) => Scenario::Ready, CatalogOutcome::Empty => Scenario::Empty, CatalogOutcome::Error => Scenario::Error }
    }
    pub fn ids(&self) -> Vec<WorkId> {
        match &self.outcome { CatalogOutcome::Ready(ids) => ids.clone(), CatalogOutcome::Pending | CatalogOutcome::Empty | CatalogOutcome::Error => Vec::new() }
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
        let session = Self { state: state.read_only(), writer: state, adapter: StoredValue::new_local(RefCell::new(Box::new(adapter))) };
        session.load(Scenario::Ready);
        session
    }
    pub fn load(self, scenario: Scenario) {
        let Some((next, request)) = self.state.get_untracked().begin(scenario) else { return; };
        self.writer.set(next);
        let response = self.adapter.with_value(|port| port.borrow_mut().fetch(request));
        self.accept(response);
    }
    pub fn accept(self, response: CatalogResponse) {
        let next = self.state.get_untracked().accept(response);
        self.writer.set(next);
    }
    pub fn scenario(self) -> Scenario { self.state.get().scenario() }
    pub fn work(self, id: WorkId) -> Work { self.adapter.with_value(|port| port.borrow().work(id)) }
    pub fn episode(self, id: EpisodeId) -> Option<Episode> { self.work(id.work).episodes.get(id.index).copied() }
    pub fn source(self, id: EpisodeId) -> Option<Source> {
        let episode = self.episode(id)?;
        if !episode.available { return None; }
        Source::new(id, episode.duration.checked_mul(1000)?)
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn newer_catalog_request_discards_older_result_including_error() {
        let (loading, old) = CatalogState::default().begin(Scenario::Ready).unwrap();
        let (newer, current) = loading.begin(Scenario::Empty).unwrap();
        let ready = newer.accept(CatalogResponse { generation: current.generation, outcome: CatalogOutcome::Empty });
        for outcome in [CatalogOutcome::Ready(WorkId::ALL.to_vec()), CatalogOutcome::Error] {
            assert_eq!(ready.accept(CatalogResponse { generation: old.generation, outcome }), ready);
        }
    }
}
