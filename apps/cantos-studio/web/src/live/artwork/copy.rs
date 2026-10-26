//! Studio artwork locale resources (vi-VN / en).
use super::{Problem, Stage};
use crate::localization::generated::artwork as resources;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Key {
    NeedFile,
    NeedIntent,
    NeedAsset,
    NeedCurrent,
    NeedLocal,
    NeedRecovery,
    NeedOwner,
    IntentReceipt,
    Title,
    Private,
    Current,
    Replacement,
    Empty,
    File,
    Bounds,
    Alt,
    AltHelp,
    Provenance,
    ProvenanceHelp,
    Rights,
    RightsHelp,
    Prepare,
    Upload,
    Select,
    Clear,
    Discard,
    Refresh,
    Reconcile,
    Replay,
    Confirm,
    Cancel,
    ClearHelp,
    PendingHelp,
    Compare,
    Owner,
    UnavailablePreview,
    Operation,
    PreparedHelp,
    PreviewTitle,
    PreviewHelp,
    PreviewScenario,
    PreviewTheme,
    PreviewLocale,
    PreviewLight,
    PreviewDark,
}
pub fn text(key: Key, locale: crate::localization::Locale) -> &'static str {
    match key {
        Key::PreviewTitle => resources::text(resources::Key::PreviewTitle, locale),
        Key::PreviewHelp => resources::text(resources::Key::PreviewHelp, locale),
        Key::PreviewScenario => resources::text(resources::Key::PreviewScenario, locale),
        Key::PreviewTheme => resources::text(resources::Key::PreviewTheme, locale),
        Key::PreviewLocale => resources::text(resources::Key::PreviewLocale, locale),
        Key::PreviewLight => resources::text(resources::Key::PreviewLight, locale),
        Key::PreviewDark => resources::text(resources::Key::PreviewDark, locale),
        Key::NeedFile => resources::text(resources::Key::NeedFile, locale),
        Key::NeedIntent => resources::text(resources::Key::NeedIntent, locale),
        Key::NeedAsset => resources::text(resources::Key::NeedAsset, locale),
        Key::NeedCurrent => resources::text(resources::Key::NeedCurrent, locale),
        Key::NeedLocal => resources::text(resources::Key::NeedLocal, locale),
        Key::NeedRecovery => resources::text(resources::Key::NeedRecovery, locale),
        Key::NeedOwner => resources::text(resources::Key::NeedOwner, locale),
        Key::IntentReceipt => resources::text(resources::Key::IntentReceipt, locale),
        Key::Title => resources::text(resources::Key::Title, locale),
        Key::Private => resources::text(resources::Key::Private, locale),
        Key::Current => resources::text(resources::Key::Current, locale),
        Key::Replacement => resources::text(resources::Key::Replacement, locale),
        Key::Empty => resources::text(resources::Key::Empty, locale),
        Key::File => resources::text(resources::Key::File, locale),
        Key::Bounds => resources::text(resources::Key::Bounds, locale),
        Key::Alt => resources::text(resources::Key::Alt, locale),
        Key::AltHelp => resources::text(resources::Key::AltHelp, locale),
        Key::Provenance => resources::text(resources::Key::Provenance, locale),
        Key::ProvenanceHelp => resources::text(resources::Key::ProvenanceHelp, locale),
        Key::Rights => resources::text(resources::Key::Rights, locale),
        Key::RightsHelp => resources::text(resources::Key::RightsHelp, locale),
        Key::Prepare => resources::text(resources::Key::Prepare, locale),
        Key::Upload => resources::text(resources::Key::Upload, locale),
        Key::Select => resources::text(resources::Key::Select, locale),
        Key::Clear => resources::text(resources::Key::Clear, locale),
        Key::Discard => resources::text(resources::Key::Discard, locale),
        Key::Refresh => resources::text(resources::Key::Refresh, locale),
        Key::Reconcile => resources::text(resources::Key::Reconcile, locale),
        Key::Replay => resources::text(resources::Key::Replay, locale),
        Key::Confirm => resources::text(resources::Key::Confirm, locale),
        Key::Cancel => resources::text(resources::Key::Cancel, locale),
        Key::ClearHelp => resources::text(resources::Key::ClearHelp, locale),
        Key::PendingHelp => resources::text(resources::Key::PendingHelp, locale),
        Key::Compare => resources::text(resources::Key::Compare, locale),
        Key::Owner => resources::text(resources::Key::Owner, locale),
        Key::UnavailablePreview => resources::text(resources::Key::UnavailablePreview, locale),
        Key::Operation => resources::text(resources::Key::Operation, locale),
        Key::PreparedHelp => resources::text(resources::Key::PreparedHelp, locale),
    }
}
pub fn stage(stage: Stage, locale: crate::localization::Locale) -> &'static str {
    match stage {
        Stage::Empty => resources::text(resources::Key::StageEmpty, locale),
        Stage::Current => resources::text(resources::Key::StageCurrent, locale),
        Stage::Replacement => resources::text(resources::Key::StageReplacement, locale),
        Stage::Validating => resources::text(resources::Key::StageValidating, locale),
        Stage::Preparing => resources::text(resources::Key::StagePreparing, locale),
        Stage::Prepared => resources::text(resources::Key::StagePrepared, locale),
        Stage::Uploading => resources::text(resources::Key::StageUploading, locale),
        Stage::AwaitingAcknowledgement => {
            resources::text(resources::Key::StageAwaitingAcknowledgement, locale)
        }
        Stage::Admitted => resources::text(resources::Key::StageAdmitted, locale),
        Stage::Saved => resources::text(resources::Key::StageSaved, locale),
        Stage::Failed => resources::text(resources::Key::StageFailed, locale),
        Stage::Expired => resources::text(resources::Key::StageExpired, locale),
        Stage::Unknown => resources::text(resources::Key::StageUnknown, locale),
        Stage::Conflict => resources::text(resources::Key::StageConflict, locale),
        Stage::Loading => resources::text(resources::Key::StageLoading, locale),
        Stage::Forbidden => resources::text(resources::Key::StageForbidden, locale),
    }
}
pub fn problem(p: Problem, locale: crate::localization::Locale) -> &'static str {
    match p {
        Problem::File => resources::text(resources::Key::ProblemFile, locale),
        Problem::Dimensions => resources::text(resources::Key::ProblemDimensions, locale),
        Problem::Decode => resources::text(resources::Key::ProblemDecode, locale),
        Problem::Provenance => resources::text(resources::Key::ProblemProvenance, locale),
        Problem::Rights => resources::text(resources::Key::ProblemRights, locale),
        Problem::Alt => resources::text(resources::Key::ProblemAlt, locale),
    }
}

/// Named Rust format parameters expand once, so braces inside a file name remain literal.
pub fn media_facts(
    byte_length: u64,
    width: u32,
    height: u32,
    mime: &str,
    locale: crate::localization::Locale,
) -> String {
    crate::localization::present(
        crate::localization::generated::Message::StudioArtworkMediaFacts {
            arg_byte_length: byte_length,
            arg_width: u64::from(width),
            arg_height: u64::from(height),
            arg_mime: mime,
        }
        .render(locale),
        locale,
    )
}

pub fn file_facts(
    name: &str,
    byte_length: u64,
    width: u32,
    height: u32,
    mime: &str,
    locale: crate::localization::Locale,
) -> String {
    crate::localization::present(
        crate::localization::generated::Message::StudioArtworkFileFacts {
            arg_name: name,
            arg_byte_length: byte_length,
            arg_width: u64::from(width),
            arg_height: u64::from(height),
            arg_mime: mime,
        }
        .render(locale),
        locale,
    )
}

pub fn transferred_bytes(
    transferred: u64,
    total: u64,
    locale: crate::localization::Locale,
) -> String {
    crate::localization::present(
        crate::localization::generated::Message::StudioArtworkTransferredBytes {
            arg_transferred: transferred,
            arg_total: total,
        }
        .render(locale),
        locale,
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_artwork_label_stage_and_problem_has_both_locales() {
        for key in [
            Key::NeedFile,
            Key::NeedIntent,
            Key::NeedAsset,
            Key::NeedCurrent,
            Key::NeedLocal,
            Key::NeedRecovery,
            Key::NeedOwner,
            Key::IntentReceipt,
            Key::Title,
            Key::Private,
            Key::Current,
            Key::Replacement,
            Key::Empty,
            Key::File,
            Key::Bounds,
            Key::Alt,
            Key::AltHelp,
            Key::Provenance,
            Key::ProvenanceHelp,
            Key::Rights,
            Key::RightsHelp,
            Key::Prepare,
            Key::Upload,
            Key::Select,
            Key::Clear,
            Key::Discard,
            Key::Refresh,
            Key::Reconcile,
            Key::Replay,
            Key::Confirm,
            Key::Cancel,
            Key::ClearHelp,
            Key::PendingHelp,
            Key::Compare,
            Key::Owner,
            Key::UnavailablePreview,
            Key::Operation,
            Key::PreparedHelp,
            Key::PreviewTitle,
            Key::PreviewHelp,
            Key::PreviewScenario,
            Key::PreviewTheme,
            Key::PreviewLocale,
            Key::PreviewLight,
            Key::PreviewDark,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(!text(key, locale).trim().is_empty(), "{key:?} / {locale}");
            }
        }
        for value in [
            Stage::Empty,
            Stage::Current,
            Stage::Replacement,
            Stage::Validating,
            Stage::Preparing,
            Stage::Prepared,
            Stage::Uploading,
            Stage::AwaitingAcknowledgement,
            Stage::Admitted,
            Stage::Saved,
            Stage::Failed,
            Stage::Expired,
            Stage::Unknown,
            Stage::Conflict,
            Stage::Loading,
            Stage::Forbidden,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(
                    !stage(value, locale).trim().is_empty(),
                    "{value:?} / {locale}"
                );
            }
        }
        for value in [
            Problem::File,
            Problem::Dimensions,
            Problem::Decode,
            Problem::Provenance,
            Problem::Rights,
            Problem::Alt,
        ] {
            for locale in crate::localization::Locale::supported() {
                assert!(
                    !problem(value, locale).trim().is_empty(),
                    "{value:?} / {locale}"
                );
            }
        }
    }
}
