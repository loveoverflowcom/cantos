//! Raw delegation transport admission and response formatting. Pure: it parses DTOs and bearer
//! text into domain values and formats immutable outcomes; current access is decided elsewhere.
use crate::contract::studio as dto;

use super::{DelegationView, IssuedDelegation};
pub use crate::contexts::catalog::domain::delegation::{
    DelegationGrantId, DelegationSecret, GrantRequest,
};
use crate::contexts::catalog::domain::{
    delegation::{DelegatedAction, DelegatedActions, DelegationError},
    OwnerId, WorkId,
};

fn field(path: &str, rule: &str) -> dto::FieldIssue {
    dto::FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

/// Raw issuance admission. Unknown actions are already refused by the closed DTO enum.
pub fn admit_issue(
    work_id: &str,
    request: dto::IssueDelegationRequest,
) -> Result<(String, GrantRequest), Vec<dto::FieldIssue>> {
    let mut issues = Vec::new();
    let work = WorkId::parse(work_id).map_err(|_| issues.push(field("work_id", "invalid_uuid")));
    let id =
        DelegationGrantId::parse(&request.id).map_err(|_| issues.push(field("id", "invalid_uuid")));
    let delegate = OwnerId::parse(&request.delegate_actor_id)
        .map_err(|_| issues.push(field("delegate_actor_id", "invalid_actor")));
    let actions = DelegatedActions::new(request.actions.iter().map(|action| match action {
        dto::DelegationAction::WorkRead => DelegatedAction::WorkRead,
        dto::DelegationAction::ContentRead => DelegatedAction::ContentRead,
        dto::DelegationAction::ContentWrite => DelegatedAction::ContentWrite,
    }))
    .map_err(|error| {
        issues.push(field(
            "actions",
            match error {
                DelegationError::NoActions => "empty",
                _ => "duplicate_action",
            },
        ))
    });
    match (work, id, delegate, actions) {
        (Ok(work_id), Ok(id), Ok(delegate), Ok(actions)) => Ok((
            request.operation_id,
            GrantRequest {
                id,
                work_id,
                delegate,
                actions,
                expires_at: request.expires_at,
            },
        )),
        _ => Err(issues),
    }
}

/// `None` for anything that is not exactly a delegation credential.
pub fn bearer(token: &str) -> Option<DelegationSecret> {
    DelegationSecret::parse(token).ok()
}

pub fn grant_id(value: &str) -> Option<DelegationGrantId> {
    DelegationGrantId::parse(value).ok()
}

pub fn grant_response(view: &DelegationView) -> dto::DelegationGrantResponse {
    let grant = &view.grant;
    dto::DelegationGrantResponse {
        id: grant.id().as_str().into(),
        work_id: grant.work_id().as_str().into(),
        grantor_id: grant.grantor().as_str().into(),
        delegate_actor_id: grant.delegate().as_str().into(),
        actions: grant
            .actions()
            .iter()
            .map(|action| match action {
                DelegatedAction::WorkRead => dto::DelegationAction::WorkRead,
                DelegatedAction::ContentRead => dto::DelegationAction::ContentRead,
                DelegatedAction::ContentWrite => dto::DelegationAction::ContentWrite,
            })
            .collect(),
        issued_at: grant.issued_at(),
        expires_at: grant.expires_at(),
        revoked_at: view.revoked_at,
    }
}

/// `true` when this response created the grant and therefore carries its only secret reveal.
pub fn issued_response(issued: IssuedDelegation) -> (bool, dto::IssueDelegationResponse) {
    let token = issued.secret.map(|secret| secret.encode());
    (
        token.is_some(),
        dto::IssueDelegationResponse {
            grant: grant_response(&issued.view),
            token,
        },
    )
}
