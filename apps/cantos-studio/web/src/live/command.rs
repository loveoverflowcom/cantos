//! Concrete route plans reuse the server's raw DTOs. No domain policy is decided here.
use super::{artwork, delegation::IssuedGrant};
use crate::contract::*;
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
    /// The shared session owner holds this surface after a 429 (bounded `Retry-After` seconds, or
    /// its conservative default). Nothing is dispatched before the hold ends. For a write the
    /// outcome stays unknown: it is reconciled by its operation, never replayed blindly.
    RateLimited {
        seconds: u32,
    },
    Protocol {
        status: u16,
    },
    Http {
        status: u16,
        error: ApiError,
    },
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
    /// Identity failures as a command outcome. A request that never left (or whose session
    /// moved) is `Transport`/`SessionChanged`, both unknown outcomes for a write; only a real
    /// 401/403 carries an API error, and a 429 hold keeps its bounded wait. Nothing here retries.
    pub fn from_identity(error: cantos_web_auth::Failure) -> Self {
        use cantos_web_auth::Failure;
        match error {
            Failure::SignedOut | Failure::InvalidCredentials => Self::Http {
                status: 401,
                error: crate::session::failure_error(error),
            },
            // A wrong Origin or a roster/permission refusal: forbidden, never an expired session.
            Failure::Forbidden => Self::Http {
                status: 403,
                error: crate::session::failure_error(error),
            },
            Failure::Changed | Failure::AccountMismatch => Self::SessionChanged,
            // Held by the shared owner: nothing was dispatched, or the API refused before
            // acting. A write stays an unknown outcome for receipt reconciliation and is never
            // replayed here.
            Failure::RateLimited(seconds) => Self::RateLimited { seconds },
            Failure::NotFound
            | Failure::Invalid
            | Failure::Unavailable
            | Failure::Protocol
            | Failure::CleanupPending => Self::Transport,
        }
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
            | Self::RateLimited { .. }
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
    ViewerState {
        work: String,
    },
    ViewerChange {
        work: String,
        request: ViewerAccessRequest,
    },
    ViewerGroups,
    ViewerGroupChange {
        group: String,
        request: ViewerGroupRequest,
    },
    MetadataState {
        work: String,
    },
    MetadataCommand {
        work: String,
        request: MetadataPublicationRequest,
    },
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
    /// A read of the exact saved Work revision the creator names (ADR 0054 D03); never a write.
    Preview {
        work: String,
        work_revision: u64,
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
    PreviewAudio {
        subject: super::audio_preview::Subject,
    },
    Readiness {
        unit: String,
        candidate: String,
        candidate_digest: String,
    },
    RecordDistributionRights {
        unit: String,
        candidate: String,
        request: RecordDistributionRightsRequest,
    },
    RecordPublicationApproval {
        unit: String,
        candidate: String,
        request: RecordPublicationApprovalRequest,
    },
    Publish {
        unit: String,
        candidate: String,
        request: PublishAudioReleaseRequest,
    },
    ActiveRelease {
        unit: String,
    },
    Withdraw {
        unit: String,
        request: WithdrawAudioReleaseRequest,
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
    MetadataArtwork {
        work: String,
        revision: u64,
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
    ViewerState(ViewerAccessState),
    ViewerReceipt(ViewerAccessReceipt),
    ViewerGroups(ViewerGroups),
    ViewerGroupReceipt(ViewerGroupReceipt),
    MetadataState(MetadataPublicationState),
    Works(CatalogPage<CatalogWork>),
    Work(CatalogWork),
    Units(CatalogPage<CatalogContentUnit>),
    Preview(StudioPreview),
    Mutation(CatalogMutationResponse),
    Content(ContentRevisionResponse),
    ContentSaved(AppendContentResponse),
    History(ContentHistoryResponse),
    Brief(Box<ProductionBriefResponse>),
    Submission(Box<AudioSubmissionResponse>),
    Rendition(Box<AudioRenditionDocument>),
    Review(AudioReviewDocument),
    Candidate(ReleaseCandidateResponse),
    AudioPreview(super::audio_preview::VerifiedAudio),
    Readiness(Box<PublicationReadinessResponse>),
    DistributionRights(DistributionClearanceDocument),
    PublicationApproval(PublicationApprovalDocument),
    ActiveRelease(Box<ActiveAudioReleaseResponse>),
    Withdrawal(AudioReleaseWithdrawalResponse),
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
            Self::ViewerState { work } | Self::ViewerChange { work, .. } => {
                format!("{}/works/{}/viewer-access", routes::STUDIO_BASE, id(work)?)
            }
            Self::ViewerGroups => format!("{}/viewer-groups", routes::STUDIO_BASE),
            Self::ViewerGroupChange { group, .. } => {
                format!("{}/viewer-groups/{}", routes::STUDIO_BASE, id(group)?)
            }
            Self::MetadataState { work } | Self::MetadataCommand { work, .. } => {
                format!("{base}/works/{}/metadata-publication", id(work)?)
            }
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
            Self::Preview {
                work,
                work_revision,
                cursor,
            } => {
                if *work_revision == 0 || *work_revision > i64::MAX as u64 {
                    return Err(ClientError::InvalidInput);
                }
                format!(
                    "{}?work_revision={work_revision}{}",
                    routes::STUDIO_WORK_PREVIEW.replace("{work_id}", id(work)?),
                    cursor_query(cursor)?.replacen('?', "&", 1)
                )
            }
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
            Self::PreviewAudio { subject } => {
                if !subject.valid() {
                    return Err(ClientError::InvalidInput);
                }
                format!(
                    "{}/renditions/{}/bytes",
                    submission_path(&subject.binding.unit_id, &subject.submission)?,
                    id(&subject.rendition)?
                )
            }
            Self::Readiness {
                unit, candidate, ..
            } => format!(
                "{}/release-candidates/{}/readiness",
                unit_path(unit)?,
                id(candidate)?
            ),
            Self::RecordDistributionRights {
                unit, candidate, ..
            } => format!(
                "{}/release-candidates/{}/distribution-rights",
                unit_path(unit)?,
                id(candidate)?
            ),
            Self::RecordPublicationApproval {
                unit, candidate, ..
            } => format!(
                "{}/release-candidates/{}/publication-approvals",
                unit_path(unit)?,
                id(candidate)?
            ),
            Self::Publish {
                unit, candidate, ..
            } => format!(
                "{}/release-candidates/{}/publish",
                unit_path(unit)?,
                id(candidate)?
            ),
            Self::ActiveRelease { unit } => format!("{}/publication/active", unit_path(unit)?),
            Self::Withdraw { unit, .. } => format!("{}/publication/withdraw", unit_path(unit)?),
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
            Self::MetadataArtwork { work, revision } => {
                if *revision == 0 {
                    return Err(ClientError::InvalidInput);
                }
                format!(
                    "{base}/works/{}/artwork?work_revision={revision}",
                    id(work)?
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
            Self::ViewerChange { .. }
            | Self::ViewerGroupChange { .. }
            | Self::MetadataCommand { .. }
            | Self::CreateWork(_)
            | Self::CreateUnit { .. }
            | Self::Reorder { .. }
            | Self::AppendContent { .. }
            | Self::CreateBrief { .. }
            | Self::Prepare { .. }
            | Self::Review { .. }
            | Self::Candidate { .. }
            | Self::RecordDistributionRights { .. }
            | Self::RecordPublicationApproval { .. }
            | Self::Publish { .. }
            | Self::Withdraw { .. }
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
            Self::ViewerChange { request, .. } => encode(request),
            Self::ViewerGroupChange { request, .. } => encode(request),
            Self::MetadataCommand { request, .. } => encode(request),
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
            Self::RecordDistributionRights { request, .. } => encode(request),
            Self::RecordPublicationApproval { request, .. } => encode(request),
            Self::Publish { request, .. } => encode(request),
            Self::Withdraw { request, .. } => encode(request),
            Self::IssueDelegation { request, .. } => encode(request),
            Self::PrepareImage { request, .. } => encode(request),
            Self::SetArtwork { request, .. } => encode(request),
            _ => Ok(None),
        }
    }
    pub fn decode(&self, status: u16, body: &[u8]) -> Result<Reply, ClientError> {
        let exact_status = match self {
            Self::RecordDistributionRights { .. }
            | Self::RecordPublicationApproval { .. }
            | Self::Publish { .. }
            | Self::PrepareImage { .. } => Some(201),
            Self::Readiness { .. } | Self::ActiveRelease { .. } | Self::Withdraw { .. } => {
                Some(200)
            }
            Self::ViewerState { .. }
            | Self::ViewerChange { .. }
            | Self::ViewerGroups
            | Self::ViewerGroupChange { .. }
            | Self::MetadataState { .. }
            | Self::MetadataCommand { .. }
            | Self::Artwork { .. }
            | Self::MetadataArtwork { .. }
            | Self::SetArtwork { .. }
            | Self::ImageIntent { .. }
            | Self::UploadImage { .. }
            | Self::Preview { .. } => Some(200),
            _ => None,
        };
        if (200..300).contains(&status) && exact_status.is_some_and(|expected| expected != status) {
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
            return Err(
                if matches!(
                    self,
                    Self::ViewerState { .. }
                        | Self::ViewerChange { .. }
                        | Self::ViewerGroups
                        | Self::ViewerGroupChange { .. }
                ) {
                    super::viewer_access::http_error(status, body)
                } else {
                    ClientError::http(status, body)
                },
            );
        }
        fn parse<T: DeserializeOwned>(body: &[u8], status: u16) -> Result<T, ClientError> {
            serde_json::from_slice(body).map_err(|_| ClientError::Protocol { status })
        }
        let reply = match self {
            Self::ViewerState { .. } => Reply::ViewerState(parse(body, status)?),
            Self::ViewerChange { .. } => Reply::ViewerReceipt(parse(body, status)?),
            Self::ViewerGroups => Reply::ViewerGroups(parse(body, status)?),
            Self::ViewerGroupChange { .. } => Reply::ViewerGroupReceipt(parse(body, status)?),
            Self::MetadataState { .. } | Self::MetadataCommand { .. } => {
                Reply::MetadataState(parse(body, status)?)
            }
            Self::Works { .. } => Reply::Works(parse(body, status)?),
            Self::Work { .. } => Reply::Work(parse(body, status)?),
            Self::Units { .. } => Reply::Units(parse(body, status)?),
            Self::Preview { .. } => Reply::Preview(parse(body, status)?),
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
            Self::PreviewAudio { .. } => return Err(ClientError::Protocol { status }),
            Self::Readiness { .. } => Reply::Readiness(Box::new(parse(body, status)?)),
            Self::RecordDistributionRights { .. } => {
                Reply::DistributionRights(parse(body, status)?)
            }
            Self::RecordPublicationApproval { .. } => {
                Reply::PublicationApproval(parse(body, status)?)
            }
            Self::Publish { .. } | Self::ActiveRelease { .. } => {
                Reply::ActiveRelease(Box::new(parse(body, status)?))
            }
            Self::Withdraw { .. } => Reply::Withdrawal(parse(body, status)?),
            Self::Revoke { .. } | Self::RevokeDelegation { .. } => Reply::Revoked,
            Self::Artwork { .. } | Self::MetadataArtwork { .. } | Self::SetArtwork { .. } => {
                Reply::Artwork(parse(body, status)?)
            }
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
            Self::ViewerState { work } => {
                matches!(reply, Reply::ViewerState(state) if state.work_id == *work && state.grants.len() <= 1000 && state.pending.len() <= 100)
            }
            Self::ViewerChange { work, request } => {
                matches!(reply, Reply::ViewerReceipt(receipt) if receipt.work_id == *work && receipt.operation_id == request.operation_id && request.expected_version.checked_add(1) == Some(receipt.version))
            }
            Self::ViewerGroups => {
                matches!(reply, Reply::ViewerGroups(groups) if groups.items.len() <= 100 && groups.items.iter().all(|g| g.members.len() <= 1000))
            }
            Self::ViewerGroupChange { group, request } => {
                matches!(reply, Reply::ViewerGroupReceipt(receipt) if receipt.group_id == *group && receipt.operation_id == request.operation_id && request.expected_version.checked_add(1) == Some(receipt.version))
            }
            Self::MetadataState { work } => {
                matches!(reply, Reply::MetadataState(state) if state.work_id == *work && super::metadata_publication::valid_state(state))
            }
            Self::MetadataCommand { work, request } => {
                matches!(reply, Reply::MetadataState(state) if state.work_id == *work && super::metadata_publication::answers(request, state))
            }
            Self::Works { .. } => matches!(reply, Reply::Works(_)),
            Self::Work { id } => matches!(reply, Reply::Work(work) if work.id == *id),
            Self::Units { work, .. } => matches!(reply, Reply::Units(page)
                if page.items.iter().all(|unit| unit.work_id == *work)),
            // The reply must be the revision asked for: a banner naming another revision, or a
            // Unit of another Work, is a mismatch and never reaches the screen.
            Self::Preview {
                work,
                work_revision,
                ..
            } => matches!(reply, Reply::Preview(page)
                if page.work.id == *work && page.work.revision == *work_revision
                    && page.units.len() <= 50),
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
            Self::PreviewAudio { subject } => {
                matches!(reply, Reply::AudioPreview(result) if result.answers(subject))
            }
            Self::Readiness {
                candidate,
                candidate_digest,
                ..
            } => matches!(reply, Reply::Readiness(result)
                if result.candidate_id == *candidate
                    && result.candidate_digest == *candidate_digest
                    && consistent_readiness(result)),
            Self::RecordDistributionRights {
                candidate, request, ..
            } => matches!(reply, Reply::DistributionRights(result)
                if result.candidate_id == *candidate
                    && consistent_clearance(result)
                    && normalized_rights(&result.request) == normalized_rights(request)),
            Self::RecordPublicationApproval {
                candidate, request, ..
            } => matches!(reply, Reply::PublicationApproval(result)
                if result.candidate_id == *candidate && result.request == *request
                    && consistent_approval(result)),
            Self::Publish {
                unit,
                candidate,
                request,
            } => matches!(reply, Reply::ActiveRelease(result)
                if request.expected_pointer_version.checked_add(1) == Some(result.pointer_version)
                    && result.release.as_ref().is_some_and(|release|
                        consistent_release(unit, release)
                            && release.manifest.candidate_id == *candidate
                            && release.manifest.candidate_digest == request.candidate_digest
                            && release.supersedes == request.expected_active_release_id)),
            Self::ActiveRelease { unit } => matches!(reply, Reply::ActiveRelease(result)
                if result.release.as_ref().is_none_or(|release|
                    result.pointer_version > 0 && consistent_release(unit, release))),
            Self::Withdraw { request, .. } => matches!(reply, Reply::Withdrawal(result)
                if result.withdrawn_release_id == request.release_id
                    && (result.pointer_version == request.expected_pointer_version
                        || request.expected_pointer_version.checked_add(1) == Some(result.pointer_version))),
            Self::Revoke { .. } | Self::RevokeDelegation { .. } => matches!(reply, Reply::Revoked),
            Self::MetadataArtwork { work, revision } => {
                matches!(reply, Reply::Artwork(r) if r.work_id == *work && r.work_revision == *revision)
            }
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
            (Command::PreviewAudio { .. }, Self::AudioPreview(reply)) => {
                reply.subject().owner == actor
            }
            (Command::RecordDistributionRights { .. }, Self::DistributionRights(reply)) => {
                reply.actor_id == actor
            }
            (Command::RecordPublicationApproval { .. }, Self::PublicationApproval(reply)) => {
                reply.actor_id == actor
            }
            (Command::Publish { .. }, Self::ActiveRelease(reply)) => reply
                .release
                .as_ref()
                .is_some_and(|release| release.published_by == actor),
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

// These checks correlate server facts to the submitted question. They neither recompute domain
// fingerprints nor decide rights, approval or delivery eligibility in the client.
fn normalized_rights(request: &RecordDistributionRightsRequest) -> RecordDistributionRightsRequest {
    let mut normalized = request.clone();
    normalized.rights_refs.sort();
    normalized.evidence_refs.sort();
    normalized
}
fn consistent_clearance(record: &DistributionClearanceDocument) -> bool {
    record.sequence > 0
        && record.recorded_at >= 0
        && id(&record.id).is_ok()
        && id(&record.candidate_id).is_ok()
        && id(&record.request.operation_id).is_ok()
        && !record.actor_id.is_empty()
        && !record.digest.is_empty()
}
fn consistent_approval(record: &PublicationApprovalDocument) -> bool {
    record.sequence > 0
        && record.recorded_at >= 0
        && id(&record.id).is_ok()
        && id(&record.candidate_id).is_ok()
        && id(&record.request.operation_id).is_ok()
        && !record.actor_id.is_empty()
        && !record.digest.is_empty()
}
fn consistent_readiness(result: &PublicationReadinessResponse) -> bool {
    result
        .active_release_id
        .as_ref()
        .is_none_or(|release| result.pointer_version > 0 && id(release).is_ok())
        && result.clearance.as_ref().is_none_or(|record| {
            consistent_clearance(record)
                && record.candidate_id == result.candidate_id
                && record.request.candidate_digest == result.candidate_digest
        })
        && result.approval.as_ref().is_none_or(|record| {
            consistent_approval(record)
                && record.candidate_id == result.candidate_id
                && record.request.candidate_digest == result.candidate_digest
        })
}
fn consistent_release(unit: &str, release: &AudioReleaseDocument) -> bool {
    let manifest = &release.manifest;
    id(&release.id).is_ok()
        && release.published_at >= 0
        && !release.published_by.is_empty()
        && !release.digest.is_empty()
        && !manifest.digest.is_empty()
        && manifest.unit_id == unit
        && manifest.binding.unit_id == unit
        && manifest.work_id == manifest.binding.work_id
        && manifest.media.profile_id == manifest.binding.profile_id
        && manifest.candidate_id == manifest.distribution_clearance.candidate_id
        && manifest.candidate_id == manifest.publication_approval.candidate_id
        && manifest.candidate_digest == manifest.distribution_clearance.request.candidate_digest
        && manifest.candidate_digest == manifest.publication_approval.request.candidate_digest
        && manifest.publication_approval.request.clearance_digest
            == manifest.distribution_clearance.digest
        && release.published_by == manifest.distribution_clearance.actor_id
        && release.published_by == manifest.publication_approval.actor_id
        && consistent_clearance(&manifest.distribution_clearance)
        && consistent_approval(&manifest.publication_approval)
}
