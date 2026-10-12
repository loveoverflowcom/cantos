//! Immutable fake-engine facts. All units are integer ms/permille; no clock, URL or I/O.
use crate::core::ids::EpisodeId;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    episode: EpisodeId,
    duration_ms: u32,
}

impl Source {
    pub fn new(episode: EpisodeId, duration_ms: u32) -> Option<Self> {
        (duration_ms > 0 && episode.index <= 65535).then_some(Self {
            episode,
            duration_ms,
        })
    }
    pub const fn episode(self) -> EpisodeId {
        self.episode
    }
    pub const fn duration_ms(self) -> u32 {
        self.duration_ms
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackIdentity {
    pub episode: EpisodeId,
    pub generation: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PauseCause {
    User,
    Interruption,
    ColdRestore,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    Play,
    Pause(PauseCause),
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Preparing,
    Ready,
    Running,
    Paused,
    Buffering,
    Ended,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Interruption {
    None,
    Active,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Status {
    Idle,
    Loading,
    Paused(PauseCause),
    Simulating,
    Buffering,
    Ended,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeSnapshot {
    pub source: Source,
    pub position_ms: u32,
    pub speed_permille: u16,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaybackSession {
    pub(super) source: Source,
    pub(super) position_ms: u32,
    pub(super) speed_permille: u16,
    pub(super) intent: Intent,
    pub(super) engine: Engine,
    pub(super) interruption: Interruption,
}

impl PlaybackSession {
    pub const fn source(&self) -> Source {
        self.source
    }
    pub const fn position_ms(&self) -> u32 {
        self.position_ms
    }
    pub const fn speed_permille(&self) -> u16 {
        self.speed_permille
    }
    pub const fn intent(&self) -> Intent {
        self.intent
    }
    pub const fn engine(&self) -> Engine {
        self.engine
    }
    pub const fn interruption(&self) -> Interruption {
        self.interruption
    }
    pub fn snapshot(&self) -> ResumeSnapshot {
        ResumeSnapshot {
            source: self.source,
            position_ms: self.position_ms,
            speed_permille: self.speed_permille,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PlayerState {
    pub(super) generation: u32,
    pub(super) session: Option<PlaybackSession>,
}

impl PlayerState {
    pub const fn generation(&self) -> u32 {
        self.generation
    }
    pub fn session(&self) -> Option<&PlaybackSession> {
        self.session.as_ref()
    }
    pub fn identity(&self) -> Option<CallbackIdentity> {
        self.session.as_ref().map(|session| CallbackIdentity {
            episode: session.source.episode,
            generation: self.generation,
        })
    }
    pub fn accepts(&self, identity: CallbackIdentity) -> bool {
        self.identity() == Some(identity)
    }
    pub fn status(&self) -> Status {
        let Some(session) = &self.session else {
            return Status::Idle;
        };
        if session.engine == Engine::Ended {
            return Status::Ended;
        }
        if let Intent::Pause(cause) = session.intent {
            return Status::Paused(cause);
        }
        if session.interruption == Interruption::Active {
            return Status::Paused(PauseCause::Interruption);
        }
        match session.engine {
            Engine::Running => Status::Simulating,
            Engine::Buffering => Status::Buffering,
            Engine::Preparing | Engine::Ready | Engine::Paused => Status::Loading,
            Engine::Ended => Status::Ended,
        }
    }
}

pub(super) fn supported_speed(speed: u16) -> bool {
    [750, 1000, 1250, 1500, 2000].contains(&speed)
}
