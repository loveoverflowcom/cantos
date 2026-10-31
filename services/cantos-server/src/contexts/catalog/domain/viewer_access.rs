//! Audience decisions for ADR 0054 D01/D04/D05. No email is a runtime principal.
//! Publication, current rights/security and the Work gate precede the optional Unit gate.
use super::OwnerId;
use crate::platform::identity::domain::ActorId;

pub const PENDING_LIFETIME_US: i64 = 14 * 24 * 60 * 60 * 1_000_000;
pub const MAX_PENDING_PER_WORK: usize = 100;
pub const MAX_GRANTS_PER_WORK: usize = 1000;
pub const MAX_GROUPS_PER_OWNER: usize = 100;
pub const MAX_MEMBERS_PER_GROUP: usize = 1000;
pub const MAX_CLAIM_BATCH: usize = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccessError {
    Invalid,
    Stale { current: u64 },
    Exhausted,
    Bound,
    Duplicate,
    Unavailable,
    NotFound,
    Reused,
    Rights,
}

/// A group ID is opaque. Names have no authority, and every use also carries its owner.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct GroupId(String);
impl GroupId {
    pub fn parse(value: &str) -> Result<Self, AccessError> {
        let id = uuid::Uuid::parse_str(value).map_err(|_| AccessError::Invalid)?;
        if id.is_nil() || id.to_string() != value {
            return Err(AccessError::Invalid);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Principal {
    Actor(ActorId),
    Group { owner: OwnerId, id: GroupId },
}

#[derive(Clone, Debug)]
pub struct Grant {
    pub principal: Principal,
    pub expires_at_us: Option<i64>,
    pub revoked: bool,
}

#[derive(Clone, Debug)]
pub struct Membership {
    pub owner: OwnerId,
    pub group: GroupId,
    pub actor: ActorId,
    pub expires_at_us: Option<i64>,
    pub removed: bool,
    pub group_revoked: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Audience {
    Public,
    Restricted,
}

#[derive(Clone, Copy, Debug)]
pub struct Gate<'a> {
    pub owner: &'a OwnerId,
    pub audience: Audience,
    pub grants: &'a [Grant],
}

#[derive(Clone, Copy, Debug)]
pub struct Viewer<'a> {
    pub actor: &'a ActorId,
    /// Current session/Identity admission, not a cached entitlement.
    pub active: bool,
    pub memberships: &'a [Membership],
}

#[derive(Clone, Copy, Debug)]
pub struct WorkFacts<'a> {
    pub published: bool,
    pub owner_active: bool,
    pub archived: bool,
    pub rights: bool,
    pub gate: Gate<'a>,
}

#[derive(Clone, Copy, Debug)]
pub enum UnitAudience<'a> {
    Inherit,
    OverrideWithinParent(Gate<'a>),
}

fn live(expires: Option<i64>, now: i64) -> bool {
    now >= 0 && expires.is_none_or(|expiry| expiry > now)
}

pub fn audience_allows(gate: Gate<'_>, viewer: Option<Viewer<'_>>, now: i64) -> bool {
    if now < 0 || viewer.is_some_and(|viewer| !viewer.active) {
        return false;
    }
    if gate.audience == Audience::Public {
        return true;
    }
    let Some(viewer) = viewer else {
        return false;
    };
    gate.grants.iter().any(|grant| {
        if grant.revoked || !live(grant.expires_at_us, now) {
            return false;
        }
        match &grant.principal {
            Principal::Actor(actor) => actor == viewer.actor,
            Principal::Group { owner, id } => {
                owner == gate.owner
                    && viewer.memberships.iter().any(|member| {
                        &member.owner == owner
                            && &member.group == id
                            && &member.actor == viewer.actor
                            && !member.removed
                            && !member.group_revoked
                            && live(member.expires_at_us, now)
                    })
            }
        }
    })
}

pub fn can_view_work(work: WorkFacts<'_>, viewer: Option<Viewer<'_>>, now: i64) -> bool {
    work.published
        && work.owner_active
        && !work.archived
        && work.rights
        && audience_allows(work.gate, viewer, now)
}

pub fn can_view_unit(
    work: WorkFacts<'_>,
    unit_published: bool,
    unit_archived: bool,
    unit_rights: bool,
    audience: UnitAudience<'_>,
    viewer: Option<Viewer<'_>>,
    now: i64,
) -> bool {
    if !can_view_work(work, viewer, now) || !unit_published || unit_archived || !unit_rights {
        return false;
    }
    match audience {
        UnitAudience::Inherit => true,
        UnitAudience::OverrideWithinParent(gate) => {
            gate.owner == work.gate.owner && audience_allows(gate, viewer, now)
        }
    }
}

pub fn next_version(current: u64, expected: u64) -> Result<u64, AccessError> {
    if expected != current {
        return Err(AccessError::Stale { current });
    }
    if current >= i64::MAX as u64 {
        return Err(AccessError::Exhausted);
    }
    Ok(current + 1)
}

pub fn pending_expiry(now: i64) -> Result<i64, AccessError> {
    if now < 0 {
        return Err(AccessError::Invalid);
    }
    now.checked_add(PENDING_LIFETIME_US)
        .ok_or(AccessError::Exhausted)
}

/// A claim can consume only a currently live, unclaimed pending row. A receipt is historical;
/// replay never calls this transition and never re-inserts a revoked actor grant.
pub fn claimable(expires_at_us: i64, revoked: bool, claimed: bool, now: i64) -> bool {
    !revoked && !claimed && live(Some(expires_at_us), now)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn actor(id: &str) -> ActorId {
        ActorId::new(id).unwrap()
    }
    fn owner(id: &str) -> OwnerId {
        OwnerId::parse(id).unwrap()
    }
    #[test]
    fn work_and_unit_never_allow_a_child_bypass_or_unpublished_unit() {
        let owner = owner("owner");
        let actor = actor("viewer");
        let grants = [Grant {
            principal: Principal::Actor(actor.clone()),
            expires_at_us: Some(11),
            revoked: false,
        }];
        let viewer = Some(Viewer {
            actor: &actor,
            active: true,
            memberships: &[],
        });
        for work_public in [false, true] {
            for unit_public in [false, true] {
                for published in [false, true] {
                    let work = WorkFacts {
                        published: true,
                        owner_active: true,
                        archived: false,
                        rights: true,
                        gate: Gate {
                            owner: &owner,
                            audience: if work_public {
                                Audience::Public
                            } else {
                                Audience::Restricted
                            },
                            grants: &[],
                        },
                    };
                    let unit = UnitAudience::OverrideWithinParent(Gate {
                        owner: &owner,
                        audience: if unit_public {
                            Audience::Public
                        } else {
                            Audience::Restricted
                        },
                        grants: &grants,
                    });
                    assert_eq!(
                        can_view_unit(work, published, false, true, unit, viewer, 10),
                        work_public && published
                    );
                    assert_eq!(
                        can_view_unit(
                            work,
                            published,
                            false,
                            true,
                            UnitAudience::Inherit,
                            viewer,
                            10
                        ),
                        work_public && published
                    );
                }
            }
        }
    }
    #[test]
    fn expiry_revoke_disable_and_live_work_fences_deny() {
        let owner = owner("owner");
        let actor = actor("viewer");
        for revoked in [false, true] {
            for active in [false, true] {
                for now in [9, 10, 11] {
                    let grants = [Grant {
                        principal: Principal::Actor(actor.clone()),
                        expires_at_us: Some(10),
                        revoked,
                    }];
                    let gate = Gate {
                        owner: &owner,
                        audience: Audience::Restricted,
                        grants: &grants,
                    };
                    let viewer = Some(Viewer {
                        actor: &actor,
                        active,
                        memberships: &[],
                    });
                    assert_eq!(
                        audience_allows(gate, viewer, now),
                        !revoked && active && now < 10
                    );
                }
            }
        }
        for published in [false, true] {
            for archived in [false, true] {
                for rights in [false, true] {
                    for owner_active in [false, true] {
                        let work = WorkFacts {
                            published,
                            archived,
                            rights,
                            owner_active,
                            gate: Gate {
                                owner: &owner,
                                audience: Audience::Public,
                                grants: &[],
                            },
                        };
                        assert_eq!(
                            can_view_work(work, None, 10),
                            published && !archived && rights && owner_active
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn group_authority_never_crosses_owner_or_membership_fences() {
        let owner = owner("owner");
        let other = self::owner("other");
        let actor = actor("viewer");
        let group = GroupId::parse("00000000-0000-4000-8000-000000000001").unwrap();
        for foreign in [false, true] {
            for removed in [false, true] {
                for revoked in [false, true] {
                    for expiry in [9, 11] {
                        let members = [Membership {
                            owner: if foreign {
                                other.clone()
                            } else {
                                owner.clone()
                            },
                            group: group.clone(),
                            actor: actor.clone(),
                            expires_at_us: Some(expiry),
                            removed,
                            group_revoked: revoked,
                        }];
                        let grants = [Grant {
                            principal: Principal::Group {
                                owner: owner.clone(),
                                id: group.clone(),
                            },
                            expires_at_us: None,
                            revoked: false,
                        }];
                        assert_eq!(
                            audience_allows(
                                Gate {
                                    owner: &owner,
                                    audience: Audience::Restricted,
                                    grants: &grants
                                },
                                Some(Viewer {
                                    actor: &actor,
                                    active: true,
                                    memberships: &members
                                }),
                                10
                            ),
                            !foreign && !removed && !revoked && expiry > 10
                        );
                    }
                }
            }
        }
    }
    #[test]
    fn pending_boundaries_and_checked_policy_versions() {
        assert_eq!(pending_expiry(5), Ok(1_209_600_000_005));
        assert_eq!(pending_expiry(i64::MAX), Err(AccessError::Exhausted));
        assert!(!claimable(10, false, false, 10));
        assert!(!claimable(11, true, false, 10));
        assert!(!claimable(11, false, true, 10));
        assert!(claimable(11, false, false, 10));
        assert_eq!(next_version(7, 6), Err(AccessError::Stale { current: 7 }));
        assert_eq!(next_version(7, 7), Ok(8));
        assert_eq!(
            next_version(i64::MAX as u64, i64::MAX as u64),
            Err(AccessError::Exhausted)
        );
    }
}
