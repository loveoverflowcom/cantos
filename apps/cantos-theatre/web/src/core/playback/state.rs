//! Immutable engine facts. All units are integer ms/permille; no clock, URL or I/O.
use super::Rejection;
use crate::core::{ids::EpisodeId, public_track::PublicTrack};

/// What is being played: a synthetic fixture of the isolated mock, or the exact pinned public
/// release. The two never convert into each other.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TrackId {
    Fixture(EpisodeId),
    Public(PublicTrack),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Source {
    track: TrackId,
    duration_ms: u32,
}

impl Source {
    /// A synthetic mock episode.
    pub fn new(episode: EpisodeId, duration_ms: u32) -> Option<Self> {
        (duration_ms > 0 && episode.index <= 65535).then_some(Self {
            track: TrackId::Fixture(episode),
            duration_ms,
        })
    }
    /// A pinned public release with its verified duration.
    pub fn public(track: PublicTrack, duration_ms: u32) -> Option<Self> {
        (duration_ms > 0).then_some(Self {
            track: TrackId::Public(track),
            duration_ms,
        })
    }
    pub const fn track(self) -> TrackId {
        self.track
    }
    /// The mock episode, or `None` for a public release.
    pub const fn episode(self) -> Option<EpisodeId> {
        match self.track {
            TrackId::Fixture(episode) => Some(episode),
            TrackId::Public(_) => None,
        }
    }
    pub const fn duration_ms(self) -> u32 {
        self.duration_ms
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CallbackIdentity {
    pub track: TrackId,
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
/// Why the engine cannot play. The listener may try again; nothing here retries by itself.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Failure {
    /// The browser declined to start audio (autoplay policy or an interrupted request).
    PlayRefused,
    /// The release is no longer available: withdrawn, revoked or replaced.
    Unavailable,
    /// The connection kept failing after the bounded recovery.
    Network,
    /// The bytes could not be decoded or the format is unsupported.
    Decode,
    /// A server answer broke the contract.
    Protocol,
    /// Re-authorization named a different release; the player never switches to it.
    IdentityChanged,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Engine {
    Preparing,
    Ready,
    Running,
    Paused,
    Buffering,
    Ended,
    Failed(Failure),
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
    /// The engine acknowledged running audio (the mock acknowledges a fake engine).
    Playing,
    Buffering,
    Ended,
    Failed(Failure),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
/// Raw event/fixture input. Admission is required before retaining it as progress.
pub struct ResumeSnapshot {
    pub source: Source,
    pub position_ms: u32,
    pub speed_permille: u16,
}

/// A position within its synthetic source and one supported mock speed.
/// The raw event value remains separate so invalid inputs can be rejected explicitly.
///
/// ```compile_fail
/// use cantos_theatre::core::playback::{ResumeSnapshot, ValidatedResumeSnapshot};
/// fn forge(raw: ResumeSnapshot) -> ValidatedResumeSnapshot {
///     ValidatedResumeSnapshot(raw)
/// }
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidatedResumeSnapshot(ResumeSnapshot);

impl TryFrom<ResumeSnapshot> for ValidatedResumeSnapshot {
    type Error = Rejection;

    fn try_from(snapshot: ResumeSnapshot) -> Result<Self, Self::Error> {
        if snapshot.position_ms > snapshot.source.duration_ms() {
            return Err(Rejection::OutOfBounds);
        }
        if !supported_speed(snapshot.speed_permille) {
            return Err(Rejection::UnsupportedSpeed);
        }
        Ok(Self(snapshot))
    }
}

impl ValidatedResumeSnapshot {
    pub const fn source(self) -> Source {
        self.0.source
    }
    pub const fn position_ms(self) -> u32 {
        self.0.position_ms
    }
    pub const fn speed_permille(self) -> u16 {
        self.0.speed_permille
    }
    pub const fn raw(self) -> ResumeSnapshot {
        self.0
    }
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
    /// Session construction and transitions preserve position and speed admission.
    pub fn validated_snapshot(&self) -> ValidatedResumeSnapshot {
        ValidatedResumeSnapshot(self.snapshot())
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
            track: session.source.track,
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
        if let Engine::Failed(failure) = session.engine {
            return Status::Failed(failure);
        }
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
            Engine::Running => Status::Playing,
            Engine::Buffering => Status::Buffering,
            Engine::Preparing | Engine::Ready | Engine::Paused => Status::Loading,
            Engine::Ended => Status::Ended,
            Engine::Failed(failure) => Status::Failed(failure),
        }
    }
}

pub(super) fn supported_speed(speed: u16) -> bool {
    [750, 1000, 1250, 1500, 2000].contains(&speed)
}
