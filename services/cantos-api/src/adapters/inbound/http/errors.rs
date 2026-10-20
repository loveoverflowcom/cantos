//! Exhaustive application/storage outcome projection onto the retained JSON error contract.
use crate::{
    application::StoreError,
    contexts::{editorial::domain::diagnostics::validation_issues, media::read::StorageError},
};
use axum::{
    http::StatusCode,
    response::{IntoResponse, Response},
    Json,
};
use cantos_contracts::{ApiError, ErrorCode};

pub(super) fn failure(code: ErrorCode, status: StatusCode) -> Response {
    (
        status,
        Json(ApiError {
            code,
            current_revision: None,
            issues: vec![],
        }),
    )
        .into_response()
}

pub(super) fn error_response(error: StoreError) -> Response {
    let (status, code, current_revision, issues) = match error {
        StoreError::Unauthenticated => (
            StatusCode::UNAUTHORIZED,
            ErrorCode::Unauthenticated,
            None,
            vec![],
        ),
        StoreError::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound, None, vec![]),
        StoreError::Forbidden => (StatusCode::FORBIDDEN, ErrorCode::Forbidden, None, vec![]),
        StoreError::InvalidRequest => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            vec![],
        ),
        StoreError::InvalidRequestFields(issues) => (
            StatusCode::BAD_REQUEST,
            ErrorCode::InvalidRequest,
            None,
            issues,
        ),
        StoreError::EvidenceUnavailable => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::EvidenceUnavailable,
            None,
            vec![],
        ),
        StoreError::StaleRevision(revision) => (
            StatusCode::CONFLICT,
            ErrorCode::StaleRevision,
            Some(revision),
            vec![],
        ),
        StoreError::OperationReused => (
            StatusCode::CONFLICT,
            ErrorCode::OperationReused,
            None,
            vec![],
        ),
        StoreError::ProposalAlreadySubmitted => (
            StatusCode::CONFLICT,
            ErrorCode::ProposalAlreadySubmitted,
            None,
            vec![],
        ),
        StoreError::StaleProductionInputs => (
            StatusCode::CONFLICT,
            ErrorCode::StaleProductionInputs,
            None,
            vec![],
        ),
        StoreError::ProductionBlocked(issues) => (
            StatusCode::CONFLICT,
            ErrorCode::ProductionBlocked,
            None,
            issues,
        ),
        StoreError::Storage(error) => return storage_error_response(error),
        StoreError::Unavailable => (
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            None,
            vec![],
        ),
        StoreError::CorruptRevision => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
            None,
            vec![],
        ),
        StoreError::AudioRejected(rule) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::AudioRejected,
            None,
            vec![cantos_contracts::FieldIssue {
                path: "audio".into(),
                rule: rule.into(),
            }],
        ),
        StoreError::AudioInspectionUnavailable(rule) => (
            StatusCode::SERVICE_UNAVAILABLE,
            ErrorCode::Unavailable,
            None,
            vec![cantos_contracts::FieldIssue {
                path: "audio".into(),
                rule: rule.into(),
            }],
        ),
        StoreError::InvalidScript(error) => (
            StatusCode::UNPROCESSABLE_ENTITY,
            ErrorCode::InvalidScript,
            None,
            crate::contexts::editorial::document::issues(validation_issues(error)),
        ),
    };
    (
        status,
        Json(ApiError {
            code,
            current_revision,
            issues,
        }),
    )
        .into_response()
}

fn storage_error_response(error: StorageError) -> Response {
    let level = match error {
        StorageError::NotFound => "DEBUG",
        StorageError::Forbidden
        | StorageError::Conflict
        | StorageError::Timeout
        | StorageError::Cancelled => "WARN",
        StorageError::InvalidTarget
        | StorageError::InvalidKey
        | StorageError::InvalidIntegrity
        | StorageError::DigestMismatch
        | StorageError::LengthMismatch
        | StorageError::Unavailable
        | StorageError::UnsupportedCapability(_) => "ERROR",
    };
    eprintln!(
        "level={level} event=private_storage_operation_failed outcome={}",
        error.code()
    );
    let (status, code) = match error {
        StorageError::NotFound => (StatusCode::NOT_FOUND, ErrorCode::NotFound),
        StorageError::Forbidden => (StatusCode::FORBIDDEN, ErrorCode::Forbidden),
        StorageError::Conflict => (StatusCode::CONFLICT, ErrorCode::OperationReused),
        StorageError::InvalidTarget
        | StorageError::InvalidKey
        | StorageError::InvalidIntegrity
        | StorageError::DigestMismatch
        | StorageError::LengthMismatch => (
            StatusCode::INTERNAL_SERVER_ERROR,
            ErrorCode::CorruptRevision,
        ),
        StorageError::Timeout
        | StorageError::Cancelled
        | StorageError::Unavailable
        | StorageError::UnsupportedCapability(_) => {
            (StatusCode::SERVICE_UNAVAILABLE, ErrorCode::Unavailable)
        }
    };
    failure(code, status)
}
