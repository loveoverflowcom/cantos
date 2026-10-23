//! Work-scoped delegation use cases (ADR 0028). Owner issuance, listing and revocation use the
//! Studio session lane. A delegated bearer is never converted into an actor session: each
//! request takes the Catalog clock first, resolves the grant and current actor facts under it,
//! asks the pure grant for a permit and re-resolves everything before commit. Revocation takes
//! the clock for update, so it is effective for every request admitted after it commits.
use crate::contract::studio as dto;

use super::content::{
    append_authorized, fingerprint_append, history_page, AppendContent, Appended, ContentWriter,
};
use super::queries::units_page;
use super::{
    authorize, current_actor, page_limit, unit, work, CatalogCursor, CatalogPage, CatalogStoreError,
};
use crate::contexts::catalog::{
    domain::{
        content::{ContentRevision, ContentRevisionId},
        delegation::{
            delegated_content_admissible, CurrentFacts, DelegatedAction, DelegationDenial,
            DelegationError, DelegationGrant, DelegationGrantId, DelegationSecret, GrantRequest,
        },
        CatalogError, ContentUnit, ContentUnitId, Lifecycle, OwnerId, Work, WorkId,
    },
    postgres::{self, delegation as sql},
};
use crate::contexts::editorial::{Access, Action};
use crate::platform::store::{authenticate, normalized_uuid, LockMode, Store, StoreError, Tx};

/// A grant with its current revocation state, for the owner's management views.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DelegationView {
    pub grant: DelegationGrant,
    pub revoked_at: Option<i64>,
}

/// `secret` exists only on the response that created the grant.
#[derive(Debug)]
pub struct IssuedDelegation {
    pub view: DelegationView,
    pub secret: Option<DelegationSecret>,
}

#[path = "delegation_wire.rs"]
pub mod wire;

fn field(path: &str, rule: &str) -> dto::FieldIssue {
    dto::FieldIssue {
        path: path.into(),
        rule: rule.into(),
    }
}

fn view(record: sql::GrantRecord) -> DelegationView {
    DelegationView {
        grant: record.grant,
        revoked_at: record.revoked_at,
    }
}

/// Grant management is human-owner-only: members, even editors, cannot delegate.
async fn owner_only(tx: &Tx<'_>, actor: &str, work: &Work) -> Result<(), CatalogStoreError> {
    if authorize(tx, actor, work, Action::Read).await? != Access::Owner {
        return Err(StoreError::Forbidden.into());
    }
    Ok(())
}

fn issue_error(error: DelegationError) -> CatalogStoreError {
    let issue = match error {
        DelegationError::SelfDelegation => field("delegate_actor_id", "self_delegation"),
        DelegationError::InvalidLifetime => field("expires_at", "invalid_expiry"),
        DelegationError::InvalidId
        | DelegationError::UnknownAction
        | DelegationError::NoActions
        | DelegationError::DuplicateAction
        | DelegationError::InvalidToken => return StoreError::CorruptRevision.into(),
    };
    StoreError::InvalidRequestFields(vec![issue]).into()
}

/// Lifecycle denials make the credential unusable, a foreign resource is indistinguishable
/// from a missing one, and a missing action or link refuses only this request.
fn denied(denial: DelegationDenial) -> CatalogStoreError {
    match denial {
        DelegationDenial::Revoked
        | DelegationDenial::NotYetValid
        | DelegationDenial::Expired
        | DelegationDenial::GrantorInactive
        | DelegationDenial::DelegateInactive
        | DelegationDenial::GrantorNotOwner
        | DelegationDenial::WorkArchived => StoreError::Unauthenticated,
        DelegationDenial::OutsideWork => StoreError::NotFound,
        DelegationDenial::ActionNotGranted | DelegationDenial::LinkNotDelegated => {
            StoreError::Forbidden
        }
    }
    .into()
}

/// Current facts for one grant. Both actors are share-locked, so deactivation waits for this
/// transaction; revocation waits on the Catalog clock the caller already holds.
async fn current_facts(tx: &Tx<'_>, record: &sql::GrantRecord) -> Result<CurrentFacts, StoreError> {
    let grantor_active = sql::actor_active(tx, record.grant.grantor()).await?;
    let delegate_active = sql::actor_active(tx, record.grant.delegate()).await?;
    Ok(CurrentFacts {
        now: postgres::now(tx).await?,
        revoked: record.revoked_at.is_some(),
        grantor_active,
        delegate_active,
    })
}

/// Resolves and admits the credential. The clock must already be held by the caller.
async fn admitted(
    tx: &Tx<'_>,
    secret: &DelegationSecret,
) -> Result<(DelegationGrant, CurrentFacts), CatalogStoreError> {
    let record = sql::by_token(tx, &secret.lookup_hash())
        .await?
        .ok_or(StoreError::Unauthenticated)?;
    let facts = current_facts(tx, &record).await?;
    record.grant.admit(&facts).map_err(denied)?;
    Ok((record.grant, facts))
}

/// Final check before commit: the same grant, re-resolved with a fresh clock reading.
async fn recheck(
    tx: &Tx<'_>,
    secret: &DelegationSecret,
    grant: &DelegationGrant,
    work: &Work,
) -> Result<(), CatalogStoreError> {
    let (current, facts) = admitted(tx, secret).await?;
    if &current != grant {
        return Err(StoreError::CorruptRevision.into());
    }
    current.admit_work(&facts, work).map_err(denied)
}

async fn unit_and_work(
    tx: &Tx<'_>,
    id: &ContentUnitId,
    lock: LockMode,
) -> Result<(ContentUnit, Work), CatalogStoreError> {
    let unit = unit(tx, id).await?;
    let work = work(tx, unit.work_id(), lock).await?;
    Ok((unit, work))
}

impl Store {
    pub async fn issue_delegation(
        &self,
        token: &str,
        operation: &str,
        request: GrantRequest,
    ) -> Result<IssuedDelegation, CatalogStoreError> {
        let operation = normalized_uuid(operation)?;
        let request_digest = request.digest();
        // Minted outside the transaction; an unused secret (replay or rollback) is dropped.
        let mut bytes = [0; 32];
        getrandom::fill(&mut bytes).map_err(|_| StoreError::Unavailable)?;
        let secret = DelegationSecret::from_random(bytes);
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Update).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, &request.work_id, LockMode::Share).await?;
        owner_only(&tx, &actor, &work).await?;
        if let Some(prior) = sql::by_operation(&tx, &actor, &operation).await? {
            if prior.request_digest != request_digest {
                return Err(CatalogStoreError::OperationReused);
            }
            authenticate(&tx, token).await?;
            tx.commit().await.map_err(StoreError::from)?;
            return Ok(IssuedDelegation {
                view: view(prior),
                secret: None,
            });
        }
        if work.lifecycle() == Lifecycle::Archived {
            return Err(CatalogError::WorkArchived.into());
        }
        if let Some(existing) = sql::by_id(&tx, &request.id).await? {
            return Err(if existing.grant.grantor().as_str() == actor {
                CatalogStoreError::DuplicateId
            } else {
                StoreError::NotFound.into()
            });
        }
        let grantor = OwnerId::parse(&actor).map_err(|_| StoreError::CorruptRevision)?;
        if request.delegate != grantor && !sql::actor_active(&tx, &request.delegate).await? {
            return Err(StoreError::InvalidRequestFields(vec![field(
                "delegate_actor_id",
                "unavailable_actor",
            )])
            .into());
        }
        let now = postgres::now(&tx).await?;
        let grant = DelegationGrant::issue(request, grantor, now).map_err(issue_error)?;
        sql::insert(
            &tx,
            &grant,
            &secret.lookup_hash(),
            &operation,
            &request_digest,
        )
        .await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(IssuedDelegation {
            view: DelegationView {
                grant,
                revoked_at: None,
            },
            secret: Some(secret),
        })
    }

    pub async fn delegations(
        &self,
        token: &str,
        work_id: &WorkId,
        after: Option<&str>,
        limit: u64,
    ) -> Result<(Vec<DelegationView>, Option<String>), CatalogStoreError> {
        let limit = page_limit(limit)?;
        let after = after
            .map(|value| {
                DelegationGrantId::parse(value).map_err(|_| CatalogStoreError::InvalidCursor)
            })
            .transpose()?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, work_id, LockMode::Share).await?;
        owner_only(&tx, &actor, &work).await?;
        let mut items = sql::list(
            &tx,
            work_id,
            after.as_ref().map(DelegationGrantId::as_str),
            limit + 1,
        )
        .await?;
        let more = items.len() > limit as usize;
        items.truncate(limit as usize);
        let next = more
            .then(|| {
                items
                    .last()
                    .map(|record| record.grant.id().as_str().to_owned())
            })
            .flatten();
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok((items.into_iter().map(view).collect(), next))
    }

    /// Idempotent. Owners may revoke at any time, including after archive or expiry.
    pub async fn revoke_delegation(
        &self,
        token: &str,
        work_id: &WorkId,
        grant_id: &DelegationGrantId,
    ) -> Result<(), CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        authenticate(&tx, token).await?;
        // The same exclusive fence every delegated request shares, so no admission straddles it.
        postgres::lock_clock(&tx, LockMode::Update).await?;
        let actor = current_actor(&tx, token).await?;
        let work = work(&tx, work_id, LockMode::Share).await?;
        owner_only(&tx, &actor, &work).await?;
        let record = sql::by_id(&tx, grant_id)
            .await?
            .filter(|record| record.grant.work_id() == work_id)
            .ok_or(StoreError::NotFound)?;
        sql::revoke(&tx, record.grant.id(), &actor, postgres::now(&tx).await?).await?;
        authenticate(&tx, token).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(())
    }

    /// The presented grant's own scope, so a delegate can discover its Work and actions.
    pub async fn delegated_grant(
        &self,
        secret: &DelegationSecret,
    ) -> Result<DelegationView, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        let work = work(&tx, grant.work_id(), LockMode::Share).await?;
        grant.admit_work(&facts, &work).map_err(denied)?;
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(DelegationView {
            grant,
            revoked_at: None,
        })
    }

    pub async fn delegated_work(
        &self,
        secret: &DelegationSecret,
        id: &WorkId,
    ) -> Result<Work, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        let work = work(&tx, id, LockMode::Share).await?;
        grant
            .authorize(&facts, &work, DelegatedAction::WorkRead)
            .map_err(denied)?;
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(work)
    }

    pub async fn delegated_units(
        &self,
        secret: &DelegationSecret,
        id: &WorkId,
        limit: u64,
        include_archived: bool,
        cursor: Option<&CatalogCursor>,
    ) -> Result<CatalogPage<ContentUnit>, CatalogStoreError> {
        let limit = page_limit(limit)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        let epoch = postgres::lock_clock(&tx, LockMode::Share).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        let work = work(&tx, id, LockMode::Share).await?;
        grant
            .authorize(&facts, &work, DelegatedAction::WorkRead)
            .map_err(denied)?;
        // ':' cannot occur in an actor ID, so delegated cursors never collide with sessions.
        let scope = format!(
            "units:dlg:{}:{}:{include_archived}",
            grant.id().as_str(),
            id.as_str()
        );
        let page = units_page(&tx, id, &scope, epoch, limit, include_archived, cursor).await?;
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(page)
    }

    pub async fn delegated_unit(
        &self,
        secret: &DelegationSecret,
        id: &ContentUnitId,
    ) -> Result<ContentUnit, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        let (unit, work) = unit_and_work(&tx, id, LockMode::Share).await?;
        grant
            .authorize(&facts, &work, DelegatedAction::WorkRead)
            .map_err(denied)?;
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(unit)
    }

    pub async fn delegated_content(
        &self,
        secret: &DelegationSecret,
        id: &ContentUnitId,
        revision: Option<&ContentRevisionId>,
    ) -> Result<ContentRevision, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        let (_, work) = unit_and_work(&tx, id, LockMode::Share).await?;
        grant
            .authorize(&facts, &work, DelegatedAction::ContentRead)
            .map_err(denied)?;
        let value =
            postgres::content::load(&tx, id.as_str(), revision.map(ContentRevisionId::as_str))
                .await?
                .ok_or(StoreError::NotFound)?;
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(value)
    }

    pub async fn delegated_content_history(
        &self,
        secret: &DelegationSecret,
        id: &ContentUnitId,
        after: u64,
        limit: u64,
    ) -> Result<dto::ContentHistoryResponse, CatalogStoreError> {
        let limit = page_limit(limit)?;
        if after > i64::MAX as u64 {
            return Err(StoreError::InvalidRequest.into());
        }
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        postgres::lock_clock(&tx, LockMode::Share).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        let (_, work) = unit_and_work(&tx, id, LockMode::Share).await?;
        grant
            .authorize(&facts, &work, DelegatedAction::ContentRead)
            .map_err(denied)?;
        let reply = history_page(&tx, id, after, limit).await?;
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(reply)
    }

    /// The delegate is the recorded actor; the receipt links back to the grant and grantor.
    pub async fn delegated_append_content(
        &self,
        secret: &DelegationSecret,
        operation: &str,
        command: AppendContent,
    ) -> Result<dto::AppendContentResponse, CatalogStoreError> {
        let mut client = self.connection().await?;
        let tx = client.transaction().await.map_err(StoreError::from)?;
        postgres::lock_clock(&tx, LockMode::Update).await?;
        let (grant, facts) = admitted(&tx, secret).await?;
        // The grant is part of the replay identity: another grant cannot replay this receipt.
        let (operation, fingerprint) = fingerprint_append(
            operation,
            &("delegated_append_content_v1", grant.id().as_str(), &command),
            &command,
        )?;
        let (unit, work) = unit_and_work(&tx, &command.unit_id, LockMode::Update).await?;
        let permit = grant
            .authorize(&facts, &work, DelegatedAction::ContentWrite)
            .map_err(denied)?;
        delegated_content_admissible(&command.details).map_err(denied)?;
        let actor = permit.delegate().as_str();
        let writer = ContentWriter {
            actor,
            may_change_rights: false,
        };
        let reply =
            match append_authorized(&tx, writer, &operation, &fingerprint, &work, &unit, command)
                .await?
            {
                Appended::Created(reply) => {
                    sql::record_lineage(&tx, permit.grant(), actor, &operation).await?;
                    reply
                }
                Appended::Replayed(reply) => {
                    if sql::lineage(&tx, actor, &operation).await?.as_deref()
                        != Some(permit.grant().as_str())
                    {
                        return Err(StoreError::CorruptRevision.into());
                    }
                    reply
                }
            };
        recheck(&tx, secret, &grant, &work).await?;
        tx.commit().await.map_err(StoreError::from)?;
        Ok(reply)
    }
}
