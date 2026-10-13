//! Production router-composition probes with lazy pools and synthetic bodies/files only.
//! No database, session-owner SQL, object storage or authorized loopback acceptance is claimed.
use axum::{
    body::{Body, Bytes},
    http::{header, Method, Request as HttpRequest, StatusCode},
};
use cantos_api::{contexts::media::ArtworkService, interfaces::http::AppState};
use tower::ServiceExt;

#[tokio::test]
async fn actual_trusted_mount_is_owner_gated_with_no_legacy_or_public_alias() {
    use cantos_api::interfaces::http::{router, router_with_private_artwork};
    use cantos_api::platform::postgres::{local_config, Store};
    let state = AppState {
        store: Store::new(local_config("postgresql://cantos@127.0.0.1/cantos").unwrap()).unwrap(),
        web_origin: "http://localhost:3000".into(),
    };
    // Pool construction is lazy. Missing cookies must refuse without a DB connection
    // or body poll; this establishes routing/session admission, not real owner SQL.
    let default = router(state.clone(), "/nonexistent-cantos-artwork-dist");
    let trusted = router_with_private_artwork(
        state.clone(),
        "/nonexistent-cantos-artwork-dist",
        ArtworkService::new(state.store.clone()),
    );
    let path = "/api/v1/studio/works/00000000-0000-4000-8000-000000000001/artwork";
    let response = default
        .oneshot(HttpRequest::get(path).body(Body::empty()).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
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
    ] {
        let path = format!("/api/v1/studio/works/00000000-0000-4000-8000-000000000001/{suffix}");
        let body = Body::from_stream(futures_util::stream::poll_fn(|_| {
            panic!("anonymous artwork admission must not poll the body");
            #[allow(unreachable_code)]
            std::task::Poll::Ready(None::<Result<Bytes, std::io::Error>>)
        }));
        let response = trusted
            .clone()
            .oneshot(
                HttpRequest::builder()
                    .method(method)
                    .uri(path)
                    .header(header::ORIGIN, &state.web_origin)
                    .body(body)
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
        assert_eq!(
            response.headers()[header::X_CONTENT_TYPE_OPTIONS],
            "nosniff"
        );
    }
    for path in [
        "/api/v1/works/00000000-0000-4000-8000-000000000001/artwork",
        "/api/v1/theatre/works/00000000-0000-4000-8000-000000000001/image-assets/00000000-0000-4000-8000-000000000002",
    ] {
        let response = trusted.clone().oneshot(HttpRequest::get(path).body(Body::empty()).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::NOT_FOUND);
    }
}

#[tokio::test]
async fn explicit_private_artwork_host_disables_filesystem_static_fallback() {
    use cantos_api::interfaces::http::{router, router_with_private_artwork};
    use cantos_api::platform::postgres::{local_config, Store};
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
    let state = AppState {
        store: Store::new(local_config("postgresql://cantos@127.0.0.1/cantos").unwrap()).unwrap(),
        web_origin: "http://localhost:3000".into(),
    };
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
