//! Read-only views of a workspace: what exists, and what is recorded about one artifact.

use crate::domain::{
    artifact::{ArtifactKind, ArtifactRevision, RevisionNumber},
    digest::Sha256Digest,
    evaluation::{self, EvaluationSummary},
    identity::ArtifactId,
    lineage::{self, Freshness, HistoryError, PinViolation, RevisionHistory},
};

use super::ports::{RecordedEvidence, Store, StoreError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ArtifactSummary {
    pub artifact: ArtifactId,
    pub kind: ArtifactKind,
    pub head: RevisionNumber,
    pub digest: Sha256Digest,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ViewError {
    UnknownArtifact(ArtifactId),
    History(HistoryError),
    Store(StoreError),
}

impl From<StoreError> for ViewError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// One summary per artifact, in artifact-identity order.
pub fn list_artifacts(store: &impl Store) -> Result<Vec<ArtifactSummary>, ViewError> {
    let mut summaries: Vec<ArtifactSummary> = Vec::new();
    for revision in store.revisions()? {
        let summary = ArtifactSummary {
            artifact: revision.artifact(),
            kind: revision.facts().payload.kind(),
            head: revision.revision(),
            digest: revision.facts().content,
        };
        match summaries.last_mut() {
            Some(last) if last.artifact == summary.artifact => *last = summary,
            _ => summaries.push(summary),
        }
    }
    summaries.sort_by_key(|summary| summary.artifact);
    Ok(summaries)
}

/// Everything recorded about one artifact, with currency worked out from history.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ArtifactDetail {
    pub revisions: Vec<ArtifactRevision>,
    /// Whether the head's pinned inputs are still the current revisions of those inputs.
    pub inputs: Result<Freshness, Vec<PinViolation>>,
    /// The three evaluation layers for the head revision, independent of one another.
    pub evaluation: Result<EvaluationSummary, Vec<PinViolation>>,
    /// Evidence about any revision of this artifact, oldest first.
    pub evidence: Vec<RecordedEvidence>,
}

pub fn show_artifact(
    store: &impl Store,
    artifact: ArtifactId,
) -> Result<ArtifactDetail, ViewError> {
    let all = store.revisions()?;
    let history = RevisionHistory::from_revisions(&all).map_err(ViewError::History)?;
    let revisions: Vec<ArtifactRevision> = all
        .into_iter()
        .filter(|revision| revision.artifact() == artifact)
        .collect();
    let head = revisions
        .last()
        .ok_or(ViewError::UnknownArtifact(artifact))?;
    let evidence_all = store.evidence()?;
    let evidence: Vec<RecordedEvidence> = evidence_all
        .iter()
        .filter(|record| record.evidence.subject.artifact == artifact)
        .cloned()
        .collect();
    let records: Vec<_> = evidence_all
        .into_iter()
        .map(|record| record.evidence)
        .collect();
    Ok(ArtifactDetail {
        inputs: lineage::assess(&head.facts().inputs, &history),
        evaluation: evaluation::summarize(head.pin(), &records, &history),
        revisions,
        evidence,
    })
}
