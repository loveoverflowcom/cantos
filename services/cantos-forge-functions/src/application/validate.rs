//! Running a deterministic check on the head revision and recording its evidence.

use crate::domain::{
    artifact::{ArtifactKind, ArtifactPayload, ArtifactRevision, InputPin},
    audio_profile::AudioProfile,
    checks::{check_audio, check_text, CheckResult},
    evaluation::{Evidence, Finding},
    identity::{ArtifactId, EvidenceId},
    label::{CheckName, LabelError},
};

use super::ports::{Ids, IdsUnavailable, RecordedEvidence, Store, StoreError};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ValidateRequest {
    pub artifact: ArtifactId,
    /// Audio needs a profile named by the producer; text takes none.
    pub profile: Option<AudioProfile>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Validated {
    pub subject: InputPin,
    pub evidence: EvidenceId,
    pub result: CheckResult,
    pub write: Write,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Write {
    Recorded,
    /// A record with the same subject, check and measurements already exists.
    AlreadyRecorded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ValidateError {
    UnknownArtifact(ArtifactId),
    /// Audio is only measured against a profile the producer chose; there is no default.
    ProfileRequired,
    ProfileNotApplicable {
        kind: ArtifactKind,
    },
    /// Forge has no deterministic check for this kind: Script IR belongs to the shared
    /// validator, and reports and instruction bundles have no byte-level rule yet.
    NoCheckFor {
        kind: ArtifactKind,
    },
    CheckName(LabelError),
    Ids(IdsUnavailable),
    Store(StoreError),
}

impl From<StoreError> for ValidateError {
    fn from(error: StoreError) -> Self {
        Self::Store(error)
    }
}

pub fn validate_artifact(
    store: &impl Store,
    ids: &impl Ids,
    request: ValidateRequest,
) -> Result<Validated, ValidateError> {
    let head = head_of(store, request.artifact)?
        .ok_or(ValidateError::UnknownArtifact(request.artifact))?;
    let kind = head.facts().payload.kind();
    let subject = head.pin();
    let run = match (&head.facts().payload, request.profile) {
        (ArtifactPayload::Audio, None) => return Err(ValidateError::ProfileRequired),
        (ArtifactPayload::Audio, Some(profile)) => Run::Audio(profile),
        (ArtifactPayload::Manuscript { .. }, None) => Run::Text,
        (ArtifactPayload::Manuscript { .. }, Some(_)) => {
            return Err(ValidateError::ProfileNotApplicable { kind })
        }
        _ => return Err(ValidateError::NoCheckFor { kind }),
    };
    let bytes = store.blob(subject.digest)?;
    let result = match run {
        Run::Audio(profile) => check_audio(&bytes, profile),
        Run::Text => check_text(&bytes),
    };
    let name = CheckName::parse(&result.check.label()).map_err(ValidateError::CheckName)?;
    if let Some(existing) = identical_record(store, subject, &name, &result)? {
        return Ok(Validated {
            subject,
            evidence: existing,
            result,
            write: Write::AlreadyRecorded,
        });
    }
    let id = ids.evidence_id().map_err(ValidateError::Ids)?;
    store.append_evidence(&RecordedEvidence {
        evidence: Evidence {
            id,
            subject,
            basis: Default::default(),
            finding: Finding::Deterministic {
                check: name,
                outcome: result.outcome,
            },
        },
        detail: result.detail.clone(),
    })?;
    Ok(Validated {
        subject,
        evidence: id,
        result,
        write: Write::Recorded,
    })
}

enum Run {
    Audio(AudioProfile),
    Text,
}

/// The newest recorded revision of `artifact`, if it exists.
pub(super) fn head_of(
    store: &impl Store,
    artifact: ArtifactId,
) -> Result<Option<ArtifactRevision>, StoreError> {
    Ok(store
        .revisions()?
        .into_iter()
        .rev()
        .find(|revision| revision.artifact() == artifact))
}

/// The check is deterministic, so the same subject, check and measurements need no second record.
fn identical_record(
    store: &impl Store,
    subject: InputPin,
    name: &CheckName,
    result: &CheckResult,
) -> Result<Option<EvidenceId>, StoreError> {
    Ok(store.evidence()?.into_iter().find_map(|record| {
        let same_check = matches!(
            &record.evidence.finding,
            Finding::Deterministic { check, outcome }
                if check == name && *outcome == result.outcome
        );
        (record.evidence.subject == subject && same_check && record.detail == result.detail)
            .then_some(record.evidence.id)
    }))
}
