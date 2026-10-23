//! Creator HTTP chain over real Turso, inspected private bytes and real publication storage.
//! Development Studio authority is synthetic; the HTTPS origin is a routing identity, not TLS.
use super::*;
use cantos_server::{
    adapters::inbound::http::router_with_public_media,
    contexts::media::{
        domain::playback_grant::GrantKey, FilesystemPublicationStorage, OriginPlayback,
        PublicMediaOrigin,
    },
};

const MEDIA_ORIGIN: &str = "https://api.example";

fn origin_router(h: &Harness, publication_root: &std::path::Path) -> Router {
    let storage = Arc::new(FilesystemPublicationStorage::new(publication_root).unwrap());
    let publication =
        PublicationService::new(h.store.clone()).with_origin_publication(storage.clone());
    let playback = OriginPlayback::new(
        PublicMediaOrigin::parse(MEDIA_ORIGIN).unwrap(),
        GrantKey::new(&[7; 32]).unwrap(),
        storage,
    );
    router_with_public_media(
        AppState {
            store: h.store.clone(),
            web_origin: ORIGIN.into(),
        },
        publication,
        playback,
    )
}

async fn media(app: &Router, path: &str, range: Option<&str>) -> (StatusCode, Vec<u8>) {
    let mut request = Request::builder().uri(path);
    if let Some(range) = range {
        request = request.header("range", range);
    }
    let response = app
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, bytes.to_vec())
}

#[tokio::test]
async fn creator_intake_to_explicit_publication_uses_real_origin_bytes_and_reconciles_after_reopen()
{
    let _media_lane = native_sql::media_lane().await;
    let mut a = AudioHarness::new_visibility(true, "opus-mono-24-v1", true).await;
    let publication_root = PrivateRoot::new();
    let bytes = a.root.encode("24k");
    let request = a.request(&bytes).await;
    let submission = request["id"].as_str().unwrap().to_owned();
    let prepared = a.prepare(request.clone()).await;
    assert_eq!(prepared["state"], "awaiting_bytes");
    assert_eq!(prepared["submission"]["binding"], request["binding"]);

    // Complete the production handler, then abandon its response before reading a receipt.
    // This covers an unobserved acknowledgement, not an injected network transport loss.
    let response =
        a.h.app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/api/v1/studio/producer/audio-submissions/{submission}/bytes"
                    ))
                    .header("origin", ORIGIN)
                    .header("content-type", "application/octet-stream")
                    .header("authorization", format!("Bearer {UPLOAD}"))
                    .body(Body::from(bytes.clone()))
                    .unwrap(),
            )
            .await
            .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    drop(response);
    a.h.restart_with_audio(&a.root.0).await;
    let reconciled = a.prepare(request.clone()).await;
    assert_eq!(reconciled["submission"], prepared["submission"]);
    assert_eq!(reconciled["state"], "admitted");
    let rendition = upload(a.h.app.clone(), &submission, UPLOAD, bytes.clone())
        .await
        .success(StatusCode::OK);
    assert_eq!(reconciled["rendition"], rendition);
    assert_eq!(rendition["binding"], request["binding"]);
    assert_eq!(
        rendition["verified_media"]["sha256"],
        request["expected_sha256"]
    );
    assert_eq!(rendition["verified_media"]["byte_length"], bytes.len());
    assert_eq!(a.counts().await["renditions"], 1);
    let private_path = format!(
        "/api/v1/studio{}/{submission}/renditions/{}/bytes",
        a.path(),
        rendition["id"].as_str().unwrap()
    );
    let (status, _, actual) = preview(&a.h.app, "GET", &private_path, Some(ALICE), &[]).await;
    assert_eq!((status, actual), (StatusCode::OK, bytes.clone()));
    assert_eq!(
        preview(&a.h.app, "GET", &private_path, Some(BOB), &[])
            .await
            .0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        theatre_call(a.h.app.clone(), "GET", "/works").await.1["items"],
        json!([])
    );

    let (candidate, _, _) = approve_candidate(
        &a.h,
        &a.unit,
        &submission,
        &rendition,
        now(&a.h).await + 3_600_000,
    )
    .await;
    let base = format!(
        "/content-units/{}/release-candidates/{}",
        a.unit,
        candidate["id"].as_str().unwrap()
    );
    let publish = json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"expected_pointer_version":0,"expected_active_release_id":null});
    let unavailable =
        a.h.json("GET", &format!("{base}/readiness"), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(unavailable["blockers"], json!(["delivery_unavailable"]));
    a.h.json(
        "POST",
        &format!("{base}/publish"),
        Some(ALICE),
        Some(publish.clone()),
    )
    .await
    .error(StatusCode::CONFLICT, "production_blocked");
    let active_path = format!("/content-units/{}/publication/active", a.unit);
    let absent =
        a.h.json("GET", &active_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(
        (absent["pointer_version"].clone(), absent["release"].clone()),
        (json!(0), Value::Null)
    );

    // This is the existing trusted origin assembly, with real promotion and byte inspection.
    // Readiness may stage approved bytes, but only the separate publish mutation activates them.
    a.h.app = origin_router(&a.h, &publication_root.0);
    let ready =
        a.h.json("GET", &format!("{base}/readiness"), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(ready["blockers"], json!([]));
    assert_eq!(
        theatre_call(a.h.app.clone(), "GET", "/works").await.1["items"],
        json!([])
    );
    a.h.json(
        "POST",
        &format!("{base}/publish"),
        Some(BOB),
        Some(publish.clone()),
    )
    .await
    .error(StatusCode::NOT_FOUND, "not_found");
    let published =
        a.h.json(
            "POST",
            &format!("{base}/publish"),
            Some(ALICE),
            Some(publish.clone()),
        )
        .await
        .success(StatusCode::CREATED);
    assert_eq!(published["pointer_version"], 1);
    assert_eq!(
        published["release"]["manifest"]["binding"],
        rendition["binding"]
    );
    assert_eq!(
        published["release"]["manifest"]["media"],
        rendition["verified_media"]
    );
    let release = published["release"]["id"].as_str().unwrap().to_owned();
    let (status, page) = theatre_call(a.h.app.clone(), "GET", "/works").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 1);
    let (status, manifest) =
        theatre_call(a.h.app.clone(), "GET", &format!("/publications/{release}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(manifest["sha256"], request["expected_sha256"]);
    assert_eq!(manifest["byte_length"], bytes.len());
    let (status, grant) = theatre_call(
        a.h.app.clone(),
        "POST",
        &format!("/publications/{release}/playback-authorizations"),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(grant["sha256"], request["expected_sha256"]);
    let media_path = grant["url"]
        .as_str()
        .unwrap()
        .strip_prefix(MEDIA_ORIGIN)
        .unwrap()
        .to_owned();
    assert_eq!(
        media(&a.h.app, &media_path, None).await,
        (StatusCode::OK, bytes.clone())
    );
    assert_eq!(
        media(&a.h.app, &media_path, Some("bytes=0-31")).await,
        (StatusCode::PARTIAL_CONTENT, bytes[..32].to_vec())
    );

    a.h.restart_with_audio(&a.root.0).await;
    a.h.app = origin_router(&a.h, &publication_root.0);
    assert_eq!(
        a.h.json("GET", &active_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        published
    );
    assert_eq!(
        a.h.json(
            "POST",
            &format!("{base}/publish"),
            Some(ALICE),
            Some(publish)
        )
        .await
        .success(StatusCode::CREATED),
        published
    );
    assert_eq!(
        media(&a.h.app, &media_path, None).await,
        (StatusCode::OK, bytes.clone())
    );
    let digest = request["expected_sha256"].as_str().unwrap();
    assert_eq!(
        fs::read(a.root.0.join("objects").join(digest)).unwrap(),
        bytes
    );
    assert_eq!(
        fs::read(publication_root.0.join("objects").join(digest)).unwrap(),
        bytes
    );
    assert_eq!(a.counts().await["renditions"], 1);

    a.h.json("POST", &format!("/content-units/{}/publication/withdraw", a.unit), Some(ALICE), Some(json!({"operation_id":operation(),"release_id":release,"expected_pointer_version":1,"reason":"Synthetic owner withdrawal after exact-byte acceptance"}))).await.success(StatusCode::OK);
    assert_eq!(
        theatre_call(a.h.app.clone(), "GET", "/works").await.1["items"],
        json!([])
    );
    assert_eq!(
        media(&a.h.app, &media_path, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        preview(&a.h.app, "GET", &private_path, Some(ALICE), &[])
            .await
            .2,
        bytes
    );
}
