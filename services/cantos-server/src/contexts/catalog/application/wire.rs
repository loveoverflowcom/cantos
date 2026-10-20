//! Raw JSON admission and response conversion for the Studio catalog boundary.
//! Ownership, link existence, permissions and transitions belong to the application.
use std::collections::BTreeSet;

use cantos_contracts::{
    ArchiveContentUnitRequest, ArchiveWorkRequest, CatalogContentUnit, CatalogMutationResponse,
    CatalogPage, CatalogScriptRef, CatalogSourceRef, CatalogWork, CreateContentUnitRequest,
    CreateWorkRequest, FieldIssue, PatchContentUnitRequest, PatchField, PatchWorkRequest,
    ReorderContentUnitsRequest, UnitDetailsDto, UnitPatchDto, WorkDetailsDto, WorkPatchDto,
};

use crate::contexts::catalog::{
    domain::{
        CatalogError, ContentUnit, ContentUnitId, ContentUnitKind, LinkPatch, ScriptRef, SourceRef,
        UnitDetails, UnitPatch, Visibility, Work, WorkDetails, WorkId, WorkKind, WorkPatch,
    },
    CatalogCommand, CatalogOutcome, CatalogPage as DomainPage, CatalogStoreError,
};
use crate::platform::ids::canonical_uuid;

type Admission = Result<(String, CatalogCommand), Vec<FieldIssue>>;

pub fn create_work(request: CreateWorkRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let id = capture(WorkId::parse(&request.id), "id", &mut issues);
    let kind = capture(WorkKind::parse(&request.kind), "kind", &mut issues);
    let details = work_details(request.details, &mut issues);
    let command = id
        .zip(kind)
        .map(|(id, kind)| CatalogCommand::CreateWork { id, kind, details });
    finish(request.operation_id, command, issues)
}

pub fn patch_work(id: &str, request: PatchWorkRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let id = capture(WorkId::parse(id), "id", &mut issues);
    revision(request.expected_revision, "expected_revision", &mut issues);
    let patch = work_patch(request.details, &mut issues);
    let command = id.map(|id| CatalogCommand::PatchWork {
        id,
        expected_revision: request.expected_revision,
        patch,
    });
    finish(request.operation_id, command, issues)
}

pub fn archive_work(id: &str, request: ArchiveWorkRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let id = capture(WorkId::parse(id), "id", &mut issues);
    revision(request.expected_revision, "expected_revision", &mut issues);
    let command = id.map(|id| CatalogCommand::ArchiveWork {
        id,
        expected_revision: request.expected_revision,
    });
    finish(request.operation_id, command, issues)
}

pub fn create_unit(work_id: &str, request: CreateContentUnitRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let work_id = capture(WorkId::parse(work_id), "work_id", &mut issues);
    let id = capture(ContentUnitId::parse(&request.id), "id", &mut issues);
    let kind = capture(ContentUnitKind::parse(&request.kind), "kind", &mut issues);
    revision(
        request.expected_work_revision,
        "expected_work_revision",
        &mut issues,
    );
    let details = unit_details(request.details, &mut issues);
    let command =
        work_id
            .zip(id)
            .zip(kind)
            .map(|((work_id, id), kind)| CatalogCommand::CreateUnit {
                work_id,
                expected_work_revision: request.expected_work_revision,
                id,
                kind,
                details,
            });
    finish(request.operation_id, command, issues)
}

pub fn patch_unit(id: &str, request: PatchContentUnitRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let id = capture(ContentUnitId::parse(id), "id", &mut issues);
    revision(request.expected_revision, "expected_revision", &mut issues);
    revision(
        request.expected_work_revision,
        "expected_work_revision",
        &mut issues,
    );
    let patch = unit_patch(request.details, &mut issues);
    let command = id.map(|id| CatalogCommand::PatchUnit {
        id,
        expected_revision: request.expected_revision,
        expected_work_revision: request.expected_work_revision,
        patch,
    });
    finish(request.operation_id, command, issues)
}

pub fn archive_unit(id: &str, request: ArchiveContentUnitRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let id = capture(ContentUnitId::parse(id), "id", &mut issues);
    revision(request.expected_revision, "expected_revision", &mut issues);
    revision(
        request.expected_work_revision,
        "expected_work_revision",
        &mut issues,
    );
    let command = id.map(|id| CatalogCommand::ArchiveUnit {
        id,
        expected_revision: request.expected_revision,
        expected_work_revision: request.expected_work_revision,
    });
    finish(request.operation_id, command, issues)
}

pub fn reorder(work_id: &str, request: ReorderContentUnitsRequest) -> Admission {
    let mut issues = Vec::new();
    operation(&request.operation_id, &mut issues);
    let work_id = capture(WorkId::parse(work_id), "work_id", &mut issues);
    revision(request.expected_revision, "expected_revision", &mut issues);
    if request.ordered.len() > 1000 {
        issues.push(field_issue("ordered", "max_items_1000"));
    }
    let mut seen = BTreeSet::new();
    let mut ordered = Vec::new();
    for (index, value) in request.ordered.iter().enumerate() {
        let path = format!("ordered[{index}]");
        if !seen.insert(value) {
            issues.push(field_issue(&path, "duplicate_id"));
        }
        if let Some(id) = capture(ContentUnitId::parse(value), &path, &mut issues) {
            ordered.push(id);
        }
    }
    let command = work_id.map(|work_id| CatalogCommand::Reorder {
        work_id,
        expected_revision: request.expected_revision,
        ordered,
    });
    finish(request.operation_id, command, issues)
}

pub fn validate_work_id(value: &str) -> Result<WorkId, Vec<FieldIssue>> {
    WorkId::parse(value).map_err(|error| vec![domain_issue(error, "id")])
}

pub fn validate_unit_id(value: &str) -> Result<ContentUnitId, Vec<FieldIssue>> {
    ContentUnitId::parse(value).map_err(|error| vec![domain_issue(error, "id")])
}

fn operation(value: &str, issues: &mut Vec<FieldIssue>) {
    if canonical_uuid(value).is_none() {
        issues.push(field_issue("operation_id", "canonical_uuid"));
    }
}

fn revision(value: u64, path: &str, issues: &mut Vec<FieldIssue>) {
    if value == 0 || value > i64::MAX as u64 {
        issues.push(field_issue(path, "positive_revision"));
    }
}

fn finish(
    operation: String,
    command: Option<CatalogCommand>,
    issues: Vec<FieldIssue>,
) -> Admission {
    match command {
        Some(command) if issues.is_empty() => Ok((operation, command)),
        _ => Err(issues),
    }
}

fn capture<T>(
    result: Result<T, CatalogError>,
    path: &str,
    issues: &mut Vec<FieldIssue>,
) -> Option<T> {
    match result {
        Ok(value) => Some(value),
        Err(error) => {
            issues.push(domain_issue(error, path));
            None
        }
    }
}

fn work_details(raw: WorkDetailsDto, issues: &mut Vec<FieldIssue>) -> WorkDetails {
    let visibility = capture(
        Visibility::parse(&raw.visibility),
        "details.visibility",
        issues,
    )
    .unwrap_or(Visibility::Private);
    let details = WorkDetails {
        title: raw.title,
        synopsis: raw.synopsis,
        language: raw.language,
        visibility,
        rights_refs: raw.rights_refs,
    };
    metadata_errors(details.validation_errors(), issues);
    details
}

fn unit_details(raw: UnitDetailsDto, issues: &mut Vec<FieldIssue>) -> UnitDetails {
    let source = raw.source.and_then(|raw| source_ref(raw, issues));
    let script = raw.script.and_then(|raw| script_ref(raw, issues));
    let details = UnitDetails {
        title: raw.title,
        description: raw.description,
        source,
        script,
        content_digest: raw.content_digest,
    };
    metadata_errors(details.validation_errors(), issues);
    details
}

fn work_patch(raw: WorkPatchDto, issues: &mut Vec<FieldIssue>) -> WorkPatch {
    if raw.title.is_missing()
        && raw.synopsis.is_missing()
        && raw.language.is_missing()
        && raw.visibility.is_missing()
        && raw.rights_refs.is_missing()
    {
        issues.push(field_issue("details", "empty_patch"));
    }
    let patch = WorkPatch {
        title: required_patch(raw.title, "details.title", issues),
        synopsis: required_patch(raw.synopsis, "details.synopsis", issues),
        language: required_patch(raw.language, "details.language", issues),
        visibility: required_patch(raw.visibility, "details.visibility", issues)
            .and_then(|value| capture(Visibility::parse(&value), "details.visibility", issues)),
        rights_refs: required_patch(raw.rights_refs, "details.rights_refs", issues),
    };
    // Only supplied fields replace valid placeholders, so domain validation reports
    // their independent errors without fetching or modifying the current snapshot.
    let candidate = patch.clone().apply(&WorkDetails {
        title: "validation placeholder".into(),
        synopsis: String::new(),
        language: "vi".into(),
        visibility: Visibility::Private,
        rights_refs: Vec::new(),
    });
    metadata_errors(candidate.validation_errors(), issues);
    patch
}

fn unit_patch(raw: UnitPatchDto, issues: &mut Vec<FieldIssue>) -> UnitPatch {
    if raw.title.is_missing()
        && raw.description.is_missing()
        && raw.source.is_missing()
        && raw.script.is_missing()
        && raw.content_digest.is_missing()
    {
        issues.push(field_issue("details", "empty_patch"));
    }
    let patch = UnitPatch {
        title: required_patch(raw.title, "details.title", issues),
        description: required_patch(raw.description, "details.description", issues),
        source: optional_patch(raw.source, issues, source_ref),
        script: optional_patch(raw.script, issues, script_ref),
        content_digest: match raw.content_digest {
            PatchField::Missing => LinkPatch::Keep,
            PatchField::Null => LinkPatch::Clear,
            PatchField::Value(value) => LinkPatch::Set(value),
        },
    };
    let candidate = patch.clone().apply(&UnitDetails {
        title: "validation placeholder".into(),
        description: String::new(),
        source: None,
        script: None,
        content_digest: None,
    });
    metadata_errors(candidate.validation_errors(), issues);
    patch
}

fn required_patch<T>(raw: PatchField<T>, path: &str, issues: &mut Vec<FieldIssue>) -> Option<T> {
    match raw {
        PatchField::Missing => None,
        PatchField::Null => {
            issues.push(field_issue(path, "required"));
            None
        }
        PatchField::Value(value) => Some(value),
    }
}

fn optional_patch<T, U>(
    raw: PatchField<T>,
    issues: &mut Vec<FieldIssue>,
    convert: impl FnOnce(T, &mut Vec<FieldIssue>) -> Option<U>,
) -> LinkPatch<U> {
    match raw {
        PatchField::Missing => LinkPatch::Keep,
        PatchField::Null => LinkPatch::Clear,
        PatchField::Value(value) => convert(value, issues).map_or(LinkPatch::Keep, LinkPatch::Set),
    }
}

fn source_ref(raw: CatalogSourceRef, issues: &mut Vec<FieldIssue>) -> Option<SourceRef> {
    metadata_errors(SourceRef::validation_errors(&raw.id, &raw.sha256), issues);
    SourceRef::new(raw.id, raw.sha256).ok()
}

fn script_ref(raw: CatalogScriptRef, issues: &mut Vec<FieldIssue>) -> Option<ScriptRef> {
    for error in ScriptRef::validation_errors(&raw.id, raw.revision, &raw.export_digest) {
        let path = if error == CatalogError::InvalidRevision {
            "details.script.revision"
        } else {
            "details"
        };
        issues.push(domain_issue(error, path));
    }
    ScriptRef::new(raw.id, raw.revision, raw.export_digest).ok()
}

fn metadata_errors(errors: Vec<CatalogError>, issues: &mut Vec<FieldIssue>) {
    issues.extend(
        errors
            .into_iter()
            .map(|error| domain_issue(error, "details")),
    );
}

fn domain_issue(error: CatalogError, path: &str) -> FieldIssue {
    let (suffix, rule) = match error {
        CatalogError::BlankField { field } => (Some(field), "required".into()),
        CatalogError::FieldTooLong { field, max_bytes } => {
            (Some(field), format!("max_bytes_{max_bytes}"))
        }
        CatalogError::ControlCharacter { field } => (Some(field), "control_character".into()),
        CatalogError::InvalidLanguage => (Some("language"), "language_tag".into()),
        CatalogError::TooManyRightsRefs => (Some("rights_refs"), "max_items_64".into()),
        CatalogError::InvalidRightsRef => (Some("rights_refs"), "token_id".into()),
        CatalogError::DuplicateRightsRef => (Some("rights_refs"), "duplicate_id".into()),
        CatalogError::InvalidSourceId => (Some("source.id"), "token_id".into()),
        CatalogError::InvalidSourceDigest => (Some("source.sha256"), "sha256".into()),
        CatalogError::InvalidScriptId => (Some("script.id"), "canonical_uuid".into()),
        CatalogError::InvalidScriptDigest => (Some("script.export_digest"), "export_digest".into()),
        CatalogError::InvalidContentDigest => (Some("content_digest"), "content_digest".into()),
        CatalogError::InvalidRevision => (None, "positive_revision".into()),
        CatalogError::InvalidId(_) => (None, "canonical_uuid".into()),
        CatalogError::UnknownWorkKind => (None, "work_kind".into()),
        CatalogError::UnknownContentUnitKind => (None, "unit_kind".into()),
        CatalogError::UnknownVisibility => (None, "visibility".into()),
        _ => (None, "invalid_catalog".into()),
    };
    FieldIssue {
        path: suffix.map_or_else(|| path.to_owned(), |suffix| format!("{path}.{suffix}")),
        rule,
    }
}

fn field_issue(path: &str, rule: &str) -> FieldIssue {
    FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

pub fn work(value: &Work) -> CatalogWork {
    CatalogWork {
        id: value.id().as_str().into(),
        kind: value.kind().as_str().into(),
        owner_id: value.owner_id().as_str().into(),
        details: WorkDetailsDto {
            title: value.details().title.clone(),
            synopsis: value.details().synopsis.clone(),
            language: value.details().language.clone(),
            visibility: value.details().visibility.as_str().into(),
            rights_refs: value.details().rights_refs.clone(),
        },
        lifecycle: value.lifecycle().as_str().into(),
        revision: value.revision(),
        created_at: value.created_at(),
        updated_at: value.updated_at(),
    }
}

pub fn unit(value: &ContentUnit) -> CatalogContentUnit {
    CatalogContentUnit {
        id: value.id().as_str().into(),
        work_id: value.work_id().as_str().into(),
        kind: value.kind().as_str().into(),
        details: UnitDetailsDto {
            title: value.details().title.clone(),
            description: value.details().description.clone(),
            source: value
                .details()
                .source
                .as_ref()
                .map(|source| CatalogSourceRef {
                    id: source.id().into(),
                    sha256: source.sha256().into(),
                }),
            script: value
                .details()
                .script
                .as_ref()
                .map(|script| CatalogScriptRef {
                    id: script.id().into(),
                    revision: script.revision(),
                    export_digest: script.export_digest().into(),
                }),
            content_digest: value.details().content_digest.clone(),
        },
        position: value.position(),
        lifecycle: value.lifecycle().as_str().into(),
        revision: value.revision(),
        created_at: value.created_at(),
        updated_at: value.updated_at(),
    }
}

pub fn outcome(value: CatalogOutcome) -> CatalogMutationResponse {
    CatalogMutationResponse {
        work: work(&value.work),
        units: value.units.iter().map(unit).collect(),
    }
}

pub fn work_page(value: DomainPage<Work>) -> Result<CatalogPage<CatalogWork>, CatalogStoreError> {
    Ok(CatalogPage {
        items: value.items.iter().map(work).collect(),
        next_cursor: value.next.map(|cursor| cursor.encode()).transpose()?,
    })
}

pub fn unit_page(
    value: DomainPage<ContentUnit>,
) -> Result<CatalogPage<CatalogContentUnit>, CatalogStoreError> {
    Ok(CatalogPage {
        items: value.items.iter().map(unit).collect(),
        next_cursor: value.next.map(|cursor| cursor.encode()).transpose()?,
    })
}

/// Pure admission/representation for the separate content lifecycle.
pub use super::content::wire as content;
pub use super::delegation::wire as delegation;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::contexts::catalog::domain::{NewContentUnit, NewWork, OwnerId};

    const WORK: &str = "10000000-0000-0000-0000-000000000001";
    const UNIT: &str = "20000000-0000-0000-0000-000000000001";
    const OPERATION: &str = "30000000-0000-0000-0000-000000000001";
    const SCRIPT: &str = "40000000-0000-0000-0000-000000000001";

    fn work_request(kind: &str) -> CreateWorkRequest {
        CreateWorkRequest {
            operation_id: OPERATION.into(),
            id: WORK.into(),
            kind: kind.into(),
            details: WorkDetailsDto {
                title: "Đêm trên Vọng Đài".into(),
                synopsis: "Một người kể chuyện trở về.\nChương mở đầu.".into(),
                language: "vi-VN".into(),
                visibility: "private".into(),
                rights_refs: Vec::new(),
            },
        }
    }

    fn unit_request(kind: &str) -> CreateContentUnitRequest {
        CreateContentUnitRequest {
            operation_id: OPERATION.into(),
            id: UNIT.into(),
            kind: kind.into(),
            expected_work_revision: 1,
            details: UnitDetailsDto {
                title: "Chương một — Ngọn đèn".into(),
                description: "Người kể chuyện:\tĐêm xuống.\r\n".into(),
                source: None,
                script: None,
                content_digest: None,
            },
        }
    }

    #[test]
    fn creates_accept_all_declared_categories_without_script_or_cast_requirements() {
        for (raw, expected) in [
            ("novel", WorkKind::Novel),
            ("story", WorkKind::Story),
            ("podcast", WorkKind::Podcast),
        ] {
            let (operation, command) = create_work(work_request(raw)).unwrap();
            assert_eq!(operation, OPERATION);
            match command {
                CatalogCommand::CreateWork { id, kind, details } => {
                    assert_eq!(id.as_str(), WORK);
                    assert_eq!(kind, expected);
                    assert_eq!(details.title, "Đêm trên Vọng Đài");
                }
                _ => panic!("expected CreateWork"),
            }
        }
        for (raw, expected) in [
            ("chapter", ContentUnitKind::Chapter),
            ("episode", ContentUnitKind::Episode),
        ] {
            let (_, command) = create_unit(WORK, unit_request(raw)).unwrap();
            match command {
                CatalogCommand::CreateUnit { kind, details, .. } => {
                    assert_eq!(kind, expected);
                    assert_eq!(details.source, None);
                    assert_eq!(details.script, None);
                }
                _ => panic!("expected CreateUnit"),
            }
        }
    }

    #[test]
    fn work_admission_accumulates_errors_without_echoing_private_input() {
        let mut raw = work_request("unknown");
        raw.operation_id = "PRIVATE OPERATION".into();
        raw.id = "PRIVATE ID".into();
        raw.details.title = format!("{}\u{0001}", "x".repeat(512));
        raw.details.synopsis = "x".repeat(16_385);
        raw.details.language = "vi--VN".into();
        raw.details.visibility = "PRIVATE VISIBILITY".into();
        raw.details.rights_refs = vec!["PRIVATE RIGHTS".into(), "PRIVATE RIGHTS".into()];
        assert_eq!(
            create_work(raw).unwrap_err(),
            vec![
                field_issue("operation_id", "canonical_uuid"),
                field_issue("id", "canonical_uuid"),
                field_issue("kind", "work_kind"),
                field_issue("details.visibility", "visibility"),
                field_issue("details.title", "max_bytes_512"),
                field_issue("details.title", "control_character"),
                field_issue("details.synopsis", "max_bytes_16384"),
                field_issue("details.language", "language_tag"),
                field_issue("details.rights_refs", "token_id"),
                field_issue("details.rights_refs", "token_id"),
                field_issue("details.rights_refs", "duplicate_id"),
            ]
        );
    }

    #[test]
    fn unit_admission_accumulates_all_nested_reference_errors() {
        let mut raw = unit_request("invalid");
        raw.expected_work_revision = 0;
        raw.details.title = "  ".into();
        raw.details.description = "private\u{0001}text".into();
        raw.details.source = Some(CatalogSourceRef {
            id: "INVALID SOURCE".into(),
            sha256: "invalid source digest".into(),
        });
        raw.details.script = Some(CatalogScriptRef {
            id: "invalid script".into(),
            revision: u64::MAX,
            export_digest: "invalid export".into(),
        });
        raw.details.content_digest = Some("invalid content".into());
        assert_eq!(
            create_unit("invalid parent", raw).unwrap_err(),
            vec![
                field_issue("work_id", "canonical_uuid"),
                field_issue("kind", "unit_kind"),
                field_issue("expected_work_revision", "positive_revision"),
                field_issue("details.source.id", "token_id"),
                field_issue("details.source.sha256", "sha256"),
                field_issue("details.script.id", "canonical_uuid"),
                field_issue("details.script.revision", "positive_revision"),
                field_issue("details.script.export_digest", "export_digest"),
                field_issue("details.title", "required"),
                field_issue("details.description", "control_character"),
                field_issue("details.content_digest", "content_digest"),
            ]
        );
    }

    #[test]
    fn metadata_sizes_count_utf8_bytes_and_allow_only_multiline_controls() {
        let mut raw = work_request("story");
        raw.details.title = "ế".repeat(170);
        raw.details.synopsis = "\n\r\t".into();
        match create_work(raw.clone()).unwrap().1 {
            CatalogCommand::CreateWork { details, .. } => assert_eq!(details.title.len(), 510),
            _ => panic!("expected CreateWork"),
        }
        raw.details.title.push('ế');
        assert_eq!(
            create_work(raw).unwrap_err(),
            vec![field_issue("details.title", "max_bytes_512")]
        );
        let mut raw = work_request("story");
        raw.details.title = "\t".into();
        assert_eq!(
            create_work(raw).unwrap_err(),
            vec![
                field_issue("details.title", "required"),
                field_issue("details.title", "control_character")
            ]
        );
    }

    #[test]
    fn work_patch_preserves_omitted_values_and_rejects_explicit_null_or_empty_patch() {
        let request = PatchWorkRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            details: WorkPatchDto {
                synopsis: PatchField::Value(String::new()),
                ..WorkPatchDto::default()
            },
        };
        match patch_work(WORK, request).unwrap().1 {
            CatalogCommand::PatchWork { patch, .. } => {
                assert_eq!(patch.title, None);
                assert_eq!(patch.synopsis, Some(String::new()));
                assert_eq!(patch.language, None);
                assert_eq!(patch.visibility, None);
                assert_eq!(patch.rights_refs, None);
            }
            _ => panic!("expected PatchWork"),
        }
        let request = PatchWorkRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            details: WorkPatchDto {
                title: PatchField::Null,
                synopsis: PatchField::Null,
                language: PatchField::Null,
                visibility: PatchField::Null,
                rights_refs: PatchField::Null,
            },
        };
        assert_eq!(
            patch_work(WORK, request).unwrap_err(),
            vec![
                field_issue("details.title", "required"),
                field_issue("details.synopsis", "required"),
                field_issue("details.language", "required"),
                field_issue("details.visibility", "required"),
                field_issue("details.rights_refs", "required"),
            ]
        );
        let request = PatchWorkRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            details: WorkPatchDto::default(),
        };
        assert_eq!(
            patch_work(WORK, request).unwrap_err(),
            vec![field_issue("details", "empty_patch")]
        );
    }

    #[test]
    fn unit_patch_distinguishes_omitted_links_explicit_clears_and_pinned_replacements() {
        let source = CatalogSourceRef {
            id: "source_1".into(),
            sha256: "a".repeat(64),
        };
        let request = PatchContentUnitRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            expected_work_revision: 2,
            details: UnitPatchDto {
                source: PatchField::Value(source.clone()),
                script: PatchField::Null,
                content_digest: PatchField::Missing,
                ..UnitPatchDto::default()
            },
        };
        match patch_unit(UNIT, request).unwrap().1 {
            CatalogCommand::PatchUnit { patch, .. } => {
                assert_eq!(patch.title, None);
                assert_eq!(
                    patch.source,
                    LinkPatch::Set(SourceRef::new(source.id, source.sha256).unwrap())
                );
                assert_eq!(patch.script, LinkPatch::Clear);
                assert_eq!(patch.content_digest, LinkPatch::Keep);
            }
            _ => panic!("expected PatchUnit"),
        }
        let request = PatchContentUnitRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            expected_work_revision: 2,
            details: UnitPatchDto {
                title: PatchField::Value("Chương hai".into()),
                content_digest: PatchField::Null,
                ..UnitPatchDto::default()
            },
        };
        match patch_unit(UNIT, request).unwrap().1 {
            CatalogCommand::PatchUnit { patch, .. } => {
                assert_eq!(patch.source, LinkPatch::Keep);
                assert_eq!(patch.script, LinkPatch::Keep);
                assert_eq!(patch.content_digest, LinkPatch::Clear);
            }
            _ => panic!("expected PatchUnit"),
        }
    }

    #[test]
    fn unit_patch_rejects_null_required_metadata_and_empty_patch() {
        let request = PatchContentUnitRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            expected_work_revision: 2,
            details: UnitPatchDto {
                title: PatchField::Null,
                description: PatchField::Null,
                ..UnitPatchDto::default()
            },
        };
        assert_eq!(
            patch_unit(UNIT, request).unwrap_err(),
            vec![
                field_issue("details.title", "required"),
                field_issue("details.description", "required"),
            ]
        );
        let request = PatchContentUnitRequest {
            operation_id: OPERATION.into(),
            expected_revision: 1,
            expected_work_revision: 2,
            details: UnitPatchDto::default(),
        };
        assert_eq!(
            patch_unit(UNIT, request).unwrap_err(),
            vec![field_issue("details", "empty_patch")]
        );
    }

    #[test]
    fn archive_requests_validate_both_revision_guards_and_canonical_ids() {
        assert_eq!(
            archive_unit(
                "invalid",
                ArchiveContentUnitRequest {
                    operation_id: "invalid".into(),
                    expected_revision: 0,
                    expected_work_revision: u64::MAX,
                },
            )
            .unwrap_err(),
            vec![
                field_issue("operation_id", "canonical_uuid"),
                field_issue("id", "canonical_uuid"),
                field_issue("expected_revision", "positive_revision"),
                field_issue("expected_work_revision", "positive_revision"),
            ]
        );
        assert_eq!(
            archive_work(
                WORK,
                ArchiveWorkRequest {
                    operation_id: OPERATION.into(),
                    expected_revision: i64::MAX as u64 + 1,
                }
            )
            .unwrap_err(),
            vec![field_issue("expected_revision", "positive_revision")]
        );
    }

    #[test]
    fn reorder_reports_each_duplicate_or_invalid_entry_and_caps_the_batch() {
        assert_eq!(
            reorder(
                WORK,
                ReorderContentUnitsRequest {
                    operation_id: OPERATION.into(),
                    expected_revision: 1,
                    ordered: vec![UNIT.into(), UNIT.into(), "invalid".into(), "invalid".into()],
                }
            )
            .unwrap_err(),
            vec![
                field_issue("ordered[1]", "duplicate_id"),
                field_issue("ordered[2]", "canonical_uuid"),
                field_issue("ordered[3]", "duplicate_id"),
                field_issue("ordered[3]", "canonical_uuid"),
            ]
        );
        let issues = reorder(
            WORK,
            ReorderContentUnitsRequest {
                operation_id: OPERATION.into(),
                expected_revision: 1,
                ordered: (0..1001)
                    .map(|index| format!("{index:08x}-0000-0000-0000-000000000001"))
                    .collect(),
            },
        )
        .unwrap_err();
        assert_eq!(issues, vec![field_issue("ordered", "max_items_1000")]);
    }

    #[test]
    fn response_conversion_preserves_identity_timestamps_history_and_pinned_links() {
        let raw = work_request("podcast");
        let work = Work::new(NewWork {
            id: WorkId::parse(WORK).unwrap(),
            kind: WorkKind::Podcast,
            owner_id: OwnerId::parse("author_1").unwrap(),
            details: work_details(raw.details, &mut Vec::new()),
            at: 101,
        })
        .unwrap();
        let unit = ContentUnit::new(
            &work,
            NewContentUnit {
                id: ContentUnitId::parse(UNIT).unwrap(),
                kind: ContentUnitKind::Episode,
                details: UnitDetails {
                    title: "Tập một".into(),
                    description: "Tiếng Việt".into(),
                    source: Some(SourceRef::new("source_1".into(), "a".repeat(64)).unwrap()),
                    script: Some(
                        ScriptRef::new(
                            SCRIPT.into(),
                            7,
                            format!("sir-e1:sha256:{}", "b".repeat(64)),
                        )
                        .unwrap(),
                    ),
                    content_digest: Some(format!("sir-c1:sha256:{}", "c".repeat(64))),
                },
                position: 3,
                at: 102,
            },
        )
        .unwrap();
        let archived = work.archive(1, 103).unwrap();
        let response = outcome(CatalogOutcome {
            work: archived,
            units: vec![unit],
        });
        assert_eq!(response.work.id, WORK);
        assert_eq!(response.work.owner_id, "author_1");
        assert_eq!(response.work.kind, "podcast");
        assert_eq!(response.work.lifecycle, "archived");
        assert_eq!(response.work.revision, 2);
        assert_eq!(response.work.created_at, 101);
        assert_eq!(response.work.updated_at, 103);
        let unit = &response.units[0];
        assert_eq!(unit.id, UNIT);
        assert_eq!(unit.work_id, WORK);
        assert_eq!(unit.kind, "episode");
        assert_eq!(unit.position, 3);
        assert_eq!(unit.lifecycle, "draft");
        assert_eq!(unit.revision, 1);
        assert_eq!(unit.created_at, 102);
        assert_eq!(unit.updated_at, 102);
        assert_eq!(unit.details.source.as_ref().unwrap().sha256, "a".repeat(64));
        assert_eq!(unit.details.script.as_ref().unwrap().revision, 7);
        assert_eq!(
            unit.details.content_digest,
            Some(format!("sir-c1:sha256:{}", "c".repeat(64)))
        );
        let page = work_page(DomainPage {
            items: vec![work],
            next: None,
        })
        .unwrap();
        assert_eq!(page.items[0].revision, 1);
        assert_eq!(page.next_cursor, None);
    }
}
