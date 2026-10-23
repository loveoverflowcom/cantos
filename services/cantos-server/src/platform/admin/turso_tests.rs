//! Actual final-commit seam: the audit and revocation are real speculative Turso writes.
//! Only the trusted clock and verified role observation are supplied by the fixture.
use super::*;
use crate::test_support::ServerFixture;
use cantos_store_turso::{
    clock::{Clock, ClockFault},
    testing::FixedClock,
    value::UnixMicros,
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
const T0: i64 = 1_800_000_000_000_000;
struct FailOnce {
    time: Arc<FixedClock>,
    fail: AtomicBool,
}
impl Clock for FailOnce {
    fn now(&self) -> Result<UnixMicros, ClockFault> {
        if self.fail.swap(false, Ordering::SeqCst) {
            Err(ClockFault)
        } else {
            self.time.now()
        }
    }
}
async fn final_check(fail_clock: bool) {
    let time = FixedClock::new(T0);
    let clock = Arc::new(FailOnce {
        time: time.clone(),
        fail: AtomicBool::new(false),
    });
    let fixture = ServerFixture::with_clock(clock.clone()).await;
    fixture.seed_actors(&["minh", "other"]).await;
    fixture.exec("INSERT INTO actor_external_identities VALUES('https://fixture.invalid','subject','minh',0)",&[]).await.unwrap();
    let role = super::super::domain::map_groups(&["cantos_identity_operators".into()]).unwrap();
    let proof = RoleEvidence::verified(
        crate::platform::identity::domain::ExternalIdentityKey::new(
            "https://fixture.invalid",
            "subject",
        )
        .unwrap(),
        role,
        u64::try_from(T0 / 1_000_000).unwrap(),
        u64::try_from(T0 / 1_000_000 + 1).unwrap(),
    );
    let target = ActorId::new("other").unwrap();
    let mut conn = fixture.store().connection().await.unwrap();
    let tx = conn.transaction().await.unwrap();
    let permit = match admit(&tx, &proof, Action::RevokeAccess, Some(&target))
        .await
        .unwrap()
    {
        Admission::Allowed(p) => p,
        Admission::Denied { .. } => panic!("current role must be admitted"),
    };
    let correlation = Uuid::new_v4();
    let id = audit(
        &tx,
        Some(permit.actor()),
        permit.action(),
        Some(&target),
        "allowed",
        correlation,
    )
    .await
    .unwrap();
    record_revocation(&tx, &permit, &target, Uuid::new_v4(), id)
        .await
        .unwrap();
    if fail_clock {
        clock.fail.store(true, Ordering::SeqCst);
    } else {
        time.advance(1_000_000);
    }
    let result = commit_authorized(tx, &permit, fixture.store(), Some(&target), correlation).await;
    assert_eq!(
        result.err(),
        Some(if fail_clock {
            AdminError::Unavailable
        } else {
            AdminError::StaleRole
        })
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM admin_operations", &[])
            .await,
        0
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM admin_access_revocations", &[])
            .await,
        0
    );
    let rows = fixture
        .rows("SELECT actor_id,outcome FROM admin_audit", &[])
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].get::<_, String>(0), "minh");
    assert_eq!(
        rows[0].get::<_, String>(1),
        if fail_clock { "unavailable" } else { "denied" }
    );
}
#[tokio::test]
async fn proof_expiring_during_required_audit_wait_rolls_back_effect_and_records_denial() {
    final_check(false).await;
}
#[tokio::test]
async fn final_clock_read_failure_rolls_back_success_and_audits_unavailable_with_known_actor() {
    final_check(true).await;
}

#[tokio::test]
async fn admin_revocation_reconciles_unknown_commit_without_duplicate_effect_or_success_audit() {
    for ack in [
        crate::platform::turso::Acknowledgement::LostAfterCommit,
        crate::platform::turso::Acknowledgement::LostAfterRollback,
    ] {
        let f = ServerFixture::with_clock(FixedClock::new(T0)).await;
        f.seed_actors(&["minh", "other"]).await;
        f.exec("INSERT INTO actor_external_identities VALUES('https://fixture.invalid','subject','minh',0)",&[]).await.unwrap();
        let proof = || {
            RoleEvidence::verified(
                crate::platform::identity::domain::ExternalIdentityKey::new(
                    "https://fixture.invalid",
                    "subject",
                )
                .unwrap(),
                super::super::domain::map_groups(&["cantos_identity_operators".into()]).unwrap(),
                u64::try_from(T0 / 1_000_000).unwrap(),
                u64::try_from(T0 / 1_000_000 + 600).unwrap(),
            )
        };
        let target = ActorId::new("other").unwrap();
        let operation = Uuid::new_v4();
        let mut faulty = f.store().clone();
        faulty.acknowledgement = ack;
        assert!(
            revoke(&faulty, proof(), target.clone(), operation, Uuid::new_v4())
                .await
                .is_err()
        );
        let receipt = revoke(
            f.store(),
            proof(),
            target.clone(),
            operation,
            Uuid::new_v4(),
        )
        .await
        .unwrap();
        assert_eq!(
            revoke(f.store(), proof(), target, operation, Uuid::new_v4())
                .await
                .unwrap(),
            receipt
        );
        assert_eq!(
            f.count("SELECT count(*) FROM admin_access_revocations", &[])
                .await,
            1
        );
        assert_eq!(
            f.count("SELECT count(*) FROM admin_operations", &[]).await,
            1
        );
        assert_eq!(
            f.count(
                "SELECT count(*) FROM admin_audit WHERE outcome='allowed'",
                &[]
            )
            .await,
            1
        );
    }
}
