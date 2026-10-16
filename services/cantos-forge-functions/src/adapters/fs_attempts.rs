//! A durable attempt ledger for one trusted, local filesystem, configured by the operator.
//!
//! This is not a Drive adapter, a distributed lock, or a Functions replica store. The directory
//! must be private to cooperating Forge processes; hostile local replacement of files between
//! metadata inspection and opening is outside this adapter's boundary. The filesystem must
//! support atomic hard links and directory `sync_all`; unsupported or failed syncs are errors.
//!
//! Each event is a checksummed, versioned, write-once file in a directory named by the operation
//! key's digest. A temporary file is fully written and synced before hard-link publication, then
//! the directory is synced before success. Newly created directories are synced through their
//! parent. A failure after publication is uncertain, never permission to dispatch. Readback
//! syncs the directory too, so the application's outcome reconciliation cannot mistake a failed
//! directory sync for durable success. Open reservations survive restart and are never resumed
//! or recovered automatically.

use std::{
    fs::{self, DirBuilder, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    num::NonZeroU64,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::{
    application::ports::{AttemptStore, AttemptStoreError},
    domain::{
        attempt::{
            AttemptNumber, Cost, Event, Operation, ProviderAudio, Rejection, Settlement,
            UncertainReason,
        },
        digest::Sha256Digest,
        generation::RequestFingerprint,
        label::OperationKey,
    },
};

const MARKER: &str = "forge-attempts.json";
const STORE_SCHEMA: &str = "cantos-forge.local-attempts/v1";
const EVENT_SCHEMA: &str = "cantos-forge.attempt/v1";
const MAX_EVENT_BYTES: u64 = 16 * 1024;
const MAX_HISTORY_BYTES: u64 = 8 * 1024 * 1024;
const MAX_EVENTS: usize = 4096;
const MAX_OPERATIONS: usize = 4096;
const MAX_TEMPORARIES: usize = 64;

/// An operator-configured, single-host ledger. Never construct its root from an HTTP request.
#[derive(Clone, Debug)]
pub struct FsAttemptStore {
    root: PathBuf,
}

impl FsAttemptStore {
    /// Creates a dedicated directory, or opens the existing ledger without changing its facts.
    /// The absolute parent directory must already exist. Interrupted initialization may leave
    /// only bounded, recognizable temporary files; they are never promoted or deleted here.
    pub fn initialize(root: &Path) -> Result<Self, AttemptStoreError> {
        absolute(root)?;
        create_private_directory(root)?;
        let root = real_directory(root)?;
        sync_parent(&root)?;
        let marker = root.join(MARKER);
        if marker
            .try_exists()
            .map_err(|error| io("inspect attempt marker", error))?
        {
            return Self::open(&root);
        }
        check_root(&root, false)?;
        let bytes = serde_json::to_vec(&Marker {
            schema: STORE_SCHEMA.to_owned(),
        })
        .map_err(|error| unavailable("encode attempt marker", error.to_string()))?;
        match publish_new(&marker, &bytes) {
            Ok(()) | Err(AttemptStoreError::Conflict) => Self::open(&root),
            Err(error) => Err(error),
        }
    }

    /// Opens a previously initialized operator directory. It creates no reservation and makes
    /// no provider call. A linked marker whose initialization response was lost is re-synced.
    pub fn open(root: &Path) -> Result<Self, AttemptStoreError> {
        absolute(root)?;
        let root = real_directory(root)?;
        let marker: Marker = serde_json::from_slice(&read_regular(&root.join(MARKER))?)
            .map_err(|_| corrupt("invalid attempt-store marker"))?;
        if marker.schema != STORE_SCHEMA {
            return Err(corrupt("unsupported attempt-store schema"));
        }
        check_root(&root, true)?;
        sync_parent(&root)?;
        sync_directory(&root)?;
        Ok(Self { root })
    }

    fn directory(&self, key: &OperationKey) -> PathBuf {
        self.root.join(format!(
            "op-{}",
            Sha256Digest::of(key.as_str().as_bytes())
                .to_string()
                .trim_start_matches("sha256:")
        ))
    }

    fn read_events(&self, key: &OperationKey) -> Result<(Vec<Event>, u64), AttemptStoreError> {
        // Re-check the configured root too: a previously opened handle is not permission to
        // follow a later replacement symlink or silently accept foreign entries.
        real_directory(&self.root)?;
        check_root(&self.root, true)?;
        let directory = self.directory(key);
        match fs::symlink_metadata(&directory) {
            Ok(metadata) if metadata.file_type().is_dir() => {}
            Ok(_) => return Err(corrupt("operation is not a real directory")),
            Err(error) if error.kind() == ErrorKind::NotFound => return Ok((Vec::new(), 0)),
            Err(error) => return Err(io("inspect attempt operation", error)),
        }
        let files = event_files(&directory)?;
        let mut events = Vec::with_capacity(files.len());
        let mut total = 0_u64;
        for (expected, (index, path)) in files.iter().enumerate() {
            if *index != expected {
                return Err(corrupt("attempt history is not contiguous"));
            }
            let bytes = read_regular(path)?;
            total = total.saturating_add(bytes.len() as u64);
            if total > MAX_HISTORY_BYTES {
                return Err(corrupt("attempt history exceeds byte limit"));
            }
            let stored: StoredEvent =
                serde_json::from_slice(&bytes).map_err(|_| corrupt("malformed attempt event"))?;
            events.push(stored.into_event(key, *index)?);
        }
        validate_history(key, &events)?;
        // An append may have linked its event but failed its directory sync. A successful
        // readback must establish durability before the application can reconcile an outcome.
        sync_directory(&self.root)?;
        sync_directory(&directory)?;
        Ok((events, total))
    }
}

impl AttemptStore for FsAttemptStore {
    fn events(&self, key: &OperationKey) -> Result<Vec<Event>, AttemptStoreError> {
        self.read_events(key).map(|(events, _)| events)
    }

    fn append(
        &self,
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<(), AttemptStoreError> {
        let (mut history, total) = self.read_events(key)?;
        if index != history.len() {
            return Err(AttemptStoreError::Conflict);
        }
        if index >= MAX_EVENTS {
            return Err(corrupt("attempt history has too many records"));
        }
        history.push(event.clone());
        validate_history(key, &history)?;
        let bytes = StoredEvent::encode(key, index, event)?;
        if total.saturating_add(bytes.len() as u64) > MAX_HISTORY_BYTES {
            return Err(corrupt("attempt history exceeds byte limit"));
        }
        let directory = self.directory(key);
        let operations = check_root(&self.root, true)?;
        if operations >= MAX_OPERATIONS && !directory.exists() {
            return Err(capacity_reached());
        }
        #[cfg(test)]
        tests::operation_creation_barrier(false);
        let created = create_private_directory(&directory)?;
        #[cfg(test)]
        tests::operation_creation_barrier(true);
        real_directory(&directory)?;
        if let Err(error) = check_root(&self.root, true) {
            // Different keys can pass the capacity check before either creates a directory.
            // remove_dir is atomic, never recursive, and refuses another writer's record.
            if created {
                remove_unused_directory(&directory)?;
                sync_directory(&self.root)?;
            }
            return Err(error);
        }
        // Persist the operation directory before any reservation can return success.
        sync_directory(&self.root)?;
        publish_new(&directory.join(event_name(index)), &bytes)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Marker {
    schema: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct StoredEvent {
    record: EventRecord,
    digest: String,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct EventRecord {
    schema: String,
    key: String,
    index: usize,
    event: EventWire,
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum EventWire {
    Opened {
        fingerprint: String,
        cost: u64,
    },
    Regenerated {
        attempt: u32,
        cost: u64,
        acknowledged: Option<u32>,
    },
    Settled {
        attempt: u32,
        settlement: SettlementWire,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case", deny_unknown_fields)]
enum SettlementWire {
    Succeeded { digest: String, bytes: u64 },
    Rejected { reason: RejectionWire },
    Uncertain { reason: UncertainWire },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum RejectionWire {
    NotAttempted,
    Refused,
    RateLimited,
    Unavailable,
    InvalidAudio,
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum UncertainWire {
    TimedOut,
    ConnectionLost,
    ResultNotRecorded,
    Abandoned,
}

impl StoredEvent {
    fn encode(
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<Vec<u8>, AttemptStoreError> {
        let record = EventRecord {
            schema: EVENT_SCHEMA.to_owned(),
            key: key.to_string(),
            index,
            event: EventWire::from(event),
        };
        let digest = record.digest()?;
        serde_json::to_vec(&Self { record, digest })
            .map_err(|error| unavailable("encode attempt event", error.to_string()))
    }

    fn into_event(self, key: &OperationKey, index: usize) -> Result<Event, AttemptStoreError> {
        if self.record.schema != EVENT_SCHEMA {
            return Err(corrupt("unsupported attempt-event schema"));
        }
        if self.record.key != key.as_str() || self.record.index != index {
            return Err(corrupt("attempt event belongs to another key or position"));
        }
        if self.record.digest()? != self.digest {
            return Err(corrupt("attempt event checksum mismatch"));
        }
        self.record.event.into_event(key)
    }
}

impl EventRecord {
    fn digest(&self) -> Result<String, AttemptStoreError> {
        serde_json::to_vec(self)
            .map(|bytes| Sha256Digest::of(&bytes).to_string())
            .map_err(|error| unavailable("encode attempt checksum", error.to_string()))
    }
}

impl From<&Event> for EventWire {
    fn from(event: &Event) -> Self {
        match event {
            Event::Opened {
                fingerprint, cost, ..
            } => Self::Opened {
                fingerprint: fingerprint.to_string(),
                cost: cost.units(),
            },
            Event::Regenerated {
                attempt,
                cost,
                acknowledged,
            } => Self::Regenerated {
                attempt: attempt.get(),
                cost: cost.units(),
                acknowledged: acknowledged.map(AttemptNumber::get),
            },
            Event::Settled {
                attempt,
                settlement,
            } => Self::Settled {
                attempt: attempt.get(),
                settlement: SettlementWire::from(*settlement),
            },
        }
    }
}

impl EventWire {
    fn into_event(self, key: &OperationKey) -> Result<Event, AttemptStoreError> {
        let invalid = || corrupt("invalid attempt-event values");
        match self {
            Self::Opened { fingerprint, cost } => Ok(Event::Opened {
                key: key.clone(),
                fingerprint: RequestFingerprint::from_digest(
                    Sha256Digest::parse(&fingerprint).map_err(|_| invalid())?,
                ),
                cost: Cost::new(cost).map_err(|_| invalid())?,
            }),
            Self::Regenerated {
                attempt,
                cost,
                acknowledged,
            } => Ok(Event::Regenerated {
                attempt: AttemptNumber::new(attempt).ok_or_else(invalid)?,
                cost: Cost::new(cost).map_err(|_| invalid())?,
                acknowledged: acknowledged
                    .map(|number| AttemptNumber::new(number).ok_or_else(invalid))
                    .transpose()?,
            }),
            Self::Settled {
                attempt,
                settlement,
            } => Ok(Event::Settled {
                attempt: AttemptNumber::new(attempt).ok_or_else(invalid)?,
                settlement: settlement.into_settlement()?,
            }),
        }
    }
}

impl From<Settlement> for SettlementWire {
    fn from(settlement: Settlement) -> Self {
        match settlement {
            Settlement::Succeeded(audio) => Self::Succeeded {
                digest: audio.digest.to_string(),
                bytes: audio.bytes.get(),
            },
            Settlement::Rejected(reason) => Self::Rejected {
                reason: match reason {
                    Rejection::NotAttempted => RejectionWire::NotAttempted,
                    Rejection::Refused => RejectionWire::Refused,
                    Rejection::RateLimited => RejectionWire::RateLimited,
                    Rejection::Unavailable => RejectionWire::Unavailable,
                    Rejection::InvalidAudio => RejectionWire::InvalidAudio,
                },
            },
            Settlement::Uncertain(reason) => Self::Uncertain {
                reason: match reason {
                    UncertainReason::TimedOut => UncertainWire::TimedOut,
                    UncertainReason::ConnectionLost => UncertainWire::ConnectionLost,
                    UncertainReason::ResultNotRecorded => UncertainWire::ResultNotRecorded,
                    UncertainReason::Abandoned => UncertainWire::Abandoned,
                },
            },
        }
    }
}

impl SettlementWire {
    fn into_settlement(self) -> Result<Settlement, AttemptStoreError> {
        let invalid = || corrupt("invalid attempt-event values");
        Ok(match self {
            Self::Succeeded { digest, bytes } => Settlement::Succeeded(ProviderAudio {
                digest: Sha256Digest::parse(&digest).map_err(|_| invalid())?,
                bytes: NonZeroU64::new(bytes).ok_or_else(invalid)?,
            }),
            Self::Rejected { reason } => Settlement::Rejected(match reason {
                RejectionWire::NotAttempted => Rejection::NotAttempted,
                RejectionWire::Refused => Rejection::Refused,
                RejectionWire::RateLimited => Rejection::RateLimited,
                RejectionWire::Unavailable => Rejection::Unavailable,
                RejectionWire::InvalidAudio => Rejection::InvalidAudio,
            }),
            Self::Uncertain { reason } => Settlement::Uncertain(match reason {
                UncertainWire::TimedOut => UncertainReason::TimedOut,
                UncertainWire::ConnectionLost => UncertainReason::ConnectionLost,
                UncertainWire::ResultNotRecorded => UncertainReason::ResultNotRecorded,
                UncertainWire::Abandoned => UncertainReason::Abandoned,
            }),
        })
    }
}

fn validate_history(key: &OperationKey, events: &[Event]) -> Result<(), AttemptStoreError> {
    if events.is_empty() {
        return Ok(());
    }
    let operation =
        Operation::replay(events).map_err(|_| corrupt("attempt history fails domain replay"))?;
    if operation.key() != key {
        return Err(corrupt("attempt history belongs to another operation"));
    }
    Ok(())
}

fn event_name(index: usize) -> String {
    format!("e{index:08}.json")
}

fn event_index(name: &str) -> Option<usize> {
    let digits = name.strip_prefix('e')?.strip_suffix(".json")?;
    (digits.len() == 8 && digits.bytes().all(|byte| byte.is_ascii_digit()))
        .then(|| digits.parse().ok())
        .flatten()
}

fn event_files(directory: &Path) -> Result<Vec<(usize, PathBuf)>, AttemptStoreError> {
    let mut files = Vec::new();
    let mut temporary_count = 0;
    for entry in fs::read_dir(directory).map_err(|error| io("list attempt events", error))? {
        let entry = entry.map_err(|error| io("list attempt events", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| corrupt("non-UTF-8 attempt entry"))?;
        if temporary(&name) {
            check_temporary(&entry.path(), &mut temporary_count)?;
        } else {
            let index = event_index(&name).ok_or_else(|| corrupt("unexpected attempt entry"))?;
            files.push((index, entry.path()));
            if files.len() > MAX_EVENTS {
                return Err(corrupt("attempt history has too many records"));
            }
        }
    }
    files.sort_by_key(|(index, _)| *index);
    Ok(files)
}

fn check_root(root: &Path, initialized: bool) -> Result<usize, AttemptStoreError> {
    let mut operations = 0;
    let mut temporary_count = 0;
    let mut marker_found = false;
    for entry in fs::read_dir(root).map_err(|error| io("list attempt root", error))? {
        let entry = entry.map_err(|error| io("list attempt root", error))?;
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| corrupt("non-UTF-8 attempt entry"))?;
        if temporary(&name) {
            check_temporary(&entry.path(), &mut temporary_count)?;
        } else if name == MARKER {
            marker_found = true;
            let marker: Marker = serde_json::from_slice(&read_regular(&entry.path())?)
                .map_err(|_| corrupt("invalid attempt-store marker"))?;
            if marker.schema != STORE_SCHEMA {
                return Err(corrupt("unsupported attempt-store schema"));
            }
        } else if initialized && operation_name(&name) {
            operations += 1;
            if operations > MAX_OPERATIONS {
                return Err(corrupt("attempt store has too many operations"));
            }
            if !fs::symlink_metadata(entry.path())
                .map_err(|error| io("inspect attempt operation", error))?
                .file_type()
                .is_dir()
            {
                return Err(corrupt("operation is not a real directory"));
            }
        } else {
            return Err(corrupt("unexpected attempt-root entry"));
        }
    }
    if initialized && !marker_found {
        return Err(corrupt("missing attempt-store marker"));
    }
    Ok(operations)
}

fn operation_name(name: &str) -> bool {
    name.strip_prefix("op-")
        .is_some_and(|hex| Sha256Digest::parse(&format!("sha256:{hex}")).is_ok())
}

fn temporary(name: &str) -> bool {
    name.strip_prefix(".tmp-").is_some_and(|hex| {
        hex.len() == 32
            && hex
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    })
}

fn check_temporary(path: &Path, count: &mut usize) -> Result<(), AttemptStoreError> {
    *count += 1;
    if *count > MAX_TEMPORARIES {
        return Err(corrupt(
            "too many attempt temporary files; operator cleanup required",
        ));
    }
    match fs::symlink_metadata(path) {
        // A cooperating writer can remove its temporary after the directory was listed.
        Err(error) if error.kind() == ErrorKind::NotFound => Ok(()),
        Err(error) => Err(io("inspect attempt temporary", error)),
        Ok(metadata) if !metadata.file_type().is_file() => {
            Err(corrupt("attempt temporary is not a regular file"))
        }
        Ok(metadata) if metadata.len() > MAX_EVENT_BYTES => {
            Err(corrupt("attempt record exceeds byte limit"))
        }
        Ok(_) => Ok(()),
    }
}

fn read_regular(path: &Path) -> Result<Vec<u8>, AttemptStoreError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io("inspect attempt record", error))?;
    if !metadata.file_type().is_file() {
        return Err(corrupt("attempt record is not a regular file"));
    }
    if metadata.len() > MAX_EVENT_BYTES {
        return Err(corrupt("attempt record exceeds byte limit"));
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(MAX_EVENT_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| io("read attempt record", error))?;
    if bytes.len() as u64 > MAX_EVENT_BYTES {
        return Err(corrupt("attempt record exceeds byte limit"));
    }
    Ok(bytes)
}

fn absolute(root: &Path) -> Result<(), AttemptStoreError> {
    if !root.is_absolute() {
        return Err(unavailable(
            "configure local attempt store",
            "root must be an absolute operator directory",
        ));
    }
    Ok(())
}

fn real_directory(path: &Path) -> Result<PathBuf, AttemptStoreError> {
    let metadata =
        fs::symlink_metadata(path).map_err(|error| io("inspect attempt directory", error))?;
    if !metadata.file_type().is_dir() {
        return Err(corrupt("attempt root is not a real directory"));
    }
    fs::canonicalize(path).map_err(|error| io("resolve attempt directory", error))
}

fn create_private_directory(path: &Path) -> Result<bool, AttemptStoreError> {
    let mut builder = DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(path) {
        Ok(()) => Ok(true),
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            real_directory(path).map(|_| false)
        }
        Err(error) => Err(io("create attempt directory", error)),
    }
}

fn remove_unused_directory(path: &Path) -> Result<(), AttemptStoreError> {
    match fs::remove_dir(path) {
        Ok(()) => Ok(()),
        // A cooperating writer can already have claimed this operation. Preserve its files.
        Err(error)
            if matches!(
                error.kind(),
                ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
            ) =>
        {
            Ok(())
        }
        Err(error) => Err(io("remove unused attempt directory", error)),
    }
}

fn capacity_reached() -> AttemptStoreError {
    unavailable(
        "reserve local operation",
        "local attempt store reached operation limit",
    )
}

fn sync_parent(path: &Path) -> Result<(), AttemptStoreError> {
    let parent = path
        .parent()
        .ok_or_else(|| unavailable("sync attempt parent", "directory has no parent"))?;
    sync_directory(parent)
}

fn sync_directory(path: &Path) -> Result<(), AttemptStoreError> {
    #[cfg(test)]
    tests::before_directory_sync(path)?;
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| io("sync attempt directory", error))
}

fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), AttemptStoreError> {
    let directory = path
        .parent()
        .ok_or_else(|| unavailable("publish attempt event", "record has no directory"))?;
    let mut random = [0_u8; 16];
    getrandom::fill(&mut random)
        .map_err(|_| unavailable("name attempt temporary", "OS randomness unavailable"))?;
    let name: String = random.iter().map(|byte| format!("{byte:02x}")).collect();
    let temporary = directory.join(format!(".tmp-{name}"));
    let mut options = OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options
        .open(&temporary)
        .map_err(|error| io("create attempt temporary", error))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let published = written
        .map_err(|error| io("sync attempt bytes", error))
        .and_then(|()| match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {
                Err(AttemptStoreError::Conflict)
            }
            Err(error) => Err(io("publish attempt event", error)),
        });
    // Only our freshly created name is removed. A crash's temporary is never adopted or erased.
    let _ = fs::remove_file(&temporary);
    published?;
    sync_directory(directory)
}

fn corrupt(detail: &'static str) -> AttemptStoreError {
    unavailable("read local attempt history", detail)
}

fn unavailable(operation: &'static str, detail: impl Into<String>) -> AttemptStoreError {
    AttemptStoreError::Unavailable {
        operation,
        detail: detail.into(),
    }
}

fn io(operation: &'static str, error: std::io::Error) -> AttemptStoreError {
    unavailable(operation, error.to_string())
}

#[cfg(test)]
#[path = "fs_attempts_tests.rs"]
mod tests;
