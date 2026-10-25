//! Production router-composition probes with lazy pools and synthetic bodies/files only.
//! No database, session-owner SQL, object storage or authorized loopback acceptance is claimed.
use axum::{
    body::{Body, Bytes},
    http::{header, Method, Request as HttpRequest, StatusCode},
    response::Response,
    Router,
};
use cantos_server::{
    adapters::inbound::http::{
        api_router, router, router_with_listener_identity, router_with_origin_playback,
        router_with_private_artwork, router_with_public_media, router_with_publication_delivery,
        router_with_theatre_delivery, server_router, AppState,
    },
    contexts::{
        listening::auth::DenyListenerAuthentication,
        media::{
            domain::{object_storage::NormalizedByteRange, playback_grant::GrantKey},
            storage::port::{StorageFuture, StorageRead},
            ArtworkService, OriginPlayback, PlaybackDelivery, PlaybackDeliveryFuture,
            PlaybackPermit, PublicMediaOrigin, PublicationDelivery, PublicationDeliveryFuture,
            PublicationObject, PublicationObjectRef, PublicationObjects, PublicationService,
        },
    },
};
use http_body_util::BodyExt;
use std::sync::Arc;
use tower::ServiceExt;

const WORK: &str = "00000000-0000-4000-8000-000000000001";
const IMAGE: &str = "00000000-0000-4000-8000-000000000002";

struct UncalledDelivery;
impl PublicationDelivery for UncalledDelivery {
    fn observe(&self, _: PublicationObject) -> PublicationDeliveryFuture {
        panic!("private artwork admission must never consult public delivery");
    }
}
impl PlaybackDelivery for UncalledDelivery {
    fn grant(&self, _: PlaybackPermit) -> PlaybackDeliveryFuture {
        panic!("private artwork admission must never consult playback delivery");
    }
}
impl PublicationObjects for UncalledDelivery {
    fn open(
        &self,
        _: PublicationObjectRef,
        _: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        panic!("private artwork admission must never read public objects");
    }
}
fn origin_playback() -> OriginPlayback {
    OriginPlayback::new(
        PublicMediaOrigin::parse("https://listen.example.test").unwrap(),
        GrantKey::new(&[7; 32]).unwrap(),
        Arc::new(UncalledDelivery),
    )
}
fn production_routers(state: AppState, dist: &str) -> [(&'static str, Router); 8] {
    [
        (
            "combined public media",
            server_router(
                state.clone(),
                &cantos_server::platform::host::SurfaceOrigins::new(
                    &state.web_origin,
                    "https://listen.example.test",
                    None,
                )
                .unwrap(),
                None,
                Some((
                    PublicationService::new(state.store.clone()),
                    origin_playback(),
                )),
            ),
        ),
        ("default", router(state.clone(), dist)),
        ("API only", api_router(state.clone())),
        (
            "origin playback",
            router_with_origin_playback(state.clone(), dist, origin_playback()),
        ),
        (
            "public media",
            router_with_public_media(
                state.clone(),
                PublicationService::new(state.store.clone()),
                origin_playback(),
            ),
        ),
        (
            "publication delivery",
            router_with_publication_delivery(state.clone(), dist, Arc::new(UncalledDelivery)),
        ),
        (
            "Theatre delivery",
            router_with_theatre_delivery(state.clone(), dist, Arc::new(UncalledDelivery)),
        ),
        (
            "listener identity",
            router_with_listener_identity(state, dist, Arc::new(DenyListenerAuthentication)),
        ),
    ]
}
fn lazy_state() -> AppState {
    AppState {
        store: cantos_server::test_support::detached_store(),
        web_origin: "http://localhost:3000".into(),
    }
}
fn artwork_routes() -> [(Method, String); 9] {
    [
        (Method::POST, "image-intents".into()),
        (Method::GET, format!("image-intents/{IMAGE}")),
        (Method::PUT, format!("image-intents/{IMAGE}/bytes")),
        (Method::GET, format!("image-assets/{IMAGE}")),
        (Method::GET, format!("image-assets/{IMAGE}/bytes")),
        (Method::HEAD, format!("image-assets/{IMAGE}/bytes")),
        (Method::GET, "artwork".into()),
        (Method::PATCH, "artwork".into()),
        (Method::POST, "cover-disclosures".into()),
    ]
}
fn unpolled_body() -> Body {
    Body::from_stream(futures_util::stream::poll_fn(|_| {
        panic!("refused artwork admission must not poll the body");
        #[allow(unreachable_code)]
        std::task::Poll::Ready(None::<Result<Bytes, std::io::Error>>)
    }))
}
async fn assert_private_error(response: Response, status: StatusCode, code: &str, head: bool) {
    assert_eq!(response.status(), status);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(
        response.headers()[header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    if head {
        assert!(bytes.is_empty());
    } else {
        assert_eq!(
            serde_json::from_slice::<serde_json::Value>(&bytes).unwrap(),
            serde_json::json!({"code": code, "current_revision": null, "issues": []}),
        );
    }
}

#[tokio::test]
async fn actual_trusted_mount_is_owner_gated_with_no_legacy_or_public_alias() {
    let state = lazy_state();
    // Pool construction is lazy. Missing cookies must refuse without a DB connection
    // or body poll; this establishes routing/session admission, not real owner SQL.
    let trusted = router_with_private_artwork(
        state.clone(),
        "/nonexistent-cantos-artwork-dist",
        ArtworkService::new(state.store.clone()),
    );
    let path = "/api/v1/studio/works/00000000-0000-4000-8000-000000000001/artwork";
    for (name, app) in production_routers(state.clone(), "/nonexistent-cantos-artwork-dist") {
        let response = app
            .oneshot(HttpRequest::get(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND, "{name}");
    }
    for (method, suffix) in [
        (Method::POST, "image-intents"),
        (
            Method::GET,
            "image-intents/00000000-0000-4000-8000-000000000002",
        ),
        (
            Method::PUT,
            "image-intents/00000000-0000-4000-8000-000000000002/bytes",
        ),
        (
            Method::GET,
            "image-assets/00000000-0000-4000-8000-000000000002",
        ),
        (
            Method::GET,
            "image-assets/00000000-0000-4000-8000-000000000002/bytes",
        ),
        (
            Method::HEAD,
            "image-assets/00000000-0000-4000-8000-000000000002/bytes",
        ),
        (Method::GET, "artwork"),
        (Method::PATCH, "artwork"),
        (Method::POST, "cover-disclosures"),
    ] {
        let path = format!("/api/v1/studio/works/00000000-0000-4000-8000-000000000001/{suffix}");
        let response = trusted
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method(method.clone())
                    .uri(path)
                    .header(header::ORIGIN, &state.web_origin)
                    .body(unpolled_body())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_private_error(
            response,
            StatusCode::UNAUTHORIZED,
            "unauthenticated",
            method == Method::HEAD,
        )
        .await;
    }
    for path in [
        "/api/v1/works/00000000-0000-4000-8000-000000000001/artwork",
        "/api/v1/theatre/works/00000000-0000-4000-8000-000000000001/image-assets/00000000-0000-4000-8000-000000000002",
        "/api/v1/admin/works/00000000-0000-4000-8000-000000000001/artwork",
        "/api/v1/works/00000000-0000-4000-8000-000000000001/cover-disclosures",
        "/api/v1/theatre/works/00000000-0000-4000-8000-000000000001/cover-disclosures",
    ] {
        let response = trusted.clone().oneshot(HttpRequest::get(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn explicit_private_artwork_host_disables_filesystem_static_fallback() {
    struct SyntheticDist(std::path::PathBuf);
    impl Drop for SyntheticDist {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let dist_path = std::env::temp_dir().join(format!(
        "cantos-artwork-static-test-{}",
        uuid::Uuid::new_v4()
    ));
    std::fs::create_dir(&dist_path).unwrap();
    let dist = SyntheticDist(dist_path);
    std::fs::write(dist.0.join("synthetic.txt"), "synthetic static marker").unwrap();
    let state = lazy_state();
    let normal = router(state.clone(), dist.0.to_str().unwrap());
    let private = router_with_private_artwork(
        state.clone(),
        dist.0.to_str().unwrap(),
        ArtworkService::new(state.store),
    );
    for (app, expected) in [(normal, StatusCode::OK), (private, StatusCode::NOT_FOUND)] {
        let response = app
            .oneshot(
                HttpRequest::get("/synthetic.txt")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), expected);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn shared_private_storage_mounts_artwork_in_every_production_constructor() {
    struct PrivateRoot(std::path::PathBuf);
    impl Drop for PrivateRoot {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
    let root = PrivateRoot(std::env::temp_dir().join(format!(
        "cantos-artwork-composition-test-{}",
        uuid::Uuid::new_v4()
    )));
    let mut state = lazy_state();
    state.store = state.store.with_audio_storage(&root.0).unwrap();
    std::fs::write(root.0.join("synthetic.txt"), "synthetic private marker").unwrap();
    for (constructor, app) in production_routers(state.clone(), root.0.to_str().unwrap()) {
        for (method, suffix) in artwork_routes() {
            let response = app
                .clone()
                .oneshot(
                    HttpRequest::builder()
                        .method(method.clone())
                        .uri(format!("/api/v1/studio/works/{WORK}/{suffix}"))
                        .header(header::ORIGIN, &state.web_origin)
                        .body(unpolled_body())
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{constructor} {method} {suffix}"
            );
            assert_private_error(
                response,
                StatusCode::UNAUTHORIZED,
                "unauthenticated",
                method == Method::HEAD,
            )
            .await;
        }
        for path in [
            "/synthetic.txt".into(),
            format!("/api/v1/works/{WORK}/artwork"),
            format!("/api/v1/theatre/works/{WORK}/artwork"),
            format!("/api/v1/admin/works/{WORK}/artwork"),
        ] {
            let response = app
                .clone()
                .oneshot(HttpRequest::get(path).body(Body::empty()).unwrap())
                .await
                .unwrap();
            assert_eq!(response.status(), StatusCode::NOT_FOUND, "{constructor}");
        }
    }
}

#[tokio::test]
async fn artwork_transport_fences_and_cookie_names_never_replace_current_studio_authentication() {
    // Only the development-token cookie name exists; the retired provider cookie name is never
    // an alternative spelling, on an http or an https Studio origin.
    for https_origin in [false, true] {
        let mut state = lazy_state();
        if https_origin {
            state.web_origin = "https://studio.example".into();
        }
        let app = router_with_private_artwork(
            state.clone(),
            "/nonexistent-cantos-artwork-dist",
            ArtworkService::new(state.store.clone()),
        );
        let expected_name = "cantos_session";
        let other_name = "__Secure-cantos_studio";
        // These failures occur before SQL or identity I/O. Accepted cookie extraction and
        // authenticated stale/duplicate fence 409s remain real-session acceptance cases.
        for (method, suffix) in artwork_routes() {
            for query in [
                "",
                "?studio_session_id=forged",
                "?studio_session_id=a&studio_session_id=b",
            ] {
                for cookie in [
                    None,
                    Some(format!("{other_name}=synthetic-token")),
                    Some(format!("{expected_name}=a; {expected_name}=b")),
                    Some(format!("{expected_name}=a; {other_name}=b")),
                ] {
                    let mut request = HttpRequest::builder()
                        .method(method.clone())
                        .uri(format!("/api/v1/studio/works/{WORK}/{suffix}{query}"))
                        .header(header::ORIGIN, &state.web_origin)
                        .header("x-cantos-studio-session", "forged")
                        .header(header::CONTENT_TYPE, "application/json")
                        .header(header::ACCEPT, "application/msgpack");
                    if let Some(cookie) = cookie {
                        request = request.header(header::COOKIE, cookie);
                    }
                    let response = app
                        .clone()
                        .oneshot(request.body(unpolled_body()).unwrap())
                        .await
                        .unwrap();
                    assert_private_error(
                        response,
                        StatusCode::UNAUTHORIZED,
                        "unauthenticated",
                        method == Method::HEAD,
                    )
                    .await;
                }
            }
        }
        let response = app
            .oneshot(
                HttpRequest::patch(format!("/api/v1/studio/works/{WORK}/artwork"))
                    .header(header::ORIGIN, "https://foreign.example")
                    .body(unpolled_body())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_private_error(response, StatusCode::FORBIDDEN, "forbidden", false).await;
    }
}
