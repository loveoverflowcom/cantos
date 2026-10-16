//! A workspace as a directory of write-once files.
//!
//! ```text
//! <root>/forge-workspace.json        marker: {"schema":"cantos-forge.workspace/v1"}
//! <root>/artifacts/<art_id>/r00000001.json   one immutable revision record per file
//! <root>/evidence/<evd_id>.json      one append-only evidence record per file
//! <root>/blobs/sha256-<hex>          exact bytes, named by their digest
//! ```
//!
//! A new file is written under a temporary name and then hard-linked to its final name, so a
//! reader never sees half a record, a crash leaves no truncated record, and a writer that loses
//! a race gets `Conflict` instead of overwriting. Reads refuse symbolic links and anything that
//! is not a regular file, bound their size and re-verify digests. The lexical checks stay in
//! the domain; these are the file-system-level ones. A time-of-check/time-of-use race against a
//! hostile local process that swaps a file between the check and the read is out of scope: the
//! workspace is the producer's own directory.

use std::{
    fs::{self, File, OpenOptions},
    io::{ErrorKind, Read, Write},
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
};

use crate::{
    application::ports::{
        Bundle, BundleError, BundleReceipt, Corruption, InitOutcome, RecordedEvidence, Runtime,
        SourceError, Store, StoreError, MAX_SOURCE_BYTES,
    },
    domain::{
        artifact::{replay, ArtifactRevision},
        digest::Sha256Digest,
        identity::ArtifactId,
    },
    wire::{EvidenceWire, RevisionWire},
};

use super::{bundle, ids::OsIds};

const MARKER_FILE: &str = "forge-workspace.json";
const WORKSPACE_SCHEMA: &str = "cantos-forge.workspace/v1";
const ARTIFACTS: &str = "artifacts";
const EVIDENCE: &str = "evidence";
const BLOBS: &str = "blobs";
const TEMP_PREFIX: &str = ".tmp-";
/// A record is a few hundred bytes; anything near this size was not written by Forge.
const MAX_RECORD_BYTES: u64 = 1024 * 1024;

#[derive(Default)]
pub struct FsRuntime {
    ids: OsIds,
}

impl FsRuntime {
    pub fn new() -> Self {
        Self::default()
    }
}

impl Runtime for FsRuntime {
    type Store = FsStore;
    type Ids = OsIds;

    fn ids(&self) -> &OsIds {
        &self.ids
    }

    fn initialize(&self, root: &Path) -> Result<InitOutcome, StoreError> {
        match fs::metadata(root) {
            Ok(metadata) if !metadata.is_dir() => {
                return Err(unavailable(
                    "initialize",
                    "the path exists and is not a directory",
                ))
            }
            Ok(_) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {
                fs::create_dir_all(root).map_err(|error| io("create the workspace", &error))?;
            }
            Err(error) => return Err(io("inspect the workspace path", &error)),
        }
        let marker = root.join(MARKER_FILE);
        match read_regular(&marker, MAX_RECORD_BYTES) {
            Ok(bytes) => {
                check_marker(&bytes)?;
                return Ok(InitOutcome::AlreadyInitialized);
            }
            Err(ReadFailure::Missing) => {}
            Err(other) => return Err(other.into_store(&marker)),
        }
        if !directory_is_empty(root)? {
            return Err(StoreError::NotEmptyDirectory);
        }
        for directory in [ARTIFACTS, EVIDENCE, BLOBS] {
            fs::create_dir(root.join(directory))
                .map_err(|error| io("create a directory", &error))?;
        }
        let marker_text = format!("{{\"schema\":\"{WORKSPACE_SCHEMA}\"}}\n");
        match publish_new(&marker, marker_text.as_bytes()) {
            Ok(()) => Ok(InitOutcome::Created),
            // A concurrent init won the race to write the same marker.
            Err(StoreError::Conflict) => Ok(InitOutcome::AlreadyInitialized),
            Err(error) => Err(error),
        }
    }

    fn open(&self, root: &Path) -> Result<FsStore, StoreError> {
        let marker = root.join(MARKER_FILE);
        match read_regular(&marker, MAX_RECORD_BYTES) {
            Ok(bytes) => check_marker(&bytes)?,
            Err(ReadFailure::Missing) => return Err(StoreError::NotInitialized),
            Err(other) => return Err(other.into_store(&marker)),
        }
        Ok(FsStore {
            root: root.to_path_buf(),
        })
    }

    fn read_source(&self, path: &Path) -> Result<Vec<u8>, SourceError> {
        let metadata = fs::metadata(path).map_err(|error| SourceError::Unavailable {
            detail: error.to_string(),
        })?;
        if !metadata.is_file() {
            return Err(SourceError::NotARegularFile);
        }
        let mut bytes = Vec::new();
        File::open(path)
            .and_then(|file| file.take(MAX_SOURCE_BYTES + 1).read_to_end(&mut bytes))
            .map_err(|error| SourceError::Unavailable {
                detail: error.to_string(),
            })?;
        if bytes.len() as u64 > MAX_SOURCE_BYTES {
            return Err(SourceError::TooLarge {
                max_bytes: MAX_SOURCE_BYTES,
            });
        }
        Ok(bytes)
    }

    fn write_bundle(
        &self,
        destination: &Path,
        bundle: &Bundle,
    ) -> Result<BundleReceipt, BundleError> {
        bundle::write(destination, bundle)
    }
}

pub struct FsStore {
    root: PathBuf,
}

impl FsStore {
    fn blob_path(&self, digest: Sha256Digest) -> PathBuf {
        self.root
            .join(BLOBS)
            .join(digest.to_string().replace(':', "-"))
    }
}

impl Store for FsStore {
    fn revisions(&self) -> Result<Vec<ArtifactRevision>, StoreError> {
        let mut artifacts = Vec::new();
        for entry in list_dir(&self.root.join(ARTIFACTS))? {
            let name = entry.name;
            let artifact = ArtifactId::parse(&name).map_err(|_| unexpected(&entry.path))?;
            if !entry.is_real_directory {
                return Err(not_regular(&entry.path));
            }
            artifacts.push((artifact, entry.path));
        }
        artifacts.sort_by_key(|(artifact, _)| *artifact);
        let mut all = Vec::new();
        for (artifact, directory) in artifacts {
            all.extend(read_artifact(artifact, &directory)?);
        }
        Ok(all)
    }

    fn evidence(&self) -> Result<Vec<RecordedEvidence>, StoreError> {
        let mut records = Vec::new();
        for entry in list_dir(&self.root.join(EVIDENCE))? {
            let name = entry
                .name
                .strip_suffix(".json")
                .unwrap_or(&entry.name)
                .to_owned();
            if !entry.name.ends_with(".json")
                || crate::domain::identity::EvidenceId::parse(&name).is_err()
            {
                return Err(unexpected(&entry.path));
            }
            let bytes = read_record(&entry.path)?;
            let wire: EvidenceWire =
                serde_json::from_slice(&bytes).map_err(|_| malformed(&entry.path))?;
            let (evidence, detail) = wire.into_record().map_err(|_| malformed(&entry.path))?;
            if evidence.id.to_string() != name {
                return Err(malformed(&entry.path));
            }
            records.push(RecordedEvidence { evidence, detail });
        }
        records.sort_by_key(|record| record.evidence.id);
        Ok(records)
    }

    fn put_blob(&self, digest: Sha256Digest, bytes: &[u8]) -> Result<(), StoreError> {
        if Sha256Digest::of(bytes) != digest {
            return Err(StoreError::Corrupt(Corruption::BlobDigestMismatch {
                digest,
            }));
        }
        match publish_new(&self.blob_path(digest), bytes) {
            Ok(()) => Ok(()),
            // Already stored: identical bytes, provided the stored file still is what it claims.
            Err(StoreError::Conflict) => self.blob(digest).map(|_| ()),
            Err(error) => Err(error),
        }
    }

    fn blob(&self, digest: Sha256Digest) -> Result<Vec<u8>, StoreError> {
        let path = self.blob_path(digest);
        let bytes = match read_regular(&path, MAX_SOURCE_BYTES) {
            Ok(bytes) => bytes,
            Err(ReadFailure::Missing) => {
                return Err(StoreError::Corrupt(Corruption::MissingBlob { digest }))
            }
            Err(other) => return Err(other.into_store(&path)),
        };
        if Sha256Digest::of(&bytes) != digest {
            return Err(StoreError::Corrupt(Corruption::BlobDigestMismatch {
                digest,
            }));
        }
        Ok(bytes)
    }

    fn append_revision(&self, revision: &ArtifactRevision) -> Result<(), StoreError> {
        let directory = self
            .root
            .join(ARTIFACTS)
            .join(revision.artifact().to_string());
        fs::create_dir_all(&directory)
            .map_err(|error| io("create the artifact directory", &error))?;
        let wire =
            RevisionWire::from_facts(revision.artifact(), revision.revision(), revision.facts());
        publish_new(
            &directory.join(revision_file(revision.revision().get())),
            &encode(&wire)?,
        )
    }

    fn append_evidence(&self, record: &RecordedEvidence) -> Result<(), StoreError> {
        let wire = EvidenceWire::from_record(&record.evidence, &record.detail);
        let name = format!("{}.json", record.evidence.id);
        publish_new(&self.root.join(EVIDENCE).join(name), &encode(&wire)?)
    }
}

fn revision_file(number: u32) -> String {
    format!("r{number:08}.json")
}

fn parse_revision_file(name: &str) -> Option<u32> {
    let digits = name.strip_prefix('r')?.strip_suffix(".json")?;
    let canonical = digits.len() == 8 && digits.bytes().all(|byte| byte.is_ascii_digit());
    canonical.then(|| digits.parse().ok()).flatten()
}

/// Reads and re-checks every revision of one artifact: file names, claimed numbers, and the
/// domain's own registration rules replayed over the recorded facts.
fn read_artifact(
    artifact: ArtifactId,
    directory: &Path,
) -> Result<Vec<ArtifactRevision>, StoreError> {
    let mut files = Vec::new();
    for entry in list_dir(directory)? {
        let number = parse_revision_file(&entry.name).ok_or_else(|| unexpected(&entry.path))?;
        files.push((number, entry.path));
    }
    files.sort();
    let mut recorded = Vec::new();
    for (index, (number, path)) in files.iter().enumerate() {
        let bytes = read_record(path)?;
        let wire: RevisionWire = serde_json::from_slice(&bytes).map_err(|_| malformed(path))?;
        let (claimed_artifact, claimed_number, facts) =
            wire.into_facts().map_err(|_| malformed(path))?;
        let in_sequence = usize::try_from(*number).ok() == Some(index + 1);
        if claimed_artifact != artifact || claimed_number != *number || !in_sequence {
            return Err(StoreError::Corrupt(Corruption::RevisionNumbering {
                artifact,
            }));
        }
        recorded.push(facts);
    }
    replay(artifact, recorded)
        .map_err(|error| StoreError::Corrupt(Corruption::HistoryRejected { artifact, error }))
}

fn check_marker(bytes: &[u8]) -> Result<(), StoreError> {
    #[derive(serde::Deserialize)]
    #[serde(deny_unknown_fields)]
    struct Marker {
        schema: String,
    }
    match serde_json::from_slice::<Marker>(bytes) {
        Ok(marker) if marker.schema == WORKSPACE_SCHEMA => Ok(()),
        Ok(marker) => Err(StoreError::UnsupportedWorkspace {
            found: marker.schema,
        }),
        Err(_) => Err(StoreError::Corrupt(Corruption::MalformedRecord {
            record: MARKER_FILE.to_owned(),
        })),
    }
}

fn encode<T: serde::Serialize>(value: &T) -> Result<Vec<u8>, StoreError> {
    let mut bytes = serde_json::to_vec(value)
        .map_err(|error| unavailable("encode a record", &error.to_string()))?;
    bytes.push(b'\n');
    Ok(bytes)
}

struct Entry {
    name: String,
    path: PathBuf,
    is_real_directory: bool,
}

/// The entries of a Forge-owned directory, without temporary files of an interrupted writer.
fn list_dir(directory: &Path) -> Result<Vec<Entry>, StoreError> {
    let reader =
        fs::read_dir(directory).map_err(|error| io("list a workspace directory", &error))?;
    let mut entries = Vec::new();
    for entry in reader {
        let entry = entry.map_err(|error| io("list a workspace directory", &error))?;
        let path = entry.path();
        let name = entry
            .file_name()
            .into_string()
            .map_err(|_| unexpected(&path))?;
        if name.starts_with(TEMP_PREFIX) {
            continue;
        }
        let file_type = entry
            .file_type()
            .map_err(|error| io("inspect a workspace entry", &error))?;
        entries.push(Entry {
            name,
            path,
            is_real_directory: file_type.is_dir(),
        });
    }
    Ok(entries)
}

fn directory_is_empty(directory: &Path) -> Result<bool, StoreError> {
    let mut reader =
        fs::read_dir(directory).map_err(|error| io("list the workspace path", &error))?;
    Ok(reader.next().is_none())
}

enum ReadFailure {
    Missing,
    NotRegular,
    TooLarge,
    Io(String),
}

impl ReadFailure {
    fn into_store(self, path: &Path) -> StoreError {
        match self {
            Self::Missing => unexpected(path),
            Self::NotRegular => not_regular(path),
            Self::TooLarge => StoreError::Corrupt(Corruption::TooLarge {
                record: display(path),
            }),
            Self::Io(detail) => unavailable("read a record", &detail),
        }
    }
}

/// Reads a regular file of at most `max_bytes`; a symbolic link is not a regular file.
fn read_regular(path: &Path, max_bytes: u64) -> Result<Vec<u8>, ReadFailure> {
    let metadata = match fs::symlink_metadata(path) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == ErrorKind::NotFound => return Err(ReadFailure::Missing),
        Err(error) => return Err(ReadFailure::Io(error.to_string())),
    };
    if !metadata.file_type().is_file() {
        return Err(ReadFailure::NotRegular);
    }
    if metadata.len() > max_bytes {
        return Err(ReadFailure::TooLarge);
    }
    let mut bytes = Vec::new();
    File::open(path)
        .and_then(|file| file.take(max_bytes + 1).read_to_end(&mut bytes))
        .map_err(|error| ReadFailure::Io(error.to_string()))?;
    if bytes.len() as u64 > max_bytes {
        return Err(ReadFailure::TooLarge);
    }
    Ok(bytes)
}

fn read_record(path: &Path) -> Result<Vec<u8>, StoreError> {
    read_regular(path, MAX_RECORD_BYTES).map_err(|error| match error {
        ReadFailure::Missing => unexpected(path),
        other => other.into_store(path),
    })
}

static TEMP_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Creates `path` with exactly `bytes`, atomically and without ever replacing a file.
pub(super) fn publish_new(path: &Path, bytes: &[u8]) -> Result<(), StoreError> {
    let directory = path
        .parent()
        .ok_or_else(|| unavailable("publish a record", "the record has no directory"))?;
    let temporary = directory.join(format!(
        "{TEMP_PREFIX}{}-{}",
        std::process::id(),
        TEMP_COUNTER.fetch_add(1, Ordering::Relaxed)
    ));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&temporary)
        .map_err(|error| io("create a temporary record", &error))?;
    let written = file.write_all(bytes).and_then(|()| file.sync_all());
    drop(file);
    let linked = written
        .map_err(|error| io("write a record", &error))
        .and_then(|()| match fs::hard_link(&temporary, path) {
            Ok(()) => Ok(()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => Err(StoreError::Conflict),
            Err(error) => Err(io("publish a record", &error)),
        });
    // The temporary name is ours; whatever happened, it must not outlive this call.
    let _ = fs::remove_file(&temporary);
    if linked.is_ok() {
        // Best effort: make the new directory entry durable.
        let _ = File::open(directory).and_then(|directory| directory.sync_all());
    }
    linked
}

fn display(path: &Path) -> String {
    path.display().to_string()
}

fn malformed(path: &Path) -> StoreError {
    StoreError::Corrupt(Corruption::MalformedRecord {
        record: display(path),
    })
}

fn not_regular(path: &Path) -> StoreError {
    StoreError::Corrupt(Corruption::NotARegularFile {
        record: display(path),
    })
}

fn unexpected(path: &Path) -> StoreError {
    StoreError::Corrupt(Corruption::UnexpectedEntry {
        record: display(path),
    })
}

fn unavailable(operation: &'static str, detail: &str) -> StoreError {
    StoreError::Unavailable {
        operation,
        detail: detail.to_owned(),
    }
}

fn io(operation: &'static str, error: &std::io::Error) -> StoreError {
    unavailable(operation, &error.to_string())
}
