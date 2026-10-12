//! Temporary public paths remain aliases of the owning contexts. These checks exercise the
//! legacy library entrypoints without duplicating their domain or persistence tests.
use std::convert::identity;

use cantos_api::{contexts, postgres, production, revisions, speech_fingerprint, tts, worker};
use cantos_contracts::ResolvedSynthesisInput;
use serde_json::{json, Value};

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
fn production_and_provider_catalog_entrypoints_remain_compatible() {
    assert_eq!(
        production::production_catalog(),
        contexts::production::production_catalog()
    );
    assert_eq!(
        tts::local_contract::capability(),
        contexts::production::catalog::local::capability()
    );
    assert_eq!(
        tts::google_contract::ADAPTER_VERSION,
        contexts::production::catalog::google::ADAPTER_VERSION
    );
}

#[test]
fn synthesis_entrypoints_preserve_fingerprints_and_receipt_decoding() {
    let vectors: Value = serde_json::from_str(include_str!(
        "../../../packages/contracts/fixtures/clip-reuse/v1/speech-vectors.json"
    ))
    .unwrap();
    let vector = &vectors["vectors"][0];
    let line: ResolvedSynthesisInput = serde_json::from_value(vector["input"].clone()).unwrap();
    let legacy = speech_fingerprint::SpeechInput::from_resolved(&line).unwrap();
    let current = contexts::synthesis::fingerprint::SpeechInput::from_resolved(&line).unwrap();
    assert_eq!(legacy, current);
    assert_eq!(
        legacy.fingerprint().as_str(),
        vector["expected"]["fingerprint"].as_str().unwrap()
    );

    let bytes = serde_json::to_vec(&json!({
        "version": tts::RECEIPT_VERSION,
        "evidence": "synthetic_fixture",
        "attempt_id": "compatibility-attempt",
        "operation_id": "compatibility-operation",
        "provider_request_id": null,
        "idempotency_key": null,
        "input_digest": "synthetic-unapproved-input",
        "dialogue_id": line.dialogue_id,
        "provider": {
            "provider_id": line.provider_id,
            "model_id": line.model_id,
            "voice_id": line.voice_id,
            "adapter_version": line.adapter_version
        },
        "reconciliation": {
            "lookup_by_operation_id": {"state": "unavailable"},
            "lookup_by_provider_request_id": {"state": "unavailable"},
            "same_key_idempotency": {"state": "unavailable"}
        },
        "usage": {"status": "unknown"},
        "charge": {"status": "unknown"},
        "outcome": {"status": "failure", "class": "timeout", "delivery": "may_have_executed"}
    }))
    .unwrap();
    let receipt: tts::TtsReceipt = contexts::synthesis::receipt::decode_receipt(&bytes).unwrap();
    assert_eq!(tts::decode_receipt(&bytes).unwrap(), receipt);
    assert_eq!(receipt.charge, tts::ChargeObservation::Unknown {});
}

#[test]
fn shell_entrypoints_reexport_identical_types() {
    // Assigning identity functions proves these paths name the very same types, without
    // opening a database connection or starting a worker.
    let _: fn(postgres::Store) -> cantos_api::platform::postgres::Store = identity;
    let _: fn(postgres::durable::JobLease) -> contexts::synthesis::JobLease = identity;
    let _: fn(postgres::durable::AttemptWork) -> contexts::synthesis::AttemptWork = identity;
    let _: fn(worker::WorkerConfig) -> contexts::synthesis::worker::WorkerConfig = identity;
    let _: fn(tts::TtsReceipt) -> contexts::synthesis::receipt::TtsReceipt = identity;
    let _: fn(
        tts::audio::InspectedTtsAudio,
    ) -> contexts::synthesis::media::audio::InspectedTtsAudio = identity;
}
