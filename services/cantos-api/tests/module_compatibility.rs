//! Compatibility aliases for the retained editorial, planning and media inspection contracts.
use cantos_api::{contexts, postgres, production, revisions};
use std::convert::identity;

#[test]
fn revision_entrypoints_share_the_same_retry_policy_and_types() {
    let script = cantos_api::script_ir::read_script(include_bytes!(
        "../../../packages/contracts/fixtures/script-ir/0.1.0/accept/two-scenes.json"
    ))
    .unwrap();
    let export = script.export_bytes();
    let prior = || {
        Some(revisions::PriorOperation {
            revision: 1,
            expected_revision: 0,
            export: &export,
        })
    };
    let legacy = revisions::decide_save(3, 0, &export, prior());
    let current = contexts::editorial::decide_save(3, 0, &export, prior());
    assert_eq!(legacy, revisions::SaveDecision::Replay { revision: 1 });
    assert_eq!(legacy, current);
    assert_eq!(
        revisions::export_digest(&export),
        contexts::editorial::export_digest(&export)
    );
}

#[test]
fn production_catalog_entrypoint_is_compatible() {
    assert_eq!(
        production::production_catalog(),
        contexts::production::production_catalog()
    );
}

#[test]
fn media_inspection_is_not_coupled_to_synthesis() {
    let bytes = b"not a wave file";
    let canonical = contexts::media::domain::wav_inspection::inspect_wav(bytes);
    let alias = cantos_api::wav_inspection::inspect_wav(bytes);
    assert_eq!(alias, canonical);
}

#[test]
fn postgres_alias_uses_the_same_store() {
    let _: fn(postgres::Store) -> cantos_api::platform::postgres::Store = identity;
    let _: fn(postgres::Store) -> cantos_api::application::Store = identity;
    let _: fn(postgres::StoreError) -> cantos_api::application::StoreError = identity;
}

#[test]
fn root_http_alias_keeps_the_canonical_router_and_state_types() {
    use cantos_api::{adapters::inbound::http as canonical, http};
    let _: fn(http::AppState) -> canonical::AppState = identity;
    let _: fn(http::AppState) -> axum::Router = canonical::api_router;
    let _: fn(canonical::AppState) -> axum::Router = http::api_router;
}
