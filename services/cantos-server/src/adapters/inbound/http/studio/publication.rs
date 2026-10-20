//! Studio-only publication transport. No listener projection or storage URL crosses this API.
use super::{catalog::CatalogBody, StudioPath};
use crate::{
    adapters::inbound::http::{
        errors::error_response as storage_error, errors::failure, studio::admission::cookie_token,
        AppState,
    },
    application::StoreError,
    contexts::media::{
        domain::publication::PublicationError, PublicationDependencyError, PublicationService,
        PublicationStoreError,
    },
};
use axum::{
    extract::{Extension, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use cantos_contracts as dto;
pub(super) fn routes() -> Router<AppState> {
    Router::new()
        .route(
            "/content-units/{unit}/release-candidates/{candidate}/readiness",
            get(readiness),
        )
        .route(
            "/content-units/{unit}/release-candidates/{candidate}/distribution-rights",
            post(rights),
        )
        .route(
            "/content-units/{unit}/release-candidates/{candidate}/publication-approvals",
            post(approve),
        )
        .route(
            "/content-units/{unit}/release-candidates/{candidate}/publish",
            post(publish),
        )
        .route("/content-units/{unit}/publication/active", get(active))
        .route("/content-units/{unit}/publication/withdraw", post(withdraw))
}
fn blocker_code(b: dto::PublicationBlocker) -> &'static str {
    use dto::PublicationBlocker as B;
    match b {
        B::CandidateMismatch => "candidate_mismatch",
        B::OwnerScopeLost => "owner_scope_lost",
        B::Archived => "archived",
        B::StaleBrief => "stale_brief",
        B::MetadataChanged => "metadata_changed",
        B::UploadPermissionRevoked => "upload_permission_revoked",
        B::UploadPermissionExpired => "upload_permission_expired",
        B::ProfileUnsupported => "profile_unsupported",
        B::ObjectMissing => "object_missing",
        B::ObjectUnverified => "object_unverified",
        B::ObjectIntegrityMismatch => "object_integrity_mismatch",
        B::QcNotApproved => "qc_not_approved",
        B::RightsReviewNotApproved => "rights_review_not_approved",
        B::EditorialNotApproved => "editorial_not_approved",
        B::CandidateReviewsChanged => "candidate_reviews_changed",
        B::RightsUnrecorded => "rights_unrecorded",
        B::RightsRevoked => "rights_revoked",
        B::RightsExpired => "rights_expired",
        B::RightsScopeMismatch => "rights_scope_mismatch",
        B::ApprovalMissing => "approval_missing",
        B::ApprovalWithdrawn => "approval_withdrawn",
        B::ApprovalStale => "approval_stale",
        B::DeliveryUnavailable => "delivery_unavailable",
        B::DeliveryMissing => "delivery_missing",
        B::DeliveryIntegrityMismatch => "delivery_integrity_mismatch",
        B::DeliveryEvidenceExpired => "delivery_evidence_expired",
    }
}
fn error(error: PublicationStoreError) -> Response {
    match error {
        PublicationStoreError::Storage(e) => storage_error(e),
        PublicationStoreError::Dependency(PublicationDependencyError::Unavailable) => {
            storage_error(StoreError::Unavailable)
        }
        PublicationStoreError::Dependency(PublicationDependencyError::InvalidObservation) => {
            storage_error(StoreError::CorruptRevision)
        }
        PublicationStoreError::Conflict {
            version,
            active_release_id: _,
        } => storage_error(StoreError::StaleRevision(version)),
        PublicationStoreError::Blocked(blockers) => storage_error(StoreError::ProductionBlocked(
            blockers
                .into_iter()
                .map(|b| dto::FieldIssue {
                    path: "publication".into(),
                    rule: blocker_code(b).into(),
                })
                .collect(),
        )),
        PublicationStoreError::Domain(e) => match e {
            PublicationError::Encoding => storage_error(StoreError::CorruptRevision),
            PublicationError::PointerConflict => {
                failure(dto::ErrorCode::StaleRevision, StatusCode::CONFLICT)
            }
            PublicationError::VersionExhausted => {
                storage_error(StoreError::ProductionBlocked(vec![dto::FieldIssue {
                    path: "publication".into(),
                    rule: "pointer_exhausted".into(),
                }]))
            }
            PublicationError::InvalidId
            | PublicationError::InvalidDigest
            | PublicationError::InvalidEvidence
            | PublicationError::InvalidTime
            | PublicationError::InvalidSequence
            | PublicationError::BindingMismatch => storage_error(StoreError::InvalidRequest),
        },
    }
}
fn result<T: serde::Serialize>(
    result: Result<T, PublicationStoreError>,
    status: StatusCode,
) -> Response {
    match result {
        Ok(value) => (status, Json(value)).into_response(),
        Err(e) => error(e),
    }
}
async fn readiness(
    State(state): State<AppState>,
    Extension(publication): Extension<PublicationService>,
    StudioPath((unit, candidate)): StudioPath<(String, String)>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(t) => t,
        Err(e) => return storage_error(e),
    };
    result(
        publication
            .audio_publication_readiness(&token, &unit, &candidate)
            .await,
        StatusCode::OK,
    )
}
async fn rights(
    State(state): State<AppState>,
    Extension(publication): Extension<PublicationService>,
    StudioPath((unit, candidate)): StudioPath<(String, String)>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::RecordDistributionRightsRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(t) => t,
        Err(e) => return storage_error(e),
    };
    result(
        publication
            .record_audio_distribution_rights(&token, &unit, &candidate, request)
            .await,
        StatusCode::CREATED,
    )
}
async fn approve(
    State(state): State<AppState>,
    Extension(publication): Extension<PublicationService>,
    StudioPath((unit, candidate)): StudioPath<(String, String)>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::RecordPublicationApprovalRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(t) => t,
        Err(e) => return storage_error(e),
    };
    result(
        publication
            .record_audio_publication_approval(&token, &unit, &candidate, request)
            .await,
        StatusCode::CREATED,
    )
}
async fn publish(
    State(state): State<AppState>,
    Extension(publication): Extension<PublicationService>,
    StudioPath((unit, candidate)): StudioPath<(String, String)>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::PublishAudioReleaseRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(t) => t,
        Err(e) => return storage_error(e),
    };
    result(
        publication
            .publish_audio_release(&token, &unit, &candidate, request)
            .await,
        StatusCode::CREATED,
    )
}
async fn active(
    State(state): State<AppState>,
    Extension(publication): Extension<PublicationService>,
    StudioPath(unit): StudioPath<String>,
    headers: HeaderMap,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(t) => t,
        Err(e) => return storage_error(e),
    };
    result(
        publication.active_audio_release(&token, &unit).await,
        StatusCode::OK,
    )
}
async fn withdraw(
    State(state): State<AppState>,
    Extension(publication): Extension<PublicationService>,
    StudioPath(unit): StudioPath<String>,
    headers: HeaderMap,
    CatalogBody(request): CatalogBody<dto::WithdrawAudioReleaseRequest>,
) -> Response {
    let token = match cookie_token(&headers, &state.store) {
        Ok(t) => t,
        Err(e) => return storage_error(e),
    };
    result(
        publication
            .withdraw_audio_release(&token, &unit, request)
            .await,
        StatusCode::OK,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;
    #[tokio::test]
    async fn blockers_conflicts_and_dependency_errors_map_exhaustively_without_private_facts() {
        let response = error(PublicationStoreError::Blocked(vec![
            dto::PublicationBlocker::RightsExpired,
            dto::PublicationBlocker::DeliveryUnavailable,
        ]));
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let body: dto::ApiError =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(body.code, dto::ErrorCode::ProductionBlocked);
        assert_eq!(
            body.issues
                .iter()
                .map(|i| i.rule.as_str())
                .collect::<Vec<_>>(),
            vec!["rights_expired", "delivery_unavailable"]
        );
        let response = error(PublicationStoreError::Conflict {
            version: 3,
            active_release_id: Some("private-only-id".into()),
        });
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = response.into_body().collect().await.unwrap().to_bytes();
        let body: dto::ApiError = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(body.current_revision, Some(3));
        assert!(!String::from_utf8_lossy(&bytes).contains("private-only-id"));
        assert_eq!(
            error(PublicationStoreError::Dependency(
                PublicationDependencyError::Unavailable
            ))
            .status(),
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            error(PublicationStoreError::Dependency(
                PublicationDependencyError::InvalidObservation
            ))
            .status(),
            StatusCode::INTERNAL_SERVER_ERROR
        );
    }
}
