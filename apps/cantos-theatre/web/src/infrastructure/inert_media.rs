//! A port that never calls an engine. Native builds of the live shell have no audio element;
//! this keeps composition total there without simulating playback.
use crate::{
    application::ports::MediaPort,
    core::playback::{Effect, Event},
};

#[derive(Clone, Copy, Debug, Default)]
pub struct InertMedia;

impl MediaPort for InertMedia {
    fn execute(&mut self, _effect: &Effect) -> Vec<Event> {
        Vec::new()
    }
}
