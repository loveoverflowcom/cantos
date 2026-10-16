//! The effects Forge needs from its environment, as the smallest traits with two implementations.

use std::path::Path;

use crate::domain::{
    artifact::{ArtifactRevision, ReplayError},
    attempt::{AttemptNumber, Event, Rejection, UncertainReason},
    checks::{CheckResult, Detail},
    digest::Sha256Digest,
    evaluation::Evidence,
    generation::GenerationRequest,
    handoff::ExportPlan,
    identity::{ArtifactId, EvidenceId},
    label::OperationKey,
};

/// Evidence plus the measurements behind it. The record is append-only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RecordedEvidence {
    pub evidence: Evidence,
    pub detail: Detail,
}

/// Why a stored record cannot be trusted. Reading fails closed with one of these.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Corruption {
    /// The record is not valid JSON of the expected shape, or holds a value a constructor refuses.
    MalformedRecord { record: String },
    /// A record or blob is a symbolic link, directory or other non-regular file.
    NotARegularFile { record: String },
    /// A name in the workspace that Forge did not write.
    UnexpectedEntry { record: String },
    /// A blob's bytes do not hash to the digest in its name.
    BlobDigestMismatch { digest: Sha256Digest },
    /// A revision names bytes that are not in the workspace.
    MissingBlob { digest: Sha256Digest },
    /// A revision file does not carry the number its position or its name says.
    RevisionNumbering { artifact: ArtifactId },
    /// The recorded history is one registration would not have produced.
    HistoryRejected {
        artifact: ArtifactId,
        error: ReplayError,
    },
    /// The record is larger than Forge reads.
    TooLarge { record: String },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreError {
    /// The path is not a Forge workspace (no marker). Action: `workspace init`.
    NotInitialized,
    /// The marker names a workspace format this build does not know.
    UnsupportedWorkspace {
        found: String,
    },
    /// `init` was pointed at a directory that holds other files and no workspace marker.
    NotEmptyDirectory,
    /// Another writer created the same record first; nothing was overwritten.
    Conflict,
    Corrupt(Corruption),
    /// The operating system refused or failed an operation.
    Unavailable {
        operation: &'static str,
        detail: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InitOutcome {
    Created,
    /// A workspace of this format already exists; nothing was written.
    AlreadyInitialized,
}

/// The persistent record of one workspace.
///
/// Appends never overwrite. `append_revision` and `append_evidence` fail with
/// [`StoreError::Conflict`] when the record already exists, which is how a lost race against
/// another writer is detected instead of silently losing a revision.
pub trait Store {
    /// Every revision of every artifact, per artifact in revision order, already re-checked
    /// by the domain's registration rules.
    fn revisions(&self) -> Result<Vec<ArtifactRevision>, StoreError>;
    fn evidence(&self) -> Result<Vec<RecordedEvidence>, StoreError>;
    /// Stores bytes under their digest. The store refuses bytes that do not hash to `digest`.
    fn put_blob(&self, digest: Sha256Digest, bytes: &[u8]) -> Result<(), StoreError>;
    /// Reads bytes and verifies them against `digest`.
    fn blob(&self, digest: Sha256Digest) -> Result<Vec<u8>, StoreError>;
    fn append_revision(&self, revision: &ArtifactRevision) -> Result<(), StoreError>;
    fn append_evidence(&self, record: &RecordedEvidence) -> Result<(), StoreError>;
}

/// The shell's randomness is unavailable, so no identity can be issued.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IdsUnavailable;

/// Local identities come from the shell's randomness, never from the domain.
pub trait Ids {
    fn artifact_id(&self) -> Result<ArtifactId, IdsUnavailable>;
    fn evidence_id(&self) -> Result<EvidenceId, IdsUnavailable>;
}

/// Files for one handoff bundle. Writing it creates local files only: nothing is uploaded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Bundle {
    pub plan: ExportPlan,
    pub audio: Vec<u8>,
    /// The profile check run again on `audio` while preparing the bundle.
    pub measured: CheckResult,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BundleReceipt {
    pub manifest: String,
    pub audio: String,
    pub manifest_digest: Sha256Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BundleError {
    /// The destination already exists; a bundle is never merged into or overwritten.
    DestinationExists,
    Unavailable {
        operation: &'static str,
        detail: String,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SourceError {
    NotARegularFile,
    TooLarge { max_bytes: u64 },
    Unavailable { detail: String },
}

/// Everything the CLI needs from the world, beyond the per-workspace [`Store`].
pub trait Runtime {
    type Store: Store;
    type Ids: Ids;

    fn ids(&self) -> &Self::Ids;
    fn initialize(&self, root: &Path) -> Result<InitOutcome, StoreError>;
    fn open(&self, root: &Path) -> Result<Self::Store, StoreError>;
    /// Reads one operator-named input file, bounded and regular files only.
    fn read_source(&self, path: &Path) -> Result<Vec<u8>, SourceError>;
    fn write_bundle(
        &self,
        destination: &Path,
        bundle: &Bundle,
    ) -> Result<BundleReceipt, BundleError>;
}

/// The largest input file Forge reads into memory: twice the audio body limit.
pub const MAX_SOURCE_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum AttemptStoreError {
    /// The position already holds an event: another writer got there first, or this very write
    /// happened and only its response was lost. A store cannot tell which, and neither can the
    /// caller from the error alone.
    Conflict,
    Unavailable {
        operation: &'static str,
        detail: String,
    },
}

/// The durable, append-only history of generation operations, one ordered list per key.
///
/// It is the only authority on whether a billable call may start: the reservation event is
/// written here **before** the call, and a write that loses a race fails with
/// [`AttemptStoreError::Conflict`] instead of overwriting. Nothing here is a Catalog: it holds
/// producer-side attempt facts, never content, grants or publication state.
pub trait AttemptStore {
    /// The events of `key` in order, or an empty list when the key has no history.
    fn events(&self, key: &OperationKey) -> Result<Vec<Event>, AttemptStoreError>;
    /// Writes `event` as number `index` of the history, once. `index` must be the number of
    /// events already stored; any other position, or a taken one, is a `Conflict`.
    fn append(
        &self,
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<(), AttemptStoreError>;
}

/// Proof that this invocation won the reservation of one attempt and may make that attempt's
/// one provider call. Only the use cases can create it, and only after the reservation event
/// was appended, so an adapter cannot be asked to call a provider without one.
#[derive(Debug)]
pub struct Reserved {
    attempt: AttemptNumber,
}

impl Reserved {
    pub(in crate::application) fn new(attempt: AttemptNumber) -> Self {
        Self { attempt }
    }

    pub fn attempt(&self) -> AttemptNumber {
        self.attempt
    }
}

/// What a provider adapter reports. `Unknown` is for every case where the call may have been
/// made or billed and no definite answer arrived; an adapter never guesses `Rejected`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ProviderResult {
    /// The raw bytes the provider returned: not a Cantos rendition.
    Audio(Vec<u8>),
    Rejected(Rejection),
    Unknown(UncertainReason),
}

/// One bounded speech provider. `synthesize` makes at most one billable call and never retries.
pub trait Provider {
    fn synthesize(&self, reserved: &Reserved, request: &GenerationRequest) -> ProviderResult;
}
