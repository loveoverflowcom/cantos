//! The effects Forge needs from its environment, as the smallest traits with two implementations.

use std::path::Path;

use crate::domain::{
    artifact::{ArtifactRevision, ReplayError},
    checks::{CheckResult, Detail},
    digest::Sha256Digest,
    evaluation::Evidence,
    handoff::ExportPlan,
    identity::{ArtifactId, EvidenceId},
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
