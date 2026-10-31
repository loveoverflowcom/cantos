//! Candidate indexes and plain access facts. SQL narrows candidates; the domain grants access.
use super::super::domain::{viewer_access::*, OwnerId};
use super::metadata_publication::get;
use crate::platform::{
    identity::domain::ActorId,
    store::{StoreError, Tx},
};

pub(crate) struct AccessFacts {
    pub audience: Audience,
    pub grants: Vec<Grant>,
    pub members: Vec<Membership>,
    pub deadline_us: Option<i64>,
}

pub(crate) async fn facts(
    tx: &Tx<'_>,
    work: &str,
    owner: &OwnerId,
    actor: &ActorId,
) -> Result<AccessFacts, StoreError> {
    let policy = tx
        .query_opt(
            "SELECT audience,owner_id FROM catalog_viewer_policies WHERE work_id=?1",
            &[&work],
        )
        .await?;
    let audience = match policy {
        None => Audience::Public,
        Some(row) => {
            if get::<String>(&row, "owner_id")? != owner.as_str() {
                return Err(StoreError::CorruptRevision);
            }
            match get::<String>(&row, "audience")?.as_str() {
                "public" => Audience::Public,
                "restricted" => Audience::Restricted,
                _ => return Err(StoreError::CorruptRevision),
            }
        }
    };
    let rows = tx.query("SELECT g.actor_id,g.group_id,g.expires_at_us,EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id) AS revoked FROM catalog_viewer_grants g WHERE g.work_id=?1 AND (g.expires_at_us IS NULL OR g.expires_at_us>?2) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id) ORDER BY g.id LIMIT 1001", &[&work,&tx.now_micros()]).await?;
    if rows.len() > MAX_GRANTS_PER_WORK {
        return Err(StoreError::CorruptRevision);
    }
    let mut deadline = None;
    let now = tx.now_micros();
    let mut deadline_at = |expiry: Option<i64>| {
        if let Some(at) = expiry.filter(|at| *at > now) {
            deadline = Some(deadline.map_or(at, |old: i64| old.min(at)));
        }
    };
    let mut grants = Vec::with_capacity(rows.len());
    for row in rows {
        let person: Option<String> = get(&row, "actor_id")?;
        let group: Option<String> = get(&row, "group_id")?;
        let principal = match (person, group) {
            (Some(id), None) => {
                Principal::Actor(ActorId::new(id).map_err(|_| StoreError::CorruptRevision)?)
            }
            (None, Some(id)) => Principal::Group {
                owner: owner.clone(),
                id: GroupId::parse(&id).map_err(|_| StoreError::CorruptRevision)?,
            },
            _ => return Err(StoreError::CorruptRevision),
        };
        let expires_at_us = get(&row, "expires_at_us")?;
        deadline_at(expires_at_us);
        grants.push(Grant {
            principal,
            expires_at_us,
            revoked: get::<i64>(&row, "revoked")? != 0,
        });
    }
    let rows = tx.query("SELECT m.group_id,m.expires_at_us,m.removed,g.revoked FROM catalog_viewer_members m JOIN catalog_viewer_groups g ON g.id=m.group_id AND g.owner_id=m.owner_id WHERE m.actor_id=?1 AND m.owner_id=?2 AND m.removed=0 AND g.revoked=0 AND (m.expires_at_us IS NULL OR m.expires_at_us>?3) ORDER BY m.group_id LIMIT 101", &[&actor.as_str(), &owner.as_str(),&tx.now_micros()]).await?;
    if rows.len() > MAX_GROUPS_PER_OWNER {
        return Err(StoreError::CorruptRevision);
    }
    let mut members = Vec::with_capacity(rows.len());
    for row in rows {
        let expires_at_us = get(&row, "expires_at_us")?;
        deadline_at(expires_at_us);
        members.push(Membership {
            owner: owner.clone(),
            group: GroupId::parse(&get::<String>(&row, "group_id")?)
                .map_err(|_| StoreError::CorruptRevision)?,
            actor: actor.clone(),
            expires_at_us,
            removed: get::<i64>(&row, "removed")? != 0,
            group_revoked: get::<i64>(&row, "revoked")? != 0,
        });
    }
    Ok(AccessFacts {
        audience,
        grants,
        members,
        deadline_us: deadline,
    })
}

/// A conservative process-local cursor/permit fence. Every publication/policy/group command
/// advances a durable version. Time-only expiry changes live counts. No aggregate grants access.
/// Any unrelated change may request restart, which is safer than continuing a stale walk.
pub(crate) async fn scope(tx: &Tx<'_>) -> Result<String, StoreError> {
    let row = tx.query_one("SELECT (SELECT coalesce(sum(version),0) FROM catalog_metadata_publication_pointers) AS publication, (SELECT coalesce(sum(version),0) FROM catalog_viewer_policies) AS policy, (SELECT coalesce(sum(version),0) FROM catalog_viewer_groups) AS groups, (SELECT count(*) FROM catalog_viewer_grants WHERE expires_at_us IS NOT NULL AND expires_at_us<=?1) AS expired_grants, (SELECT count(*) FROM catalog_viewer_members WHERE expires_at_us IS NOT NULL AND expires_at_us<=?1) AS expired_members", &[&tx.now_micros()]).await?;
    let values: Vec<i64> = [
        "publication",
        "policy",
        "groups",
        "expired_grants",
        "expired_members",
    ]
    .iter()
    .map(|name| get(&row, name))
    .collect::<Result<_, _>>()?;
    let bytes = serde_json::to_vec(&values).map_err(|_| StoreError::CorruptRevision)?;
    Ok(super::digest(&bytes))
}

/// Two indexable principal lanes, then dedup before keyset/LIMIT. Expiry/revoke checks here are
/// candidate reduction only; `facts` + `can_view_work/unit` are the final current authority.
pub(crate) async fn candidates(
    tx: &Tx<'_>,
    actor: &ActorId,
    after: Option<(i64, &str)>,
    limit: u8,
    kind: Option<&str>,
    language: Option<&str>,
    search: &str,
) -> Result<Vec<(String, i64)>, StoreError> {
    if !(1..=51).contains(&limit) {
        return Err(StoreError::InvalidRequest);
    }
    let (at, id) = after.map_or((None, None), |(at, id)| (Some(at), Some(id)));
    let rows = tx.query("SELECT DISTINCT p.work_id,p.first_visible_at_ms FROM (SELECT d.work_id FROM catalog_viewer_grants d WHERE d.actor_id=?1 AND (d.expires_at_us IS NULL OR d.expires_at_us>?5) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=d.id) UNION SELECT g.work_id FROM catalog_viewer_members m JOIN catalog_viewer_groups grp ON grp.id=m.group_id AND grp.owner_id=m.owner_id JOIN catalog_viewer_grants g ON g.group_id=m.group_id AND g.owner_id=m.owner_id WHERE m.actor_id=?1 AND m.removed=0 AND grp.revoked=0 AND (m.expires_at_us IS NULL OR m.expires_at_us>?5) AND (g.expires_at_us IS NULL OR g.expires_at_us>?5) AND NOT EXISTS(SELECT 1 FROM catalog_viewer_grant_revocations r WHERE r.grant_id=g.id)) c JOIN catalog_metadata_publication_pointers p ON p.work_id=c.work_id JOIN catalog_viewer_policies policy ON policy.work_id=p.work_id JOIN catalog_works w ON w.id=p.work_id JOIN catalog_metadata_generations generation ON generation.work_id=p.work_id AND generation.generation=p.generation WHERE p.lifecycle='active' AND policy.audience='restricted' AND w.lifecycle<>'archived' AND (?6 IS NULL OR generation.kind=?6) AND (?7 IS NULL OR generation.language=?7) AND (?8='' OR instr(generation.search_text,?8)>0) AND (?2 IS NULL OR p.first_visible_at_ms<?2 OR (p.first_visible_at_ms=?2 AND p.work_id>?3)) ORDER BY p.first_visible_at_ms DESC,p.work_id ASC LIMIT ?4", &[&actor.as_str(), &at, &id, &i64::from(limit),&tx.now_micros(),&kind,&language,&search]).await?;
    rows.iter()
        .map(|r| Ok((get(r, "work_id")?, get(r, "first_visible_at_ms")?)))
        .collect()
}
