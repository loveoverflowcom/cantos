//! Unit restrictions use the Work's policy CAS, event and receipt. No Unit publication pointer.
use super::*;
use crate::contexts::catalog::{
    application::viewer_access::ViewerAccessError,
    domain::{ContentUnitId, OwnerId},
    turso,
};
use std::collections::{BTreeMap, BTreeSet};

pub(crate) async fn owned_unit(tx: &Tx<'_>, work: &str, unit: &str) -> Result<()> {
    ContentUnitId::parse(unit).map_err(|_| AccessError::Invalid)?;
    let current = turso::load_unit(tx, unit)
        .await?
        .ok_or(AccessError::NotFound)?;
    if current.work_id().as_str() != work || current.lifecycle() == Lifecycle::Archived {
        return Err(AccessError::NotFound.into());
    }
    Ok(())
}

pub(crate) async fn state(tx: &Tx<'_>, work: &str, unit: &str) -> Result<UnitViewerAccessState> {
    owned_unit(tx, work, unit).await?;
    let version = policy(tx, work).await?.0;
    let row = tx
        .query_opt(
            "SELECT mode FROM catalog_unit_viewer_policies WHERE unit_id=?1 AND work_id=?2",
            &[&unit, &work],
        )
        .await?;
    let policy = match row
        .as_ref()
        .map(|r| get::<String>(r, "mode"))
        .transpose()?
        .as_deref()
    {
        None | Some("inherit") => UnitViewerPolicy::Inherit,
        Some("restricted") => {
            let rows = tx.query("SELECT actor_id,group_id,expires_at_us FROM catalog_unit_viewer_grants WHERE unit_id=?1 AND work_id=?2 ORDER BY principal_key LIMIT 1001", &[&unit, &work]).await?;
            if rows.len() > MAX_GRANTS_PER_WORK {
                return Err(StoreError::CorruptRevision.into());
            }
            let grants = rows
                .iter()
                .map(|r| {
                    Ok(UnitViewerGrant {
                        principal: principal(r)?,
                        expires_at_us: get(r, "expires_at_us")?,
                    })
                })
                .collect::<Result<_>>()?;
            UnitViewerPolicy::Restricted { grants }
        }
        _ => return Err(StoreError::CorruptRevision.into()),
    };
    Ok(UnitViewerAccessState {
        work_id: work.into(),
        unit_id: unit.into(),
        version,
        policy,
    })
}

fn principal(row: &Row) -> Result<ViewerPrincipal> {
    match (
        get::<Option<String>>(row, "actor_id")?,
        get::<Option<String>>(row, "group_id")?,
    ) {
        (Some(actor_id), None) => {
            ActorId::new(&actor_id).map_err(|_| StoreError::CorruptRevision)?;
            Ok(ViewerPrincipal::Actor { actor_id })
        }
        (None, Some(group_id)) => {
            GroupId::parse(&group_id).map_err(|_| StoreError::CorruptRevision)?;
            Ok(ViewerPrincipal::Group { group_id })
        }
        _ => Err(StoreError::CorruptRevision.into()),
    }
}

fn domain_grant(owner: &OwnerId, grant: &UnitViewerGrant) -> Result<Grant> {
    let principal = match &grant.principal {
        ViewerPrincipal::Actor { actor_id } => {
            Principal::Actor(ActorId::new(actor_id).map_err(|_| AccessError::Invalid)?)
        }
        ViewerPrincipal::Group { group_id } => Principal::Group {
            owner: owner.clone(),
            id: GroupId::parse(group_id)?,
        },
    };
    Ok(Grant {
        principal,
        expires_at_us: grant.expires_at_us,
        revoked: false,
    })
}

pub(super) async fn replace(
    tx: &Tx<'_>,
    owner: &OwnerWork,
    unit: &str,
    policy: &UnitViewerPolicy,
) -> Result<()> {
    let work = owner.work.id().as_str();
    owned_unit(tx, work, unit).await?;
    let (mode, grants) = match policy {
        UnitViewerPolicy::Inherit => ("inherit", &[][..]),
        UnitViewerPolicy::Restricted { grants } => ("restricted", grants.as_slice()),
    };
    let domain: Vec<_> = grants
        .iter()
        .map(|g| domain_grant(&owner.actor, g))
        .collect::<Result<_>>()?;
    validate_unit_grants(&domain, tx.now_micros())?;
    let other = count(
        tx,
        "SELECT count(*) AS n FROM catalog_unit_viewer_grants WHERE work_id=?1 AND unit_id<>?2",
        &[&work, &unit],
    )
    .await?;
    if other + grants.len() as i64 > MAX_GRANTS_PER_WORK as i64 {
        return Err(AccessError::Bound.into());
    }
    for g in &domain {
        match &g.principal {
            Principal::Actor(actor) => if !actor_active(tx, actor.as_str()).await? { return Err(AccessError::NotFound.into()); },
            Principal::Group { owner, id } => if tx.query_opt("SELECT id FROM catalog_viewer_groups WHERE id=?1 AND owner_id=?2 AND revoked=0", &[&id.as_str(),&owner.as_str()]).await?.is_none() { return Err(AccessError::NotFound.into()); },
        }
    }
    if tx
        .query_opt(
            "SELECT unit_id FROM catalog_unit_viewer_policies WHERE unit_id=?1",
            &[&unit],
        )
        .await?
        .is_some()
    {
        tx.execute("UPDATE catalog_unit_viewer_policies SET mode=?1 WHERE unit_id=?2 AND work_id=?3 AND owner_id=?4", &[&mode,&unit,&work,&owner.actor.as_str()]).await?;
    } else {
        tx.execute("INSERT INTO catalog_unit_viewer_policies(unit_id,work_id,owner_id,mode) VALUES(?1,?2,?3,?4)", &[&unit,&work,&owner.actor.as_str(),&mode]).await?;
    }
    // These rows are current projections. The caller seals the full replacement in the
    // immutable Work event and operation receipt in this same write unit.
    tx.execute(
        "DELETE FROM catalog_unit_viewer_grants WHERE unit_id=?1",
        &[&unit],
    )
    .await?;
    let values: Vec<_> = grants.iter().map(|g| match &g.principal {
        ViewerPrincipal::Actor { actor_id } => serde_json::json!({"key":format!("actor:{actor_id}"),"actor":actor_id,"group":null,"expiry":g.expires_at_us}),
        ViewerPrincipal::Group { group_id } => serde_json::json!({"key":format!("group:{group_id}"),"actor":null,"group":group_id,"expiry":g.expires_at_us}),
    }).collect();
    tx.execute("INSERT INTO catalog_unit_viewer_grants(unit_id,work_id,owner_id,principal_key,actor_id,group_id,expires_at_us) SELECT ?1,?2,?3,json_extract(value,'$.key'),json_extract(value,'$.actor'),json_extract(value,'$.group'),json_extract(value,'$.expiry') FROM json_each(?4)", &[&unit,&work,&owner.actor.as_str(),&json(&values)?]).await?;
    Ok(())
}

pub(crate) struct Rules {
    restricted: BTreeMap<String, Vec<Grant>>,
    pub deadline_us: Option<i64>,
}
impl Rules {
    pub fn policy(&self, unit: &str) -> UnitViewerPolicy {
        match self.restricted.get(unit) {
            None => UnitViewerPolicy::Inherit,
            Some(grants) => UnitViewerPolicy::Restricted {
                grants: grants
                    .iter()
                    .map(|g| UnitViewerGrant {
                        principal: match &g.principal {
                            Principal::Actor(actor) => ViewerPrincipal::Actor {
                                actor_id: actor.as_str().into(),
                            },
                            Principal::Group { id, .. } => ViewerPrincipal::Group {
                                group_id: id.as_str().into(),
                            },
                        },
                        expires_at_us: g.expires_at_us,
                    })
                    .collect(),
            },
        }
    }
    pub fn audience<'a>(&'a self, unit: &str, owner: &'a OwnerId) -> UnitAudience<'a> {
        match self.restricted.get(unit) {
            None => UnitAudience::Inherit,
            Some(grants) => UnitAudience::OverrideWithinParent(Gate {
                owner,
                audience: Audience::Restricted,
                grants,
            }),
        }
    }
}

/// Two bounded statements per Work, not a policy/member query per chapter. The caller applies
/// the common Work AND Unit kernel before counting, ranking, hydrating or choosing a page.
pub(crate) async fn rules(
    tx: &Tx<'_>,
    work: &str,
    owner: &OwnerId,
) -> std::result::Result<Rules, StoreError> {
    let rows = tx.query("SELECT unit_id,owner_id FROM catalog_unit_viewer_policies WHERE work_id=?1 AND mode='restricted' ORDER BY unit_id LIMIT 1001", &[&work]).await?;
    if rows.len() > 1000 {
        return Err(StoreError::CorruptRevision);
    }
    let mut restricted = BTreeMap::new();
    for row in rows {
        if get::<String>(&row, "owner_id").map_err(storage)? != owner.as_str() {
            return Err(StoreError::CorruptRevision);
        }
        restricted.insert(get::<String>(&row, "unit_id").map_err(storage)?, Vec::new());
    }
    let rows = tx.query("SELECT unit_id,actor_id,group_id,expires_at_us FROM catalog_unit_viewer_grants WHERE work_id=?1 ORDER BY unit_id,principal_key LIMIT 1001", &[&work]).await?;
    if rows.len() > MAX_GRANTS_PER_WORK {
        return Err(StoreError::CorruptRevision);
    }
    let mut deadline_us = None;
    for row in rows {
        let unit: String = get(&row, "unit_id").map_err(storage)?;
        let grant = UnitViewerGrant {
            principal: principal(&row).map_err(storage)?,
            expires_at_us: get(&row, "expires_at_us").map_err(storage)?,
        };
        if let Some(at) = grant.expires_at_us.filter(|at| *at > tx.now_micros()) {
            deadline_us = Some(deadline_us.map_or(at, |old: i64| old.min(at)));
        }
        restricted
            .get_mut(&unit)
            .ok_or(StoreError::CorruptRevision)?
            .push(domain_grant(owner, &grant).map_err(storage)?);
    }
    Ok(Rules {
        restricted,
        deadline_us,
    })
}
fn storage(_: ViewerAccessError) -> StoreError {
    StoreError::CorruptRevision
}

pub(crate) async fn public_hidden(
    tx: &Tx<'_>,
    work: &str,
) -> std::result::Result<BTreeSet<String>, StoreError> {
    let rows = tx.query("SELECT unit_id FROM catalog_unit_viewer_policies WHERE work_id=?1 AND mode<>'inherit' ORDER BY unit_id LIMIT 1001", &[&work]).await?;
    if rows.len() > 1000 {
        return Err(StoreError::CorruptRevision);
    }
    rows.iter()
        .map(|row| get(row, "unit_id").map_err(storage))
        .collect()
}
