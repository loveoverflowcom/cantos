//! Pure decisions of the public-audio engine: the grant lifecycle, bounded recovery and the
//! mapping from audio-element facts to player events. It performs no I/O: the browser shell
//! executes each [`Command`] and hands every outcome back as an [`Input`].
//!
//! Three rules shape it. The pinned release never changes: every re-authorization is for the
//! same publication and is checked against the pinned identity, so a republish cannot switch the
//! audio. A temporary grant is a capability, replaced when it nears expiry while the listener is
//! playing and again whenever the element reports it failed. Recovery is bounded: a withdrawn
//! release stops at once, a retryable answer backs off a few times, and nothing loops.
use crate::contract::discovery::PlaybackAuthorization;
use crate::core::{
    playback::{CallbackIdentity, Effect, Event, Failure, Source, TrackId},
    public_track::{AdmittedGrant, GrantRejection, MediaUrl, PublicTrack},
};
use uuid::Uuid;

/// Total authorization attempts for one purpose before the answer is reported.
const MAX_ATTEMPTS: u8 = 4;
/// Consecutive element failures recovered without audible progress.
const MAX_RECOVERIES: u8 = 3;
/// Playback past the recovery point by this long proves the recovery worked.
const PROGRESS_RESETS_RECOVERY_MS: u32 = 5_000;
/// The next grant is requested this long before the current one expires.
const REFRESH_MARGIN_MS: i64 = 30_000;
/// A short rights window never makes refreshes more frequent than this.
const MIN_REFRESH_DELAY_MS: i64 = 10_000;

/// Retry delay after a failed attempt, `attempt` being the one that just failed (1-based).
const fn backoff_ms(attempt: u8) -> u32 {
    1_000 << (attempt - 1)
}

/// Names one request or timer; a result for any other ticket is a stale answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Ticket(u64);

/// Names one attachment of a URL to the audio element; its facts never apply to another.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachId(u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthorizationError {
    Diagnostic(super::errors::ErrorKind),
    /// The contract's merged answer: withdrawn, revoked, expired or never public.
    NotFound,
    /// Transport, timeout or a retryable server answer.
    Unavailable,
    /// The answer breaks the contract.
    Protocol,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MediaFault {
    Network,
    Decode,
    /// The element could not use the source: the answer a missing, expired or forbidden URL gives.
    Unsupported,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayRejection {
    /// The browser's autoplay or interruption policy refused `play()`.
    NotAllowed,
    /// A newer load or pause superseded the request; not a failure.
    Aborted,
    /// The browser cannot play this media.
    NotSupported,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ElementEvent {
    Loaded,
    Playing,
    Paused,
    Waiting,
    /// A progress tick.
    Position(u32),
    /// A requested seek completed; the element may land a few milliseconds off the target.
    Seeked(u32),
    Ended,
    Error(MediaFault),
    PlayRejected(PlayRejection),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Input {
    Engine(Box<Effect>),
    Authorized {
        ticket: Ticket,
        result: Result<PlaybackAuthorization, AuthorizationError>,
    },
    Timer(Ticket),
    Element {
        attach: AttachId,
        event: ElementEvent,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Command {
    /// POST a playback authorization for exactly this publication, then answer `Authorized`.
    Authorize {
        ticket: Ticket,
        publication: Uuid,
    },
    /// Answer `Timer(ticket)` after `delay_ms`.
    Wait {
        ticket: Ticket,
        delay_ms: u32,
    },
    /// Replace the element's source, starting at `start_ms`.
    Attach {
        attach: AttachId,
        url: MediaUrl,
        start_ms: u32,
        play: bool,
        speed_permille: u16,
    },
    /// Start playback, first moving to `from_ms` when the element is elsewhere.
    Play {
        from_ms: u32,
    },
    Pause,
    Seek {
        to_ms: u32,
    },
    Speed {
        permille: u16,
    },
    /// Stop the element and drop every source, listener, request and timer of this engine.
    Release,
    /// Report to the player.
    Emit(Event),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Purpose {
    /// The element needs a URL: first load or recovery.
    Attach,
    /// Keep a fresh grant ready while the listener plays.
    Refresh,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Request {
    ticket: Ticket,
    purpose: Purpose,
    attempt: u8,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum WaitKind {
    Refresh,
    Retry { purpose: Purpose, attempt: u8 },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Wait {
    ticket: Ticket,
    kind: WaitKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Attached {
    id: AttachId,
    grant_expires_at_ms: i64,
}

#[derive(Debug)]
struct Active {
    track: PublicTrack,
    duration_ms: u32,
    identity: CallbackIdentity,
    position_ms: u32,
    speed_permille: u16,
    wants_play: bool,
    /// `play()` was issued and neither started nor was rejected: a `pause` fact is then stale.
    pending_play: bool,
    failed: bool,
    attached: Option<Attached>,
    held: Option<AdmittedGrant>,
    request: Option<Request>,
    wait: Option<Wait>,
    recoveries: u8,
    recovery_floor_ms: u32,
    /// The position the player asked for and the element has not yet confirmed reaching.
    seek_target: Option<u32>,
}

#[derive(Debug, Default)]
pub struct MediaDriver {
    counter: u64,
    active: Option<Active>,
}

impl MediaDriver {
    pub fn new() -> Self {
        Self::default()
    }

    fn next(&mut self) -> u64 {
        self.counter = self.counter.saturating_add(1);
        self.counter
    }

    pub fn step(&mut self, now_ms: i64, input: Input) -> Vec<Command> {
        match input {
            Input::Engine(effect) => self.engine(now_ms, *effect),
            Input::Authorized { ticket, result } => self.authorized(now_ms, ticket, result),
            Input::Timer(ticket) => self.timer(now_ms, ticket),
            Input::Element { attach, event } => self.element(now_ms, attach, event),
        }
    }

    fn engine(&mut self, now_ms: i64, effect: Effect) -> Vec<Command> {
        let (command, prepare) = match effect {
            Effect::PrepareEngine(command) => (command, true),
            Effect::StartEngine(command)
            | Effect::PauseEngine(command)
            | Effect::SeekEngine(command)
            | Effect::SetSpeed(command) => (command, false),
            Effect::OfferResume { .. } | Effect::Rejected(_) => return Vec::new(),
        };
        let TrackId::Public(track) = command.source.track() else {
            return Vec::new();
        };
        let same = self
            .active
            .as_ref()
            .is_some_and(|active| active.track == track);
        let mut out = Vec::new();
        if !same {
            if !prepare {
                // Only a preparation can begin a source; a stray command is stale.
                return out;
            }
            if self.active.take().is_some() {
                out.push(Command::Release);
            }
            self.active = Some(Active::new(track, command.source, command.identity));
            let (purpose, speed) = (Purpose::Attach, command.speed_permille);
            if let Some(active) = self.active.as_mut() {
                active.position_ms = command.position_ms;
                active.speed_permille = speed;
            }
            out.extend(self.request(purpose, 1));
            return out;
        }
        let Some(active) = self.active.as_mut() else {
            return out;
        };
        active.identity = command.identity;
        let previous_position = active.position_ms;
        active.position_ms = command.position_ms.min(active.duration_ms);
        let speed_changed = active.speed_permille != command.speed_permille;
        active.speed_permille = command.speed_permille;
        match effect {
            Effect::PrepareEngine(_) => {
                if active.attached.is_some() && previous_position != active.position_ms {
                    active.seek_target = Some(active.position_ms);
                    out.push(Command::Seek {
                        to_ms: active.position_ms,
                    });
                }
            }
            Effect::StartEngine(_) => {
                active.wants_play = true;
                if active.failed {
                    active.failed = false;
                    active.recoveries = 0;
                    out.extend(self.request(Purpose::Attach, 1));
                } else if active.attached.is_some() {
                    active.pending_play = true;
                    out.push(Command::Play {
                        from_ms: active.position_ms,
                    });
                    out.extend(self.arm_refresh(now_ms));
                }
                // Otherwise the pending attachment starts playing as it lands.
            }
            Effect::PauseEngine(_) => {
                active.wants_play = false;
                active.pending_play = false;
                active.wait = active
                    .wait
                    .filter(|wait| !matches!(wait.kind, WaitKind::Refresh));
                if active.attached.is_some() {
                    out.push(Command::Pause);
                }
            }
            Effect::SeekEngine(_) => {
                if active.attached.is_some() && !active.failed {
                    active.seek_target = Some(active.position_ms);
                    out.push(Command::Seek {
                        to_ms: active.position_ms,
                    });
                }
            }
            Effect::SetSpeed(_) => {
                if speed_changed && active.attached.is_some() {
                    out.push(Command::Speed {
                        permille: active.speed_permille,
                    });
                }
            }
            Effect::OfferResume { .. } | Effect::Rejected(_) => {}
        }
        out
    }

    /// Issue an authorization request for the active release.
    fn request(&mut self, purpose: Purpose, attempt: u8) -> Vec<Command> {
        let ticket = Ticket(self.next());
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        active.request = Some(Request {
            ticket,
            purpose,
            attempt,
        });
        active.wait = None;
        vec![Command::Authorize {
            ticket,
            publication: active.track.publication(),
        }]
    }

    fn arm_refresh(&mut self, now_ms: i64) -> Vec<Command> {
        let ticket = Ticket(self.next());
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if !active.wants_play || active.failed || active.request.is_some() {
            return Vec::new();
        }
        if active
            .wait
            .is_some_and(|wait| matches!(wait.kind, WaitKind::Refresh))
        {
            return Vec::new();
        }
        let due = active.held.as_ref().map_or(0, |grant| {
            (grant.expires_at_ms() - REFRESH_MARGIN_MS - now_ms).max(MIN_REFRESH_DELAY_MS)
        });
        let delay_ms = u32::try_from(due).unwrap_or(u32::MAX);
        active.wait = Some(Wait {
            ticket,
            kind: WaitKind::Refresh,
        });
        vec![Command::Wait { ticket, delay_ms }]
    }

    fn authorized(
        &mut self,
        now_ms: i64,
        ticket: Ticket,
        result: Result<PlaybackAuthorization, AuthorizationError>,
    ) -> Vec<Command> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        let Some(request) = active.request.filter(|request| request.ticket == ticket) else {
            return Vec::new();
        };
        active.request = None;
        let admitted = match result {
            Ok(authorization) => AdmittedGrant::admit(&active.track, &authorization, now_ms)
                .map_err(|rejection| match rejection {
                    GrantRejection::IdentityChanged => Failure::IdentityChanged,
                    GrantRejection::Protocol => Failure::Protocol,
                }),
            Err(AuthorizationError::Diagnostic(kind)) => {
                use super::errors::ErrorKind;
                if matches!(kind, ErrorKind::Network | ErrorKind::Timeout)
                    && request.attempt < MAX_ATTEMPTS
                {
                    return self.retry(request);
                }
                if matches!(kind, ErrorKind::Network | ErrorKind::Timeout)
                    && request.purpose == Purpose::Refresh
                {
                    return Vec::new();
                }
                Err(match kind {
                    ErrorKind::Network => Failure::Network,
                    ErrorKind::Timeout => Failure::Timeout,
                    ErrorKind::Unauthenticated => Failure::Unauthenticated,
                    ErrorKind::Forbidden => Failure::Forbidden,
                    ErrorKind::Configuration => Failure::Configuration,
                    ErrorKind::ContentUnavailable => Failure::Unavailable,
                    ErrorKind::Protocol | ErrorKind::Storage => Failure::Protocol,
                    ErrorKind::ServiceUnavailable => Failure::Network,
                })
            }
            Err(AuthorizationError::NotFound) => Err(Failure::Unavailable),
            Err(AuthorizationError::Protocol) => Err(Failure::Protocol),
            Err(AuthorizationError::Unavailable) => {
                if request.attempt < MAX_ATTEMPTS {
                    return self.retry(request);
                }
                // A refresh that keeps failing leaves the working grant alone; the element's
                // own failure would trigger recovery.
                return match request.purpose {
                    Purpose::Refresh => Vec::new(),
                    Purpose::Attach => self.fail(Failure::Network),
                };
            }
        };
        let grant = match admitted {
            Ok(grant) => grant,
            Err(failure) => return self.fail(failure),
        };
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        active.held = Some(grant);
        let mut out = Vec::new();
        if request.purpose == Purpose::Attach {
            out.extend(self.attach(now_ms));
        }
        out.extend(self.arm_refresh(now_ms));
        out
    }

    fn retry(&mut self, failed: Request) -> Vec<Command> {
        let ticket = Ticket(self.next());
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        active.wait = Some(Wait {
            ticket,
            kind: WaitKind::Retry {
                purpose: failed.purpose,
                attempt: failed.attempt + 1,
            },
        });
        vec![Command::Wait {
            ticket,
            delay_ms: backoff_ms(failed.attempt),
        }]
    }

    /// Attach the held grant at the current position.
    fn attach(&mut self, now_ms: i64) -> Vec<Command> {
        let id = AttachId(self.next());
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        let Some(grant) = active
            .held
            .as_ref()
            .filter(|grant| grant.is_fresh_at(now_ms))
        else {
            return Vec::new();
        };
        active.attached = Some(Attached {
            id,
            grant_expires_at_ms: grant.expires_at_ms(),
        });
        active.pending_play = active.wants_play;
        active.recovery_floor_ms = active.position_ms;
        active.seek_target = (active.position_ms > 0).then_some(active.position_ms);
        vec![Command::Attach {
            attach: id,
            url: grant.url().clone(),
            start_ms: active.position_ms,
            play: active.wants_play,
            speed_permille: active.speed_permille,
        }]
    }

    fn timer(&mut self, now_ms: i64, ticket: Ticket) -> Vec<Command> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        let Some(wait) = active.wait.filter(|wait| wait.ticket == ticket) else {
            return Vec::new();
        };
        active.wait = None;
        match wait.kind {
            WaitKind::Refresh if active.wants_play && !active.failed => {
                self.request(Purpose::Refresh, 1)
            }
            WaitKind::Refresh => Vec::new(),
            WaitKind::Retry { purpose, attempt } => {
                let _ = now_ms;
                self.request(purpose, attempt)
            }
        }
    }

    fn element(&mut self, now_ms: i64, attach: AttachId, event: ElementEvent) -> Vec<Command> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if active.failed || active.attached.map(|attached| attached.id) != Some(attach) {
            return Vec::new();
        }
        let identity = active.identity;
        match event {
            ElementEvent::Loaded => vec![Command::Emit(Event::EngineReady { identity })],
            ElementEvent::Playing => {
                active.pending_play = false;
                vec![Command::Emit(Event::EngineStarted { identity })]
            }
            ElementEvent::Paused if active.pending_play => Vec::new(),
            // Nothing of ours paused it: a media key, headset or system interruption did. The
            // listener's intent changes with it, or the page would claim to be playing.
            ElementEvent::Paused if active.wants_play => {
                vec![Command::Emit(Event::PauseRequested)]
            }
            ElementEvent::Paused => vec![Command::Emit(Event::EnginePaused { identity })],
            ElementEvent::Waiting if active.wants_play => {
                vec![Command::Emit(Event::EngineBuffering { identity })]
            }
            ElementEvent::Waiting => Vec::new(),
            ElementEvent::Position(position_ms) => {
                let position_ms = position_ms.min(active.duration_ms);
                active.position_ms = position_ms;
                if position_ms
                    >= active
                        .recovery_floor_ms
                        .saturating_add(PROGRESS_RESETS_RECOVERY_MS)
                {
                    active.recoveries = 0;
                }
                vec![Command::Emit(Event::EnginePosition {
                    identity,
                    position_ms,
                })]
            }
            ElementEvent::Seeked(landed) => {
                let landed = landed.min(active.duration_ms);
                active.position_ms = landed;
                let mut out = Vec::new();
                if let Some(target) = active.seek_target.take() {
                    // The element confirmed the seek: acknowledge the requested position exactly,
                    // then report where it really is. Browsers rarely land on the millisecond.
                    out.push(Command::Emit(Event::EnginePosition {
                        identity,
                        position_ms: target,
                    }));
                }
                out.push(Command::Emit(Event::EnginePosition {
                    identity,
                    position_ms: landed,
                }));
                out
            }
            ElementEvent::Ended => {
                active.wants_play = false;
                active.pending_play = false;
                active.position_ms = active.duration_ms;
                vec![Command::Emit(Event::EngineEnded { identity })]
            }
            ElementEvent::PlayRejected(rejection) => {
                active.pending_play = false;
                match rejection {
                    PlayRejection::Aborted => Vec::new(),
                    PlayRejection::NotAllowed | PlayRejection::Other => {
                        self.fail_keeping_element(Failure::PlayRefused)
                    }
                    PlayRejection::NotSupported => self.fail(Failure::Decode),
                }
            }
            ElementEvent::Error(MediaFault::Decode) => self.fail(Failure::Decode),
            ElementEvent::Error(MediaFault::Network | MediaFault::Unsupported) => {
                self.recover(now_ms)
            }
        }
    }

    /// The element failed to use its URL: switch to a fresher grant, or ask for one.
    fn recover(&mut self, now_ms: i64) -> Vec<Command> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        if active.recoveries >= MAX_RECOVERIES {
            // Fresh authorizations were admitted yet the element still cannot load the bytes: the
            // transport, not the format, is failing. A format this browser cannot play never
            // reaches here; the shell reports it as `Decode` before attaching.
            return self.fail(Failure::Network);
        }
        active.recoveries += 1;
        let attached_until = active
            .attached
            .map_or(i64::MIN, |attached| attached.grant_expires_at_ms);
        active.attached = None;
        let newer_ready = active.held.as_ref().is_some_and(|grant| {
            grant.is_fresh_at(now_ms) && grant.expires_at_ms() > attached_until
        });
        if newer_ready {
            return self.attach(now_ms);
        }
        self.request(Purpose::Attach, 1)
    }

    /// Stop with `failure`, releasing the element and everything pending.
    fn fail(&mut self, failure: Failure) -> Vec<Command> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        active.failed = true;
        active.wants_play = false;
        active.pending_play = false;
        active.seek_target = None;
        active.attached = None;
        active.request = None;
        active.wait = None;
        vec![
            Command::Release,
            Command::Emit(Event::EngineFailed {
                identity: active.identity,
                failure,
            }),
        ]
    }

    /// A refused `play()` leaves a working element in place so a gesture can start it.
    fn fail_keeping_element(&mut self, failure: Failure) -> Vec<Command> {
        let Some(active) = self.active.as_mut() else {
            return Vec::new();
        };
        active.wants_play = false;
        active.wait = None;
        vec![
            Command::Pause,
            Command::Emit(Event::EngineFailed {
                identity: active.identity,
                failure,
            }),
        ]
    }

    /// Whether this driver currently owns `track`.
    pub fn holds(&self, track: PublicTrack) -> bool {
        self.active
            .as_ref()
            .is_some_and(|active| active.track == track)
    }
}

impl Active {
    fn new(track: PublicTrack, source: Source, identity: CallbackIdentity) -> Self {
        Self {
            track,
            duration_ms: source.duration_ms(),
            identity,
            position_ms: 0,
            speed_permille: 1000,
            wants_play: false,
            pending_play: false,
            failed: false,
            attached: None,
            held: None,
            request: None,
            wait: None,
            recoveries: 0,
            recovery_floor_ms: 0,
            seek_target: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::discovery::PublicationManifest;
    use crate::core::{
        playback::EngineCommand,
        public_ids::PublicWorkId,
        public_track::{PinnedRelease, ReleaseRequest},
    };

    const MANIFEST: &str = include_str!(
        "../../../../../services/cantos-server/contracts/theatre/discovery/v1/fixtures/manifest.json"
    );
    const WORK: &str = "10000000-0000-4000-8000-000000000001";
    const UNIT: &str = "20000000-0000-4000-8000-000000000002";
    const T0: i64 = 1_000_000;

    fn manifest(publication: &str, rendition: &str, sha: &str) -> PublicationManifest {
        let mut manifest: PublicationManifest = serde_json::from_str(MANIFEST).unwrap();
        manifest.publication_id = publication.into();
        manifest.rendition_id = rendition.into();
        manifest.sha256 = sha.into();
        manifest.duration_ms = 600_000;
        manifest
    }
    fn release(publication: &str, rendition: &str, sha: &str) -> PinnedRelease {
        let request =
            ReleaseRequest::parse(PublicWorkId::parse(WORK).unwrap(), UNIT, publication).unwrap();
        PinnedRelease::admit(&manifest(publication, rendition, sha), request).unwrap()
    }
    const P1: &str = "80000000-0000-4000-8000-000000000008";
    const R1: &str = "60000000-0000-4000-8000-000000000006";
    const SHA1: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
    const P2: &str = "80000000-0000-4000-8000-000000000009";
    const R2: &str = "60000000-0000-4000-8000-000000000007";
    const SHA2: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

    fn first() -> PinnedRelease {
        release(P1, R1, SHA1)
    }
    fn republished() -> PinnedRelease {
        release(P2, R2, SHA2)
    }

    fn source(release: PinnedRelease) -> Source {
        Source::public(release.track(), release.duration_ms()).unwrap()
    }
    fn command_for(release: PinnedRelease, generation: u32, position_ms: u32) -> EngineCommand {
        EngineCommand {
            identity: CallbackIdentity {
                track: TrackId::Public(release.track()),
                generation,
            },
            source: source(release),
            position_ms,
            speed_permille: 1000,
        }
    }
    fn authorization(release: PinnedRelease, now: i64, ttl: i64) -> PlaybackAuthorization {
        let track = release.track();
        PlaybackAuthorization {
            publication_id: track.publication().to_string(),
            rendition_id: track.rendition().to_string(),
            sha256: track.sha256().to_string(),
            url: format!("https://localhost:8443/api/v1/theatre/media/pg1_{now}"),
            expires_at_ms: now + ttl,
            content_type: "audio/ogg".into(),
            byte_range_supported: true,
        }
    }

    struct Harness {
        driver: MediaDriver,
        now: i64,
        release: PinnedRelease,
        generation: u32,
    }
    impl Harness {
        fn new(release: PinnedRelease) -> Self {
            Self {
                driver: MediaDriver::new(),
                now: T0,
                release,
                generation: 1,
            }
        }
        fn engine(&mut self, make: fn(EngineCommand) -> Effect, position_ms: u32) -> Vec<Command> {
            self.generation += 1;
            let effect = make(command_for(self.release, self.generation, position_ms));
            self.driver.step(self.now, Input::Engine(Box::new(effect)))
        }
        fn prepare(&mut self) -> Vec<Command> {
            self.generation = 1;
            let effect = Effect::PrepareEngine(command_for(self.release, 1, 0));
            self.driver.step(self.now, Input::Engine(Box::new(effect)))
        }
        fn answer(&mut self, ticket: Ticket, ttl: i64) -> Vec<Command> {
            let result = Ok(authorization(self.release, self.now, ttl));
            self.driver
                .step(self.now, Input::Authorized { ticket, result })
        }
        fn fail_with(&mut self, ticket: Ticket, error: AuthorizationError) -> Vec<Command> {
            self.driver.step(
                self.now,
                Input::Authorized {
                    ticket,
                    result: Err(error),
                },
            )
        }
        fn element(&mut self, attach: AttachId, event: ElementEvent) -> Vec<Command> {
            self.driver.step(self.now, Input::Element { attach, event })
        }
        fn identity(&self) -> CallbackIdentity {
            CallbackIdentity {
                track: TrackId::Public(self.release.track()),
                generation: self.generation,
            }
        }
    }

    fn ticket_of(commands: &[Command]) -> Ticket {
        commands
            .iter()
            .find_map(|command| match command {
                Command::Authorize { ticket, .. } | Command::Wait { ticket, .. } => Some(*ticket),
                _ => None,
            })
            .expect("a ticketed command")
    }
    fn attach_of(commands: &[Command]) -> AttachId {
        commands
            .iter()
            .find_map(|command| match command {
                Command::Attach { attach, .. } => Some(*attach),
                _ => None,
            })
            .expect("an attach command")
    }
    fn failure_of(commands: &[Command]) -> Option<Failure> {
        commands.iter().find_map(|command| match command {
            Command::Emit(Event::EngineFailed { failure, .. }) => Some(*failure),
            _ => None,
        })
    }
    fn authorizations(commands: &[Command]) -> usize {
        commands
            .iter()
            .filter(|command| matches!(command, Command::Authorize { .. }))
            .count()
    }

    /// A harness attached to its first grant and playing.
    fn playing() -> (Harness, AttachId) {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        let attach = attach_of(&h.answer(ticket, 120_000));
        h.engine(Effect::StartEngine, 0);
        h.element(attach, ElementEvent::Playing);
        (h, attach)
    }

    #[test]
    fn preparing_authorizes_the_pinned_publication_and_attaches_without_playing() {
        let mut h = Harness::new(first());
        let prepared = h.prepare();
        assert_eq!(
            prepared,
            vec![Command::Authorize {
                ticket: ticket_of(&prepared),
                publication: first().track().publication()
            }]
        );
        let granted = h.answer(ticket_of(&prepared), 120_000);
        assert_eq!(
            granted.len(),
            1,
            "selection alone arms no refresh: {granted:?}"
        );
        match &granted[0] {
            Command::Attach {
                url,
                start_ms,
                play,
                speed_permille,
                ..
            } => {
                assert!(url.as_str().starts_with("https://localhost:8443/"));
                assert_eq!((*start_ms, *play, *speed_permille), (0, false, 1000));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn a_start_that_arrives_before_the_grant_plays_as_the_source_lands() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        assert!(h.engine(Effect::StartEngine, 0).is_empty());
        let granted = h.answer(ticket, 120_000);
        assert!(matches!(&granted[0], Command::Attach { play: true, .. }));
        assert!(
            matches!(
                &granted[1],
                Command::Wait {
                    delay_ms: 90_000,
                    ..
                }
            ),
            "refresh at expiry minus 30 s: {granted:?}"
        );
    }

    #[test]
    fn engine_facts_become_player_events_with_the_current_identity() {
        let (mut h, attach) = playing();
        let identity = h.identity();
        assert_eq!(
            h.element(attach, ElementEvent::Loaded),
            vec![Command::Emit(Event::EngineReady { identity })]
        );
        assert_eq!(
            h.element(attach, ElementEvent::Waiting),
            vec![Command::Emit(Event::EngineBuffering { identity })]
        );
        assert_eq!(
            h.element(attach, ElementEvent::Position(2_500)),
            vec![Command::Emit(Event::EnginePosition {
                identity,
                position_ms: 2_500
            })]
        );
        // The element may report slightly past the verified duration; the pin bounds it.
        assert_eq!(
            h.element(attach, ElementEvent::Position(9_999_999)),
            vec![Command::Emit(Event::EnginePosition {
                identity,
                position_ms: 600_000
            })]
        );
        assert_eq!(
            h.element(attach, ElementEvent::Ended),
            vec![Command::Emit(Event::EngineEnded { identity })]
        );
    }

    #[test]
    fn a_completed_seek_is_acknowledged_at_the_requested_position_then_at_the_landing() {
        let (mut h, attach) = playing();
        h.engine(Effect::SeekEngine, 90_000);
        let identity = h.identity();
        assert_eq!(
            h.element(attach, ElementEvent::Seeked(90_023)),
            vec![
                Command::Emit(Event::EnginePosition {
                    identity,
                    position_ms: 90_000
                }),
                Command::Emit(Event::EnginePosition {
                    identity,
                    position_ms: 90_023
                }),
            ]
        );
        // The confirmation is consumed: a later `seeked` without a request is just a position.
        assert_eq!(
            h.element(attach, ElementEvent::Seeked(91_000)),
            vec![Command::Emit(Event::EnginePosition {
                identity,
                position_ms: 91_000
            })]
        );
    }

    #[test]
    fn a_source_attached_at_a_position_confirms_that_position_when_the_element_lands() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        h.engine(Effect::SeekEngine, 30_000);
        h.engine(Effect::StartEngine, 30_000);
        let granted = h.answer(ticket, 120_000);
        let attach = attach_of(&granted);
        assert!(matches!(
            granted[0],
            Command::Attach {
                start_ms: 30_000,
                ..
            }
        ));
        let out = h.element(attach, ElementEvent::Seeked(30_010));
        assert!(matches!(
            out[0],
            Command::Emit(Event::EnginePosition {
                position_ms: 30_000,
                ..
            })
        ));
    }

    #[test]
    fn a_new_intent_gives_later_engine_facts_the_new_identity() {
        let (mut h, attach) = playing();
        let before = h.identity();
        assert_eq!(h.engine(Effect::PauseEngine, 4_000), vec![Command::Pause]);
        let after = h.identity();
        assert_ne!(before, after);
        assert_eq!(
            h.element(attach, ElementEvent::Paused),
            vec![Command::Emit(Event::EnginePaused { identity: after })]
        );
    }

    #[test]
    fn a_pause_fact_caused_by_an_older_command_cannot_cancel_a_new_play() {
        let (mut h, attach) = playing();
        h.engine(Effect::PauseEngine, 4_000);
        let resumed = h.engine(Effect::StartEngine, 4_000);
        assert!(matches!(resumed[0], Command::Play { from_ms: 4_000 }));
        assert!(h.element(attach, ElementEvent::Paused).is_empty());
        h.element(attach, ElementEvent::Playing);
        // Once playing, a pause nobody asked for is the system's: see the next test.
        assert_eq!(
            h.element(attach, ElementEvent::Paused),
            vec![Command::Emit(Event::PauseRequested)]
        );
    }

    #[test]
    fn a_pause_nobody_asked_for_becomes_the_listeners_pause_intent_and_our_own_is_only_acknowledged(
    ) {
        let (mut h, attach) = playing();
        // A media key or interruption paused the element behind the player's back.
        assert_eq!(
            h.element(attach, ElementEvent::Paused),
            vec![Command::Emit(Event::PauseRequested)]
        );
        // The player then asks for the pause itself; the element's own pause fact is just the
        // acknowledgement and never loops back into another request.
        assert_eq!(h.engine(Effect::PauseEngine, 1_000), vec![Command::Pause]);
        assert_eq!(
            h.element(attach, ElementEvent::Paused),
            vec![Command::Emit(Event::EnginePaused {
                identity: h.identity()
            })]
        );
    }

    #[test]
    fn seek_speed_and_replay_commands_follow_the_reducer_position() {
        let (mut h, _) = playing();
        assert_eq!(
            h.engine(Effect::SeekEngine, 30_000),
            vec![Command::Seek { to_ms: 30_000 }]
        );
        let speed = {
            h.generation += 1;
            let mut fast = command_for(h.release, h.generation, 30_000);
            fast.speed_permille = 1500;
            h.driver
                .step(h.now, Input::Engine(Box::new(Effect::SetSpeed(fast))))
        };
        assert_eq!(speed, vec![Command::Speed { permille: 1500 }]);
        h.engine(Effect::PauseEngine, 30_000);
        let replay = h.engine(Effect::StartEngine, 0);
        assert!(matches!(replay[0], Command::Play { from_ms: 0 }));
    }

    #[test]
    fn an_authorization_for_another_release_stops_instead_of_switching_audio() {
        for (name, edit) in [
            (
                "publication",
                (|a: &mut PlaybackAuthorization| a.publication_id = P2.into()) as fn(&mut _),
            ),
            ("rendition", |a| a.rendition_id = R2.into()),
            ("sha", |a| a.sha256 = SHA2.into()),
        ] {
            let mut h = Harness::new(first());
            let ticket = ticket_of(&h.prepare());
            let mut changed = authorization(first(), h.now, 120_000);
            edit(&mut changed);
            let out = h.driver.step(
                h.now,
                Input::Authorized {
                    ticket,
                    result: Ok(changed),
                },
            );
            assert_eq!(failure_of(&out), Some(Failure::IdentityChanged), "{name}");
            assert!(out.contains(&Command::Release), "{name}");
            assert!(
                !out.iter().any(|c| matches!(c, Command::Attach { .. })),
                "{name}"
            );
        }
    }

    #[test]
    fn a_withdrawn_release_stops_at_once_without_a_second_request() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        let out = h.fail_with(ticket, AuthorizationError::NotFound);
        assert_eq!(failure_of(&out), Some(Failure::Unavailable));
        assert_eq!(authorizations(&out), 0);
        // Nothing the engine reports afterwards revives it.
        assert!(h.answer(ticket, 120_000).is_empty());
        assert!(h
            .driver
            .step(h.now + 60_000, Input::Timer(ticket))
            .is_empty());
    }

    #[test]
    fn retryable_answers_back_off_a_bounded_number_of_times_then_report_a_network_failure() {
        let mut h = Harness::new(first());
        let mut out = h.prepare();
        let mut requests = authorizations(&out);
        let mut delays = Vec::new();
        loop {
            out = h.fail_with(ticket_of(&out), AuthorizationError::Unavailable);
            if let Some(failure) = failure_of(&out) {
                assert_eq!(failure, Failure::Network);
                break;
            }
            let Command::Wait { ticket, delay_ms } = out[0] else {
                panic!("{out:?}")
            };
            delays.push(delay_ms);
            out = h.driver.step(h.now, Input::Timer(ticket));
            requests += authorizations(&out);
        }
        assert_eq!(delays, vec![1_000, 2_000, 4_000]);
        assert_eq!(requests, 4);
    }

    #[test]
    fn contract_violations_are_reported_not_retried() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        let out = h.fail_with(ticket, AuthorizationError::Protocol);
        assert_eq!(failure_of(&out), Some(Failure::Protocol));
        assert_eq!(authorizations(&out), 0);
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        let mut expired = authorization(first(), h.now, 120_000);
        expired.expires_at_ms = h.now;
        let out = h.driver.step(
            h.now,
            Input::Authorized {
                ticket,
                result: Ok(expired),
            },
        );
        assert_eq!(failure_of(&out), Some(Failure::Protocol));
    }

    #[test]
    fn refresh_asks_again_for_the_same_publication_and_never_reattaches() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        h.engine(Effect::StartEngine, 0);
        let granted = h.answer(ticket, 120_000);
        let attach = attach_of(&granted);
        let Command::Wait {
            ticket: timer,
            delay_ms,
        } = granted[1]
        else {
            panic!("{granted:?}")
        };
        assert_eq!(delay_ms, 90_000);
        h.now += i64::from(delay_ms);
        let refresh = h.driver.step(h.now, Input::Timer(timer));
        assert_eq!(
            refresh,
            vec![Command::Authorize {
                ticket: ticket_of(&refresh),
                publication: first().track().publication()
            }]
        );
        let renewed = h.answer(ticket_of(&refresh), 120_000);
        assert!(
            !renewed.iter().any(|c| matches!(c, Command::Attach { .. })),
            "{renewed:?}"
        );
        assert!(matches!(
            renewed[0],
            Command::Wait {
                delay_ms: 90_000,
                ..
            }
        ));
        // The element still belongs to the first attachment.
        assert_eq!(h.element(attach, ElementEvent::Playing).len(), 1);
    }

    #[test]
    fn a_refresh_that_finds_the_release_withdrawn_stops_playback() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        h.engine(Effect::StartEngine, 0);
        let granted = h.answer(ticket, 120_000);
        let Command::Wait {
            ticket: timer,
            delay_ms,
        } = granted[1]
        else {
            panic!("{granted:?}")
        };
        h.now += i64::from(delay_ms);
        let refresh = h.driver.step(h.now, Input::Timer(timer));
        let out = h.fail_with(ticket_of(&refresh), AuthorizationError::NotFound);
        assert_eq!(failure_of(&out), Some(Failure::Unavailable));
        assert!(out.contains(&Command::Release));
    }

    #[test]
    fn a_refresh_that_keeps_failing_leaves_the_working_source_alone() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        h.engine(Effect::StartEngine, 0);
        let granted = h.answer(ticket, 120_000);
        let Command::Wait {
            ticket: timer,
            delay_ms,
        } = granted[1]
        else {
            panic!("{granted:?}")
        };
        h.now += i64::from(delay_ms);
        let mut out = h.driver.step(h.now, Input::Timer(timer));
        let mut requests = authorizations(&out);
        loop {
            out = h.fail_with(ticket_of(&out), AuthorizationError::Unavailable);
            assert_eq!(failure_of(&out), None);
            let Some(Command::Wait { ticket, .. }) = out.first().cloned() else {
                break;
            };
            out = h.driver.step(h.now, Input::Timer(ticket));
            requests += authorizations(&out);
        }
        assert_eq!(requests, 4);
    }

    #[test]
    fn an_element_failure_reattaches_a_fresher_grant_at_the_last_position() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        h.engine(Effect::StartEngine, 0);
        let granted = h.answer(ticket, 120_000);
        let attach = attach_of(&granted);
        let Command::Wait {
            ticket: timer,
            delay_ms,
        } = granted[1]
        else {
            panic!("{granted:?}")
        };
        h.element(attach, ElementEvent::Playing);
        h.element(attach, ElementEvent::Position(75_000));
        h.now += i64::from(delay_ms);
        let refresh = h.driver.step(h.now, Input::Timer(timer));
        h.answer(ticket_of(&refresh), 120_000);
        // The old URL expires; the element reports it cannot continue.
        h.now += 40_000;
        let recovered = h.element(attach, ElementEvent::Error(MediaFault::Unsupported));
        match &recovered[..] {
            [Command::Attach {
                attach: next,
                start_ms,
                play,
                ..
            }] => {
                assert_ne!(*next, attach);
                assert_eq!((*start_ms, *play), (75_000, true));
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn an_element_failure_without_a_fresher_grant_asks_again_then_resumes_in_place() {
        let (mut h, attach) = playing();
        h.element(attach, ElementEvent::Position(12_000));
        h.now += 200_000;
        let asked = h.element(attach, ElementEvent::Error(MediaFault::Network));
        assert_eq!(authorizations(&asked), 1);
        let reattached = h.answer(ticket_of(&asked), 120_000);
        let next = attach_of(&reattached);
        assert!(matches!(
            reattached[0],
            Command::Attach {
                start_ms: 12_000,
                play: true,
                ..
            }
        ));
        // The first attachment is dead: its late facts change nothing.
        assert!(h.element(attach, ElementEvent::Position(99_000)).is_empty());
        assert_eq!(h.element(next, ElementEvent::Position(12_500)).len(), 1);
    }

    #[test]
    fn a_decode_error_is_reported_without_reauthorizing() {
        let (mut h, attach) = playing();
        let out = h.element(attach, ElementEvent::Error(MediaFault::Decode));
        assert_eq!(failure_of(&out), Some(Failure::Decode));
        assert_eq!(authorizations(&out), 0);
    }

    #[test]
    fn recovery_is_bounded_without_progress_and_reset_by_it() {
        let (mut h, mut attach) = playing();
        for round in 1..=3 {
            h.now += 200_000;
            let asked = h.element(attach, ElementEvent::Error(MediaFault::Unsupported));
            assert_eq!(authorizations(&asked), 1, "round {round}");
            attach = attach_of(&h.answer(ticket_of(&asked), 120_000));
        }
        h.now += 200_000;
        let stopped = h.element(attach, ElementEvent::Error(MediaFault::Unsupported));
        assert_eq!(failure_of(&stopped), Some(Failure::Network));
        assert_eq!(authorizations(&stopped), 0);

        // Audible progress proves the last recovery worked and renews the budget.
        let (mut h, mut attach) = playing();
        for round in 1..=6 {
            h.now += 200_000;
            let asked = h.element(attach, ElementEvent::Error(MediaFault::Network));
            assert_eq!(authorizations(&asked), 1, "round {round}");
            attach = attach_of(&h.answer(ticket_of(&asked), 120_000));
            h.element(attach, ElementEvent::Playing);
            h.element(attach, ElementEvent::Position(round * 30_000));
        }
    }

    #[test]
    fn a_refused_play_keeps_the_element_so_a_gesture_can_start_it() {
        let (mut h, attach) = playing();
        let out = h.element(
            attach,
            ElementEvent::PlayRejected(PlayRejection::NotAllowed),
        );
        assert_eq!(failure_of(&out), Some(Failure::PlayRefused));
        assert!(out.contains(&Command::Pause) && !out.contains(&Command::Release));
        let again = h.engine(Effect::StartEngine, 0);
        assert!(matches!(again[0], Command::Play { .. }), "{again:?}");
        assert_eq!(authorizations(&again), 0);
        // A play superseded by a newer load or pause is not a failure.
        assert!(h
            .element(attach, ElementEvent::PlayRejected(PlayRejection::Aborted))
            .is_empty());
    }

    #[test]
    fn a_failed_source_restarts_only_on_a_new_start_intent() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        h.fail_with(ticket, AuthorizationError::Protocol);
        assert!(h.engine(Effect::SeekEngine, 5_000).is_empty());
        let restart = h.engine(Effect::StartEngine, 5_000);
        assert_eq!(authorizations(&restart), 1);
        let attached = h.answer(ticket_of(&restart), 120_000);
        assert!(matches!(
            attached[0],
            Command::Attach {
                start_ms: 5_000,
                play: true,
                ..
            }
        ));
    }

    #[test]
    fn a_new_source_releases_the_old_one_and_fences_every_late_answer_and_fact() {
        let mut h = Harness::new(first());
        let old_request = ticket_of(&h.prepare());
        let old_attach = attach_of(&h.answer(old_request, 120_000));
        let stale_request = {
            h.engine(Effect::StartEngine, 0);
            let wait = h.engine(Effect::PauseEngine, 0);
            drop(wait);
            old_request
        };
        // The listener picks the republished release.
        h.release = republished();
        let prepared = h.prepare();
        assert_eq!(prepared[0], Command::Release);
        assert!(matches!(
            &prepared[1],
            Command::Authorize { publication, .. } if *publication == republished().track().publication()
        ));
        // Late answers and element facts of the first release change nothing.
        assert!(h
            .driver
            .step(
                h.now,
                Input::Authorized {
                    ticket: stale_request,
                    result: Ok(authorization(first(), h.now, 120_000)),
                },
            )
            .is_empty());
        assert!(h.element(old_attach, ElementEvent::Playing).is_empty());
        assert!(h.element(old_attach, ElementEvent::Position(1)).is_empty());
        // The new release's own grant attaches normally, for the new pin only.
        let attach = attach_of(&h.answer(ticket_of(&prepared), 120_000));
        assert!(h.driver.holds(republished().track()) && !h.driver.holds(first().track()));
        assert_ne!(attach, old_attach);
    }

    #[test]
    fn a_withdrawn_release_never_falls_back_to_its_replacement() {
        let mut h = Harness::new(first());
        let ticket = ticket_of(&h.prepare());
        let out = h.fail_with(ticket, AuthorizationError::NotFound);
        assert_eq!(failure_of(&out), Some(Failure::Unavailable));
        // Only a listener-selected new source (a different pin) starts anything.
        assert!(!h
            .driver
            .step(h.now, Input::Timer(ticket))
            .iter()
            .any(|c| matches!(c, Command::Authorize { .. })));
        assert!(h.driver.holds(first().track()));
    }

    #[test]
    fn fixture_effects_and_stray_commands_are_ignored() {
        let mut driver = MediaDriver::new();
        let fixture = Source::new(
            crate::core::ids::EpisodeId {
                work: crate::core::ids::WorkId::River,
                index: 0,
            },
            60_000,
        )
        .unwrap();
        let command = EngineCommand {
            identity: CallbackIdentity {
                track: fixture.track(),
                generation: 1,
            },
            source: fixture,
            position_ms: 0,
            speed_permille: 1000,
        };
        for effect in [Effect::PrepareEngine(command), Effect::StartEngine(command)] {
            assert!(driver.step(T0, Input::Engine(Box::new(effect))).is_empty());
        }
        // A start for a public release that was never prepared cannot begin a source.
        let stray = Effect::StartEngine(command_for(first(), 2, 0));
        assert!(driver.step(T0, Input::Engine(Box::new(stray))).is_empty());
    }

    #[test]
    fn no_sequence_of_answers_issues_more_requests_than_the_budget_allows() {
        // Every answer is a retryable failure and every timer fires at once.
        let mut h = Harness::new(first());
        let mut out = h.prepare();
        let mut requests = authorizations(&out);
        for _ in 0..100 {
            let next = match out.first().cloned() {
                Some(Command::Authorize { ticket, .. }) => {
                    h.fail_with(ticket, AuthorizationError::Unavailable)
                }
                Some(Command::Wait { ticket, .. }) => h.driver.step(h.now, Input::Timer(ticket)),
                _ => break,
            };
            requests += authorizations(&next);
            out = next;
        }
        assert_eq!(requests, usize::from(MAX_ATTEMPTS));
    }
}
