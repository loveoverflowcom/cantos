//! Pure revisioned listener registers. No Axum, SQL, runtime, clocks or authentication effects.
use crate::contract::theatre::library as dto;
use crate::{
    contexts::catalog::domain::{ContentUnitId, WorkId},
    platform::{identity::domain::ActorId, ids::canonical_uuid},
};
pub mod bookmark;
pub(crate) mod projection;
pub mod reconcile;
pub use bookmark::{
    reconcile_bookmark_create, reconcile_bookmark_delete, BookmarkCandidate, BookmarkCreateOutcome,
    BookmarkDeleteOutcome, BookmarkDeletion, BookmarkRecord,
};
pub use reconcile::{classify, successor, Admission};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DomainError {
    InvalidId,
    InvalidRevision,
    InvalidSequence,
    InvalidPosition,
    InvalidTime,
    BindingMismatch,
    /// One identifier names one immutable value; a different body under it is never accepted.
    IdentityReused,
    RevisionExhausted,
}
macro_rules! uuid_id {
    ($name:ident) => {
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub struct $name(String);
        impl $name {
            pub fn parse(raw: &str) -> Result<Self, DomainError> {
                canonical_uuid(raw).map(Self).ok_or(DomainError::InvalidId)
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
uuid_id!(OperationId);
uuid_id!(DeviceId);
uuid_id!(PublicationId);
uuid_id!(RenditionId);
uuid_id!(EngineSessionId);
uuid_id!(BookmarkId);
/// The account scope of listener state: a registry actor acting as a listener, never a catalog
/// owner. Parsing identity does not authenticate. Only an authenticator may bind this to a principal.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ListenerId(ActorId);
impl ListenerId {
    pub(crate) fn from_actor(actor: ActorId) -> Self {
        Self(actor)
    }
    pub fn as_str(&self) -> &str {
        self.0.as_str()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Revision(u64);
impl Revision {
    pub fn new(raw: u64) -> Result<Self, DomainError> {
        if raw > i64::MAX as u64 {
            return Err(DomainError::InvalidRevision);
        }
        Ok(Self(raw))
    }
    pub fn value(self) -> u64 {
        self.0
    }
    pub fn next(self) -> Result<Self, DomainError> {
        successor(self.0).map(Self)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Intent {
    operation: OperationId,
    device: DeviceId,
    sequence: u64,
    reported_at: Option<i64>,
}
impl Intent {
    pub fn new(raw: &dto::Operation) -> Result<Self, DomainError> {
        if raw.device_sequence == 0 || raw.device_sequence > i64::MAX as u64 {
            return Err(DomainError::InvalidSequence);
        }
        if raw.device_reported_at_ms.is_some_and(|v| v < 0) {
            return Err(DomainError::InvalidTime);
        }
        Ok(Self {
            operation: OperationId::parse(&raw.operation_id)?,
            device: DeviceId::parse(&raw.device_id)?,
            sequence: raw.device_sequence,
            reported_at: raw.device_reported_at_ms,
        })
    }
    pub fn operation(&self) -> &OperationId {
        &self.operation
    }
    pub fn device(&self) -> &DeviceId {
        &self.device
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn reported_at(&self) -> Option<i64> {
        self.reported_at
    }
}
/// Exact audio identity is a value, never an access grant.
/// ```compile_fail,E0277
/// use cantos_server::contexts::listening::domain::AudioIdentity;
/// let forged = serde_json::from_str::<AudioIdentity>("{}");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AudioIdentity {
    work: WorkId,
    unit: ContentUnitId,
    publication: PublicationId,
    rendition: RenditionId,
    sha256: String,
    duration: u64,
}
impl AudioIdentity {
    pub fn new(raw: &dto::AudioIdentity) -> Result<Self, DomainError> {
        if raw.duration_ms == 0 || raw.duration_ms > i64::MAX as u64 {
            return Err(DomainError::InvalidPosition);
        }
        if raw.sha256.len() != 64
            || !raw
                .sha256
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(DomainError::BindingMismatch);
        }
        Ok(Self {
            work: WorkId::parse(&raw.work_id).map_err(|_| DomainError::InvalidId)?,
            unit: ContentUnitId::parse(&raw.unit_id).map_err(|_| DomainError::InvalidId)?,
            publication: PublicationId::parse(&raw.publication_id)?,
            rendition: RenditionId::parse(&raw.rendition_id)?,
            sha256: raw.sha256.clone(),
            duration: raw.duration_ms,
        })
    }
    pub fn work(&self) -> &WorkId {
        &self.work
    }
    pub fn unit(&self) -> &ContentUnitId {
        &self.unit
    }
    pub fn publication(&self) -> &PublicationId {
        &self.publication
    }
    pub fn duration(&self) -> u64 {
        self.duration
    }
    pub fn raw(&self) -> dto::AudioIdentity {
        dto::AudioIdentity {
            work_id: self.work.as_str().into(),
            unit_id: self.unit.as_str().into(),
            publication_id: self.publication.as_str().into(),
            rendition_id: self.rendition.as_str().into(),
            sha256: self.sha256.clone(),
            duration_ms: self.duration,
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressCandidate {
    intent: Intent,
    audio: AudioIdentity,
    base: Revision,
    position: u64,
    kind: dto::ProgressKind,
}
impl ProgressCandidate {
    pub fn new(raw: &dto::ProgressWrite) -> Result<Self, DomainError> {
        let audio = AudioIdentity::new(&raw.audio)?;
        if raw.position_ms > audio.duration()
            || (raw.kind == dto::ProgressKind::Restart && raw.position_ms != 0)
            || (raw.kind == dto::ProgressKind::MarkCompleted && raw.position_ms != audio.duration())
        {
            return Err(DomainError::InvalidPosition);
        }
        Ok(Self {
            intent: Intent::new(&raw.operation)?,
            audio,
            base: Revision::new(raw.base_revision)?,
            position: raw.position_ms,
            kind: raw.kind,
        })
    }
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    pub fn audio(&self) -> &AudioIdentity {
        &self.audio
    }
    pub fn base(&self) -> Revision {
        self.base
    }
    pub fn position(&self) -> u64 {
        self.position
    }
    pub fn kind(&self) -> dto::ProgressKind {
        self.kind
    }
}
/// Only validated persisted state or reconcile constructs accepted progress.
/// ```compile_fail,E0616
/// use cantos_server::contexts::listening::domain::ProgressRecord;
/// fn forge(record: &mut ProgressRecord) { record.revision = 999; }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProgressRecord(dto::ProgressRecord);
impl ProgressRecord {
    pub(crate) fn restore(raw: dto::ProgressRecord) -> Result<Self, DomainError> {
        let audio = AudioIdentity::new(&raw.audio)?;
        DeviceId::parse(&raw.written_by)?;
        if raw.position_ms > audio.duration()
            || raw.accepted_at_ms < 0
            || raw.revision == 0
            || raw.listener_order == 0
        {
            return Err(DomainError::InvalidPosition);
        }
        Revision::new(raw.revision)?;
        Revision::new(raw.listener_order)?;
        if raw.kind == dto::ProgressKind::Restart
            && (raw.completion == dto::Completion::Completed || raw.position_ms != 0)
        {
            return Err(DomainError::BindingMismatch);
        }
        if raw.kind == dto::ProgressKind::MarkCompleted
            && (raw.position_ms != audio.duration() || raw.completion != dto::Completion::Completed)
        {
            return Err(DomainError::BindingMismatch);
        }
        Ok(Self(raw))
    }
    pub fn raw(&self) -> &dto::ProgressRecord {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressOutcome {
    Accepted(ProgressRecord),
    AlreadyCurrent(ProgressRecord),
    Conflict {
        current: Option<ProgressRecord>,
        resolution: dto::Resolution,
    },
}
pub fn reconcile_progress(
    current: Option<&ProgressRecord>,
    candidate: &ProgressCandidate,
    now: i64,
    order: Revision,
) -> Result<ProgressOutcome, DomainError> {
    if now < 0 || order.value() == 0 {
        return Err(DomainError::InvalidTime);
    }
    if current.is_some_and(|c| c.0.audio != candidate.audio.raw()) {
        return Err(DomainError::BindingMismatch);
    }
    let completion = match candidate.kind {
        dto::ProgressKind::Restart => dto::Completion::InProgress,
        dto::ProgressKind::MarkCompleted => dto::Completion::Completed,
        dto::ProgressKind::Checkpoint
        | dto::ProgressKind::ExplicitSeek
        | dto::ProgressKind::ResolveHere
        | dto::ProgressKind::ResolveOther => {
            current.map_or(dto::Completion::InProgress, |c| c.0.completion)
        }
    };
    let revision = current.map_or(0, |v| v.0.revision);
    let same = current
        .is_some_and(|v| v.0.position_ms == candidate.position && v.0.completion == completion);
    let protected = current.is_some_and(|v| v.0.kind != dto::ProgressKind::Checkpoint);
    match classify(
        candidate.base.value(),
        revision,
        same,
        protected,
        candidate.kind == dto::ProgressKind::Checkpoint,
    )? {
        Admission::Accept(next) => Ok(ProgressOutcome::Accepted(ProgressRecord::restore(
            dto::ProgressRecord {
                audio: candidate.audio.raw(),
                position_ms: candidate.position,
                completion,
                revision: next,
                accepted_at_ms: now,
                listener_order: order.value(),
                written_by: candidate.intent.device.as_str().into(),
                kind: candidate.kind,
            },
        )?)),
        Admission::AlreadyCurrent => current
            .cloned()
            .map(ProgressOutcome::AlreadyCurrent)
            .ok_or(DomainError::BindingMismatch),
        Admission::ListenerChooses | Admission::KeepServer => Ok(ProgressOutcome::Conflict {
            current: current.cloned(),
            resolution: if protected && candidate.kind == dto::ProgressKind::Checkpoint {
                dto::Resolution::KeepServer
            } else {
                dto::Resolution::ListenerChooses
            },
        }),
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedCandidate {
    intent: Intent,
    work: WorkId,
    base: Revision,
    saved: bool,
}
impl SavedCandidate {
    pub fn new(work: &str, raw: &dto::SavedWrite) -> Result<Self, DomainError> {
        Ok(Self {
            intent: Intent::new(&raw.operation)?,
            work: WorkId::parse(work).map_err(|_| DomainError::InvalidId)?,
            base: Revision::new(raw.base_revision)?,
            saved: raw.saved,
        })
    }
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    pub fn work(&self) -> &WorkId {
        &self.work
    }
    pub fn desired(&self) -> bool {
        self.saved
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedRecord(dto::SavedRecord);
impl SavedRecord {
    pub fn empty(work: &WorkId) -> Self {
        Self(dto::SavedRecord {
            work_id: work.as_str().into(),
            saved: false,
            revision: 0,
            listener_order: 0,
            accepted_at_ms: 0,
        })
    }
    pub(crate) fn restore(raw: dto::SavedRecord) -> Result<Self, DomainError> {
        WorkId::parse(&raw.work_id).map_err(|_| DomainError::InvalidId)?;
        Revision::new(raw.revision)?;
        Revision::new(raw.listener_order)?;
        if raw.accepted_at_ms < 0
            || (raw.revision == 0
                && (raw.saved || raw.listener_order != 0 || raw.accepted_at_ms != 0))
            || (raw.revision > 0 && raw.listener_order == 0)
        {
            return Err(DomainError::BindingMismatch);
        }
        Ok(Self(raw))
    }
    pub fn raw(&self) -> &dto::SavedRecord {
        &self.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SavedOutcome {
    Accepted(SavedRecord),
    AlreadyCurrent(SavedRecord),
    Conflict(SavedRecord),
}
pub fn reconcile_saved(
    current: &SavedRecord,
    candidate: &SavedCandidate,
    now: i64,
    order: Revision,
) -> Result<SavedOutcome, DomainError> {
    if current.0.work_id != candidate.work.as_str() {
        return Err(DomainError::BindingMismatch);
    }
    if now < 0 || order.value() == 0 {
        return Err(DomainError::InvalidTime);
    }
    if current.0.saved == candidate.saved {
        return Ok(SavedOutcome::AlreadyCurrent(current.clone()));
    }
    if current.0.revision != candidate.base.value() {
        return Ok(SavedOutcome::Conflict(current.clone()));
    }
    Ok(SavedOutcome::Accepted(SavedRecord::restore(
        dto::SavedRecord {
            work_id: candidate.work.as_str().into(),
            saved: candidate.saved,
            revision: successor(current.0.revision)?,
            listener_order: order.value(),
            accepted_at_ms: now,
        },
    )?))
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryStart {
    intent: Intent,
    audio: AudioIdentity,
    session: EngineSessionId,
    epoch: Revision,
}
impl HistoryStart {
    pub fn new(raw: &dto::HistoryStart) -> Result<Self, DomainError> {
        // The closed wire event only admits playback_started, never intent/detail/failed events.
        Ok(Self {
            intent: Intent::new(&raw.operation)?,
            audio: AudioIdentity::new(&raw.audio)?,
            session: EngineSessionId::parse(&raw.engine_session_id)?,
            epoch: Revision::new(raw.history_epoch)?,
        })
    }
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    pub fn audio(&self) -> &AudioIdentity {
        &self.audio
    }
    pub fn session(&self) -> &EngineSessionId {
        &self.session
    }
    pub fn epoch(&self) -> Revision {
        self.epoch
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HistoryClear {
    intent: Intent,
    base: Revision,
    epoch: Revision,
}
impl HistoryClear {
    pub fn new(raw: &dto::HistoryClear) -> Result<Self, DomainError> {
        Ok(Self {
            intent: Intent::new(&raw.operation)?,
            base: Revision::new(raw.base_revision)?,
            epoch: Revision::new(raw.history_epoch)?,
        })
    }
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    pub fn base(&self) -> Revision {
        self.base
    }
    pub fn epoch(&self) -> Revision {
        self.epoch
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryState {
    revision: Revision,
    epoch: Revision,
}
impl HistoryState {
    pub fn restore(revision: u64, epoch: u64) -> Result<Self, DomainError> {
        if epoch > revision {
            return Err(DomainError::BindingMismatch);
        }
        Ok(Self {
            revision: Revision::new(revision)?,
            epoch: Revision::new(epoch)?,
        })
    }
    pub fn raw(self) -> dto::HistoryState {
        dto::HistoryState {
            revision: self.revision.value(),
            epoch: self.epoch.value(),
        }
    }
    pub fn start(
        self,
        candidate: &HistoryStart,
        seen_session: bool,
    ) -> Result<HistoryOutcome, DomainError> {
        if candidate.epoch != self.epoch {
            return Ok(HistoryOutcome::EpochConflict(self));
        }
        if seen_session {
            return Ok(HistoryOutcome::AlreadyCurrent(self));
        }
        Ok(HistoryOutcome::Accepted(Self {
            revision: self.revision.next()?,
            epoch: self.epoch,
        }))
    }
    pub fn clear(self, candidate: &HistoryClear) -> Result<HistoryOutcome, DomainError> {
        if candidate.epoch != self.epoch {
            return Ok(HistoryOutcome::EpochConflict(self));
        }
        if candidate.base != self.revision {
            return Ok(HistoryOutcome::RevisionConflict(self));
        }
        Ok(HistoryOutcome::Accepted(Self {
            revision: self.revision.next()?,
            epoch: self.epoch.next()?,
        }))
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HistoryOutcome {
    Accepted(HistoryState),
    AlreadyCurrent(HistoryState),
    RevisionConflict(HistoryState),
    EpochConflict(HistoryState),
}
/// Read-only resume classification never substitutes an audio identity or starts an engine.
pub fn resume_status(
    stored: &AudioIdentity,
    exact: Option<&AudioIdentity>,
    current_unit_audio: Option<&AudioIdentity>,
) -> dto::ResumeStatus {
    if exact == Some(stored) {
        dto::ResumeStatus::ExactAvailable
    } else if current_unit_audio
        .is_some_and(|a| a.unit() == stored.unit() && a.publication() != stored.publication())
    {
        dto::ResumeStatus::ReleaseReplaced
    } else {
        dto::ResumeStatus::AccessUnavailable
    }
}
#[cfg(test)]
mod tests;
