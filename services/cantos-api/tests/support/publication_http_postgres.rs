//! Real PostgreSQL/application/Router/private-byte integration with named fake public delivery.
//! Ignored and run only by the disposable-cluster xtask. No CDN/live-delivery claim.
use super::*;
use cantos_api::contexts::media::{
    domain::publication::{DeliveryEvidence, DeliveryObservation},
    PublicationDelivery, PublicationDeliveryFuture, PublicationObject,
};
use cantos_api::{
    contexts::media::PublicationService, interfaces::http::router_with_publication_delivery,
};
use std::sync::Arc;
struct VerifiedDeliveryFake;
impl PublicationDelivery for VerifiedDeliveryFake {
    fn observe(&self, object: PublicationObject) -> PublicationDeliveryFuture {
        Box::pin(async move {
            Ok(DeliveryObservation::Verified(
                DeliveryEvidence::new(
                    format!("publication-{}", object.rendition_id),
                    object.sha256,
                    object.byte_length,
                    0,
                    i64::MAX,
                )
                .unwrap(),
            ))
        })
    }
}
struct PublishedFixture {
    a: AudioHarness,
    submission: Value,
    rendition: Value,
    candidate: Value,
    clearance: Value,
    approval: Value,
}
impl PublishedFixture {
    async fn new(delivery: bool) -> Self {
        let mut a = AudioHarness::new(true, "opus-mono-24-v1").await;
        a.h.admin.batch_execute("GRANT SELECT,INSERT ON audio_distribution_clearances,audio_publication_approvals,audio_releases,audio_release_withdrawals,audio_publication_operations,audio_publication_events,audio_publication_pointers TO cantos_app; GRANT UPDATE(active_release_id,version) ON audio_publication_pointers TO cantos_app;").await.unwrap();
        if delivery {
            a.h.app = router_with_publication_delivery(
                AppState {
                    store: a.h.store.clone(),
                    web_origin: ORIGIN.into(),
                },
                "unused-public-root",
                Arc::new(VerifiedDeliveryFake),
            );
        }
        let bytes = a.root.encode("24k");
        let submission = a.request(&bytes).await;
        a.prepare(submission.clone()).await;
        let id = submission["id"].as_str().unwrap();
        let rendition = upload(a.h.app.clone(), id, UPLOAD, bytes)
            .await
            .success(StatusCode::OK);
        for kind in ["qc", "rights", "editorial"] {
            a.h.json(
                "POST",
                &format!("{}/{id}/reviews", a.path()),
                Some(ALICE),
                Some(review_request(&rendition, kind, "approved")),
            )
            .await
            .success(StatusCode::CREATED);
        }
        let candidate =
            a.h.json(
                "POST",
                &format!("{}/{id}/release-candidates", a.path()),
                Some(ALICE),
                Some(json!({"operation_id":operation(),"rendition_digest":rendition["digest"]})),
            )
            .await
            .success(StatusCode::CREATED)["candidate"]
                .clone();
        let base = format!(
            "/content-units/{}/release-candidates/{}",
            a.unit,
            candidate["id"].as_str().unwrap()
        );
        let clearance=a.h.json("POST",&format!("{base}/distribution-rights"),Some(ALICE),Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"decision":"cleared","rights_refs":["catalog-rights"],"valid_until":now(&a.h).await+3_600_000,"evidence_refs":["catalog-rights"],"notes":"Named synthetic owner's distribution decision; no legal proof claim."}))).await.success(StatusCode::CREATED);
        let approval=a.h.json("POST",&format!("{base}/publication-approvals"),Some(ALICE),Some(json!({"operation_id":operation(),"candidate_digest":candidate["digest"],"clearance_digest":clearance["digest"],"decision":"approved","notes":"Approve exact synthetic candidate, measured envelope and fixture-only QC."}))).await.success(StatusCode::CREATED);
        Self {
            a,
            submission,
            rendition,
            candidate,
            clearance,
            approval,
        }
    }
    fn base(&self) -> String {
        format!(
            "/content-units/{}/release-candidates/{}",
            self.a.unit,
            self.candidate["id"].as_str().unwrap()
        )
    }
    fn publish(&self, version: u64, active: Value) -> Value {
        json!({"operation_id":operation(),"candidate_digest":self.candidate["digest"],"expected_pointer_version":version,"expected_active_release_id":active})
    }
    async fn counts(&self) -> Value {
        let text:String=self.a.h.admin.query_one("SELECT json_build_object('releases',(SELECT count(*) FROM audio_releases),'pointers',(SELECT count(*) FROM audio_publication_pointers),'operations',(SELECT count(*) FROM audio_publication_operations),'events',(SELECT count(*) FROM audio_publication_events),'withdrawals',(SELECT count(*) FROM audio_release_withdrawals))::text",&[]).await.unwrap().get(0);
        serde_json::from_str(&text).unwrap()
    }
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_flow_replays_restarts_and_republishes_immutable_history() {
    let f = PublishedFixture::new(true).await;
    let h = &f.a.h;
    let ready = h
        .json("GET", &format!("{}/readiness", f.base()), Some(ALICE), None)
        .await
        .success(StatusCode::OK);
    assert_eq!(ready["blockers"], json!([]));
    let request = f.publish(0, Value::Null);
    let published = h
        .json(
            "POST",
            &format!("{}/publish", f.base()),
            Some(ALICE),
            Some(request.clone()),
        )
        .await
        .success(StatusCode::CREATED);
    assert_eq!(published["pointer_version"], 1);
    assert_eq!(
        published["release"]["manifest"]["binding"],
        f.rendition["binding"]
    );
    assert_eq!(
        published["release"]["manifest"]["media"],
        f.rendition["verified_media"]
    );
    assert_eq!(
        h.json(
            "POST",
            &format!("{}/publish", f.base()),
            Some(ALICE),
            Some(request)
        )
        .await
        .success(StatusCode::CREATED),
        published
    );
    assert_eq!(
        f.counts().await,
        json!({"releases":1,"pointers":1,"operations":1,"events":1,"withdrawals":0})
    );
    let mut cfg = h.config.clone();
    cfg.user("cantos_app");
    let restarted = PublicationService::new(
        Store::new(cfg)
            .unwrap()
            .with_audio_storage(&f.a.root.0)
            .unwrap(),
    )
    .with_publication_delivery(Arc::new(VerifiedDeliveryFake));
    assert_eq!(
        serde_json::to_value(
            restarted
                .active_audio_release(ALICE, &f.a.unit)
                .await
                .unwrap()
        )
        .unwrap(),
        published
    );
    let second = h
        .json(
            "POST",
            &format!("{}/publish", f.base()),
            Some(ALICE),
            Some(f.publish(1, published["release"]["id"].clone())),
        )
        .await
        .success(StatusCode::CREATED);
    assert_eq!(second["pointer_version"], 2);
    assert_ne!(second["release"]["id"], published["release"]["id"]);
    assert_eq!(second["release"]["supersedes"], published["release"]["id"]);
    assert_eq!(
        second["release"]["manifest"],
        published["release"]["manifest"]
    );
    let old: Vec<u8> = h
        .admin
        .query_one(
            "SELECT snapshot FROM audio_releases WHERE id=$1",
            &[&published["release"]["id"].as_str().unwrap()],
        )
        .await
        .unwrap()
        .get(0);
    assert_eq!(
        serde_json::from_slice::<Value>(&old).unwrap(),
        published["release"]
    );
    let result = h
        .admin
        .execute(
            "UPDATE audio_releases SET digest=digest WHERE id=$1",
            &[&published["release"]["id"].as_str().unwrap()],
        )
        .await;
    assert!(result.is_err());
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_two_publishers_have_one_winner_and_duplicate_requests_one_receipt() {
    let f = PublishedFixture::new(true).await;
    let path = format!("{}/publish", f.base());
    let a = f.publish(0, Value::Null);
    let b = f.publish(0, Value::Null);
    let (one, two) = tokio::join!(
        f.a.h.json("POST", &path, Some(ALICE), Some(a)),
        f.a.h.json("POST", &path, Some(ALICE), Some(b))
    );
    assert_eq!(
        [one.status, two.status]
            .iter()
            .filter(|s| **s == StatusCode::CREATED)
            .count(),
        1
    );
    assert_eq!(
        [one.status, two.status]
            .iter()
            .filter(|s| **s == StatusCode::CONFLICT)
            .count(),
        1
    );
    assert_eq!(f.counts().await["releases"], 1);
    let g = PublishedFixture::new(true).await;
    let path = format!("{}/publish", g.base());
    let request = g.publish(0, Value::Null);
    let (one, two) = tokio::join!(
        g.a.h
            .json("POST", &path, Some(ALICE), Some(request.clone())),
        g.a.h.json("POST", &path, Some(ALICE), Some(request))
    );
    assert_eq!(
        one.success(StatusCode::CREATED),
        two.success(StatusCode::CREATED)
    );
    assert_eq!(g.counts().await["events"], 1);
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_stale_qc_rights_revocation_missing_bytes_and_unauthorized_requests_fail_closed(
) {
    let f = PublishedFixture::new(true).await;
    let before = f.counts().await;
    f.a.h
        .json(
            "POST",
            &format!("{}/publish", f.base()),
            Some(BOB),
            Some(f.publish(0, Value::Null)),
        )
        .await
        .error(StatusCode::NOT_FOUND, "not_found");
    assert_eq!(f.counts().await, before);
    let id = f.submission["id"].as_str().unwrap();
    f.a.h
        .json(
            "POST",
            &format!("{}/{id}/reviews", f.a.path()),
            Some(ALICE),
            Some(review_request(&f.rendition, "qc", "withdrawn")),
        )
        .await
        .success(StatusCode::CREATED);
    let blocked =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(0, Value::Null)),
            )
            .await
            .error(StatusCode::CONFLICT, "production_blocked");
    assert!(blocked["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["rule"] == "qc_not_approved"));
    assert_eq!(f.counts().await, before);
    let g = PublishedFixture::new(true).await;
    let mut revoke = g.clearance["request"].clone();
    revoke["operation_id"] = json!(operation());
    revoke["decision"] = json!("revoked");
    g.a.h
        .json(
            "POST",
            &format!("{}/distribution-rights", g.base()),
            Some(ALICE),
            Some(revoke),
        )
        .await
        .success(StatusCode::CREATED);
    let before = g.counts().await;
    g.a.h
        .json(
            "POST",
            &format!("{}/publish", g.base()),
            Some(ALICE),
            Some(g.publish(0, Value::Null)),
        )
        .await
        .error(StatusCode::CONFLICT, "production_blocked");
    assert_eq!(g.counts().await, before);
    let missing = PublishedFixture::new(true).await;
    let sha = missing.rendition["verified_media"]["sha256"]
        .as_str()
        .unwrap();
    fs::remove_file(missing.a.root.0.join("objects").join(sha)).unwrap();
    let before = missing.counts().await;
    missing
        .a
        .h
        .json(
            "POST",
            &format!("{}/publish", missing.base()),
            Some(ALICE),
            Some(missing.publish(0, Value::Null)),
        )
        .await
        .error(StatusCode::CONFLICT, "production_blocked");
    assert_eq!(missing.counts().await, before);
}
#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_default_delivery_is_deferred_and_withdrawal_does_not_resurrect_prior_release()
{
    let unavailable = PublishedFixture::new(false).await;
    let before = unavailable.counts().await;
    let response = unavailable
        .a
        .h
        .json(
            "POST",
            &format!("{}/publish", unavailable.base()),
            Some(ALICE),
            Some(unavailable.publish(0, Value::Null)),
        )
        .await
        .error(StatusCode::CONFLICT, "production_blocked");
    assert!(response["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["rule"] == "delivery_unavailable"));
    assert_eq!(unavailable.counts().await, before);
    let f = PublishedFixture::new(true).await;
    let first =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(0, Value::Null)),
            )
            .await
            .success(StatusCode::CREATED);
    let second =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(1, first["release"]["id"].clone())),
            )
            .await
            .success(StatusCode::CREATED);
    let path = format!("/content-units/{}/publication/withdraw", f.a.unit);
    let request = json!({"operation_id":operation(),"release_id":second["release"]["id"],"expected_pointer_version":2,"reason":"Synthetic takedown"});
    let receipt =
        f.a.h
            .json("POST", &path, Some(ALICE), Some(request.clone()))
            .await
            .success(StatusCode::OK);
    assert_eq!(receipt["pointer_version"], 3);
    assert_eq!(
        f.a.h
            .json("POST", &path, Some(ALICE), Some(request))
            .await
            .success(StatusCode::OK),
        receipt
    );
    let active =
        f.a.h
            .json(
                "GET",
                &format!("/content-units/{}/publication/active", f.a.unit),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
    assert_eq!(active["release"], Value::Null);
    assert_eq!(f.counts().await["releases"], 2);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_new_approval_is_blocked_after_rights_revocation_and_published_release_stops_resolution(
) {
    let f = PublishedFixture::new(true).await;
    let published =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(0, Value::Null)),
            )
            .await
            .success(StatusCode::CREATED);
    let mut request = f.clearance["request"].clone();
    request["operation_id"] = json!(operation());
    request["decision"] = json!("revoked");
    let revoked =
        f.a.h
            .json(
                "POST",
                &format!("{}/distribution-rights", f.base()),
                Some(ALICE),
                Some(request),
            )
            .await
            .success(StatusCode::CREATED);
    let mut approve = f.approval["request"].clone();
    approve["operation_id"] = json!(operation());
    approve["clearance_digest"] = revoked["digest"].clone();
    let response =
        f.a.h
            .json(
                "POST",
                &format!("{}/publication-approvals", f.base()),
                Some(ALICE),
                Some(approve),
            )
            .await
            .error(StatusCode::CONFLICT, "production_blocked");
    assert!(response["issues"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["rule"] == "rights_revoked"));
    let active =
        f.a.h
            .json(
                "GET",
                &format!("/content-units/{}/publication/active", f.a.unit),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
    assert_eq!(active["pointer_version"], published["pointer_version"]);
    assert_eq!(active["release"], Value::Null);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_failed_replacement_preserves_prior_active_release_after_qc_changes() {
    let f = PublishedFixture::new(true).await;
    let published =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(0, Value::Null)),
            )
            .await
            .success(StatusCode::CREATED);
    let before = f.counts().await;
    let id = f.submission["id"].as_str().unwrap();
    f.a.h
        .json(
            "POST",
            &format!("{}/{id}/reviews", f.a.path()),
            Some(ALICE),
            Some(review_request(&f.rendition, "qc", "withdrawn")),
        )
        .await
        .success(StatusCode::CREATED);
    f.a.h
        .json(
            "POST",
            &format!("{}/publish", f.base()),
            Some(ALICE),
            Some(f.publish(1, published["release"]["id"].clone())),
        )
        .await
        .error(StatusCode::CONFLICT, "production_blocked");
    assert_eq!(f.counts().await, before);
    let active =
        f.a.h
            .json(
                "GET",
                &format!("/content-units/{}/publication/active", f.a.unit),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
    assert_eq!(active, published);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_withdrawing_superseded_release_preserves_new_active_pointer() {
    let f = PublishedFixture::new(true).await;
    let first =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(0, Value::Null)),
            )
            .await
            .success(StatusCode::CREATED);
    let second =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(f.publish(1, first["release"]["id"].clone())),
            )
            .await
            .success(StatusCode::CREATED);
    let request = json!({"operation_id":operation(),"release_id":first["release"]["id"],"expected_pointer_version":2,"reason":"Withdraw superseded fixture only"});
    let receipt =
        f.a.h
            .json(
                "POST",
                &format!("/content-units/{}/publication/withdraw", f.a.unit),
                Some(ALICE),
                Some(request),
            )
            .await
            .success(StatusCode::OK);
    assert_eq!(receipt["pointer_version"], 2);
    assert_eq!(receipt["withdrawn_release_id"], first["release"]["id"]);
    let active =
        f.a.h
            .json(
                "GET",
                &format!("/content-units/{}/publication/active", f.a.unit),
                Some(ALICE),
                None,
            )
            .await
            .success(StatusCode::OK);
    assert_eq!(active, second);
    assert_eq!(f.counts().await["releases"], 2);
    assert_eq!(f.counts().await["withdrawals"], 1);
}

#[tokio::test]
#[ignore = "disposable PostgreSQL and trusted media tools required"]
async fn publication_sql_failure_rolls_back_release_pointer_event_and_receipt_then_retries() {
    let f = PublishedFixture::new(true).await;
    // A named fixture-only SQL failure at the final event write establishes rollback
    // across all earlier writes. It does not simulate a power loss or CDN side effect.
    f.a.h.admin.batch_execute("CREATE FUNCTION fixture_reject_publication_event() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'named publication rollback fixture'; END $$; CREATE TRIGGER fixture_failure BEFORE INSERT ON audio_publication_events FOR EACH ROW EXECUTE FUNCTION fixture_reject_publication_event();").await.unwrap();
    let before = f.counts().await;
    let request = f.publish(0, Value::Null);
    let response =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(request.clone()),
            )
            .await;
    assert_eq!(response.status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(f.counts().await, before);
    f.a.h.admin.batch_execute("DROP TRIGGER fixture_failure ON audio_publication_events; DROP FUNCTION fixture_reject_publication_event();").await.unwrap();
    let published =
        f.a.h
            .json(
                "POST",
                &format!("{}/publish", f.base()),
                Some(ALICE),
                Some(request),
            )
            .await
            .success(StatusCode::CREATED);
    assert_eq!(published["pointer_version"], 1);
    assert_eq!(
        f.counts().await,
        json!({"releases":1,"pointers":1,"operations":1,"events":1,"withdrawals":0})
    );
}
