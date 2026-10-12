//! Concrete route plans reuse the server's raw DTOs. No domain policy is decided here.
use cantos_contracts::*;
use serde::de::DeserializeOwned;
use std::sync::Arc;

pub const MAX_AUDIO_BYTES: u64 = 32 * 1024 * 1024;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Credentials {
    SameOrigin,
    Omit,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ClientError {
    InvalidInput,
    Cancelled,
    SessionChanged,
    ReceiptNotEstablished,
    Transport,
    Protocol { status: u16 },
    Http { status: u16, error: ApiError },
}
impl ClientError {
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
        })
    }
    pub fn method(&self) -> &'static str {
        match self {
            Self::PatchWork { .. } | Self::PatchUnit { .. } => "PATCH",
            Self::ArchiveWork { .. } | Self::ArchiveUnit { .. } | Self::Revoke { .. } => "DELETE",
            Self::Upload { .. } => "PUT",
            Self::CreateWork(_)
            | Self::CreateUnit { .. }
            | Self::Reorder { .. }
            | Self::AppendContent { .. }
            | Self::CreateBrief { .. }
            | Self::Prepare { .. }
            | Self::Review { .. }
            | Self::Candidate { .. } => "POST",
            _ => "GET",
        }
    }
    pub fn is_write(&self) -> bool {
        self.method() != "GET"
    }
    pub fn credentials(&self) -> Credentials {
        match self {
            Self::ProducerStatus { .. } | Self::Upload { .. } => Credentials::Omit,
            _ => Credentials::SameOrigin,
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
            _ => Ok(None),
        }
    }
    pub fn decode(&self, status: u16, body: &[u8]) -> Result<Reply, ClientError> {
        if matches!(self, Self::Revoke { .. }) && (200..300).contains(&status) {
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
        Ok(match self {
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
            Self::Upload { submission, .. } => {
                let rendition: AudioRenditionDocument = parse(body, status)?;
                if rendition.submission_id != *submission {
                    return Err(ClientError::Protocol { status });
                }
                Reply::Rendition(Box::new(rendition))
            }
            Self::Review { .. } => Reply::Review(parse(body, status)?),
            Self::Candidate { .. } | Self::CandidateRead { .. } => {
                Reply::Candidate(parse(body, status)?)
            }
            Self::Revoke { .. } => Reply::Revoked,
        })
    }
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
            // Reads and editor mutations may legitimately return another owner's record.
            _ => true,
        }
    }
}

impl Command {
    pub fn reconciliation_read(&self) -> Option<Self> {
        match self {
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
        let Reply::Submission(status) = reply else {
            return false;
        };
        if status.submission.owner_id != actor {
            return false;
        }
        match self {
            Self::Prepare { unit, request } => {
                status.submission.id == request.id
                    && status.submission.operation_id == request.operation_id
                    && status.submission.binding == request.binding
                    && status.submission.binding.unit_id == *unit
                    && status.submission.expected_sha256 == request.expected_sha256
                    && status.submission.expected_byte_length == request.expected_byte_length
                    && status.submission.declarations == request.declarations
                    && status.submission.expires_at == request.expires_at
            }
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
