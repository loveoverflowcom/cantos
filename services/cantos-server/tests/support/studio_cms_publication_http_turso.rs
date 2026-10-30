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
    let work_id = a.brief["work"]["id"].as_str().unwrap();
    publish_metadata(&a.h, work_id, &[&a.unit]).await;
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

    // Metadata withdrawal alone revokes an already-issued grant while audio remains active.
    withdraw_metadata(&a.h, work_id).await;
    assert_eq!(
        media(&a.h.app, &media_path, None).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        theatre_call(a.h.app.clone(), "GET", "/works").await.1["items"],
        json!([])
    );
    assert_eq!(
        a.h.json("GET", &active_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        published
    );

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

/// #539 + ADR 0054: saved metadata alone remains private. Explicit metadata publication may
/// disclose text without audio. Later exact audio activation atomically records its association.
#[tokio::test]
async fn metadata_saved_without_audio_requires_publication_then_exact_audio_activation() {
    let _media_lane = native_sql::media_lane().await;
    let mut a = AudioHarness::new_visibility(true, "opus-mono-24-v1", true).await;
    let publication_root = PrivateRoot::new();
    let work = a.brief["work"]["id"].as_str().unwrap().to_owned();
    let audio_free =
        json!({"submissions":0,"renditions":0,"reviews":0,"candidates":0,"revocations":0});
    assert_eq!(a.counts().await, audio_free);

    let created =
        a.h.json("GET", &format!("/works/{work}"), Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(created["details"]["visibility"], "public");
    let work_title = "Ánh đèn cuối sân khấu — bản sửa chưa có âm thanh";
    let edited =
        a.h.json(
            "PATCH",
            &format!("/works/{work}"),
            Some(ALICE),
            Some(json!({"operation_id":operation(),"expected_revision":created["revision"],"details":{"title":work_title}})),
        )
        .await
        .success(StatusCode::OK)["work"]
            .clone();
    assert_eq!(edited["id"], work);
    assert_eq!(
        edited["revision"],
        created["revision"].as_u64().unwrap() + 1
    );
    let unit_path = format!("/content-units/{}", a.unit);
    let unit_before =
        a.h.json("GET", &unit_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    let unit_title = "Tập một — đã sửa, chờ âm thanh";
    a.h.json(
        "PATCH",
        &unit_path,
        Some(ALICE),
        Some(json!({"operation_id":operation(),"expected_revision":unit_before["revision"],"expected_work_revision":edited["revision"],"details":{"title":unit_title}})),
    )
    .await
    .success(StatusCode::OK);
    let unit_after =
        a.h.json("GET", &unit_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK);
    assert_eq!(
        (unit_after["id"].as_str(), unit_after["work_id"].as_str()),
        (Some(a.unit.as_str()), Some(work.as_str()))
    );
    assert_eq!(
        unit_after["revision"],
        unit_before["revision"].as_u64().unwrap() + 1
    );
    assert_eq!(unit_after["details"]["title"], unit_title);
    let catalog = a.h.counts().await;
    assert_eq!(
        (catalog["works"].clone(), catalog["units"].clone()),
        (json!(1), json!(1))
    );
    assert_eq!(a.counts().await, audio_free);

    // Public visibility alone is not a metadata disclosure: every anonymous read stays closed.
    for list in [
        "/works".to_owned(),
        "/search?q=%C3%81nh%20%C4%91%C3%A8n".to_owned(),
    ] {
        assert_eq!(
            theatre_call(a.h.app.clone(), "GET", &list).await,
            (StatusCode::OK, json!({"items":[],"next_cursor":null})),
            "{list}"
        );
    }
    for detail in [format!("/works/{work}"), format!("/works/{work}/units")] {
        let (status, body) = theatre_call(a.h.app.clone(), "GET", &detail).await;
        assert_eq!(
            (status, &body["code"]),
            (StatusCode::NOT_FOUND, &json!("not_found")),
            "{detail}"
        );
    }

    publish_metadata(&a.h, &work, &[&a.unit]).await;
    assert_eq!(
        theatre_call(a.h.app.clone(), "GET", &format!("/works/{work}/units"))
            .await
            .1["items"][0]["playback"],
        Value::Null
    );

    // Audio arrives later through the existing intake, review, rights and approval gates.
    let bytes = a.root.encode("24k");
    let request = a.request(&bytes).await;
    let submission = request["id"].as_str().unwrap().to_owned();
    assert_eq!(request["binding"]["work_id"], work);
    assert_eq!(request["binding"]["unit_id"], a.unit);
    a.prepare(request.clone()).await;
    let rendition = upload(a.h.app.clone(), &submission, UPLOAD, bytes)
        .await
        .success(StatusCode::OK);
    let (candidate, _, _) = approve_candidate(
        &a.h,
        &a.unit,
        &submission,
        &rendition,
        now(&a.h).await + 3_600_000,
    )
    .await;
    a.h.app = origin_router(&a.h, &publication_root.0);
    let published =
        a.h.json(
            "POST",
            &format!(
                "/content-units/{}/release-candidates/{}/publish",
                a.unit,
                candidate["id"].as_str().unwrap()
            ),
            Some(ALICE),
            Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"expected_pointer_version":0,"expected_active_release_id":null})),
        )
        .await
        .success(StatusCode::CREATED);
    let release = published["release"]["id"].as_str().unwrap();
    let state =
        a.h.json(
            "GET",
            &format!("/works/{work}/metadata-publication"),
            Some(ALICE),
            None,
        )
        .await
        .success(StatusCode::OK);
    assert_eq!(state["version"], 2);
    assert_eq!(
        state["generation"]["entries"][0]["audio"]["release_id"],
        release
    );
    assert_eq!(state["generation"]["entries"][0]["audio"]["basis"], "exact");

    let (status, page) = theatre_call(a.h.app.clone(), "GET", "/works").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(page["items"].as_array().unwrap().len(), 1, "{page}");
    assert_eq!(
        (&page["items"][0]["work_id"], &page["items"][0]["title"]),
        (&json!(work), &json!(work_title))
    );
    let (status, units) =
        theatre_call(a.h.app.clone(), "GET", &format!("/works/{work}/units")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(units["items"].as_array().unwrap().len(), 1, "{units}");
    assert_eq!(
        (
            &units["items"][0]["unit_id"],
            &units["items"][0]["title"],
            &units["items"][0]["playback"]["publication_id"]
        ),
        (&json!(a.unit), &json!(unit_title), &json!(release))
    );
    // Adding audio created no Catalog identity or revision.
    assert_eq!(a.h.counts().await, catalog);
    assert_eq!(
        a.h.json("GET", &unit_path, Some(ALICE), None)
            .await
            .success(StatusCode::OK),
        unit_after
    );
}
