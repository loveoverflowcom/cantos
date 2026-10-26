//! Visible and accessible copy belongs to the app, in both supported locales.
use crate::localization::generated::live as resources;
/// Locale-neutral field diagnostics keep the server path and rule unchanged.
pub fn diagnostic(path: &str, rule: &str) -> String {
    format!("{path} · {rule}")
}

/// Locale-neutral revision notation preserves the caller's exact title or kind label.
pub fn labelled_revision(label: &str, revision: u64) -> String {
    format!("{label} · r{revision}")
}

/// Locale-neutral history identity; never abbreviate the immutable revision ID.
pub fn revision_identity(revision: u64, id: &str) -> String {
    format!("r{revision} · {id}")
}

/// Locale-neutral notation for an immutable Script export, owned with visible Studio copy.
pub fn script_binding(script: &crate::contract::CatalogScriptRef) -> String {
    format!(
        "{} · r{} · {}",
        script.id, script.revision, script.export_digest
    )
}

/// Locale-neutral identity notation keeps complete IDs and digests unchanged.
pub fn brief_binding(brief: &crate::contract::ProductionBriefDocument) -> String {
    format!(
        "v{} · {}\n{}\nr{} · {}\n{}",
        brief.version,
        brief.id,
        brief.digest,
        brief.content.revision,
        brief.content.id,
        brief.content.revision_digest
    )
}

pub fn content_binding(content: &crate::contract::ContentRevisionResponse) -> String {
    format!(
        "r{} · {} · {}",
        content.revision, content.id, content.revision_digest
    )
}

pub fn submission_binding(submission: &crate::contract::AudioSubmissionDocument) -> String {
    format!(
        "{} · {} · {}",
        submission.id, submission.binding.brief_digest, submission.expected_sha256
    )
}

pub fn rendition_summary(rendition: &crate::contract::AudioRenditionDocument) -> String {
    format!(
        "{} · {} · {} B · {} ms",
        rendition.id,
        rendition.verified_media.profile_id,
        rendition.verified_media.byte_length,
        rendition.verified_media.duration_ms
    )
}

pub fn candidate_binding(candidate: &crate::contract::ReleaseCandidateDocument) -> String {
    format!(
        "{} · {} · {}",
        candidate.id, candidate.digest, candidate.rendition_digest
    )
}

pub fn active_release(
    release: &crate::contract::AudioReleaseDocument,
    pointer_version: u64,
) -> String {
    format!(
        "{} · {} · {}",
        release.id, release.manifest.digest, pointer_version
    )
}

pub fn work_visibility(value: &str, locale: crate::localization::Locale) -> &'static str {
    text(
        if value == "public" {
            Key::PublicVisibility
        } else {
            Key::PrivateVisibility
        },
        locale,
    )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    DistributionRevoked,
    AudioPreview,
    LoadAudioPreview,
    ClearAudioPreview,
    AudioPreviewHelp,
    AudioPreviewFailed,

    WorkVisibility,
    PrivateVisibility,
    PublicVisibility,
    SaveVisibility,
    VisibilityHelp,
    VisibilityConfirm,

    BriefHeadHelp,
    InspectDocument,
    CopyBrief,
    CopyBriefHelp,
    CopiedBrief,
    CopyBriefFailed,
    ProducerProvenance,
    CredentialMissing,
    Publication,
    PublicationHelp,
    ReadReadiness,
    ReadinessNotRead,
    ReadyToPublish,
    PublicationBlocked,
    DistributionRights,
    DistributionDecision,
    Cleared,
    ValidUntil,
    ValidUntilHelp,
    DistributionNotes,
    RecordRights,
    PublicationApproval,
    ApprovalNotes,
    RecordApproval,
    PublishRelease,
    PublishConfirm,
    ReadActiveRelease,
    ActiveRelease,
    NoActiveRelease,
    ActiveNotRead,
    PublicationReceipt,
    ReceiptNotCurrent,
    WithdrawRelease,
    WithdrawalReason,
    WithdrawConfirm,
    WithdrawalReceipt,
    ResetPublicationForm,
    ResetPublicationConfirm,
    PublicationFormDirty,

    Pane,
    Help,
    Refresh,
    Next,
    Previous,
    HistoryPageHelp,
    PageUnavailable,
    WorkResults,
    UnitResults,
    HistoryResults,
    Title,
    Kind,
    Rights,
    Synopsis,
    CreateWork,
    Work,
    CreateUnit,
    Unit,
    SaveMetadata,
    EditWork,
    ReadWork,
    ReloadWorkMetadata,
    ReloadWorkConfirm,
    ReloadMetadata,
    UnsavedDrafts,
    SelectedUnitMetadata,
    Source,
    Script,
    OpenEditor,
    EditorScript,
    ScriptBindingHelp,
    ArchiveWork,
    ArchiveUnit,
    ArchiveWorkConfirm,
    ArchiveUnitConfirm,
    ArchiveHelp,
    RefreshUnits,
    MoveEarlier,
    MoveLater,
    ReorderHelp,
    Content,
    Body,
    Text,
    Recording,
    Provenance,
    SaveContent,
    ReadHead,
    History,
    Brief,
    Version,
    Notes,
    Profile,
    Use,
    Attribution,
    Naming,
    Qa,
    CreateBrief,
    ReadBrief,
    Audio,
    File,
    Producer,
    Method,
    Quality,
    Prepare,
    Submission,
    ReadSubmission,
    Upload,
    Review,
    ReviewKind,
    Decision,
    Evidence,
    Candidate,
    CreateCandidate,
    ReadCandidate,
    Revoke,
    Cancel,
    Retry,
    Ready,
    Pending,
    Ambiguous,
    Idle,
    Failed,
    Private,
    Empty,
    SelectProfile,
    Confirm,
    ConfirmHelp,
    Stale,
    NoContent,
    FileReady,
    FileReadFailed,
    Reconcile,
    ReviewQc,
    ReviewRights,
    ReviewEditorial,
    Rejected,
    Approved,
    Withdrawn,
    Novel,
    Story,
    Podcast,
    Chapter,
    Episode,
    UnknownKind,
    ErrorInvalidInput,
    ErrorInterrupted,
    ErrorRateLimited,
    ErrorUnexpectedReply,
    ErrorSignIn,
    ErrorForbidden,
    ErrorNotFound,
    ErrorConflict,
    ErrorTooLarge,
    ErrorUnsupported,
    ErrorAdmission,
    ErrorUnavailable,
    FieldRequired,
    FieldByteLimit,
    FieldControlCharacter,
    FieldLanguage,
    FieldIdentifier,
    FieldRevision,
    FieldDigest,
    FieldDuplicate,
    FieldItemLimit,
    FieldChoice,
    FieldEmptyPatch,
    FieldReview,
}
pub fn text(key: Key, locale: crate::localization::Locale) -> &'static str {
    match key {
        Key::DistributionRevoked => resources::text(resources::Key::DistributionRevoked, locale),
        Key::AudioPreview => resources::text(resources::Key::AudioPreview, locale),
        Key::LoadAudioPreview => resources::text(resources::Key::LoadAudioPreview, locale),
        Key::ClearAudioPreview => resources::text(resources::Key::ClearAudioPreview, locale),
        Key::AudioPreviewHelp => resources::text(resources::Key::AudioPreviewHelp, locale),
        Key::AudioPreviewFailed => resources::text(resources::Key::AudioPreviewFailed, locale),
        Key::WorkVisibility => resources::text(resources::Key::WorkVisibility, locale),
        Key::PrivateVisibility => resources::text(resources::Key::PrivateVisibility, locale),
        Key::PublicVisibility => resources::text(resources::Key::PublicVisibility, locale),
        Key::SaveVisibility => resources::text(resources::Key::SaveVisibility, locale),
        Key::VisibilityHelp => resources::text(resources::Key::VisibilityHelp, locale),
        Key::VisibilityConfirm => resources::text(resources::Key::VisibilityConfirm, locale),
        Key::BriefHeadHelp => resources::text(resources::Key::BriefHeadHelp, locale),
        Key::InspectDocument => resources::text(resources::Key::InspectDocument, locale),
        Key::CopyBrief => resources::text(resources::Key::CopyBrief, locale),
        Key::CopyBriefHelp => resources::text(resources::Key::CopyBriefHelp, locale),
        Key::CopiedBrief => resources::text(resources::Key::CopiedBrief, locale),
        Key::CopyBriefFailed => resources::text(resources::Key::CopyBriefFailed, locale),
        Key::ProducerProvenance => resources::text(resources::Key::ProducerProvenance, locale),
        Key::CredentialMissing => resources::text(resources::Key::CredentialMissing, locale),
        Key::Publication => resources::text(resources::Key::Publication, locale),
        Key::PublicationHelp => resources::text(resources::Key::PublicationHelp, locale),
        Key::ReadReadiness => resources::text(resources::Key::ReadReadiness, locale),
        Key::ReadinessNotRead => resources::text(resources::Key::ReadinessNotRead, locale),
        Key::ReadyToPublish => resources::text(resources::Key::ReadyToPublish, locale),
        Key::PublicationBlocked => resources::text(resources::Key::PublicationBlocked, locale),
        Key::DistributionRights => resources::text(resources::Key::DistributionRights, locale),
        Key::DistributionDecision => resources::text(resources::Key::DistributionDecision, locale),
        Key::Cleared => resources::text(resources::Key::Cleared, locale),
        Key::ValidUntil => resources::text(resources::Key::ValidUntil, locale),
        Key::ValidUntilHelp => resources::text(resources::Key::ValidUntilHelp, locale),
        Key::DistributionNotes => resources::text(resources::Key::DistributionNotes, locale),
        Key::RecordRights => resources::text(resources::Key::RecordRights, locale),
        Key::PublicationApproval => resources::text(resources::Key::PublicationApproval, locale),
        Key::ApprovalNotes => resources::text(resources::Key::ApprovalNotes, locale),
        Key::RecordApproval => resources::text(resources::Key::RecordApproval, locale),
        Key::PublishRelease => resources::text(resources::Key::PublishRelease, locale),
        Key::PublishConfirm => resources::text(resources::Key::PublishConfirm, locale),
        Key::ReadActiveRelease => resources::text(resources::Key::ReadActiveRelease, locale),
        Key::ActiveRelease => resources::text(resources::Key::ActiveRelease, locale),
        Key::NoActiveRelease => resources::text(resources::Key::NoActiveRelease, locale),
        Key::ActiveNotRead => resources::text(resources::Key::ActiveNotRead, locale),
        Key::PublicationReceipt => resources::text(resources::Key::PublicationReceipt, locale),
        Key::ReceiptNotCurrent => resources::text(resources::Key::ReceiptNotCurrent, locale),
        Key::WithdrawRelease => resources::text(resources::Key::WithdrawRelease, locale),
        Key::WithdrawalReason => resources::text(resources::Key::WithdrawalReason, locale),
        Key::WithdrawConfirm => resources::text(resources::Key::WithdrawConfirm, locale),
        Key::WithdrawalReceipt => resources::text(resources::Key::WithdrawalReceipt, locale),
        Key::ResetPublicationForm => resources::text(resources::Key::ResetPublicationForm, locale),
        Key::ResetPublicationConfirm => {
            resources::text(resources::Key::ResetPublicationConfirm, locale)
        }
        Key::PublicationFormDirty => resources::text(resources::Key::PublicationFormDirty, locale),
        Key::UnsavedDrafts => resources::text(resources::Key::UnsavedDrafts, locale),
        Key::SelectedUnitMetadata => resources::text(resources::Key::SelectedUnitMetadata, locale),
        Key::Source => resources::text(resources::Key::Source, locale),
        Key::Script => resources::text(resources::Key::Script, locale),
        Key::OpenEditor => resources::text(resources::Key::OpenEditor, locale),
        Key::EditorScript => resources::text(resources::Key::EditorScript, locale),
        Key::ScriptBindingHelp => resources::text(resources::Key::ScriptBindingHelp, locale),
        Key::ReloadMetadata => resources::text(resources::Key::ReloadMetadata, locale),
        Key::ReloadWorkMetadata => resources::text(resources::Key::ReloadWorkMetadata, locale),
        Key::ReloadWorkConfirm => resources::text(resources::Key::ReloadWorkConfirm, locale),
        Key::ReadWork => resources::text(resources::Key::ReadWork, locale),
        Key::EditWork => resources::text(resources::Key::EditWork, locale),
        Key::ArchiveWork => resources::text(resources::Key::ArchiveWork, locale),
        Key::ArchiveUnit => resources::text(resources::Key::ArchiveUnit, locale),
        Key::ArchiveWorkConfirm => resources::text(resources::Key::ArchiveWorkConfirm, locale),
        Key::ArchiveUnitConfirm => resources::text(resources::Key::ArchiveUnitConfirm, locale),
        Key::ArchiveHelp => resources::text(resources::Key::ArchiveHelp, locale),
        Key::RefreshUnits => resources::text(resources::Key::RefreshUnits, locale),
        Key::MoveEarlier => resources::text(resources::Key::MoveEarlier, locale),
        Key::MoveLater => resources::text(resources::Key::MoveLater, locale),
        Key::ReorderHelp => resources::text(resources::Key::ReorderHelp, locale),
        Key::FileReadFailed => resources::text(resources::Key::FileReadFailed, locale),
        Key::Reconcile => resources::text(resources::Key::Reconcile, locale),
        Key::ReviewQc => resources::text(resources::Key::ReviewQc, locale),
        Key::ReviewRights => resources::text(resources::Key::ReviewRights, locale),
        Key::ReviewEditorial => resources::text(resources::Key::ReviewEditorial, locale),
        Key::Rejected => resources::text(resources::Key::Rejected, locale),
        Key::Approved => resources::text(resources::Key::Approved, locale),
        Key::Withdrawn => resources::text(resources::Key::Withdrawn, locale),
        Key::Novel => resources::text(resources::Key::Novel, locale),
        Key::Story => resources::text(resources::Key::Story, locale),
        Key::Podcast => resources::text(resources::Key::Podcast, locale),
        Key::Chapter => resources::text(resources::Key::Chapter, locale),
        Key::Episode => resources::text(resources::Key::Episode, locale),
        Key::UnknownKind => resources::text(resources::Key::UnknownKind, locale),
        Key::ErrorInvalidInput => resources::text(resources::Key::ErrorInvalidInput, locale),
        Key::ErrorInterrupted => resources::text(resources::Key::ErrorInterrupted, locale),
        Key::ErrorRateLimited => resources::text(resources::Key::ErrorRateLimited, locale),
        Key::ErrorUnexpectedReply => resources::text(resources::Key::ErrorUnexpectedReply, locale),
        Key::ErrorSignIn => resources::text(resources::Key::ErrorSignIn, locale),
        Key::ErrorForbidden => resources::text(resources::Key::ErrorForbidden, locale),
        Key::ErrorNotFound => resources::text(resources::Key::ErrorNotFound, locale),
        Key::ErrorConflict => resources::text(resources::Key::ErrorConflict, locale),
        Key::ErrorTooLarge => resources::text(resources::Key::ErrorTooLarge, locale),
        Key::ErrorUnsupported => resources::text(resources::Key::ErrorUnsupported, locale),
        Key::ErrorAdmission => resources::text(resources::Key::ErrorAdmission, locale),
        Key::ErrorUnavailable => resources::text(resources::Key::ErrorUnavailable, locale),
        Key::FieldRequired => resources::text(resources::Key::FieldRequired, locale),
        Key::FieldByteLimit => resources::text(resources::Key::FieldByteLimit, locale),
        Key::FieldControlCharacter => {
            resources::text(resources::Key::FieldControlCharacter, locale)
        }
        Key::FieldLanguage => resources::text(resources::Key::FieldLanguage, locale),
        Key::FieldIdentifier => resources::text(resources::Key::FieldIdentifier, locale),
        Key::FieldRevision => resources::text(resources::Key::FieldRevision, locale),
        Key::FieldDigest => resources::text(resources::Key::FieldDigest, locale),
        Key::FieldDuplicate => resources::text(resources::Key::FieldDuplicate, locale),
        Key::FieldItemLimit => resources::text(resources::Key::FieldItemLimit, locale),
        Key::FieldChoice => resources::text(resources::Key::FieldChoice, locale),
        Key::FieldEmptyPatch => resources::text(resources::Key::FieldEmptyPatch, locale),
        Key::FieldReview => resources::text(resources::Key::FieldReview, locale),
        Key::Pane => resources::text(resources::Key::Pane, locale),
        Key::Help => resources::text(resources::Key::Help, locale),
        Key::Refresh => resources::text(resources::Key::Refresh, locale),
        Key::Previous => resources::text(resources::Key::Previous, locale),
        Key::WorkResults => resources::text(resources::Key::WorkResults, locale),
        Key::UnitResults => resources::text(resources::Key::UnitResults, locale),
        Key::HistoryResults => resources::text(resources::Key::HistoryResults, locale),
        Key::HistoryPageHelp => resources::text(resources::Key::HistoryPageHelp, locale),
        Key::PageUnavailable => resources::text(resources::Key::PageUnavailable, locale),
        Key::Next => resources::text(resources::Key::Next, locale),
        Key::Title => resources::text(resources::Key::Title, locale),
        Key::Kind => resources::text(resources::Key::Kind, locale),
        Key::Rights => resources::text(resources::Key::Rights, locale),
        Key::Synopsis => resources::text(resources::Key::Synopsis, locale),
        Key::CreateWork => resources::text(resources::Key::CreateWork, locale),
        Key::Work => resources::text(resources::Key::Work, locale),
        Key::CreateUnit => resources::text(resources::Key::CreateUnit, locale),
        Key::Unit => resources::text(resources::Key::Unit, locale),
        Key::SaveMetadata => resources::text(resources::Key::SaveMetadata, locale),
        Key::Content => resources::text(resources::Key::Content, locale),
        Key::Body => resources::text(resources::Key::Body, locale),
        Key::Text => resources::text(resources::Key::Text, locale),
        Key::Recording => resources::text(resources::Key::Recording, locale),
        Key::Provenance => resources::text(resources::Key::Provenance, locale),
        Key::SaveContent => resources::text(resources::Key::SaveContent, locale),
        Key::ReadHead => resources::text(resources::Key::ReadHead, locale),
        Key::History => resources::text(resources::Key::History, locale),
        Key::Brief => resources::text(resources::Key::Brief, locale),
        Key::Version => resources::text(resources::Key::Version, locale),
        Key::Notes => resources::text(resources::Key::Notes, locale),
        Key::Profile => resources::text(resources::Key::Profile, locale),
        Key::Use => resources::text(resources::Key::Use, locale),
        Key::Attribution => resources::text(resources::Key::Attribution, locale),
        Key::Naming => resources::text(resources::Key::Naming, locale),
        Key::Qa => resources::text(resources::Key::Qa, locale),
        Key::CreateBrief => resources::text(resources::Key::CreateBrief, locale),
        Key::ReadBrief => resources::text(resources::Key::ReadBrief, locale),
        Key::Audio => resources::text(resources::Key::Audio, locale),
        Key::File => resources::text(resources::Key::File, locale),
        Key::Producer => resources::text(resources::Key::Producer, locale),
        Key::Method => resources::text(resources::Key::Method, locale),
        Key::Quality => resources::text(resources::Key::Quality, locale),
        Key::Prepare => resources::text(resources::Key::Prepare, locale),
        Key::Submission => resources::text(resources::Key::Submission, locale),
        Key::ReadSubmission => resources::text(resources::Key::ReadSubmission, locale),
        Key::Upload => resources::text(resources::Key::Upload, locale),
        Key::Review => resources::text(resources::Key::Review, locale),
        Key::ReviewKind => resources::text(resources::Key::ReviewKind, locale),
        Key::Decision => resources::text(resources::Key::Decision, locale),
        Key::Evidence => resources::text(resources::Key::Evidence, locale),
        Key::Candidate => resources::text(resources::Key::Candidate, locale),
        Key::CreateCandidate => resources::text(resources::Key::CreateCandidate, locale),
        Key::ReadCandidate => resources::text(resources::Key::ReadCandidate, locale),
        Key::Revoke => resources::text(resources::Key::Revoke, locale),
        Key::Cancel => resources::text(resources::Key::Cancel, locale),
        Key::Retry => resources::text(resources::Key::Retry, locale),
        Key::Ready => resources::text(resources::Key::Ready, locale),
        Key::Pending => resources::text(resources::Key::Pending, locale),
        Key::Ambiguous => resources::text(resources::Key::Ambiguous, locale),
        Key::Idle => resources::text(resources::Key::Idle, locale),
        Key::Failed => resources::text(resources::Key::Failed, locale),
        Key::Private => resources::text(resources::Key::Private, locale),
        Key::Empty => resources::text(resources::Key::Empty, locale),
        Key::SelectProfile => resources::text(resources::Key::SelectProfile, locale),
        Key::Confirm => resources::text(resources::Key::Confirm, locale),
        Key::ConfirmHelp => resources::text(resources::Key::ConfirmHelp, locale),
        Key::Stale => resources::text(resources::Key::Stale, locale),
        Key::NoContent => resources::text(resources::Key::NoContent, locale),
        Key::FileReady => resources::text(resources::Key::FileReady, locale),
    }
}

/// UI copy is independent from the protocol values kept in the create request.
pub fn work_kind(kind: &str, locale: crate::localization::Locale) -> &'static str {
    text(
        match kind {
            "novel" => Key::Novel,
            "story" => Key::Story,
            "podcast" => Key::Podcast,
            _ => Key::UnknownKind,
        },
        locale,
    )
}

pub fn unit_kind(kind: &str, locale: crate::localization::Locale) -> &'static str {
    text(
        match kind {
            "chapter" => Key::Chapter,
            "episode" => Key::Episode,
            _ => Key::UnknownKind,
        },
        locale,
    )
}

pub fn review_kind(
    kind: crate::contract::AudioReviewKind,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::AudioReviewKind;
    text(
        match kind {
            AudioReviewKind::Qc => Key::ReviewQc,
            AudioReviewKind::Rights => Key::ReviewRights,
            AudioReviewKind::Editorial => Key::ReviewEditorial,
        },
        locale,
    )
}

pub fn review_decision(
    decision: crate::contract::AudioReviewDecision,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::AudioReviewDecision;
    text(
        match decision {
            AudioReviewDecision::Rejected => Key::Rejected,
            AudioReviewDecision::Approved => Key::Approved,
            AudioReviewDecision::Withdrawn => Key::Withdrawn,
        },
        locale,
    )
}

/// Closed client errors map to resources; transport/debug details never become user prose.
pub fn error_key(error: &super::ClientError) -> Key {
    use super::ClientError;
    match error {
        ClientError::InvalidInput => Key::ErrorInvalidInput,
        ClientError::Cancelled
        | ClientError::SessionChanged
        | ClientError::ReceiptNotEstablished
        | ClientError::Transport => Key::ErrorInterrupted,
        ClientError::RateLimited { .. } => Key::ErrorRateLimited,
        ClientError::ReplyMismatch | ClientError::Protocol { .. } => Key::ErrorUnexpectedReply,
        ClientError::Http { status, .. } => match status {
            401 => Key::ErrorSignIn,
            403 => Key::ErrorForbidden,
            404 => Key::ErrorNotFound,
            409 => Key::ErrorConflict,
            413 => Key::ErrorTooLarge,
            415 => Key::ErrorUnsupported,
            422 => Key::ErrorAdmission,
            _ => Key::ErrorUnavailable,
        },
    }
}

/// Known Catalog/Content validation facts select guidance; open wire rules stay diagnostic.
pub fn field_rule_key(rule: &str) -> Key {
    match rule {
        "required" => Key::FieldRequired,
        "control_character" => Key::FieldControlCharacter,
        "language_tag" | "invalid_language" => Key::FieldLanguage,
        "canonical_uuid" | "invalid_uuid" | "token_id" => Key::FieldIdentifier,
        "positive_revision" | "invalid_revision" => Key::FieldRevision,
        "sha256" | "export_digest" | "content_digest" => Key::FieldDigest,
        "duplicate_id" => Key::FieldDuplicate,
        "max_items_64" => Key::FieldItemLimit,
        "work_kind" | "unit_kind" | "visibility" => Key::FieldChoice,
        "empty_patch" => Key::FieldEmptyPatch,
        _ if rule
            .strip_prefix("max_bytes_")
            .filter(|limit| limit.bytes().all(|byte| byte.is_ascii_digit()))
            .and_then(|limit| limit.parse::<u64>().ok())
            .is_some_and(|limit| limit > 0) =>
        {
            Key::FieldByteLimit
        }
        _ => Key::FieldReview,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contract::{ApiError, ErrorCode};
    use crate::live::ClientError;

    #[test]
    fn every_workspace_resource_has_both_locales() {
        for key in [
            Key::Pane,
            Key::Help,
            Key::Refresh,
            Key::Next,
            Key::Previous,
            Key::HistoryPageHelp,
            Key::PageUnavailable,
            Key::WorkResults,
            Key::UnitResults,
            Key::HistoryResults,
            Key::Title,
            Key::Kind,
            Key::Rights,
            Key::Synopsis,
            Key::CreateWork,
            Key::Work,
            Key::CreateUnit,
            Key::Unit,
            Key::SaveMetadata,
            Key::Content,
            Key::Body,
            Key::Text,
            Key::Recording,
            Key::Provenance,
            Key::SaveContent,
            Key::ReadHead,
            Key::History,
            Key::Brief,
            Key::Version,
            Key::Notes,
            Key::Profile,
            Key::Use,
            Key::Attribution,
            Key::Naming,
            Key::Qa,
            Key::CreateBrief,
            Key::ReadBrief,
            Key::Audio,
            Key::File,
            Key::Producer,
            Key::Method,
            Key::Quality,
            Key::Prepare,
            Key::Submission,
            Key::ReadSubmission,
            Key::Upload,
            Key::Review,
            Key::ReviewKind,
            Key::Decision,
            Key::Evidence,
            Key::Candidate,
            Key::CreateCandidate,
            Key::ReadCandidate,
            Key::Revoke,
            Key::Cancel,
            Key::Retry,
            Key::Ready,
            Key::Pending,
            Key::Ambiguous,
            Key::Idle,
            Key::Failed,
            Key::Private,
            Key::Empty,
            Key::SelectProfile,
            Key::Confirm,
            Key::ConfirmHelp,
            Key::Stale,
            Key::NoContent,
            Key::FileReady,
            Key::FileReadFailed,
            Key::Reconcile,
            Key::ReviewQc,
            Key::ReviewRights,
            Key::ReviewEditorial,
            Key::Rejected,
            Key::Approved,
            Key::Withdrawn,
            Key::Novel,
            Key::Story,
            Key::Podcast,
            Key::Chapter,
            Key::Episode,
            Key::UnknownKind,
            Key::ErrorInvalidInput,
            Key::ErrorInterrupted,
            Key::ErrorRateLimited,
            Key::ErrorUnexpectedReply,
            Key::ErrorSignIn,
            Key::ErrorForbidden,
            Key::ErrorNotFound,
            Key::ErrorConflict,
            Key::ErrorTooLarge,
            Key::ErrorUnsupported,
            Key::ErrorAdmission,
            Key::ErrorUnavailable,
            Key::FieldRequired,
            Key::FieldByteLimit,
            Key::FieldControlCharacter,
            Key::FieldLanguage,
            Key::FieldIdentifier,
            Key::FieldRevision,
            Key::FieldDigest,
            Key::FieldDuplicate,
            Key::FieldItemLimit,
            Key::FieldChoice,
            Key::FieldEmptyPatch,
            Key::FieldReview,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!text(key, locale).trim().is_empty(), "{key:?} / {locale}");
            }
        }
    }

    #[test]
    fn field_rule_guidance_maps_known_facts_and_never_echoes_unknown_rule_text() {
        for (rule, expected) in [
            ("required", Key::FieldRequired),
            ("max_bytes_512", Key::FieldByteLimit),
            ("max_bytes_65536", Key::FieldByteLimit),
            ("control_character", Key::FieldControlCharacter),
            ("language_tag", Key::FieldLanguage),
            ("invalid_language", Key::FieldLanguage),
            ("canonical_uuid", Key::FieldIdentifier),
            ("invalid_uuid", Key::FieldIdentifier),
            ("token_id", Key::FieldIdentifier),
            ("positive_revision", Key::FieldRevision),
            ("invalid_revision", Key::FieldRevision),
            ("sha256", Key::FieldDigest),
            ("export_digest", Key::FieldDigest),
            ("content_digest", Key::FieldDigest),
            ("duplicate_id", Key::FieldDuplicate),
            ("max_items_64", Key::FieldItemLimit),
            ("work_kind", Key::FieldChoice),
            ("unit_kind", Key::FieldChoice),
            ("visibility", Key::FieldChoice),
            ("empty_patch", Key::FieldEmptyPatch),
        ] {
            assert_eq!(field_rule_key(rule), expected, "{rule}");
        }
        for rule in [
            "",
            "future_rule_<script>{time}",
            "max_bytes_",
            "max_bytes_0",
            "max_bytes_-1",
            "max_bytes_+512",
            "max_bytes_512 backend prose",
            "max_bytes_18446744073709551616",
        ] {
            assert_eq!(field_rule_key(rule), Key::FieldReview, "{rule}");
            assert_eq!(
                text(
                    field_rule_key(rule),
                    crate::localization::Locale::Vietnamese
                ),
                "Kiểm tra trường này và mã chẩn đoán."
            );
            assert_eq!(
                text(field_rule_key(rule), crate::localization::Locale::English),
                "Review this field and its diagnostic code."
            );
        }
    }

    #[test]
    fn every_client_failure_maps_to_copy_without_transport_or_server_prose() {
        let cases = [
            (ClientError::InvalidInput, Key::ErrorInvalidInput),
            (ClientError::Cancelled, Key::ErrorInterrupted),
            (ClientError::SessionChanged, Key::ErrorInterrupted),
            (ClientError::ReceiptNotEstablished, Key::ErrorInterrupted),
            (ClientError::Transport, Key::ErrorInterrupted),
            (
                ClientError::RateLimited { seconds: 30 },
                Key::ErrorRateLimited,
            ),
            (ClientError::ReplyMismatch, Key::ErrorUnexpectedReply),
            (
                ClientError::Protocol { status: 502 },
                Key::ErrorUnexpectedReply,
            ),
        ];
        for (error, expected) in cases {
            assert_eq!(error_key(&error), expected);
            for locale in crate::localization::Locale::supported() {
                assert!(!text(error_key(&error), locale).is_empty());
            }
        }
        for (status, expected) in [
            (401, Key::ErrorSignIn),
            (403, Key::ErrorForbidden),
            (404, Key::ErrorNotFound),
            (409, Key::ErrorConflict),
            (413, Key::ErrorTooLarge),
            (415, Key::ErrorUnsupported),
            (422, Key::ErrorAdmission),
            (500, Key::ErrorUnavailable),
            (599, Key::ErrorUnavailable),
        ] {
            let error = ClientError::Http {
                status,
                error: ApiError {
                    code: ErrorCode::InvalidCatalog,
                    current_revision: Some(987654321),
                    issues: vec![],
                },
            };
            assert_eq!(error_key(&error), expected);
            for locale in crate::localization::Locale::supported() {
                let message = text(error_key(&error), locale);
                assert!(!message.contains("HTTP"));
                assert!(!message.contains("InvalidCatalog"));
                assert!(!message.contains("987654321"));
            }
        }
    }

    #[test]
    fn script_binding_preserves_exact_identifiers_without_expanding_inserted_braces() {
        let script = crate::contract::CatalogScriptRef {
            id: "script-{revision}".into(),
            revision: 7,
            export_digest: "sir-e1:sha256:abc".into(),
        };
        assert_eq!(
            script_binding(&script),
            "script-{revision} · r7 · sir-e1:sha256:abc"
        );
    }

    #[test]
    fn review_variants_and_catalog_types_have_exact_bilingual_presentation() {
        use crate::contract::{AudioReviewDecision as Decision, AudioReviewKind as Kind};
        for (kind, vi, locale) in [
            (Kind::Qc, "QC", "QC"),
            (Kind::Rights, "Quyền sử dụng", "Rights"),
            (Kind::Editorial, "Biên tập", "Editorial"),
        ] {
            assert_eq!(
                review_kind(kind, crate::localization::Locale::Vietnamese),
                vi
            );
            assert_eq!(
                review_kind(kind, crate::localization::Locale::English),
                locale
            );
        }
        for (decision, vi, locale) in [
            (Decision::Rejected, "Từ chối", "Rejected"),
            (Decision::Approved, "Chấp thuận", "Approved"),
            (Decision::Withdrawn, "Rút lại", "Withdrawn"),
        ] {
            assert_eq!(
                review_decision(decision, crate::localization::Locale::Vietnamese),
                vi
            );
            assert_eq!(
                review_decision(decision, crate::localization::Locale::English),
                locale
            );
        }
        assert_eq!(
            work_kind("story", crate::localization::Locale::Vietnamese),
            "Truyện"
        );
        assert_eq!(
            unit_kind("episode", crate::localization::Locale::English),
            "Episode"
        );
        assert_eq!(
            unit_kind("chapter", crate::localization::Locale::Vietnamese),
            "Chương"
        );
        assert_eq!(
            work_kind("new-private-kind", crate::localization::Locale::Vietnamese),
            "Loại chưa xác định"
        );
        assert_eq!(
            unit_kind("new-private-kind", crate::localization::Locale::English),
            "Unknown type"
        );
    }
}

pub fn brief_freshness(
    value: crate::contract::BriefFreshness,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::BriefFreshness;
    match value {
        BriefFreshness::Current => resources::text(resources::Key::BriefFreshnessCurrent, locale),
        BriefFreshness::ContentChanged => {
            resources::text(resources::Key::BriefFreshnessContentChanged, locale)
        }
        BriefFreshness::RightsChanged => {
            resources::text(resources::Key::BriefFreshnessRightsChanged, locale)
        }
        BriefFreshness::GuidanceChanged => {
            resources::text(resources::Key::BriefFreshnessGuidanceChanged, locale)
        }
        BriefFreshness::Archived => resources::text(resources::Key::BriefFreshnessArchived, locale),
    }
}
pub fn submission_state(
    value: crate::contract::AudioSubmissionState,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::AudioSubmissionState;
    match value {
        AudioSubmissionState::AwaitingBytes => {
            resources::text(resources::Key::SubmissionStateAwaitingBytes, locale)
        }
        AudioSubmissionState::Admitted => {
            resources::text(resources::Key::SubmissionStateAdmitted, locale)
        }
        AudioSubmissionState::Revoked => {
            resources::text(resources::Key::SubmissionStateRevoked, locale)
        }
        AudioSubmissionState::Expired => {
            resources::text(resources::Key::SubmissionStateExpired, locale)
        }
        AudioSubmissionState::Stale => {
            resources::text(resources::Key::SubmissionStateStale, locale)
        }
    }
}
pub fn audio_blocker(
    value: crate::contract::AudioReleaseBlocker,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::AudioReleaseBlocker;
    match value {
        AudioReleaseBlocker::StaleBrief => {
            resources::text(resources::Key::AudioBlockerStaleBrief, locale)
        }
        AudioReleaseBlocker::UploadPermissionRevoked => {
            resources::text(resources::Key::AudioBlockerUploadPermissionRevoked, locale)
        }
        AudioReleaseBlocker::UploadPermissionExpired => {
            resources::text(resources::Key::AudioBlockerUploadPermissionExpired, locale)
        }
        AudioReleaseBlocker::RightsUnrecorded => {
            resources::text(resources::Key::AudioBlockerRightsUnrecorded, locale)
        }
        AudioReleaseBlocker::QcNotApproved => {
            resources::text(resources::Key::AudioBlockerQcNotApproved, locale)
        }
        AudioReleaseBlocker::RightsNotApproved => {
            resources::text(resources::Key::AudioBlockerRightsNotApproved, locale)
        }
        AudioReleaseBlocker::EditorialNotApproved => {
            resources::text(resources::Key::AudioBlockerEditorialNotApproved, locale)
        }
        AudioReleaseBlocker::CandidateReviewsChanged => {
            resources::text(resources::Key::AudioBlockerCandidateReviewsChanged, locale)
        }
        AudioReleaseBlocker::MetadataChanged => {
            resources::text(resources::Key::AudioBlockerMetadataChanged, locale)
        }
        AudioReleaseBlocker::PublicDeliveryNotImplemented => resources::text(
            resources::Key::AudioBlockerPublicDeliveryNotImplemented,
            locale,
        ),
    }
}
pub fn publication_blocker(
    value: crate::contract::PublicationBlocker,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::PublicationBlocker;
    match value {
        PublicationBlocker::CandidateMismatch => {
            resources::text(resources::Key::PublicationBlockerCandidateMismatch, locale)
        }
        PublicationBlocker::OwnerScopeLost => {
            resources::text(resources::Key::PublicationBlockerOwnerScopeLost, locale)
        }
        PublicationBlocker::Archived => {
            resources::text(resources::Key::PublicationBlockerArchived, locale)
        }
        PublicationBlocker::StaleBrief => {
            resources::text(resources::Key::PublicationBlockerStaleBrief, locale)
        }
        PublicationBlocker::MetadataChanged => {
            resources::text(resources::Key::PublicationBlockerMetadataChanged, locale)
        }
        PublicationBlocker::UploadPermissionRevoked => resources::text(
            resources::Key::PublicationBlockerUploadPermissionRevoked,
            locale,
        ),
        PublicationBlocker::UploadPermissionExpired => resources::text(
            resources::Key::PublicationBlockerUploadPermissionExpired,
            locale,
        ),
        PublicationBlocker::ProfileUnsupported => {
            resources::text(resources::Key::PublicationBlockerProfileUnsupported, locale)
        }
        PublicationBlocker::ObjectMissing => {
            resources::text(resources::Key::PublicationBlockerObjectMissing, locale)
        }
        PublicationBlocker::ObjectUnverified => {
            resources::text(resources::Key::PublicationBlockerObjectUnverified, locale)
        }
        PublicationBlocker::ObjectIntegrityMismatch => resources::text(
            resources::Key::PublicationBlockerObjectIntegrityMismatch,
            locale,
        ),
        PublicationBlocker::QcNotApproved => {
            resources::text(resources::Key::PublicationBlockerQcNotApproved, locale)
        }
        PublicationBlocker::RightsReviewNotApproved => resources::text(
            resources::Key::PublicationBlockerRightsReviewNotApproved,
            locale,
        ),
        PublicationBlocker::EditorialNotApproved => resources::text(
            resources::Key::PublicationBlockerEditorialNotApproved,
            locale,
        ),
        PublicationBlocker::CandidateReviewsChanged => resources::text(
            resources::Key::PublicationBlockerCandidateReviewsChanged,
            locale,
        ),
        PublicationBlocker::RightsUnrecorded => {
            resources::text(resources::Key::PublicationBlockerRightsUnrecorded, locale)
        }
        PublicationBlocker::RightsRevoked => {
            resources::text(resources::Key::PublicationBlockerRightsRevoked, locale)
        }
        PublicationBlocker::RightsExpired => {
            resources::text(resources::Key::PublicationBlockerRightsExpired, locale)
        }
        PublicationBlocker::RightsScopeMismatch => resources::text(
            resources::Key::PublicationBlockerRightsScopeMismatch,
            locale,
        ),
        PublicationBlocker::ApprovalMissing => {
            resources::text(resources::Key::PublicationBlockerApprovalMissing, locale)
        }
        PublicationBlocker::ApprovalWithdrawn => {
            resources::text(resources::Key::PublicationBlockerApprovalWithdrawn, locale)
        }
        PublicationBlocker::ApprovalStale => {
            resources::text(resources::Key::PublicationBlockerApprovalStale, locale)
        }
        PublicationBlocker::DeliveryUnavailable => resources::text(
            resources::Key::PublicationBlockerDeliveryUnavailable,
            locale,
        ),
        PublicationBlocker::DeliveryMissing => {
            resources::text(resources::Key::PublicationBlockerDeliveryMissing, locale)
        }
        PublicationBlocker::DeliveryIntegrityMismatch => resources::text(
            resources::Key::PublicationBlockerDeliveryIntegrityMismatch,
            locale,
        ),
        PublicationBlocker::DeliveryEvidenceExpired => resources::text(
            resources::Key::PublicationBlockerDeliveryEvidenceExpired,
            locale,
        ),
    }
}

pub fn publication_confirmation(
    action: Key,
    title: &str,
    identity: &str,
    locale: crate::localization::Locale,
) -> String {
    format!("{}\n{}\n{}", text(action, locale), title, identity)
}

#[cfg(test)]
mod production_copy_tests {
    use super::*;
    #[test]
    fn production_resources_and_closed_states_have_both_locales() {
        for key in [
            Key::BriefHeadHelp,
            Key::InspectDocument,
            Key::CopyBrief,
            Key::CopyBriefHelp,
            Key::CopiedBrief,
            Key::CopyBriefFailed,
            Key::ProducerProvenance,
            Key::CredentialMissing,
            Key::Publication,
            Key::PublicationHelp,
            Key::ReadReadiness,
            Key::ReadinessNotRead,
            Key::ReadyToPublish,
            Key::PublicationBlocked,
            Key::DistributionRights,
            Key::DistributionDecision,
            Key::Cleared,
            Key::ValidUntil,
            Key::ValidUntilHelp,
            Key::DistributionNotes,
            Key::RecordRights,
            Key::PublicationApproval,
            Key::ApprovalNotes,
            Key::RecordApproval,
            Key::PublishRelease,
            Key::PublishConfirm,
            Key::ReadActiveRelease,
            Key::ActiveRelease,
            Key::NoActiveRelease,
            Key::ActiveNotRead,
            Key::PublicationReceipt,
            Key::ReceiptNotCurrent,
            Key::WithdrawRelease,
            Key::WithdrawalReason,
            Key::WithdrawConfirm,
            Key::WithdrawalReceipt,
            Key::ResetPublicationForm,
            Key::ResetPublicationConfirm,
            Key::PublicationFormDirty,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!text(key, locale).trim().is_empty());
            }
        }
        for value in [
            crate::contract::BriefFreshness::Current,
            crate::contract::BriefFreshness::ContentChanged,
            crate::contract::BriefFreshness::RightsChanged,
            crate::contract::BriefFreshness::GuidanceChanged,
            crate::contract::BriefFreshness::Archived,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!brief_freshness(value.clone(), locale).trim().is_empty());
            }
        }
        for value in [
            crate::contract::AudioSubmissionState::AwaitingBytes,
            crate::contract::AudioSubmissionState::Admitted,
            crate::contract::AudioSubmissionState::Revoked,
            crate::contract::AudioSubmissionState::Expired,
            crate::contract::AudioSubmissionState::Stale,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!submission_state(value, locale).trim().is_empty());
            }
        }
        for value in [
            crate::contract::AudioReleaseBlocker::StaleBrief,
            crate::contract::AudioReleaseBlocker::UploadPermissionRevoked,
            crate::contract::AudioReleaseBlocker::UploadPermissionExpired,
            crate::contract::AudioReleaseBlocker::RightsUnrecorded,
            crate::contract::AudioReleaseBlocker::QcNotApproved,
            crate::contract::AudioReleaseBlocker::RightsNotApproved,
            crate::contract::AudioReleaseBlocker::EditorialNotApproved,
            crate::contract::AudioReleaseBlocker::CandidateReviewsChanged,
            crate::contract::AudioReleaseBlocker::MetadataChanged,
            crate::contract::AudioReleaseBlocker::PublicDeliveryNotImplemented,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!audio_blocker(value, locale).trim().is_empty());
            }
        }
        for value in [
            crate::contract::PublicationBlocker::CandidateMismatch,
            crate::contract::PublicationBlocker::OwnerScopeLost,
            crate::contract::PublicationBlocker::Archived,
            crate::contract::PublicationBlocker::StaleBrief,
            crate::contract::PublicationBlocker::MetadataChanged,
            crate::contract::PublicationBlocker::UploadPermissionRevoked,
            crate::contract::PublicationBlocker::UploadPermissionExpired,
            crate::contract::PublicationBlocker::ProfileUnsupported,
            crate::contract::PublicationBlocker::ObjectMissing,
            crate::contract::PublicationBlocker::ObjectUnverified,
            crate::contract::PublicationBlocker::ObjectIntegrityMismatch,
            crate::contract::PublicationBlocker::QcNotApproved,
            crate::contract::PublicationBlocker::RightsReviewNotApproved,
            crate::contract::PublicationBlocker::EditorialNotApproved,
            crate::contract::PublicationBlocker::CandidateReviewsChanged,
            crate::contract::PublicationBlocker::RightsUnrecorded,
            crate::contract::PublicationBlocker::RightsRevoked,
            crate::contract::PublicationBlocker::RightsExpired,
            crate::contract::PublicationBlocker::RightsScopeMismatch,
            crate::contract::PublicationBlocker::ApprovalMissing,
            crate::contract::PublicationBlocker::ApprovalWithdrawn,
            crate::contract::PublicationBlocker::ApprovalStale,
            crate::contract::PublicationBlocker::DeliveryUnavailable,
            crate::contract::PublicationBlocker::DeliveryMissing,
            crate::contract::PublicationBlocker::DeliveryIntegrityMismatch,
            crate::contract::PublicationBlocker::DeliveryEvidenceExpired,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!publication_blocker(value, locale).trim().is_empty());
            }
        }
    }
    #[test]
    fn confirmation_keeps_private_identity_literal_and_exact() {
        for locale in crate::localization::Locale::supported() {
            let result = publication_confirmation(
                Key::PublishConfirm,
                "Lời hẹn {identity}",
                "candidate-{title} · cc-r1:abc",
                locale,
            );
            assert!(result.ends_with("Lời hẹn {identity}\ncandidate-{title} · cc-r1:abc"));
        }
    }
}
