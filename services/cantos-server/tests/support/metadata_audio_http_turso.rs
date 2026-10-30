//! ADR 0054 D06c through the real Studio HTTP chain over real Turso: the audio activation that
//! moves a Unit's pointer also writes metadata generation N+1 associating that exact release,
//! in the same write unit, once. Only the public-delivery observer is a named fake.
use super::*;

fn metadata_path(work: &str) -> String {
    format!("/works/{work}/metadata-publication")
}

async fn head(f: &PublishedFixture, table: &str, id: &str) -> u64 {
    let revision: i64 =
        f.a.h
            .admin
            .query_one(
                &format!("SELECT head_revision FROM {table} WHERE id=?1"),
                &[&id],
            )
            .await
            .unwrap()
            .get(0);
    u64::try_from(revision).unwrap()
}

async fn events(f: &PublishedFixture) -> Vec<(i64, String)> {
    f.a.h
        .admin
        .query(
            "SELECT version,action FROM catalog_metadata_publication_events ORDER BY version",
            &[],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| (row.get(0), row.get(1)))
        .collect()
}

#[tokio::test]
async fn audio_activation_associates_the_exact_release_with_the_active_generation() {
    let _media_lane = native_sql::media_lane().await;
    let f = PublishedFixture::new(true).await;
    let work = f.a.brief["work"]["id"].as_str().unwrap().to_owned();
    let unit = f.a.unit.clone();
    let publish = json!({
        "action": "publish",
        "operation_id": operation(),
        "expected_version": 0,
        "work_revision": head(&f, "catalog_works", &work).await,
        "units": [{"unit_id": unit, "revision": head(&f, "catalog_units", &unit).await}],
        "confirm_metadata_rights": true
    });
    let h = &f.a.h;
    // Metadata first: the approved candidate is not active yet, so the entry is metadata-only.
    let published: Value = h
        .json("POST", &metadata_path(&work), Some(ALICE), Some(publish))
        .await
        .success(StatusCode::OK);
    assert_eq!(published["version"], 1);
    assert_eq!(published["generation"]["entries"][0]["audio"], Value::Null);

    // The real activation associates the exact release in its own write unit.
    let path = format!("{}/publish", f.base());
    let request = f.publish(0, Value::Null);
    let activated: Value = h
        .json("POST", &path, Some(ALICE), Some(request.clone()))
        .await
        .success(StatusCode::CREATED);
    let release = activated["release"]["id"].as_str().unwrap().to_owned();
    let state: Value = h
        .json("GET", &metadata_path(&work), Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(
        (
            state["version"].clone(),
            state["generation"]["number"].clone()
        ),
        (json!(2), json!(2))
    );
    assert_eq!(
        state["first_visible_at_ms"],
        published["first_visible_at_ms"]
    );
    assert_eq!(
        state["generation"]["entries"][0]["audio"],
        json!({
            "release_id": release,
            "release_digest": activated["release"]["digest"],
            "basis": "exact"
        })
    );
    assert_eq!(
        events(&f).await,
        [(1, "publish".to_owned()), (2, "activate_audio".to_owned())]
    );

    // A replayed activation is the same receipt and writes no second generation.
    let replay: Value = h
        .json("POST", &path, Some(ALICE), Some(request))
        .await
        .success(StatusCode::CREATED);
    assert_eq!(replay, activated);
    assert_eq!(events(&f).await.len(), 2);

    // Withdrawal stops the audio through Media's live gate; the generation keeps the fact and
    // no metadata event is written for it.
    h.json(
        "POST",
        &format!("/content-units/{unit}/publication/withdraw"),
        Some(ALICE),
        Some(json!({
            "operation_id": operation(),
            "release_id": release,
            "expected_pointer_version": 1,
            "reason": "Synthetic withdrawal"
        })),
    )
    .await
    .success(StatusCode::OK);
    let after: Value = h
        .json("GET", &metadata_path(&work), Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(after, state);
    assert_eq!(events(&f).await.len(), 2);
}
