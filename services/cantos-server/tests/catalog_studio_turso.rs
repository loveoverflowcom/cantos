//! Native Catalog application oracles on real run-owned Turso files. Synthetic Vietnamese fixtures.

use cantos_server::contract::studio::SaveRevisionRequest;
use cantos_server::{
    contexts::catalog::{
        domain::{
            CatalogError, ContentUnitId, ContentUnitKind, Lifecycle, LinkPatch, OwnerId, ScriptRef,
            SourceRef, UnitDetails, UnitPatch, Visibility, Work, WorkDetails, WorkId, WorkKind,
            WorkPatch,
        },
        CatalogCommand, CatalogOutcome, CatalogStoreError, MemberRole,
    },
    platform::store::{Store, StoreError},
    script_ir::read_script,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
#[path = "support/native_sql.rs"]
mod native_sql;
use native_sql::NativeSql;
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CAROL: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const SCRIPT: &str =
    include_str!("../contracts/studio/script-ir/0.1.0/fixtures/accept/two-scenes.json");
const ORIGINAL: &str = "\u{feff}Ánh đèn cuối sân khấu\r\nNgười kể chuyện đã trở về.\r\n";
struct Harness {
    admin: NativeSql,
    store: Store,
}
impl Harness {
    async fn new() -> Self {
        Self::with_fixture(NativeSql::new().await).await
    }
    async fn with_fixture(admin: NativeSql) -> Self {
        admin.0.seed_actors(&["alice", "bob", "carol"]).await;
        for (actor, token) in [("alice", ALICE), ("bob", BOB), ("carol", CAROL)] {
            admin.0.seed_development_session(actor, token, 1).await;
            for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
                admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,?2,?3,'Synthetic reference; no clearance')", &[&actor,&kind,&id]).await.unwrap();
            }
            admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,'rights','catalog-rights','Synthetic rights')", &[&actor]).await.unwrap();
        }
        let store = admin.store();
        Self { admin, store }
    }
    async fn create(&self, token: &str, kind: WorkKind) -> Work {
        self.store
            .catalog_execute(
                token,
                &operation(),
                CatalogCommand::CreateWork {
                    id: work_id(),
                    kind,
                    details: work_details("Ánh đèn cuối sân khấu"),
                },
            )
            .await
            .unwrap()
            .work
    }

    async fn unit(&self, token: &str, work: &Work, title: &str) -> CatalogOutcome {
        self.store
            .catalog_execute(
                token,
                &operation(),
                CatalogCommand::CreateUnit {
                    work_id: work.id().clone(),
                    expected_work_revision: work.revision(),
                    id: unit_id(),
                    kind: work.kind().content_unit_kind(),
                    details: unit_details(title),
                },
            )
            .await
            .unwrap()
    }

    async fn counts(&self) -> Value {
        let encoded = self.admin.query_one("SELECT json_object('works',(SELECT count(*) FROM catalog_works),'work_revisions',(SELECT count(*) FROM catalog_work_revisions),'units',(SELECT count(*) FROM catalog_units),'unit_revisions',(SELECT count(*) FROM catalog_unit_revisions),'members',(SELECT count(*) FROM catalog_members),'operations',(SELECT count(*) FROM catalog_operations),'clock',(SELECT revision FROM catalog_clock))", &[]).await.unwrap().get::<_,String>(0);
        serde_json::from_str(&encoded).unwrap()
    }

    async fn source(&self, actor: &str, id: &str) -> SourceRef {
        let checksum = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
        self.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES(?1,'source',?2,'Original synthetic manuscript bytes')", &[&actor,&id]).await.unwrap();
        self.admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256,recorded_at) VALUES(?1,?2,'synthetic://catalog/manuscript',?3,?4,0)", &[&actor,&id,&ORIGINAL,&checksum]).await.unwrap();
        SourceRef::new(id.into(), checksum).unwrap()
    }

    async fn script(&self, token: &str) -> ScriptRef {
        let id = operation();
        let saved = self
            .store
            .save(
                token,
                &id,
                SaveRevisionRequest {
                    operation_id: operation(),
                    expected_revision: 0,
                    script_json: SCRIPT.into(),
                },
            )
            .await
            .unwrap();
        ScriptRef::new(id, saved.revision, saved.export_digest).unwrap()
    }
}

fn operation() -> String {
    Uuid::new_v4().to_string()
}
fn work_id() -> WorkId {
    WorkId::parse(&operation()).unwrap()
}
fn unit_id() -> ContentUnitId {
    ContentUnitId::parse(&operation()).unwrap()
}
fn work_details(title: &str) -> WorkDetails {
    WorkDetails {
        title: title.into(),
        synopsis: "Một tác phẩm thử nghiệm với tên và dấu tiếng Việt.".into(),
        language: "vi-VN".into(),
        visibility: Visibility::Private,
        rights_refs: vec![],
    }
}
fn unit_details(title: &str) -> UnitDetails {
    UnitDetails {
        title: title.into(),
        description: "Một chương hoặc tập chưa có âm thanh.".into(),
        source: None,
        script: None,
        content_digest: None,
    }
}

#[tokio::test]
async fn novel_story_and_podcast_accept_metadata_without_script_cast_or_scene() {
    let h = Harness::new().await;
    for kind in [WorkKind::Novel, WorkKind::Story, WorkKind::Podcast] {
        let work = h.create(ALICE, kind).await;
        assert_eq!(work.owner_id().as_str(), "alice");
        let outcome = h.unit(ALICE, &work, "Chương một — Người kể trở về").await;
        let unit = outcome
            .units
            .iter()
            .find(|unit| unit.position() == 0)
            .unwrap();
        assert_eq!(unit.work_id(), work.id());
        assert_eq!(unit.kind(), kind.content_unit_kind());
        assert_eq!(unit.details().source, None);
        assert_eq!(unit.details().script, None);
        assert_eq!(h.store.catalog_unit(ALICE, unit.id()).await.unwrap(), *unit);
    }
    assert_eq!(h.counts().await["units"], 3);
    assert!(matches!(
        h.store
            .catalog_execute(
                "bad-token",
                &operation(),
                CatalogCommand::CreateWork {
                    id: work_id(),
                    kind: WorkKind::Novel,
                    details: work_details("Không được tạo"),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::Unauthenticated))
    ));
}

#[tokio::test]
async fn rejected_kind_invalid_metadata_stale_revisions_and_duplicate_ids_leave_no_rows() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Podcast).await;
    let before = h.counts().await;
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::CreateUnit {
                    work_id: work.id().clone(),
                    expected_work_revision: 1,
                    id: unit_id(),
                    kind: ContentUnitKind::Chapter,
                    details: unit_details("Không đúng loại"),
                }
            )
            .await,
        Err(CatalogStoreError::Domain(
            CatalogError::WrongContentUnitKind {
                work: WorkKind::Podcast,
                unit: ContentUnitKind::Chapter,
            }
        ))
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::CreateWork {
                    id: work.id().clone(),
                    kind: WorkKind::Podcast,
                    details: work_details("Trùng ID"),
                }
            )
            .await,
        Err(CatalogStoreError::DuplicateId)
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::EditWork {
                    id: work.id().clone(),
                    expected_revision: 0,
                    details: work_details("Cũ"),
                }
            )
            .await,
        Err(CatalogStoreError::Domain(CatalogError::RevisionConflict {
            current_revision: 1,
            expected_revision: 0,
        }))
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::EditWork {
                    id: work.id().clone(),
                    expected_revision: 1,
                    details: work_details(" \t"),
                }
            )
            .await,
        Err(CatalogStoreError::Domain(CatalogError::BlankField {
            field: "title"
        }))
    ));
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn work_patch_fingerprints_intent_and_replays_without_resolving_again() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Story).await;
    let op = operation();
    let intent = CatalogCommand::PatchWork {
        id: work.id().clone(),
        expected_revision: 1,
        patch: WorkPatch {
            title: Some("Tên mới vẫn giữ lời giới thiệu".into()),
            ..WorkPatch::default()
        },
    };
    let first = h
        .store
        .catalog_execute(ALICE, &op, intent.clone())
        .await
        .unwrap();
    assert_eq!(first.work.details().synopsis, work.details().synopsis);
    assert_eq!(first.work.details().language, work.details().language);
    assert_eq!(first.work.details().visibility, work.details().visibility);
    assert_eq!(first.work.details().rights_refs, work.details().rights_refs);
    let later = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::PatchWork {
                id: work.id().clone(),
                expected_revision: 2,
                patch: WorkPatch {
                    synopsis: Some("Lời giới thiệu ở revision tiếp theo".into()),
                    ..WorkPatch::default()
                },
            },
        )
        .await
        .unwrap();
    let before = h.counts().await;
    assert_eq!(
        h.store.catalog_execute(ALICE, &op, intent).await.unwrap(),
        first
    );
    assert_eq!(
        h.store.catalog_work(ALICE, work.id()).await.unwrap(),
        later.work
    );
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &op,
                CatalogCommand::PatchWork {
                    id: work.id().clone(),
                    expected_revision: 1,
                    patch: WorkPatch {
                        title: Some("Tên mới vẫn giữ lời giới thiệu".into()),
                        synopsis: Some(work.details().synopsis.clone()),
                        ..WorkPatch::default()
                    },
                }
            )
            .await,
        Err(CatalogStoreError::OperationReused)
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::PatchWork {
                    id: work.id().clone(),
                    expected_revision: 2,
                    patch: WorkPatch {
                        title: Some("Không ghi revision cũ".into()),
                        ..WorkPatch::default()
                    },
                }
            )
            .await,
        Err(CatalogStoreError::Domain(CatalogError::RevisionConflict {
            current_revision: 3,
            expected_revision: 2,
        }))
    ));
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn unit_patch_preserves_or_clears_links_and_replays_original_snapshot() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Podcast).await;
    let source = h.source("alice", "pinned-patch-source").await;
    let created = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::CreateUnit {
                work_id: work.id().clone(),
                expected_work_revision: 1,
                id: unit_id(),
                kind: ContentUnitKind::Episode,
                details: UnitDetails {
                    source: Some(source.clone()),
                    content_digest: Some(source.sha256().into()),
                    ..unit_details("Tập chưa biên tập")
                },
            },
        )
        .await
        .unwrap();
    let id = created.units[0].id().clone();
    let op = operation();
    let intent = CatalogCommand::PatchUnit {
        id: id.clone(),
        expected_revision: 1,
        expected_work_revision: 2,
        patch: UnitPatch {
            title: Some("Tập đã đổi tên".into()),
            ..UnitPatch::default()
        },
    };
    let first = h
        .store
        .catalog_execute(ALICE, &op, intent.clone())
        .await
        .unwrap();
    assert_eq!(first.units[0].details().source, Some(source.clone()));
    assert_eq!(
        first.units[0].details().content_digest.as_deref(),
        Some(source.sha256())
    );
    let later = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::PatchUnit {
                id: id.clone(),
                expected_revision: 2,
                expected_work_revision: 3,
                patch: UnitPatch {
                    source: LinkPatch::Clear,
                    content_digest: LinkPatch::Clear,
                    ..UnitPatch::default()
                },
            },
        )
        .await
        .unwrap();
    assert_eq!(later.units[0].details().source, None);
    assert_eq!(later.units[0].details().content_digest, None);
    let before = h.counts().await;
    assert_eq!(
        h.store.catalog_execute(ALICE, &op, intent).await.unwrap(),
        first
    );
    assert_eq!(
        h.store.catalog_unit(ALICE, &id).await.unwrap(),
        later.units[0]
    );
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &op,
                CatalogCommand::PatchUnit {
                    id: id.clone(),
                    expected_revision: 1,
                    expected_work_revision: 2,
                    patch: UnitPatch {
                        title: Some("Tập đã đổi tên".into()),
                        source: LinkPatch::Clear,
                        ..UnitPatch::default()
                    },
                }
            )
            .await,
        Err(CatalogStoreError::OperationReused)
    ));
    let foreign = h.source("bob", "foreign-patch-source").await;
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::PatchUnit {
                    id,
                    expected_revision: 3,
                    expected_work_revision: 4,
                    patch: UnitPatch {
                        source: LinkPatch::Set(foreign),
                        ..UnitPatch::default()
                    },
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
    ));
    assert_eq!(h.counts().await, before);
    assert_eq!(h.admin.query_one("SELECT sha256 FROM source_records WHERE owner_id='alice' AND id='pinned-patch-source'", &[]).await.unwrap().get::<_, String>(0), source.sha256());
}

#[tokio::test]
async fn editor_work_patch_only_requires_owner_for_effective_protected_changes() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Story).await;
    let member = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: 1,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap()
        .work;
    let edited = h
        .store
        .catalog_execute(
            BOB,
            &operation(),
            CatalogCommand::PatchWork {
                id: work.id().clone(),
                expected_revision: 2,
                patch: WorkPatch {
                    title: Some("Biên tập viên chỉ sửa tên".into()),
                    visibility: Some(member.details().visibility),
                    rights_refs: Some(member.details().rights_refs.clone()),
                    ..WorkPatch::default()
                },
            },
        )
        .await
        .unwrap()
        .work;
    let before = h.counts().await;
    for patch in [
        WorkPatch {
            visibility: Some(Visibility::Public),
            ..WorkPatch::default()
        },
        WorkPatch {
            rights_refs: Some(vec!["catalog-rights".into()]),
            ..WorkPatch::default()
        },
    ] {
        assert!(matches!(
            h.store
                .catalog_execute(
                    BOB,
                    &operation(),
                    CatalogCommand::PatchWork {
                        id: work.id().clone(),
                        expected_revision: edited.revision(),
                        patch,
                    }
                )
                .await,
            Err(CatalogStoreError::Storage(StoreError::Forbidden))
        ));
    }
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn archived_work_owner_can_revoke_membership_and_block_history_and_replay() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Story).await;
    let unit = h.unit(ALICE, &work, "Chương riêng tư").await;
    let member = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: 2,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap()
        .work;
    let op = operation();
    let intent = CatalogCommand::PatchUnit {
        id: unit.units[0].id().clone(),
        expected_revision: 1,
        expected_work_revision: member.revision(),
        patch: UnitPatch {
            title: Some("Chương đã biên tập".into()),
            ..UnitPatch::default()
        },
    };
    let edited = h
        .store
        .catalog_execute(BOB, &op, intent.clone())
        .await
        .unwrap();
    let archived = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::ArchiveWork {
                id: work.id().clone(),
                expected_revision: edited.work.revision(),
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(
        h.store.catalog_work(BOB, work.id()).await.unwrap(),
        archived
    );
    let original_bytes: Vec<u8> = h
        .admin
        .query_one(
            "SELECT snapshot FROM catalog_work_revisions WHERE work_id=?1 AND revision=?2",
            &[&work.id().as_str(), &(archived.revision() as i64)],
        )
        .await
        .unwrap()
        .get(0);
    let revoked = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: archived.revision(),
                actor_id: OwnerId::parse("bob").unwrap(),
                role: None,
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(revoked.lifecycle(), Lifecycle::Archived);
    assert_eq!(revoked.revision(), archived.revision() + 1);
    assert_eq!(revoked.details(), archived.details());
    assert!(matches!(
        h.store.catalog_work(BOB, work.id()).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(matches!(
        h.store.catalog_unit(BOB, unit.units[0].id()).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(matches!(
        h.store.catalog_work_history(BOB, work.id(), 10, None).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(matches!(
        h.store.catalog_execute(BOB, &op, intent).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    let history = h
        .store
        .catalog_work_history(ALICE, work.id(), 10, None)
        .await
        .unwrap();
    assert_eq!(history.items.len(), 6);
    assert_eq!(history.items.last(), Some(&revoked));
    assert_eq!(
        h.admin
            .query_one(
                "SELECT snapshot FROM catalog_work_revisions WHERE work_id=?1 AND revision=?2",
                &[&work.id().as_str(), &(archived.revision() as i64)]
            )
            .await
            .unwrap()
            .get::<_, Vec<u8>>(0),
        original_bytes
    );
    assert_eq!(h.counts().await["members"], 0);
    assert_eq!(h.counts().await["unit_revisions"], 2);
}

#[tokio::test]
async fn identical_operation_replays_original_snapshot_but_changed_payload_conflicts() {
    let h = Harness::new().await;
    let id = work_id();
    let intent = CatalogCommand::CreateWork {
        id: id.clone(),
        kind: WorkKind::Story,
        details: work_details("Ngày đầu tiên"),
    };
    let op = operation();
    let first = h
        .store
        .catalog_execute(ALICE, &op, intent.clone())
        .await
        .unwrap();
    let later = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::EditWork {
                id: id.clone(),
                expected_revision: 1,
                details: work_details("Ngày thứ hai"),
            },
        )
        .await
        .unwrap();
    let before = h.counts().await;
    assert_eq!(
        h.store.catalog_execute(ALICE, &op, intent).await.unwrap(),
        first
    );
    assert_eq!(h.store.catalog_work(ALICE, &id).await.unwrap(), later.work);
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &op,
                CatalogCommand::CreateWork {
                    id,
                    kind: WorkKind::Story,
                    details: work_details("Payload khác"),
                }
            )
            .await,
        Err(CatalogStoreError::OperationReused)
    ));
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn duplicate_unit_ids_never_reparent_and_foreign_ids_do_not_disclose_existence() {
    let h = Harness::new().await;
    let alice = h.create(ALICE, WorkKind::Story).await;
    let alice_unit = h.unit(ALICE, &alice, "Chương của Alice").await;
    let bob = h.create(BOB, WorkKind::Story).await;
    let bob_unit = h.unit(BOB, &bob, "Chương của Bob").await;
    let before = h.counts().await;
    for (id, foreign) in [
        (alice_unit.units[0].id(), false),
        (bob_unit.units[0].id(), true),
    ] {
        let result = h
            .store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::CreateUnit {
                    work_id: alice.id().clone(),
                    expected_work_revision: 2,
                    id: id.clone(),
                    kind: ContentUnitKind::Chapter,
                    details: unit_details("Không đổi cha của chương"),
                },
            )
            .await;
        if foreign {
            assert!(matches!(
                result,
                Err(CatalogStoreError::Storage(StoreError::NotFound))
            ));
        } else {
            assert!(matches!(result, Err(CatalogStoreError::DuplicateId)));
        }
    }
    assert_eq!(h.counts().await, before);
    assert!(matches!(
        h.store.catalog_unit(ALICE, bob_unit.units[0].id()).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::CreateWork {
                    id: bob.id().clone(),
                    kind: WorkKind::Story,
                    details: work_details("Không lộ tác phẩm của Bob"),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
}

#[tokio::test]
async fn owner_reader_editor_and_revocation_are_checked_at_application_boundary() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Novel).await;
    assert!(matches!(
        h.store.catalog_work(BOB, work.id()).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                BOB,
                &operation(),
                CatalogCommand::EditWork {
                    id: work.id().clone(),
                    expected_revision: 1,
                    details: work_details("Không có quyền"),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    let reader = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: 1,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Reader),
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(h.store.catalog_work(BOB, work.id()).await.unwrap(), reader);
    assert!(matches!(
        h.store
            .catalog_execute(
                BOB,
                &operation(),
                CatalogCommand::EditWork {
                    id: work.id().clone(),
                    expected_revision: 2,
                    details: work_details("Chỉ đọc"),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::Forbidden))
    ));
    let editor = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: 2,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap()
        .work;
    let created_unit = h.unit(BOB, &editor, "Biên tập viên tạo chương").await;
    let edited = h
        .store
        .catalog_execute(
            BOB,
            &operation(),
            CatalogCommand::EditWork {
                id: work.id().clone(),
                expected_revision: created_unit.work.revision(),
                details: work_details("Biên tập viên sửa thông tin"),
            },
        )
        .await
        .unwrap()
        .work;
    let before_denial = h.counts().await;
    for protected in [
        WorkDetails {
            visibility: Visibility::Public,
            ..edited.details().clone()
        },
        WorkDetails {
            rights_refs: vec!["catalog-rights".into()],
            ..edited.details().clone()
        },
    ] {
        assert!(matches!(
            h.store
                .catalog_execute(
                    BOB,
                    &operation(),
                    CatalogCommand::EditWork {
                        id: work.id().clone(),
                        expected_revision: edited.revision(),
                        details: protected,
                    }
                )
                .await,
            Err(CatalogStoreError::Storage(StoreError::Forbidden))
        ));
    }
    assert!(matches!(
        h.store
            .catalog_execute(
                BOB,
                &operation(),
                CatalogCommand::ArchiveUnit {
                    id: created_unit.units[0].id().clone(),
                    expected_revision: 1,
                    expected_work_revision: edited.revision(),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::Forbidden))
    ));
    assert_eq!(h.counts().await, before_denial);
    assert!(matches!(
        h.store
            .catalog_execute(
                BOB,
                &operation(),
                CatalogCommand::ArchiveWork {
                    id: work.id().clone(),
                    expected_revision: edited.revision(),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::Forbidden))
    ));
    assert!(matches!(
        h.store
            .catalog_execute(
                BOB,
                &operation(),
                CatalogCommand::SetMember {
                    work_id: work.id().clone(),
                    expected_revision: edited.revision(),
                    actor_id: OwnerId::parse("carol").unwrap(),
                    role: Some(MemberRole::Reader),
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::Forbidden))
    ));
    h.store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: edited.revision(),
                actor_id: OwnerId::parse("bob").unwrap(),
                role: None,
            },
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store.catalog_work(BOB, work.id()).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(h
        .store
        .catalog_works(BOB, 10, true, None)
        .await
        .unwrap()
        .items
        .is_empty());
    h.admin
        .execute(
            "UPDATE development_sessions SET revoked=1 WHERE actor_id='alice'",
            &[],
        )
        .await
        .unwrap();
    assert!(matches!(
        h.store.catalog_work(ALICE, work.id()).await,
        Err(CatalogStoreError::Storage(StoreError::Unauthenticated))
    ));
}

#[tokio::test]
async fn owner_can_remove_inactive_member_and_reactivation_does_not_restore_access() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Story).await;
    let granted = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: 1,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: Some(MemberRole::Editor),
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(h.store.catalog_work(BOB, work.id()).await.unwrap(), granted);
    h.admin
        .execute("UPDATE actors SET active=0 WHERE id='bob'", &[])
        .await
        .unwrap();
    let before = h.counts().await;
    let removed = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::SetMember {
                work_id: work.id().clone(),
                expected_revision: 2,
                actor_id: OwnerId::parse("bob").unwrap(),
                role: None,
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(removed.revision(), 3);
    assert_eq!(removed.id(), work.id());
    let mut expected = before;
    expected["members"] = json!(0);
    expected["work_revisions"] = json!(3);
    expected["operations"] = json!(3);
    expected["clock"] = json!(3);
    assert_eq!(h.counts().await, expected);
    h.admin
        .execute("UPDATE actors SET active=1 WHERE id='bob'", &[])
        .await
        .unwrap();
    assert!(matches!(
        h.store.catalog_work(BOB, work.id()).await,
        Err(CatalogStoreError::Storage(StoreError::NotFound))
    ));
    assert!(h
        .store
        .catalog_works(BOB, 10, true, None)
        .await
        .unwrap()
        .items
        .is_empty());
}

#[tokio::test]
async fn reorder_is_atomic_stable_identity_and_archival_preserves_history() {
    let h = Harness::new().await;
    let initial = h.create(ALICE, WorkKind::Novel).await;
    let first = h.unit(ALICE, &initial, "Chương một").await;
    let second = h.unit(ALICE, &first.work, "Chương hai").await;
    let units = h
        .store
        .catalog_units(ALICE, initial.id(), 10, true, None)
        .await
        .unwrap()
        .items;
    let ids: Vec<_> = units.iter().map(|unit| unit.id().clone()).collect();
    let before = h.counts().await;
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::Reorder {
                    work_id: initial.id().clone(),
                    expected_revision: second.work.revision(),
                    ordered: vec![ids[0].clone(), ids[0].clone()],
                }
            )
            .await,
        Err(CatalogStoreError::Domain(
            CatalogError::DuplicateReorderUnit { .. }
        ))
    ));
    assert_eq!(h.counts().await, before);
    let reversed: Vec<_> = ids.iter().rev().cloned().collect();
    let reordered = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::Reorder {
                work_id: initial.id().clone(),
                expected_revision: second.work.revision(),
                ordered: reversed.clone(),
            },
        )
        .await
        .unwrap();
    let current = h
        .store
        .catalog_units(ALICE, initial.id(), 10, true, None)
        .await
        .unwrap()
        .items;
    assert_eq!(
        current
            .iter()
            .map(|unit| unit.id().clone())
            .collect::<Vec<_>>(),
        reversed
    );
    for (position, unit) in current.iter().enumerate() {
        assert_eq!(unit.position(), position as u32);
        assert_eq!(unit.work_id(), initial.id());
        assert_eq!(unit.revision(), 2);
    }
    let archived_unit = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::ArchiveUnit {
                id: current[0].id().clone(),
                expected_revision: 2,
                expected_work_revision: reordered.work.revision(),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        h.store
            .catalog_unit(ALICE, current[0].id())
            .await
            .unwrap()
            .lifecycle(),
        Lifecycle::Archived
    );
    assert_eq!(
        h.store
            .catalog_units(ALICE, initial.id(), 10, false, None)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let archived = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::ArchiveWork {
                id: initial.id().clone(),
                expected_revision: archived_unit.work.revision(),
            },
        )
        .await
        .unwrap()
        .work;
    assert_eq!(archived.lifecycle(), Lifecycle::Archived);
    assert!(h
        .store
        .catalog_works(ALICE, 10, false, None)
        .await
        .unwrap()
        .items
        .is_empty());
    let history = h
        .store
        .catalog_work_history(ALICE, initial.id(), 50, None)
        .await
        .unwrap()
        .items;
    assert_eq!(history.first(), Some(&initial));
    assert_eq!(history.last(), Some(&archived));
    assert_eq!(
        history.iter().map(Work::revision).collect::<Vec<_>>(),
        (1..=archived.revision()).collect::<Vec<_>>()
    );
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::EditWork {
                    id: initial.id().clone(),
                    expected_revision: archived.revision(),
                    details: work_details("Không hồi sinh ngầm"),
                }
            )
            .await,
        Err(CatalogStoreError::Domain(CatalogError::WorkArchived))
    ));
    let before = h.counts().await;
    assert_eq!(
        h.admin
            .query_one("SELECT count(*) FROM catalog_unit_revisions", &[])
            .await
            .unwrap()
            .get::<_, i64>(0),
        5
    );
    assert_eq!(h.counts().await, before);
}

#[tokio::test]
async fn optional_legacy_links_require_owner_and_exact_pinned_digests() {
    let h = Harness::new().await;
    let source = h.source("alice", "catalog-manuscript").await;
    let foreign_source = h.source("bob", "bob-manuscript").await;
    let script = h.script(ALICE).await;
    let foreign_script = h.script(BOB).await;
    let work = h.create(ALICE, WorkKind::Podcast).await;
    let source_bytes: Vec<u8> = h.admin.query_one("SELECT coalesce(original_bytes,CAST(original_text AS BLOB)) FROM source_records WHERE owner_id='alice' AND id='catalog-manuscript'", &[]).await.unwrap().get(0);
    let export_bytes: Vec<u8> = h
        .admin
        .query_one(
            "SELECT canonical_export FROM script_revisions WHERE script_id=?1 AND revision=1",
            &[&script.id()],
        )
        .await
        .unwrap()
        .get(0);
    let before = h.counts().await;
    for details in [
        UnitDetails {
            source: Some(foreign_source),
            ..unit_details("Nguồn của người khác")
        },
        UnitDetails {
            script: Some(foreign_script),
            ..unit_details("Kịch bản của người khác")
        },
        UnitDetails {
            source: Some(SourceRef::new(source.id().into(), "0".repeat(64)).unwrap()),
            ..unit_details("Checksum sai")
        },
        UnitDetails {
            script: Some(
                ScriptRef::new(
                    script.id().into(),
                    1,
                    format!("sir-e1:sha256:{}", "0".repeat(64)),
                )
                .unwrap(),
            ),
            ..unit_details("Export sai")
        },
    ] {
        assert!(matches!(
            h.store
                .catalog_execute(
                    ALICE,
                    &operation(),
                    CatalogCommand::CreateUnit {
                        work_id: work.id().clone(),
                        expected_work_revision: 1,
                        id: unit_id(),
                        kind: ContentUnitKind::Episode,
                        details,
                    }
                )
                .await,
            Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
        ));
    }
    let mut wrong_rights = work_details("Quyền không tồn tại");
    wrong_rights.rights_refs = vec!["missing-rights".into()];
    assert!(matches!(
        h.store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::EditWork {
                    id: work.id().clone(),
                    expected_revision: 1,
                    details: wrong_rights,
                }
            )
            .await,
        Err(CatalogStoreError::Storage(StoreError::EvidenceUnavailable))
    ));
    assert_eq!(h.counts().await, before);
    let linked = h
        .store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::CreateUnit {
                work_id: work.id().clone(),
                expected_work_revision: 1,
                id: unit_id(),
                kind: ContentUnitKind::Episode,
                details: UnitDetails {
                    source: Some(source.clone()),
                    script: Some(script.clone()),
                    ..unit_details("Tham chiếu được giữ nguyên")
                },
            },
        )
        .await
        .unwrap();
    let unit = linked
        .units
        .iter()
        .find(|unit| unit.details().source.is_some())
        .unwrap();
    assert_eq!(unit.details().source, Some(source));
    assert_eq!(unit.details().script, Some(script.clone()));
    h.store
        .save(
            ALICE,
            script.id(),
            SaveRevisionRequest {
                operation_id: operation(),
                expected_revision: 1,
                script_json: SCRIPT.replace("Ngày mai", "Tối mai"),
            },
        )
        .await
        .unwrap();
    assert_eq!(
        h.store
            .catalog_unit(ALICE, unit.id())
            .await
            .unwrap()
            .details()
            .script,
        Some(script.clone())
    );
    assert_eq!(h.admin.query_one("SELECT coalesce(original_bytes,CAST(original_text AS BLOB)) FROM source_records WHERE owner_id='alice' AND id='catalog-manuscript'", &[]).await.unwrap().get::<_,Vec<u8>>(0), source_bytes);
    assert_eq!(
        h.admin
            .query_one(
                "SELECT canonical_export FROM script_revisions WHERE script_id=?1 AND revision=1",
                &[&script.id()]
            )
            .await
            .unwrap()
            .get::<_, Vec<u8>>(0),
        export_bytes
    );
}

#[tokio::test]
async fn deterministic_pagination_rejects_stale_epoch_and_cross_actor_cursor() {
    let h = Harness::new().await;
    let mut ids = Vec::new();
    for _ in 0..3 {
        ids.push(h.create(ALICE, WorkKind::Story).await.id().clone());
    }
    ids.sort();
    let first = h.store.catalog_works(ALICE, 1, false, None).await.unwrap();
    assert_eq!(first.items[0].id(), &ids[0]);
    let cursor = first.next.unwrap();
    let second = h
        .store
        .catalog_works(ALICE, 1, false, Some(&cursor))
        .await
        .unwrap();
    assert_eq!(second.items[0].id(), &ids[1]);
    let last = h
        .store
        .catalog_works(ALICE, 1, false, second.next.as_ref())
        .await
        .unwrap();
    assert_eq!(last.items[0].id(), &ids[2]);
    assert!(last.next.is_none());
    assert!(matches!(
        h.store.catalog_works(BOB, 1, false, Some(&cursor)).await,
        Err(CatalogStoreError::InvalidCursor)
    ));
    assert!(matches!(
        h.store.catalog_works(ALICE, 1, true, Some(&cursor)).await,
        Err(CatalogStoreError::InvalidCursor)
    ));
    h.create(ALICE, WorkKind::Story).await;
    assert!(matches!(
        h.store.catalog_works(ALICE, 1, false, Some(&cursor)).await,
        Err(CatalogStoreError::StaleCursor)
    ));
    assert!(matches!(
        h.store.catalog_works(ALICE, 0, false, None).await,
        Err(CatalogStoreError::Limit)
    ));
}

#[tokio::test]
async fn concurrent_edit_and_reorder_have_one_winner_without_partial_child_changes() {
    let h = Harness::new().await;
    let first = h.create(ALICE, WorkKind::Story).await;
    let one = h.unit(ALICE, &first, "Một").await;
    let two = h.unit(ALICE, &one.work, "Hai").await;
    let units = h
        .store
        .catalog_units(ALICE, first.id(), 10, true, None)
        .await
        .unwrap()
        .items;
    let epoch = h.counts().await["clock"].as_i64().unwrap();
    let tx = h.admin.0.writer().await;
    let edit = CatalogCommand::EditWork {
        id: first.id().clone(),
        expected_revision: two.work.revision(),
        details: work_details("Chỉnh sửa đồng thời"),
    };
    let reorder = CatalogCommand::Reorder {
        work_id: first.id().clone(),
        expected_revision: two.work.revision(),
        ordered: units.iter().rev().map(|u| u.id().clone()).collect(),
    };
    let op1 = operation();
    let op2 = operation();
    let mut a = Box::pin(h.store.catalog_execute(ALICE, &op1, edit));
    let mut b = Box::pin(h.store.catalog_execute(ALICE, &op2, reorder));
    assert!(futures_util::poll!(&mut a).is_pending());
    assert!(futures_util::poll!(&mut b).is_pending());
    tx.commit().await.unwrap();
    let (a, b) = tokio::join!(a, b);
    assert_ne!(a.is_ok(), b.is_ok());
    let error = match a {
        Ok(_) => b.unwrap_err(),
        Err(error) => error,
    };
    assert!(matches!(
        error,
        CatalogStoreError::Domain(CatalogError::RevisionConflict { .. })
    ));
    let after = h
        .store
        .catalog_units(ALICE, first.id(), 10, true, None)
        .await
        .unwrap()
        .items;
    assert_eq!(
        h.store
            .catalog_work(ALICE, first.id())
            .await
            .unwrap()
            .revision(),
        two.work.revision() + 1
    );
    assert_eq!(h.counts().await["clock"], epoch + 1);
    assert!(
        after == units
            || after
                .iter()
                .map(|u| u.id())
                .eq(units.iter().rev().map(|u| u.id()))
    );
    assert_eq!(
        h.counts().await["unit_revisions"],
        if after == units { 2 } else { 4 }
    );
}
#[tokio::test]
async fn expiry_and_membership_revocation_while_waiting_reauthenticate_before_commit() {
    for expired in [false, true] {
        let clock = cantos_store_turso::testing::FixedClock::new(1_800_000_000_000_000);
        let h = Harness::with_fixture(NativeSql(
            cantos_server::test_support::ServerFixture::with_clock(clock.clone()).await,
        ))
        .await;
        let first = h.create(ALICE, WorkKind::Story).await;
        let member = h
            .store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::SetMember {
                    work_id: first.id().clone(),
                    expected_revision: 1,
                    actor_id: OwnerId::parse("bob").unwrap(),
                    role: Some(MemberRole::Editor),
                },
            )
            .await
            .unwrap();
        let counts = h.counts().await;
        let blocker = h.admin.0.writer().await;
        let op = operation();
        let command = CatalogCommand::EditWork {
            id: first.id().clone(),
            expected_revision: member.work.revision(),
            details: work_details("Chờ quyền hiện tại"),
        };
        let mut waiting = Box::pin(h.store.catalog_execute(BOB, &op, command));
        assert!(futures_util::poll!(&mut waiting).is_pending());
        if expired {
            clock.advance(3_600_000_001);
        } else {
            blocker
                .execute("DELETE FROM catalog_members WHERE actor_id='bob'", ())
                .await
                .unwrap();
        }
        blocker.commit().await.unwrap();
        let rejected = waiting.await.unwrap_err();
        assert!(if expired {
            matches!(
                rejected,
                CatalogStoreError::Storage(StoreError::Unauthenticated)
            )
        } else {
            matches!(rejected, CatalogStoreError::Storage(StoreError::NotFound))
        });
        let after = h.counts().await;
        for key in [
            "works",
            "work_revisions",
            "units",
            "unit_revisions",
            "operations",
            "clock",
        ] {
            assert_eq!(after[key], counts[key]);
        }
        assert_eq!(after["members"], if expired { 1 } else { 0 });
    }
}
#[tokio::test]
async fn settled_catalog_records_are_immutable_and_app_has_no_delete_identity_update_or_ddl() {
    let h = Harness::new().await;
    let w = h.create(ALICE, WorkKind::Story).await;
    h.unit(ALICE, &w, "Một").await;
    for table in [
        "catalog_work_revisions",
        "catalog_unit_revisions",
        "catalog_operations",
    ] {
        for statement in [
            format!("DELETE FROM {table}"),
            format!("REPLACE INTO {table} SELECT * FROM {table}"),
        ] {
            assert!(
                h.admin
                    .0
                    .try_exec(&statement, &[])
                    .await
                    .unwrap_err()
                    .guard()
                    .is_some(),
                "{statement}"
            );
        }
    }
    for statement in [
        "UPDATE catalog_works SET owner_id='bob'",
        "DELETE FROM catalog_works",
        "UPDATE catalog_units SET work_kind='podcast'",
        "DELETE FROM catalog_units",
        "UPDATE catalog_work_revisions SET snapshot=snapshot",
        "UPDATE catalog_unit_revisions SET snapshot=snapshot",
        "UPDATE catalog_operations SET result_bytes=result_bytes",
    ] {
        assert!(
            h.admin
                .0
                .try_exec(statement, &[])
                .await
                .unwrap_err()
                .guard()
                .is_some(),
            "{statement}"
        );
    }
    // File/process ownership and read-only connections replace PG grants; the shared owner suite
    // separately rejects writes through a reader. No table-level SQL privilege claim is made.
}
#[tokio::test]
async fn baseline_replay_preserves_exact_source_script_and_catalog_bytes() {
    let h = Harness::new().await;
    let source = h.source("alice", "baseline-manuscript").await;
    let script = h.script(ALICE).await;
    let work = h.create(ALICE, WorkKind::Story).await;
    let sql="SELECT json_array((SELECT json_array(hex(CAST(original_text AS BLOB)),sha256,recorded_at) FROM source_records WHERE id='baseline-manuscript'),(SELECT json_array(hex(canonical_export),content_digest,export_digest,accepted_at,accepted_by) FROM script_revisions WHERE script_id=?1 AND revision=1),(SELECT json_array(hex(snapshot),snapshot_digest,recorded_by) FROM catalog_work_revisions WHERE work_id=?2 AND revision=1),(SELECT revision FROM catalog_clock))";
    let before: String = h
        .admin
        .query_one(sql, &[&script.id(), &work.id().as_str()])
        .await
        .unwrap()
        .get(0);
    let Harness { mut admin, store } = h;
    drop(store);
    admin.0.close();
    for _ in 0..2 {
        let owner = Store::open(
            admin.0.database_path(),
            cantos_store_turso::registry::Intent::Migrate,
        )
        .await
        .unwrap();
        drop(owner);
    }
    admin.0.restart().await;
    let h = Harness {
        store: admin.store(),
        admin,
    };
    let after: String = h
        .admin
        .query_one(sql, &[&script.id(), &work.id().as_str()])
        .await
        .unwrap()
        .get(0);
    assert_eq!(after, before);
    assert_eq!(h.store.catalog_work(ALICE, work.id()).await.unwrap(), work);
    h.store
        .catalog_execute(
            ALICE,
            &operation(),
            CatalogCommand::CreateUnit {
                work_id: work.id().clone(),
                expected_work_revision: 1,
                id: unit_id(),
                kind: ContentUnitKind::Chapter,
                details: UnitDetails {
                    source: Some(source),
                    script: Some(script),
                    ..unit_details("Sau mở lại")
                },
            },
        )
        .await
        .unwrap();
}
#[tokio::test]
async fn committed_catalog_fixture_survives_new_store_and_emits_server_restart_oracle() {
    let h = Harness::new().await;
    let original = h.create(ALICE, WorkKind::Novel).await;
    let unit = h.unit(ALICE, &original, "Giữ nguyên").await;
    let before = h.counts().await;
    let Harness { mut admin, store } = h;
    drop(store);
    admin.0.restart().await;
    let h = Harness {
        store: admin.store(),
        admin,
    };
    assert_eq!(
        h.store.catalog_work(ALICE, original.id()).await.unwrap(),
        unit.work
    );
    assert_eq!(
        h.store
            .catalog_unit(ALICE, unit.units[0].id())
            .await
            .unwrap(),
        unit.units[0]
    );
    assert_eq!(h.counts().await, before);
}
#[tokio::test]
async fn authorized_read_baseline_is_bounded_and_records_select_plans_at_one_hundred_works() {
    let h = Harness::new().await;
    for i in 0..100 {
        h.create(if i < 80 { ALICE } else { BOB }, WorkKind::Story)
            .await;
    }
    let mut times = Vec::new();
    for i in 0..35 {
        let start = std::time::Instant::now();
        let page = h.store.catalog_works(ALICE, 20, false, None).await.unwrap();
        assert_eq!(page.items.len(), 20);
        assert!(page.items.iter().all(|w| w.owner_id().as_str() == "alice"));
        if i >= 5 {
            times.push(start.elapsed().as_secs_f64() * 1000.);
        }
    }
    let plan=h.admin.query("EXPLAIN QUERY PLAN SELECT w.id,w.owner_id,w.kind,w.lifecycle,w.head_revision,r.revision,r.snapshot,r.snapshot_digest FROM catalog_works w LEFT JOIN catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision WHERE (w.owner_id=?1 OR EXISTS(SELECT 1 FROM catalog_members m WHERE m.work_id=w.id AND m.actor_id=?1)) AND (?2 IS NULL OR w.id>?2) AND (?3 OR w.lifecycle<>'archived') ORDER BY w.id LIMIT ?4",&[&"alice",&None::<String>,&false,&21_i64]).await.unwrap();
    let plan: Vec<String> = plan.iter().map(|r| r.get(3)).collect();
    assert!(plan
        .iter()
        .any(|p| p.contains("catalog_members") && p.contains("INDEX")));
    assert!(plan
        .iter()
        .any(|p| p.contains("catalog_work_revisions") && p.contains("INDEX")));
    times.sort_by(f64::total_cmp);
    assert!(
        times[28] < 500.0,
        "local p95 target, 100 synthetic works: {times:?}"
    );
    eprintln!("Turso 0.8.3; 100 works/80 owned; 5 warmup/30 samples ms={times:?}; plan={plan:?}; no production SLO or speedup claim");
}
