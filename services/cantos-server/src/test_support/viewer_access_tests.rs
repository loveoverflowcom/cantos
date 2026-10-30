//! Actual Turso files and production control-plane entrypoints. Development Studio credentials
//! are explicit fixtures; they do not prove Identity admission. The separate two-process test does.
use super::{ServerFixture, WorkSeed};
use crate::contract::theatre::viewer_access::*;
use crate::{
    contexts::catalog::{
        domain::{viewer_access::AccessError, WorkId},
        public_work, ViewerAccessError,
    },
    contract::studio::*,
    platform::{store::StoreError, turso::Acknowledgement},
};
use cantos_store_turso::{clock::Clock, testing::FixedClock};
use uuid::Uuid;
const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn req(version: u64, change: ViewerAccessChange) -> ViewerAccessRequest {
    ViewerAccessRequest {
        operation_id: id(),
        expected_version: version,
        change,
    }
}
async fn world() -> (ServerFixture, String) {
    let f = ServerFixture::new().await;
    f.studio_caller("alice", ALICE).await;
    f.studio_caller("bob", BOB).await;
    f.seed_actors(&["viewer"]).await;
    let work = f
        .seed_work(None, &WorkSeed::podcast(0x652, "S06 synthetic", &[]))
        .await;
    f.publish_work_metadata(&work).await;
    (f, work.work_id)
}
async fn visible(f: &ServerFixture, work: &str) -> bool {
    let conn = f.store().connection().await.unwrap();
    let tx = conn.read().await.unwrap();
    public_work(&tx, &WorkId::parse(work).unwrap())
        .await
        .unwrap()
        .is_some()
}
#[tokio::test]
async fn viewer_audience_is_separate_from_metadata_and_restricted_serving_stays_closed() {
    let (f, w) = world().await;
    let before = f
        .store()
        .metadata_publication_state(ALICE, &w)
        .await
        .unwrap();
    assert!(visible(&f, &w).await);
    let change = req(
        0,
        ViewerAccessChange::SetAudience {
            audience: ViewerAudience::Restricted,
            confirm_metadata_rights: false,
        },
    );
    f.store()
        .viewer_access_execute(ALICE, &w, change)
        .await
        .unwrap();
    assert!(!visible(&f, &w).await);
    assert_eq!(
        serde_json::to_value(
            f.store()
                .metadata_publication_state(ALICE, &w)
                .await
                .unwrap()
        )
        .unwrap(),
        serde_json::to_value(before).unwrap()
    );
    let denied = req(
        1,
        ViewerAccessChange::SetAudience {
            audience: ViewerAudience::Public,
            confirm_metadata_rights: false,
        },
    );
    assert!(matches!(
        f.store().viewer_access_execute(ALICE, &w, denied).await,
        Err(ViewerAccessError::Access(AccessError::Rights))
    ));
    f.store()
        .viewer_access_execute(
            ALICE,
            &w,
            req(
                1,
                ViewerAccessChange::SetAudience {
                    audience: ViewerAudience::Public,
                    confirm_metadata_rights: true,
                },
            ),
        )
        .await
        .unwrap();
    assert!(visible(&f, &w).await);
}
#[tokio::test]
async fn viewer_grant_replay_after_revoke_and_restart_never_resurrects_and_payload_reuse_conflicts()
{
    let (mut f, w) = world().await;
    let grant_id = id();
    let request = req(
        0,
        ViewerAccessChange::GrantActor {
            grant_id: grant_id.clone(),
            actor_id: "viewer".into(),
            expires_at_us: None,
        },
    );
    let mut lost = f.store().clone();
    lost.acknowledgement = Acknowledgement::LostAfterCommit;
    assert!(matches!(
        lost.viewer_access_execute(ALICE, &w, request.clone()).await,
        Err(ViewerAccessError::Storage(StoreError::Unavailable))
    ));
    drop(lost);
    let original = f
        .store()
        .viewer_access_execute(ALICE, &w, request.clone())
        .await
        .unwrap();
    assert!(original.replayed);
    assert_eq!(original.version, 1);
    f.store()
        .viewer_access_execute(
            ALICE,
            &w,
            req(1, ViewerAccessChange::RevokeGrant { grant_id }),
        )
        .await
        .unwrap();
    let mut changed = request.clone();
    changed.expected_version = 2;
    assert!(matches!(
        f.store().viewer_access_execute(ALICE, &w, changed).await,
        Err(ViewerAccessError::Access(AccessError::Reused))
    ));
    f.restart().await;
    let replay = f
        .store()
        .viewer_access_execute(ALICE, &w, request)
        .await
        .unwrap();
    assert!(replay.replayed);
    assert_eq!(replay.version, 1);
    let current = f.store().viewer_access_state(ALICE, &w).await.unwrap();
    assert_eq!(current.version, 2);
    assert!(current.grants.is_empty());
    assert_eq!(
        f.count("SELECT count(*) FROM catalog_viewer_grants", &[])
            .await,
        1
    );
    assert_eq!(
        f.count("SELECT count(*) FROM catalog_viewer_operations", &[])
            .await,
        2
    );
}
#[tokio::test]
async fn viewer_policy_cas_has_one_winner_and_current_owner_only() {
    let (f, w) = world().await;
    let a = req(
        0,
        ViewerAccessChange::GrantActor {
            grant_id: id(),
            actor_id: "viewer".into(),
            expires_at_us: None,
        },
    );
    let b = req(
        0,
        ViewerAccessChange::SetAudience {
            audience: ViewerAudience::Restricted,
            confirm_metadata_rights: false,
        },
    );
    assert!(matches!(
        f.store().viewer_access_execute(BOB, &w, a.clone()).await,
        Err(ViewerAccessError::Storage(StoreError::NotFound))
    ));
    let (a, b) = tokio::join!(
        f.store().viewer_access_execute(ALICE, &w, a),
        f.store().viewer_access_execute(ALICE, &w, b)
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    let error = match (a, b) {
        (Err(error), _) | (_, Err(error)) => error,
        _ => panic!("one CAS contender must lose"),
    };
    assert!(matches!(
        error,
        ViewerAccessError::Access(AccessError::Stale { current: 1 })
    ));
    assert_eq!(
        f.count("SELECT count(*) FROM catalog_viewer_events", &[])
            .await,
        1
    );
}
#[tokio::test]
async fn viewer_groups_are_flat_owner_scoped_and_replaying_add_does_not_undo_removal() {
    let (f, w) = world().await;
    let group = id();
    let request = ViewerGroupRequest {
        operation_id: id(),
        expected_version: 0,
        change: ViewerGroupChange::Create {
            name: "Readers".into(),
        },
    };
    f.store()
        .viewer_group_execute(ALICE, &group, request.clone())
        .await
        .unwrap();
    assert!(matches!(
        f.store().viewer_group_execute(BOB, &group, request).await,
        Err(ViewerAccessError::Access(AccessError::NotFound))
    ));
    let add = ViewerGroupRequest {
        operation_id: id(),
        expected_version: 1,
        change: ViewerGroupChange::AddMember {
            actor_id: "viewer".into(),
            expires_at_us: None,
        },
    };
    f.store()
        .viewer_group_execute(ALICE, &group, add.clone())
        .await
        .unwrap();
    f.store()
        .viewer_access_execute(
            ALICE,
            &w,
            req(
                0,
                ViewerAccessChange::GrantGroup {
                    grant_id: id(),
                    group_id: group.clone(),
                    expires_at_us: None,
                },
            ),
        )
        .await
        .unwrap();
    f.store()
        .viewer_group_execute(
            ALICE,
            &group,
            ViewerGroupRequest {
                operation_id: id(),
                expected_version: 2,
                change: ViewerGroupChange::RemoveMember {
                    actor_id: "viewer".into(),
                },
            },
        )
        .await
        .unwrap();
    assert!(
        f.store()
            .viewer_group_execute(ALICE, &group, add)
            .await
            .unwrap()
            .replayed
    );
    let groups = f.store().viewer_groups(ALICE).await.unwrap();
    assert_eq!(groups.items[0].version, 3);
    assert!(groups.items[0].members.is_empty());
    assert!(f.store().viewer_groups(BOB).await.unwrap().items.is_empty());
    let foreign = id();
    f.store()
        .viewer_group_execute(
            BOB,
            &foreign,
            ViewerGroupRequest {
                operation_id: id(),
                expected_version: 0,
                change: ViewerGroupChange::Create {
                    name: "Readers".into(),
                },
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        f.store()
            .viewer_access_execute(
                ALICE,
                &w,
                req(
                    1,
                    ViewerAccessChange::GrantGroup {
                        grant_id: id(),
                        group_id: foreign.clone(),
                        expires_at_us: None
                    }
                )
            )
            .await,
        Err(ViewerAccessError::Access(AccessError::NotFound))
    ));
    let sql="INSERT INTO catalog_viewer_grants(id,work_id,owner_id,actor_id,group_id,created_at_us) VALUES(?1,?2,'bob',NULL,?3,0)";
    assert!(f
        .try_exec(sql, &[&id(), &w, &foreign])
        .await
        .unwrap_err()
        .is_integrity_violation());
}
#[tokio::test]
async fn viewer_expiry_and_schema_guards_are_real_turso_not_clock_strings() {
    let clock = FixedClock::new(1_700_000_000_000_000);
    let f = ServerFixture::with_clock(clock.clone()).await;
    f.studio_caller("alice", ALICE).await;
    f.seed_actors(&["viewer"]).await;
    let work = f
        .seed_work(None, &WorkSeed::podcast(0x653, "Expiry", &[]))
        .await;
    let until = clock.now().unwrap().get() + 1;
    let grant = id();
    f.store()
        .viewer_access_execute(
            ALICE,
            &work.work_id,
            req(
                0,
                ViewerAccessChange::GrantActor {
                    grant_id: grant.clone(),
                    actor_id: "viewer".into(),
                    expires_at_us: Some(until),
                },
            ),
        )
        .await
        .unwrap();
    assert_eq!(
        f.store()
            .viewer_access_state(ALICE, &work.work_id)
            .await
            .unwrap()
            .grants
            .len(),
        1
    );
    clock.set(until);
    assert!(f
        .store()
        .viewer_access_state(ALICE, &work.work_id)
        .await
        .unwrap()
        .grants
        .is_empty());
    assert_eq!(
        f.try_exec(
            "UPDATE catalog_viewer_grants SET actor_id='alice' WHERE id=?1",
            &[&grant]
        )
        .await
        .unwrap_err()
        .guard(),
        Some("viewer.immutable")
    );
    assert_eq!(f.try_exec("INSERT OR REPLACE INTO catalog_viewer_grants(id,work_id,owner_id,actor_id,group_id,expires_at_us,created_at_us) SELECT id,work_id,owner_id,'alice',NULL,NULL,created_at_us FROM catalog_viewer_grants WHERE id=?1",&[&grant]).await.unwrap_err().guard(),Some("viewer.immutable"));
    assert!(f
        .store()
        .claim_viewer_shares(ALICE, ClaimViewerSharesRequest { operation_id: id() })
        .await
        .is_err());
}

#[tokio::test]
async fn viewer_pending_schema_bounds_expiry_and_stable_binding_survive_a_reused_mailbox_fact() {
    use crate::contexts::catalog::claim_viewer_shares_fixture;
    use crate::platform::identity_auth::{
        credential::Channel,
        decision::{Epoch, ObservedSecurity, Subject},
        SessionRecord,
    };
    let clock = FixedClock::new(1_700_000_000_000_000);
    let f = ServerFixture::with_clock(clock.clone()).await;
    f.studio_caller("alice", ALICE).await;
    f.seed_actors(&["viewer", "other"]).await;
    let work = f
        .seed_work(None, &WorkSeed::podcast(0x654, "Bindings", &[]))
        .await;
    let now = clock.now().unwrap().get();
    let expires = now + crate::contexts::catalog::domain::viewer_access::PENDING_LIFETIME_US;
    let share = id();
    f.exec("INSERT INTO catalog_viewer_pending(id,work_id,owner_id,canonical_email,created_at_us,expires_at_us) VALUES(?1,?2,'alice','mail@example.test',?3,?4)",&[&share,&work.work_id,&now,&expires]).await.unwrap();
    let session = |actor: &str, subject: u8| SessionRecord {
        session_id: id(),
        actor_id: actor.into(),
        channel: Channel::Native,
        observed: ObservedSecurity {
            subject: Subject::from_bytes([subject; 16]).unwrap(),
            epoch: Epoch::from_bytes([2; 16]).unwrap(),
            version: 1,
        },
        authenticated_at: 0,
        absolute_expires: u64::MAX,
        idle_expires: u64::MAX,
        token_expires: u64::MAX,
    };
    let first = session("viewer", 1);
    let operation = id();
    let mut conn = f.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    let receipt = claim_viewer_shares_fixture(
        &tx,
        &first,
        ClaimViewerSharesRequest {
            operation_id: operation.clone(),
        },
        "mail@example.test",
    )
    .await
    .unwrap();
    assert_eq!(receipt.grants.len(), 1);
    tx.commit().await.unwrap();
    // Synthetic later verified-mailbox fact for a different subject: actual SQL refuses any
    // transfer. This does not claim that Identity implements changing/re-registering mailboxes.
    let second = session("other", 3);
    let mut conn = f.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    assert!(claim_viewer_shares_fixture(
        &tx,
        &second,
        ClaimViewerSharesRequest { operation_id: id() },
        "mail@example.test"
    )
    .await
    .unwrap()
    .grants
    .is_empty());
    tx.commit().await.unwrap();
    assert_eq!(
        f.rows("SELECT actor_id FROM catalog_viewer_grants", &[])
            .await
            .unwrap()[0]
            .try_get::<_, String>(0)
            .unwrap(),
        "viewer"
    );
    // Fresh pending issuance expires at the boundary and is not consumed into a grant.
    f.exec("INSERT INTO catalog_viewer_pending(id,work_id,owner_id,canonical_email,created_at_us,expires_at_us) VALUES(?1,?2,'alice','late@example.test',?3,?4)",&[&id(),&work.work_id,&now,&expires]).await.unwrap();
    clock.set(expires);
    let mut conn = f.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    assert!(claim_viewer_shares_fixture(
        &tx,
        &second,
        ClaimViewerSharesRequest { operation_id: id() },
        "late@example.test"
    )
    .await
    .unwrap()
    .grants
    .is_empty());
    tx.commit().await.unwrap();
    // Raw inserts prove the selected engine's 100-row guard without a recipient lookup.
    let now = clock.now().unwrap().get();
    let expires = now + crate::contexts::catalog::domain::viewer_access::PENDING_LIFETIME_US;
    for i in 0..100 {
        f.exec("INSERT INTO catalog_viewer_pending(id,work_id,owner_id,canonical_email,created_at_us,expires_at_us) VALUES(?1,?2,'alice',?3,?4,?5)",&[&id(),&work.work_id,&format!("reader{i}@example.test"),&now,&expires]).await.unwrap();
    }
    let error=f.try_exec("INSERT INTO catalog_viewer_pending(id,work_id,owner_id,canonical_email,created_at_us,expires_at_us) VALUES(?1,?2,'alice','overflow@example.test',?3,?4)",&[&id(),&work.work_id,&now,&expires]).await.unwrap_err();
    assert_eq!(error.guard(), Some("viewer.pending_bound"));
}

#[tokio::test]
async fn viewer_group_member_and_grant_limits_hold_on_the_real_engine() {
    let (f, w) = world().await;
    let mut group = String::new();
    for _ in 0..100 {
        let id = id();
        f.store()
            .viewer_group_execute(
                ALICE,
                &id,
                ViewerGroupRequest {
                    operation_id: self::id(),
                    expected_version: 0,
                    change: ViewerGroupChange::Create {
                        name: "Readers".into(),
                    },
                },
            )
            .await
            .unwrap();
        group = id;
    }
    assert!(matches!(
        f.store()
            .viewer_group_execute(
                ALICE,
                &id(),
                ViewerGroupRequest {
                    operation_id: id(),
                    expected_version: 0,
                    change: ViewerGroupChange::Create {
                        name: "Overflow".into()
                    }
                }
            )
            .await,
        Err(ViewerAccessError::Access(AccessError::Bound))
    ));
    let actors = (0..1001).map(|n| format!("member_{n}")).collect::<Vec<_>>();
    let text = serde_json::to_string(&actors).unwrap();
    f.exec(
        "INSERT INTO actors(id) SELECT value FROM json_each(?1)",
        &[&text],
    )
    .await
    .unwrap();
    let members = serde_json::to_string(&actors[..1000]).unwrap();
    f.exec("INSERT INTO catalog_viewer_members(owner_id,group_id,actor_id,expires_at_us,removed) SELECT 'alice',?1,value,NULL,0 FROM json_each(?2)",&[&group,&members]).await.unwrap();
    let listed = f.store().viewer_groups(ALICE).await.unwrap();
    assert_eq!(listed.items.len(), 100);
    assert_eq!(
        listed
            .items
            .iter()
            .find(|g| g.group_id == group)
            .unwrap()
            .members
            .len(),
        1000
    );
    assert!(matches!(
        f.store()
            .viewer_group_execute(
                ALICE,
                &group,
                ViewerGroupRequest {
                    operation_id: id(),
                    expected_version: 1,
                    change: ViewerGroupChange::AddMember {
                        actor_id: actors[1000].clone(),
                        expires_at_us: None
                    }
                }
            )
            .await,
        Err(ViewerAccessError::Access(AccessError::Bound))
    ));
    let raw=f.try_exec("INSERT INTO catalog_viewer_members(owner_id,group_id,actor_id,expires_at_us,removed) VALUES('alice',?1,?2,NULL,0)",&[&group,&actors[1000]]).await.unwrap_err();
    assert_eq!(raw.guard(), Some("viewer.member_bound"));
    let grants = actors[..1000]
        .iter()
        .map(|actor| serde_json::json!({"id":id(),"actor":actor}))
        .collect::<Vec<_>>();
    let grants = serde_json::to_string(&grants).unwrap();
    f.exec("INSERT INTO catalog_viewer_grants(id,work_id,owner_id,actor_id,group_id,expires_at_us,created_at_us) SELECT json_extract(value,'$.id'),?1,'alice',json_extract(value,'$.actor'),NULL,NULL,?2 FROM json_each(?3)",&[&w,&f.clock_reading().unwrap(),&grants]).await.unwrap();
    assert_eq!(
        f.store()
            .viewer_access_state(ALICE, &w)
            .await
            .unwrap()
            .grants
            .len(),
        1000
    );
    assert!(matches!(
        f.store()
            .viewer_access_execute(
                ALICE,
                &w,
                req(
                    0,
                    ViewerAccessChange::GrantActor {
                        grant_id: id(),
                        actor_id: actors[1000].clone(),
                        expires_at_us: None
                    }
                )
            )
            .await,
        Err(ViewerAccessError::Access(AccessError::Bound))
    ));
}
