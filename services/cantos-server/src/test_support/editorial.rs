//! Shared real-file Editorial/Ingestion fixture. Test support only: admitted Script IR and
//! synthetic actors/evidence seed a world; private transaction faults never enter a runtime build.
use super::ServerFixture;
use cantos_contracts::{ImportFormat, ImportMetadata, ImportRequest, SaveRevisionRequest};
use cantos_store_turso::testing::FixedClock;

pub const T0: i64 = 1_700_000_000_000_000;
pub const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
pub const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
pub const SCRIPT: &str = "00000000-0000-0000-0000-000000000001";
pub const FIXTURE: &str =
    include_str!("../../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");

pub fn request(base: u64) -> SaveRevisionRequest {
    SaveRevisionRequest {
        expected_revision: base,
        operation_id: uuid::Uuid::new_v4().to_string(),
        script_json: FIXTURE.into(),
    }
}

pub fn import() -> ImportRequest {
    ImportRequest {
        metadata: ImportMetadata {
            operation_id: uuid::Uuid::new_v4().to_string(),
            file_name: "Bản thảo.txt".into(),
            format: ImportFormat::Txt,
            reference: "Synthetic original".into(),
            rights_holder: None,
            permission_evidence: None,
            usage_scope: None,
        },
        original_bytes: "Người bạn đã về.\r\n".as_bytes().to_vec(),
    }
}

pub async fn fixture() -> ServerFixture {
    let fixture = ServerFixture::with_clock(FixedClock::new(T0 + 123456)).await;
    for (actor, token) in [("alice", ALICE), ("bob", BOB)] {
        fixture.studio_caller(actor, token).await;
        for (kind, id) in crate::script_ir::read_script(FIXTURE.as_bytes())
            .unwrap()
            .evidence_refs()
        {
            fixture
                .exec(
                    "INSERT INTO script_evidence VALUES(?1,?2,?3,'synthetic fixture')",
                    &[&actor, &kind, &id],
                )
                .await
                .unwrap();
        }
    }
    fixture
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::{
        store::{utc_timestamp, StoreError},
        turso::Acknowledgement,
    };
    use cantos_contracts::ReviewRequest;
    #[test]
    fn wire_timestamps_preserve_six_digits_and_refuse_unrepresentable_stored_time() {
        for (micros, expected) in [
            (0, "1970-01-01T00:00:00.000000Z"),
            (1_234_567, "1970-01-01T00:00:01.234567Z"),
            (253_402_300_799_999_999, "9999-12-31T23:59:59.999999Z"),
        ] {
            assert_eq!(utc_timestamp(micros).unwrap(), expected);
        }
        for micros in [-1, 253_402_300_800_000_000, i64::MAX] {
            assert!(matches!(
                utc_timestamp(micros),
                Err(StoreError::CorruptRevision)
            ));
        }
    }

    #[tokio::test]
    async fn lost_commit_acknowledgements_reconcile_each_receipt_without_duplicate_effects() {
        for acknowledgement in [
            Acknowledgement::LostAfterCommit,
            Acknowledgement::LostAfterRollback,
        ] {
            let mut fixture = fixture().await;
            let durable = matches!(acknowledgement, Acknowledgement::LostAfterCommit);
            let mut uncertain = fixture.store().clone();
            uncertain.acknowledgement = acknowledgement;
            let intent = request(0);
            assert!(matches!(
                uncertain.save(ALICE, SCRIPT, intent.clone()).await,
                Err(StoreError::Unavailable)
            ));
            assert_eq!(
                fixture
                    .count("SELECT count(*) FROM script_revisions", &[])
                    .await,
                i64::from(durable)
            );
            let saved = fixture
                .store()
                .save(ALICE, SCRIPT, intent.clone())
                .await
                .unwrap();
            assert_eq!(saved.revision, 1);
            assert_eq!(saved.accepted_at, "2023-11-14T22:13:20.123456Z");

            let review = ReviewRequest {
                revision: 1,
                operation_id: uuid::Uuid::new_v4().to_string(),
            };
            assert!(matches!(
                uncertain.review(ALICE, SCRIPT, review.clone()).await,
                Err(StoreError::Unavailable)
            ));
            assert_eq!(
                fixture
                    .count("SELECT count(*) FROM script_reviews", &[])
                    .await,
                i64::from(durable)
            );
            let reviewed = fixture
                .store()
                .review(ALICE, SCRIPT, review.clone())
                .await
                .unwrap();
            let input = import();
            assert!(matches!(
                uncertain.import_manuscript(ALICE, input.clone()).await,
                Err(StoreError::Unavailable)
            ));
            assert_eq!(
                fixture
                    .count("SELECT count(*) FROM source_records", &[])
                    .await,
                i64::from(durable)
            );
            let imported = fixture
                .store()
                .import_manuscript(ALICE, input.clone())
                .await
                .unwrap();
            drop(uncertain);
            fixture.restart().await;
            assert_eq!(
                fixture.store().save(ALICE, SCRIPT, intent).await.unwrap(),
                saved
            );
            assert_eq!(
                fixture.store().review(ALICE, SCRIPT, review).await.unwrap(),
                reviewed
            );
            assert_eq!(
                fixture
                    .store()
                    .import_manuscript(ALICE, input)
                    .await
                    .unwrap(),
                imported
            );
            for table in [
                "script_revisions",
                "script_reviews",
                "script_review_operations",
                "source_records",
            ] {
                assert_eq!(
                    fixture
                        .count(&format!("SELECT count(*) FROM {table}"), &[])
                        .await,
                    1
                );
            }
        }
    }

    #[tokio::test]
    async fn rejecting_a_final_step_rolls_back_the_provisional_script_and_review() {
        let fixture = fixture().await;
        let intent = request(0);
        fixture.exec("CREATE TRIGGER reject_revision AFTER INSERT ON script_revisions BEGIN SELECT RAISE(ABORT,'cantos:test.final_step'); END", &[]).await.unwrap();
        assert!(matches!(
            fixture.store().save(ALICE, SCRIPT, intent.clone()).await,
            Err(StoreError::Unavailable)
        ));
        for table in ["scripts", "script_revisions", "revision_evidence"] {
            assert_eq!(
                fixture
                    .count(&format!("SELECT count(*) FROM {table}"), &[])
                    .await,
                0
            );
        }
        fixture
            .exec("DROP TRIGGER reject_revision", &[])
            .await
            .unwrap();
        fixture.store().save(ALICE, SCRIPT, intent).await.unwrap();
        fixture.exec("CREATE TRIGGER reject_review_receipt BEFORE INSERT ON script_review_operations BEGIN SELECT RAISE(ABORT,'cantos:test.final_step'); END", &[]).await.unwrap();
        let review = ReviewRequest {
            revision: 1,
            operation_id: uuid::Uuid::new_v4().to_string(),
        };
        assert!(matches!(
            fixture.store().review(ALICE, SCRIPT, review.clone()).await,
            Err(StoreError::Unavailable)
        ));
        assert_eq!(
            fixture
                .count("SELECT count(*) FROM script_reviews", &[])
                .await,
            0
        );
        assert_eq!(
            fixture
                .count("SELECT count(*) FROM script_review_operations", &[])
                .await,
            0
        );
        fixture
            .exec("DROP TRIGGER reject_review_receipt", &[])
            .await
            .unwrap();
        assert_eq!(
            fixture
                .store()
                .review(ALICE, SCRIPT, review)
                .await
                .unwrap()
                .revision,
            1
        );
    }

    #[tokio::test]
    async fn waiting_saves_and_imports_recheck_admission_after_the_ordered_revocation() {
        let fixture = fixture().await;
        let mut conn = fixture.store().connection().await.unwrap();
        let blocker = conn.transaction().await.unwrap();
        blocker
            .execute("UPDATE actors SET active=0 WHERE id='alice'", &[])
            .await
            .unwrap();
        let mut save = Box::pin(fixture.store().save(ALICE, SCRIPT, request(0)));
        let mut intake = Box::pin(fixture.store().import_manuscript(ALICE, import()));
        assert!(futures_util::poll!(&mut save).is_pending());
        assert!(futures_util::poll!(&mut intake).is_pending());
        blocker.commit().await.unwrap();
        assert!(matches!(save.await, Err(StoreError::Unauthenticated)));
        assert!(matches!(intake.await, Err(StoreError::Unauthenticated)));
        assert_eq!(fixture.count("SELECT count(*) FROM scripts", &[]).await, 0);
        assert_eq!(
            fixture
                .count("SELECT count(*) FROM source_records", &[])
                .await,
            0
        );
    }

    #[tokio::test]
    async fn competing_saves_have_one_winner_and_duplicate_delivery_has_one_revision() {
        let fixture = fixture().await;
        let mut conn = fixture.store().connection().await.unwrap();
        let blocker = conn.transaction().await.unwrap();
        let mut one = Box::pin(fixture.store().save(ALICE, SCRIPT, request(0)));
        let mut two = Box::pin(fixture.store().save(ALICE, SCRIPT, request(0)));
        assert!(futures_util::poll!(&mut one).is_pending());
        assert!(futures_util::poll!(&mut two).is_pending());
        blocker.commit().await.unwrap();
        let (one, two) = tokio::join!(one, two);
        match (one, two) {
            (Ok(saved), Err(StoreError::StaleRevision(1)))
            | (Err(StoreError::StaleRevision(1)), Ok(saved)) => assert_eq!(saved.revision, 1),
            other => panic!("one accepted save and one stale refusal required: {other:?}"),
        }
        let intent = request(1);
        let (one, two) = tokio::join!(
            fixture.store().save(ALICE, SCRIPT, intent.clone()),
            fixture.store().save(ALICE, SCRIPT, intent)
        );
        assert_eq!(one.unwrap(), two.unwrap());
        assert_eq!(
            fixture
                .count("SELECT count(*) FROM script_revisions", &[])
                .await,
            2
        );
    }

    #[tokio::test]
    async fn competing_imports_preserve_one_receipt_and_reject_changed_operation() {
        let fixture = fixture().await;
        let input = import();
        let mut conn = fixture.store().connection().await.unwrap();
        let blocker = conn.transaction().await.unwrap();
        let mut one = Box::pin(fixture.store().import_manuscript(ALICE, input.clone()));
        let mut two = Box::pin(fixture.store().import_manuscript(ALICE, input.clone()));
        assert!(futures_util::poll!(&mut one).is_pending());
        assert!(futures_util::poll!(&mut two).is_pending());
        blocker.commit().await.unwrap();
        let (one, two) = tokio::join!(one, two);
        assert_eq!(one.unwrap(), two.unwrap());
        let mut changed = input;
        changed.original_bytes = b"changed original".to_vec();
        assert!(matches!(
            fixture.store().import_manuscript(ALICE, changed).await,
            Err(StoreError::OperationReused)
        ));
        assert_eq!(
            fixture
                .count("SELECT count(*) FROM source_records", &[])
                .await,
            1
        );
    }

    #[tokio::test]
    async fn pinned_editorial_reads_do_not_wait_for_or_observe_an_uncommitted_writer() {
        let fixture = fixture().await;
        let saved = fixture
            .store()
            .save(ALICE, SCRIPT, request(0))
            .await
            .unwrap();
        let mut conn = fixture.store().connection().await.unwrap();
        let blocker = conn.transaction().await.unwrap();
        blocker
            .execute("UPDATE actors SET active=0 WHERE id='alice'", &[])
            .await
            .unwrap();
        let loaded = tokio::time::timeout(
            std::time::Duration::from_secs(1),
            fixture.store().load(ALICE, SCRIPT, None),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(loaded, saved);
        blocker.commit().await.unwrap();
        assert!(matches!(
            fixture.store().load(ALICE, SCRIPT, None).await,
            Err(StoreError::Unauthenticated)
        ));
    }

    #[tokio::test]
    async fn a_review_counts_only_for_the_exact_revision_and_current_active_owner() {
        let fixture = fixture().await;
        fixture
            .store()
            .save(ALICE, SCRIPT, request(0))
            .await
            .unwrap();
        fixture
            .store()
            .review(
                ALICE,
                SCRIPT,
                ReviewRequest {
                    revision: 1,
                    operation_id: uuid::Uuid::new_v4().to_string(),
                },
            )
            .await
            .unwrap();
        let conn = fixture.store().connection().await.unwrap();
        let tx = conn.read().await.unwrap();
        for (revision, owner, expected) in
            [(1, "alice", true), (2, "alice", false), (1, "bob", false)]
        {
            assert_eq!(
                crate::contexts::editorial::reviewed_by_active_owner(&tx, SCRIPT, revision, owner)
                    .await
                    .unwrap(),
                expected
            );
        }
        tx.commit().await.unwrap();
        fixture.set_actor_active("alice", false).await;
        let tx = conn.read().await.unwrap();
        assert!(
            !crate::contexts::editorial::reviewed_by_active_owner(&tx, SCRIPT, 1, "alice")
                .await
                .unwrap()
        );
        tx.commit().await.unwrap();
    }
}
