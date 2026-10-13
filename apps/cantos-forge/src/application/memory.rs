//! In-memory fakes of the ports, for deterministic use-case and CLI tests.

use std::{cell::RefCell, collections::BTreeMap, path::Path};

use crate::domain::{
    artifact::ArtifactRevision,
    digest::Sha256Digest,
    identity::{ArtifactId, EvidenceId},
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
}

impl Store for MemoryStore {
    fn revisions(&self) -> Result<Vec<ArtifactRevision>, StoreError> {
        Ok(self.revisions.borrow().clone())
    }

    fn evidence(&self) -> Result<Vec<RecordedEvidence>, StoreError> {
        Ok(self.evidence.borrow().clone())
    }

    fn put_blob(&self, digest: Sha256Digest, bytes: &[u8]) -> Result<(), StoreError> {
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
