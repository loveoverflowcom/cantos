//! Registering bytes as a revision of an artifact.

use crate::domain::{
    artifact::{
        decide_registration, ArtifactPayload, ArtifactRevision, InputPin, InputPins,
        InputPinsError, Provenance, Registration, RevisionError, RevisionFacts, RevisionNumber,
        RightsClaim,
    },
    digest::Sha256Digest,
    identity::ArtifactId,
    label::ProducerLabel,
};

use super::ports::{Ids, IdsUnavailable, Store, StoreError};

/// Whether a registration starts a new artifact or adds a revision to an existing one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    /// Forge chooses a fresh identity; the caller can never choose one.
    New,
    Existing(ArtifactId),
}

/// An input named by the operator. Its digest is looked up in recorded history, never typed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputRef {
    pub artifact: ArtifactId,
    pub revision: RevisionNumber,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RegisterRequest {
    pub target: Target,
    pub payload: ArtifactPayload,
    pub producer: ProducerLabel,
    pub rights: RightsClaim,
    pub inputs: Vec<InputRef>,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Registered {
    Created(Placement),
    /// The same facts are already the head revision; nothing was written.
    AlreadyCurrent(Placement),
}

/// Where a registration landed.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placement {
    pub artifact: ArtifactId,
    pub revision: RevisionNumber,
    pub digest: Sha256Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RegisterError {
    UnknownArtifact(ArtifactId),
    UnknownInput(InputRef),
    RepeatedInput(InputPinsError),
    Refused(RevisionError),
    Ids(IdsUnavailable),
    Store(StoreError),
}

impl From<StoreError> for RegisterError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

pub fn register_artifact(
    store: &impl Store,
    ids: &impl Ids,
    request: RegisterRequest,
) -> Result<Registered, RegisterError> {
    let revisions = store.revisions()?;
    let (artifact, head) = match request.target {
        Target::New => (ids.artifact_id().map_err(RegisterError::Ids)?, None),
        Target::Existing(artifact) => {
            let head = revisions
                .iter()
                .rev()
                .find(|revision| revision.artifact() == artifact)
                .ok_or(RegisterError::UnknownArtifact(artifact))?;
            (artifact, Some(head))
        }
    };
    let pins = resolve_inputs(&revisions, &request.inputs)?;
    let facts = RevisionFacts {
        content: Sha256Digest::of(&request.bytes),
        payload: request.payload,
        provenance: Provenance::HostAssisted {
            producer: request.producer,
        },
        rights: request.rights,
        inputs: pins,
    };
    let digest = facts.content;
    match decide_registration(artifact, head, facts).map_err(RegisterError::Refused)? {
        Registration::AlreadyCurrent(revision) => Ok(Registered::AlreadyCurrent(Placement {
            artifact,
            revision,
            digest,
        })),
        Registration::Created(revision) => {
            // Bytes first: a blob without a revision is harmless, a revision without bytes is not.
            store.put_blob(digest, &request.bytes)?;
            store.append_revision(&revision)?;
            Ok(Registered::Created(Placement {
                artifact,
                revision: revision.revision(),
                digest,
            }))
        }
    }
}

fn resolve_inputs(
    revisions: &[ArtifactRevision],
    inputs: &[InputRef],
) -> Result<InputPins, RegisterError> {
    let pins = inputs
        .iter()
        .map(|input| {
            revisions
                .iter()
                .find(|revision| {
                    revision.artifact() == input.artifact && revision.revision() == input.revision
                })
                .map(ArtifactRevision::pin)
                .ok_or(RegisterError::UnknownInput(*input))
        })
        .collect::<Result<Vec<InputPin>, _>>()?;
    InputPins::new(pins).map_err(RegisterError::RepeatedInput)
}
