//! Synthetic attestations and isolated databases only; no real rights are asserted by tests.
use super::ServerFixture;
use crate::contexts::catalog::domain::{
    metadata_disclosure::FICTIONAL_SAMPLE_SOURCE_SHA256,
    sample_seed_request::{SampleSeedAction, SampleSeedRequest},
};
use crate::platform::operator;
use axum::{
    body::Body,
    http::{Request, StatusCode},
};
use http_body_util::BodyExt;
use tower::ServiceExt;

fn request(action: SampleSeedAction, id: &str) -> SampleSeedRequest {
    SampleSeedRequest {
        operation_id: id.into(),
        action,
        source_sha256: FICTIONAL_SAMPLE_SOURCE_SHA256.into(),
        operator_reference: "synthetic operator".into(),
        evidence_reference: "synthetic evidence".into(),
        rights_statement: "synthetic statement for an isolated test".into(),
        confirm_source: true,
        confirm_metadata_rights: true,
    }
}

#[tokio::test]
async fn offline_operator_seed_is_atomic_idempotent_non_login_and_revocable() {
    let input = include_bytes!("../../tests/fixtures/fictional-sample-metadata.json").to_vec();
    let fixture = ServerFixture::new().await;
    let publish = request(
        SampleSeedAction::Publish,
        "00000000-0000-4000-8000-000000000001",
    );
    let mut missing = request(
        SampleSeedAction::Publish,
        "00000000-0000-4000-8000-000000000002",
    );
    missing.confirm_metadata_rights = false;
    assert!(
        crate::contexts::catalog::seed_sample(fixture.store(), &missing, Some(&input))
            .await
            .is_err()
    );
    assert_eq!(fixture.count("SELECT count(*) FROM actors", &[]).await, 0);
    assert!(crate::contexts::catalog::seed_sample(
        fixture.store(),
        &publish,
        Some(b"unrelated source")
    )
    .await
    .is_err());
    assert_eq!(fixture.count("SELECT count(*) FROM actors", &[]).await, 0);
    crate::contexts::catalog::seed_sample(fixture.store(), &publish, Some(&input))
        .await
        .unwrap();
    crate::contexts::catalog::seed_sample(fixture.store(), &publish, Some(&input))
        .await
        .unwrap();
    let mut changed = request(
        SampleSeedAction::Publish,
        "00000000-0000-4000-8000-000000000001",
    );
    changed.rights_statement = "different claim".into();
    assert!(
        crate::contexts::catalog::seed_sample(fixture.store(), &changed, Some(&input))
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_works", &[])
            .await,
        6
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_units", &[])
            .await,
        12
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_fictional_metadata_disclosures",
                &[]
            )
            .await,
        18
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_fictional_seed_actions", &[])
            .await,
        1
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM identity_sessions", &[])
            .await,
        0
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM development_sessions", &[])
            .await,
        0
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM actor_external_identities", &[])
            .await,
        0
    );
    assert!(
        operator::issue_token(fixture.store(), "cantos_fictional_seed_v2", 1)
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM development_sessions", &[])
            .await,
        0
    );
    let actor =
        crate::platform::identity::domain::ActorId::new("cantos_fictional_seed_v2").unwrap();
    let external =
        crate::platform::identity::domain::ExternalIdentityKey::new("issuer", "subject").unwrap();
    assert!(operator::link_identity(fixture.store(), &actor, &external)
        .await
        .is_err());

    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let old = app
        .clone()
        .oneshot(
            Request::get("/api/v1/theatre/works?presentation=2")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let old: serde_json::Value =
        serde_json::from_slice(&old.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(old["items"].as_array().unwrap().is_empty());
    let response = app
        .clone()
        .oneshot(
            Request::get("/api/v1/theatre/works?presentation=3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let page: crate::contract::theatre::discovery::MetadataWorkPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page.items.len(), 6);
    for work in page.items {
        let response = app
            .clone()
            .oneshot(
                Request::get(format!(
                    "/api/v1/theatre/works/{}/units?presentation=3",
                    work.work_id
                ))
                .body(Body::empty())
                .unwrap(),
            )
            .await
            .unwrap();
        let units: crate::contract::theatre::discovery::MetadataUnitPage =
            serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes())
                .unwrap();
        assert_eq!(units.items.len(), 2);
        assert!(units.items.iter().all(|unit| unit.playback.is_none()));
    }
    let withdrawal = request(
        SampleSeedAction::Withdraw,
        "00000000-0000-4000-8000-000000000003",
    );
    crate::contexts::catalog::seed_sample(fixture.store(), &withdrawal, None)
        .await
        .unwrap();
    // A delayed retry of the original publication cannot reverse the newer withdrawal.
    crate::contexts::catalog::seed_sample(fixture.store(), &publish, Some(&input))
        .await
        .unwrap();
    crate::contexts::catalog::seed_sample(fixture.store(), &withdrawal, None)
        .await
        .unwrap();
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_fictional_seed_actions", &[])
            .await,
        2
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_fictional_metadata_disclosures",
                &[]
            )
            .await,
        36
    );
    let response = app
        .oneshot(
            Request::get("/api/v1/theatre/works?presentation=3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page: crate::contract::theatre::discovery::MetadataWorkPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert!(page.items.is_empty());
}

#[tokio::test]
async fn occupied_sample_work_rolls_back_the_whole_operator_transaction() {
    use crate::contexts::catalog::{
        domain::{Visibility, WorkDetails, WorkId, WorkKind},
        CatalogCommand,
    };
    let input = include_bytes!("../../tests/fixtures/fictional-sample-metadata.json").to_vec();
    let fixture = ServerFixture::new().await;
    let token = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fixture.studio_caller("alice", token).await;
    fixture
        .store()
        .catalog_execute(
            token,
            "00000000-0000-4000-8000-000000000011",
            CatalogCommand::CreateWork {
                id: WorkId::parse("dbe1f751-0e5f-47d2-8910-62e2eafb2dcf").unwrap(),
                kind: WorkKind::Story,
                details: WorkDetails {
                    title: "Alice's private work".into(),
                    synopsis: "".into(),
                    language: "vi-VN".into(),
                    visibility: Visibility::Private,
                    rights_refs: vec![],
                },
            },
        )
        .await
        .unwrap();
    let publish = request(
        SampleSeedAction::Publish,
        "00000000-0000-4000-8000-000000000001",
    );
    assert!(
        crate::contexts::catalog::seed_sample(fixture.store(), &publish, Some(&input))
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_works WHERE owner_id='alice'",
                &[]
            )
            .await,
        1
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM actors WHERE id='cantos_fictional_seed_v2'",
                &[]
            )
            .await,
        0
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_fictional_seed_actions", &[])
            .await,
        0
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_fictional_metadata_disclosures",
                &[]
            )
            .await,
        0
    );
}

#[tokio::test]
async fn occupied_actor_id_is_not_adopted() {
    let input = include_bytes!("../../tests/fixtures/fictional-sample-metadata.json").to_vec();
    let fixture = ServerFixture::new().await;
    fixture.seed_actors(&["cantos_fictional_seed_v2"]).await;
    let publish = request(
        SampleSeedAction::Publish,
        "00000000-0000-4000-8000-000000000001",
    );
    assert!(
        crate::contexts::catalog::seed_sample(fixture.store(), &publish, Some(&input))
            .await
            .is_err()
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM fictional_seed_principals", &[])
            .await,
        0
    );
    assert_eq!(
        fixture
            .count("SELECT count(*) FROM catalog_works", &[])
            .await,
        0
    );
}

#[tokio::test]
async fn fictional_discovery_excludes_other_private_works() {
    use crate::contexts::catalog::{
        domain::{Visibility, WorkDetails, WorkId, WorkKind},
        CatalogCommand,
    };
    let fixture = ServerFixture::new().await;
    let token = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    fixture.studio_caller("alice", token).await;
    fixture
        .store()
        .catalog_execute(
            token,
            "00000000-0000-4000-8000-000000000011",
            CatalogCommand::CreateWork {
                id: WorkId::parse("00000000-0000-4000-8000-000000000022").unwrap(),
                kind: WorkKind::Story,
                details: WorkDetails {
                    title: "private".into(),
                    synopsis: "".into(),
                    language: "vi-VN".into(),
                    visibility: Visibility::Private,
                    rights_refs: vec![],
                },
            },
        )
        .await
        .unwrap();
    let bytes = include_bytes!("../../tests/fixtures/fictional-sample-metadata.json");
    crate::contexts::catalog::seed_sample(
        fixture.store(),
        &request(
            SampleSeedAction::Publish,
            "00000000-0000-4000-8000-000000000001",
        ),
        Some(bytes),
    )
    .await
    .unwrap();
    let app =
        crate::adapters::inbound::http::api_router(crate::adapters::inbound::http::AppState {
            store: fixture.store().clone(),
            web_origin: "https://studio.example.test".into(),
        });
    let response = app
        .oneshot(
            Request::get("/api/v1/theatre/works?presentation=3")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let page: crate::contract::theatre::discovery::MetadataWorkPage =
        serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap();
    assert_eq!(page.items.len(), 6);
    assert!(!page
        .items
        .iter()
        .any(|item| item.work_id == "00000000-0000-4000-8000-000000000022"));
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_works WHERE owner_id='alice'",
                &[]
            )
            .await,
        1
    );
    assert_eq!(
        fixture
            .count(
                "SELECT count(*) FROM catalog_sample_metadata_disclosures",
                &[]
            )
            .await,
        0
    );
}
