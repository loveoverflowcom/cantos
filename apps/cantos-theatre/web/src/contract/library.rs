//! Library/Progress v1 raw wire boundary. Authentication and refinements belong to the server.
use serde::{Deserialize, Deserializer, Serialize};

// Option means nullable, not an omitted member, for this strict versioned contract.
fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Operation {
    pub operation_id: String,
    pub device_id: String,
    pub device_sequence: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub device_reported_at_ms: Option<i64>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioIdentity {
    pub work_id: String,
    pub unit_id: String,
    pub publication_id: String,
    pub rendition_id: String,
    pub sha256: String,
    pub duration_ms: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ProgressKind {
    Checkpoint,
    ExplicitSeek,
    Restart,
    ResolveHere,
    ResolveOther,
    MarkCompleted,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Completion {
    InProgress,
    Completed,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProgressWrite {
    pub operation: Operation,
    pub audio: AudioIdentity,
    pub base_revision: u64,
    pub position_ms: u64,
    pub kind: ProgressKind,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProgressRecord {
    pub audio: AudioIdentity,
    pub position_ms: u64,
    pub completion: Completion,
    pub revision: u64,
    pub accepted_at_ms: i64,
    pub listener_order: u64,
    pub written_by: String,
    pub kind: ProgressKind,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedWrite {
    pub operation: Operation,
    pub base_revision: u64,
    pub saved: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedRecord {
    pub work_id: String,
    pub saved: bool,
    pub revision: u64,
    pub listener_order: u64,
    pub accepted_at_ms: i64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum PlaybackEvent {
    PlaybackStarted,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryStart {
    pub operation: Operation,
    pub audio: AudioIdentity,
    pub engine_session_id: String,
    pub event: PlaybackEvent,
    pub history_epoch: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ClearConfirmation {
    ClearHistory,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryClear {
    pub operation: Operation,
    pub base_revision: u64,
    pub history_epoch: u64,
    pub confirmation: ClearConfirmation,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryState {
    pub revision: u64,
    pub epoch: u64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ResumeStatus {
    ExactAvailable,
    ReleaseReplaced,
    AccessUnavailable,
}
/// Released public metadata, never the current editable Work head.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkPresentation {
    pub work_id: String,
    pub publication_id: String,
    pub title: String,
    pub kind: super::discovery::WorkKind,
    pub language: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct AudioPresentation {
    pub work: WorkPresentation,
    pub unit_id: String,
    pub unit_title: String,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedContinueItem {
    pub progress: ProgressRecord,
    pub resume: ResumeStatus,
    #[serde(deserialize_with = "required_nullable")]
    pub presentation: Option<AudioPresentation>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedSavedItem {
    pub saved: SavedRecord,
    #[serde(deserialize_with = "required_nullable")]
    pub presentation: Option<WorkPresentation>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedHistoryItem {
    pub audio: AudioIdentity,
    pub started_at_ms: i64,
    pub listener_order: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub progress: Option<ProgressRecord>,
    pub resume: ResumeStatus,
    #[serde(deserialize_with = "required_nullable")]
    pub presentation: Option<AudioPresentation>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedContinuePage {
    pub items: Vec<PresentedContinueItem>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedSavedPage {
    pub items: Vec<PresentedSavedItem>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PresentedHistoryPage {
    pub state: HistoryState,
    pub items: Vec<PresentedHistoryItem>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
/// The accepted value, or its final tombstone (`deleted`). Audio and position never change.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct BookmarkRecord {
    pub bookmark_id: String,
    pub audio: AudioIdentity,
    pub position_ms: u64,
    pub deleted: bool,
    pub listener_order: u64,
    pub accepted_at_ms: i64,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriteOutcome {
    Accepted,
    AlreadyCurrent,
    RevisionConflict,
    HistoryEpochConflict,
    /// A create for a bookmark ID whose tombstone is final; deletion is never undone by a replay.
    BookmarkDeleted,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Resolution {
    ListenerChooses,
    KeepServer,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "resource", rename_all = "snake_case", deny_unknown_fields)]
pub enum MutationState {
    Progress {
        #[serde(deserialize_with = "required_nullable")]
        current: Option<ProgressRecord>,
    },
    Saved {
        current: SavedRecord,
    },
    History {
        current: HistoryState,
    },
    Bookmark {
        current: BookmarkRecord,
    },
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct Receipt {
    pub operation_id: String,
    pub outcome: WriteOutcome,
    #[serde(deserialize_with = "required_nullable")]
    pub resolution: Option<Resolution>,
    pub state: MutationState,
}
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCode {
    InvalidRequest,
    InvalidPosition,
    InvalidCursor,
    Unauthenticated,
    NotFound,
    IdempotencyKeyReused,
    StaleDeviceSequence,
    RevisionExhausted,
    Unavailable,
    CorruptState,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct LibraryError {
    pub code: ErrorCode,
    pub retryable: bool,
}
