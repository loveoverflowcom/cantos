//! Bounded private access projections and append-only receipts on the Server's single writer.
use super::super::{
    application::{
        owner_work::OwnerWork,
        viewer_access::{operation, Result},
    },
    domain::{viewer_access::*, Lifecycle},
};
use crate::contract::theatre::viewer_access::*;
use crate::{
    contract::studio::*,
    platform::{
        identity::domain::ActorId,
        identity_auth::SessionRecord,
        store::{
            actor_active, checked_number, FromColumn, Row, StoreError, Tx, WORK_OWNER_ACTIVE_FILTER,
        },
    },
};
use serde::{de::DeserializeOwned, Serialize};
use sha2::{Digest, Sha256};

fn get<T: FromColumn>(row: &Row, name: &str) -> Result<T> {
    row.try_get(name)
        .map_err(|_| StoreError::CorruptRevision.into())
}
fn json<T: Serialize>(v: &T) -> Result<String> {
    serde_json::to_string(v).map_err(|_| StoreError::CorruptRevision.into())
}
fn fingerprint<T: Serialize>(v: &T) -> Result<String> {
    Ok(format!("{:x}", Sha256::digest(json(v)?.as_bytes())))
}
async fn count(tx: &Tx<'_>, sql: &str, params: crate::platform::store::Params<'_>) -> Result<i64> {
    get(&tx.query_one(sql, params).await?, "n")
}
fn future(expiry: Option<i64>, now: i64) -> Result<()> {
    if expiry.is_some_and(|x| x <= now) {
        return Err(AccessError::Invalid.into());
    }
    Ok(())
}
async fn replay<T: DeserializeOwned>(
    tx: &Tx<'_>,
    actor: &str,
    op: &str,
    kind: &str,
    resource: &str,
    hash: &str,
) -> Result<Option<T>> {
    let Some(row)=tx.query_opt("SELECT kind,resource_id,fingerprint,response FROM catalog_viewer_operations WHERE actor_id=?1 AND operation_id=?2",&[&actor,&op]).await? else {return Ok(None);};
    if get::<String>(&row, "kind")? != kind
        || get::<String>(&row, "resource_id")? != resource
        || get::<String>(&row, "fingerprint")? != hash
    {
        return Err(AccessError::Reused.into());
    }
    Ok(Some(
        serde_json::from_str(&get::<String>(&row, "response")?)
            .map_err(|_| StoreError::CorruptRevision)?,
    ))
}
async fn record<T: Serialize>(
    tx: &Tx<'_>,
    actor: &str,
    op: &str,
    kind: &str,
    resource: &str,
    hash: &str,
    response: &T,
) -> Result<()> {
    tx.execute("INSERT INTO catalog_viewer_operations(actor_id,operation_id,kind,resource_id,fingerprint,response,recorded_at_us) VALUES(?1,?2,?3,?4,?5,?6,?7)",&[&actor,&op,&kind,&resource,&hash,&json(response)?,&tx.now_micros()]).await?;
    Ok(())
}
async fn policy(tx: &Tx<'_>, work: &str) -> Result<(u64, ViewerAudience)> {
    let Some(row) = tx
        .query_opt(
            "SELECT version,audience FROM catalog_viewer_policies WHERE work_id=?1",
            &[&work],
        )
        .await?
    else {
        return Ok((0, ViewerAudience::Public));
    };
    let raw: i64 = get(&row, "version")?;
    let version = u64::try_from(raw)
        .ok()
        .filter(|v| *v > 0)
        .ok_or(StoreError::CorruptRevision)?;
    let audience = match get::<String>(&row, "audience")?.as_str() {
        "public" => ViewerAudience::Public,
        "restricted" => ViewerAudience::Restricted,
        _ => return Err(StoreError::CorruptRevision.into()),
    };
    Ok((version, audience))
}
struct PolicyEvent<'a> {
    owner: &'a str,
    recorded_by: &'a str,
    work: &'a str,
    op: &'a str,
    version: u64,
    audience: ViewerAudience,
    action: &'a str,
    payload: &'a str,
}
async fn policy_event(tx: &Tx<'_>, event: PolicyEvent<'_>) -> Result<()> {
    let PolicyEvent {
        owner,
        recorded_by,
        work,
        op,
        version,
        audience,
        action,
        payload,
    } = event;
    let version = checked_number(version)?;
    let text = match audience {
        ViewerAudience::Public => "public",
        ViewerAudience::Restricted => "restricted",
    };
    tx.execute("INSERT INTO catalog_viewer_events(work_id,version,actor_id,operation_id,action,payload,recorded_at_us) VALUES(?1,?2,?3,?4,?5,?6,?7)",&[&work,&version,&recorded_by,&op,&action,&payload,&tx.now_micros()]).await?;
    if version == 1 {
        tx.execute("INSERT INTO catalog_viewer_policies(work_id,owner_id,version,audience) VALUES(?1,?2,?3,?4)",&[&work,&owner,&version,&text]).await?;
    } else {
        let changed=tx.execute("UPDATE catalog_viewer_policies SET version=?1,audience=?2 WHERE work_id=?3 AND version=?4",&[&version,&text,&work,&(version-1)]).await?;
        if changed != 1 {
            return Err(AccessError::Unavailable.into());
        }
    }
    Ok(())
}
pub(crate) async fn state(tx: &Tx<'_>, work: &str) -> Result<ViewerAccessState> {
    let (version, audience) = policy(tx, work).await?;
    let now = tx.now_micros();
    let rows=tx.query("SELECT g.id,g.actor_id,g.group_id,g.expires_at_us FROM catalog_viewer_grants g WHERE g.work_id=?1 AND (g.expires_at_us IS NULL OR g.expires_at_us>?2) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id) ORDER BY g.id LIMIT 1001",&[&work,&now]).await?;
    if rows.len() > MAX_GRANTS_PER_WORK {
        return Err(StoreError::CorruptRevision.into());
    }
    let mut grants = Vec::new();
    for row in rows {
        let actor: Option<String> = get(&row, "actor_id")?;
        let group: Option<String> = get(&row, "group_id")?;
        let principal = match (actor, group) {
            (Some(actor_id), None) => {
                ActorId::new(&actor_id).map_err(|_| StoreError::CorruptRevision)?;
                ViewerPrincipal::Actor { actor_id }
            }
            (None, Some(group_id)) => {
                GroupId::parse(&group_id).map_err(|_| StoreError::CorruptRevision)?;
                ViewerPrincipal::Group { group_id }
            }
            _ => return Err(StoreError::CorruptRevision.into()),
        };
        grants.push(ViewerGrant {
            grant_id: get(&row, "id")?,
            principal,
            expires_at_us: get(&row, "expires_at_us")?,
        });
    }
    let rows=tx.query("SELECT p.id,p.canonical_email,p.expires_at_us FROM catalog_viewer_pending p WHERE p.work_id=?1 AND p.expires_at_us>?2 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations r WHERE r.share_id=p.id) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_claims c WHERE c.share_id=p.id) ORDER BY p.id LIMIT 101",&[&work,&now]).await?;
    if rows.len() > MAX_PENDING_PER_WORK {
        return Err(StoreError::CorruptRevision.into());
    }
    let mut pending = Vec::new();
    for r in rows {
        pending.push(PendingViewerShare {
            share_id: get(&r, "id")?,
            email: get(&r, "canonical_email")?,
            expires_at_us: get(&r, "expires_at_us")?,
        });
    }
    Ok(ViewerAccessState {
        work_id: work.to_owned(),
        version,
        audience,
        grants,
        pending,
    })
}
async fn grant_bound(tx: &Tx<'_>, work: &str, now: i64) -> Result<()> {
    if count(tx,"SELECT count(*) AS n FROM catalog_viewer_grants g WHERE g.work_id=?1 AND (g.expires_at_us IS NULL OR g.expires_at_us>?2) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id)",&[&work,&now]).await? >= MAX_GRANTS_PER_WORK as i64 {return Err(AccessError::Bound.into());}
    Ok(())
}
async fn grant(
    tx: &Tx<'_>,
    owner: &str,
    work: &str,
    id: &str,
    actor: Option<&str>,
    group: Option<&str>,
    expiry: Option<i64>,
) -> Result<()> {
    operation(id)?;
    future(expiry, tx.now_micros())?;
    grant_bound(tx, work, tx.now_micros()).await?;
    if let Some(actor) = actor {
        ActorId::new(actor).map_err(|_| AccessError::Invalid)?;
        if !actor_active(tx, actor).await? {
            return Err(AccessError::NotFound.into());
        }
    }
    if let Some(group) = group {
        GroupId::parse(group)?;
        if tx
            .query_opt(
                "SELECT id FROM catalog_viewer_groups WHERE id=?1 AND owner_id=?2 AND revoked=0",
                &[&group, &owner],
            )
            .await?
            .is_none()
        {
            return Err(AccessError::NotFound.into());
        }
    }
    if count(tx,"SELECT count(*) AS n FROM catalog_viewer_grants g WHERE g.work_id=?1 AND (g.actor_id=?2 OR g.group_id=?3) AND (g.expires_at_us IS NULL OR g.expires_at_us>?4) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id)",&[&work,&actor,&group,&tx.now_micros()]).await?>0 {return Err(AccessError::Duplicate.into());}
    if tx
        .query_opt("SELECT id FROM catalog_viewer_grants WHERE id=?1", &[&id])
        .await?
        .is_some()
    {
        return Err(AccessError::Duplicate.into());
    }
    tx.execute("INSERT INTO catalog_viewer_grants(id,work_id,owner_id,actor_id,group_id,expires_at_us,created_at_us) VALUES(?1,?2,?3,?4,?5,?6,?7)",&[&id,&work,&owner,&actor,&group,&expiry,&tx.now_micros()]).await?;
    Ok(())
}
pub(crate) async fn policy_command(
    tx: &Tx<'_>,
    owner: &OwnerWork,
    request: &ViewerAccessRequest,
    rights: bool,
) -> Result<ViewerAccessReceipt> {
    let work = owner.work.id().as_str();
    let actor = owner.actor.as_str();
    let op = &request.operation_id;
    let hash = fingerprint(request)?;
    if let Some(mut receipt) =
        replay::<ViewerAccessReceipt>(tx, actor, op, "policy", work, &hash).await?
    {
        receipt.replayed = true;
        return Ok(receipt);
    }
    let (current, mut audience) = policy(tx, work).await?;
    let version = next_version(current, request.expected_version)?;
    let widening = matches!(
        request.change,
        ViewerAccessChange::SetAudience {
            audience: ViewerAudience::Public,
            ..
        }
    );
    if owner.work.lifecycle() == Lifecycle::Archived
        && !matches!(
            request.change,
            ViewerAccessChange::RevokeGrant { .. } | ViewerAccessChange::RevokeShare { .. }
        )
    {
        return Err(AccessError::NotFound.into());
    }
    if widening && !rights {
        return Err(AccessError::Rights.into());
    }
    match &request.change {
        ViewerAccessChange::SetAudience {
            audience: new,
            confirm_metadata_rights,
        } => {
            if *new == ViewerAudience::Public && !confirm_metadata_rights {
                return Err(AccessError::Rights.into());
            }
            audience = *new;
        }
        ViewerAccessChange::GrantActor {
            grant_id,
            actor_id,
            expires_at_us,
        } => {
            grant(
                tx,
                actor,
                work,
                grant_id,
                Some(actor_id),
                None,
                *expires_at_us,
            )
            .await?
        }
        ViewerAccessChange::GrantGroup {
            grant_id,
            group_id,
            expires_at_us,
        } => {
            grant(
                tx,
                actor,
                work,
                grant_id,
                None,
                Some(group_id),
                *expires_at_us,
            )
            .await?
        }
        ViewerAccessChange::RevokeGrant { grant_id } => {
            operation(grant_id)?;
            if tx.query_opt("SELECT g.id FROM catalog_viewer_grants g WHERE g.id=?1 AND g.work_id=?2 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id)",&[&grant_id,&work]).await?.is_none() {return Err(AccessError::NotFound.into());}
            tx.execute("INSERT INTO catalog_viewer_grant_revocations(grant_id,revoked_by,recorded_at_us) VALUES(?1,?2,?3)",&[&grant_id,&actor,&tx.now_micros()]).await?;
        }
        ViewerAccessChange::ShareMailbox { share_id, email } => {
            operation(share_id)?;
            let now = tx.now_micros();
            let expiry = pending_expiry(now)?;
            let existing = state(tx, work).await?;
            if existing.pending.len() >= MAX_PENDING_PER_WORK {
                return Err(AccessError::Bound.into());
            }
            if existing.pending.iter().any(|p| p.email == *email)
                || tx
                    .query_opt(
                        "SELECT id FROM catalog_viewer_pending WHERE id=?1",
                        &[&share_id],
                    )
                    .await?
                    .is_some()
            {
                return Err(AccessError::Duplicate.into());
            }
            tx.execute("INSERT INTO catalog_viewer_pending(id,work_id,owner_id,canonical_email,created_at_us,expires_at_us) VALUES(?1,?2,?3,?4,?5,?6)",&[&share_id,&work,&actor,&email,&now,&expiry]).await?;
        }
        ViewerAccessChange::RevokeShare { share_id } => {
            operation(share_id)?;
            if tx.query_opt("SELECT p.id FROM catalog_viewer_pending p WHERE p.id=?1 AND p.work_id=?2 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations r WHERE r.share_id=p.id)",&[&share_id,&work]).await?.is_none() {return Err(AccessError::NotFound.into());}
            tx.execute("INSERT INTO catalog_viewer_pending_revocations(share_id,revoked_by,recorded_at_us) VALUES(?1,?2,?3)",&[&share_id,&actor,&tx.now_micros()]).await?;
            // A pending row that already claimed still revokes its exact actor grant.
            tx.execute("INSERT INTO catalog_viewer_grant_revocations(grant_id,revoked_by,recorded_at_us) SELECT c.grant_id,?2,?3 FROM catalog_viewer_claims c WHERE c.share_id=?1 AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=c.grant_id)",&[&share_id,&actor,&tx.now_micros()]).await?;
        }
    }
    policy_event(
        tx,
        PolicyEvent {
            owner: actor,
            recorded_by: actor,
            work,
            op,
            version,
            audience,
            action: "owner_change",
            payload: &json(&request.change)?,
        },
    )
    .await?;
    let receipt = ViewerAccessReceipt {
        operation_id: op.clone(),
        work_id: work.to_owned(),
        version,
        replayed: false,
    };
    record(tx, actor, op, "policy", work, &hash, &receipt).await?;
    Ok(receipt)
}

pub(crate) async fn groups(tx: &Tx<'_>, owner: &str) -> Result<ViewerGroups> {
    let rows=tx.query("SELECT id,name,version,revoked FROM catalog_viewer_groups WHERE owner_id=?1 AND revoked=0 ORDER BY id LIMIT 101",&[&owner]).await?;
    if rows.len() > MAX_GROUPS_PER_OWNER {
        return Err(StoreError::CorruptRevision.into());
    }
    let mut items = Vec::new();
    for row in rows {
        let id: String = get(&row, "id")?;
        let rows=tx.query("SELECT actor_id,expires_at_us FROM catalog_viewer_members WHERE group_id=?1 AND removed=0 AND (expires_at_us IS NULL OR expires_at_us>?2) ORDER BY actor_id LIMIT 1001",&[&id,&tx.now_micros()]).await?;
        if rows.len() > MAX_MEMBERS_PER_GROUP {
            return Err(StoreError::CorruptRevision.into());
        }
        let members = rows
            .iter()
            .map(|r| {
                Ok(ViewerGroupMember {
                    actor_id: get(r, "actor_id")?,
                    expires_at_us: get(r, "expires_at_us")?,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        items.push(ViewerGroupState {
            group_id: id,
            name: get(&row, "name")?,
            version: u64::try_from(get::<i64>(&row, "version")?)
                .map_err(|_| StoreError::CorruptRevision)?,
            revoked: get::<i64>(&row, "revoked")? != 0,
            members,
        });
    }
    Ok(ViewerGroups { items })
}
pub(crate) async fn group_command(
    tx: &Tx<'_>,
    owner: &str,
    id: &str,
    request: &ViewerGroupRequest,
) -> Result<ViewerGroupReceipt> {
    let op = &request.operation_id;
    let hash = fingerprint(request)?;
    if let Some(row) = tx
        .query_opt(
            "SELECT owner_id FROM catalog_viewer_groups WHERE id=?1",
            &[&id],
        )
        .await?
    {
        if get::<String>(&row, "owner_id")? != owner {
            return Err(AccessError::NotFound.into());
        }
    }
    if let Some(mut receipt) =
        replay::<ViewerGroupReceipt>(tx, owner, op, "group", id, &hash).await?
    {
        receipt.replayed = true;
        return Ok(receipt);
    }
    let row = tx
        .query_opt(
            "SELECT version,revoked FROM catalog_viewer_groups WHERE id=?1 AND owner_id=?2",
            &[&id, &owner],
        )
        .await?;
    let current = match &row {
        Some(r) => {
            u64::try_from(get::<i64>(r, "version")?).map_err(|_| StoreError::CorruptRevision)?
        }
        None => 0,
    };
    let version = next_version(current, request.expected_version)?;
    if row
        .as_ref()
        .map(|r| get::<i64>(r, "revoked"))
        .transpose()?
        .is_some_and(|r| r != 0)
    {
        return Err(AccessError::NotFound.into());
    }
    match &request.change {
        ViewerGroupChange::Create { name } => {
            if current != 0 {
                return Err(AccessError::Duplicate.into());
            }
            if name.trim().is_empty() || name.len() > 200 {
                return Err(AccessError::Invalid.into());
            }
            if count(
                tx,
                "SELECT count(*) AS n FROM catalog_viewer_groups WHERE owner_id=?1 AND revoked=0",
                &[&owner],
            )
            .await?
                >= MAX_GROUPS_PER_OWNER as i64
            {
                return Err(AccessError::Bound.into());
            }
            tx.execute("INSERT INTO catalog_viewer_groups(id,owner_id,name,version,revoked) VALUES(?1,?2,?3,1,0)",&[&id,&owner,&name]).await?;
        }
        change => {
            if current == 0 {
                return Err(AccessError::NotFound.into());
            }
            match change {
                ViewerGroupChange::AddMember {
                    actor_id,
                    expires_at_us,
                } => {
                    ActorId::new(actor_id).map_err(|_| AccessError::Invalid)?;
                    future(*expires_at_us, tx.now_micros())?;
                    if !actor_active(tx, actor_id).await? {
                        return Err(AccessError::NotFound.into());
                    }
                    let member=tx.query_opt("SELECT removed,expires_at_us FROM catalog_viewer_members WHERE group_id=?1 AND actor_id=?2",&[&id,&actor_id]).await?;
                    if member
                        .as_ref()
                        .is_some_and(|m| get::<i64>(m, "removed").ok() == Some(0))
                    {
                        return Err(AccessError::Duplicate.into());
                    }
                    if count(tx,"SELECT count(*) AS n FROM catalog_viewer_members WHERE group_id=?1 AND removed=0",&[&id]).await?>=MAX_MEMBERS_PER_GROUP as i64 {return Err(AccessError::Bound.into());}
                    if member.is_some() {
                        tx.execute("UPDATE catalog_viewer_members SET removed=0,expires_at_us=?1 WHERE group_id=?2 AND actor_id=?3",&[&expires_at_us,&id,&actor_id]).await?;
                    } else {
                        tx.execute("INSERT INTO catalog_viewer_members(owner_id,group_id,actor_id,expires_at_us,removed) VALUES(?1,?2,?3,?4,0)",&[&owner,&id,&actor_id,&expires_at_us]).await?;
                    }
                }
                ViewerGroupChange::RemoveMember { actor_id } => {
                    ActorId::new(actor_id).map_err(|_| AccessError::Invalid)?;
                    if tx.execute("UPDATE catalog_viewer_members SET removed=1 WHERE group_id=?1 AND actor_id=?2 AND removed=0",&[&id,&actor_id]).await?!=1 {return Err(AccessError::NotFound.into());}
                }
                ViewerGroupChange::Revoke => {}
                ViewerGroupChange::Create { .. } => return Err(AccessError::Invalid.into()),
            }
            let revoked = i64::from(matches!(change, ViewerGroupChange::Revoke));
            if tx.execute("UPDATE catalog_viewer_groups SET version=?1,revoked=?2 WHERE id=?3 AND version=?4",&[&checked_number(version)?,&revoked,&id,&checked_number(current)?]).await?!=1 {return Err(AccessError::Unavailable.into());}
        }
    }
    tx.execute("INSERT INTO catalog_viewer_group_events(group_id,version,actor_id,operation_id,payload,recorded_at_us) VALUES(?1,?2,?3,?4,?5,?6)",&[&id,&checked_number(version)?,&owner,&op,&json(&request.change)?,&tx.now_micros()]).await?;
    let receipt = ViewerGroupReceipt {
        operation_id: op.clone(),
        group_id: id.to_owned(),
        version,
        replayed: false,
    };
    record(tx, owner, op, "group", id, &hash, &receipt).await?;
    Ok(receipt)
}

pub(crate) async fn claim(
    tx: &Tx<'_>,
    session: &SessionRecord,
    request: ClaimViewerSharesRequest,
    email: &str,
) -> Result<ClaimViewerSharesReceipt> {
    let actor = &session.actor_id;
    let op = &request.operation_id;
    let resource = session.observed.subject.hex();
    let hash = fingerprint(&request)?;
    if let Some(mut receipt) =
        replay::<ClaimViewerSharesReceipt>(tx, actor, op, "claim", &resource, &hash).await?
    {
        receipt.replayed = true;
        return Ok(receipt);
    }
    let now = tx.now_micros();
    let sql=format!("SELECT p.id,p.work_id,p.owner_id,p.expires_at_us FROM catalog_viewer_pending p JOIN catalog_works w ON w.id=p.work_id AND w.owner_id=p.owner_id WHERE p.canonical_email=?1 AND p.expires_at_us>?2 AND w.lifecycle<>'archived' AND {WORK_OWNER_ACTIVE_FILTER} AND NOT EXISTS(SELECT 1 FROM catalog_viewer_pending_revocations r WHERE r.share_id=p.id) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_claims c WHERE c.share_id=p.id) ORDER BY p.id LIMIT 100");
    let rows = tx.query(&sql, &[&email, &now]).await?;
    let mut grants = Vec::new();
    let mut affected = std::collections::BTreeMap::<String, String>::new();
    for row in rows {
        let id: String = get(&row, "id")?;
        let work: String = get(&row, "work_id")?;
        let owner: String = get(&row, "owner_id")?;
        if !claimable(get(&row, "expires_at_us")?, false, false, now) {
            return Err(AccessError::Unavailable.into());
        }
        let existing=tx.query_opt("SELECT g.id FROM catalog_viewer_grants g WHERE g.work_id=?1 AND g.actor_id=?2 AND (g.expires_at_us IS NULL OR g.expires_at_us>?3) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id) ORDER BY g.id LIMIT 1",&[&work,&actor,&now]).await?;
        let grant_id = if let Some(row) = existing {
            get::<String>(&row, "id")?
        } else {
            grant_bound(tx, &work, now).await?;
            tx.execute("INSERT INTO catalog_viewer_grants(id,work_id,owner_id,actor_id,group_id,expires_at_us,created_at_us) VALUES(?1,?2,?3,?4,NULL,NULL,?5)",&[&id,&work,&owner,&actor,&now]).await?;
            id.clone()
        };
        tx.execute("INSERT INTO catalog_viewer_claims(share_id,grant_id,actor_id,subject,instance_epoch,security_version,operation_id,recorded_at_us) VALUES(?1,?2,?3,?4,?5,?6,?7,?8)",&[&id,&grant_id,&actor,&session.observed.subject.bytes().to_vec(),&session.observed.epoch.bytes().to_vec(),&checked_number(session.observed.version)?,&op,&now]).await?;
        affected.insert(work.clone(), owner);
        grants.push(ClaimedViewerShare {
            work_id: work,
            grant_id,
        });
    }
    for (work, owner) in affected {
        let (version, audience) = policy(tx, &work).await?;
        let version = next_version(version, version)?;
        let scoped: Vec<_> = grants
            .iter()
            .filter(|grant| grant.work_id == work)
            .collect();
        policy_event(
            tx,
            PolicyEvent {
                owner: &owner,
                recorded_by: actor,
                work: &work,
                op,
                version,
                audience,
                action: "claim",
                payload: &json(&scoped)?,
            },
        )
        .await?;
    }
    let receipt = ClaimViewerSharesReceipt {
        operation_id: op.clone(),
        grants,
        replayed: false,
    };
    record(tx, actor, op, "claim", &resource, &hash, &receipt).await?;
    Ok(receipt)
}
