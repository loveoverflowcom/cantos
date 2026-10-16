//! In-memory fakes of the ports, for deterministic use-case and CLI tests.

use std::{
    cell::RefCell,
    collections::{BTreeMap, VecDeque},
    path::Path,
};

use crate::domain::{
    artifact::ArtifactRevision,
    attempt::Event,
    digest::Sha256Digest,
    identity::{ArtifactId, EvidenceId},
    label::OperationKey,
};

use super::ports::*;

/// A workspace held in memory. Interior mutability mirrors a store that other writers share.
#[derive(Default)]
pub struct MemoryStore {
    pub revisions: RefCell<Vec<ArtifactRevision>>,
    pub evidence: RefCell<Vec<RecordedEvidence>>,
    pub blobs: RefCell<BTreeMap<Sha256Digest, Vec<u8>>>,
    /// Make the next `append_revision` fail as if another writer got there first.
    pub lose_next_revision_race: RefCell<bool>,
    /// Make the next `put_blob` fail as if the disk refused the bytes.
    pub fail_next_blob: RefCell<bool>,
}

impl Store for MemoryStore {
    fn revisions(&self) -> Result<Vec<ArtifactRevision>, StoreError> {
        Ok(self.revisions.borrow().clone())
    }

    fn evidence(&self) -> Result<Vec<RecordedEvidence>, StoreError> {
        Ok(self.evidence.borrow().clone())
    }

    fn put_blob(&self, digest: Sha256Digest, bytes: &[u8]) -> Result<(), StoreError> {
        if self.fail_next_blob.replace(false) {
            return Err(StoreError::Unavailable {
                operation: "put_blob",
                detail: "injected fault".to_owned(),
            });
        }
        if Sha256Digest::of(bytes) != digest {
            return Err(StoreError::Corrupt(Corruption::BlobDigestMismatch {
                digest,
            }));
        }
        self.blobs.borrow_mut().insert(digest, bytes.to_vec());
        Ok(())
    }

    fn blob(&self, digest: Sha256Digest) -> Result<Vec<u8>, StoreError> {
        let blobs = self.blobs.borrow();
        let bytes = blobs
            .get(&digest)
            .ok_or(StoreError::Corrupt(Corruption::MissingBlob { digest }))?;
        if Sha256Digest::of(bytes) != digest {
            return Err(StoreError::Corrupt(Corruption::BlobDigestMismatch {
                digest,
            }));
        }
        Ok(bytes.clone())
    }

    fn append_revision(&self, revision: &ArtifactRevision) -> Result<(), StoreError> {
        if self.lose_next_revision_race.replace(false) {
            return Err(StoreError::Conflict);
        }
        let mut revisions = self.revisions.borrow_mut();
        let taken = revisions.iter().any(|existing| {
            existing.artifact() == revision.artifact() && existing.revision() == revision.revision()
        });
        if taken {
            return Err(StoreError::Conflict);
        }
        revisions.push(revision.clone());
        Ok(())
    }

    fn append_evidence(&self, record: &RecordedEvidence) -> Result<(), StoreError> {
        let mut evidence = self.evidence.borrow_mut();
        if evidence
            .iter()
            .any(|existing| existing.evidence.id == record.evidence.id)
        {
            return Err(StoreError::Conflict);
        }
        evidence.push(record.clone());
        Ok(())
    }
}

/// Predictable identities: 1, 2, 3 … per kind.
#[derive(Default)]
pub struct CountingIds {
    artifacts: RefCell<u128>,
    evidence: RefCell<u128>,
}

impl Ids for CountingIds {
    fn artifact_id(&self) -> Result<ArtifactId, IdsUnavailable> {
        *self.artifacts.borrow_mut() += 1;
        Ok(ArtifactId::from_bits(*self.artifacts.borrow()))
    }

    fn evidence_id(&self) -> Result<EvidenceId, IdsUnavailable> {
        *self.evidence.borrow_mut() += 1;
        Ok(EvidenceId::from_bits(*self.evidence.borrow()))
    }
}

/// A runtime whose "filesystem" is a map of named workspaces, sources and written bundles.
#[derive(Default)]
pub struct MemoryRuntime {
    pub ids: CountingIds,
    pub workspaces: RefCell<BTreeMap<String, std::rc::Rc<MemoryStore>>>,
    pub sources: RefCell<BTreeMap<String, Vec<u8>>>,
    pub bundles: RefCell<BTreeMap<String, Bundle>>,
}

impl MemoryRuntime {
    pub fn with_source(self, path: &str, bytes: &[u8]) -> Self {
        self.sources
            .borrow_mut()
            .insert(path.to_owned(), bytes.to_vec());
        self
    }
}

/// Shares one in-memory store between the runtime and the commands that open it.
pub struct SharedStore(pub std::rc::Rc<MemoryStore>);

impl Store for SharedStore {
    fn revisions(&self) -> Result<Vec<ArtifactRevision>, StoreError> {
        self.0.revisions()
    }
    fn evidence(&self) -> Result<Vec<RecordedEvidence>, StoreError> {
        self.0.evidence()
    }
    fn put_blob(&self, digest: Sha256Digest, bytes: &[u8]) -> Result<(), StoreError> {
        self.0.put_blob(digest, bytes)
    }
    fn blob(&self, digest: Sha256Digest) -> Result<Vec<u8>, StoreError> {
        self.0.blob(digest)
    }
    fn append_revision(&self, revision: &ArtifactRevision) -> Result<(), StoreError> {
        self.0.append_revision(revision)
    }
    fn append_evidence(&self, record: &RecordedEvidence) -> Result<(), StoreError> {
        self.0.append_evidence(record)
    }
}

impl Runtime for MemoryRuntime {
    type Store = SharedStore;
    type Ids = CountingIds;

    fn ids(&self) -> &CountingIds {
        &self.ids
    }

    fn initialize(&self, root: &Path) -> Result<InitOutcome, StoreError> {
        let key = root.display().to_string();
        let mut workspaces = self.workspaces.borrow_mut();
        if workspaces.contains_key(&key) {
            return Ok(InitOutcome::AlreadyInitialized);
        }
        workspaces.insert(key, Default::default());
        Ok(InitOutcome::Created)
    }

    fn open(&self, root: &Path) -> Result<SharedStore, StoreError> {
        self.workspaces
            .borrow()
            .get(&root.display().to_string())
            .cloned()
            .map(SharedStore)
            .ok_or(StoreError::NotInitialized)
    }

    fn read_source(&self, path: &Path) -> Result<Vec<u8>, SourceError> {
        self.sources
            .borrow()
            .get(&path.display().to_string())
            .cloned()
            .ok_or(SourceError::NotARegularFile)
    }

    fn write_bundle(
        &self,
        destination: &Path,
        bundle: &Bundle,
    ) -> Result<BundleReceipt, BundleError> {
        let key = destination.display().to_string();
        let mut bundles = self.bundles.borrow_mut();
        if bundles.contains_key(&key) {
            return Err(BundleError::DestinationExists);
        }
        let manifest = format!("{key}/bundle.json");
        let audio = format!("{key}/audio.ogg");
        bundles.insert(key, bundle.clone());
        Ok(BundleReceipt {
            manifest,
            audio,
            manifest_digest: Sha256Digest::of(b"in-memory manifest"),
        })
    }
}

/// What happens to one `append` call, chosen by the test before the call is made.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Fault {
    /// The write is refused: nothing is stored and the caller sees `Unavailable`.
    Refuse,
    /// The write succeeds but the caller sees `Unavailable`: a lost response.
    LoseResponse,
    /// Another writer stores an identical event first; the caller sees `Conflict`.
    LoseRace,
}

/// An attempt history held in memory that follows the port's rules (write-once, in order) and
/// can fail on demand, so crash points between the reservation and the outcome are testable.
#[derive(Default)]
pub struct MemoryAttempts {
    pub logs: RefCell<BTreeMap<OperationKey, Vec<Event>>>,
    /// One entry per upcoming `append`, consumed in order; `None` is a normal write. An empty
    /// queue means every further append is normal.
    pub faults: RefCell<VecDeque<Option<Fault>>>,
    /// Make `events` fail as if the store were unreachable.
    pub unreadable: RefCell<bool>,
    /// How many `append` calls were made, faulted or not.
    pub appends: RefCell<usize>,
}

impl MemoryAttempts {
    /// Schedules faults for the next appends, in order (`None` = no fault for that append).
    pub fn plan(&self, faults: impl IntoIterator<Item = Option<Fault>>) {
        self.faults.borrow_mut().extend(faults);
    }

    pub fn log(&self, key: &OperationKey) -> Vec<Event> {
        self.logs.borrow().get(key).cloned().unwrap_or_default()
    }

    fn store(
        &self,
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<(), AttemptStoreError> {
        let mut logs = self.logs.borrow_mut();
        let log = logs.entry(key.clone()).or_default();
        if log.len() != index {
            return Err(AttemptStoreError::Conflict);
        }
        log.push(event.clone());
        Ok(())
    }
}

impl AttemptStore for MemoryAttempts {
    fn events(&self, key: &OperationKey) -> Result<Vec<Event>, AttemptStoreError> {
        if *self.unreadable.borrow() {
            return Err(AttemptStoreError::Unavailable {
                operation: "events",
                detail: "injected fault".to_owned(),
            });
        }
        Ok(self.log(key))
    }

    fn append(
        &self,
        key: &OperationKey,
        index: usize,
        event: &Event,
    ) -> Result<(), AttemptStoreError> {
        *self.appends.borrow_mut() += 1;
        let unavailable = || AttemptStoreError::Unavailable {
            operation: "append",
            detail: "injected fault".to_owned(),
        };
        match self.faults.borrow_mut().pop_front().flatten() {
            None => self.store(key, index, event),
            Some(Fault::Refuse) => Err(unavailable()),
            Some(Fault::LoseResponse) => {
                self.store(key, index, event)?;
                Err(unavailable())
            }
            Some(Fault::LoseRace) => {
                self.store(key, index, event)?;
                Err(AttemptStoreError::Conflict)
            }
        }
    }
}
