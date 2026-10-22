//! Concrete route plans reuse the server's raw DTOs. No domain policy is decided here.
use super::{artwork, delegation::IssuedGrant};
use cantos_contracts::*;
use serde::de::DeserializeOwned;
use std::sync::Arc;

pub const MAX_AUDIO_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credentials {
    /// The Studio owner's HttpOnly cookie session: `credentials: include` and the session fence,
    /// never an `Authorization` header.
    OwnerSession,
    /// No ambient credential. The producer capability Bearer, where a route takes one, is
    /// attached explicitly from its own `UploadCredential`.
    Omit,
}
impl Credentials {
    pub fn includes_cookie(self) -> bool {
        self == Self::OwnerSession
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    InvalidInput,
    Cancelled,
    SessionChanged,
    ReceiptNotEstablished,
    ReplyMismatch,
    Transport,
    Protocol { status: u16 },
    Http { status: u16, error: ApiError },
}
impl ClientError {
    /// Producer capability expiry says nothing about the Studio owner's cookie session.
    pub fn owner_session_denial(&self, command: &Command) -> Option<&ApiError> {
        match self {
            Self::Http {
                status: 401 | 409,
                error,
            } if command.credentials() == Credentials::OwnerSession
                && error.code == ErrorCode::Unauthenticated =>
            {
                Some(error)
            }
            _ => None,
        }
    }
    /// A 409 `unauthenticated` names a session fence the cookie no longer holds.
    pub fn is_session_fence_mismatch(&self) -> bool {
        matches!(self, Self::Http { status, error } if crate::session::is_fence_mismatch(*status, error))
    }
    pub fn http(status: u16, body: &[u8]) -> Self {
        match serde_json::from_slice(body) {
            Ok(error) => Self::Http { status, error },
            Err(_) => Self::Protocol { status },
        }
    }
    pub fn ambiguous(&self) -> bool {
        match self {
            Self::Cancelled
            | Self::SessionChanged
            | Self::ReceiptNotEstablished
            | Self::ReplyMismatch
            | Self::Transport
            | Self::Protocol { .. } => true,
            Self::Http { status, .. } => *status >= 500,
            Self::InvalidInput => false,
        }
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct UploadCredential(String);
impl std::fmt::Debug for UploadCredential {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("UploadCredential([redacted])")
    }
}
impl UploadCredential {
    pub fn new(value: String) -> Result<Self, ClientError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(ClientError::InvalidInput);
        }
        Ok(Self(value))
    }
    #[cfg(target_arch = "wasm32")]
    pub(super) fn bearer(&self) -> String {
        format!("Bearer {}", self.0)
    }
}
pub fn admit_audio_size(size: u64) -> Result<(), ClientError> {
    if (1..=MAX_AUDIO_BYTES).contains(&size) {
        Ok(())
    } else {
        Err(ClientError::InvalidInput)
    }
}
pub fn valid_profile(profile: &str) -> bool {
    matches!(
        profile,
        "opus-mono-20-v1" | "opus-mono-24-v1" | "opus-mono-32-v1" | "opus-mono-40-v1"
    )
}
fn id(value: &str) -> Result<&str, ClientError> {
    if crate::adaptation::is_run_id(value) {
        Ok(value)
    } else {
        Err(ClientError::InvalidInput)
    }
}
fn cursor_query(cursor: &Option<String>) -> Result<String, ClientError> {
    match cursor {
        None => Ok("?limit=20".into()),
        Some(value)
            if !value.is_empty()
                && value.len() <= 2048
                && value.bytes().all(|b| b.is_ascii_hexdigit()) =>
        {
            Ok(format!("?limit=20&cursor={value}"))
        }
        Some(_) => Err(ClientError::InvalidInput),
    }
}
/// Secrets intentionally have no Debug/Serialize representation at this shell boundary.
#[derive(Clone)]
pub enum Command {
    Works {
        cursor: Option<String>,
    },
    Work {
        id: String,
    },
    CreateWork(CreateWorkRequest),
    PatchWork {
        work: String,
        request: PatchWorkRequest,
    },
    ArchiveWork {
        work: String,
        request: ArchiveWorkRequest,
    },
    Units {
        work: String,
        cursor: Option<String>,
    },
    CreateUnit {
        work: String,
        request: CreateContentUnitRequest,
    },
    PatchUnit {
        unit: String,
        request: PatchContentUnitRequest,
    },
    ArchiveUnit {
        unit: String,
        request: ArchiveContentUnitRequest,
    },
    Reorder {
        work: String,
        request: ReorderContentUnitsRequest,
    },
    ContentHead {
        unit: String,
    },
    ContentHistory {
        unit: String,
        after: u64,
    },
    ContentRevision {
        unit: String,
        revision: String,
    },
    AppendContent {
        unit: String,
        request: AppendContentRequest,
    },
    Brief {
        unit: String,
        version: u64,
    },
    CreateBrief {
        unit: String,
        request: CreateProductionBriefRequest,
    },
    Submission {
        unit: String,
        submission: String,
    },
    Prepare {
        unit: String,
        request: CreateAudioSubmissionRequest,
    },
    Revoke {
        unit: String,
        submission: String,
    },
    Review {
        unit: String,
        submission: String,
        request: CreateAudioReviewRequest,
    },
    Candidate {
        unit: String,
        submission: String,
        request: CreateReleaseCandidateRequest,
    },
    CandidateRead {
        unit: String,
        submission: String,
        candidate: String,
    },
    ProducerStatus {
        submission: String,
        credential: UploadCredential,
    },
    Upload {
        binding: AudioBinding,
        sha256: String,
        submission: String,
        credential: UploadCredential,
        bytes: Arc<Vec<u8>>,
    },
    Artwork {
        work: String,
    },
    PrepareImage {
        work: String,
        request: CreateImageIntentRequest,
    },
    ImageIntent {
        work: String,
        intent: String,
    },
    UploadImage {
        work: String,
        intent: ImageIntentDocument,
        bytes: Arc<Vec<u8>>,
    },
    SetArtwork {
        work: String,
        request: SetWorkArtworkRequest,
    },
    /// Owner-only Work delegation (ADR 0028), on the Studio owner cookie session.
    Delegations {
        work: String,
        after: Option<String>,
    },
    IssueDelegation {
        work: String,
        request: IssueDelegationRequest,
    },
    RevokeDelegation {
        work: String,
        grant: String,
    },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Reply {
    Works(CatalogPage<CatalogWork>),
    Work(CatalogWork),
    Units(CatalogPage<CatalogContentUnit>),
    Mutation(CatalogMutationResponse),
    Content(ContentRevisionResponse),
    ContentSaved(AppendContentResponse),
    History(ContentHistoryResponse),
    Brief(Box<ProductionBriefResponse>),
    Submission(Box<AudioSubmissionResponse>),
    Rendition(Box<AudioRenditionDocument>),
    Review(AudioReviewDocument),
    Candidate(ReleaseCandidateResponse),
    Revoked,
    Artwork(WorkArtworkResponse),
    ImagePrepared(ImageIntentDocument),
    ImageIntent(ImageIntentResponse),
    ImageAdmitted(ImageAssetDocument),
    Delegations(DelegationGrantPage),
    /// The show-once secret is redacted from Debug; see `IssuedGrant`.
    DelegationIssued(Box<IssuedGrant>),
}
impl Command {
    pub fn path(&self) -> Result<String, ClientError> {
        let base = routes::STUDIO_BASE;
        let unit_path = |unit: &str| -> Result<String, ClientError> {
            Ok(format!("{base}/content-units/{}", id(unit)?))
        };
        let submission_path = |unit: &str, submission: &str| -> Result<String, ClientError> {
            Ok(format!(
                "{}/audio-submissions/{}",
                unit_path(unit)?,
                id(submission)?
            ))
        };
        Ok(match self {
            Self::Works { cursor } => format!("{}{}", routes::STUDIO_WORKS, cursor_query(cursor)?),
            Self::CreateWork(_) => routes::STUDIO_WORKS.into(),
            Self::Work { id: work }
            | Self::PatchWork { work, .. }
            | Self::ArchiveWork { work, .. } => format!("{base}/works/{}", id(work)?),
            Self::Units { work, cursor } => format!(
                "{base}/works/{}/content-units{}",
                id(work)?,
                cursor_query(cursor)?
            ),
            Self::CreateUnit { work, .. } => format!("{base}/works/{}/content-units", id(work)?),
            Self::Reorder { work, .. } => {
                format!("{base}/works/{}/content-units/reorder", id(work)?)
            }
            Self::PatchUnit { unit, .. } | Self::ArchiveUnit { unit, .. } => unit_path(unit)?,
            Self::ContentHead { unit } => format!("{}/content/head", unit_path(unit)?),
            Self::ContentRevision { unit, revision } => {
                format!("{}/content/revisions/{}", unit_path(unit)?, id(revision)?)
            }
            Self::ContentHistory { unit, after } => {
                if *after > i64::MAX as u64 {
                    return Err(ClientError::InvalidInput);
                };
                format!(
                    "{}/content/history?limit=20&after_revision={after}",
                    unit_path(unit)?
                )
            }
            Self::AppendContent { unit, .. } => format!("{}/content/revisions", unit_path(unit)?),
            Self::CreateBrief { unit, .. } => format!("{}/production-briefs", unit_path(unit)?),
            Self::Brief { unit, version } => {
                if *version == 0 || *version > i64::MAX as u64 {
                    return Err(ClientError::InvalidInput);
                };
                format!("{}/production-briefs/{version}", unit_path(unit)?)
            }
            Self::Prepare { unit, .. } => format!("{}/audio-submissions", unit_path(unit)?),
            Self::Submission { unit, submission } => submission_path(unit, submission)?,
            Self::Revoke { unit, submission } => {
                format!("{}/upload-permission", submission_path(unit, submission)?)
            }
            Self::Review {
                unit, submission, ..
            } => format!("{}/reviews", submission_path(unit, submission)?),
            Self::Candidate {
                unit, submission, ..
            } => format!("{}/release-candidates", submission_path(unit, submission)?),
            Self::CandidateRead {
                unit,
                submission,
                candidate,
            } => format!(
                "{}/release-candidates/{}",
                submission_path(unit, submission)?,
                id(candidate)?
            ),
            Self::ProducerStatus { submission, .. } => {
                format!("{base}/producer/audio-submissions/{}", id(submission)?)
            }
            Self::Upload {
                submission, bytes, ..
            } => {
                admit_audio_size(bytes.len() as u64)?;
                format!(
                    "{base}/producer/audio-submissions/{}/bytes",
                    id(submission)?
                )
            }
            Self::Artwork { work } | Self::SetArtwork { work, .. } => {
                format!("{base}/works/{}/artwork", id(work)?)
            }
            Self::PrepareImage { work, .. } => format!("{base}/works/{}/image-intents", id(work)?),
            Self::ImageIntent { work, intent } => {
                format!("{base}/works/{}/image-intents/{}", id(work)?, id(intent)?)
            }
            Self::UploadImage {
                work,
                intent,
                bytes,
            } => {
                if bytes.is_empty()
                    || bytes.len() > artwork::MAX_BYTES as usize
                    || intent.work_id != *work
                {
                    return Err(ClientError::InvalidInput);
                }
                format!(
                    "{base}/works/{}/image-intents/{}/bytes",
                    id(work)?,
                    id(&intent.id)?
                )
            }
            Self::Delegations { work, after } => {
                let after = match after {
                    None => String::new(),
                    Some(grant) => format!("&after={}", id(grant)?),
                };
                format!(
                    "{}?limit={}{after}",
                    routes::STUDIO_WORK_DELEGATIONS.replace("{work_id}", id(work)?),
                    super::delegation::PAGE_LIMIT
                )
            }
            Self::IssueDelegation { work, .. } => {
                routes::STUDIO_WORK_DELEGATIONS.replace("{work_id}", id(work)?)
            }
            Self::RevokeDelegation { work, grant } => routes::STUDIO_WORK_DELEGATION
                .replace("{work_id}", id(work)?)
                .replace("{grant_id}", id(grant)?),
        })
    }
    pub fn method(&self) -> &'static str {
        match self {
            Self::PatchWork { .. } | Self::PatchUnit { .. } | Self::SetArtwork { .. } => "PATCH",
            Self::ArchiveWork { .. }
            | Self::ArchiveUnit { .. }
            | Self::Revoke { .. }
            | Self::RevokeDelegation { .. } => "DELETE",
            Self::Upload { .. } | Self::UploadImage { .. } => "PUT",
            Self::CreateWork(_)
            | Self::CreateUnit { .. }
            | Self::Reorder { .. }
            | Self::AppendContent { .. }
            | Self::CreateBrief { .. }
            | Self::Prepare { .. }
            | Self::Review { .. }
            | Self::Candidate { .. }
            | Self::IssueDelegation { .. }
            | Self::PrepareImage { .. } => "POST",
            _ => "GET",
        }
    }
    /// A successful revocation is exactly `204 No Content`, without a JSON body.
    pub fn expects_no_content(&self) -> bool {
        matches!(self, Self::Revoke { .. } | Self::RevokeDelegation { .. })
    }
    pub fn is_write(&self) -> bool {
        self.method() != "GET"
    }
    pub fn credentials(&self) -> Credentials {
        match self {
            Self::ProducerStatus { .. } | Self::Upload { .. } => Credentials::Omit,
            _ => Credentials::OwnerSession,
        }
    }
    /// The producer capability a route carries as `Authorization: Bearer`. It is the only
    /// Authorization any Studio command sends, and never travels with the owner cookie.
    pub fn producer_credential(&self) -> Option<&UploadCredential> {
        match self {
            Self::ProducerStatus { credential, .. } | Self::Upload { credential, .. } => {
                Some(credential)
            }
            _ => None,
        }
    }
    pub fn json(&self) -> Result<Option<String>, ClientError> {
        fn encode<T: serde::Serialize>(v: &T) -> Result<Option<String>, ClientError> {
            serde_json::to_string(v)
                .map(Some)
                .map_err(|_| ClientError::InvalidInput)
        }
        match self {
            Self::CreateWork(v) => encode(v),
            Self::PatchWork { request, .. } => encode(request),
            Self::ArchiveWork { request, .. } => encode(request),
            Self::CreateUnit { request, .. } => encode(request),
            Self::PatchUnit { request, .. } => encode(request),
            Self::ArchiveUnit { request, .. } => encode(request),
            Self::Reorder { request, .. } => encode(request),
            Self::AppendContent { request, .. } => encode(request),
            Self::CreateBrief { request, .. } => encode(request),
            Self::Prepare { request, .. } => encode(request),
            Self::Review { request, .. } => encode(request),
            Self::Candidate { request, .. } => encode(request),
            Self::IssueDelegation { request, .. } => encode(request),
            Self::PrepareImage { request, .. } => encode(request),
            Self::SetArtwork { request, .. } => encode(request),
            _ => Ok(None),
        }
    }
    pub fn decode(&self, status: u16, body: &[u8]) -> Result<Reply, ClientError> {
        let artwork_status = match self {
            Self::PrepareImage { .. } => Some(201),
            Self::Artwork { .. }
            | Self::SetArtwork { .. }
            | Self::ImageIntent { .. }
            | Self::UploadImage { .. } => Some(200),
            _ => None,
        };
        if (200..300).contains(&status) && artwork_status.is_some_and(|expected| expected != status)
        {
            return Err(ClientError::Protocol { status });
        }
        if self.expects_no_content() && (200..300).contains(&status) {
            return if status == 204 {
                Ok(Reply::Revoked)
            } else {
                Err(ClientError::Protocol { status })
            };
        }
        if !(200..300).contains(&status) {
            return Err(ClientError::http(status, body));
        }
        fn parse<T: DeserializeOwned>(body: &[u8], status: u16) -> Result<T, ClientError> {
            serde_json::from_slice(body).map_err(|_| ClientError::Protocol { status })
        }
        let reply = match self {
            Self::Works { .. } => Reply::Works(parse(body, status)?),
            Self::Work { .. } => Reply::Work(parse(body, status)?),
            Self::Units { .. } => Reply::Units(parse(body, status)?),
            Self::CreateWork(_)
            | Self::PatchWork { .. }
            | Self::ArchiveWork { .. }
            | Self::CreateUnit { .. }
            | Self::PatchUnit { .. }
            | Self::ArchiveUnit { .. }
            | Self::Reorder { .. } => Reply::Mutation(parse(body, status)?),
            Self::ContentHead { .. } | Self::ContentRevision { .. } => {
                Reply::Content(parse(body, status)?)
            }
            Self::AppendContent { .. } => Reply::ContentSaved(parse(body, status)?),
            Self::ContentHistory { .. } => Reply::History(parse(body, status)?),
            Self::Brief { .. } | Self::CreateBrief { .. } => {
                Reply::Brief(Box::new(parse(body, status)?))
            }
            Self::Submission { .. } | Self::Prepare { .. } | Self::ProducerStatus { .. } => {
                Reply::Submission(Box::new(parse(body, status)?))
            }
            Self::Upload { .. } => Reply::Rendition(Box::new(parse(body, status)?)),
            Self::Review { .. } => Reply::Review(parse(body, status)?),
            Self::Candidate { .. } | Self::CandidateRead { .. } => {
                Reply::Candidate(parse(body, status)?)
            }
            Self::Revoke { .. } | Self::RevokeDelegation { .. } => Reply::Revoked,
            Self::Artwork { .. } | Self::SetArtwork { .. } => Reply::Artwork(parse(body, status)?),
            Self::PrepareImage { .. } => Reply::ImagePrepared(parse(body, status)?),
            Self::ImageIntent { .. } => Reply::ImageIntent(parse(body, status)?),
            Self::UploadImage { .. } => Reply::ImageAdmitted(parse(body, status)?),
            Self::Delegations { .. } => Reply::Delegations(parse(body, status)?),
            Self::IssueDelegation { .. } => Reply::DelegationIssued(Box::new(
                IssuedGrant::admit(status, parse(body, status)?)
                    .ok_or(ClientError::Protocol { status })?,
            )),
        };
        self.check_reply(&reply)
            .map_err(|_| ClientError::Protocol { status })?;
        Ok(reply)
    }

    /// Correlate a raw DTO to the question asked, without deciding access or admission.
    pub fn check_reply(&self, reply: &Reply) -> Result<(), ClientError> {
        let matches = match self {
            Self::Works { .. } => matches!(reply, Reply::Works(_)),
            Self::Work { id } => matches!(reply, Reply::Work(work) if work.id == *id),
            Self::Units { work, .. } => matches!(reply, Reply::Units(page)
                if page.items.iter().all(|unit| unit.work_id == *work)),
            Self::CreateWork(request) => matches!(reply, Reply::Mutation(result)
                if result.work.id == request.id && consistent_units(result)),
            Self::PatchWork { work, .. }
            | Self::ArchiveWork { work, .. }
            | Self::Reorder { work, .. } => matches!(reply, Reply::Mutation(result)
                if result.work.id == *work && consistent_units(result)),
            Self::CreateUnit { work, request } => matches!(reply, Reply::Mutation(result)
                if result.work.id == *work && consistent_units(result)
                    && result.units.iter().any(|unit| unit.id == request.id)),
            Self::PatchUnit { unit, .. } | Self::ArchiveUnit { unit, .. } => {
                matches!(reply, Reply::Mutation(result)
                    if consistent_units(result)
                        && result.units.iter().any(|saved| saved.id == *unit))
            }
            Self::ContentHead { unit } => matches!(reply, Reply::Content(content)
                if content.unit_id == *unit),
            Self::ContentRevision { unit, revision } => matches!(reply, Reply::Content(content)
                if content.unit_id == *unit && content.id == *revision),
            Self::AppendContent { unit, request } => matches!(reply, Reply::ContentSaved(saved)
                if saved.content.unit_id == *unit && saved.content.id == request.id),
            // The existing history contract does not echo a unit identity.
            Self::ContentHistory { .. } => matches!(reply, Reply::History(_)),
            Self::Brief { unit, version } => matches!(reply, Reply::Brief(result)
                if result.brief.content.unit_id == *unit && result.brief.version == *version),
            Self::CreateBrief { unit, request } => matches!(reply, Reply::Brief(result)
                if result.brief.content.unit_id == *unit && result.brief.id == request.id
                    && result.brief.content.id == request.content_revision_id
                    && result.brief.content.revision_digest == request.content_revision_digest),
            Self::Submission { unit, submission } => matches!(reply, Reply::Submission(result)
                if result.submission.id == *submission
                    && result.submission.binding.unit_id == *unit && consistent_rendition(result)),
            Self::Prepare { unit, request } => matches!(reply, Reply::Submission(result)
                if prepared_submission_matches(unit, request, result) && consistent_rendition(result)),
            Self::ProducerStatus { submission, .. } => matches!(reply, Reply::Submission(result)
                if result.submission.id == *submission && consistent_rendition(result)),
            Self::Upload {
                binding,
                submission,
                sha256,
                bytes,
                ..
            } => {
                matches!(reply, Reply::Rendition(result)
                    if result.submission_id == *submission && result.binding == *binding
                        && result.verified_media.sha256 == *sha256
                        && result.verified_media.byte_length == bytes.len() as u64)
            }
            Self::Review { request, .. } => matches!(reply, Reply::Review(result)
                if result.operation_id == request.operation_id && result.request == *request),
            Self::Candidate { unit, request, .. } => matches!(reply, Reply::Candidate(result)
                if result.candidate.binding.unit_id == *unit
                    && result.candidate.operation_id == request.operation_id
                    && result.candidate.rendition_digest == request.rendition_digest),
            // Candidate documents do not echo the submission identity.
            Self::CandidateRead {
                unit, candidate, ..
            } => matches!(reply, Reply::Candidate(result)
                if result.candidate.binding.unit_id == *unit && result.candidate.id == *candidate),
            Self::Revoke { .. } | Self::RevokeDelegation { .. } => matches!(reply, Reply::Revoked),
            Self::Artwork { work } => {
                matches!(reply, Reply::Artwork(r) if r.work_id == *work && r.work_revision > 0)
            }
            Self::SetArtwork { work, request } => matches!(reply, Reply::Artwork(r)
                if r.work_id == *work && Some(r.work_revision) == request.expected_work_revision.checked_add(1)
                    && r.artwork == request.artwork),
            Self::PrepareImage { work, request } => matches!(reply, Reply::ImagePrepared(r)
                if artwork::prepared_matches(work, request, r)),
            Self::ImageIntent { work, intent } => matches!(reply, Reply::ImageIntent(r)
                if r.intent.work_id == *work && r.intent.id == *intent && artwork::consistent_intent(r)),
            Self::UploadImage {
                work,
                intent,
                bytes,
            } => matches!(reply, Reply::ImageAdmitted(r)
                if intent.work_id == *work && intent.request.expected_byte_length == bytes.len() as u64
                    && artwork::asset_matches(intent, r)),
            Self::Delegations { work, .. } => matches!(reply, Reply::Delegations(page)
                if page.items.iter().all(|grant| grant.work_id == *work)
                    && page.next_after.as_deref().is_none_or(crate::adaptation::is_run_id)),
            Self::IssueDelegation { work, request } => {
                matches!(reply, Reply::DelegationIssued(issued)
                if issued.answers(work, request))
            }
        };
        if matches {
            Ok(())
        } else {
            Err(ClientError::ReplyMismatch)
        }
    }
}

fn consistent_units(result: &CatalogMutationResponse) -> bool {
    result
        .units
        .iter()
        .all(|unit| unit.work_id == result.work.id)
}

fn consistent_rendition(result: &AudioSubmissionResponse) -> bool {
    match &result.rendition {
        Some(rendition) => {
            rendition.submission_id == result.submission.id
                && rendition.binding == result.submission.binding
                && rendition.verified_media.sha256 == result.submission.expected_sha256
                && rendition.verified_media.byte_length == result.submission.expected_byte_length
                && result
                    .reviews
                    .iter()
                    .all(|review| review.rendition_id == rendition.id)
        }
        None => result.reviews.is_empty(),
    }
}

fn prepared_submission_matches(
    unit: &str,
    request: &CreateAudioSubmissionRequest,
    result: &AudioSubmissionResponse,
) -> bool {
    let submission = &result.submission;
    submission.id == request.id
        && submission.operation_id == request.operation_id
        && submission.binding == request.binding
        && submission.binding.unit_id == unit
        && submission.expected_sha256 == request.expected_sha256
        && submission.expected_byte_length == request.expected_byte_length
        && submission.declarations == request.declarations
        && submission.expires_at == request.expires_at
}

impl Reply {
    pub fn acknowledges_actor(&self, command: &Command, actor: &str) -> bool {
        match (command, self) {
            (Command::CreateWork(_), Self::Mutation(reply)) => reply.work.owner_id == actor,
            (Command::AppendContent { .. }, Self::ContentSaved(reply)) => {
                reply.content.recorded_by == actor
            }
            (Command::CreateBrief { .. }, Self::Brief(reply)) => reply.brief.created_by == actor,
            (Command::Prepare { .. }, Self::Submission(reply)) => {
                reply.submission.owner_id == actor
            }
            (Command::Review { .. }, Self::Review(reply)) => reply.reviewer_id == actor,
            (Command::Candidate { .. }, Self::Candidate(reply)) => {
                reply.candidate.created_by == actor
            }
            // The grantor is always the authenticated owner, never a request field.
            (Command::IssueDelegation { .. }, Self::DelegationIssued(issued)) => {
                issued.grant.grantor_id == actor
            }
            (Command::PrepareImage { .. }, Self::ImagePrepared(r)) => r.owner_id == actor,
            (Command::UploadImage { .. }, Self::ImageAdmitted(r)) => r.owner_id == actor,
            (Command::PrepareImage { .. } | Command::UploadImage { .. }, Self::ImageIntent(r)) => {
                r.intent.owner_id == actor
            }
            // Reads and editor mutations may legitimately return another owner's record.
            _ => true,
        }
    }
}

impl Command {
    pub fn reconciliation_read(&self) -> Option<Self> {
        match self {
            Self::PrepareImage { work, request } => Some(Self::ImageIntent {
                work: work.clone(),
                intent: request.id.clone(),
            }),
            Self::UploadImage { work, intent, .. } => Some(Self::ImageIntent {
                work: work.clone(),
                intent: intent.id.clone(),
            }),
            Self::Prepare { unit, request } => Some(Self::Submission {
                unit: unit.clone(),
                submission: request.id.clone(),
            }),
            Self::Upload {
                binding,
                submission,
                ..
            } => Some(Self::Submission {
                unit: binding.unit_id.clone(),
                submission: submission.clone(),
            }),
            _ => None,
        }
    }
    pub fn reconciles(&self, reply: &Reply, actor: &str) -> bool {
        if let Reply::ImageIntent(status) = reply {
            if status.intent.owner_id != actor || !artwork::consistent_intent(status) {
                return false;
            }
            return match self {
                Self::PrepareImage { work, request } => {
                    artwork::prepared_matches(work, request, &status.intent)
                }
                Self::UploadImage {
                    work,
                    intent,
                    bytes,
                } => {
                    status.intent == *intent
                        && intent.work_id == *work
                        && bytes.len() as u64 == intent.request.expected_byte_length
                        // A correlated expiry settles failure, not admission. Pending without
                        // an asset still cannot establish whether the PUT committed.
                        && match status.state {
                            ImageIntentState::Expired => true,
                            ImageIntentState::Admitted => status
                                .asset
                                .as_ref()
                                .is_some_and(|asset| artwork::asset_matches(intent, asset)),
                            ImageIntentState::Pending => false,
                        }
                }
                _ => false,
            };
        }
        let Reply::Submission(status) = reply else {
            return false;
        };
        if status.submission.owner_id != actor || !consistent_rendition(status) {
            return false;
        }
        match self {
            Self::Prepare { unit, request } => prepared_submission_matches(unit, request, status),
            Self::Upload {
                binding,
                submission,
                sha256,
                bytes,
                ..
            } => {
                status.submission.id == *submission
                    && status.submission.binding == *binding
                    && status.submission.expected_sha256 == *sha256
                    && status.submission.expected_byte_length == bytes.len() as u64
                    && status.rendition.as_ref().is_some_and(|rendition| {
                        rendition.submission_id == *submission
                            && rendition.binding == *binding
                            && rendition.verified_media.sha256 == *sha256
                            && rendition.verified_media.byte_length == bytes.len() as u64
                    })
            }
            _ => false,
        }
    }
}
