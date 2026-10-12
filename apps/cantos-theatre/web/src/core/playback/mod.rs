mod event;
mod reducer;
mod state;

pub use event::{EngineCommand, Event, Effect, Rejection, Transition};
pub use reducer::reduce;
pub use state::{CallbackIdentity, Engine, Intent, Interruption, PauseCause, PlaybackSession, PlayerState, ResumeSnapshot, Source, Status};

#[cfg(test)]
mod tests;
#[cfg(test)]
mod vector_tests;
