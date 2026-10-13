//! Preparing a local handoff bundle for the head revision of an audio artifact.

use crate::domain::{
    audio_profile::AudioProfile,
    checks::{check_audio, Check, CheckResult},
    evaluation::{CheckOutcome, Evidence},
    handoff::{plan_export, ExportRefusal},
    identity::ArtifactId,
    label::{CheckName, LabelError},
    lineage::{HistoryError, RevisionHistory},
};

use super::{
    ports::{Bundle, Store, StoreError},
    validate::head_of,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ExportRequest {
    pub artifact: ArtifactId,
    pub profile: AudioProfile,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ExportError {
    UnknownArtifact(ArtifactId),
    /// Every reason the revision may not be exported yet, together.
    Refused(Vec<ExportRefusal>),
    /// The bytes passed when they were checked but fail the same check now.
    RemeasureFailed(CheckResult),
    CheckName(LabelError),
    History(HistoryError),
    Store(StoreError),
}

impl From<StoreError> for ExportError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

/// Decides, reads the exact bytes, measures them again and returns the bundle to write.
///
/// Nothing is written or sent here. The caller writes the bundle locally; submission is a
/// separate, later step that does not exist yet.
pub fn prepare_export(store: &impl Store, request: ExportRequest) -> Result<Bundle, ExportError> {
    let head =
        head_of(store, request.artifact)?.ok_or(ExportError::UnknownArtifact(request.artifact))?;
    let revisions = store.revisions()?;
    let history = RevisionHistory::from_revisions(&revisions).map_err(ExportError::History)?;
    let evidence: Vec<Evidence> = store
        .evidence()?
        .into_iter()
        .map(|record| record.evidence)
        .collect();
    let gate = CheckName::parse(&Check::OggOpusProfile(request.profile).label())
        .map_err(ExportError::CheckName)?;
    let plan = plan_export(&head, &history, &evidence, &gate, request.profile)
        .map_err(ExportError::Refused)?;
    let audio = store.blob(head.facts().content)?;
    let measured = check_audio(&audio, request.profile);
    if measured.outcome != CheckOutcome::Passed {
        return Err(ExportError::RemeasureFailed(measured));
    }
    Ok(Bundle {
        plan,
        audio,
        measured,
    })
}
