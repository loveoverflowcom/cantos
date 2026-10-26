//! Typed resource catalog; vi-VN and en are exhaustive, with no view literals.
use crate::editor::Status;
use crate::localization::generated::root as resources;

mod adaptation;
pub use adaptation::*;
pub mod authoring;
pub mod entry;
#[cfg(debug_assertions)]
pub mod forge;
pub mod production;
pub mod signin;
pub mod workspace;

pub use crate::localization::Locale;

#[derive(Clone, Copy)]
pub struct Copy {
    pub theme: &'static str,
    pub language: &'static str,
    pub system: &'static str,
    pub light: &'static str,
    pub dark: &'static str,
    pub unknown: &'static str,
    pub title: &'static str,
    pub mode: &'static str,
    pub token: &'static str,
    pub login: &'static str,
    pub script: &'static str,
    pub read: &'static str,
    pub draft: &'static str,
    pub save: &'static str,
    pub retry: &'static str,
    pub base: &'static str,
    pub preview: &'static str,
    pub rebase: &'static str,
    pub help: &'static str,
    pub busy: &'static str,
    pub pending: &'static str,
    pub actor: &'static str,
    pub validation: &'static str,
    pub sign_in_first: &'static str,
}

pub fn copy(locale: crate::localization::Locale) -> Copy {
    Copy {
        theme: resources::text(resources::Key::CopyTheme, locale),
        language: resources::text(resources::Key::CopyLanguage, locale),
        system: resources::text(resources::Key::CopySystem, locale),
        light: resources::text(resources::Key::CopyLight, locale),
        dark: resources::text(resources::Key::CopyDark, locale),
        unknown: resources::text(resources::Key::CopyUnknown, locale),
        title: resources::text(resources::Key::CopyTitle, locale),
        mode: resources::text(resources::Key::CopyMode, locale),
        token: resources::text(resources::Key::CopyToken, locale),
        login: resources::text(resources::Key::CopyLogin, locale),
        script: resources::text(resources::Key::CopyScript, locale),
        read: resources::text(resources::Key::CopyRead, locale),
        draft: resources::text(resources::Key::CopyDraft, locale),
        save: resources::text(resources::Key::CopySave, locale),
        retry: resources::text(resources::Key::CopyRetry, locale),
        base: resources::text(resources::Key::CopyBase, locale),
        preview: resources::text(resources::Key::CopyPreview, locale),
        rebase: resources::text(resources::Key::CopyRebase, locale),
        help: resources::text(resources::Key::CopyHelp, locale),
        busy: resources::text(resources::Key::CopyBusy, locale),
        pending: resources::text(resources::Key::CopyPending, locale),
        actor: resources::text(resources::Key::CopyActor, locale),
        validation: resources::text(resources::Key::CopyValidation, locale),
        sign_in_first: resources::text(resources::Key::CopySignInFirst, locale),
    }
}

pub fn status(status: Status, locale: crate::localization::Locale) -> &'static str {
    match status {
        Status::Dirty => resources::text(resources::Key::StatusDirty, locale),
        Status::Idle => resources::text(resources::Key::StatusIdle, locale),
        Status::Loading => resources::text(resources::Key::StatusLoading, locale),
        Status::Saving => resources::text(resources::Key::StatusSaving, locale),
        Status::Loaded => resources::text(resources::Key::StatusLoaded, locale),
        Status::Saved => resources::text(resources::Key::StatusSaved, locale),
        Status::DraftPreserved => resources::text(resources::Key::StatusDraftPreserved, locale),
        Status::SignedIn => resources::text(resources::Key::StatusSignedIn, locale),
        Status::Unauthenticated => resources::text(resources::Key::StatusUnauthenticated, locale),
        Status::NotFound => resources::text(resources::Key::StatusNotFound, locale),
        Status::Forbidden => resources::text(resources::Key::StatusForbidden, locale),
        Status::InvalidRequest => resources::text(resources::Key::StatusInvalidRequest, locale),
        Status::InvalidScript => resources::text(resources::Key::StatusInvalidScript, locale),
        Status::EvidenceUnavailable => {
            resources::text(resources::Key::StatusEvidenceUnavailable, locale)
        }
        Status::StaleRevision => resources::text(resources::Key::StatusStaleRevision, locale),
        Status::OperationReused => resources::text(resources::Key::StatusOperationReused, locale),
        Status::ProposalAlreadySubmitted => {
            resources::text(resources::Key::StatusProposalAlreadySubmitted, locale)
        }
        Status::Unavailable => resources::text(resources::Key::StatusUnavailable, locale),
        Status::CorruptRevision => resources::text(resources::Key::StatusCorruptRevision, locale),
        Status::StaleProductionInputs => {
            resources::text(resources::Key::StatusStaleProductionInputs, locale)
        }
        Status::ProductionBlocked => {
            resources::text(resources::Key::StatusProductionBlocked, locale)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{copy, status, Locale};
    use crate::editor::Status;

    #[test]
    fn locale_registry_resolves_explicit_aliases_and_regional_lookup() {
        assert_eq!(Locale::default().lang(), "vi-VN");
        assert_eq!(
            Locale::supported().map(Locale::lang).collect::<Vec<_>>(),
            ["vi-VN", "en"]
        );
        for (tag, expected) in [
            ("vi", "vi-VN"),
            ("vi-VN", "vi-VN"),
            ("EN", "en"),
            ("en-GB", "en"),
        ] {
            assert_eq!(Locale::from_lang(tag).map(Locale::lang), Some(expected));
        }
        for unsupported in ["", "fr", "zh-Hant", "ja"] {
            assert_eq!(Locale::from_lang(unsupported), None);
        }
        assert_eq!(copy(Locale::default()).theme, "Giao diện");
    }
    #[test]
    fn every_async_and_error_state_has_both_locales() {
        for state in [
            Status::Idle,
            Status::Dirty,
            Status::Loading,
            Status::Saving,
            Status::Loaded,
            Status::Saved,
            Status::DraftPreserved,
            Status::SignedIn,
            Status::Unauthenticated,
            Status::NotFound,
            Status::Forbidden,
            Status::InvalidRequest,
            Status::InvalidScript,
            Status::EvidenceUnavailable,
            Status::StaleRevision,
            Status::OperationReused,
            Status::ProposalAlreadySubmitted,
            Status::Unavailable,
            Status::CorruptRevision,
        ] {
            assert!(!status(state, crate::localization::Locale::Vietnamese).is_empty());
            assert!(!status(state, crate::localization::Locale::English).is_empty());
        }
        assert!(copy(crate::localization::Locale::Vietnamese)
            .mode
            .contains("Live"));
        assert!(copy(crate::localization::Locale::English)
            .mode
            .contains("no mock"));
    }
}

#[derive(Clone, Copy)]
pub struct ImportCopy {
    pub title: &'static str,
    pub help: &'static str,
    pub file: &'static str,
    pub format: &'static str,
    pub txt: &'static str,
    pub markdown: &'static str,
    pub docx: &'static str,
    pub script_ir: &'static str,
    pub reference: &'static str,
    pub rights_holder: &'static str,
    pub permission_evidence: &'static str,
    pub usage_scope: &'static str,
    pub rights_help: &'static str,
    pub save: &'static str,
    pub retry: &'static str,
    pub pending: &'static str,
    pub required: &'static str,
    pub source_id: &'static str,
    pub open: &'static str,
    pub stored: &'static str,
    pub original: &'static str,
    pub download: &'static str,
    pub binary: &'static str,
    pub parsed: &'static str,
    pub parsed_help: &'static str,
    pub warnings: &'static str,
    pub no_warnings: &'static str,
    pub failed: &'static str,
    pub speaker: &'static str,
    pub scene: &'static str,
    pub cue: &'static str,
    pub unknown: &'static str,
    pub script_json: &'static str,
    pub checksum: &'static str,
    pub operation: &'static str,
    pub extractor: &'static str,
    pub recorded_at: &'static str,
    pub imported_by: &'static str,
    pub file_name: &'static str,
    pub byte_len: &'static str,
    pub block: &'static str,
    pub support_code: &'static str,
    pub absent: &'static str,
    pub byte_offset: &'static str,
    pub previous_blocks: &'static str,
    pub next_blocks: &'static str,
}

pub fn import_copy(locale: crate::localization::Locale) -> ImportCopy {
    ImportCopy {
        title: resources::text(resources::Key::ImportCopyTitle, locale),
        help: resources::text(resources::Key::ImportCopyHelp, locale),
        file: resources::text(resources::Key::ImportCopyFile, locale),
        format: resources::text(resources::Key::ImportCopyFormat, locale),
        txt: resources::text(resources::Key::ImportCopyTxt, locale),
        markdown: resources::text(resources::Key::ImportCopyMarkdown, locale),
        docx: resources::text(resources::Key::ImportCopyDocx, locale),
        script_ir: resources::text(resources::Key::ImportCopyScriptIr, locale),
        reference: resources::text(resources::Key::ImportCopyReference, locale),
        rights_holder: resources::text(resources::Key::ImportCopyRightsHolder, locale),
        permission_evidence: resources::text(resources::Key::ImportCopyPermissionEvidence, locale),
        usage_scope: resources::text(resources::Key::ImportCopyUsageScope, locale),
        rights_help: resources::text(resources::Key::ImportCopyRightsHelp, locale),
        save: resources::text(resources::Key::ImportCopySave, locale),
        retry: resources::text(resources::Key::ImportCopyRetry, locale),
        pending: resources::text(resources::Key::ImportCopyPending, locale),
        required: resources::text(resources::Key::ImportCopyRequired, locale),
        source_id: resources::text(resources::Key::ImportCopySourceId, locale),
        open: resources::text(resources::Key::ImportCopyOpen, locale),
        stored: resources::text(resources::Key::ImportCopyStored, locale),
        original: resources::text(resources::Key::ImportCopyOriginal, locale),
        download: resources::text(resources::Key::ImportCopyDownload, locale),
        binary: resources::text(resources::Key::ImportCopyBinary, locale),
        parsed: resources::text(resources::Key::ImportCopyParsed, locale),
        parsed_help: resources::text(resources::Key::ImportCopyParsedHelp, locale),
        warnings: resources::text(resources::Key::ImportCopyWarnings, locale),
        no_warnings: resources::text(resources::Key::ImportCopyNoWarnings, locale),
        failed: resources::text(resources::Key::ImportCopyFailed, locale),
        speaker: resources::text(resources::Key::ImportCopySpeaker, locale),
        scene: resources::text(resources::Key::ImportCopyScene, locale),
        cue: resources::text(resources::Key::ImportCopyCue, locale),
        unknown: resources::text(resources::Key::ImportCopyUnknown, locale),
        script_json: resources::text(resources::Key::ImportCopyScriptJson, locale),
        checksum: resources::text(resources::Key::ImportCopyChecksum, locale),
        operation: resources::text(resources::Key::ImportCopyOperation, locale),
        extractor: resources::text(resources::Key::ImportCopyExtractor, locale),
        recorded_at: resources::text(resources::Key::ImportCopyRecordedAt, locale),
        imported_by: resources::text(resources::Key::ImportCopyImportedBy, locale),
        file_name: resources::text(resources::Key::ImportCopyFileName, locale),
        byte_len: resources::text(resources::Key::ImportCopyByteLen, locale),
        block: resources::text(resources::Key::ImportCopyBlock, locale),
        support_code: resources::text(resources::Key::ImportCopySupportCode, locale),
        absent: resources::text(resources::Key::ImportCopyAbsent, locale),
        byte_offset: resources::text(resources::Key::ImportCopyByteOffset, locale),
        previous_blocks: resources::text(resources::Key::ImportCopyPreviousBlocks, locale),
        next_blocks: resources::text(resources::Key::ImportCopyNextBlocks, locale),
    }
}

pub fn import_status(
    value: &crate::import::ImportStatus,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::ErrorCode;
    use crate::import::ImportStatus;
    match value {
        ImportStatus::Error(ErrorCode::StaleProductionInputs) => {
            resources::text(resources::Key::ImportStatusStaleProductionInputs, locale)
        }
        ImportStatus::Error(ErrorCode::ProductionBlocked) => {
            resources::text(resources::Key::ImportStatusProductionBlocked, locale)
        }
        ImportStatus::Idle => resources::text(resources::Key::ImportStatusIdle, locale),
        ImportStatus::ReadingFile => {
            resources::text(resources::Key::ImportStatusReadingFile, locale)
        }
        ImportStatus::Ready => resources::text(resources::Key::ImportStatusReady, locale),
        ImportStatus::Saving => resources::text(resources::Key::ImportStatusSaving, locale),
        ImportStatus::Loading => resources::text(resources::Key::ImportStatusLoading, locale),
        ImportStatus::Stored => resources::text(resources::Key::ImportStatusStored, locale),
        ImportStatus::FileTooLarge => {
            resources::text(resources::Key::ImportStatusFileTooLarge, locale)
        }
        ImportStatus::FileReadFailed => {
            resources::text(resources::Key::ImportStatusFileReadFailed, locale)
        }
        ImportStatus::Error(ErrorCode::Unauthenticated) => {
            resources::text(resources::Key::ImportStatusUnauthenticated, locale)
        }
        ImportStatus::Error(ErrorCode::Forbidden) => {
            resources::text(resources::Key::ImportStatusForbidden, locale)
        }
        ImportStatus::Error(ErrorCode::NotFound) => {
            resources::text(resources::Key::ImportStatusNotFound, locale)
        }
        ImportStatus::Error(ErrorCode::InvalidRequest) => {
            resources::text(resources::Key::ImportStatusInvalidRequest, locale)
        }
        ImportStatus::Error(ErrorCode::OperationReused) => {
            resources::text(resources::Key::ImportStatusOperationReused, locale)
        }
        ImportStatus::Error(ErrorCode::Unavailable) => {
            resources::text(resources::Key::ImportStatusUnavailable, locale)
        }
        ImportStatus::Error(ErrorCode::CorruptRevision) => {
            resources::text(resources::Key::ImportStatusCorruptRevision, locale)
        }
        ImportStatus::Error(ErrorCode::InvalidScript) => {
            resources::text(resources::Key::ImportStatusInvalidScript, locale)
        }
        ImportStatus::Error(ErrorCode::EvidenceUnavailable) => {
            resources::text(resources::Key::ImportStatusEvidenceUnavailable, locale)
        }
        ImportStatus::Error(ErrorCode::StaleRevision) => {
            resources::text(resources::Key::ImportStatusStaleRevision, locale)
        }
        ImportStatus::Error(ErrorCode::ProposalAlreadySubmitted) => {
            status(Status::ProposalAlreadySubmitted, locale)
        }
        ImportStatus::Error(ErrorCode::InvalidCatalog | ErrorCode::AudioRejected) => {
            status(Status::InvalidRequest, locale)
        }
        ImportStatus::Error(ErrorCode::CatalogConflict | ErrorCode::StaleCursor) => {
            status(Status::StaleRevision, locale)
        }
    }
}

pub fn import_format(
    format: crate::contract::ImportFormat,
    locale: crate::localization::Locale,
) -> &'static str {
    let copy = import_copy(locale);
    match format {
        crate::contract::ImportFormat::Txt => copy.txt,
        crate::contract::ImportFormat::Markdown => copy.markdown,
        crate::contract::ImportFormat::Docx => copy.docx,
        crate::contract::ImportFormat::ScriptIr => copy.script_ir,
    }
}

pub fn import_kind(
    kind: crate::contract::ImportBlockKind,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::ImportBlockKind;
    match kind {
        ImportBlockKind::Paragraph => resources::text(resources::Key::ImportKindParagraph, locale),
        ImportBlockKind::Dialogue => resources::text(resources::Key::ImportKindDialogue, locale),
        ImportBlockKind::Narration => resources::text(resources::Key::ImportKindNarration, locale),
        ImportBlockKind::SceneCue => resources::text(resources::Key::ImportKindSceneCue, locale),
        ImportBlockKind::SoundCue => resources::text(resources::Key::ImportKindSoundCue, locale),
    }
}

pub fn import_diagnostic(code: &str, locale: crate::localization::Locale) -> &'static str {
    match code {
    "utf8_bom_removed" => resources::text(resources::Key::ImportDiagnosticUtf8BomRemoved, locale),
    "line_endings_normalized" => resources::text(resources::Key::ImportDiagnosticLineEndingsNormalized, locale),
    "unicode_nfc_normalized" => resources::text(resources::Key::ImportDiagnosticUnicodeNfcNormalized, locale),
    "paragraph_edge_whitespace_trimmed" => resources::text(resources::Key::ImportDiagnosticParagraphEdgeWhitespaceTrimmed, locale),
    "markdown_list_or_dialogue_ambiguous" => resources::text(resources::Key::ImportDiagnosticMarkdownListOrDialogueAmbiguous, locale),
    "docx_images_omitted" => resources::text(resources::Key::ImportDiagnosticDocxImagesOmitted, locale),
    "docx_annotations_omitted" => resources::text(resources::Key::ImportDiagnosticDocxAnnotationsOmitted, locale),
    "docx_headers_footers_omitted" => resources::text(resources::Key::ImportDiagnosticDocxHeadersFootersOmitted, locale),
    "docx_soft_breaks_segmented" => resources::text(resources::Key::ImportDiagnosticDocxSoftBreaksSegmented, locale),
    "xml_processing_instruction_ignored" => resources::text(resources::Key::ImportDiagnosticXmlProcessingInstructionIgnored, locale),
    "docx_tables_flattened" => resources::text(resources::Key::ImportDiagnosticDocxTablesFlattened, locale),
    "docx_textboxes_flattened" => resources::text(resources::Key::ImportDiagnosticDocxTextboxesFlattened, locale),
    "docx_field_instructions_omitted" => resources::text(resources::Key::ImportDiagnosticDocxFieldInstructionsOmitted, locale),
    "docx_external_content_omitted" => resources::text(resources::Key::ImportDiagnosticDocxExternalContentOmitted, locale),
    "docx_symbols_omitted" => resources::text(resources::Key::ImportDiagnosticDocxSymbolsOmitted, locale),
    "script_ir_full_document_preserved" => resources::text(resources::Key::ImportDiagnosticScriptIrFullDocumentPreserved, locale),
    "speaker_label_requires_review" => resources::text(resources::Key::ImportDiagnosticSpeakerLabelRequiresReview, locale),
    "speaker_unknown" => resources::text(resources::Key::ImportDiagnosticSpeakerUnknown, locale),
    "heading_scene_suggestion" => resources::text(resources::Key::ImportDiagnosticHeadingSceneSuggestion, locale),
    "ambiguous_bracket_cue" => resources::text(resources::Key::ImportDiagnosticAmbiguousBracketCue, locale),
    "markdown_markup_preserved" => resources::text(resources::Key::ImportDiagnosticMarkdownMarkupPreserved, locale),
    "docx_formatting_omitted" => resources::text(resources::Key::ImportDiagnosticDocxFormattingOmitted, locale),
    "external_relationships_ignored" => resources::text(resources::Key::ImportDiagnosticExternalRelationshipsIgnored, locale),
    "macros_ignored" => resources::text(resources::Key::ImportDiagnosticMacrosIgnored, locale),
    "embedded_objects_ignored" => resources::text(resources::Key::ImportDiagnosticEmbeddedObjectsIgnored, locale),
    "invalid_utf8" => resources::text(resources::Key::ImportDiagnosticInvalidUtf8, locale),
    "empty_source" => resources::text(resources::Key::ImportDiagnosticEmptySource, locale),
    "source_too_large" | "decoded_text_limit" | "archive_entry_limit" | "archive_expansion_limit" | "archive_ratio_limit" => resources::text(resources::Key::ImportDiagnosticSourceTooLargeDecodedTextLimitArchiveEntryLimitArchiveExpansionLimitArchiveRatioLimit, locale),
    "format_mismatch" | "legacy_or_encrypted_word" => resources::text(resources::Key::ImportDiagnosticFormatMismatchLegacyOrEncryptedWord, locale),
    "docx_tracked_changes_require_choice" => resources::text(resources::Key::ImportDiagnosticDocxTrackedChangesRequireChoice, locale),
    "docx_legacy_encoding_requires_choice" | "unsupported_xml_encoding" => resources::text(resources::Key::ImportDiagnosticDocxLegacyEncodingRequiresChoiceUnsupportedXmlEncoding, locale),
    "unsupported_script_ir_version" | "missing_script_ir_version" => resources::text(resources::Key::ImportDiagnosticUnsupportedScriptIrVersionMissingScriptIrVersion, locale),
    "invalid_script_ir" | "script_ir_shape_invalid" | "script_ir_semantics_invalid" => resources::text(resources::Key::ImportDiagnosticInvalidScriptIrScriptIrShapeInvalidScriptIrSemanticsInvalid, locale),
    "malformed_archive" | "malformed_or_encrypted_archive" | "duplicate_archive_entry" | "unsafe_archive_path" | "malformed_xml" | "xml_doctype_forbidden" | "unsupported_control_character" => resources::text(resources::Key::ImportDiagnosticMalformedArchiveMalformedOrEncryptedArchiveDuplicateArchiveEntryUnsafeArchivePathMalformedXmlXmlDoctypeForbiddenUnsupportedControlCharacter, locale),
    "archive_zip64_unsupported" | "archive_multidisk_unsupported" => resources::text(resources::Key::ImportDiagnosticArchiveZip64UnsupportedArchiveMultidiskUnsupported, locale),
    _ => resources::text(resources::Key::ImportDiagnostic, locale),
}
}

pub fn import_cue(
    kind: crate::contract::ImportCueKind,
    locale: crate::localization::Locale,
) -> &'static str {
    use crate::contract::ImportCueKind;
    match kind {
        ImportCueKind::Ambience => resources::text(resources::Key::ImportCueAmbience, locale),
        ImportCueKind::Music => resources::text(resources::Key::ImportCueMusic, locale),
        ImportCueKind::Sfx => resources::text(resources::Key::ImportCueSfx, locale),
    }
}

#[cfg(test)]
mod import_tests {
    use super::{import_copy, import_diagnostic, import_kind, import_status};
    use crate::contract::{ErrorCode, ImportBlockKind};
    use crate::import::ImportStatus;

    #[test]
    fn import_states_and_identity_ambiguity_have_both_locales() {
        let mut states = vec![
            ImportStatus::Idle,
            ImportStatus::ReadingFile,
            ImportStatus::Ready,
            ImportStatus::Saving,
            ImportStatus::Loading,
            ImportStatus::Stored,
            ImportStatus::FileTooLarge,
            ImportStatus::FileReadFailed,
        ];
        for code in [
            ErrorCode::Unauthenticated,
            ErrorCode::NotFound,
            ErrorCode::Forbidden,
            ErrorCode::InvalidRequest,
            ErrorCode::InvalidCatalog,
            ErrorCode::AudioRejected,
            ErrorCode::CatalogConflict,
            ErrorCode::StaleCursor,
            ErrorCode::InvalidScript,
            ErrorCode::EvidenceUnavailable,
            ErrorCode::StaleRevision,
            ErrorCode::OperationReused,
            ErrorCode::ProposalAlreadySubmitted,
            ErrorCode::Unavailable,
            ErrorCode::CorruptRevision,
        ] {
            states.push(ImportStatus::Error(code));
        }
        for state in states {
            assert!(!import_status(&state, crate::localization::Locale::Vietnamese).is_empty());
            assert!(!import_status(&state, crate::localization::Locale::English).is_empty());
            assert_ne!(
                import_status(&state, crate::localization::Locale::Vietnamese),
                import_status(&state, crate::localization::Locale::English)
            );
        }
        for kind in [
            ImportBlockKind::Paragraph,
            ImportBlockKind::Dialogue,
            ImportBlockKind::Narration,
            ImportBlockKind::SceneCue,
            ImportBlockKind::SoundCue,
        ] {
            assert!(!import_kind(kind, crate::localization::Locale::Vietnamese).is_empty());
            assert!(!import_kind(kind, crate::localization::Locale::English).is_empty());
        }
        assert!(import_copy(crate::localization::Locale::English)
            .rights_help
            .contains("remain unknown"));
        assert!(import_copy(crate::localization::Locale::Vietnamese)
            .rights_help
            .contains("chưa xác định"));
        assert!(import_diagnostic(
            "speaker_label_requires_review",
            crate::localization::Locale::English
        )
        .contains("not a resolved"));
        assert!(
            import_diagnostic("future-diagnostic", crate::localization::Locale::English)
                .contains("operator")
        );
        assert!(
            import_diagnostic("future-diagnostic", crate::localization::Locale::Vietnamese)
                .contains("người vận hành")
        );
    }
}

#[cfg(test)]
mod import_warning_tests {
    use super::import_diagnostic;

    #[test]
    fn every_implemented_extraction_warning_has_specific_vietnamese_and_english_copy() {
        for code in [
            "utf8_bom_removed",
            "line_endings_normalized",
            "unicode_nfc_normalized",
            "paragraph_edge_whitespace_trimmed",
            "heading_scene_suggestion",
            "speaker_label_requires_review",
            "speaker_unknown",
            "markdown_list_or_dialogue_ambiguous",
            "ambiguous_bracket_cue",
            "markdown_markup_preserved",
            "macros_ignored",
            "embedded_objects_ignored",
            "docx_images_omitted",
            "docx_annotations_omitted",
            "docx_headers_footers_omitted",
            "docx_formatting_omitted",
            "docx_soft_breaks_segmented",
            "external_relationships_ignored",
            "xml_processing_instruction_ignored",
            "docx_tables_flattened",
            "docx_textboxes_flattened",
            "docx_field_instructions_omitted",
            "docx_external_content_omitted",
            "docx_symbols_omitted",
            "script_ir_full_document_preserved",
        ] {
            for locale in crate::localization::Locale::supported() {
                assert_ne!(
                    import_diagnostic(code, locale),
                    import_diagnostic("unknown-future-code", locale),
                    "{code}"
                );
            }
        }
    }
}

pub fn import_block_range(
    page: usize,
    total: usize,
    locale: crate::localization::Locale,
) -> String {
    let range = crate::import::page_bounds(page, total);
    let start = if total == 0 { 0 } else { range.start + 1 };
    crate::localization::present(
        crate::localization::generated::Message::StudioRootImportBlockRange {
            arg_start: start as u64,
            arg_end: range.end as u64,
            arg_total: total as u64,
        }
        .render(locale),
        locale,
    )
}

pub fn import_field(path: &str, locale: crate::localization::Locale) -> &'static str {
    let copy = import_copy(locale);
    match path {
        "/metadata/file_name" => copy.file_name,
        "/metadata/reference" => copy.reference,
        "/metadata/rights_holder" => copy.rights_holder,
        "/metadata/permission_evidence" => copy.permission_evidence,
        "/metadata/usage_scope" => copy.usage_scope,
        "/metadata/operation_id" => copy.operation,
        "/original_bytes" => copy.file,
        _ => copy.support_code,
    }
}

pub fn import_field_rule(rule: &str, locale: crate::localization::Locale) -> &'static str {
    match rule {
        "nonblank" => resources::text(resources::Key::ImportFieldRuleNonblank, locale),
        "byte_length" => resources::text(resources::Key::ImportFieldRuleByteLength, locale),
        "control_character" => {
            resources::text(resources::Key::ImportFieldRuleControlCharacter, locale)
        }
        "canonical_uuid" => resources::text(resources::Key::ImportFieldRuleCanonicalUuid, locale),
        _ => resources::text(resources::Key::ImportFieldRule, locale),
    }
}

#[cfg(test)]
mod import_field_tests {
    use super::{import_field, import_field_rule};

    #[test]
    fn safe_metadata_diagnostics_have_field_names_and_recovery_copy_in_both_locales() {
        for path in [
            "/metadata/file_name",
            "/metadata/reference",
            "/metadata/rights_holder",
            "/metadata/permission_evidence",
            "/metadata/usage_scope",
            "/metadata/operation_id",
            "/original_bytes",
        ] {
            for locale in crate::localization::Locale::supported() {
                assert_ne!(
                    import_field(path, locale),
                    import_field("future-field", locale)
                );
            }
        }
        for rule in [
            "nonblank",
            "byte_length",
            "control_character",
            "canonical_uuid",
        ] {
            for locale in crate::localization::Locale::supported() {
                assert_ne!(
                    import_field_rule(rule, locale),
                    import_field_rule("future-rule", locale)
                );
            }
        }
    }
}
