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
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContinueItem {
    pub progress: ProgressRecord,
    pub resume: ResumeStatus,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedItem {
    pub saved: SavedRecord,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryItem {
    pub audio: AudioIdentity,
    pub started_at_ms: i64,
    pub listener_order: u64,
    #[serde(deserialize_with = "required_nullable")]
    pub progress: Option<ProgressRecord>,
    pub resume: ResumeStatus,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ContinuePage {
    pub items: Vec<ContinueItem>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SavedPage {
    pub items: Vec<SavedItem>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct HistoryPage {
    pub state: HistoryState,
    pub items: Vec<HistoryItem>,
    #[serde(deserialize_with = "required_nullable")]
    pub next_cursor: Option<String>,
}
/// Released public metadata, never the current editable Work head.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkPresentation {
    pub work_id: String,
    pub publication_id: String,
    pub title: String,
    pub kind: crate::theatre::WorkKind,
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
#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum WriteOutcome {
    Accepted,
    AlreadyCurrent,
    RevisionConflict,
    HistoryEpochConflict,
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
