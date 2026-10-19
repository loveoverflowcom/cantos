//! Concrete consumer boundaries; mock adapters implement these ports in this app.
#[cfg(feature = "preview")]
use super::catalog::{CatalogRequest, CatalogResponse};
use crate::core::playback::{Effect, Event};
#[cfg(feature = "preview")]
use crate::core::{catalog::Work, ids::WorkId};

#[cfg(feature = "preview")]
pub trait CatalogPort {
    fn work(&self, id: WorkId) -> Work;
    fn fetch(&mut self, request: CatalogRequest) -> CatalogResponse;
}
/// Callbacks are values delivered back to the session, never mutations of core state.
pub trait MediaPort {
    fn execute(&mut self, effect: &Effect) -> Vec<Event>;
}
