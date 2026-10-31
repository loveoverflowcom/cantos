//! Real file-backed Turso and production policy/generation/read entrypoints. Session facts here
//! are explicit development fixtures; real Identity/HTTP/bytes have a separate journey.
use super::{SeededWork, ServerFixture, WorkSeed};
use crate::{
    contexts::{
        catalog::{
            self,
            domain::{viewer_access::AccessError, WorkId},
            ViewerAccessError,
        },
        media::{ListenerError, ListenerService},
    },
    contract::studio::*,
    platform::{identity::domain::ActorId, store::StoreError, turso::Acknowledgement},
};
use cantos_store_turso::testing::FixedClock;
use serde_json::to_value;
use uuid::Uuid;
const OWNER: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
fn id() -> String {
    Uuid::new_v4().to_string()
}
fn change(version: u64, change: ViewerAccessChange) -> ViewerAccessRequest {
    ViewerAccessRequest {
        operation_id: id(),
        expected_version: version,
        change,
    }
}
fn actor(name: &str, expiry: Option<i64>) -> UnitViewerGrant {
    UnitViewerGrant {
        principal: ViewerPrincipal::Actor {
            actor_id: name.into(),
        },
        expires_at_us: expiry,
    }
}
fn restrict(unit: &str, grants: Vec<UnitViewerGrant>) -> ViewerAccessChange {
    ViewerAccessChange::SetUnitPolicy {
        unit_id: unit.into(),
        policy: UnitViewerPolicy::Restricted { grants },
        confirm_metadata_rights: false,
    }
}
fn inherit(unit: &str) -> ViewerAccessChange {
    ViewerAccessChange::SetUnitPolicy {
        unit_id: unit.into(),
        policy: UnitViewerPolicy::Inherit,
        confirm_metadata_rights: true,
    }
}
async fn setup(f: &ServerFixture) -> SeededWork {
    f.studio_caller("alice", OWNER).await;
    f.studio_caller("bob", OTHER).await;
    f.seed_actors(&["viewer", "beta"]).await;
    let work = f
        .seed_work(
            None,
            &WorkSeed::podcast(0x656, "Visible Work", &["One", "HIDDEN_SENTINEL", "Three"]),
        )
        .await;
    f.publish_work_metadata(&work).await;
    work
}
async fn shared_ids(
    f: &ServerFixture,
    work: &str,
    actor: &str,
) -> Option<(u32, Vec<String>, Vec<u32>)> {
    let conn = f.store().connection().await.unwrap();
    let tx = conn.read().await.unwrap();
    let w = catalog::shared_work(
        &tx,
        &WorkId::parse(work).unwrap(),
        &ActorId::new(actor).unwrap(),
    )
    .await
    .unwrap()?;
    let (rows, _) = catalog::shared_units(&tx, &w, 0, 50).await.unwrap();
    Some((
        w.row.unit_count,
        rows.iter().map(|r| r.unit.id().as_str().into()).collect(),
        rows.iter().map(|r| r.ordinal).collect(),
    ))
}
#[tokio::test]
async fn unit_hide_filters_count_before_pages_and_consecutive_wire_positions_and_invalidates_cursors(
) {
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let service = ListenerService::new(f.store().clone());
    let before = service.metadata_units(&w.work_id, 1, None).await.unwrap();
    let cursor = before.next_cursor.unwrap();
    let publication = to_value(
        f.store()
            .metadata_publication_state(OWNER, &w.work_id)
            .await
            .unwrap(),
    )
    .unwrap();
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                0,
                restrict(&w.units[1].unit_id, vec![actor("viewer", None)]),
            ),
        )
        .await
        .unwrap();
    assert!(matches!(
        service.metadata_units(&w.work_id, 1, Some(&cursor)).await,
        Err(ListenerError::InvalidCursor)
    ));
    let detail = service.metadata_work(&w.work_id).await.unwrap();
    assert_eq!(detail.unit_count, 2);
    let page = service.metadata_units(&w.work_id, 1, None).await.unwrap();
    assert_eq!(
        (
            page.items[0].unit_id.as_str(),
            page.items[0].ordinal,
            page.items[0].position
        ),
        (w.units[0].unit_id.as_str(), 1, 1)
    );
    let last = service
        .metadata_units(&w.work_id, 1, page.next_cursor.as_deref())
        .await
        .unwrap();
    assert_eq!(
        (
            last.items[0].unit_id.as_str(),
            last.items[0].ordinal,
            last.items[0].position
        ),
        (w.units[2].unit_id.as_str(), 2, 2)
    );
    assert!(last.next_cursor.is_none());
    assert!(!serde_json::to_string(&last)
        .unwrap()
        .contains("HIDDEN_SENTINEL"));
    let unknown = shared_ids(&f, &w.work_id, "beta").await.unwrap();
    assert_eq!(unknown.0, 2);
    assert_eq!(unknown.2, [1, 2]);
    let allowed = shared_ids(&f, &w.work_id, "viewer").await.unwrap();
    assert_eq!(allowed.0, 3);
    assert_eq!(allowed.2, [1, 2, 3]);
    assert_eq!(
        to_value(
            f.store()
                .metadata_publication_state(OWNER, &w.work_id)
                .await
                .unwrap()
        )
        .unwrap(),
        publication
    );
    // Search uses only Work-owned published text, never hidden Unit text.
    assert!(service
        .metadata_works("HIDDEN_SENTINEL", None, None, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
}
#[tokio::test]
async fn unit_replacement_receipt_replay_after_removal_lost_ack_and_reopen_never_regrants() {
    let mut f = ServerFixture::new().await;
    let w = setup(&f).await;
    let request = change(
        0,
        restrict(&w.units[1].unit_id, vec![actor("viewer", None)]),
    );
    let mut lost = f.store().clone();
    lost.acknowledgement = Acknowledgement::LostAfterCommit;
    assert!(matches!(
        lost.viewer_access_execute(OWNER, &w.work_id, request.clone())
            .await,
        Err(ViewerAccessError::Storage(StoreError::Unavailable))
    ));
    drop(lost);
    let receipt = f
        .store()
        .viewer_access_execute(OWNER, &w.work_id, request.clone())
        .await
        .unwrap();
    assert!(receipt.replayed);
    assert_eq!(receipt.version, 1);
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(1, restrict(&w.units[1].unit_id, vec![])),
        )
        .await
        .unwrap();
    f.restart().await;
    assert!(
        f.store()
            .viewer_access_execute(OWNER, &w.work_id, request.clone())
            .await
            .unwrap()
            .replayed
    );
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 2);
    let mut changed = request;
    changed.expected_version = 2;
    assert!(matches!(
        f.store()
            .viewer_access_execute(OWNER, &w.work_id, changed)
            .await,
        Err(ViewerAccessError::Access(AccessError::Reused))
    ));
    let state = f
        .store()
        .unit_viewer_access_state(OWNER, &w.work_id, &w.units[1].unit_id)
        .await
        .unwrap();
    assert_eq!(state.version, 2);
    assert!(matches!(state.policy,UnitViewerPolicy::Restricted {grants} if grants.is_empty()));
}
#[tokio::test]
async fn unit_policy_uses_work_cas_and_rejects_foreign_units_owners_groups_duplicates_and_expiry() {
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let request = change(
        0,
        restrict(&w.units[1].unit_id, vec![actor("viewer", None)]),
    );
    assert!(matches!(
        f.store()
            .viewer_access_execute(OTHER, &w.work_id, request.clone())
            .await,
        Err(ViewerAccessError::Storage(StoreError::NotFound))
    ));
    let (a, b) = tokio::join!(
        f.store().viewer_access_execute(OWNER, &w.work_id, request),
        f.store()
            .viewer_access_execute(OWNER, &w.work_id, change(0, inherit(&w.units[0].unit_id)))
    );
    assert_eq!(usize::from(a.is_ok()) + usize::from(b.is_ok()), 1);
    assert!(matches!(
        a.err().or_else(|| b.err()),
        Some(ViewerAccessError::Access(AccessError::Stale { current: 1 }))
    ));
    let foreign = f
        .seed_work(None, &WorkSeed::podcast(0x6562, "Other", &["Foreign"]))
        .await;
    assert!(matches!(
        f.store()
            .viewer_access_execute(
                OWNER,
                &w.work_id,
                change(1, inherit(&foreign.units[0].unit_id))
            )
            .await,
        Err(ViewerAccessError::Access(AccessError::NotFound))
    ));
    for (grants, expected) in [
        (
            vec![actor("viewer", None), actor("viewer", None)],
            AccessError::Duplicate,
        ),
        (vec![actor("viewer", Some(0))], AccessError::Invalid),
        (
            vec![UnitViewerGrant {
                principal: ViewerPrincipal::Group { group_id: id() },
                expires_at_us: None,
            }],
            AccessError::NotFound,
        ),
    ] {
        assert!(
            matches!(f.store().viewer_access_execute(OWNER,&w.work_id,change(1,restrict(&w.units[1].unit_id,grants))).await,Err(ViewerAccessError::Access(error)) if error==expected)
        );
    }
    assert_eq!(
        f.store()
            .viewer_access_state(OWNER, &w.work_id)
            .await
            .unwrap()
            .version,
        1
    );
    assert!(matches!(
        f.store()
            .unit_viewer_access_state(OTHER, &w.work_id, &w.units[1].unit_id)
            .await,
        Err(ViewerAccessError::Storage(StoreError::NotFound))
    ));
}
#[tokio::test]
async fn unit_current_expiry_fences_shared_count_and_scope_without_a_write() {
    let clock = FixedClock::new(1_700_000_000_000_000);
    let f = ServerFixture::with_clock(clock.clone()).await;
    let w = setup(&f).await;
    let until = f.clock_reading().unwrap() + 1000;
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                0,
                restrict(&w.units[1].unit_id, vec![actor("viewer", Some(until))]),
            ),
        )
        .await
        .unwrap();
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 3);
    let conn = f.store().connection().await.unwrap();
    let tx = conn.read().await.unwrap();
    let before = catalog::shared_scope(&tx).await.unwrap();
    tx.commit().await.unwrap();
    drop(conn);
    clock.set(until);
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 2);
    let conn = f.store().connection().await.unwrap();
    let tx = conn.read().await.unwrap();
    assert_ne!(catalog::shared_scope(&tx).await.unwrap(), before);
}
#[tokio::test]
async fn unit_grant_never_bypasses_work_and_inherit_never_republishes_withdrawn_unit() {
    use crate::contexts::media::MetadataPublicationService;
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                0,
                restrict(&w.units[1].unit_id, vec![actor("viewer", None)]),
            ),
        )
        .await
        .unwrap();
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                1,
                ViewerAccessChange::SetAudience {
                    audience: ViewerAudience::Restricted,
                    confirm_metadata_rights: false,
                },
            ),
        )
        .await
        .unwrap();
    assert!(shared_ids(&f, &w.work_id, "viewer").await.is_none());
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                2,
                ViewerAccessChange::GrantActor {
                    grant_id: id(),
                    actor_id: "viewer".into(),
                    expires_at_us: None,
                },
            ),
        )
        .await
        .unwrap();
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 3);
    MetadataPublicationService::new(f.store().clone())
        .settle(
            OWNER,
            &w.work_id,
            MetadataPublicationRequest::WithdrawUnit {
                operation_id: id(),
                expected_version: 1,
                unit_id: w.units[1].unit_id.clone(),
            },
        )
        .await
        .unwrap();
    f.store()
        .viewer_access_execute(OWNER, &w.work_id, change(3, inherit(&w.units[1].unit_id)))
        .await
        .unwrap();
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 2);
    assert_eq!(
        f.store()
            .metadata_publication_state(OWNER, &w.work_id)
            .await
            .unwrap()
            .version,
        2
    );
}

#[tokio::test]
async fn unit_alpha_work_beta_chapter_intersection_tracks_membership_removal_and_group_owner() {
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let alpha = id();
    let beta = id();
    let foreign = id();
    for (token, group) in [(OWNER, &alpha), (OWNER, &beta), (OTHER, &foreign)] {
        f.store()
            .viewer_group_execute(
                token,
                group,
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
    }
    for group in [&alpha, &beta] {
        f.store()
            .viewer_group_execute(
                OWNER,
                group,
                ViewerGroupRequest {
                    operation_id: id(),
                    expected_version: 1,
                    change: ViewerGroupChange::AddMember {
                        actor_id: "viewer".into(),
                        expires_at_us: None,
                    },
                },
            )
            .await
            .unwrap();
    }
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                0,
                ViewerAccessChange::SetAudience {
                    audience: ViewerAudience::Restricted,
                    confirm_metadata_rights: false,
                },
            ),
        )
        .await
        .unwrap();
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                1,
                ViewerAccessChange::GrantGroup {
                    grant_id: id(),
                    group_id: alpha.clone(),
                    expires_at_us: None,
                },
            ),
        )
        .await
        .unwrap();
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                2,
                restrict(
                    &w.units[1].unit_id,
                    vec![UnitViewerGrant {
                        principal: ViewerPrincipal::Group {
                            group_id: beta.clone(),
                        },
                        expires_at_us: None,
                    }],
                ),
            ),
        )
        .await
        .unwrap();
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 3);
    assert!(shared_ids(&f, &w.work_id, "beta").await.is_none());
    f.store()
        .viewer_group_execute(
            OWNER,
            &beta,
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
    assert_eq!(shared_ids(&f, &w.work_id, "viewer").await.unwrap().0, 2);
    assert!(matches!(
        f.store()
            .viewer_access_execute(
                OWNER,
                &w.work_id,
                change(
                    3,
                    restrict(
                        &w.units[1].unit_id,
                        vec![UnitViewerGrant {
                            principal: ViewerPrincipal::Group { group_id: foreign },
                            expires_at_us: None
                        }]
                    )
                )
            )
            .await,
        Err(ViewerAccessError::Access(AccessError::NotFound))
    ));
    f.store()
        .viewer_group_execute(
            OWNER,
            &alpha,
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
    assert!(shared_ids(&f, &w.work_id, "viewer").await.is_none());
}

#[tokio::test]
async fn unit_matrix_is_bounded_and_stale_policy_invalidates_its_walk_through_the_real_studio_router(
) {
    use axum::{
        body::Body,
        http::{header, Request, StatusCode},
    };
    use http_body_util::BodyExt;
    use tower::ServiceExt;
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: f.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let request = |path: String, token: Option<&str>| {
        let mut r = Request::get(path).header(header::ORIGIN, "https://studio.example.test");
        if let Some(token) = token {
            r = r.header(header::COOKIE, format!("cantos_session={token}"));
        }
        r.body(Body::empty()).unwrap()
    };
    let path = format!("/api/v1/studio/works/{}/chapter-matrix?limit=1", w.work_id);
    let response = app
        .clone()
        .oneshot(request(path.clone(), Some(OWNER)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    let page: ChapterMatrixPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page.items.len(), 1);
    assert_eq!(page.items[0].draft_revision, 1);
    assert_eq!(page.items[0].published_revision, Some(1));
    assert_eq!(page.policy_version, 0);
    let cursor = page.next_cursor.unwrap();
    let response = app
        .clone()
        .oneshot(request(path.clone(), Some(OTHER)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let response = app.clone().oneshot(request(path, None)).await.unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(0, restrict(&w.units[1].unit_id, vec![])),
        )
        .await
        .unwrap();
    let stale = format!(
        "/api/v1/studio/works/{}/chapter-matrix?limit=1&cursor={cursor}",
        w.work_id
    );
    let response = app
        .clone()
        .oneshot(request(stale, Some(OWNER)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    let unit = format!(
        "/api/v1/studio/works/{}/units/{}/viewer-access",
        w.work_id, w.units[1].unit_id
    );
    let response = app.oneshot(request(unit, Some(OWNER))).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let unit: UnitViewerAccessState =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(unit.version, 1);
    assert!(matches!(unit.policy,UnitViewerPolicy::Restricted {grants} if grants.is_empty()));
}

#[tokio::test]
async fn unit_schema_rejects_cross_parent_binding_and_invalid_mode_on_the_pinned_engine() {
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let foreign = f
        .seed_work(None, &WorkSeed::podcast(0x6563, "Foreign", &["Other"]))
        .await;
    assert!(f.try_exec("INSERT INTO catalog_unit_viewer_policies(unit_id,work_id,owner_id,mode) VALUES(?1,?2,'alice','restricted')",&[&foreign.units[0].unit_id,&w.work_id]).await.unwrap_err().is_integrity_violation());
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(0, restrict(&w.units[1].unit_id, vec![])),
        )
        .await
        .unwrap();
    assert!(f
        .try_exec(
            "UPDATE catalog_unit_viewer_policies SET work_id=?1 WHERE unit_id=?2",
            &[&foreign.work_id, &w.units[1].unit_id]
        )
        .await
        .unwrap_err()
        .is_integrity_violation());
    assert!(f
        .try_exec(
            "UPDATE catalog_unit_viewer_policies SET mode='public' WHERE unit_id=?1",
            &[&w.units[1].unit_id]
        )
        .await
        .unwrap_err()
        .is_integrity_violation());
    assert_eq!(
        f.store()
            .viewer_access_state(OWNER, &w.work_id)
            .await
            .unwrap()
            .version,
        1
    );
}

#[tokio::test]
async fn unit_hide_removes_library_rows_before_paging_and_never_resumes_an_older_chapter() {
    use super::library::{audio, bookmark, history_start, principal, progress, saved};
    use crate::contexts::listening::{LibraryError, LibraryService};
    use crate::contract::theatre::library::ProgressKind;
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let library = LibraryService::new(f.store().clone());
    let who = principal("viewer");
    let visible = audio(&w, &w.units[0]);
    let hidden = audio(&w, &w.units[1]);
    library
        .put_progress(
            &who,
            &visible.publication_id,
            progress(&visible, 1, 90, 0, 10, ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    library
        .put_progress(
            &who,
            &hidden.publication_id,
            progress(&hidden, 2, 90, 0, 20, ProgressKind::Checkpoint),
        )
        .await
        .unwrap();
    library
        .playback_started(&who, history_start(&hidden, 3, 90, 99, 0))
        .await
        .unwrap();
    library
        .set_saved(&who, &w.work_id, saved(4, 90, 0, true))
        .await
        .unwrap();
    for (n, a) in [(1, &visible), (2, &hidden)] {
        let b = bookmark(n, a, 4 + n, 90, 10);
        library
            .create_bookmark(&who, &b.bookmark_id, b.clone())
            .await
            .unwrap();
    }
    assert_eq!(
        library
            .presented_continue_listening(&who, 20, None)
            .await
            .unwrap()
            .items
            .len(),
        1
    );
    let bookmarks = library.bookmarks(&who, 1, None).await.unwrap();
    assert_eq!(bookmarks.items.len(), 1);
    let cursor = bookmarks.next_cursor.unwrap();
    f.store()
        .viewer_access_execute(
            OWNER,
            &w.work_id,
            change(
                0,
                restrict(&w.units[1].unit_id, vec![actor("viewer", None)]),
            ),
        )
        .await
        .unwrap();
    // Private durable Library restoration stays unavailable even for an admitted shared viewer.
    assert!(library
        .presented_continue_listening(&who, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(library
        .presented_history(&who, 20, None)
        .await
        .unwrap()
        .items
        .is_empty());
    assert!(matches!(
        library.bookmarks(&who, 1, Some(&cursor)).await,
        Err(LibraryError::InvalidCursor)
    ));
    let bookmarks = library.bookmarks(&who, 1, None).await.unwrap();
    assert_eq!(bookmarks.items.len(), 1);
    assert_eq!(bookmarks.items[0].bookmark.audio.unit_id, visible.unit_id);
    assert!(bookmarks.next_cursor.is_none());
    let saved = library.presented_saved_works(&who, 20, None).await.unwrap();
    assert_eq!(saved.items.len(), 1);
    assert!(!serde_json::to_string(&saved)
        .unwrap()
        .contains("HIDDEN_SENTINEL"));
}

#[tokio::test]
async fn archive_and_owner_disable_invalidate_public_walks_without_policy_writes() {
    let f = ServerFixture::new().await;
    let w = setup(&f).await;
    let service = ListenerService::new(f.store().clone());
    let first = service.metadata_units(&w.work_id, 1, None).await.unwrap();
    f.archive_unit(&w.units[1]).await;
    assert!(matches!(
        service
            .metadata_units(&w.work_id, 1, first.next_cursor.as_deref())
            .await,
        Err(ListenerError::InvalidCursor)
    ));
    assert_eq!(
        service.metadata_work(&w.work_id).await.unwrap().unit_count,
        2
    );
    let next = service.metadata_units(&w.work_id, 1, None).await.unwrap();
    f.set_actor_active(&w.owner, false).await;
    assert!(matches!(
        service
            .metadata_units(&w.work_id, 1, next.next_cursor.as_deref())
            .await,
        Err(ListenerError::InvalidCursor)
    ));
    assert!(matches!(
        service.metadata_work(&w.work_id).await,
        Err(ListenerError::NotFound)
    ));
    f.set_actor_active(&w.owner, true).await;
    // Disable/enable must not revive the old cursor either.
    assert!(matches!(
        service
            .metadata_units(&w.work_id, 1, next.next_cursor.as_deref())
            .await,
        Err(ListenerError::InvalidCursor)
    ));
    let current = service.metadata_units(&w.work_id, 1, None).await.unwrap();
    f.archive_work(&w).await;
    assert!(matches!(
        service
            .metadata_units(&w.work_id, 1, current.next_cursor.as_deref())
            .await,
        Err(ListenerError::InvalidCursor)
    ));
}
