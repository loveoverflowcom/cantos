//! Authenticated listener register reconciliation, operation outbox and playback sync state.
//!
//! Pure domain/application state machine: no DOM, timers, storage or HTTP calls live here.
//! Receipts are historical operation outcomes; registers only advance monotonically by revision.
pub(super) mod publication;
use super::account_view::AccountFailure;
use crate::core::{
    playback::{CallbackIdentity, Source, TrackId},
    public_ids::{canonical_uuid, PublicWorkId},
    public_track::PinnedRelease,
};
use cantos_contracts::listener_library::{
    AudioIdentity, AudioPresentation, ClearConfirmation, Completion, HistoryClear, HistoryStart,
    HistoryState, MutationState, Operation, PlaybackEvent, PresentedContinueItem,
    PresentedHistoryItem, PresentedSavedItem, ProgressKind, ProgressRecord, ProgressWrite, Receipt,
    Resolution, ResumeStatus, SavedRecord, SavedWrite, WorkPresentation, WriteOutcome,
};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

/// Minimum interval between periodic background playback checkpoints for the same publication.
pub const CHECKPOINT_THROTTLE_MS: i64 = 10_000;
/// Maximum persisted unacknowledged operations per account in browser recovery storage.
pub const MAX_PENDING_OPERATIONS: usize = 32;

/// Generate a canonical lowercase UUID v4 string. Uses `crypto.randomUUID()` in the browser and
/// a monotonic RFC 4122 v4 builder fallback on non-wasm targets.
pub fn generate_uuid_string() -> String {
    #[cfg(target_arch = "wasm32")]
    if let Some(id) = leptos::web_sys::window()
        .and_then(|window| window.crypto().ok())
        .map(|crypto| crypto.random_uuid())
        .and_then(|raw| canonical_uuid(&raw))
    {
        return id.to_string();
    }
    use std::sync::atomic::{AtomicU64, Ordering};
    static COUNTER: AtomicU64 = AtomicU64::new(1);
    let seq = COUNTER.fetch_add(1, Ordering::Relaxed);
    let mut bytes = [0u8; 16];
    bytes[8..16].copy_from_slice(&seq.to_be_bytes());
    uuid::Builder::from_random_bytes(bytes)
        .into_uuid()
        .to_string()
}

/// Extract the contract [`AudioIdentity`] from a public playback [`Source`].
pub fn audio_identity_from_source(source: Source) -> Option<AudioIdentity> {
    let TrackId::Public(track) = source.track() else {
        return None;
    };
    Some(AudioIdentity {
        work_id: track.work().to_string(),
        unit_id: track.unit().to_string(),
        publication_id: track.publication().to_string(),
        rendition_id: track.rendition().to_string(),
        sha256: track.sha256().to_string(),
        duration_ms: u64::from(source.duration_ms()),
    })
}

/// Verify that an admitted [`PinnedRelease`] matches every field of a stored [`AudioIdentity`].
pub fn release_matches_audio_identity(release: &PinnedRelease, expected: &AudioIdentity) -> bool {
    let track = release.track();
    track.work().to_string() == expected.work_id
        && track.unit().to_string() == expected.unit_id
        && track.publication().to_string() == expected.publication_id
        && track.rendition().to_string() == expected.rendition_id
        && track.sha256().to_string() == expected.sha256
        && u64::from(release.duration_ms()) == expected.duration_ms
}

pub fn valid_audio_identity(audio: &AudioIdentity) -> bool {
    PublicWorkId::parse(&audio.work_id).is_some()
        && canonical_uuid(&audio.unit_id).is_some()
        && canonical_uuid(&audio.publication_id).is_some()
        && canonical_uuid(&audio.rendition_id).is_some()
        && audio.sha256.len() == 64
        && audio
            .sha256
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        && (1..=crate::core::public_track::MAX_DURATION_MS).contains(&audio.duration_ms)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RecoveryMode {
    SendIntent,
    ReadReceiptFirst,
}

impl RecoveryMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SendIntent => "send_intent",
            Self::ReadReceiptFirst => "read_receipt_first",
        }
    }

    pub fn from_str_name(value: &str) -> Option<Self> {
        match value {
            "send_intent" => Some(Self::SendIntent),
            "read_receipt_first" => Some(Self::ReadReceiptFirst),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MutationPayload {
    Saved {
        work_id: String,
        write: SavedWrite,
    },
    Progress {
        publication_id: String,
        write: ProgressWrite,
    },
    HistoryStart {
        write: HistoryStart,
    },
    HistoryClear {
        write: HistoryClear,
    },
}

impl MutationPayload {
    pub fn to_json_value(&self) -> serde_json::Value {
        match self {
            Self::Saved { work_id, write } => serde_json::json!({
                "kind": "saved",
                "work_id": work_id,
                "write": serde_json::to_value(write).unwrap_or(serde_json::Value::Null),
            }),
            Self::Progress {
                publication_id,
                write,
            } => serde_json::json!({
                "kind": "progress",
                "publication_id": publication_id,
                "write": serde_json::to_value(write).unwrap_or(serde_json::Value::Null),
            }),
            Self::HistoryStart { write } => serde_json::json!({
                "kind": "history_start",
                "write": serde_json::to_value(write).unwrap_or(serde_json::Value::Null),
            }),
            Self::HistoryClear { write } => serde_json::json!({
                "kind": "history_clear",
                "write": serde_json::to_value(write).unwrap_or(serde_json::Value::Null),
            }),
        }
    }

    pub fn from_json_value(value: &serde_json::Value) -> Option<Self> {
        let kind = value.get("kind")?.as_str()?;
        let write_val = value.get("write")?.clone();
        match kind {
            "saved" => Some(Self::Saved {
                work_id: value.get("work_id")?.as_str()?.to_owned(),
                write: serde_json::from_value(write_val).ok()?,
            }),
            "progress" => Some(Self::Progress {
                publication_id: value.get("publication_id")?.as_str()?.to_owned(),
                write: serde_json::from_value(write_val).ok()?,
            }),
            "history_start" => Some(Self::HistoryStart {
                write: serde_json::from_value(write_val).ok()?,
            }),
            "history_clear" => Some(Self::HistoryClear {
                write: serde_json::from_value(write_val).ok()?,
            }),
            _ => None,
        }
    }

    pub fn operation(&self) -> &Operation {
        match self {
            Self::Saved { write, .. } => &write.operation,
            Self::Progress { write, .. } => &write.operation,
            Self::HistoryStart { write } => &write.operation,
            Self::HistoryClear { write } => &write.operation,
        }
    }

    pub fn resource_key(&self) -> String {
        match self {
            Self::Saved { work_id, .. } => format!("saved:{work_id}"),
            Self::Progress { publication_id, .. } => format!("progress:{publication_id}"),
            Self::HistoryStart { .. } | Self::HistoryClear { .. } => "history".to_owned(),
        }
    }

    pub fn is_valid(&self) -> bool {
        let op = self.operation();
        if canonical_uuid(&op.operation_id).is_none()
            || canonical_uuid(&op.device_id).is_none()
            || op.device_sequence == 0
            || op.device_sequence > i64::MAX as u64
            || op.device_reported_at_ms.is_some_and(|t| t < 0)
        {
            return false;
        }
        match self {
            Self::Saved { work_id, write } => {
                PublicWorkId::parse(work_id).is_some() && write.base_revision <= i64::MAX as u64
            }
            Self::Progress {
                publication_id,
                write,
            } => {
                canonical_uuid(publication_id).is_some()
                    && write.audio.publication_id == *publication_id
                    && valid_audio_identity(&write.audio)
                    && write.base_revision <= i64::MAX as u64
                    && write.position_ms <= write.audio.duration_ms
                    && (write.kind != ProgressKind::Restart || write.position_ms == 0)
                    && (write.kind != ProgressKind::MarkCompleted
                        || write.position_ms == write.audio.duration_ms)
            }
            Self::HistoryStart { write } => {
                valid_audio_identity(&write.audio)
                    && canonical_uuid(&write.engine_session_id).is_some()
                    && write.event == PlaybackEvent::PlaybackStarted
                    && write.history_epoch <= i64::MAX as u64
            }
            Self::HistoryClear { write } => {
                write.base_revision <= i64::MAX as u64
                    && write.history_epoch <= write.base_revision
                    && write.confirmation == ClearConfirmation::ClearHistory
            }
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingMutation {
    pub operation_id: String,
    pub resource_key: String,
    pub payload: MutationPayload,
    pub recovery: RecoveryMode,
}

impl PendingMutation {
    pub fn is_valid(&self) -> bool {
        self.payload.is_valid()
            && self.operation_id == self.payload.operation().operation_id
            && self.resource_key == self.payload.resource_key()
    }

    pub fn to_json_value(&self) -> serde_json::Value {
        serde_json::json!({
            "operation_id": self.operation_id,
            "resource_key": self.resource_key,
            "payload": self.payload.to_json_value(),
            "recovery": self.recovery.as_str(),
        })
    }

    pub fn from_json_value(value: &serde_json::Value) -> Option<Self> {
        Some(Self {
            operation_id: value.get("operation_id")?.as_str()?.to_owned(),
            resource_key: value.get("resource_key")?.as_str()?.to_owned(),
            payload: MutationPayload::from_json_value(value.get("payload")?)?,
            recovery: RecoveryMode::from_str_name(value.get("recovery")?.as_str()?)?,
        })
    }
}

/// Non-secret account-isolated outbox journal for reload/reauth recovery.
/// Never stores Bearer tokens, session IDs, cookies or signed media URLs.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AccountSyncJournal {
    pub actor_id: String,
    pub device_id: String,
    pub sequences: BTreeMap<String, u64>,
    pub pending: Vec<PendingMutation>,
}

impl AccountSyncJournal {
    pub fn new(actor_id: impl Into<String>, device_id: impl Into<String>) -> Option<Self> {
        let actor_id = actor_id.into();
        let device_id = device_id.into();
        if actor_id.is_empty()
            || actor_id.len() > 128
            || actor_id.chars().any(char::is_control)
            || canonical_uuid(&device_id).is_none()
        {
            return None;
        }
        Some(Self {
            actor_id,
            device_id,
            sequences: BTreeMap::new(),
            pending: Vec::new(),
        })
    }

    pub fn to_json_string(&self) -> Option<String> {
        let mut seq_map = serde_json::Map::new();
        for (k, v) in &self.sequences {
            seq_map.insert(k.clone(), serde_json::Value::from(*v));
        }
        let pending_vals: Vec<serde_json::Value> = self
            .pending
            .iter()
            .map(PendingMutation::to_json_value)
            .collect();
        let val = serde_json::json!({
            "actor_id": self.actor_id,
            "device_id": self.device_id,
            "sequences": seq_map,
            "pending": pending_vals,
        });
        serde_json::to_string(&val).ok()
    }

    pub fn from_json_str(raw: &str) -> Option<Self> {
        let value: serde_json::Value = serde_json::from_str(raw).ok()?;
        let actor_id = value.get("actor_id")?.as_str()?.to_owned();
        let device_id = value.get("device_id")?.as_str()?.to_owned();
        let seq_obj = value.get("sequences")?.as_object()?;
        let mut sequences = BTreeMap::new();
        for (k, v) in seq_obj {
            sequences.insert(k.clone(), v.as_u64()?);
        }
        let pending_arr = value.get("pending")?.as_array()?;
        let mut pending = Vec::with_capacity(pending_arr.len());
        for item in pending_arr {
            pending.push(PendingMutation::from_json_value(item)?);
        }
        Some(Self {
            actor_id,
            device_id,
            sequences,
            pending,
        })
    }

    /// Validate a restored journal for `expected_actor` and upgrade every pending entry to
    /// [`RecoveryMode::ReadReceiptFirst`] because pre-reload network outcomes are unknown.
    pub fn restore_for_actor(mut self, expected_actor: &str) -> Option<Self> {
        if self.actor_id != expected_actor
            || canonical_uuid(&self.device_id).is_none()
            || self.pending.len() > MAX_PENDING_OPERATIONS
            || self
                .sequences
                .values()
                .any(|seq| *seq == 0 || *seq > i64::MAX as u64)
        {
            return None;
        }
        let mut seen_ops = BTreeSet::new();
        let mut seen_keys = BTreeSet::new();
        for item in &mut self.pending {
            if !item.is_valid()
                || item.payload.operation().device_id != self.device_id
                || !seen_ops.insert(item.operation_id.clone())
                || !seen_keys.insert(item.resource_key.clone())
            {
                return None;
            }
            let seq = item.payload.operation().device_sequence;
            let entry = self
                .sequences
                .entry(item.resource_key.clone())
                .or_insert(seq);
            if *entry < seq {
                *entry = seq;
            }
            item.recovery = RecoveryMode::ReadReceiptFirst;
        }
        Some(self)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommandError {
    SignedOut,
    SessionChanged,
    StaleDeviceSequence,
    IdempotencyKeyReused,
    RevisionExhausted,
    InvalidPosition,
    NotFound,
    Unavailable,
    Protocol,
}

impl CommandError {
    pub fn to_account_failure(self) -> Option<AccountFailure> {
        match self {
            Self::SignedOut => Some(AccountFailure::SignedOut),
            Self::SessionChanged => Some(AccountFailure::SessionChanged),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum SavedCommandPhase {
    #[default]
    Idle,
    Pending {
        operation_id: String,
        desired: bool,
    },
    UnknownOutcome {
        operation_id: String,
        desired: bool,
    },
    Conflict {
        desired: bool,
        current: SavedRecord,
    },
    Failed {
        desired: bool,
        error: CommandError,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct SavedWorkState {
    pub record: Option<SavedRecord>,
    pub loading: bool,
    pub command: SavedCommandPhase,
    pub queued_desired: Option<bool>,
    pub work_presentation: Option<WorkPresentation>,
}

impl SavedWorkState {
    pub fn is_saved(&self) -> bool {
        self.record.as_ref().is_some_and(|r| r.saved)
    }

    pub fn base_revision(&self) -> u64 {
        self.record.as_ref().map_or(0, |r| r.revision)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProgressConflict {
    /// A delayed local checkpoint lost to a protected server write (`explicit_seek`, `restart`,
    /// `mark_completed`, `resolve_here`, `resolve_other`). The server record becomes the new base
    /// revision; the local player is not auto-sought.
    KeepServer {
        local_position_ms: u64,
        server: ProgressRecord,
    },
    /// Competing sessions wrote different progress. The listener chooses between keeping the
    /// local position (`resolve_here`) or adopting the server position (`resolve_other`) against
    /// `server.revision`.
    ListenerChooses {
        audio: AudioIdentity,
        local_position_ms: u64,
        server: ProgressRecord,
    },
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ProgressCommandPhase {
    #[default]
    Idle,
    Pending {
        operation_id: String,
        write: ProgressWrite,
    },
    UnknownOutcome {
        operation_id: String,
        write: ProgressWrite,
    },
    Conflict(ProgressConflict),
    Failed {
        audio: AudioIdentity,
        position_ms: u64,
        kind: ProgressKind,
        error: CommandError,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UnsentProgress {
    pub audio: AudioIdentity,
    pub position_ms: u64,
    pub kind: ProgressKind,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PublicationProgressState {
    pub record: Option<ProgressRecord>,
    pub point_read_resolved: bool,
    pub point_read_in_flight: bool,
    pub last_checkpoint_at_ms: Option<i64>,
    pub unsent: Option<UnsentProgress>,
    pub command: ProgressCommandPhase,
    pub presentation: Option<AudioPresentation>,
}

impl PublicationProgressState {
    pub fn base_revision(&self) -> u64 {
        self.record.as_ref().map_or(0, |r| r.revision)
    }
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum HistoryCommandPhase {
    #[default]
    Idle,
    PendingStart {
        operation_id: String,
        write: HistoryStart,
    },
    PendingClear {
        operation_id: String,
        write: HistoryClear,
    },
    UnknownOutcome {
        operation_id: String,
        is_clear: bool,
    },
    ClearConflict {
        current: HistoryState,
    },
    Failed {
        is_clear: bool,
        error: CommandError,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ResumeRefusal {
    ReleaseReplaced,
    AccessUnavailable,
    Protocol,
    LoadFailed,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub enum ResumeAttemptState {
    #[default]
    Idle,
    Preparing {
        publication_id: String,
    },
    Restored {
        publication_id: String,
        position_ms: u32,
    },
    Refused {
        publication_id: String,
        reason: ResumeRefusal,
    },
}

/// One exact resume intent; a repeated request for the same publication is still a new intent.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResumeTicket(u64);

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RereadTarget {
    Saved { work_id: String },
    Progress { publication_id: String },
    History,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedReceipt {
    pub outcome: WriteOutcome,
    pub resolution: Option<Resolution>,
    pub reread: Option<RereadTarget>,
    pub next_dispatch: Option<PendingMutation>,
    pub accepted_history_start: Option<(AudioIdentity, i64)>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct QueuedHistoryStart {
    audio: AudioIdentity,
    engine_session_id: String,
    presentation: Option<AudioPresentation>,
}

/// Account-scoped register and mutation coordinator.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LibrarySyncState {
    actor_id: String,
    device_id: String,
    sequences: BTreeMap<String, u64>,
    pending: BTreeMap<String, PendingMutation>,
    saved: BTreeMap<String, SavedWorkState>,
    progress: BTreeMap<String, PublicationProgressState>,
    history_state: Option<HistoryState>,
    history_command: HistoryCommandPhase,
    clear_confirmation: Option<HistoryState>,
    active_track_session: Option<(TrackId, String)>,
    acknowledged_sessions: BTreeSet<String>,
    unsent_starts: VecDeque<QueuedHistoryStart>,
    resume_attempt: ResumeAttemptState,
    resume_generation: u64,
}

impl LibrarySyncState {
    pub fn new(actor_id: impl Into<String>, device_id: impl Into<String>) -> Option<Self> {
        let journal = AccountSyncJournal::new(actor_id, device_id)?;
        Some(Self::from_journal(journal))
    }

    pub fn from_journal(journal: AccountSyncJournal) -> Self {
        let mut state = Self {
            actor_id: journal.actor_id,
            device_id: journal.device_id,
            sequences: journal.sequences,
            pending: BTreeMap::new(),
            saved: BTreeMap::new(),
            progress: BTreeMap::new(),
            history_state: None,
            history_command: HistoryCommandPhase::Idle,
            clear_confirmation: None,
            active_track_session: None,
            acknowledged_sessions: BTreeSet::new(),
            unsent_starts: VecDeque::new(),
            resume_attempt: ResumeAttemptState::Idle,
            resume_generation: 0,
        };
        for item in journal.pending {
            let op_id = item.operation_id.clone();
            match &item.payload {
                MutationPayload::Saved { work_id, write } => {
                    let entry = state.saved.entry(work_id.clone()).or_default();
                    entry.command = SavedCommandPhase::UnknownOutcome {
                        operation_id: op_id.clone(),
                        desired: write.saved,
                    };
                }
                MutationPayload::Progress {
                    publication_id,
                    write,
                } => {
                    let entry = state.progress.entry(publication_id.clone()).or_default();
                    entry.command = ProgressCommandPhase::UnknownOutcome {
                        operation_id: op_id.clone(),
                        write: write.clone(),
                    };
                }
                MutationPayload::HistoryStart { .. } => {
                    state.history_command = HistoryCommandPhase::UnknownOutcome {
                        operation_id: op_id.clone(),
                        is_clear: false,
                    };
                }
                MutationPayload::HistoryClear { .. } => {
                    state.history_command = HistoryCommandPhase::UnknownOutcome {
                        operation_id: op_id.clone(),
                        is_clear: true,
                    };
                }
            }
            state.pending.insert(op_id, item);
        }
        state
    }

    pub fn to_journal(&self) -> AccountSyncJournal {
        AccountSyncJournal {
            actor_id: self.actor_id.clone(),
            device_id: self.device_id.clone(),
            sequences: self.sequences.clone(),
            pending: self
                .pending
                .values()
                .take(MAX_PENDING_OPERATIONS)
                .cloned()
                .collect(),
        }
    }

    pub fn actor_id(&self) -> &str {
        &self.actor_id
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn pending_mutations(&self) -> Vec<PendingMutation> {
        self.pending.values().cloned().collect()
    }

    pub fn pending_mutation(&self, operation_id: &str) -> Option<&PendingMutation> {
        self.pending.get(operation_id)
    }

    pub fn mark_dispatched(&mut self, operation_id: &str) -> Option<PendingMutation> {
        let item = self.pending.get_mut(operation_id)?;
        let dispatched = item.clone();
        item.recovery = RecoveryMode::ReadReceiptFirst;
        Some(dispatched)
    }

    pub fn saved_work(&self, work_id: &str) -> Option<&SavedWorkState> {
        self.saved.get(work_id)
    }

    pub fn progress_for(&self, publication_id: &str) -> Option<&PublicationProgressState> {
        self.progress.get(publication_id)
    }

    pub fn history_state(&self) -> Option<HistoryState> {
        self.history_state.clone()
    }

    pub fn history_command(&self) -> &HistoryCommandPhase {
        &self.history_command
    }

    pub fn clear_confirmation(&self) -> Option<&HistoryState> {
        self.clear_confirmation.as_ref()
    }

    pub fn resume_attempt(&self) -> &ResumeAttemptState {
        &self.resume_attempt
    }

    pub fn set_resume_attempt(&mut self, state: ResumeAttemptState) {
        self.resume_generation = self.resume_generation.saturating_add(1);
        self.resume_attempt = state;
    }

    pub fn begin_resume(&mut self, publication_id: String) -> Option<ResumeTicket> {
        let generation = self.resume_generation.checked_add(1)?;
        self.resume_generation = generation;
        self.resume_attempt = ResumeAttemptState::Preparing { publication_id };
        Some(ResumeTicket(generation))
    }

    pub fn accepts_resume(&self, ticket: ResumeTicket) -> bool {
        ticket.0 == self.resume_generation
            && matches!(self.resume_attempt, ResumeAttemptState::Preparing { .. })
    }

    /// Record presentation metadata for a Work/Unit so local projection updates can render titles.
    pub fn remember_work_presentation(&mut self, work: WorkPresentation) {
        let work_id = work.work_id.clone();
        self.saved.entry(work_id).or_default().work_presentation = Some(work);
    }

    pub fn remember_audio_presentation(&mut self, presentation: AudioPresentation) {
        self.remember_work_presentation(presentation.work.clone());
        let publication_id = presentation.work.publication_id.clone();
        self.progress
            .entry(publication_id)
            .or_default()
            .presentation = Some(presentation);
    }

    /// Monotonically observe a [`SavedRecord`]. Older revisions never overwrite newer state.
    pub fn observe_saved_record(&mut self, record: SavedRecord) -> bool {
        if PublicWorkId::parse(&record.work_id).is_none()
            || record.revision > i64::MAX as u64
            || record.listener_order > i64::MAX as u64
            || record.accepted_at_ms < 0
            || (record.revision == 0
                && (record.saved || record.listener_order != 0 || record.accepted_at_ms != 0))
            || (record.revision > 0 && record.listener_order == 0)
        {
            return false;
        }
        let entry = self.saved.entry(record.work_id.clone()).or_default();
        entry.loading = false;
        let accept = match &entry.record {
            None => true,
            Some(current) => {
                record.revision > current.revision
                    || (record.revision == current.revision
                        && record.listener_order >= current.listener_order)
            }
        };
        if accept {
            entry.record = Some(record);
        }
        accept
    }

    /// Monotonically observe a [`ProgressRecord`]. Older revisions never overwrite newer state.
    pub fn observe_progress_record(&mut self, record: ProgressRecord) -> bool {
        if !valid_audio_identity(&record.audio)
            || canonical_uuid(&record.written_by).is_none()
            || record.position_ms > record.audio.duration_ms
            || record.accepted_at_ms < 0
            || record.revision == 0
            || record.revision > i64::MAX as u64
            || record.listener_order == 0
            || record.listener_order > i64::MAX as u64
            || (record.kind == ProgressKind::Restart
                && (record.completion == Completion::Completed || record.position_ms != 0))
            || (record.kind == ProgressKind::MarkCompleted
                && (record.position_ms != record.audio.duration_ms
                    || record.completion != Completion::Completed))
        {
            return false;
        }
        let entry = self
            .progress
            .entry(record.audio.publication_id.clone())
            .or_default();
        entry.point_read_resolved = true;
        entry.point_read_in_flight = false;
        let accept = match &entry.record {
            None => true,
            Some(current) => {
                record.revision > current.revision
                    || (record.revision == current.revision
                        && record.listener_order >= current.listener_order)
            }
        };
        if accept {
            entry.record = Some(record);
        }
        accept
    }

    /// Finish a `GET /progress/{publication}` point read (which may return `current: None`).
    pub fn finish_progress_read(
        &mut self,
        publication_id: &str,
        current: Option<ProgressRecord>,
        next_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        let entry = self.progress.entry(publication_id.to_owned()).or_default();
        entry.point_read_resolved = true;
        entry.point_read_in_flight = false;
        if let Some(record) = current {
            self.observe_progress_record(record);
        }
        self.try_mint_progress(publication_id, next_operation_id, reported_at_ms)
    }

    pub fn begin_progress_read(&mut self, publication_id: &str) -> bool {
        if canonical_uuid(publication_id).is_none() {
            return false;
        }
        let entry = self.progress.entry(publication_id.to_owned()).or_default();
        if entry.point_read_in_flight {
            return false;
        }
        entry.point_read_in_flight = true;
        true
    }

    pub fn begin_saved_read(&mut self, work_id: &str) -> bool {
        if PublicWorkId::parse(work_id).is_none() {
            return false;
        }
        let entry = self.saved.entry(work_id.to_owned()).or_default();
        if entry.loading {
            return false;
        }
        entry.loading = true;
        true
    }

    pub fn fail_saved_read(&mut self, work_id: &str) {
        if let Some(entry) = self.saved.get_mut(work_id) {
            entry.loading = false;
        }
    }

    pub fn fail_progress_read(&mut self, publication_id: &str) {
        if let Some(entry) = self.progress.get_mut(publication_id) {
            entry.point_read_in_flight = false;
        }
    }

    /// Monotonically observe [`HistoryState`]. If `epoch` advances, unsent pre-clear starts are
    /// dropped so delayed local starts never recreate cleared history.
    pub fn observe_history_state(&mut self, state: HistoryState) -> bool {
        if state.revision > i64::MAX as u64
            || state.epoch > i64::MAX as u64
            || state.epoch > state.revision
        {
            return false;
        }
        let accept = match &self.history_state {
            None => true,
            Some(current) => {
                state.epoch > current.epoch
                    || (state.epoch == current.epoch && state.revision >= current.revision)
            }
        };
        if accept {
            let epoch_advanced = self
                .history_state
                .as_ref()
                .is_some_and(|current| state.epoch > current.epoch);
            self.history_state = Some(state);
            if epoch_advanced {
                self.unsent_starts.clear();
            }
        }
        accept
    }

    fn next_sequence(&mut self, resource_key: &str) -> Option<u64> {
        let current = self.sequences.get(resource_key).copied().unwrap_or(0);
        if current >= i64::MAX as u64 {
            return None;
        }
        let next = current + 1;
        self.sequences.insert(resource_key.to_owned(), next);
        Some(next)
    }

    fn mint_operation(
        &mut self,
        resource_key: &str,
        operation_id: String,
        reported_at_ms: Option<i64>,
    ) -> Option<Operation> {
        let operation_id = canonical_uuid(&operation_id)?.to_string();
        if self.pending.contains_key(&operation_id) {
            return None;
        }
        let device_sequence = self.next_sequence(resource_key)?;
        Some(Operation {
            operation_id,
            device_id: self.device_id.clone(),
            device_sequence,
            device_reported_at_ms: reported_at_ms.filter(|t| *t >= 0),
        })
    }

    /// Request setting `saved` to `desired` for `work_id`.
    /// If a mutation is already in flight for `work_id`, `desired` is queued until it settles.
    pub fn request_saved(
        &mut self,
        work_id: &str,
        desired: bool,
        operation_id: String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        let work_id = PublicWorkId::parse(work_id)?.to_string();
        let entry = self.saved.entry(work_id.clone()).or_default();
        if matches!(
            entry.command,
            SavedCommandPhase::Pending { .. } | SavedCommandPhase::UnknownOutcome { .. }
        ) {
            entry.queued_desired = Some(desired);
            return None;
        }
        entry.queued_desired = None;
        let base_revision = entry.base_revision();
        let resource_key = format!("saved:{work_id}");
        let operation = self.mint_operation(&resource_key, operation_id, reported_at_ms)?;
        let op_id = operation.operation_id.clone();
        let write = SavedWrite {
            operation,
            base_revision,
            saved: desired,
        };
        let mutation = PendingMutation {
            operation_id: op_id.clone(),
            resource_key,
            payload: MutationPayload::Saved {
                work_id: work_id.clone(),
                write,
            },
            recovery: RecoveryMode::SendIntent,
        };
        self.pending.insert(op_id.clone(), mutation.clone());
        self.saved.entry(work_id).or_default().command = SavedCommandPhase::Pending {
            operation_id: op_id,
            desired,
        };
        Some(mutation)
    }

    /// Retry an unconfirmed or failed Saved mutation.
    /// An [`SavedCommandPhase::UnknownOutcome`] reuses the exact existing operation with
    /// [`RecoveryMode::ReadReceiptFirst`]; a conflict or terminal failure mints a fresh intent
    /// against the current base revision.
    pub fn retry_saved(
        &mut self,
        work_id: &str,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        let entry = self.saved.get_mut(work_id)?;
        match &entry.command {
            SavedCommandPhase::UnknownOutcome {
                operation_id,
                desired,
            } => {
                let op_id = operation_id.clone();
                let desired = *desired;
                let pending = self.pending.get_mut(&op_id)?;
                pending.recovery = RecoveryMode::ReadReceiptFirst;
                let out = pending.clone();
                entry.command = SavedCommandPhase::Pending {
                    operation_id: op_id,
                    desired,
                };
                Some(out)
            }
            SavedCommandPhase::Failed { desired, .. }
            | SavedCommandPhase::Conflict { desired, .. } => {
                let desired = *desired;
                entry.command = SavedCommandPhase::Idle;
                self.request_saved(work_id, desired, new_operation_id(), reported_at_ms)
            }
            _ => None,
        }
    }

    /// Record an engine-confirmed playback position or explicit progress intent.
    ///
    /// - Periodic `Checkpoint` writes are throttled by [`CHECKPOINT_THROTTLE_MS`] unless `flush`
    ///   is `true` (pause, source switch, ended, tab hide).
    /// - Unsent candidates coalesce in place; an unsent `ExplicitSeek`, `Restart` or
    ///   `MarkCompleted` is never downgraded to `Checkpoint`.
    /// - At most one operation is in flight per `progress:<publication_id>`.
    pub fn record_progress_intent(
        &mut self,
        audio: AudioIdentity,
        position_ms: u64,
        kind: ProgressKind,
        flush: bool,
        now_ms: i64,
        next_operation_id: impl FnOnce() -> String,
    ) -> Option<PendingMutation> {
        if !valid_audio_identity(&audio) {
            return None;
        }
        let position_ms = match kind {
            ProgressKind::Restart => 0,
            ProgressKind::MarkCompleted => audio.duration_ms,
            _ => position_ms.min(audio.duration_ms),
        };
        let pub_id = audio.publication_id.clone();
        let entry = self.progress.entry(pub_id.clone()).or_default();

        // While a ListenerChooses conflict is unresolved, only explicit resolution intents
        // (`ResolveHere` / `ResolveOther`) may proceed.
        if matches!(
            entry.command,
            ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses { .. })
        ) && !matches!(kind, ProgressKind::ResolveHere | ProgressKind::ResolveOther)
        {
            return None;
        }

        // Skip redundant checkpoints that match the already confirmed server state and have no
        // pending unsent intent, or passive flushes after an explicit MarkCompleted.
        if kind == ProgressKind::Checkpoint && entry.unsent.is_none() {
            if let Some(current) = &entry.record {
                if current.position_ms == position_ms
                    || (flush && current.completion == Completion::Completed)
                {
                    return None;
                }
            }
            if let ProgressCommandPhase::Pending { write, .. }
            | ProgressCommandPhase::UnknownOutcome { write, .. } = &entry.command
            {
                if write.position_ms == position_ms || write.kind == ProgressKind::MarkCompleted {
                    return None;
                }
            }
        }

        // Coalesce into `unsent`: a protected intent is never downgraded to `Checkpoint`.
        let merged_kind = match (&entry.unsent, kind) {
            (Some(prev), ProgressKind::Checkpoint) if prev.kind != ProgressKind::Checkpoint => {
                prev.kind
            }
            _ => kind,
        };
        let merged_pos = match merged_kind {
            ProgressKind::Restart => 0,
            ProgressKind::MarkCompleted => audio.duration_ms,
            _ => position_ms,
        };
        entry.unsent = Some(UnsentProgress {
            audio,
            position_ms: merged_pos,
            kind: merged_kind,
        });

        // Throttle periodic non-flush checkpoints while playing.
        if merged_kind == ProgressKind::Checkpoint && !flush {
            if let Some(last) = entry.last_checkpoint_at_ms {
                if now_ms.saturating_sub(last) < CHECKPOINT_THROTTLE_MS {
                    return None;
                }
            }
        }

        self.try_mint_progress(&pub_id, next_operation_id, Some(now_ms))
    }

    /// Resolve a [`ProgressConflict::ListenerChooses`] conflict against `server.revision`.
    pub fn resolve_progress_conflict(
        &mut self,
        publication_id: &str,
        keep_local: bool,
        next_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<(PendingMutation, u64)> {
        let entry = self.progress.get_mut(publication_id)?;
        let ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses {
            audio,
            local_position_ms,
            server,
        }) = entry.command.clone()
        else {
            return None;
        };
        let (position_ms, kind) = if keep_local {
            (
                local_position_ms.min(audio.duration_ms),
                ProgressKind::ResolveHere,
            )
        } else {
            (server.position_ms, ProgressKind::ResolveOther)
        };
        // Ensure `server` is the base revision for the resolution write.
        self.observe_progress_record(server);
        let entry = self.progress.get_mut(publication_id)?;
        entry.command = ProgressCommandPhase::Idle;
        entry.unsent = Some(UnsentProgress {
            audio,
            position_ms,
            kind,
        });
        let mutation = self.try_mint_progress(publication_id, next_operation_id, reported_at_ms)?;
        Some((mutation, position_ms))
    }

    pub fn dismiss_keep_server_notice(&mut self, publication_id: &str) {
        if let Some(entry) = self.progress.get_mut(publication_id) {
            if matches!(
                entry.command,
                ProgressCommandPhase::Conflict(ProgressConflict::KeepServer { .. })
            ) {
                entry.command = ProgressCommandPhase::Idle;
            }
        }
    }

    pub fn retry_progress(
        &mut self,
        publication_id: &str,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        let entry = self.progress.get_mut(publication_id)?;
        match &entry.command {
            ProgressCommandPhase::UnknownOutcome {
                operation_id,
                write,
            } => {
                let op_id = operation_id.clone();
                let write = write.clone();
                let pending = self.pending.get_mut(&op_id)?;
                pending.recovery = RecoveryMode::ReadReceiptFirst;
                let out = pending.clone();
                entry.command = ProgressCommandPhase::Pending {
                    operation_id: op_id,
                    write,
                };
                Some(out)
            }
            ProgressCommandPhase::Failed {
                audio,
                position_ms,
                kind,
                ..
            } => {
                let unsent = UnsentProgress {
                    audio: audio.clone(),
                    position_ms: *position_ms,
                    kind: *kind,
                };
                entry.command = ProgressCommandPhase::Idle;
                entry.unsent = Some(unsent);
                self.try_mint_progress(publication_id, new_operation_id, reported_at_ms)
            }
            _ => None,
        }
    }

    fn try_mint_progress(
        &mut self,
        publication_id: &str,
        next_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        let entry = self.progress.get_mut(publication_id)?;
        if entry.point_read_in_flight
            || matches!(
                entry.command,
                ProgressCommandPhase::Pending { .. }
                    | ProgressCommandPhase::UnknownOutcome { .. }
                    | ProgressCommandPhase::Conflict(ProgressConflict::ListenerChooses { .. })
            )
        {
            return None;
        }
        let unsent = entry.unsent.take()?;
        let base_revision = entry.base_revision();
        let resource_key = format!("progress:{publication_id}");
        let operation = self.mint_operation(&resource_key, next_operation_id(), reported_at_ms)?;
        let op_id = operation.operation_id.clone();
        let write = ProgressWrite {
            operation,
            audio: unsent.audio,
            base_revision,
            position_ms: unsent.position_ms,
            kind: unsent.kind,
        };
        let mutation = PendingMutation {
            operation_id: op_id.clone(),
            resource_key,
            payload: MutationPayload::Progress {
                publication_id: publication_id.to_owned(),
                write: write.clone(),
            },
            recovery: RecoveryMode::SendIntent,
        };
        self.pending.insert(op_id.clone(), mutation.clone());
        let entry = self.progress.get_mut(publication_id)?;
        if write.kind == ProgressKind::Checkpoint {
            entry.last_checkpoint_at_ms = reported_at_ms;
        }
        entry.command = ProgressCommandPhase::Pending {
            operation_id: op_id,
            write,
        };
        Some(mutation)
    }

    /// Reset the active engine session when a new playback source is selected or restored.
    pub fn reset_engine_session(&mut self) {
        self.active_track_session = None;
    }

    /// Record a real-engine `EngineStarted` acknowledgement for `identity` and `source`.
    /// Deduplicated per engine session so buffering recovery or pause/resume within the same
    /// playback session never re-emits or reorders history.
    pub fn record_engine_started(
        &mut self,
        identity: CallbackIdentity,
        source: Source,
        new_engine_session_id: impl FnOnce() -> String,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        if identity.track != source.track() {
            return None;
        }
        let audio = audio_identity_from_source(source)?;
        let engine_session_id = match &self.active_track_session {
            Some((track, session_id)) if *track == source.track() => session_id.clone(),
            _ => {
                let candidate = new_engine_session_id();
                let session_id = canonical_uuid(&candidate)
                    .map(|u| u.to_string())
                    .unwrap_or_else(generate_uuid_string);
                self.active_track_session = Some((source.track(), session_id.clone()));
                session_id
            }
        };
        if !self.acknowledged_sessions.insert(engine_session_id.clone()) {
            return None;
        }
        let presentation = self
            .progress
            .get(&audio.publication_id)
            .and_then(|p| p.presentation.clone());
        self.unsent_starts.push_back(QueuedHistoryStart {
            audio,
            engine_session_id,
            presentation,
        });
        self.try_mint_history_start(new_operation_id, reported_at_ms)
    }

    pub fn flush_queued_history_start(
        &mut self,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        self.try_mint_history_start(new_operation_id, reported_at_ms)
    }

    fn try_mint_history_start(
        &mut self,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        if !matches!(self.history_command, HistoryCommandPhase::Idle) {
            return None;
        }
        let history_epoch = self.history_state.as_ref()?.epoch;
        let queued = self.unsent_starts.pop_front()?;
        if let Some(presentation) = queued.presentation {
            self.remember_audio_presentation(presentation);
        }
        let operation = self.mint_operation("history", new_operation_id(), reported_at_ms)?;
        let op_id = operation.operation_id.clone();
        let write = HistoryStart {
            operation,
            audio: queued.audio,
            engine_session_id: queued.engine_session_id,
            event: PlaybackEvent::PlaybackStarted,
            history_epoch,
        };
        let mutation = PendingMutation {
            operation_id: op_id.clone(),
            resource_key: "history".to_owned(),
            payload: MutationPayload::HistoryStart {
                write: write.clone(),
            },
            recovery: RecoveryMode::SendIntent,
        };
        self.pending.insert(op_id.clone(), mutation.clone());
        self.history_command = HistoryCommandPhase::PendingStart {
            operation_id: op_id,
            write,
        };
        Some(mutation)
    }

    /// Open the Clear History confirmation dialog, pinning the currently observed [`HistoryState`].
    pub fn prompt_clear_history(&mut self) -> Option<HistoryState> {
        let state = self.history_state.clone()?;
        self.clear_confirmation = Some(state.clone());
        if matches!(
            self.history_command,
            HistoryCommandPhase::ClearConflict { .. }
                | HistoryCommandPhase::Failed { is_clear: true, .. }
        ) {
            self.history_command = HistoryCommandPhase::Idle;
        }
        Some(state)
    }

    pub fn cancel_clear_history(&mut self) {
        self.clear_confirmation = None;
        if matches!(
            self.history_command,
            HistoryCommandPhase::ClearConflict { .. }
                | HistoryCommandPhase::Failed { is_clear: true, .. }
        ) {
            self.history_command = HistoryCommandPhase::Idle;
        }
    }

    /// Confirm clearing history using the pinned confirmation [`HistoryState`].
    pub fn confirm_clear_history(
        &mut self,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        if matches!(
            self.history_command,
            HistoryCommandPhase::PendingStart { .. }
                | HistoryCommandPhase::PendingClear { .. }
                | HistoryCommandPhase::UnknownOutcome { .. }
        ) {
            return None;
        }
        let confirmed = self.clear_confirmation.take()?;
        // Drop any unsent starts from the pre-clear epoch so they cannot execute after clear.
        self.unsent_starts.clear();
        let operation = self.mint_operation("history", new_operation_id(), reported_at_ms)?;
        let op_id = operation.operation_id.clone();
        let write = HistoryClear {
            operation,
            base_revision: confirmed.revision,
            history_epoch: confirmed.epoch,
            confirmation: ClearConfirmation::ClearHistory,
        };
        let mutation = PendingMutation {
            operation_id: op_id.clone(),
            resource_key: "history".to_owned(),
            payload: MutationPayload::HistoryClear {
                write: write.clone(),
            },
            recovery: RecoveryMode::SendIntent,
        };
        self.pending.insert(op_id.clone(), mutation.clone());
        self.history_command = HistoryCommandPhase::PendingClear {
            operation_id: op_id,
            write,
        };
        Some(mutation)
    }

    pub fn retry_history(
        &mut self,
        new_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Option<PendingMutation> {
        match &self.history_command {
            HistoryCommandPhase::UnknownOutcome {
                operation_id,
                is_clear,
            } => {
                let op_id = operation_id.clone();
                let is_clear = *is_clear;
                let pending = self.pending.get_mut(&op_id)?;
                pending.recovery = RecoveryMode::ReadReceiptFirst;
                let out = pending.clone();
                self.history_command = match (&out.payload, is_clear) {
                    (MutationPayload::HistoryClear { write }, _) => {
                        HistoryCommandPhase::PendingClear {
                            operation_id: op_id,
                            write: write.clone(),
                        }
                    }
                    (MutationPayload::HistoryStart { write }, _) => {
                        HistoryCommandPhase::PendingStart {
                            operation_id: op_id,
                            write: write.clone(),
                        }
                    }
                    _ => return None,
                };
                Some(out)
            }
            HistoryCommandPhase::Failed { is_clear: true, .. } => {
                self.prompt_clear_history()?;
                self.confirm_clear_history(new_operation_id, reported_at_ms)
            }
            _ => None,
        }
    }

    /// Apply a verified server [`Receipt`] for `operation_id`.
    ///
    /// When `recovered_via_lookup` is `true` (the receipt came from `GET /operations/{op}` after
    /// an unknown outcome or reload), `AppliedReceipt::reread` instructs the caller to refresh
    /// the resource's current state so an older historical receipt is never assumed to be latest.
    pub fn apply_receipt(
        &mut self,
        operation_id: &str,
        receipt: Receipt,
        recovered_via_lookup: bool,
        next_operation_id: impl FnOnce() -> String,
        reported_at_ms: Option<i64>,
    ) -> Result<AppliedReceipt, CommandError> {
        if receipt.operation_id != operation_id {
            return Err(CommandError::Protocol);
        }
        let is_conflict = matches!(
            receipt.outcome,
            WriteOutcome::RevisionConflict | WriteOutcome::HistoryEpochConflict
        );
        if is_conflict != receipt.resolution.is_some() {
            return Err(CommandError::Protocol);
        }
        let Some(pending) = self.pending.remove(operation_id) else {
            return Err(CommandError::Protocol);
        };

        match (pending.payload, receipt.state) {
            (MutationPayload::Saved { work_id, write }, MutationState::Saved { current }) => {
                if current.work_id != work_id || !self.observe_saved_record(current.clone()) {
                    // Even if a newer revision was already observed, ensure the record is valid.
                    if current.work_id != work_id {
                        return Err(CommandError::Protocol);
                    }
                }
                let entry = self.saved.entry(work_id.clone()).or_default();
                let mut next_dispatch = None;
                match receipt.outcome {
                    WriteOutcome::Accepted | WriteOutcome::AlreadyCurrent => {
                        entry.command = SavedCommandPhase::Idle;
                        if let Some(queued) = entry.queued_desired.take() {
                            if entry.is_saved() != queued {
                                next_dispatch = self.request_saved(
                                    &work_id,
                                    queued,
                                    next_operation_id(),
                                    reported_at_ms,
                                );
                            }
                        }
                    }
                    WriteOutcome::RevisionConflict => {
                        entry.queued_desired = None;
                        entry.command = SavedCommandPhase::Conflict {
                            desired: write.saved,
                            current,
                        };
                    }
                    // Theatre Web has no bookmark owner; a bookmark outcome is never a Saved/progress answer.
                    WriteOutcome::HistoryEpochConflict | WriteOutcome::BookmarkDeleted => {
                        return Err(CommandError::Protocol)
                    }
                }
                Ok(AppliedReceipt {
                    outcome: receipt.outcome,
                    resolution: receipt.resolution,
                    reread: recovered_via_lookup.then_some(RereadTarget::Saved { work_id }),
                    next_dispatch,
                    accepted_history_start: None,
                })
            }
            (
                MutationPayload::Progress {
                    publication_id,
                    write,
                },
                MutationState::Progress { current },
            ) => {
                if let Some(record) = &current {
                    if record.audio.publication_id != publication_id {
                        return Err(CommandError::Protocol);
                    }
                    self.observe_progress_record(record.clone());
                } else if !is_conflict {
                    return Err(CommandError::Protocol);
                }
                let mut next_dispatch = None;
                match receipt.outcome {
                    WriteOutcome::Accepted | WriteOutcome::AlreadyCurrent => {
                        if let Some(entry) = self.progress.get_mut(&publication_id) {
                            entry.command = ProgressCommandPhase::Idle;
                        }
                        next_dispatch = self.try_mint_progress(
                            &publication_id,
                            next_operation_id,
                            reported_at_ms,
                        );
                    }
                    WriteOutcome::RevisionConflict => {
                        let Some(server_record) = current else {
                            return Err(CommandError::Protocol);
                        };
                        let entry = self.progress.entry(publication_id.clone()).or_default();
                        match receipt.resolution {
                            Some(Resolution::KeepServer) => {
                                entry.command =
                                    ProgressCommandPhase::Conflict(ProgressConflict::KeepServer {
                                        local_position_ms: write.position_ms,
                                        server: server_record,
                                    });
                                next_dispatch = self.try_mint_progress(
                                    &publication_id,
                                    next_operation_id,
                                    reported_at_ms,
                                );
                            }
                            Some(Resolution::ListenerChooses) => {
                                entry.unsent = None;
                                entry.command = ProgressCommandPhase::Conflict(
                                    ProgressConflict::ListenerChooses {
                                        audio: write.audio,
                                        local_position_ms: write.position_ms,
                                        server: server_record,
                                    },
                                );
                            }
                            None => return Err(CommandError::Protocol),
                        }
                    }
                    // Theatre Web has no bookmark owner; a bookmark outcome is never a Saved/progress answer.
                    WriteOutcome::HistoryEpochConflict | WriteOutcome::BookmarkDeleted => {
                        return Err(CommandError::Protocol)
                    }
                }
                Ok(AppliedReceipt {
                    outcome: receipt.outcome,
                    resolution: receipt.resolution,
                    reread: recovered_via_lookup
                        .then_some(RereadTarget::Progress { publication_id }),
                    next_dispatch,
                    accepted_history_start: None,
                })
            }
            (MutationPayload::HistoryStart { write }, MutationState::History { current }) => {
                if !self.observe_history_state(current.clone())
                    && (current.epoch > current.revision || current.revision > i64::MAX as u64)
                {
                    return Err(CommandError::Protocol);
                }
                let accepted_history_start = (matches!(
                    receipt.outcome,
                    WriteOutcome::Accepted | WriteOutcome::AlreadyCurrent
                ) && self
                    .history_state
                    .as_ref()
                    .is_some_and(|s| s.epoch == current.epoch))
                .then_some((
                    write.audio,
                    write
                        .operation
                        .device_reported_at_ms
                        .or(reported_at_ms)
                        .unwrap_or(0),
                ));
                // Whether Accepted, AlreadyCurrent or HistoryEpochConflict (keep_server), an old
                // start is finished and never re-minted under a post-clear epoch.
                self.history_command = HistoryCommandPhase::Idle;
                let next_dispatch = self.try_mint_history_start(next_operation_id, reported_at_ms);
                Ok(AppliedReceipt {
                    outcome: receipt.outcome,
                    resolution: receipt.resolution,
                    reread: (recovered_via_lookup
                        || receipt.outcome == WriteOutcome::HistoryEpochConflict)
                        .then_some(RereadTarget::History),
                    next_dispatch,
                    accepted_history_start,
                })
            }
            (MutationPayload::HistoryClear { .. }, MutationState::History { current }) => {
                if !self.observe_history_state(current.clone())
                    && (current.epoch > current.revision || current.revision > i64::MAX as u64)
                {
                    return Err(CommandError::Protocol);
                }
                let mut next_dispatch = None;
                match receipt.outcome {
                    WriteOutcome::Accepted | WriteOutcome::AlreadyCurrent => {
                        self.clear_confirmation = None;
                        self.unsent_starts.clear();
                        self.history_command = HistoryCommandPhase::Idle;
                    }
                    WriteOutcome::RevisionConflict | WriteOutcome::HistoryEpochConflict => {
                        // Conflict on clear requires a fresh confirmation against `current`.
                        self.clear_confirmation = Some(current.clone());
                        self.history_command = HistoryCommandPhase::ClearConflict { current };
                    }
                    WriteOutcome::BookmarkDeleted => return Err(CommandError::Protocol),
                }
                if matches!(self.history_command, HistoryCommandPhase::Idle) {
                    next_dispatch = self.try_mint_history_start(next_operation_id, reported_at_ms);
                }
                Ok(AppliedReceipt {
                    outcome: receipt.outcome,
                    resolution: receipt.resolution,
                    reread: (recovered_via_lookup || is_conflict).then_some(RereadTarget::History),
                    next_dispatch,
                    accepted_history_start: None,
                })
            }
            _ => Err(CommandError::Protocol),
        }
    }

    /// Record an unknown outcome (network failure, timeout or HTTP 5xx) for an in-flight operation.
    /// The operation remains in `self.pending` with [`RecoveryMode::ReadReceiptFirst`] and keeps
    /// its exact `operation_id`, `device_id`, `device_sequence` and body.
    pub fn record_unknown_outcome(&mut self, operation_id: &str) -> bool {
        let Some(pending) = self.pending.get_mut(operation_id) else {
            return false;
        };
        pending.recovery = RecoveryMode::ReadReceiptFirst;
        let op_id = pending.operation_id.clone();
        match &pending.payload {
            MutationPayload::Saved { work_id, write } => {
                if let Some(entry) = self.saved.get_mut(work_id) {
                    entry.command = SavedCommandPhase::UnknownOutcome {
                        operation_id: op_id,
                        desired: write.saved,
                    };
                }
            }
            MutationPayload::Progress {
                publication_id,
                write,
            } => {
                if let Some(entry) = self.progress.get_mut(publication_id) {
                    entry.command = ProgressCommandPhase::UnknownOutcome {
                        operation_id: op_id,
                        write: write.clone(),
                    };
                }
            }
            MutationPayload::HistoryStart { .. } => {
                self.history_command = HistoryCommandPhase::UnknownOutcome {
                    operation_id: op_id,
                    is_clear: false,
                };
            }
            MutationPayload::HistoryClear { .. } => {
                self.history_command = HistoryCommandPhase::UnknownOutcome {
                    operation_id: op_id,
                    is_clear: true,
                };
            }
        }
        true
    }

    /// Record a definitive error (such as `409 stale_device_sequence`, `422 idempotency_key_reused`,
    /// `404 not_found` or `409 revision_exhausted`) for an operation that was rejected by the server.
    pub fn record_command_error(
        &mut self,
        operation_id: &str,
        error: CommandError,
        rotated_device_id: impl FnOnce() -> String,
    ) -> bool {
        let Some(pending) = self.pending.remove(operation_id) else {
            return false;
        };
        if error == CommandError::StaleDeviceSequence {
            let candidate = rotated_device_id();
            let next_device = canonical_uuid(&candidate)
                .map(|u| u.to_string())
                .unwrap_or_else(generate_uuid_string);
            self.device_id = next_device;
            self.sequences.clear();
        }
        match pending.payload {
            MutationPayload::Saved { work_id, write } => {
                if let Some(entry) = self.saved.get_mut(&work_id) {
                    entry.command = SavedCommandPhase::Failed {
                        desired: write.saved,
                        error,
                    };
                }
            }
            MutationPayload::Progress {
                publication_id,
                write,
            } => {
                if let Some(entry) = self.progress.get_mut(&publication_id) {
                    entry.command = ProgressCommandPhase::Failed {
                        audio: write.audio,
                        position_ms: write.position_ms,
                        kind: write.kind,
                        error,
                    };
                }
            }
            MutationPayload::HistoryStart { .. } => {
                self.history_command = HistoryCommandPhase::Failed {
                    is_clear: false,
                    error,
                };
            }
            MutationPayload::HistoryClear { write } => {
                self.clear_confirmation = Some(HistoryState {
                    revision: write.base_revision,
                    epoch: write.history_epoch,
                });
                self.history_command = HistoryCommandPhase::Failed {
                    is_clear: true,
                    error,
                };
            }
        }
        true
    }

    /// Synchronize an in-memory [`super::library::LibraryProjection`] with the latest confirmed
    /// registers so `Của tôi` stays consistent with detail/player actions without erasing other
    /// sections on Unsave or Clear History.
    pub fn sync_projection(&self, projection: &mut super::library::LibraryProjection) -> bool {
        // Report actual presentation changes while retaining the existing reconciliation order.
        // No bulk projection clone, and no assumption that paging left it synchronized.
        fn replace<T: PartialEq>(target: &mut T, next: T) -> bool {
            let changed = *target != next;
            *target = next;
            changed
        }
        let mut changed = false;
        if let Some(state) = &self.history_state {
            let epoch_cleared = projection
                .history_state
                .as_ref()
                .is_some_and(|old| state.epoch > old.epoch);
            changed |= replace(&mut projection.history_state, Some(state.clone()));
            if epoch_cleared {
                projection.history.clear();
                projection.clear_history_cursor();
            }
        }

        // Reconcile Saved list: remove unsaved works, update/insert confirmed saved works.
        if projection.phase(super::library::Section::Saved) != super::library::PagePhase::Loading {
            let before_len = projection.saved.len();
            projection.saved.retain(|item| {
                self.saved
                    .get(&item.saved.work_id)
                    .and_then(|s| s.record.as_ref())
                    .is_none_or(|record| record.saved)
            });
            changed |= before_len != projection.saved.len();
            for item in &mut projection.saved {
                if let Some(state) = self.saved.get(&item.saved.work_id) {
                    if let Some(record) = &state.record {
                        if record.revision >= item.saved.revision {
                            changed |= replace(&mut item.saved, record.clone());
                        }
                    }
                    if item.presentation.is_none() {
                        changed |= replace(&mut item.presentation, state.work_presentation.clone());
                    }
                }
            }
            let mut inserted = false;
            for (work_id, state) in &self.saved {
                let Some(record) = &state.record else {
                    continue;
                };
                if record.saved
                    && !projection
                        .saved
                        .iter()
                        .any(|item| &item.saved.work_id == work_id)
                {
                    projection.saved.push(PresentedSavedItem {
                        saved: record.clone(),
                        presentation: state.work_presentation.clone(),
                    });
                    inserted = true;
                }
            }
            changed |= inserted;
            if inserted {
                projection.saved.sort_by(|a, b| {
                    b.saved
                        .listener_order
                        .cmp(&a.saved.listener_order)
                        .then_with(|| a.saved.work_id.cmp(&b.saved.work_id))
                });
            }
        }

        // Reconcile History list progress snapshots without removing history on completion or unsave.
        for item in &mut projection.history {
            if let Some(state) = self.progress.get(&item.audio.publication_id) {
                if let Some(record) = &state.record {
                    let newer = item
                        .progress
                        .as_ref()
                        .is_none_or(|old| record.revision >= old.revision);
                    if newer {
                        changed |= replace(&mut item.progress, Some(record.clone()));
                    }
                }
                if item.presentation.is_none() {
                    changed |= replace(&mut item.presentation, state.presentation.clone());
                }
            }
        }

        // Reconcile Continue list: completed items leave Continue; in-progress items update/insert.
        if projection.phase(super::library::Section::Continue) != super::library::PagePhase::Loading
        {
            let before_len = projection.continuing.len();
            projection.continuing.retain(|item| {
                self.progress
                    .get(&item.progress.audio.publication_id)
                    .and_then(|s| s.record.as_ref())
                    .is_none_or(|record| record.completion == Completion::InProgress)
            });
            changed |= before_len != projection.continuing.len();
            for item in &mut projection.continuing {
                if let Some(state) = self.progress.get(&item.progress.audio.publication_id) {
                    if let Some(record) = &state.record {
                        if record.revision >= item.progress.revision {
                            changed |= replace(&mut item.progress, record.clone());
                        }
                    }
                    if item.presentation.is_none() {
                        changed |= replace(&mut item.presentation, state.presentation.clone());
                    }
                }
            }
            let mut reordered = false;
            for (pub_id, state) in &self.progress {
                let Some(record) = &state.record else {
                    continue;
                };
                if record.completion != Completion::InProgress {
                    continue;
                };
                if let Some(existing) = projection
                    .continuing
                    .iter_mut()
                    .find(|item| item.progress.audio.work_id == record.audio.work_id)
                {
                    if record.listener_order > existing.progress.listener_order
                        || (&existing.progress.audio.publication_id == pub_id
                            && record.revision > existing.progress.revision)
                    {
                        existing.progress = record.clone();
                        if let Some(p) = &state.presentation {
                            existing.presentation = Some(p.clone());
                        }
                        reordered = true;
                    }
                } else if state.presentation.is_some() {
                    projection.continuing.push(PresentedContinueItem {
                        progress: record.clone(),
                        resume: ResumeStatus::ExactAvailable,
                        presentation: state.presentation.clone(),
                    });
                    reordered = true;
                }
            }
            changed |= reordered;
            if reordered {
                projection.continuing.sort_by(|a, b| {
                    b.progress
                        .listener_order
                        .cmp(&a.progress.listener_order)
                        .then_with(|| a.progress.audio.work_id.cmp(&b.progress.audio.work_id))
                });
            }
        }
        changed
    }
}

/// Record a newly accepted history start into `projection.history` when presentation facts exist.
pub fn record_started_in_projection(
    projection: &mut super::library::LibraryProjection,
    sync: &LibrarySyncState,
    audio: &AudioIdentity,
    started_at_ms: i64,
) {
    let listener_order = sync.history_state.as_ref().map_or(1, |s| s.revision);
    let progress = sync
        .progress_for(&audio.publication_id)
        .and_then(|s| s.record.clone());
    let presentation = sync
        .progress_for(&audio.publication_id)
        .and_then(|s| s.presentation.clone());
    if let Some(existing) = projection
        .history
        .iter_mut()
        .find(|item| item.audio.work_id == audio.work_id)
    {
        existing.audio = audio.clone();
        existing.started_at_ms = started_at_ms.max(0);
        existing.listener_order = listener_order;
        existing.progress = progress;
        existing.resume = ResumeStatus::ExactAvailable;
        if presentation.is_some() {
            existing.presentation = presentation;
        }
    } else {
        projection.history.insert(
            0,
            PresentedHistoryItem {
                audio: audio.clone(),
                started_at_ms: started_at_ms.max(0),
                listener_order,
                progress,
                resume: ResumeStatus::ExactAvailable,
                presentation,
            },
        );
    }
    projection.history.sort_by(|a, b| {
        b.listener_order
            .cmp(&a.listener_order)
            .then_with(|| a.audio.work_id.cmp(&b.audio.work_id))
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    fn uuid(n: u128) -> String {
        uuid::Uuid::from_u128(n).to_string()
    }

    fn sample_audio(work: u128, unit: u128, pub_id: u128) -> AudioIdentity {
        AudioIdentity {
            work_id: uuid(work),
            unit_id: uuid(unit),
            publication_id: uuid(pub_id),
            rendition_id: uuid(pub_id + 100),
            sha256: "a".repeat(64),
            duration_ms: 120_000,
        }
    }

    #[test]
    fn only_the_latest_resume_intent_accepts_a_manifest_completion() {
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();
        let first = sync.begin_resume(uuid(3)).unwrap();
        assert!(sync.accepts_resume(first));
        // Even choosing a different position of the same release supersedes the old request.
        let second = sync.begin_resume(uuid(3)).unwrap();
        assert!(!sync.accepts_resume(first));
        assert!(sync.accepts_resume(second));
        let other_release = sync.begin_resume(uuid(4)).unwrap();
        assert!(!sync.accepts_resume(second));
        assert!(sync.accepts_resume(other_release));
        // A completed/refused request and a fresh public listen both invalidate old replies.
        sync.set_resume_attempt(ResumeAttemptState::Restored {
            publication_id: uuid(4),
            position_ms: 25_000,
        });
        assert!(!sync.accepts_resume(other_release));
        let cancelled = sync.begin_resume(uuid(3)).unwrap();
        sync.set_resume_attempt(ResumeAttemptState::Idle);
        assert!(!sync.accepts_resume(cancelled));
    }

    #[test]
    fn saved_intent_preserves_operation_across_unknown_outcome_and_queues_rapid_toggle() {
        let work = uuid(10);
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();

        let first = sync
            .request_saved(&work, true, uuid(100), Some(1_000))
            .unwrap();
        assert_eq!(first.payload.operation().operation_id, uuid(100));
        assert_eq!(first.payload.operation().device_sequence, 1);

        // Rapid toggle while first request is in flight queues `false` without minting a second PUT.
        assert!(sync
            .request_saved(&work, false, uuid(101), Some(1_100))
            .is_none());
        assert_eq!(sync.saved_work(&work).unwrap().queued_desired, Some(false));

        // Simulate timeout / network drop -> UnknownOutcome retains the exact same operation.
        assert!(sync.record_unknown_outcome(&uuid(100)));
        assert!(matches!(
            sync.saved_work(&work).unwrap().command,
            SavedCommandPhase::UnknownOutcome { desired: true, .. }
        ));
        let retry = sync.retry_saved(&work, || uuid(999), Some(1_200)).unwrap();
        assert_eq!(retry.operation_id, uuid(100));
        assert_eq!(retry.recovery, RecoveryMode::ReadReceiptFirst);
        assert_eq!(retry.payload, first.payload);

        // Receipt recovery via GET /operations/{op} requests a reread and dispatches queued unsave
        // against the newly confirmed revision 1.
        let applied = sync
            .apply_receipt(
                &uuid(100),
                Receipt {
                    operation_id: uuid(100),
                    outcome: WriteOutcome::Accepted,
                    resolution: None,
                    state: MutationState::Saved {
                        current: SavedRecord {
                            work_id: work.clone(),
                            saved: true,
                            revision: 1,
                            listener_order: 1,
                            accepted_at_ms: 1_250,
                        },
                    },
                },
                true,
                || uuid(102),
                Some(1_300),
            )
            .unwrap();
        assert_eq!(
            applied.reread,
            Some(RereadTarget::Saved {
                work_id: work.clone()
            })
        );
        let second = applied.next_dispatch.expect("queued unsave must dispatch");
        assert_eq!(second.operation_id, uuid(102));
        assert_eq!(second.payload.operation().device_sequence, 2);
        let MutationPayload::Saved { write, .. } = second.payload else {
            panic!("expected saved payload");
        };
        assert_eq!(write.base_revision, 1);
        assert!(!write.saved);
    }

    #[test]
    fn stale_device_sequence_rotates_device_id_and_resets_sequence_counter() {
        let work = uuid(11);
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();
        let first = sync
            .request_saved(&work, true, uuid(200), Some(1_000))
            .unwrap();
        assert_eq!(first.payload.operation().device_id, uuid(1));
        assert_eq!(first.payload.operation().device_sequence, 1);

        assert!(
            sync.record_command_error(&uuid(200), CommandError::StaleDeviceSequence, || uuid(2),)
        );
        assert_eq!(sync.device_id(), uuid(2));

        let retried = sync.retry_saved(&work, || uuid(201), Some(2_000)).unwrap();
        assert_eq!(retried.payload.operation().device_id, uuid(2));
        assert_eq!(retried.payload.operation().device_sequence, 1);
    }

    #[test]
    fn progress_throttles_checkpoints_coalesces_unsent_and_preserves_intentional_rewind() {
        let audio = sample_audio(1, 2, 3);
        let pub_id = audio.publication_id.clone();
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();

        // First checkpoint at t=1_000 dispatches immediately.
        let first = sync
            .record_progress_intent(
                audio.clone(),
                15_000,
                ProgressKind::Checkpoint,
                false,
                1_000,
                || uuid(300),
            )
            .unwrap();
        assert_eq!(first.operation_id, uuid(300));

        // While `first` is in flight, a checkpoint at 17_000 and then an explicit backward seek to
        // 5_000 coalesce into `ExplicitSeek(5_000)` without sending a second concurrent PUT.
        assert!(sync
            .record_progress_intent(
                audio.clone(),
                17_000,
                ProgressKind::Checkpoint,
                false,
                2_000,
                || uuid(301),
            )
            .is_none());
        assert!(sync
            .record_progress_intent(
                audio.clone(),
                5_000,
                ProgressKind::ExplicitSeek,
                true,
                2_500,
                || uuid(302),
            )
            .is_none());
        // A subsequent timeupdate checkpoint at 5_200 before `first` settles updates position
        // without downgrading `ExplicitSeek` to `Checkpoint`.
        assert!(sync
            .record_progress_intent(
                audio.clone(),
                5_200,
                ProgressKind::Checkpoint,
                false,
                2_700,
                || uuid(303),
            )
            .is_none());

        // When `first` resolves at revision 1, the coalesced rewind dispatches with base_revision=1.
        let applied = sync
            .apply_receipt(
                &uuid(300),
                Receipt {
                    operation_id: uuid(300),
                    outcome: WriteOutcome::Accepted,
                    resolution: None,
                    state: MutationState::Progress {
                        current: Some(ProgressRecord {
                            audio: audio.clone(),
                            position_ms: 15_000,
                            completion: Completion::InProgress,
                            revision: 1,
                            accepted_at_ms: 1_100,
                            listener_order: 1,
                            written_by: uuid(1),
                            kind: ProgressKind::Checkpoint,
                        }),
                    },
                },
                false,
                || uuid(304),
                Some(2_800),
            )
            .unwrap();
        let second = applied
            .next_dispatch
            .expect("coalesced rewind must dispatch");
        let MutationPayload::Progress { write, .. } = second.payload else {
            panic!("expected progress write");
        };
        assert_eq!(write.base_revision, 1);
        assert_eq!(write.position_ms, 5_200);
        assert_eq!(write.kind, ProgressKind::ExplicitSeek);
        assert_eq!(write.operation.device_sequence, 2);

        // Confirm the rewind at revision 2.
        sync.apply_receipt(
            &uuid(304),
            Receipt {
                operation_id: uuid(304),
                outcome: WriteOutcome::Accepted,
                resolution: None,
                state: MutationState::Progress {
                    current: Some(ProgressRecord {
                        audio: audio.clone(),
                        position_ms: 5_200,
                        completion: Completion::InProgress,
                        revision: 2,
                        accepted_at_ms: 2_900,
                        listener_order: 2,
                        written_by: uuid(1),
                        kind: ProgressKind::ExplicitSeek,
                    }),
                },
            },
            false,
            || uuid(305),
            Some(2_900),
        )
        .unwrap();
        assert_eq!(
            sync.progress_for(&pub_id)
                .unwrap()
                .record
                .as_ref()
                .unwrap()
                .position_ms,
            5_200
        );

        // An older snapshot (revision 1, 15_000) arriving late from a page read never overwrites revision 2.
        assert!(!sync.observe_progress_record(ProgressRecord {
            audio,
            position_ms: 15_000,
            completion: Completion::InProgress,
            revision: 1,
            accepted_at_ms: 1_100,
            listener_order: 1,
            written_by: uuid(1),
            kind: ProgressKind::Checkpoint,
        }));
        assert_eq!(
            sync.progress_for(&pub_id)
                .unwrap()
                .record
                .as_ref()
                .unwrap()
                .position_ms,
            5_200
        );
    }

    #[test]
    fn competing_progress_listener_chooses_and_keep_server_follow_contract_rules() {
        let audio = sample_audio(1, 2, 3);
        let pub_id = audio.publication_id.clone();
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();

        let write = sync
            .record_progress_intent(
                audio.clone(),
                30_000,
                ProgressKind::ExplicitSeek,
                true,
                1_000,
                || uuid(400),
            )
            .unwrap();
        assert_eq!(write.operation_id, uuid(400));

        // Server returns 409 RevisionConflict with Resolution::ListenerChooses at revision 4 (position 90_000).
        let server_record = ProgressRecord {
            audio: audio.clone(),
            position_ms: 90_000,
            completion: Completion::InProgress,
            revision: 4,
            accepted_at_ms: 1_200,
            listener_order: 4,
            written_by: uuid(9),
            kind: ProgressKind::ExplicitSeek,
        };
        sync.apply_receipt(
            &uuid(400),
            Receipt {
                operation_id: uuid(400),
                outcome: WriteOutcome::RevisionConflict,
                resolution: Some(Resolution::ListenerChooses),
                state: MutationState::Progress {
                    current: Some(server_record.clone()),
                },
            },
            false,
            || uuid(401),
            Some(1_300),
        )
        .unwrap();

        // Background checkpoints are blocked while ListenerChooses conflict is active.
        assert!(sync
            .record_progress_intent(
                audio.clone(),
                35_000,
                ProgressKind::Checkpoint,
                true,
                15_000,
                || uuid(402),
            )
            .is_none());

        // Listener chooses `ResolveHere` (keep local 30_000) against server revision 4.
        let (resolved, pos) = sync
            .resolve_progress_conflict(&pub_id, true, || uuid(403), Some(16_000))
            .unwrap();
        assert_eq!(pos, 30_000);
        let MutationPayload::Progress { write, .. } = resolved.payload else {
            panic!("expected progress resolution");
        };
        assert_eq!(write.base_revision, 4);
        assert_eq!(write.position_ms, 30_000);
        assert_eq!(write.kind, ProgressKind::ResolveHere);
    }

    #[test]
    fn history_starts_only_once_per_engine_session_and_clear_epoch_blocks_delayed_starts() {
        let audio = sample_audio(1, 2, 3);
        let release = PinnedRelease::admit(
            &cantos_contracts::theatre::PublicationManifest {
                schema_version: "1".into(),
                publication_id: audio.publication_id.clone(),
                work_id: audio.work_id.clone(),
                unit_id: audio.unit_id.clone(),
                rendition_id: audio.rendition_id.clone(),
                manifest_digest: format!("rm-r1:sha256:{}", "b".repeat(64)),
                sha256: audio.sha256.clone(),
                byte_length: 1024,
                duration_ms: audio.duration_ms,
                container: "ogg".into(),
                codec: "opus".into(),
                profile_id: "opus-mono-24-v1".into(),
                channels: 1,
                sample_rate_hz: 48_000,
                language: "vi-VN".into(),
                published_at_ms: 1_000,
            },
            crate::core::public_track::ReleaseRequest::parse(
                PublicWorkId::parse(&audio.work_id).unwrap(),
                &audio.unit_id,
                &audio.publication_id,
            )
            .unwrap(),
        )
        .unwrap();
        let source = Source::public(release.track(), release.duration_ms()).unwrap();
        let session = CallbackIdentity {
            generation: 1,
            track: source.track(),
        };

        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();
        assert!(sync.observe_history_state(HistoryState {
            revision: 2,
            epoch: 1,
        }));

        let first = sync
            .record_engine_started(session, source, || uuid(50), || uuid(500), Some(1_000))
            .unwrap();
        let MutationPayload::HistoryStart { write } = &first.payload else {
            panic!("expected history start");
        };
        assert_eq!(write.engine_session_id, uuid(50));
        assert_eq!(write.history_epoch, 1);

        // Duplicate EngineStarted in the same session (even after generation advances on pause/play) is ignored.
        let resumed = CallbackIdentity {
            generation: 2,
            track: source.track(),
        };
        assert!(sync
            .record_engine_started(resumed, source, || uuid(51), || uuid(501), Some(2_000))
            .is_none());

        // If the server returns 409 HistoryEpochConflict because history was cleared on another
        // device (moving to epoch 2, revision 3), the old start is dropped and never resent!
        let applied = sync
            .apply_receipt(
                &uuid(500),
                Receipt {
                    operation_id: uuid(500),
                    outcome: WriteOutcome::HistoryEpochConflict,
                    resolution: Some(Resolution::KeepServer),
                    state: MutationState::History {
                        current: HistoryState {
                            revision: 3,
                            epoch: 2,
                        },
                    },
                },
                false,
                || uuid(502),
                Some(2_500),
            )
            .unwrap();
        assert!(applied.next_dispatch.is_none());
        assert_eq!(
            sync.history_state(),
            Some(HistoryState {
                revision: 3,
                epoch: 2
            })
        );
    }

    #[test]
    fn unsave_and_clear_history_preserve_other_library_sections() {
        let audio = sample_audio(1, 2, 3);
        let work_id = audio.work_id.clone();
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();
        let mut projection = super::super::library::LibraryProjection::default();

        let progress = ProgressRecord {
            audio: audio.clone(),
            position_ms: 40_000,
            completion: Completion::InProgress,
            revision: 1,
            accepted_at_ms: 1_000,
            listener_order: 1,
            written_by: uuid(1),
            kind: ProgressKind::Checkpoint,
        };
        projection.continuing.push(PresentedContinueItem {
            progress: progress.clone(),
            resume: ResumeStatus::ExactAvailable,
            presentation: None,
        });
        projection.saved.push(PresentedSavedItem {
            saved: SavedRecord {
                work_id: work_id.clone(),
                saved: true,
                revision: 1,
                listener_order: 1,
                accepted_at_ms: 1_000,
            },
            presentation: None,
        });
        projection.history.push(PresentedHistoryItem {
            audio: audio.clone(),
            started_at_ms: 1_000,
            listener_order: 1,
            progress: Some(progress.clone()),
            resume: ResumeStatus::ExactAvailable,
            presentation: None,
        });
        projection.history_state = Some(HistoryState {
            revision: 1,
            epoch: 0,
        });

        // Unsave removes from `saved` only; `continuing` and `history` remain intact.
        assert!(sync.observe_saved_record(SavedRecord {
            work_id,
            saved: false,
            revision: 2,
            listener_order: 2,
            accepted_at_ms: 2_000,
        }));
        sync.sync_projection(&mut projection);
        assert!(projection.saved.is_empty());
        assert_eq!(projection.continuing.len(), 1);
        assert_eq!(projection.history.len(), 1);

        // Clear history advances epoch and clears `history` only; `continuing` remains intact.
        assert!(sync.observe_history_state(HistoryState {
            revision: 2,
            epoch: 1,
        }));
        sync.sync_projection(&mut projection);
        assert!(projection.history.is_empty());
        assert_eq!(projection.continuing.len(), 1);
    }

    #[test]
    fn journal_json_roundtrip_recovers_pending_mutations_for_same_actor_only() {
        let work = uuid(42);
        let mut sync = LibrarySyncState::new("listener-a", uuid(1)).unwrap();
        let pending = sync
            .request_saved(&work, true, uuid(900), Some(1_000))
            .unwrap();
        let json = sync.to_journal().to_json_string().expect("journal json");
        let parsed = AccountSyncJournal::from_json_str(&json).expect("parse journal");
        assert!(parsed.clone().restore_for_actor("listener-b").is_none());

        let restored_journal = parsed
            .restore_for_actor("listener-a")
            .expect("restore for listener-a");
        let restored = LibrarySyncState::from_journal(restored_journal);
        let recovered = restored.pending_mutations();
        assert_eq!(restored.device_id(), uuid(1));
        assert_eq!(recovered.len(), 1);
        assert_eq!(recovered[0].operation_id, pending.operation_id);
        assert_eq!(recovered[0].recovery, RecoveryMode::ReadReceiptFirst);
    }
}

/// Refuse unreadable, oversized or cross-account durable state without replacing it.
pub fn decode_journal(
    raw: &str,
    actor: &str,
) -> Result<AccountSyncJournal, super::account_view::JournalStorageFault> {
    use super::account_view::JournalStorageFault;
    if raw.len() > 65_536 {
        return Err(JournalStorageFault::Corrupt);
    }
    AccountSyncJournal::from_json_str(raw)
        .and_then(|journal| journal.restore_for_actor(actor))
        .ok_or(JournalStorageFault::Corrupt)
}
#[cfg(test)]
mod journal_refusal_tests {
    use super::*;
    #[test]
    fn malformed_and_oversized_journals_are_explicit_failures_not_absence() {
        for raw in [
            "{}".to_owned(),
            "null".to_owned(),
            "invalid".to_owned(),
            " ".repeat(65_537),
        ] {
            assert_eq!(
                decode_journal(&raw, "a").err(),
                Some(super::super::account_view::JournalStorageFault::Corrupt)
            );
        }
    }
}
