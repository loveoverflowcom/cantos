//! PostgreSQL effects only; policy is pure and all success/denial decisions require audit.
use super::domain::{authorize, Action, AdminError, AdminPermit, RoleEvidence};
use crate::platform::{
    identity::domain::ActorId,
    store::{Store, Tx},
};
use cantos_contracts::admin::{CapabilityResponse, RevokeAccessResponse};
use uuid::Uuid;

async fn audit(
    tx: &Tx<'_>,
    actor: Option<&ActorId>,
    action: Action,
    target: Option<&ActorId>,
    outcome: &str,
    correlation: Uuid,
) -> Result<Uuid, AdminError> {
    let id = Uuid::new_v4();
    tx.query_one(
        "SELECT admin_append_audit($1::text::uuid,$2,$3,$4,$5,$6::text::uuid)",
        &[
            &id.to_string(),
            &actor.map(ActorId::as_str),
            &action.as_str(),
            &target.map(ActorId::as_str),
            &outcome,
            &correlation.to_string(),
        ],
    )
    .await
    .map_err(|_| AdminError::AuditUnavailable)?;
    Ok(id)
}
pub(super) async fn deny(
    store: &Store,
    actor: Option<&ActorId>,
    action: Action,
    target: Option<&ActorId>,
    outcome: &str,
    correlation: Uuid,
) -> Result<(), AdminError> {
    let mut client = store
        .connection()
        .await
        .map_err(|_| AdminError::AuditUnavailable)?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| AdminError::AuditUnavailable)?;
    audit(&tx, actor, action, target, outcome, correlation).await?;
    tx.commit().await.map_err(|_| AdminError::AuditUnavailable)
}
enum Admission {
    Allowed(AdminPermit),
    Denied {
        actor: Option<ActorId>,
        error: AdminError,
    },
}

async fn admit(
    tx: &Tx<'_>,
    proof: &RoleEvidence,
    action: Action,
    target: Option<&ActorId>,
) -> Result<Admission, AdminError> {
    let row = tx
        .query_opt(
            "SELECT * FROM admin_identity_facts($1,$2,$3)",
            &[
                &proof.key().issuer(),
                &proof.key().subject(),
                &target.map(ActorId::as_str),
            ],
        )
        .await
        .map_err(|_| AdminError::Unavailable)?;
    let Some(row) = row else {
        return Ok(Admission::Denied {
            actor: None,
            error: AdminError::ActorMissing,
        });
    };
    let actor = ActorId::new(
        row.try_get::<_, String>(0)
            .map_err(|_| AdminError::CorruptRecord)?,
    )
    .map_err(|_| AdminError::CorruptRecord)?;
    let active = row.try_get(1).map_err(|_| AdminError::CorruptRecord)?;
    let revoked = row.try_get(2).map_err(|_| AdminError::CorruptRecord)?;
    let time = u64::try_from(
        row.try_get::<_, i64>(4)
            .map_err(|_| AdminError::CorruptRecord)?,
    )
    .map_err(|_| AdminError::CorruptRecord)?;
    let permit = match authorize(proof, &actor, active, revoked, action, time) {
        Ok(p) => p,
        Err(error) => {
            return Ok(Admission::Denied {
                actor: Some(actor),
                error,
            })
        }
    };
    if !row
        .try_get::<_, bool>(3)
        .map_err(|_| AdminError::CorruptRecord)?
    {
        return Ok(Admission::Denied {
            actor: Some(actor),
            error: AdminError::TargetMissing,
        });
    }
    Ok(Admission::Allowed(permit))
}
// A SQL failure aborts the transaction; log the failed decision in a separate transaction.
// This also records denials after failed admission without inventing a verified internal actor.
async fn denied(
    store: &Store,
    error: AdminError,
    action: Action,
    target: Option<&ActorId>,
    correlation: Uuid,
) -> AdminError {
    match deny(
        store,
        None,
        action,
        target,
        error.audit_outcome(),
        correlation,
    )
    .await
    {
        Ok(()) => error,
        Err(e) => e,
    }
}
/// Recheck after ALL SQL waits. Stale speculative effects/audit never become committed state.
/// The final check is the decision linearization; a delayed commit acknowledgement is not a
/// promise that the response is received before proof expiry.
async fn commit_authorized(
    tx: Tx<'_>,
    permit: &AdminPermit,
    store: &Store,
    target: Option<&ActorId>,
    correlation: Uuid,
) -> Result<(), AdminError> {
    let clock = tx
        .query_one(
            "SELECT floor(extract(epoch FROM clock_timestamp()))::bigint",
            &[],
        )
        .await;
    let clock = clock
        .map_err(|_| AdminError::Unavailable)
        .and_then(|row| {
            row.try_get::<_, i64>(0)
                .map_err(|_| AdminError::CorruptRecord)
        })
        .and_then(|time| u64::try_from(time).map_err(|_| AdminError::CorruptRecord));
    let time = match clock {
        Ok(time) => time,
        Err(error) => {
            tx.rollback().await.map_err(|_| AdminError::Unavailable)?;
            deny(
                store,
                Some(permit.actor()),
                permit.action(),
                target,
                error.audit_outcome(),
                correlation,
            )
            .await?;
            return Err(error);
        }
    };
    if let Err(error) = permit.ensure_current(time) {
        tx.rollback().await.map_err(|_| AdminError::Unavailable)?;
        deny(
            store,
            Some(permit.actor()),
            permit.action(),
            target,
            "denied",
            correlation,
        )
        .await?;
        return Err(error);
    }
    tx.commit().await.map_err(|_| AdminError::AuditUnavailable)
}

pub(super) async fn capabilities(
    store: &Store,
    proof: RoleEvidence,
    correlation: Uuid,
) -> Result<CapabilityResponse, AdminError> {
    let mut client = store
        .connection()
        .await
        .map_err(|_| AdminError::Unavailable)?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| AdminError::Unavailable)?;
    let permit = match admit(&tx, &proof, Action::Capabilities, None).await {
        Ok(Admission::Allowed(p)) => p,
        Ok(Admission::Denied { actor, error }) => {
            audit(
                &tx,
                actor.as_ref(),
                Action::Capabilities,
                None,
                "denied",
                correlation,
            )
            .await?;
            tx.commit()
                .await
                .map_err(|_| AdminError::AuditUnavailable)?;
            return Err(error);
        }
        Err(e) => {
            drop(tx);
            return Err(denied(store, e, Action::Capabilities, None, correlation).await);
        }
    };
    let audit_id = audit(
        &tx,
        Some(permit.actor()),
        permit.action(),
        None,
        "allowed",
        correlation,
    )
    .await?;
    commit_authorized(tx, &permit, store, None, correlation).await?;
    Ok(CapabilityResponse {
        actor_id: permit.actor().as_str().into(),
        role: permit.role().as_str().into(),
        source: "verified_issuer_group_and_current_introspection".into(),
        valid_until: proof.valid_until(),
        audit_id: audit_id.to_string(),
    })
}
pub(super) async fn revoke(
    store: &Store,
    proof: RoleEvidence,
    target: ActorId,
    operation: Uuid,
    correlation: Uuid,
) -> Result<RevokeAccessResponse, AdminError> {
    let mut client = store
        .connection()
        .await
        .map_err(|_| AdminError::Unavailable)?;
    let tx = client
        .transaction()
        .await
        .map_err(|_| AdminError::Unavailable)?;
    let permit = match admit(&tx, &proof, Action::RevokeAccess, Some(&target)).await {
        Ok(Admission::Allowed(p)) => p,
        Ok(Admission::Denied { actor, error }) => {
            audit(
                &tx,
                actor.as_ref(),
                Action::RevokeAccess,
                Some(&target),
                "denied",
                correlation,
            )
            .await?;
            tx.commit()
                .await
                .map_err(|_| AdminError::AuditUnavailable)?;
            return Err(error);
        }
        Err(e) => {
            drop(tx);
            return Err(denied(store, e, Action::RevokeAccess, Some(&target), correlation).await);
        }
    };
    let existing = tx
        .query_opt(
            "SELECT target_actor_id,audit_id::text FROM admin_operation($1,$2::text::uuid)",
            &[&permit.actor().as_str(), &operation.to_string()],
        )
        .await;
    let existing = match existing {
        Ok(row) => row,
        Err(_) => {
            tx.rollback().await.map_err(|_| AdminError::Unavailable)?;
            deny(
                store,
                Some(permit.actor()),
                permit.action(),
                Some(&target),
                "unavailable",
                correlation,
            )
            .await?;
            return Err(AdminError::Unavailable);
        }
    };
    let audit_id = if let Some(row) = existing {
        let old_target: String = row.try_get(0).map_err(|_| AdminError::CorruptRecord)?;
        if old_target != target.as_str() {
            audit(
                &tx,
                Some(permit.actor()),
                permit.action(),
                Some(&target),
                "conflict",
                correlation,
            )
            .await?;
            commit_authorized(tx, &permit, store, Some(&target), correlation).await?;
            return Err(AdminError::OperationReused);
        }
        audit(
            &tx,
            Some(permit.actor()),
            permit.action(),
            Some(&target),
            "replayed",
            correlation,
        )
        .await?;
        Uuid::parse_str(
            &row.try_get::<_, String>(1)
                .map_err(|_| AdminError::CorruptRecord)?,
        )
        .map_err(|_| AdminError::CorruptRecord)?
    } else {
        let id = audit(
            &tx,
            Some(permit.actor()),
            permit.action(),
            Some(&target),
            "allowed",
            correlation,
        )
        .await?;
        let effect = tx
            .query_one(
                "SELECT admin_revoke_access($1,$2::text::uuid,$3,$4::text::uuid)",
                &[
                    &permit.actor().as_str(),
                    &operation.to_string(),
                    &target.as_str(),
                    &id.to_string(),
                ],
            )
            .await;
        if effect.is_err() {
            tx.rollback().await.map_err(|_| AdminError::Unavailable)?;
            deny(
                store,
                Some(permit.actor()),
                permit.action(),
                Some(&target),
                "unavailable",
                correlation,
            )
            .await?;
            return Err(AdminError::Unavailable);
        }
        id
    };
    commit_authorized(tx, &permit, store, Some(&target), correlation).await?;
    Ok(RevokeAccessResponse {
        operation_id: operation.to_string(),
        target_actor_id: target.as_str().into(),
        revoked: true,
        audit_id: audit_id.to_string(),
    })
}
