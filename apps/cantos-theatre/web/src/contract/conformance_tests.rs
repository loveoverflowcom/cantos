//! Theatre Web's own representation against the Server-owned canonical fixtures and schemas:
//! each fixture this client sends or receives decodes into its DTO and re-encodes to the same
//! JSON value. The Server validates the fixtures against the schemas in its own
//! `contract_conformance` tests; this proves only that the client's copy has not drifted.
use super::{discovery, identity_auth, library, ApiError};
use serde::{de::DeserializeOwned, Serialize};
use serde_json::{json, Value};

macro_rules! server {
    ($path:literal) => {
        include_str!(concat!(
            "../../../../../services/cantos-server/contracts/",
            $path
        ))
    };
}

fn same<T: DeserializeOwned + Serialize>(json: &str) {
    let expected: Value = serde_json::from_str(json).unwrap();
    let decoded: T = serde_json::from_value(expected.clone()).unwrap();
    assert_eq!(serde_json::to_value(decoded).unwrap(), expected);
}

#[test]
fn discovery_fixtures_round_trip_through_the_theatre_representation() {
    same::<discovery::CoveredWorkPage>(server!(
        "theatre/discovery/v1/fixtures/cover/covered_works.json"
    ));
    same::<discovery::PresentedUnitPage>(server!(
        "theatre/discovery/v1/fixtures/presented_units.json"
    ));
    same::<discovery::PublicationManifest>(server!("theatre/discovery/v1/fixtures/manifest.json"));
    for json in [
        server!("theatre/discovery/v1/fixtures/not_found.json"),
        server!("theatre/discovery/v1/fixtures/invalid_request.json"),
        server!("theatre/discovery/v1/fixtures/invalid_cursor.json"),
        server!("theatre/discovery/v1/fixtures/unavailable.json"),
        server!("theatre/discovery/v1/fixtures/rendition_unavailable.json"),
    ] {
        same::<discovery::ListenerError>(json);
    }
}

#[test]
fn library_fixtures_round_trip_through_the_theatre_representation() {
    same::<library::ProgressWrite>(server!("theatre/library/v1/fixtures/progress_write.json"));
    same::<library::SavedWrite>(server!("theatre/library/v1/fixtures/saved_write.json"));
    same::<library::HistoryStart>(server!("theatre/library/v1/fixtures/history_start.json"));
    same::<library::HistoryClear>(server!("theatre/library/v1/fixtures/history_clear.json"));
    same::<library::MutationState>(server!("theatre/library/v1/fixtures/progress_read.json"));
    for json in [
        server!("theatre/library/v1/fixtures/accepted.json"),
        server!("theatre/library/v1/fixtures/progress_conflict.json"),
        server!("theatre/library/v1/fixtures/epoch_conflict.json"),
    ] {
        same::<library::Receipt>(json);
    }
    for json in [
        server!("theatre/library/v1/fixtures/invalid_request.json"),
        server!("theatre/library/v1/fixtures/invalid_position.json"),
        server!("theatre/library/v1/fixtures/invalid_cursor.json"),
        server!("theatre/library/v1/fixtures/unauthenticated.json"),
        server!("theatre/library/v1/fixtures/not_found.json"),
        server!("theatre/library/v1/fixtures/idempotency_key_reused.json"),
        server!("theatre/library/v1/fixtures/stale_device_sequence.json"),
        server!("theatre/library/v1/fixtures/revision_exhausted.json"),
        server!("theatre/library/v1/fixtures/unavailable.json"),
        server!("theatre/library/v1/fixtures/corrupt_state.json"),
    ] {
        same::<library::LibraryError>(json);
    }
    for json in [
        server!("theatre/library/v1/fixtures/presentation/presented_continue.json"),
        server!("theatre/library/v1/fixtures/presentation/presented_continue_unavailable.json"),
    ] {
        same::<library::PresentedContinuePage>(json);
    }
    for json in [
        server!("theatre/library/v1/fixtures/presentation/presented_saved.json"),
        server!("theatre/library/v1/fixtures/presentation/presented_saved_unavailable.json"),
    ] {
        same::<library::PresentedSavedPage>(json);
    }
    for json in [
        server!("theatre/library/v1/fixtures/presentation/presented_history.json"),
        server!("theatre/library/v1/fixtures/presentation/presented_history_unavailable.json"),
    ] {
        same::<library::PresentedHistoryPage>(json);
    }
}

#[test]
fn the_shared_error_envelope_round_trips() {
    same::<ApiError>(server!("studio/editorial/v1/fixtures/stale-revision.json"));
}

/// Identity auth v1 has no fixture files: compare the signup responses this client decodes with
/// the Server-owned schema's property and required sets, then round-trip a representative value.
#[test]
fn signup_responses_match_the_identity_auth_schema() {
    let schema: Value =
        serde_json::from_str(server!("auth/identity-auth/v1/identity-auth.schema.json")).unwrap();
    let root = |title: &str| -> Value {
        schema["oneOf"]
            .as_array()
            .unwrap()
            .iter()
            .find(|root| root["title"] == title)
            .cloned()
            .unwrap()
    };
    for (title, value) in [
        (
            "SignupAccepted",
            json!({"accepted": true, "proof_ttl_seconds": 900}),
        ),
        ("SignupCompleted", json!({"status": "replayed"})),
    ] {
        let root = root(title);
        let mut properties: Vec<&String> = root["properties"].as_object().unwrap().keys().collect();
        let mut keys: Vec<&String> = value.as_object().unwrap().keys().collect();
        properties.sort();
        keys.sort();
        assert_eq!(keys, properties, "{title}");
        assert_eq!(
            root["required"].as_array().unwrap().len(),
            keys.len(),
            "{title}"
        );
        match title {
            "SignupAccepted" => same::<identity_auth::SignupAccepted>(&value.to_string()),
            _ => same::<identity_auth::SignupCompleted>(&value.to_string()),
        }
    }
}
