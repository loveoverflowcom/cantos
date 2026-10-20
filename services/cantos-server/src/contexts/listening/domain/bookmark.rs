//! Release-bound bookmarks: immutable `(id, exact audio, position)` values with a final tombstone.
//! Pure decisions only; ownership, the public-audio admission and storage belong to the shell.
use super::{AudioIdentity, BookmarkId, DomainError, Intent, Revision};
use cantos_contracts::listener_library as dto;

/// A create intent. The ID is client-minted so a retry names the same bookmark; the account is
/// never a field. The position is already within the identity's verified duration.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkCandidate {
    intent: Intent,
    id: BookmarkId,
    audio: AudioIdentity,
    position: u64,
}
impl BookmarkCandidate {
    pub fn new(path_id: &str, raw: &dto::BookmarkWrite) -> Result<Self, DomainError> {
        let id = BookmarkId::parse(&raw.bookmark_id)?;
        if id.as_str() != path_id {
            return Err(DomainError::BindingMismatch);
        }
        let audio = AudioIdentity::new(&raw.audio)?;
        if raw.position_ms > audio.duration() {
            return Err(DomainError::InvalidPosition);
        }
        Ok(Self {
            intent: Intent::new(&raw.operation)?,
            id,
            audio,
            position: raw.position_ms,
        })
    }
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    pub fn id(&self) -> &BookmarkId {
        &self.id
    }
    pub fn audio(&self) -> &AudioIdentity {
        &self.audio
    }
    pub fn position(&self) -> u64 {
        self.position
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkDeletion {
    intent: Intent,
    id: BookmarkId,
}
impl BookmarkDeletion {
    pub fn new(path_id: &str, raw: &dto::BookmarkDelete) -> Result<Self, DomainError> {
        Ok(Self {
            intent: Intent::new(&raw.operation)?,
            id: BookmarkId::parse(path_id)?,
        })
    }
    pub fn intent(&self) -> &Intent {
        &self.intent
    }
    pub fn id(&self) -> &BookmarkId {
        &self.id
    }
}

/// Only validated persisted state or a reconcile decision constructs a bookmark record.
/// ```compile_fail,E0616
/// use cantos_server::contexts::listening::domain::BookmarkRecord;
/// fn forge(record: &mut BookmarkRecord) { record.deleted = false; }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BookmarkRecord(dto::BookmarkRecord);
impl BookmarkRecord {
    pub(crate) fn restore(raw: dto::BookmarkRecord) -> Result<Self, DomainError> {
        BookmarkId::parse(&raw.bookmark_id)?;
        let audio = AudioIdentity::new(&raw.audio)?;
        if raw.position_ms > audio.duration() || raw.accepted_at_ms < 0 || raw.listener_order == 0 {
            return Err(DomainError::InvalidPosition);
        }
        Revision::new(raw.listener_order)?;
        Ok(Self(raw))
    }
    pub fn raw(&self) -> &dto::BookmarkRecord {
        &self.0
    }
    pub fn id(&self) -> &str {
        &self.0.bookmark_id
    }
    pub fn deleted(&self) -> bool {
        self.0.deleted
    }
    pub fn audio(&self) -> Result<AudioIdentity, DomainError> {
        AudioIdentity::new(&self.0.audio)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BookmarkCreateOutcome {
    Accepted(BookmarkRecord),
    /// The same live value already exists under this ID; nothing is duplicated or rewritten.
    AlreadyCurrent(BookmarkRecord),
    /// The ID's tombstone is final: no create, replay or restore brings the bookmark back.
    Deleted(BookmarkRecord),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BookmarkDeleteOutcome {
    Accepted(BookmarkRecord),
    AlreadyCurrent(BookmarkRecord),
    /// Nothing was ever accepted under this ID, so there is nothing to tombstone.
    Unknown,
}

pub fn reconcile_bookmark_create(
    current: Option<&BookmarkRecord>,
    candidate: &BookmarkCandidate,
    now: i64,
    order: Revision,
) -> Result<BookmarkCreateOutcome, DomainError> {
    if now < 0 || order.value() == 0 {
        return Err(DomainError::InvalidTime);
    }
    match current {
        None => Ok(BookmarkCreateOutcome::Accepted(BookmarkRecord::restore(
            dto::BookmarkRecord {
                bookmark_id: candidate.id.as_str().into(),
                audio: candidate.audio.raw(),
                position_ms: candidate.position,
                deleted: false,
                listener_order: order.value(),
                accepted_at_ms: now,
            },
        )?)),
        Some(record) if record.0.deleted => Ok(BookmarkCreateOutcome::Deleted(record.clone())),
        Some(record)
            if record.0.audio == candidate.audio.raw()
                && record.0.position_ms == candidate.position =>
        {
            Ok(BookmarkCreateOutcome::AlreadyCurrent(record.clone()))
        }
        // An ID names exactly one immutable value; a different body under it is a client bug.
        Some(_) => Err(DomainError::IdentityReused),
    }
}

pub fn reconcile_bookmark_delete(
    current: Option<&BookmarkRecord>,
    candidate: &BookmarkDeletion,
    now: i64,
    order: Revision,
) -> Result<BookmarkDeleteOutcome, DomainError> {
    if now < 0 || order.value() == 0 {
        return Err(DomainError::InvalidTime);
    }
    match current {
        None => Ok(BookmarkDeleteOutcome::Unknown),
        Some(record) if record.0.bookmark_id != candidate.id.as_str() => {
            Err(DomainError::BindingMismatch)
        }
        Some(record) if record.0.deleted => {
            Ok(BookmarkDeleteOutcome::AlreadyCurrent(record.clone()))
        }
        Some(record) => Ok(BookmarkDeleteOutcome::Accepted(BookmarkRecord::restore(
            dto::BookmarkRecord {
                deleted: true,
                listener_order: order.value(),
                accepted_at_ms: now,
                ..record.0.clone()
            },
        )?)),
    }
}
