//! Import presentation state. Original bytes and retry identity stay separate from extraction.
use cantos_contracts::{
    ApiError, ErrorCode, FieldIssue, ImportFormat, ImportMetadata, ImportRequest, ImportResponse,
};

/// A browser preflight only; the backend independently enforces its intake limits.
pub const MAX_FILE_BYTES: usize = 1024 * 1024;
pub const BLOCKS_PER_PAGE: usize = 100;

pub fn page_bounds(page: usize, total: usize) -> std::ops::Range<usize> {
    let last_page = total.saturating_sub(1) / BLOCKS_PER_PAGE;
    let start = page.min(last_page) * BLOCKS_PER_PAGE;
    start..(start + BLOCKS_PER_PAGE).min(total)
}

pub fn warning_on_page(block: Option<u32>, page: usize, total: usize) -> bool {
    block.is_none_or(|block| page_bounds(page, total).contains(&(block as usize)))
}

pub fn is_import_source_id(id: &str) -> bool {
    id.len() == 36
        && id.strip_prefix("src_").is_some_and(|suffix| {
            suffix
                .bytes()
                .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportStatus {
    Idle,
    ReadingFile,
    Ready,
    Saving,
    Loading,
    Stored,
    FileTooLarge,
    FileReadFailed,
    Error(ErrorCode),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImportIntent {
    pub actor: String,
    pub request: ImportRequest,
}

#[derive(Clone, Debug)]
pub struct ImportEditor {
    pub actor: String,
    pub metadata: ImportMetadata,
    pub bytes: Option<Vec<u8>>,
    pub source_id: String,
    pub stored: Option<ImportResponse>,
    pub issues: Vec<FieldIssue>,
    pub pending: Option<ImportIntent>,
    pub ambiguous: bool,
    pub busy: bool,
    pub ticket: u64,
    pub status: ImportStatus,
}

impl Default for ImportEditor {
    fn default() -> Self {
        Self {
            actor: String::new(),
            metadata: ImportMetadata {
                operation_id: String::new(),
                file_name: String::new(),
                format: ImportFormat::Txt,
                reference: String::new(),
                rights_holder: None,
                permission_evidence: None,
                usage_scope: None,
            },
            bytes: None,
            source_id: String::new(),
            stored: None,
            issues: vec![],
            pending: None,
            ambiguous: false,
            busy: false,
            ticket: 0,
            status: ImportStatus::Idle,
        }
    }
}

impl ImportEditor {
    pub fn blocked(&self) -> bool {
        self.busy || self.pending.is_some()
    }

    /// Account changes hide the prior account's response and invalidate outstanding reads.
    /// An ambiguous mutation is retained so only its submitting actor can reconcile it.
    pub fn signed_in_as(&self, actor: String) -> Self {
        if self.actor == actor {
            return self.clone();
        }
        let mut next = self.clone();
        next.actor = actor;
        next.stored = None;
        next.issues.clear();
        next.ticket = self.ticket.saturating_add(1);
        next.busy = false;
        next.status = if next.pending.is_some() {
            ImportStatus::Error(ErrorCode::Forbidden)
        } else if next.bytes.is_some() {
            ImportStatus::Ready
        } else {
            ImportStatus::Idle
        };
        next
    }

    pub fn edit_metadata(&self, edit: impl FnOnce(&mut ImportMetadata)) -> Self {
        if self.blocked() {
            return self.clone();
        }
        let mut next = self.clone();
        edit(&mut next.metadata);
        next.issues.clear();
        next.metadata.operation_id.clear();
        next.status = if next.bytes.is_some() {
            ImportStatus::Ready
        } else {
            ImportStatus::Idle
        };
        next
    }

    pub fn start_file(&self, file_name: String, size: u64) -> Option<Self> {
        if self.blocked() {
            return None;
        }
        let mut next = self.clone();
        next.ticket = self.ticket.checked_add(1)?;
        next.metadata.file_name = file_name;
        next.metadata.operation_id.clear();
        next.bytes = None;
        next.stored = None;
        next.issues.clear();
        if size == 0 || size > MAX_FILE_BYTES as u64 {
            next.status = ImportStatus::FileTooLarge;
        } else {
            next.busy = true;
            next.status = ImportStatus::ReadingFile;
        }
        Some(next)
    }

    pub fn file_loaded(&self, ticket: u64, bytes: Option<Vec<u8>>) -> Self {
        if ticket != self.ticket || self.status != ImportStatus::ReadingFile {
            return self.clone();
        }
        let mut next = self.clone();
        next.busy = false;
        match bytes {
            Some(bytes) if !bytes.is_empty() && bytes.len() <= MAX_FILE_BYTES => {
                next.bytes = Some(bytes);
                next.status = ImportStatus::Ready;
            }
            Some(_) => next.status = ImportStatus::FileTooLarge,
            None => next.status = ImportStatus::FileReadFailed,
        }
        next
    }

    pub fn start_import(&self, operation_id: String) -> Option<(Self, ImportIntent)> {
        if self.blocked() || self.actor.is_empty() || self.metadata.reference.trim().is_empty() {
            return None;
        }
        let bytes = self.bytes.as_ref()?;
        let mut metadata = self.metadata.clone();
        metadata.operation_id = operation_id;
        let intent = ImportIntent {
            actor: self.actor.clone(),
            request: ImportRequest {
                metadata,
                original_bytes: bytes.clone(),
            },
        };
        let mut next = self.clone();
        next.busy = true;
        next.pending = Some(intent.clone());
        next.issues.clear();
        next.status = ImportStatus::Saving;
        Some((next, intent))
    }

    pub fn retry(&self) -> Option<(Self, ImportIntent)> {
        let intent = self.pending.as_ref()?;
        if self.busy || self.actor.is_empty() || intent.actor != self.actor {
            return None;
        }
        let mut next = self.clone();
        next.busy = true;
        next.issues.clear();
        next.status = ImportStatus::Saving;
        Some((next, intent.clone()))
    }

    pub fn imported(&self, intent: &ImportIntent, response: ImportResponse) -> Self {
        if self.pending.as_ref() != Some(intent) {
            return self.clone();
        }
        if response.imported_by != intent.actor {
            let mut next = self.clone();
            next.busy = false;
            next.ambiguous = true;
            next.status = ImportStatus::Error(ErrorCode::Forbidden);
            return next;
        }
        if response.metadata != intent.request.metadata
            || response.byte_len != intent.request.original_bytes.len() as u64
        {
            let mut next = self.clone();
            next.busy = false;
            next.ambiguous = true;
            next.status = ImportStatus::Error(ErrorCode::CorruptRevision);
            return next;
        }
        let mut next = self.clone();
        next.pending = None;
        next.ambiguous = false;
        next.busy = false;
        if self.actor == intent.actor {
            next.source_id = response.id.clone();
            next.stored = Some(response);
            next.status = ImportStatus::Stored;
        }
        next
    }

    pub fn failed(&self, error: &ApiError) -> Self {
        let mut next = self.clone();
        next.busy = false;
        next.issues = error.issues.clone();
        next.status = ImportStatus::Error(error.code.clone());
        if matches!(
            error.code,
            ErrorCode::Unavailable | ErrorCode::CorruptRevision
        ) {
            next.ambiguous = next.pending.is_some();
        } else if self.ambiguous
            && matches!(
                error.code,
                ErrorCode::Unauthenticated | ErrorCode::Forbidden | ErrorCode::NotFound
            )
        {
            // An authorization failure cannot prove that the earlier request rolled back.
        } else {
            next.pending = None;
            next.ambiguous = false;
        }
        next
    }

    pub fn select_source(&self, source_id: String) -> Self {
        if self.blocked() || self.source_id == source_id {
            return self.clone();
        }
        let mut next = self.clone();
        next.source_id = source_id;
        next.stored = None;
        next.issues.clear();
        next.ticket = self.ticket.saturating_add(1);
        next.status = if next.bytes.is_some() {
            ImportStatus::Ready
        } else {
            ImportStatus::Idle
        };
        next
    }

    pub fn start_read(&self) -> Option<Self> {
        if self.blocked() || self.actor.is_empty() {
            return None;
        }
        let mut next = self.clone();
        next.ticket = self.ticket.checked_add(1)?;
        next.busy = true;
        next.stored = None;
        next.issues.clear();
        next.status = ImportStatus::Loading;
        Some(next)
    }

    pub fn loaded(&self, ticket: u64, actor: &str, response: ImportResponse) -> Self {
        if ticket != self.ticket || actor != self.actor || response.id != self.source_id {
            return self.clone();
        }
        let mut next = self.clone();
        next.busy = false;
        if response.imported_by != actor {
            next.stored = None;
            next.status = ImportStatus::Error(ErrorCode::Forbidden);
            return next;
        }
        next.stored = Some(response);
        next.status = ImportStatus::Stored;
        next
    }

    pub fn read_failed(&self, ticket: u64, actor: &str, error: &ApiError) -> Self {
        if ticket != self.ticket || actor != self.actor {
            return self.clone();
        }
        self.failed(error)
    }
}

pub fn optional_metadata(text: String) -> Option<String> {
    if text.trim().is_empty() {
        None
    } else {
        Some(text)
    }
}

#[cfg(target_arch = "wasm32")]
mod browser;
#[cfg(target_arch = "wasm32")]
pub(crate) use browser::ExtractionPreview;
#[cfg(target_arch = "wasm32")]
pub use browser::ManuscriptImport;

#[cfg(test)]
mod tests {
    use super::{ImportEditor, ImportStatus, MAX_FILE_BYTES};
    use cantos_contracts::{ApiError, ErrorCode, Extraction, ImportOutcome, ImportResponse};

    fn ready() -> ImportEditor {
        let actor = ImportEditor::default().signed_in_as("alice".into());
        let reading = actor
            .start_file("Ánh đèn cuối sân khấu.txt".into(), 20)
            .unwrap();
        let loaded = reading.file_loaded(
            reading.ticket,
            Some("Người dẫn chuyện: Xin chào!\r\n".as_bytes().to_vec()),
        );
        loaded.edit_metadata(|metadata| {
            metadata.reference = "Synthetic chapter written for testing".into()
        })
    }

    fn error(code: ErrorCode) -> ApiError {
        ApiError {
            code,
            current_revision: None,
            issues: vec![],
        }
    }

    fn response(intent: &super::ImportIntent) -> ImportResponse {
        ImportResponse {
            id: "src_00000000000040008000000000000001".into(),
            imported_by: intent.actor.clone(),
            recorded_at: "2026-10-10T10:00:00Z".into(),
            sha256: "opaque-sha256".into(),
            byte_len: intent.request.original_bytes.len() as u64,
            metadata: intent.request.metadata.clone(),
            outcome: ImportOutcome::Parsed {
                extraction: Extraction {
                    extractor_version: "test".into(),
                    blocks: vec![],
                    warnings: vec![],
                    script_json: None,
                },
            },
            original_text: Some(String::from_utf8(intent.request.original_bytes.clone()).unwrap()),
        }
    }

    #[test]
    fn timeout_retry_preserves_exact_bytes_actor_and_operation() {
        let prepared = ready();
        let original = prepared.bytes.clone();
        let (saving, intent) = prepared.start_import("immutable-operation".into()).unwrap();
        let unavailable = saving.failed(&error(ErrorCode::Unavailable));
        let (retrying, retried) = unavailable.retry().unwrap();
        assert_eq!(retried, intent);
        assert_eq!(retrying.bytes, original);
        let stored = retrying.imported(&retried, response(&intent));
        assert_eq!(stored.bytes, original);
        assert_eq!(stored.status, ImportStatus::Stored);
        assert_eq!(stored.pending, None);
    }

    #[test]
    fn expiration_after_ambiguous_import_cannot_discard_retry_identity() {
        let (saving, intent) = ready().start_import("operation".into()).unwrap();
        let expired = saving
            .failed(&error(ErrorCode::Unavailable))
            .failed(&error(ErrorCode::Unauthenticated));
        assert_eq!(expired.pending, Some(intent.clone()));
        assert!(expired.ambiguous);
        let other = expired.signed_in_as("bob".into());
        assert!(other.retry().is_none());
        assert!(other.stored.is_none());
        assert_eq!(
            other.signed_in_as("alice".into()).retry().unwrap().1,
            intent
        );
    }

    #[test]
    fn metadata_changes_cannot_mutate_an_unresolved_snapshot() {
        let (saving, intent) = ready().start_import("operation".into()).unwrap();
        let unavailable = saving.failed(&error(ErrorCode::Unavailable));
        let edited =
            unavailable.edit_metadata(|metadata| metadata.reference = "other source".into());
        assert_eq!(edited.metadata.reference, saving.metadata.reference);
        assert_eq!(edited.pending, Some(intent));
        assert!(edited.start_file("other.txt".into(), 5).is_none());
        assert!(edited.start_import("other-operation".into()).is_none());
    }

    #[test]
    fn definitive_failure_keeps_file_and_allows_a_fresh_request() {
        let prepared = ready();
        let (saving, _) = prepared.start_import("first-operation".into()).unwrap();
        let rejected = saving.failed(&error(ErrorCode::InvalidRequest));
        assert_eq!(rejected.bytes, prepared.bytes);
        assert_eq!(rejected.pending, None);
        let edited =
            rejected.edit_metadata(|metadata| metadata.reference = "Corrected reference".into());
        let (_, next) = edited.start_import("new-operation".into()).unwrap();
        assert_eq!(next.request.metadata.operation_id, "new-operation");
        assert_eq!(next.request.metadata.reference, "Corrected reference");
    }

    #[test]
    fn oversized_file_is_rejected_before_a_read_can_start() {
        let rejected = ImportEditor::default()
            .start_file("too-large.docx".into(), MAX_FILE_BYTES as u64 + 1)
            .unwrap();
        assert_eq!(rejected.status, ImportStatus::FileTooLarge);
        assert!(!rejected.busy);
        assert_eq!(rejected.bytes, None);
        let empty = ImportEditor::default()
            .start_file("empty.txt".into(), 0)
            .unwrap();
        assert_eq!(empty.status, ImportStatus::FileTooLarge);
    }

    #[test]
    fn late_file_or_read_response_after_account_change_is_ignored() {
        let reading = ImportEditor::default()
            .signed_in_as("alice".into())
            .start_file("source.txt".into(), 1)
            .unwrap();
        let other = reading.signed_in_as("bob".into());
        assert_eq!(
            other.file_loaded(reading.ticket, Some(vec![b'a'])).bytes,
            None
        );
        let (_, intent) = ready().start_import("operation".into()).unwrap();
        let reading = ready()
            .select_source(response(&intent).id)
            .start_read()
            .unwrap();
        let other = reading.signed_in_as("bob".into());
        assert!(other
            .loaded(reading.ticket, "alice", response(&intent))
            .stored
            .is_none());
        assert_eq!(
            other
                .read_failed(reading.ticket, "alice", &error(ErrorCode::NotFound))
                .status,
            ImportStatus::Ready
        );
    }

    #[test]
    fn acknowledgement_from_changed_cookie_preserves_original_pending_import() {
        let (saving, intent) = ready().start_import("operation".into()).unwrap();
        let mut other_response = response(&intent);
        other_response.imported_by = "bob".into();
        let rejected = saving.imported(&intent, other_response);
        assert_eq!(rejected.status, ImportStatus::Error(ErrorCode::Forbidden));
        assert!(rejected.stored.is_none());
        assert_eq!(rejected.pending, Some(intent));
        assert!(rejected.ambiguous);
    }

    #[test]
    fn mismatched_acknowledgement_does_not_claim_the_wrong_source_was_saved() {
        let (saving, intent) = ready().start_import("operation".into()).unwrap();
        let mut wrong_response = response(&intent);
        wrong_response.metadata.reference = "different-source".into();
        let rejected = saving.imported(&intent, wrong_response);
        assert_eq!(
            rejected.status,
            ImportStatus::Error(ErrorCode::CorruptRevision)
        );
        assert!(rejected.stored.is_none());
        assert_eq!(rejected.pending, Some(intent));
    }

    #[test]
    fn selecting_another_source_clears_prior_success_or_failure_claims() {
        let (saving, intent) = ready().start_import("operation".into()).unwrap();
        let stored = saving.imported(&intent, response(&intent));
        let selected = stored.select_source("src_11111111111141118111111111111111".into());
        assert!(selected.stored.is_none());
        assert_eq!(selected.status, ImportStatus::Ready);
        let failed = selected.failed(&error(ErrorCode::NotFound));
        let changed = failed.select_source("src_22222222222242228222222222222222".into());
        assert_eq!(changed.status, ImportStatus::Ready);
        assert!(changed.stored.is_none());
        let empty = ImportEditor::default()
            .failed(&error(ErrorCode::NotFound))
            .select_source("src_11111111111141118111111111111111".into());
        assert_eq!(empty.status, ImportStatus::Idle);
    }

    #[test]
    fn changed_cookie_cannot_load_another_actors_receipt_under_the_local_actor() {
        let (_, intent) = ready().start_import("operation".into()).unwrap();
        let mut other_response = response(&intent);
        let reading = ready()
            .select_source(other_response.id.clone())
            .start_read()
            .unwrap();
        other_response.imported_by = "bob".into();
        let rejected = reading.loaded(reading.ticket, "alice", other_response);
        assert_eq!(rejected.status, ImportStatus::Error(ErrorCode::Forbidden));
        assert!(!rejected.busy);
        assert!(rejected.stored.is_none());
        assert_eq!(rejected.bytes, reading.bytes);
    }

    #[test]
    fn metadata_diagnostics_are_retained_with_the_file_until_a_new_request_is_prepared() {
        let (saving, _) = ready().start_import("operation".into()).unwrap();
        let mut invalid = error(ErrorCode::InvalidRequest);
        invalid.issues.push(cantos_contracts::FieldIssue {
            path: "/metadata/reference".into(),
            rule: "byte_length".into(),
        });
        let rejected = saving.failed(&invalid);
        assert_eq!(rejected.issues, invalid.issues);
        assert_eq!(rejected.bytes, saving.bytes);
        assert_eq!(rejected.metadata, saving.metadata);
        assert!(rejected.pending.is_none());
        let corrected =
            rejected.edit_metadata(|metadata| metadata.reference = "Shorter reference".into());
        assert!(corrected.issues.is_empty());
        assert_eq!(corrected.bytes, saving.bytes);
    }
}

#[cfg(test)]
mod source_id_tests {
    use super::{is_import_source_id, page_bounds, warning_on_page};

    #[test]
    fn bounded_pages_cover_every_block_once_and_keep_global_warnings_visible() {
        let pages = (0..3)
            .flat_map(|page| page_bounds(page, 250))
            .collect::<Vec<_>>();
        assert_eq!(pages, (0..250).collect::<Vec<_>>());
        assert_eq!(page_bounds(usize::MAX, 250), 200..250);
        assert_eq!(page_bounds(0, 0), 0..0);
        assert!(warning_on_page(None, 2, 250));
        assert!(warning_on_page(Some(200), 2, 250));
        assert!(!warning_on_page(Some(199), 2, 250));
        assert!(!warning_on_page(Some(250), 2, 250));
    }

    #[test]
    fn empty_first_page_does_not_hide_a_warning_on_the_next_page() {
        let warning = cantos_contracts::ImportWarning {
            code: "speaker_unknown".into(),
            block: Some(150),
        };
        assert!(!warning_on_page(warning.block, 0, 200));
        assert!(warning_on_page(warning.block, 1, 200));
        assert!(crate::messages::import_copy(true)
            .no_warnings
            .contains("Other pages may have warnings"));
        assert!(crate::messages::import_copy(false)
            .no_warnings
            .contains("Các trang khác vẫn có thể có cảnh báo"));
    }

    #[test]
    fn actual_backend_source_id_is_accepted_without_interpreting_paths() {
        assert!(is_import_source_id("src_00000000000040008000000000000001"));
        for invalid in [
            "00000000-0000-4000-8000-000000000001",
            "src_0000000000004000800000000000000",
            "src_000000000000400080000000000000001",
            "src_0000000000004000800000000000000G",
            "src_0000000000004000800000000000000F",
            "src_../../../../../../../../../../..",
            "src_00000000000040008000000000000001/original",
        ] {
            assert!(!is_import_source_id(invalid), "{invalid}");
        }
    }
}
