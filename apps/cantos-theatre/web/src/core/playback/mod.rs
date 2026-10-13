mod event;
mod reducer;
mod state;

pub use event::{Effect, EngineCommand, Event, Rejection, Transition};
pub use reducer::reduce;
pub use state::{
    CallbackIdentity, Engine, Intent, Interruption, PauseCause, PlaybackSession, PlayerState,
    ResumeSnapshot, Source, Status, ValidatedResumeSnapshot,
};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vector_tests;
