//! S07 real Identity process + independent Turso files + production HTTP/byte paths.
//! Owner control records/catalog/audio are synthetic fixtures; viewer credentials are earned.
use super::*;
use cantos_server::{
    adapters::inbound::http::router_with_restricted_media,
    contexts::media::{
        domain::object_storage::NormalizedByteRange,
        storage::port::{StorageFuture, StorageRead},
        PublicationObjectRef, PublicationObjects,
    },
    contract::studio::{
        ViewerAccessChange as Change, ViewerAccessRequest, ViewerAudience, ViewerGroupChange,
        ViewerGroupRequest,
    },
    platform::store::Store,
};
use std::sync::atomic::{AtomicBool, Ordering};
use uuid::Uuid;
const CONTROL: &str = "d653d653d653d653d653d653d653d653d653d653d653d653d653d653d653d653";

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn unit_policy_hide_and_every_fetch_never_bypass_parent_or_restore_old_public_access() {
    use cantos_server::contract::studio::{UnitViewerGrant, UnitViewerPolicy, ViewerPrincipal};
    let _ = supplied_identity();
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let _identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let storage =
        Arc::new(FilesystemPublicationStorage::new(&fixture.dir().join("publication")).unwrap());
    let work = fixture
        .seed_published_work(
            Some(&storage),
            &WorkSeed::podcast(
                0x6564,
                "Public Work",
                &["One", "HIDDEN_UNIT_SENTINEL", "Three"],
            ),
        )
        .await;
    fixture.studio_caller("alice", CONTROL).await;
    let public = compose(&fixture, &env);
    let a = enroll(&public, &sink, OWNER, 1).await;
    let b = enroll(&public, &sink, SECOND, 2).await;
    let hidden = &work.units[1];
    let release = hidden.chain.release_id.as_ref().unwrap();
    let old = send(
        &public.router,
        "POST",
        &theatre(&format!("/publications/{release}/playback-authorizations")),
        &[],
        Some(b"{}".to_vec()),
    )
    .await;
    assert_eq!(old.status, StatusCode::OK);
    let old_path = old.json()["url"]
        .as_str()
        .unwrap()
        .strip_prefix("https://api.example")
        .unwrap()
        .to_owned();
    let first = send(
        &public.router,
        "GET",
        &theatre(&format!("/works/{}/units?limit=1", work.work_id)),
        &[],
        None,
    )
    .await;
    assert_eq!(first.status, StatusCode::OK);
    let cursor = first.json()["next_cursor"].as_str().unwrap().to_owned();
    let replacement = |grants| Change::SetUnitPolicy {
        unit_id: hidden.unit_id.clone(),
        policy: UnitViewerPolicy::Restricted { grants },
        confirm_metadata_rights: false,
    };
    change(
        &fixture,
        &work.work_id,
        replacement(vec![UnitViewerGrant {
            principal: ViewerPrincipal::Actor {
                actor_id: a.actor.clone(),
            },
            expires_at_us: None,
        }]),
    )
    .await;
    let host = restricted_host(&fixture, &env, storage);
    for method in ["GET", "HEAD"] {
        assert_eq!(
            send(&public.router, method, &old_path, &[], None)
                .await
                .status,
            StatusCode::NOT_FOUND
        );
    }
    let page = send(
        &public.router,
        "GET",
        &theatre(&format!("/works/{}/units?limit=50", work.work_id)),
        &[],
        None,
    )
    .await;
    assert_eq!(page.status, StatusCode::OK);
    let page = page.json();
    let rows = page["items"].as_array().unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(
        (rows[0]["ordinal"].clone(), rows[0]["position"].clone()),
        (json!(1), json!(1))
    );
    assert_eq!(
        (rows[1]["ordinal"].clone(), rows[1]["position"].clone()),
        (json!(2), json!(2))
    );
    assert!(!page.to_string().contains("HIDDEN_UNIT_SENTINEL"));
    let stale = send(
        &public.router,
        "GET",
        &theatre(&format!(
            "/works/{}/units?limit=1&cursor={cursor}",
            work.work_id
        )),
        &[],
        None,
    )
    .await;
    assert_eq!(stale.status, StatusCode::BAD_REQUEST);
    let shared = send(
        &host.router,
        "GET",
        &theatre("/shared/works"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(shared.status, StatusCode::OK);
    assert_eq!(shared.json()["items"][0]["work_id"], work.work_id);
    assert_eq!(shared.json()["items"][0]["unit_count"], 3);
    for (token, count) in [(&a.access, 3), (&b.access, 2)] {
        let response = send(
            &host.router,
            "GET",
            &theatre(&format!("/shared/works/{}/units?limit=50", work.work_id)),
            &[bearer(token)],
            None,
        )
        .await;
        assert_eq!(response.status, StatusCode::OK);
        assert_eq!(response.json()["items"].as_array().unwrap().len(), count);
        if count == 2 {
            assert!(!response.json().to_string().contains("HIDDEN_UNIT_SENTINEL"));
        }
    }
    let path = private_grant(&host, &a.access, release).await;
    let seek = send(
        &host.router,
        "GET",
        &path,
        &[bearer(&a.access), ("range", "bytes=0-9".into())],
        None,
    )
    .await;
    assert_eq!(seek.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(seek.body, hidden.chain.bytes[..10]);
    refused_fetch(&host, &path, &b.access).await;
    change(&fixture, &work.work_id, replacement(vec![])).await;
    refused_fetch(&host, &path, &a.access).await;
    // A child grant does not broaden a now-restricted Work, even for the same authenticated actor.
    change(
        &fixture,
        &work.work_id,
        replacement(vec![UnitViewerGrant {
            principal: ViewerPrincipal::Actor {
                actor_id: a.actor.clone(),
            },
            expires_at_us: None,
        }]),
    )
    .await;
    change(
        &fixture,
        &work.work_id,
        Change::SetAudience {
            audience: ViewerAudience::Restricted,
            confirm_metadata_rights: false,
        },
    )
    .await;
    let denied = send(
        &host.router,
        "GET",
        &theatre(&format!("/shared/works/{}", work.work_id)),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(denied.status, StatusCode::NOT_FOUND);
    actor_grant(&fixture, &work.work_id, &a.actor).await;
    let path = private_grant(&host, &a.access, release).await;
    use cantos_server::{
        contexts::media::MetadataPublicationService, contract::studio::MetadataPublicationRequest,
    };
    MetadataPublicationService::new(fixture.store().clone())
        .settle(
            CONTROL,
            &work.work_id,
            MetadataPublicationRequest::WithdrawUnit {
                operation_id: Uuid::new_v4().to_string(),
                expected_version: 1,
                unit_id: hidden.unit_id.clone(),
            },
        )
        .await
        .unwrap();
    refused_fetch(&host, &path, &a.access).await;
    change(
        &fixture,
        &work.work_id,
        Change::SetUnitPolicy {
            unit_id: hidden.unit_id.clone(),
            policy: UnitViewerPolicy::Inherit,
            confirm_metadata_rights: true,
        },
    )
    .await;
    let response = send(
        &host.router,
        "GET",
        &theatre(&format!("/shared/works/{}/units?limit=50", work.work_id)),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(response.status, StatusCode::OK);
    assert_eq!(response.json()["items"].as_array().unwrap().len(), 2);
    assert!(!response.json().to_string().contains("HIDDEN_UNIT_SENTINEL"));
}

async fn change(f: &ServerFixture, work: &str, change: Change) {
    let state = f.store().viewer_access_state(CONTROL, work).await.unwrap();
    f.store()
        .viewer_access_execute(
            CONTROL,
            work,
            ViewerAccessRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_version: state.version,
                change,
            },
        )
        .await
        .unwrap();
}
async fn actor_grant(f: &ServerFixture, work: &str, actor: &str) -> String {
    let grant = Uuid::new_v4().to_string();
    change(
        f,
        work,
        Change::GrantActor {
            grant_id: grant.clone(),
            actor_id: actor.into(),
            expires_at_us: None,
        },
    )
    .await;
    grant
}
fn restricted_host(
    f: &ServerFixture,
    env: &IdentityEnv,
    objects: Arc<dyn PublicationObjects>,
) -> Host {
    let store = f
        .store()
        .clone()
        .without_development_sessions()
        .with_identity_auth(identity_auth(env));
    let origin = OriginPlayback::new(
        PublicMediaOrigin::parse("https://api.example").unwrap(),
        GrantKey::new(&[7; 32]).unwrap(),
        objects,
    );
    let publication = PublicationService::new(store.clone());
    let origins = SurfaceOrigins::new("https://studio.example", THEATRE, None).unwrap();
    Host {
        router: router_with_restricted_media(
            AppState {
                store,
                web_origin: THEATRE.into(),
            },
            &origins,
            publication,
            origin,
        )
        .unwrap(),
    }
}
async fn private_grant(host: &Host, token: &str, release: &str) -> String {
    let response = send(
        &host.router,
        "POST",
        &theatre(&format!(
            "/shared/publications/{release}/playback-authorizations"
        )),
        &[bearer(token)],
        Some(b"{}".to_vec()),
    )
    .await;
    assert_eq!(
        response.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&response.body)
    );
    conforms(DISCOVERY, "PlaybackAuthorization", &response.json());
    assert_eq!(
        response.header("cache-control"),
        Some("private, no-store, no-transform")
    );
    response.json()["url"]
        .as_str()
        .unwrap()
        .strip_prefix("https://api.example")
        .unwrap()
        .to_owned()
}
fn private_headers(r: &Reply) {
    assert_eq!(
        r.header("cache-control"),
        Some("private, no-store, no-transform")
    );
    assert_eq!(r.header("referrer-policy"), Some("no-referrer"));
}
async fn refused_fetch(host: &Host, path: &str, token: &str) {
    for (method, extra) in [
        ("GET", vec![]),
        ("HEAD", vec![]),
        ("GET", vec![("range", "bytes=0-9".into())]),
        ("GET", vec![("if-none-match", "*".into())]),
        ("GET", vec![("range", "bytes=999999-".into())]),
    ] {
        let mut headers = vec![bearer(token)];
        headers.extend(extra);
        let r = send(&host.router, method, path, &headers, None).await;
        assert_eq!(
            r.status,
            StatusCode::NOT_FOUND,
            "{method} private media (URL redacted)"
        );
        assert_ne!(r.header("content-type"), Some("audio/ogg"));
        private_headers(&r);
        if method == "HEAD" {
            assert!(r.body.is_empty());
        } else {
            assert_eq!(r.json(), json!({"code":"not_found","retryable":false}));
        }
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_shared_metadata_and_each_fetch_bind_current_account_session_and_policy() {
    let _ = supplied_identity();
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let storage =
        Arc::new(FilesystemPublicationStorage::new(&fixture.dir().join("publication")).unwrap());
    let work = fixture
        .seed_published_work(
            Some(&storage),
            &WorkSeed::podcast(0x653, "Riêng tư sentinel", &["Chương kín", "Chương hai"]),
        )
        .await;
    let second = fixture
        .seed_published_work(
            Some(&storage),
            &WorkSeed::podcast(0x654, "Riêng thứ hai", &[]),
        )
        .await;
    fixture.studio_caller("alice", CONTROL).await;
    let public = compose(&fixture, &env);
    let a = enroll(&public, &sink, OWNER, 1).await;
    let b = enroll(&public, &sink, SECOND, 2).await;
    let release = work.units[0].chain.release_id.as_deref().unwrap();
    let old = send(
        &public.router,
        "POST",
        &theatre(&format!("/publications/{release}/playback-authorizations")),
        &[],
        Some(b"{}".to_vec()),
    )
    .await;
    assert_eq!(old.status, StatusCode::OK);
    let old_public_path = old.json()["url"]
        .as_str()
        .unwrap()
        .strip_prefix("https://api.example")
        .unwrap()
        .to_owned();
    let audio_identity = audio(&work, &work.units[0]);
    for (method, path, payload) in [
        (
            "PUT",
            theatre(&format!("/progress/{release}")),
            serde_json::to_value(progress(
                &audio_identity,
                1,
                653,
                0,
                100,
                dto::ProgressKind::Checkpoint,
            ))
            .unwrap(),
        ),
        (
            "PUT",
            theatre(&format!("/library/saved/{}", work.work_id)),
            serde_json::to_value(saved(2, 653, 0, true)).unwrap(),
        ),
        (
            "POST",
            theatre("/library/history/playback-starts"),
            serde_json::to_value(history_start(&audio_identity, 3, 653, 653, 0)).unwrap(),
        ),
    ] {
        let r = send(
            &public.router,
            method,
            &path,
            &[bearer(&a.access)],
            json_body(&payload),
        )
        .await;
        assert_eq!(
            r.status,
            StatusCode::OK,
            "{}",
            String::from_utf8_lossy(&r.body)
        );
        assert_eq!(r.json()["outcome"], "accepted");
    }
    for w in [&work.work_id, &second.work_id] {
        change(
            &fixture,
            w,
            Change::SetAudience {
                audience: ViewerAudience::Restricted,
                confirm_metadata_rights: false,
            },
        )
        .await;
    }
    let direct = actor_grant(&fixture, &work.work_id, &a.actor).await;
    actor_grant(&fixture, &work.work_id, &b.actor).await;
    actor_grant(&fixture, &second.work_id, &a.actor).await;
    let group = Uuid::new_v4().to_string();
    for (version, change) in [
        (
            0,
            ViewerGroupChange::Create {
                name: "synthetic shared readers".into(),
            },
        ),
        (
            1,
            ViewerGroupChange::AddMember {
                actor_id: a.actor.clone(),
                expires_at_us: None,
            },
        ),
    ] {
        fixture
            .store()
            .viewer_group_execute(
                CONTROL,
                &group,
                ViewerGroupRequest {
                    operation_id: Uuid::new_v4().to_string(),
                    expected_version: version,
                    change,
                },
            )
            .await
            .unwrap();
    }
    change(
        &fixture,
        &work.work_id,
        Change::GrantGroup {
            grant_id: Uuid::new_v4().to_string(),
            group_id: group.clone(),
            expires_at_us: None,
        },
    )
    .await;
    let host = restricted_host(&fixture, &env, storage.clone());
    // Default composition cannot activate, even with a valid viewer and public-media root.
    let closed = send(
        &public.router,
        "GET",
        &theatre("/shared/works"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(closed.status, StatusCode::NOT_FOUND);
    private_headers(&closed);
    let page = send(
        &host.router,
        "GET",
        &theatre("/shared/works"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(
        page.status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&page.body)
    );
    conforms(METADATA, "works", &page.json());
    assert_eq!(page.json()["items"].as_array().unwrap().len(), 2); // actor + group dedup
    private_headers(&page);
    let search = send(
        &host.router,
        "GET",
        &theatre("/shared/search?q=RI%C3%8ANG%20T%C6%AF&limit=1"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(search.json()["items"].as_array().unwrap().len(), 1);
    // Lowercase normalization may expand a valid raw query (U+0130 -> i + dot).
    let expanded = url::form_urlencoded::Serializer::new(String::new())
        .append_pair("q", &"İ".repeat(171))
        .finish();
    let no_match = send(
        &host.router,
        "GET",
        &theatre(&format!("/shared/search?{expanded}")),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(no_match.status, StatusCode::OK);
    assert!(no_match.json()["items"].as_array().unwrap().is_empty());
    let units_path = theatre(&format!("/shared/works/{}/units?limit=1", work.work_id));
    let first = send(&host.router, "GET", &units_path, &[bearer(&a.access)], None).await;
    assert_eq!(first.status, StatusCode::OK);
    conforms(METADATA, "units", &first.json());
    assert_eq!(first.json()["items"][0]["ordinal"], 1);
    let cursor = first.json()["next_cursor"].as_str().unwrap().to_owned();
    let continuation = format!("{units_path}&cursor={cursor}");
    let switched = send(
        &host.router,
        "GET",
        &continuation,
        &[bearer(&b.access)],
        None,
    )
    .await;
    assert_eq!(switched.status, StatusCode::BAD_REQUEST);
    assert_eq!(switched.json()["code"], "invalid_cursor");
    let next = send(
        &host.router,
        "GET",
        &continuation,
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(next.json()["items"][0]["ordinal"], 2);
    // Public IDs/search/old publication/URL and old presentation versions never fall back.
    for suffix in [
        format!("/works/{}", work.work_id),
        format!("/works/{}/units", work.work_id),
        format!("/publications/{release}"),
        format!("/works/{}/cover/{}", work.work_id, "a".repeat(64)),
    ] {
        let denied = send(&host.router, "GET", &theatre(&suffix), &[], None).await;
        assert_eq!(denied.status, StatusCode::NOT_FOUND);
    }
    let public_list = send(
        &host.router,
        "GET",
        &theatre("/search?q=sentinel"),
        &[],
        None,
    )
    .await;
    assert!(public_list.json()["items"].as_array().unwrap().is_empty());
    assert_eq!(
        send(&host.router, "GET", &old_public_path, &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    for section in ["continue", "saved", "history", "bookmarks"] {
        let r = send(
            &host.router,
            "GET",
            &theatre(&format!("/library/{section}")),
            &[bearer(&a.access)],
            None,
        )
        .await;
        assert_eq!(r.status, StatusCode::OK);
        let text = String::from_utf8_lossy(&r.body);
        assert!(!text.contains("sentinel"));
        assert!(!text.contains("Chương kín"));
    }
    let manifest = send(
        &host.router,
        "GET",
        &theatre(&format!("/shared/publications/{release}")),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(manifest.status, StatusCode::OK);
    conforms(DISCOVERY, "PublicationManifest", &manifest.json());
    private_headers(&manifest);
    let public_as_private = old_public_path.replace("/media/", "/shared/media/");
    refused_fetch(&host, &public_as_private, &a.access).await;
    let path = private_grant(&host, &a.access, release).await;
    refused_fetch(&host, &path, &b.access).await; // B has the same Work grant: only session binding differs.
    let anon = send(&host.router, "GET", &path, &[], None).await;
    assert_eq!(anon.status, StatusCode::NOT_FOUND);
    let public_replay = path.replace("/shared/media/", "/media/");
    assert_eq!(
        send(&host.router, "GET", &public_replay, &[], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let head = send(&host.router, "HEAD", &path, &[bearer(&a.access)], None).await;
    assert_eq!(head.status, StatusCode::OK);
    assert!(head.body.is_empty());
    assert_eq!(head.header("content-length"), Some("4096"));
    private_headers(&head);
    let etag = head.header("etag").unwrap().to_owned();
    let conditional = send(
        &host.router,
        "GET",
        &path,
        &[bearer(&a.access), ("if-none-match", etag)],
        None,
    )
    .await;
    assert_eq!(conditional.status, StatusCode::NOT_MODIFIED);
    assert!(conditional.body.is_empty());
    private_headers(&conditional);
    let seek = send(
        &host.router,
        "GET",
        &path,
        &[bearer(&a.access), ("range", "bytes=100-199".into())],
        None,
    )
    .await;
    assert_eq!(seek.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(seek.body, work.units[0].chain.bytes[100..200]);
    private_headers(&seek);
    // Actual loopback TCP HTTP + byte response, through the same production route graph.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = host.router.clone();
    let task = tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<SocketAddr>(),
        )
        .await
        .unwrap();
    });
    let net = reqwest::Client::new()
        .get(format!("http://{address}{path}"))
        .bearer_auth(&a.access)
        .header("range", "bytes=7-16")
        .send()
        .await
        .unwrap();
    assert_eq!(net.status().as_u16(), 206);
    assert_eq!(
        net.bytes().await.unwrap().as_ref(),
        &work.units[0].chain.bytes[7..17]
    );
    task.abort();
    // A new session of the same actor cannot reuse the URL or cursor.
    let login = send(
        &host.router,
        "POST",
        &theatre("/native/identity/login"),
        &[],
        json_body(&json!({"email":OWNER,"password":PASSWORD})),
    )
    .await;
    let a2 = login.json()["access_token"].as_str().unwrap().to_owned();
    refused_fetch(&host, &path, &a2).await;
    assert_eq!(
        send(&host.router, "GET", &continuation, &[bearer(&a2)], None)
            .await
            .status,
        StatusCode::BAD_REQUEST
    );
    // Browser cookie POST requires exact Origin; media consumes that cookie, never a Studio one.
    let web = send(
        &host.router,
        "POST",
        &theatre("/identity/login"),
        &[("origin", THEATRE.into())],
        json_body(&json!({"email":OWNER,"password":PASSWORD})),
    )
    .await;
    assert_eq!(web.status, StatusCode::OK);
    let cookie = web
        .headers
        .get("set-cookie")
        .unwrap()
        .to_str()
        .unwrap()
        .split(';')
        .next()
        .unwrap()
        .to_owned();
    let auth_path = theatre(&format!(
        "/shared/publications/{release}/playback-authorizations"
    ));
    let csrf = send(
        &host.router,
        "POST",
        &auth_path,
        &[("cookie", cookie.clone())],
        Some(b"{}".to_vec()),
    )
    .await;
    assert_eq!(csrf.status, StatusCode::UNAUTHORIZED);
    private_headers(&csrf);
    let web_grant = send(
        &host.router,
        "POST",
        &auth_path,
        &[("origin", THEATRE.into()), ("cookie", cookie.clone())],
        Some(b"{}".to_vec()),
    )
    .await;
    assert_eq!(web_grant.status, StatusCode::OK);
    let web_path = web_grant.json()["url"]
        .as_str()
        .unwrap()
        .strip_prefix("https://api.example")
        .unwrap()
        .to_owned();
    let web_bytes = send(
        &host.router,
        "GET",
        &web_path,
        &[
            ("origin", THEATRE.into()),
            ("cookie", cookie.clone()),
            ("range", "bytes=0-9".into()),
        ],
        None,
    )
    .await;
    assert_eq!(web_bytes.status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(web_bytes.body, work.units[0].chain.bytes[..10]);
    assert_eq!(
        web_bytes.header("access-control-allow-origin"),
        Some(THEATRE)
    );
    assert_eq!(
        web_bytes.header("access-control-allow-credentials"),
        Some("true")
    );
    let mixed = send(
        &host.router,
        "GET",
        &web_path,
        &[("cookie", cookie), bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(mixed.status, StatusCode::UNAUTHORIZED);
    // Committed changes invalidate cursors/grants, even when another grant still permits a fresh read.
    change(
        &fixture,
        &work.work_id,
        Change::RevokeGrant { grant_id: direct },
    )
    .await;
    refused_fetch(&host, &path, &a.access).await;
    assert_eq!(
        send(
            &host.router,
            "GET",
            &continuation,
            &[bearer(&a.access)],
            None
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    let group_path = private_grant(&host, &a.access, release).await;
    fixture
        .store()
        .viewer_group_execute(
            CONTROL,
            &group,
            ViewerGroupRequest {
                operation_id: Uuid::new_v4().to_string(),
                expected_version: 2,
                change: ViewerGroupChange::RemoveMember {
                    actor_id: a.actor.clone(),
                },
            },
        )
        .await
        .unwrap();
    refused_fetch(&host, &group_path, &a.access).await;
    let hidden = send(
        &host.router,
        "GET",
        &theatre(&format!("/shared/works/{}", work.work_id)),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(hidden.status, StatusCode::NOT_FOUND);
    let b_path = private_grant(&host, &b.access, release).await;
    fixture.withdraw_release("alice", &work.units[0]).await;
    refused_fetch(&host, &b_path, &b.access).await;
    // A new process activation epoch cannot accept a previously issued grant/cursor.
    let restarted = restricted_host(&fixture, &env, storage);
    refused_fetch(&restarted, &group_path, &a.access).await;
    assert_eq!(
        send(
            &restarted.router,
            "GET",
            &continuation,
            &[bearer(&a.access)],
            None
        )
        .await
        .status,
        StatusCode::BAD_REQUEST
    );
    drop(identity);
    let outage = send(
        &host.router,
        "GET",
        &theatre("/shared/works"),
        &[bearer(&b.access)],
        None,
    )
    .await;
    assert_eq!(outage.status, StatusCode::SERVICE_UNAVAILABLE);
    private_headers(&outage);
}

/// The real filesystem open succeeds, then an actual owner grant revocation commits in Turso.
/// This is a named interleaving adapter, not a fake object or authorization result.
struct RevokeDuringOpen {
    storage: Arc<FilesystemPublicationStorage>,
    store: Store,
    work: String,
    grant: String,
    fired: Arc<AtomicBool>,
}
impl PublicationObjects for RevokeDuringOpen {
    fn open(
        &self,
        object: PublicationObjectRef,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        let opened = self.storage.open(object, range);
        let store = self.store.clone();
        let work = self.work.clone();
        let grant = self.grant.clone();
        let fired = self.fired.clone();
        Box::pin(async move {
            let result = opened.await?;
            if !fired.swap(true, Ordering::SeqCst) {
                let state = store.viewer_access_state(CONTROL, &work).await.unwrap();
                store
                    .viewer_access_execute(
                        CONTROL,
                        &work,
                        ViewerAccessRequest {
                            operation_id: Uuid::new_v4().to_string(),
                            expected_version: state.version,
                            change: Change::RevokeGrant { grant_id: grant },
                        },
                    )
                    .await
                    .unwrap();
            }
            Ok(result)
        })
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_bytes_opened_before_a_committed_revoke_are_dropped_unread() {
    let _ = supplied_identity();
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let _identity = env.serve().await;
    let fixture = ServerFixture::new().await;
    let storage =
        Arc::new(FilesystemPublicationStorage::new(&fixture.dir().join("publication")).unwrap());
    let work = fixture
        .seed_published_work(
            Some(&storage),
            &WorkSeed::podcast(0x6531, "Post-open private", &["Unread bytes"]),
        )
        .await;
    fixture.studio_caller("alice", CONTROL).await;
    let account = enroll(&compose(&fixture, &env), &sink, OWNER, 1).await;
    change(
        &fixture,
        &work.work_id,
        Change::SetAudience {
            audience: ViewerAudience::Restricted,
            confirm_metadata_rights: false,
        },
    )
    .await;
    let grant = actor_grant(&fixture, &work.work_id, &account.actor).await;
    let fired = Arc::new(AtomicBool::new(false));
    let objects = Arc::new(RevokeDuringOpen {
        storage,
        store: fixture.store().clone(),
        work: work.work_id.clone(),
        grant,
        fired: fired.clone(),
    });
    let host = restricted_host(&fixture, &env, objects);
    let path = private_grant(
        &host,
        &account.access,
        work.units[0].chain.release_id.as_deref().unwrap(),
    )
    .await;
    let response = send(
        &host.router,
        "GET",
        &path,
        &[bearer(&account.access), ("range", "bytes=0-15".into())],
        None,
    )
    .await;
    assert!(fired.load(Ordering::SeqCst));
    assert_eq!(response.status, StatusCode::NOT_FOUND);
    assert_eq!(
        response.json(),
        json!({"code":"not_found","retryable":false})
    );
    private_headers(&response);
    refused_fetch(&host, &path, &account.access).await;
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn restricted_unpublished_grant_expiry_generation_logout_and_reopen_stay_closed() {
    use cantos_server::{
        contexts::media::MetadataPublicationService,
        contract::studio::MetadataPublicationRequest as Metadata,
    };
    let _ = supplied_identity();
    let run = RunDir::new();
    let sink = SmtpSink::start().await;
    let env = IdentityEnv::new(&run, sink.port);
    env.migrate().await;
    let _identity = env.serve().await;
    let mut fixture = ServerFixture::new().await;
    let storage =
        Arc::new(FilesystemPublicationStorage::new(&fixture.dir().join("publication")).unwrap());
    let work = fixture
        .seed_work(
            Some(&storage),
            &WorkSeed::podcast(
                0x6532,
                "Unpublished cannot be shared",
                &["Selected", "Unselected"],
            ),
        )
        .await;
    fixture.studio_caller("alice", CONTROL).await;
    let public = compose(&fixture, &env);
    let a = enroll(&public, &sink, OWNER, 1).await;
    change(
        &fixture,
        &work.work_id,
        Change::SetAudience {
            audience: ViewerAudience::Restricted,
            confirm_metadata_rights: false,
        },
    )
    .await;
    let direct = actor_grant(&fixture, &work.work_id, &a.actor).await;
    let host = restricted_host(&fixture, &env, storage.clone());
    let detail = theatre(&format!("/shared/works/{}", work.work_id));
    assert_eq!(
        send(&host.router, "GET", &detail, &[bearer(&a.access)], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    let before = send(
        &host.router,
        "GET",
        &theatre("/shared/works"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert!(before.json()["items"].as_array().unwrap().is_empty());
    fixture.publish_work_metadata(&work).await;
    let release = work.units[0].chain.release_id.as_deref().unwrap();
    let path = private_grant(&host, &a.access, release).await;
    let first = send(
        &host.router,
        "GET",
        &format!("{detail}/units?limit=1"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    let cursor = first.json()["next_cursor"].as_str().unwrap().to_owned();
    let metadata = MetadataPublicationService::new(fixture.store().clone());
    metadata
        .settle(
            CONTROL,
            &work.work_id,
            Metadata::WithdrawUnit {
                operation_id: Uuid::new_v4().to_string(),
                expected_version: 1,
                unit_id: work.units[1].unit_id.clone(),
            },
        )
        .await
        .unwrap();
    refused_fetch(&host, &path, &a.access).await;
    let stale = send(
        &host.router,
        "GET",
        &format!("{detail}/units?limit=1&cursor={cursor}"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(stale.status, StatusCode::BAD_REQUEST);
    assert_eq!(stale.json()["code"], "invalid_cursor");
    let units = send(
        &host.router,
        "GET",
        &format!("{detail}/units"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(units.json()["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        send(
            &host.router,
            "GET",
            &theatre(&format!(
                "/shared/publications/{}",
                work.units[1].chain.release_id.as_ref().unwrap()
            )),
            &[bearer(&a.access)],
            None
        )
        .await
        .status,
        StatusCode::NOT_FOUND
    );
    change(
        &fixture,
        &work.work_id,
        Change::RevokeGrant { grant_id: direct },
    )
    .await;
    let deadline = fixture.clock_reading().unwrap() + 3_000_000;
    change(
        &fixture,
        &work.work_id,
        Change::GrantActor {
            grant_id: Uuid::new_v4().to_string(),
            actor_id: a.actor.clone(),
            expires_at_us: Some(deadline),
        },
    )
    .await;
    let expires = private_grant(&host, &a.access, release).await;
    let token = expires
        .strip_prefix("/api/v1/theatre/shared/media/")
        .unwrap();
    assert!(
        cantos_server::contexts::media::domain::playback_grant::GrantClaims::parse(token)
            .unwrap()
            .expires_at_ms()
            <= deadline / 1000
    );
    tokio::time::sleep(std::time::Duration::from_millis(3200)).await;
    refused_fetch(&host, &expires, &a.access).await;
    assert_eq!(
        send(&host.router, "GET", &detail, &[bearer(&a.access)], None)
            .await
            .status,
        StatusCode::NOT_FOUND
    );
    actor_grant(&fixture, &work.work_id, &a.actor).await;
    let fresh = private_grant(&host, &a.access, release).await;
    fixture
        .exec("UPDATE actors SET active=0 WHERE id='alice'", &[])
        .await
        .unwrap();
    refused_fetch(&host, &fresh, &a.access).await;
    let hidden_owner = send(
        &host.router,
        "GET",
        &theatre("/shared/works"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(hidden_owner.status, StatusCode::OK);
    assert!(hidden_owner.json()["items"].as_array().unwrap().is_empty());
    assert!(hidden_owner.json()["next_cursor"].is_null());
    fixture
        .exec("UPDATE actors SET active=1 WHERE id='alice'", &[])
        .await
        .unwrap();
    let fresh = private_grant(&host, &a.access, release).await;
    let logout = send(
        &host.router,
        "DELETE",
        &theatre("/native/identity/session"),
        &[bearer(&a.access)],
        None,
    )
    .await;
    assert_eq!(logout.status, StatusCode::NO_CONTENT);
    let denied = send(&host.router, "GET", &fresh, &[bearer(&a.access)], None).await;
    assert_eq!(denied.status, StatusCode::UNAUTHORIZED);
    assert_ne!(denied.header("content-type"), Some("audio/ogg"));
    private_headers(&denied);
    let login = send(
        &host.router,
        "POST",
        &theatre("/native/identity/login"),
        &[],
        json_body(&json!({"email":OWNER,"password":PASSWORD})),
    )
    .await;
    let next = login.json()["access_token"].as_str().unwrap().to_owned();
    let before_reopen = private_grant(&host, &next, release).await;
    drop(metadata);
    drop(host);
    drop(public);
    fixture.restart().await; // Actual close/serve of server.db, with Identity still independent.
    let closed = compose(&fixture, &env);
    let no_activation = send(&closed.router, "GET", &detail, &[bearer(&next)], None).await;
    assert_eq!(no_activation.status, StatusCode::NOT_FOUND);
    let reopened = restricted_host(&fixture, &env, storage);
    refused_fetch(&reopened, &before_reopen, &next).await;
    let after_reopen = private_grant(&reopened, &next, release).await;
    let body = send(
        &reopened.router,
        "GET",
        &after_reopen,
        &[bearer(&next)],
        None,
    )
    .await;
    assert_eq!(body.status, StatusCode::OK);
    assert_eq!(body.body, work.units[0].chain.bytes);
    let metadata = MetadataPublicationService::new(fixture.store().clone());
    metadata
        .settle(
            CONTROL,
            &work.work_id,
            Metadata::Withdraw {
                operation_id: Uuid::new_v4().to_string(),
                expected_version: 2,
            },
        )
        .await
        .unwrap();
    refused_fetch(&reopened, &after_reopen, &next).await;
}
