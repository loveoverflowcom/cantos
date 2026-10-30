//! Metadata publication generations (#648) over real `server.db` files: owner commands with
//! CAS/receipts, draft isolation, live gates, persistence across reopen, schema barriers and
//! the Studio routes. No Theatre route reads generations yet (S05), so the public projection is
//! exercised through its read facade inside real read snapshots.
use super::{ServerFixture, WorkSeed};
use crate::contexts::catalog::{
    domain::{
        metadata_publication::{GenerationNumber, PublicationError},
        ContentUnitId, ContentUnitKind, OwnerId, UnitDetails, Visibility, WorkDetails, WorkId,
        WorkKind,
    },
    public_units, public_work, public_works, CatalogCommand, CatalogStoreError, MemberRole,
    PublicReadError, PublicWorksQuery,
};
use crate::contexts::media::ActiveAudio;
use crate::contract::studio::{
    ApiError, ErrorCode, MetadataPublicationLifecycle as Lifecycle,
    MetadataPublicationRequest as Request, MetadataPublicationState as State,
    MetadataPublicationUnit,
};
use crate::platform::{store::StoreError, turso::Acknowledgement};
use axum::{
    body::Body,
    http::{header, Request as HttpRequest, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WORK: &str = "61000000-0000-4000-8000-000000000001";
const OTHER_WORK: &str = "61000000-0000-4000-8000-000000000002";
const A: &str = "62000000-0000-4000-8000-00000000000a";
const B: &str = "62000000-0000-4000-8000-00000000000b";
const C: &str = "62000000-0000-4000-8000-00000000000c";
const D: &str = "62000000-0000-4000-8000-00000000000d";
const FOREIGN: &str = "62000000-0000-4000-8000-0000000000ff";

async fn world() -> ServerFixture {
    let fixture = ServerFixture::new().await;
    fixture.studio_caller("alice", ALICE).await;
    fixture.studio_caller("bob", BOB).await;
    fixture
        .exec(
            "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','r1','synthetic')",
            &[],
        )
        .await
        .unwrap();
    fixture
}

async fn run(fixture: &ServerFixture, command: CatalogCommand) {
    fixture
        .store()
        .catalog_execute(ALICE, &Uuid::new_v4().to_string(), command)
        .await
        .unwrap();
}

fn work_details(title: &str, visibility: Visibility, rights: &[&str]) -> WorkDetails {
    WorkDetails {
        title: title.into(),
        synopsis: "Một câu chuyện tổng hợp.".into(),
        language: "vi-VN".into(),
        visibility,
        rights_refs: rights.iter().map(|r| (*r).to_owned()).collect(),
    }
}

fn unit_details(title: &str) -> UnitDetails {
    UnitDetails {
        title: title.into(),
        description: "Tập thử nghiệm".into(),
        source: None,
        script: None,
        content_digest: None,
    }
}

async fn work_revision(fixture: &ServerFixture, id: &str) -> u64 {
    fixture
        .store()
        .catalog_work(ALICE, &WorkId::parse(id).unwrap())
        .await
        .unwrap()
        .revision()
}

async fn unit_revision(fixture: &ServerFixture, id: &str) -> u64 {
    fixture
        .store()
        .catalog_unit(ALICE, &ContentUnitId::parse(id).unwrap())
        .await
        .unwrap()
        .revision()
}

/// A private Work (visibility is editorial only) with rights and the given chapters.
async fn work(fixture: &ServerFixture, id: &str, title: &str, rights: &[&str], units: &[&str]) {
    run(
        fixture,
        CatalogCommand::CreateWork {
            id: WorkId::parse(id).unwrap(),
            kind: WorkKind::Novel,
            details: work_details(title, Visibility::Private, rights),
        },
    )
    .await;
    for (index, unit) in units.iter().enumerate() {
        run(
            fixture,
            CatalogCommand::CreateUnit {
                work_id: WorkId::parse(id).unwrap(),
                expected_work_revision: work_revision(fixture, id).await,
                id: ContentUnitId::parse(unit).unwrap(),
                kind: ContentUnitKind::Chapter,
                details: unit_details(&format!("Chương {}", index + 1)),
            },
        )
        .await;
    }
}

fn publish(expected: u64, work_revision: u64, units: &[(&str, u64)]) -> Request {
    Request::Publish {
        operation_id: Uuid::new_v4().to_string(),
        expected_version: expected,
        work_revision,
        units: units
            .iter()
            .map(|(unit, revision)| MetadataPublicationUnit {
                unit_id: (*unit).into(),
                revision: *revision,
            })
            .collect(),
        confirm_metadata_rights: true,
    }
}

fn publish_unit(expected: u64, unit: &str, revision: u64, position: u32) -> Request {
    Request::PublishUnit {
        operation_id: Uuid::new_v4().to_string(),
        expected_version: expected,
        unit_id: unit.into(),
        revision,
        position,
        confirm_metadata_rights: true,
    }
}

fn withdraw(expected: u64) -> Request {
    Request::Withdraw {
        operation_id: Uuid::new_v4().to_string(),
        expected_version: expected,
    }
}

async fn execute(fixture: &ServerFixture, request: Request) -> Result<State, CatalogStoreError> {
    fixture
        .store()
        .metadata_publication_execute(ALICE, WORK, request, &ActiveAudio)
        .await
}

fn entries(state: &State) -> Vec<(&str, u64)> {
    state
        .generation
        .as_ref()
        .unwrap()
        .entries
        .iter()
        .map(|entry| (entry.unit_id.as_str(), entry.revision))
        .collect()
}

/// One public snapshot: listed Works (ID, pinned title, generation, count) and, per listed Work,
/// its visible Units (ID, pinned title, ordinal). Detail is read too and must agree with the list.
#[derive(Debug, PartialEq, Eq)]
struct Projection {
    works: Vec<(String, String, u64, u32)>,
    units: Vec<Vec<(String, String, u32)>>,
}

async fn projection(fixture: &ServerFixture) -> Projection {
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let listed = public_works(
        &tx,
        PublicWorksQuery {
            search: "",
            kind: None,
            language: None,
            limit: 50,
            after: None,
        },
    )
    .await
    .unwrap();
    let mut works = Vec::new();
    let mut units = Vec::new();
    for item in &listed {
        let detail = public_work(&tx, item.work.id()).await.unwrap().unwrap();
        assert_eq!(
            (
                detail.work.revision(),
                detail.generation,
                detail.first_visible_at
            ),
            (item.work.revision(), item.generation, item.first_visible_at)
        );
        let page = public_units(&tx, item.work.id(), None, 50)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(page.generation, item.generation);
        assert_eq!(page.next_after, None);
        // The list count and the Unit walk apply the same eligibility.
        assert_eq!(page.items.len(), item.unit_count as usize);
        works.push((
            item.work.id().as_str().to_owned(),
            item.work.details().title.clone(),
            item.generation.get(),
            item.unit_count,
        ));
        units.push(
            page.items
                .iter()
                .map(|unit| {
                    (
                        unit.unit.id().as_str().to_owned(),
                        unit.unit.details().title.clone(),
                        unit.ordinal,
                    )
                })
                .collect(),
        );
    }
    tx.commit().await.unwrap();
    Projection { works, units }
}

async fn absent(fixture: &ServerFixture, id: &str) {
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let id = WorkId::parse(id).unwrap();
    assert!(public_work(&tx, &id).await.unwrap().is_none());
    assert!(public_units(&tx, &id, None, 50).await.unwrap().is_none());
    tx.commit().await.unwrap();
    assert!(projection(fixture)
        .await
        .works
        .iter()
        .all(|(listed, ..)| listed != id.as_str()));
}

async fn count(fixture: &ServerFixture, table: &str) -> i64 {
    fixture
        .count(&format!("SELECT count(*) FROM {table}"), &[])
        .await
}

#[tokio::test]
async fn publish_pins_exact_revisions_and_a_saved_draft_never_changes_the_projection() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A, B, C]).await;
    let never = fixture
        .store()
        .metadata_publication_state(ALICE, WORK)
        .await
        .unwrap();
    assert_eq!(never.lifecycle, Lifecycle::NeverPublished);
    assert_eq!(
        (never.version, never.generation, never.first_visible_at_ms),
        (0, None, None)
    );
    absent(&fixture, WORK).await;

    let r3 = work_revision(&fixture, WORK).await;
    let (a, c) = (
        unit_revision(&fixture, A).await,
        unit_revision(&fixture, C).await,
    );
    // Explicit order C, A; B is a saved draft that is not selected.
    let published = execute(&fixture, publish(0, r3, &[(C, c), (A, a)]))
        .await
        .unwrap();
    assert_eq!(published.lifecycle, Lifecycle::Active);
    assert_eq!(published.version, 1);
    assert_eq!(entries(&published), [(C, c), (A, a)]);
    let generation = published.generation.clone().unwrap();
    assert_eq!(
        (
            generation.number,
            generation.parent,
            generation.work_revision
        ),
        (1, None, r3)
    );
    let before = projection(&fixture).await;
    assert_eq!(
        before.works,
        [(WORK.to_owned(), "Mùa trăng".to_owned(), 1, 2)]
    );
    assert_eq!(
        before.units,
        [vec![
            (C.to_owned(), "Chương 3".to_owned(), 1),
            (A.to_owned(), "Chương 1".to_owned(), 2)
        ]]
    );

    // Save r4 changing every Work field, then Unit text, the order and membership.
    fixture
        .exec(
            "INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice','rights','r2','synthetic')",
            &[],
        )
        .await
        .unwrap();
    run(
        &fixture,
        CatalogCommand::EditWork {
            id: WorkId::parse(WORK).unwrap(),
            expected_revision: work_revision(&fixture, WORK).await,
            details: WorkDetails {
                title: "Bản nháp mới".into(),
                synopsis: "Tóm tắt nháp chưa duyệt.".into(),
                language: "en".into(),
                visibility: Visibility::Public,
                rights_refs: vec!["r2".into()],
            },
        },
    )
    .await;
    run(
        &fixture,
        CatalogCommand::EditUnit {
            id: ContentUnitId::parse(A).unwrap(),
            expected_revision: unit_revision(&fixture, A).await,
            expected_work_revision: work_revision(&fixture, WORK).await,
            details: UnitDetails {
                description: "Mô tả nháp".into(),
                ..unit_details("Chương một (nháp)")
            },
        },
    )
    .await;
    run(
        &fixture,
        CatalogCommand::Reorder {
            work_id: WorkId::parse(WORK).unwrap(),
            expected_revision: work_revision(&fixture, WORK).await,
            ordered: [B, A, C]
                .map(|id| ContentUnitId::parse(id).unwrap())
                .to_vec(),
        },
    )
    .await;
    run(
        &fixture,
        CatalogCommand::CreateUnit {
            work_id: WorkId::parse(WORK).unwrap(),
            expected_work_revision: work_revision(&fixture, WORK).await,
            id: ContentUnitId::parse(D).unwrap(),
            kind: ContentUnitKind::Chapter,
            details: unit_details("Chương nháp"),
        },
    )
    .await;
    // Visibility private → public → private again is still only editorial.
    run(
        &fixture,
        CatalogCommand::EditWork {
            id: WorkId::parse(WORK).unwrap(),
            expected_revision: work_revision(&fixture, WORK).await,
            details: work_details("Bản nháp riêng", Visibility::Private, &["r1"]),
        },
    )
    .await;
    assert!(work_revision(&fixture, WORK).await > r3);
    assert_eq!(projection(&fixture).await, before);
    // Every published field still comes from r3, and the language filter uses r3's language.
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let id = WorkId::parse(WORK).unwrap();
    let shown = public_work(&tx, &id).await.unwrap().unwrap();
    assert_eq!(
        shown.work.details(),
        &work_details("Mùa trăng", Visibility::Private, &["r1"])
    );
    let units = public_units(&tx, &id, None, 50).await.unwrap().unwrap();
    assert_eq!(units.items[1].unit.details(), &unit_details("Chương 1"));
    let english = public_works(
        &tx,
        PublicWorksQuery {
            search: "nháp",
            kind: None,
            language: Some("en"),
            limit: 50,
            after: None,
        },
    )
    .await
    .unwrap();
    assert!(english.is_empty());
    tx.commit().await.unwrap();
    let state = fixture
        .store()
        .metadata_publication_state(ALICE, WORK)
        .await
        .unwrap();
    assert_eq!(state, published);

    // A per-Unit publish adds exactly one Unit at its asked position. A's newer draft and the
    // Work's newer head are not pulled in.
    let b = unit_revision(&fixture, B).await;
    let added = execute(&fixture, publish_unit(1, B, b, 2)).await.unwrap();
    assert_eq!(entries(&added), [(C, c), (B, b), (A, a)]);
    let generation = added.generation.unwrap();
    assert_eq!(
        (
            generation.number,
            generation.parent,
            generation.work_revision
        ),
        (2, Some(1), r3)
    );
    assert_eq!(added.first_visible_at_ms, published.first_visible_at_ms);
    let after = projection(&fixture).await;
    assert_eq!(
        after.works,
        [(WORK.to_owned(), "Mùa trăng".to_owned(), 2, 3)]
    );
    assert_eq!(
        after.units[0]
            .iter()
            .map(|u| (u.0.as_str(), u.2))
            .collect::<Vec<_>>(),
        [(C, 1), (B, 2), (A, 3)]
    );
    assert_eq!(after.units[0][2].1, "Chương 1");
}

#[tokio::test]
async fn replay_changed_payload_stale_version_and_concurrent_publishers_settle_once() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A, B]).await;
    let w = work_revision(&fixture, WORK).await;
    let (a, b) = (
        unit_revision(&fixture, A).await,
        unit_revision(&fixture, B).await,
    );
    let first = publish(0, w, &[(A, a)]);
    let settled = execute(&fixture, first.clone()).await.unwrap();
    // A later version does not change what the first operation settled.
    let later = execute(&fixture, publish_unit(1, B, b, 2)).await.unwrap();
    assert_eq!(later.version, 2);
    assert_eq!(execute(&fixture, first.clone()).await.unwrap(), settled);
    let (events, generations) = (
        count(&fixture, "catalog_metadata_publication_events").await,
        count(&fixture, "catalog_metadata_generations").await,
    );
    let Request::Publish { operation_id, .. } = &first else {
        unreachable!()
    };
    let mut changed = publish(0, w, &[(B, b)]);
    if let Request::Publish {
        operation_id: id, ..
    } = &mut changed
    {
        id.clone_from(operation_id);
    }
    assert!(matches!(
        execute(&fixture, changed).await,
        Err(CatalogStoreError::OperationReused)
    ));
    // The same operation ID on another Work is also a different payload.
    work(&fixture, OTHER_WORK, "Khác", &["r1"], &[]).await;
    let other_revision = work_revision(&fixture, OTHER_WORK).await;
    let mut elsewhere = publish(0, other_revision, &[]);
    if let Request::Publish {
        operation_id: id, ..
    } = &mut elsewhere
    {
        id.clone_from(operation_id);
    }
    assert!(matches!(
        fixture
            .store()
            .metadata_publication_execute(ALICE, OTHER_WORK, elsewhere, &ActiveAudio)
            .await,
        Err(CatalogStoreError::OperationReused)
    ));
    assert!(matches!(
        execute(&fixture, withdraw(1)).await,
        Err(CatalogStoreError::Publication(
            PublicationError::StaleVersion { current: 2 }
        ))
    ));
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        events
    );
    assert_eq!(
        count(&fixture, "catalog_metadata_generations").await,
        generations
    );

    // Two publishers read version 2; exactly one settles and the other learns version 3.
    let (left, right) = tokio::join!(
        execute(&fixture, publish(2, w, &[(B, b), (A, a)])),
        execute(&fixture, withdraw(2))
    );
    let outcomes = [left, right];
    assert_eq!(outcomes.iter().filter(|outcome| outcome.is_ok()).count(), 1);
    assert!(outcomes.iter().any(|outcome| matches!(
        outcome,
        Err(CatalogStoreError::Publication(
            PublicationError::StaleVersion { current: 3 }
        ))
    )));
}

#[tokio::test]
async fn a_lost_acknowledgement_is_reconciled_by_the_operation_and_survives_reopen() {
    let mut fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A]).await;
    let w = work_revision(&fixture, WORK).await;
    let a = unit_revision(&fixture, A).await;
    let request = publish(0, w, &[(A, a)]);
    let mut lost = fixture.store().clone();
    lost.acknowledgement = Acknowledgement::LostAfterCommit;
    assert!(matches!(
        lost.metadata_publication_execute(ALICE, WORK, request.clone(), &ActiveAudio)
            .await,
        Err(CatalogStoreError::Storage(StoreError::Unavailable))
    ));
    // The commit happened: the retry answers the settled outcome and writes nothing new.
    let settled = execute(&fixture, request.clone()).await.unwrap();
    assert_eq!(settled.version, 1);
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        1
    );

    // A commit that never happened leaves nothing; the retry settles it freshly.
    let mut rolled_back = fixture.store().clone();
    rolled_back.acknowledgement = Acknowledgement::LostAfterRollback;
    let withdrawal = withdraw(1);
    assert!(matches!(
        rolled_back
            .metadata_publication_execute(ALICE, WORK, withdrawal.clone(), &ActiveAudio)
            .await,
        Err(CatalogStoreError::Storage(StoreError::Unavailable))
    ));
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        1
    );
    assert_eq!(projection(&fixture).await.works.len(), 1);
    let withdrawn = execute(&fixture, withdrawal.clone()).await.unwrap();
    assert_eq!(
        (withdrawn.lifecycle, withdrawn.version),
        (Lifecycle::Withdrawn, 2)
    );

    // The test stores share the owner handle; release them as a stopped process would.
    drop((lost, rolled_back));
    fixture.restart().await;
    assert_eq!(
        fixture
            .store()
            .metadata_publication_state(ALICE, WORK)
            .await
            .unwrap(),
        withdrawn
    );
    assert_eq!(execute(&fixture, request).await.unwrap(), settled);
    assert_eq!(execute(&fixture, withdrawal).await.unwrap(), withdrawn);
    absent(&fixture, WORK).await;
}

#[tokio::test]
async fn withdraw_hides_every_path_and_republish_keeps_history_and_the_anchor() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A, B]).await;
    let w = work_revision(&fixture, WORK).await;
    let (a, b) = (
        unit_revision(&fixture, A).await,
        unit_revision(&fixture, B).await,
    );
    let first = execute(&fixture, publish(0, w, &[(A, a), (B, b)]))
        .await
        .unwrap();
    let removed = execute(
        &fixture,
        Request::WithdrawUnit {
            operation_id: Uuid::new_v4().to_string(),
            expected_version: 1,
            unit_id: A.into(),
        },
    )
    .await
    .unwrap();
    assert_eq!(entries(&removed), [(B, b)]);
    assert!(matches!(
        execute(
            &fixture,
            Request::WithdrawUnit {
                operation_id: Uuid::new_v4().to_string(),
                expected_version: 2,
                unit_id: A.into(),
            }
        )
        .await,
        Err(CatalogStoreError::Publication(
            PublicationError::UnitNotPublished { .. }
        ))
    ));
    let withdrawn = execute(&fixture, withdraw(2)).await.unwrap();
    assert_eq!(withdrawn.lifecycle, Lifecycle::Withdrawn);
    assert_eq!(withdrawn.generation.as_ref().unwrap().number, 2);
    absent(&fixture, WORK).await;
    assert!(matches!(
        execute(&fixture, withdraw(3)).await,
        Err(CatalogStoreError::Publication(PublicationError::NotActive))
    ));
    assert!(matches!(
        execute(&fixture, publish_unit(3, A, a, 1)).await,
        Err(CatalogStoreError::Publication(PublicationError::NotActive))
    ));
    // Republish writes generation 3; the first anchor stays and history is intact.
    let again = execute(&fixture, publish(3, w, &[(A, a), (B, b)]))
        .await
        .unwrap();
    assert_eq!(
        (again.version, again.generation.as_ref().unwrap().number),
        (4, 3)
    );
    assert_eq!(again.first_visible_at_ms, first.first_visible_at_ms);
    assert_eq!(count(&fixture, "catalog_metadata_generations").await, 3);
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        4
    );
    assert_eq!(projection(&fixture).await.works[0].2, 3);
}

#[tokio::test]
async fn archive_and_owner_disable_are_live_gates_with_consecutive_ordinals() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A, B, C]).await;
    let w = work_revision(&fixture, WORK).await;
    let selection: Vec<_> = [A, B, C].into_iter().collect();
    let mut units = Vec::new();
    for unit in &selection {
        units.push((*unit, unit_revision(&fixture, unit).await));
    }
    execute(&fixture, publish(0, w, &units)).await.unwrap();
    run(
        &fixture,
        CatalogCommand::ArchiveUnit {
            id: ContentUnitId::parse(B).unwrap(),
            expected_revision: unit_revision(&fixture, B).await,
            expected_work_revision: work_revision(&fixture, WORK).await,
        },
    )
    .await;
    let visible = projection(&fixture).await;
    assert_eq!(visible.works[0].3, 2);
    assert_eq!(
        visible.units[0]
            .iter()
            .map(|u| (u.0.as_str(), u.2))
            .collect::<Vec<_>>(),
        [(A, 1), (C, 2)]
    );
    // Publishing the archived Unit again is refused; the generation still names it.
    assert!(matches!(
        execute(&fixture, publish_unit(1, B, units[1].1, 2)).await,
        Err(CatalogStoreError::Publication(
            PublicationError::UnitArchived { .. }
        ))
    ));
    fixture.set_actor_active("alice", false).await;
    absent(&fixture, WORK).await;
    fixture.set_actor_active("alice", true).await;
    assert_eq!(projection(&fixture).await, visible);
    run(
        &fixture,
        CatalogCommand::ArchiveWork {
            id: WorkId::parse(WORK).unwrap(),
            expected_revision: work_revision(&fixture, WORK).await,
        },
    )
    .await;
    absent(&fixture, WORK).await;
    // Withdrawal stays possible for an archived Work; publishing does not.
    assert!(matches!(
        execute(&fixture, publish(1, w, &[])).await,
        Err(CatalogStoreError::Publication(
            PublicationError::WorkArchived
        ))
    ));
    assert_eq!(
        execute(&fixture, withdraw(1)).await.unwrap().lifecycle,
        Lifecycle::Withdrawn
    );
}

#[tokio::test]
async fn only_the_current_owner_publishes_or_reads_the_state() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A]).await;
    run(
        &fixture,
        CatalogCommand::SetMember {
            work_id: WorkId::parse(WORK).unwrap(),
            expected_revision: work_revision(&fixture, WORK).await,
            actor_id: OwnerId::parse("bob").unwrap(),
            role: Some(MemberRole::Editor),
        },
    )
    .await;
    let w = work_revision(&fixture, WORK).await;
    let a = unit_revision(&fixture, A).await;
    for token in [BOB, "unknown-token"] {
        let refused = fixture
            .store()
            .metadata_publication_execute(token, WORK, publish(0, w, &[(A, a)]), &ActiveAudio)
            .await;
        assert!(matches!(
            refused,
            Err(CatalogStoreError::Storage(
                StoreError::NotFound | StoreError::Unauthenticated
            ))
        ));
        assert!(matches!(
            fixture
                .store()
                .metadata_publication_state(token, WORK)
                .await,
            Err(CatalogStoreError::Storage(
                StoreError::NotFound | StoreError::Unauthenticated
            ))
        ));
    }
    assert!(matches!(
        fixture
            .store()
            .metadata_publication_execute(BOB, WORK, publish(0, w, &[(A, a)]), &ActiveAudio)
            .await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        0
    );
}

#[tokio::test]
async fn unsubstantiated_unknown_foreign_and_duplicate_selections_write_nothing() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &[], &[A]).await;
    work(&fixture, OTHER_WORK, "Khác", &["r1"], &[FOREIGN]).await;
    let w = work_revision(&fixture, WORK).await;
    let a = unit_revision(&fixture, A).await;
    let foreign = unit_revision(&fixture, FOREIGN).await;
    assert!(matches!(
        execute(&fixture, publish(0, w, &[(A, a)])).await,
        Err(CatalogStoreError::Publication(
            PublicationError::RightsUnavailable
        ))
    ));
    run(
        &fixture,
        CatalogCommand::EditWork {
            id: WorkId::parse(WORK).unwrap(),
            expected_revision: w,
            details: work_details("Mùa trăng", Visibility::Private, &["r1"]),
        },
    )
    .await;
    let w = work_revision(&fixture, WORK).await;
    let mut unconfirmed = publish(0, w, &[(A, a)]);
    if let Request::Publish {
        confirm_metadata_rights,
        ..
    } = &mut unconfirmed
    {
        *confirm_metadata_rights = false;
    }
    assert!(matches!(
        execute(&fixture, unconfirmed).await,
        Err(CatalogStoreError::Storage(StoreError::InvalidRequest))
    ));
    for (request, expected) in [
        (
            publish(0, w + 1, &[(A, a)]),
            PublicationError::UnknownRevision,
        ),
        (
            publish(0, w, &[(A, a + 1)]),
            PublicationError::UnknownUnit {
                unit: ContentUnitId::parse(A).unwrap(),
            },
        ),
        (
            publish(0, w, &[(D, 1)]),
            PublicationError::UnknownUnit {
                unit: ContentUnitId::parse(D).unwrap(),
            },
        ),
        (
            publish(0, w, &[(FOREIGN, foreign)]),
            PublicationError::UnknownUnit {
                unit: ContentUnitId::parse(FOREIGN).unwrap(),
            },
        ),
        (
            publish(0, w, &[(A, a), (A, a)]),
            PublicationError::DuplicateUnit {
                unit: ContentUnitId::parse(A).unwrap(),
            },
        ),
    ] {
        match execute(&fixture, request).await {
            Err(CatalogStoreError::Publication(actual)) => assert_eq!(actual, expected),
            other => panic!("{other:?}"),
        }
    }
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        0
    );
    assert_eq!(count(&fixture, "catalog_metadata_generations").await, 0);
    absent(&fixture, WORK).await;
}

#[tokio::test]
async fn released_and_public_works_stay_absent_until_a_generation_is_published() {
    let fixture = world().await;
    // An eligible audio release chain and a public-visibility Work: neither is a generation.
    let released = fixture
        .seed_work(
            None,
            &WorkSeed::podcast(0x6300_0001, "Đã phát hành", &["Tập 1"]),
        )
        .await;
    run(
        &fixture,
        CatalogCommand::CreateWork {
            id: WorkId::parse(WORK).unwrap(),
            kind: WorkKind::Story,
            details: work_details("Công khai", Visibility::Public, &["r1"]),
        },
    )
    .await;
    assert_eq!(projection(&fixture).await.works, []);
    absent(&fixture, &released.work_id).await;
    absent(&fixture, WORK).await;
    let w = work_revision(&fixture, WORK).await;
    execute(&fixture, publish(0, w, &[])).await.unwrap();
    // A Work may be visible with zero Units.
    assert_eq!(
        projection(&fixture).await.works,
        [(WORK.to_owned(), "Công khai".to_owned(), 1, 0)]
    );
    absent(&fixture, &released.work_id).await;
}

#[tokio::test]
async fn public_list_filters_before_the_keyset_and_unit_walks_bind_the_generation() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A, B, C]).await;
    work(&fixture, OTHER_WORK, "Biển xanh", &["r1"], &[]).await;
    let w = work_revision(&fixture, WORK).await;
    let mut units = Vec::new();
    for unit in [A, B, C] {
        units.push((unit, unit_revision(&fixture, unit).await));
    }
    execute(&fixture, publish(0, w, &units)).await.unwrap();
    let other = work_revision(&fixture, OTHER_WORK).await;
    fixture
        .store()
        .metadata_publication_execute(ALICE, OTHER_WORK, publish(0, other, &[]), &ActiveAudio)
        .await
        .unwrap();
    // Search reads the pinned revision, not a later draft title.
    run(
        &fixture,
        CatalogCommand::EditWork {
            id: WorkId::parse(OTHER_WORK).unwrap(),
            expected_revision: work_revision(&fixture, OTHER_WORK).await,
            details: work_details("Mùa trăng giả", Visibility::Private, &["r1"]),
        },
    )
    .await;
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    let query = |search, limit, after| PublicWorksQuery {
        search,
        kind: None,
        language: None,
        limit,
        after,
    };
    let found = public_works(&tx, query("MÙA TRĂNG", 50, None))
        .await
        .unwrap();
    assert_eq!(
        found
            .iter()
            .map(|w| w.work.id().as_str())
            .collect::<Vec<_>>(),
        [WORK]
    );
    let all = public_works(&tx, query("", 50, None)).await.unwrap();
    assert_eq!(all.len(), 2);
    assert!(all[0].first_visible_at >= all[1].first_visible_at);
    let first = public_works(&tx, query("", 1, None)).await.unwrap();
    let rest = public_works(
        &tx,
        query("", 1, Some((first[0].first_visible_at, first[0].work.id()))),
    )
    .await
    .unwrap();
    assert_eq!(
        [first[0].work.id(), rest[0].work.id()],
        [all[0].work.id(), all[1].work.id()]
    );
    let filtered = public_works(
        &tx,
        PublicWorksQuery {
            search: "",
            kind: Some(WorkKind::Podcast),
            language: None,
            limit: 50,
            after: None,
        },
    )
    .await
    .unwrap();
    assert!(filtered.is_empty());
    let language = public_works(
        &tx,
        PublicWorksQuery {
            search: "",
            kind: Some(WorkKind::Novel),
            language: Some("VI-vn"),
            limit: 50,
            after: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(language.len(), 2);
    assert!(matches!(
        public_works(&tx, query("", 0, None)).await,
        Err(StoreError::InvalidRequest)
    ));

    let id = WorkId::parse(WORK).unwrap();
    let page = public_units(&tx, &id, None, 2).await.unwrap().unwrap();
    assert_eq!(
        page.items
            .iter()
            .map(|u| (u.ordinal, u.position))
            .collect::<Vec<_>>(),
        [(1, 1), (2, 2)]
    );
    assert_eq!(page.next_after, Some(2));
    let next = public_units(&tx, &id, Some((page.generation, 2)), 2)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        next.items
            .iter()
            .map(|u| u.unit.id().as_str())
            .collect::<Vec<_>>(),
        [C]
    );
    assert_eq!(next.next_after, None);
    tx.commit().await.unwrap();

    // A newer generation restarts an older walk instead of mixing two orders.
    execute(&fixture, publish(1, w, &[units[2], units[0]]))
        .await
        .unwrap();
    let connection = fixture.store().connection().await.unwrap();
    let tx = connection.read().await.unwrap();
    assert!(matches!(
        public_units(&tx, &id, Some((page.generation, 2)), 2).await,
        Err(PublicReadError::StaleGeneration)
    ));
    let restarted = public_units(&tx, &id, None, 50).await.unwrap().unwrap();
    assert_eq!(restarted.generation, GenerationNumber::new(2).unwrap());
    assert!(matches!(
        public_units(&tx, &id, None, 51).await,
        Err(PublicReadError::Storage(StoreError::InvalidRequest))
    ));
    tx.commit().await.unwrap();
}

#[tokio::test]
async fn the_schema_refuses_rewrites_foreign_pins_and_pointer_jumps() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A]).await;
    work(&fixture, OTHER_WORK, "Khác", &["r1"], &[FOREIGN]).await;
    let w = work_revision(&fixture, WORK).await;
    let a = unit_revision(&fixture, A).await;
    execute(&fixture, publish(0, w, &[(A, a)])).await.unwrap();
    for statement in [
        "UPDATE catalog_metadata_generations SET work_revision=1",
        "DELETE FROM catalog_metadata_generations",
        "UPDATE catalog_metadata_generation_entries SET position=1",
        "DELETE FROM catalog_metadata_generation_entries",
        "UPDATE catalog_metadata_publication_events SET lifecycle='withdrawn'",
        "DELETE FROM catalog_metadata_publication_events",
        "DELETE FROM catalog_metadata_publication_pointers",
        "UPDATE catalog_metadata_publication_pointers SET first_visible_at_ms=0",
        "UPDATE catalog_metadata_publication_pointers SET version=3",
        "UPDATE catalog_metadata_publication_pointers SET lifecycle='withdrawn'",
    ] {
        assert!(
            fixture.try_exec(statement, &[]).await.is_err(),
            "{statement}"
        );
    }
    let digest: String = fixture
        .rows(
            "SELECT snapshot_digest FROM catalog_unit_revisions WHERE unit_id=?1",
            &[&FOREIGN],
        )
        .await
        .unwrap()[0]
        .try_get("snapshot_digest")
        .unwrap();
    let work_digest: String = fixture
        .rows(
            "SELECT snapshot_digest FROM catalog_work_revisions WHERE work_id=?1 AND revision=?2",
            &[&WORK, &(w as i64)],
        )
        .await
        .unwrap()[0]
        .try_get("snapshot_digest")
        .unwrap();
    // Generation 2 may be written, but not with a wrong pin, a skipped number or a foreign
    // Unit, and an entry cannot be added to the sealed generation 1.
    let insert_generation = "INSERT INTO catalog_metadata_generations(work_id,generation,owner_id,kind,work_revision,work_digest,entry_count,language,search_text,recorded_by,recorded_at_ms) VALUES(?1,?2,'alice','novel',?3,?4,1,'vi-vn','x','alice',1)";
    let wrong = "0".repeat(64);
    for (number, pinned) in [(2_i64, wrong.as_str()), (3, work_digest.as_str())] {
        assert!(fixture
            .try_exec(insert_generation, &[&WORK, &number, &(w as i64), &pinned])
            .await
            .is_err());
    }
    let insert_entry = "INSERT INTO catalog_metadata_generation_entries(work_id,generation,position,unit_id,unit_revision,unit_digest) VALUES(?1,?2,?3,?4,1,?5)";
    assert!(fixture
        .try_exec(insert_entry, &[&WORK, &1_i64, &2_i64, &FOREIGN, &digest])
        .await
        .is_err());
    // An unsealed generation 2 still refuses a Unit of another Work.
    fixture
        .try_exec(
            insert_generation,
            &[&WORK, &2_i64, &(w as i64), &work_digest],
        )
        .await
        .unwrap();
    assert!(fixture
        .try_exec(insert_entry, &[&WORK, &2_i64, &1_i64, &FOREIGN, &digest])
        .await
        .is_err());
    assert_eq!(
        count(&fixture, "catalog_metadata_generation_entries").await,
        1
    );
}

#[tokio::test]
async fn studio_routes_require_the_owner_session_and_answer_the_contract_shape() {
    let fixture = world().await;
    work(&fixture, WORK, "Mùa trăng", &["r1"], &[A]).await;
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let path = format!("/api/v1/studio/works/{WORK}/metadata-publication");
    let send = |cookie: Option<&str>, body: Option<String>| {
        let mut request = match &body {
            Some(_) => HttpRequest::post(&path),
            None => HttpRequest::get(&path),
        }
        .header(header::ORIGIN, "https://studio.example.test");
        if let Some(token) = cookie {
            request = request.header(header::COOKIE, format!("cantos_session={token}"));
        }
        if body.is_some() {
            request = request.header(header::CONTENT_TYPE, "application/json");
        }
        request
            .body(body.map_or_else(Body::empty, Body::from))
            .unwrap()
    };
    let schema: serde_json::Value = serde_json::from_str(include_str!(
        "../../contracts/studio/metadata-publication/v1/metadata-publication.schema.json"
    ))
    .unwrap();
    let valid = |name: &str, value: &serde_json::Value| {
        jsonschema::validator_for(&serde_json::json!({
            "$schema": schema["$schema"], "$defs": schema["$defs"],
            "$ref": format!("#/$defs/{name}")
        }))
        .unwrap()
        .is_valid(value)
    };
    async fn json(response: axum::response::Response) -> serde_json::Value {
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
    }

    let response = app.clone().oneshot(send(None, None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let response = app.clone().oneshot(send(Some(BOB), None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app.clone().oneshot(send(Some(ALICE), None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let never = json(response).await;
    assert!(valid("MetadataPublicationState", &never));
    assert_eq!(never["lifecycle"], "never_published");

    let w = work_revision(&fixture, WORK).await;
    let a = unit_revision(&fixture, A).await;
    let request = serde_json::to_value(publish(0, w, &[(A, a)])).unwrap();
    assert!(valid("MetadataPublicationRequest", &request));
    let response = app
        .clone()
        .oneshot(send(Some(ALICE), Some(request.to_string())))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let active = json(response).await;
    assert!(valid("MetadataPublicationState", &active));
    let parsed: State = serde_json::from_value(active.clone()).unwrap();
    assert_eq!((parsed.lifecycle, parsed.version), (Lifecycle::Active, 1));

    let stale = serde_json::to_value(withdraw(0)).unwrap();
    let response = app
        .clone()
        .oneshot(send(Some(ALICE), Some(stale.to_string())))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let error: ApiError = serde_json::from_value(json(response).await).unwrap();
    assert_eq!(
        (error.code, error.current_revision),
        (ErrorCode::StaleRevision, Some(1))
    );

    let mut unknown = request.clone();
    unknown["presentation"] = serde_json::json!(3);
    for body in [unknown.to_string(), "{}".to_owned(), "not json".to_owned()] {
        let response = app
            .clone()
            .oneshot(send(Some(ALICE), Some(body)))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    }
    assert_eq!(
        count(&fixture, "catalog_metadata_publication_events").await,
        1
    );
}
