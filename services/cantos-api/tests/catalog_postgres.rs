//! Catalog foundation boundary evidence. Only the disposable-cluster xtask runner enables it.
use std::{
    env, fs,
    time::{Duration, Instant},
};

use cantos_api::{
    contexts::catalog::{
        domain::{
            CatalogError, ContentUnitId, ContentUnitKind, Lifecycle, LinkPatch, OwnerId, ScriptRef,
            SourceRef, UnitDetails, UnitPatch, Visibility, Work, WorkDetails, WorkId, WorkKind,
            WorkPatch,
        },
        CatalogCommand, CatalogOutcome, CatalogStoreError, MemberRole,
    },
    platform::postgres::{local_config, migrate, token_hash, Store, StoreError},
    script_ir::read_script,
};
use cantos_contracts::SaveRevisionRequest;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use tokio_postgres::{Client, Config, NoTls};
use uuid::Uuid;

const ALICE: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const BOB: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const CAROL: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const SCRIPT: &str =
    include_str!("../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json");
const ORIGINAL: &str = "\u{feff}Ánh đèn cuối sân khấu\r\nNgười kể chuyện đã trở về.\r\n";
const EVIDENCE: &str = "../../target/catalog-evidence";
const BASELINE: [&str; 9] = [
    include_str!("../migrations/0001_script_revisions.sql"),
    include_str!("../migrations/0002_editorial_handoff.sql"),
    include_str!("../migrations/0003_manuscript_import.sql"),
    include_str!("../migrations/0004_ai_adaptation.sql"),
    include_str!("../migrations/0005_caller_adaptation.sql"),
    include_str!("../migrations/0006_production_inputs.sql"),
    include_str!("../migrations/0007_durable_jobs.sql"),
    include_str!("../migrations/0008_speech_clips.sql"),
    include_str!("../migrations/0009_durable_step_changes.sql"),
];

struct Harness {
    admin: Client,
    config: Config,
    app_config: Config,
    store: Store,
    database: String,
}

async fn connect(config: &Config) -> Client {
    let (client, connection) = config.connect(NoTls).await.unwrap();
    tokio::spawn(async move {
        let _ = connection.await;
    });
    client
}

async fn database_config() -> (Config, String) {
    let url = env::var("CANTOS_TEST_CLUSTER_URL").expect("use cargo xtask verify postgres");
    let mut config = local_config(&url).unwrap();
    assert_eq!(config.get_user(), Some("cantos_test_admin"));
    assert_eq!(config.get_dbname(), Some("postgres"));
    let database = format!("cantos_test_{}", Uuid::new_v4().simple());
    connect(&config)
        .await
        .batch_execute(&format!("CREATE DATABASE {database}"))
        .await
        .unwrap();
    config.dbname(&database);
    (config, database)
}

async fn grant_and_seed(admin: &Client) {
    admin.batch_execute(
        "GRANT USAGE ON SCHEMA public TO cantos_app;
         GRANT SELECT ON actors,sessions,scripts,script_revisions,script_members,script_evidence,source_records,revision_evidence,
         catalog_clock,catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations TO cantos_app;
         GRANT INSERT ON scripts,script_revisions,revision_evidence,catalog_works,catalog_work_revisions,catalog_units,catalog_unit_revisions,catalog_members,catalog_operations TO cantos_app;
         GRANT UPDATE(head_revision) ON scripts TO cantos_app;
         GRANT UPDATE(revoked) ON sessions TO cantos_app;
         GRANT UPDATE(revision) ON catalog_clock TO cantos_app;
         GRANT UPDATE(head_revision,lifecycle) ON catalog_works TO cantos_app;
         GRANT UPDATE(head_revision,position,lifecycle) ON catalog_units TO cantos_app;
         GRANT UPDATE(role),DELETE ON catalog_members TO cantos_app;
         GRANT EXECUTE ON FUNCTION catalog_lock_actor(text),catalog_lock_member(text,text) TO cantos_app;
         INSERT INTO actors(id) VALUES('alice'),('bob'),('carol') ON CONFLICT DO NOTHING;",
    ).await.unwrap();
    for (actor, token) in [("alice", ALICE), ("bob", BOB), ("carol", CAROL)] {
        admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,$2,clock_timestamp()+interval '1 hour') ON CONFLICT DO NOTHING", &[&token_hash(token),&actor]).await.unwrap();
        for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
            admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,$2,$3,'Synthetic reference; no production or publication clearance') ON CONFLICT DO NOTHING", &[&actor,&kind,&id]).await.unwrap();
        }
        admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,'rights','catalog-rights','Synthetic catalog rights reference') ON CONFLICT DO NOTHING", &[&actor]).await.unwrap();
    }
}

impl Harness {
    async fn new() -> Self {
        let (config, database) = database_config().await;
        migrate(&config).await.unwrap();
        let admin = connect(&config).await;
        grant_and_seed(&admin).await;
        Self::from_connections(config, database, admin)
    }

    fn from_connections(config: Config, database: String, admin: Client) -> Self {
        let mut app_config = config.clone();
        app_config
            .user("cantos_app")
            .application_name("cantos-catalog-test");
        let store = Store::new(app_config.clone()).unwrap();
        Self {
            admin,
            config,
            app_config,
            store,
            database,
        }
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
        let encoded = self.admin.query_one("SELECT json_build_object('works',(SELECT count(*) FROM catalog_works),'work_revisions',(SELECT count(*) FROM catalog_work_revisions),'units',(SELECT count(*) FROM catalog_units),'unit_revisions',(SELECT count(*) FROM catalog_unit_revisions),'members',(SELECT count(*) FROM catalog_members),'operations',(SELECT count(*) FROM catalog_operations),'clock',(SELECT revision FROM catalog_clock))::text", &[]).await.unwrap().get::<_,String>(0);
        serde_json::from_str(&encoded).unwrap()
    }

    async fn source(&self, actor: &str, id: &str) -> SourceRef {
        let checksum = format!("{:x}", Sha256::digest(ORIGINAL.as_bytes()));
        self.admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES($1,'source',$2,'Original synthetic manuscript bytes')", &[&actor,&id]).await.unwrap();
        self.admin.execute("INSERT INTO source_records(owner_id,id,reference,original_text,sha256) VALUES($1,$2,'synthetic://catalog/manuscript',$3,$4)", &[&actor,&id,&ORIGINAL,&checksum]).await.unwrap();
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
            "SELECT snapshot FROM catalog_work_revisions WHERE work_id=$1 AND revision=$2",
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
                "SELECT snapshot FROM catalog_work_revisions WHERE work_id=$1 AND revision=$2",
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
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
            "UPDATE sessions SET revoked=true WHERE actor_id='alice'",
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
#[ignore = "disposable PostgreSQL cluster required"]
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
        .batch_execute("UPDATE actors SET active=false WHERE id='bob'")
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
        .batch_execute("UPDATE actors SET active=true WHERE id='bob'")
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
#[ignore = "disposable PostgreSQL cluster required"]
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
#[ignore = "disposable PostgreSQL cluster required"]
async fn optional_legacy_links_require_owner_and_exact_pinned_digests() {
    let h = Harness::new().await;
    let source = h.source("alice", "catalog-manuscript").await;
    let foreign_source = h.source("bob", "bob-manuscript").await;
    let script = h.script(ALICE).await;
    let foreign_script = h.script(BOB).await;
    let work = h.create(ALICE, WorkKind::Podcast).await;
    let source_bytes: Vec<u8> = h.admin.query_one("SELECT coalesce(original_bytes,convert_to(original_text,'UTF8')) FROM source_records WHERE owner_id='alice' AND id='catalog-manuscript'", &[]).await.unwrap().get(0);
    let export_bytes: Vec<u8> = h
        .admin
        .query_one(
            "SELECT canonical_export FROM script_revisions WHERE script_id=$1 AND revision=1",
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
    assert_eq!(h.admin.query_one("SELECT coalesce(original_bytes,convert_to(original_text,'UTF8')) FROM source_records WHERE owner_id='alice' AND id='catalog-manuscript'", &[]).await.unwrap().get::<_,Vec<u8>>(0), source_bytes);
    assert_eq!(
        h.admin
            .query_one(
                "SELECT canonical_export FROM script_revisions WHERE script_id=$1 AND revision=1",
                &[&script.id()]
            )
            .await
            .unwrap()
            .get::<_, Vec<u8>>(0),
        export_bytes
    );
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
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

async fn blocked(admin: &Client, count: i64) {
    let deadline = Instant::now() + Duration::from_secs(2);
    loop {
        let waiting: i64 = admin.query_one("SELECT count(*) FROM pg_stat_activity WHERE application_name='cantos-catalog-test' AND wait_event_type='Lock'", &[]).await.unwrap().get(0);
        if waiting >= count {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "catalog operation did not reach the lock barrier"
        );
        tokio::task::yield_now().await;
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn concurrent_edit_and_reorder_have_one_winner_without_partial_child_changes() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Novel).await;
    let work = h.unit(ALICE, &work, "Một").await.work;
    let work = h.unit(ALICE, &work, "Hai").await.work;
    let units = h
        .store
        .catalog_units(ALICE, work.id(), 10, false, None)
        .await
        .unwrap()
        .items;
    let mut blocker = connect(&h.config).await;
    let tx = blocker.transaction().await.unwrap();
    tx.query_one(
        "SELECT revision FROM catalog_clock WHERE singleton FOR UPDATE",
        &[],
    )
    .await
    .unwrap();
    let store = h.store.clone();
    let id = work.id().clone();
    let revision = work.revision();
    let edit = tokio::spawn(async move {
        store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::EditWork {
                    id,
                    expected_revision: revision,
                    details: work_details("Đồng thời sửa"),
                },
            )
            .await
    });
    blocked(&h.admin, 1).await;
    let store = h.store.clone();
    let id = work.id().clone();
    let reordered = units.iter().rev().map(|unit| unit.id().clone()).collect();
    let reorder = tokio::spawn(async move {
        store
            .catalog_execute(
                ALICE,
                &operation(),
                CatalogCommand::Reorder {
                    work_id: id,
                    expected_revision: revision,
                    ordered: reordered,
                },
            )
            .await
    });
    blocked(&h.admin, 2).await;
    tx.commit().await.unwrap();
    let results = [edit.await.unwrap(), reorder.await.unwrap()];
    assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
    assert_eq!(
        results
            .iter()
            .filter(|result| matches!(
                result,
                Err(CatalogStoreError::Domain(CatalogError::RevisionConflict {
                    current_revision: 4,
                    expected_revision: 3,
                }))
            ))
            .count(),
        1
    );
    let current = h
        .store
        .catalog_units(ALICE, work.id(), 10, false, None)
        .await
        .unwrap()
        .items;
    let ids: Vec<_> = current.iter().map(|unit| unit.id()).collect();
    let original: Vec<_> = units.iter().map(|unit| unit.id()).collect();
    assert!(ids == original || ids == original.into_iter().rev().collect::<Vec<_>>());
    assert_eq!(h.counts().await["operations"], 4);
    assert_eq!(h.counts().await["work_revisions"], 4);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn expiry_and_membership_revocation_while_waiting_reauthenticate_before_commit() {
    for (expired, work_lock) in [(false, false), (true, false), (false, true), (true, true)] {
        let h = Harness::new().await;
        let work = h.create(ALICE, WorkKind::Story).await;
        let work = h
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
        let before = h.counts().await;
        let mut blocker = connect(&h.config).await;
        let tx = blocker.transaction().await.unwrap();
        if work_lock {
            tx.query_one(
                "SELECT id FROM catalog_works WHERE id=$1 FOR UPDATE",
                &[&work.id().as_str()],
            )
            .await
            .unwrap();
        } else {
            tx.query_one(
                "SELECT revision FROM catalog_clock WHERE singleton FOR UPDATE",
                &[],
            )
            .await
            .unwrap();
        }
        if expired {
            h.admin.execute("UPDATE sessions SET expires_at=clock_timestamp()+interval '200 milliseconds' WHERE actor_id='bob'", &[]).await.unwrap();
        }
        let store = h.store.clone();
        let id = work.id().clone();
        let task = tokio::spawn(async move {
            store
                .catalog_execute(
                    BOB,
                    &operation(),
                    CatalogCommand::EditWork {
                        id,
                        expected_revision: 2,
                        details: work_details("Sau khi mất quyền"),
                    },
                )
                .await
        });
        blocked(&h.admin, 1).await;
        if expired {
            let deadline = Instant::now() + Duration::from_secs(1);
            while !h
                .admin
                .query_one(
                    "SELECT expires_at<=clock_timestamp() FROM sessions WHERE actor_id='bob'",
                    &[],
                )
                .await
                .unwrap()
                .get::<_, bool>(0)
            {
                assert!(Instant::now() < deadline);
                tokio::task::yield_now().await;
            }
        } else {
            tx.execute(
                "DELETE FROM catalog_members WHERE work_id=$1 AND actor_id='bob'",
                &[&work.id().as_str()],
            )
            .await
            .unwrap();
        }
        tx.commit().await.unwrap();
        let result = task.await.unwrap();
        if expired {
            assert!(matches!(
                result,
                Err(CatalogStoreError::Storage(StoreError::Unauthenticated))
            ));
            assert_eq!(h.counts().await, before);
        } else {
            assert!(matches!(
                result,
                Err(CatalogStoreError::Storage(StoreError::NotFound))
            ));
            let mut expected = before;
            expected["members"] = json!(0);
            assert_eq!(h.counts().await, expected);
        }
    }
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn settled_catalog_records_are_immutable_and_app_has_no_delete_identity_update_or_ddl() {
    let h = Harness::new().await;
    let work = h.create(ALICE, WorkKind::Novel).await;
    h.unit(ALICE, &work, "Chương được giữ lại").await;
    let app = connect(&h.app_config).await;
    for query in [
        "DELETE FROM catalog_works",
        "DELETE FROM catalog_units",
        "DELETE FROM catalog_work_revisions",
        "UPDATE catalog_work_revisions SET snapshot=snapshot",
        "UPDATE catalog_unit_revisions SET snapshot=snapshot",
        "UPDATE catalog_operations SET result_bytes=result_bytes",
        "UPDATE catalog_works SET owner_id='bob'",
        "CREATE TABLE forbidden_catalog_table(id integer)",
    ] {
        assert_eq!(
            app.batch_execute(query)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "42501",
            "{query}"
        );
    }
    for query in [
        "UPDATE catalog_work_revisions SET snapshot=snapshot",
        "DELETE FROM catalog_unit_revisions",
        // Delegated lineage (0026) references receipts, so only CASCADE reaches the trigger;
        // a plain TRUNCATE is refused earlier by that foreign key (asserted below).
        "TRUNCATE catalog_operations CASCADE",
        "UPDATE catalog_works SET owner_id='bob'",
    ] {
        assert_eq!(
            h.admin
                .batch_execute(query)
                .await
                .unwrap_err()
                .code()
                .unwrap()
                .code(),
            "55000",
            "{query}"
        );
    }
    assert_eq!(
        h.admin
            .batch_execute("TRUNCATE catalog_operations")
            .await
            .unwrap_err()
            .code()
            .unwrap()
            .code(),
        "0A000"
    );
    assert_eq!(h.counts().await["works"], 1);
    assert_eq!(h.counts().await["unit_revisions"], 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn migrations_0010_and_0011_upgrade_0009_and_preserve_existing_bytes_and_checksums() {
    verify_combined_migration_upgrade(false).await;
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn migration_0010_fills_gap_after_0011_through_0013_and_preserves_bytes_and_checksums() {
    verify_combined_migration_upgrade(true).await;
}

async fn verify_combined_migration_upgrade(catalog_already_applied: bool) {
    let (config, database) = database_config().await;
    let admin = connect(&config).await;
    admin
        .batch_execute(
            "CREATE TABLE cantos_migrations(version integer PRIMARY KEY,checksum bytea NOT NULL)",
        )
        .await
        .unwrap();
    for (index, sql) in BASELINE.iter().enumerate() {
        admin.batch_execute(sql).await.unwrap();
        let version = (index + 1) as i32;
        let checksum = Sha256::digest(sql.as_bytes()).to_vec();
        admin
            .execute(
                "INSERT INTO cantos_migrations VALUES($1,$2)",
                &[&version, &checksum],
            )
            .await
            .unwrap();
    }
    if catalog_already_applied {
        for (version, sql) in [
            (11, include_str!("../migrations/0011_catalog.sql")),
            (12, include_str!("../migrations/0012_content_identity.sql")),
            (13, include_str!("../migrations/0013_production_briefs.sql")),
        ] {
            admin.batch_execute(sql).await.unwrap();
            admin
                .execute(
                    "INSERT INTO cantos_migrations VALUES($1,$2)",
                    &[&version, &Sha256::digest(sql.as_bytes()).to_vec()],
                )
                .await
                .unwrap();
        }
    }
    // Seed old surfaces through their existing authority before audio migration 0010 exists.
    admin.batch_execute("INSERT INTO actors(id) VALUES('alice'); GRANT USAGE ON SCHEMA public TO cantos_app; GRANT SELECT ON actors,sessions,scripts,script_revisions,script_members,script_evidence,revision_evidence TO cantos_app; GRANT INSERT ON scripts,script_revisions,revision_evidence TO cantos_app; GRANT UPDATE(head_revision) ON scripts TO cantos_app;
         GRANT UPDATE(revoked) ON sessions TO cantos_app;").await.unwrap();
    admin.execute("INSERT INTO sessions(token_hash,actor_id,expires_at) VALUES($1,'alice',clock_timestamp()+interval '1 hour')", &[&token_hash(ALICE)]).await.unwrap();
    for (kind, id) in read_script(SCRIPT.as_bytes()).unwrap().evidence_refs() {
        admin.execute("INSERT INTO script_evidence(owner_id,kind,id,description) VALUES('alice',$1,$2,'Legacy synthetic evidence')", &[&kind,&id]).await.unwrap();
    }
    let h = Harness::from_connections(config.clone(), database, admin);
    let source = h.source("alice", "old-manuscript").await;
    let script = h.script(ALICE).await;
    let catalog_before = if catalog_already_applied {
        grant_and_seed(&h.admin).await;
        Some(h.create(ALICE, WorkKind::Story).await)
    } else {
        None
    };
    let historical_text = h.admin.query_one("SELECT json_build_object('migrations',(SELECT json_agg(json_build_array(version,encode(checksum,'hex')) ORDER BY version) FROM cantos_migrations),'source',(SELECT json_build_array(encode(coalesce(original_bytes,convert_to(original_text,'UTF8')),'hex'),sha256) FROM source_records WHERE id='old-manuscript'),'script',(SELECT json_build_array(encode(canonical_export,'hex'),content_digest,export_digest) FROM script_revisions WHERE script_id=$1 AND revision=1))::text", &[&script.id()]).await.unwrap().get::<_,String>(0);
    let historical: Value = serde_json::from_str(&historical_text).unwrap();
    migrate(&config).await.unwrap();
    migrate(&config).await.unwrap();
    grant_and_seed(&h.admin).await;
    let versions: Vec<i32> = h
        .admin
        .query(
            "SELECT version FROM cantos_migrations ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert_eq!(versions, (1..=29).collect::<Vec<_>>());
    let preserved_versions = if catalog_already_applied {
        "version<=13 AND version<>10"
    } else {
        "version<=9"
    };
    let retained_query = format!("SELECT json_build_object('migrations',(SELECT json_agg(json_build_array(version,encode(checksum,'hex')) ORDER BY version) FROM cantos_migrations WHERE {preserved_versions}),'source',(SELECT json_build_array(encode(coalesce(original_bytes,convert_to(original_text,'UTF8')),'hex'),sha256) FROM source_records WHERE id='old-manuscript'),'script',(SELECT json_build_array(encode(canonical_export,'hex'),content_digest,export_digest) FROM script_revisions WHERE script_id=$1 AND revision=1))::text");
    let retained_text = h
        .admin
        .query_one(&retained_query, &[&script.id()])
        .await
        .unwrap()
        .get::<_, String>(0);
    let retained: Value = serde_json::from_str(&retained_text).unwrap();
    assert_eq!(historical, retained);
    if let Some(before) = catalog_before {
        assert_eq!(
            h.store.catalog_work(ALICE, before.id()).await.unwrap(),
            before
        );
    }
    let work = h.create(ALICE, WorkKind::Story).await;
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
                    ..unit_details("Legacy links")
                },
            },
        )
        .await
        .unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn committed_catalog_fixture_survives_new_store_and_emits_server_restart_oracle() {
    let h = Harness::new().await;
    let id = work_id();
    let create_op = operation();
    let create = CatalogCommand::CreateWork {
        id: id.clone(),
        kind: WorkKind::Podcast,
        details: work_details("Bản ghi qua khởi động lại"),
    };
    let original = h
        .store
        .catalog_execute(ALICE, &create_op, create.clone())
        .await
        .unwrap();
    let unit_op = operation();
    let uid = unit_id();
    let unit = CatalogCommand::CreateUnit {
        work_id: id.clone(),
        expected_work_revision: 1,
        id: uid.clone(),
        kind: ContentUnitKind::Episode,
        details: unit_details("Tập một vẫn còn nguyên"),
    };
    let current = h
        .store
        .catalog_execute(ALICE, &unit_op, unit.clone())
        .await
        .unwrap();
    let reconnected = Store::new(h.app_config.clone()).unwrap();
    assert_eq!(
        reconnected.catalog_work(ALICE, &id).await.unwrap(),
        current.work
    );
    assert_eq!(
        reconnected
            .catalog_execute(ALICE, &create_op, create)
            .await
            .unwrap(),
        original
    );
    assert_eq!(
        reconnected
            .catalog_execute(ALICE, &unit_op, unit)
            .await
            .unwrap(),
        current
    );
    fs::create_dir_all(EVIDENCE).unwrap();
    fs::write(format!("{EVIDENCE}/restart.json"), serde_json::to_vec_pretty(&json!({
        "database": h.database, "work_id": id.as_str(), "unit_id": uid.as_str(),
        "create_operation": create_op, "unit_operation": unit_op,
        "original": original, "current": current, "counts": h.counts().await,
        "provenance": "Synthetic Vietnamese metadata; no manuscript, media, provider, publication or HTTP calls",
    })).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "runner enables this exact test only after its PostgreSQL restart"]
async fn catalog_recovery_after_server_restart() {
    let fixture: Value =
        serde_json::from_slice(&fs::read(format!("{EVIDENCE}/restart.json")).unwrap()).unwrap();
    let database = fixture["database"].as_str().unwrap();
    assert!(
        database.starts_with("cantos_test_")
            && database
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || byte == b'_')
    );
    let mut config = local_config(&env::var("CANTOS_TEST_CLUSTER_URL").unwrap()).unwrap();
    assert_eq!(config.get_user(), Some("cantos_test_admin"));
    config.dbname(database);
    migrate(&config).await.unwrap();
    let h = Harness::from_connections(config.clone(), database.into(), connect(&config).await);
    let id = WorkId::parse(fixture["work_id"].as_str().unwrap()).unwrap();
    let uid = ContentUnitId::parse(fixture["unit_id"].as_str().unwrap()).unwrap();
    assert_eq!(
        serde_json::to_value(h.store.catalog_work(ALICE, &id).await.unwrap()).unwrap(),
        fixture["current"]["work"]
    );
    let saved = h
        .store
        .catalog_execute(
            ALICE,
            fixture["create_operation"].as_str().unwrap(),
            CatalogCommand::CreateWork {
                id: id.clone(),
                kind: WorkKind::Podcast,
                details: work_details("Bản ghi qua khởi động lại"),
            },
        )
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(saved).unwrap(), fixture["original"]);
    let replay = h
        .store
        .catalog_execute(
            ALICE,
            fixture["unit_operation"].as_str().unwrap(),
            CatalogCommand::CreateUnit {
                work_id: id,
                expected_work_revision: 1,
                id: uid.clone(),
                kind: ContentUnitKind::Episode,
                details: unit_details("Tập một vẫn còn nguyên"),
            },
        )
        .await
        .unwrap();
    assert_eq!(serde_json::to_value(replay).unwrap(), fixture["current"]);
    assert_eq!(
        h.store
            .catalog_unit(ALICE, &uid)
            .await
            .unwrap()
            .details()
            .title,
        "Tập một vẫn còn nguyên"
    );
    assert_eq!(h.counts().await, fixture["counts"]);
    fs::write(format!("{EVIDENCE}/server-restart.json"), serde_json::to_vec_pretty(&json!({
        "database": database, "checks": "PostgreSQL restart, migration replay, exact catalog work/unit snapshots, original-operation replay and independent row counts",
        "before": fixture["counts"], "after": h.counts().await,
    })).unwrap()).unwrap();
}

#[tokio::test]
#[ignore = "disposable PostgreSQL cluster required"]
async fn authorized_read_baseline_is_bounded_and_records_select_plans_at_one_hundred_works() {
    let h = Harness::new().await;
    for index in 0..100 {
        h.store
            .catalog_execute(
                if index < 80 { ALICE } else { BOB },
                &operation(),
                CatalogCommand::CreateWork {
                    id: work_id(),
                    kind: WorkKind::Novel,
                    details: work_details(&format!("Ánh đèn — tác phẩm {index}")),
                },
            )
            .await
            .unwrap();
    }
    h.admin
        .batch_execute(
            "ANALYZE catalog_works; ANALYZE catalog_work_revisions; ANALYZE catalog_members;",
        )
        .await
        .unwrap();
    let app = connect(&h.app_config).await;
    let sql = "SELECT w.id,w.owner_id,w.kind,w.lifecycle,w.head_revision,r.revision,r.snapshot,r.snapshot_digest FROM catalog_works w LEFT JOIN catalog_work_revisions r ON r.work_id=w.id AND r.revision=w.head_revision WHERE (w.owner_id=$1 OR EXISTS(SELECT 1 FROM catalog_members m WHERE m.work_id=w.id AND m.actor_id=$1)) AND ($2::text IS NULL OR w.id>$2) AND ($3 OR w.lifecycle<>'archived') ORDER BY w.id LIMIT $4";
    let after: Option<&str> = None;
    let started = Instant::now();
    let page = h.store.catalog_works(ALICE, 20, false, None).await.unwrap();
    let request_micros = started.elapsed().as_micros();
    assert_eq!(page.items.len(), 20);
    assert!(page
        .items
        .iter()
        .all(|work| work.owner_id().as_str() == "alice"));
    let rows = app
        .query(sql, &[&"alice", &after, &false, &21_i64])
        .await
        .unwrap();
    assert_eq!(
        rows.iter()
            .take(20)
            .map(|row| row.get::<_, String>("id"))
            .collect::<Vec<_>>(),
        page.items
            .iter()
            .map(|work| work.id().as_str().to_owned())
            .collect::<Vec<_>>()
    );
    let plan: Vec<String> = app
        .query(
            &format!("EXPLAIN (ANALYZE, BUFFERS) {sql}"),
            &[&"alice", &after, &false, &21_i64],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect();
    assert!(plan.iter().any(|line| line.contains("Limit")));
    assert!(!plan.iter().any(|line| line.contains("Disk:")));
    let indexes: Vec<String> = h.admin.query("SELECT indexname FROM pg_indexes WHERE tablename IN ('catalog_works','catalog_work_revisions','catalog_members','catalog_units') ORDER BY indexname", &[]).await.unwrap().iter().map(|row| row.get(0)).collect();
    assert!(indexes.iter().any(|name| name == "catalog_works_owner"));
    assert!(indexes.iter().any(|name| name == "catalog_units_order"));
    let version: String = h
        .admin
        .query_one("SELECT version()", &[])
        .await
        .unwrap()
        .get(0);
    fs::create_dir_all(EVIDENCE).unwrap();
    fs::write(format!("{EVIDENCE}/read-baseline.json"), serde_json::to_vec_pretty(&json!({
        "server": version, "work_count": 100, "owner_count": 2, "alice_count": 80,
        "page_limit": 20, "application_role": "cantos_app", "first_page_request_micros": request_micros,
        "query": sql, "plan": plan, "indexes": indexes,
        "measurement": "One warm isolated request and safe SELECT plan; EXPLAIN overhead is not API latency; no throughput or percentile claim",
        "dbsp_decision": "defer: bounded indexed point/list queries have no repeated aggregate; production scale/frequency is unknown",
    })).unwrap()).unwrap();
}
