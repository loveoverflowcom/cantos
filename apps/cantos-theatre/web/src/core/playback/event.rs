use super::state::{CallbackIdentity, Failure, PlayerState, ResumeSnapshot, Source};

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Event {
    /// Drop account-bound source and buffered media without a checkpoint or resume offer.
    AccessCleared,
    SourceSelected {
        source: Source,
        position_ms: u32,
    },
    PlayRequested,
    PauseRequested,
    SeekRequested {
        position_ms: u32,
    },
    SkipRequested {
        delta_ms: i32,
    },
    SpeedRequested {
        speed_permille: u16,
    },
    RestoreRequested {
        snapshot: ResumeSnapshot,
    },
    InterruptionBegan,
    InterruptionEnded {
        resume_allowed: bool,
    },
    EngineReady {
        identity: CallbackIdentity,
    },
    EngineStarted {
        identity: CallbackIdentity,
    },
    EnginePaused {
        identity: CallbackIdentity,
    },
    EngineBuffering {
        identity: CallbackIdentity,
    },
    EnginePosition {
        identity: CallbackIdentity,
        position_ms: u32,
    },
    EngineEnded {
        identity: CallbackIdentity,
    },
    /// The engine cannot play. It is a fact about the identity it names, so a failure of a
    /// replaced source or intent cannot change the current state.
    EngineFailed {
        identity: CallbackIdentity,
        failure: Failure,
    },
}

/// Complete current-generation facts, so a command never depends on a stale prepare.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EngineCommand {
    pub identity: CallbackIdentity,
    pub source: Source,
    pub position_ms: u32,
    pub speed_permille: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Rejection {
    NoSession,
    Interrupted,
    OutOfBounds,
    UnsupportedSpeed,
    GenerationExhausted,
    InvalidSource,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Effect {
    ReleaseEngine,
    PrepareEngine(EngineCommand),
    StartEngine(EngineCommand),
    PauseEngine(EngineCommand),
    SeekEngine(EngineCommand),
    SetSpeed(EngineCommand),
    OfferResume { source: Source, position_ms: u32 },
    Rejected(Rejection),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Transition {
    pub state: PlayerState,
    pub effects: Vec<Effect>,
}
