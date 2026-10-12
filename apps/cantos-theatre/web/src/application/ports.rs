//! Concrete consumer boundaries; mock adapters implement these ports in this app.
use super::catalog::{CatalogRequest, CatalogResponse};
use crate::core::{
    catalog::Work,
    ids::WorkId,
    playback::{Effect, Event},
};

pub trait CatalogPort {
    fn work(&self, id: WorkId) -> Work;
    fn fetch(&mut self, request: CatalogRequest) -> CatalogResponse;
}
/// Callbacks are values delivered back to the session, never mutations of core state.
pub trait MediaPort {
    fn execute(&mut self, effect: &Effect) -> Vec<Event>;
}
