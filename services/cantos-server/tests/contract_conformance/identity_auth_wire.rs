//! Email/password identity v1 DTOs: strict inputs, no secret in a Debug rendering.
use cantos_server::contract::auth::identity_auth::{
    Login, SignupAccepted, SignupComplete, SignupCompleted, SignupStart, SignupStatus,
};
use serde_json::json;

#[test]
fn requests_reject_unknown_fields_and_missing_values() {
    assert!(serde_json::from_value::<SignupStart>(json!({"email": "a@b.c"})).is_ok());
    assert!(
        serde_json::from_value::<SignupStart>(json!({"email": "a@b.c", "password": "x"})).is_err()
    );
    assert!(serde_json::from_value::<Login>(json!({"email": "a@b.c", "password": "p"})).is_ok());
    for bad in [
        json!({"email": "a@b.c"}),
        json!({"password": "p"}),
        json!({"email": "a@b.c", "password": "p", "purpose": "studio"}),
        json!({"email": 1, "password": "p"}),
    ] {
        assert!(serde_json::from_value::<Login>(bad).is_err());
    }
    let complete = json!({
        "operation_id": "00".repeat(16), "proof_id": "11".repeat(16),
        "proof_secret": "22".repeat(32), "password": "chosen", "explicit_confirmation": true,
    });
    assert!(serde_json::from_value::<SignupComplete>(complete.clone()).is_ok());
    for field in [
        "operation_id",
        "proof_id",
        "proof_secret",
        "password",
        "explicit_confirmation",
    ] {
        let mut missing = complete.clone();
        missing.as_object_mut().unwrap().remove(field);
        assert!(
            serde_json::from_value::<SignupComplete>(missing).is_err(),
            "{field}"
        );
    }
    let mut extra = complete;
    extra["actor_id"] = json!("x");
    assert!(serde_json::from_value::<SignupComplete>(extra).is_err());
}

#[test]
fn responses_have_the_exact_wire_shape() {
    assert_eq!(
        serde_json::to_value(SignupAccepted {
            accepted: true,
            proof_ttl_seconds: 900
        })
        .unwrap(),
        json!({"accepted": true, "proof_ttl_seconds": 900})
    );
    for (status, text) in [
        (SignupStatus::Completed, "completed"),
        (SignupStatus::Replayed, "replayed"),
    ] {
        assert_eq!(
            serde_json::to_value(SignupCompleted { status }).unwrap(),
            json!({"status": text})
        );
    }
    assert!(serde_json::from_value::<SignupCompleted>(json!({"status": "pending"})).is_err());
}

#[test]
fn debug_output_never_contains_an_address_a_password_or_a_proof() {
    let login = Login {
        email: "owner@example.com".into(),
        password: "hunter2-hunter2".into(),
    };
    let complete = SignupComplete {
        operation_id: "aa".repeat(16),
        proof_id: "bb".repeat(16),
        proof_secret: "cc".repeat(32),
        password: "hunter2-hunter2".into(),
        explicit_confirmation: true,
    };
    let start = SignupStart {
        email: "owner@example.com".into(),
    };
    for text in [
        format!("{login:?}"),
        format!("{complete:?}"),
        format!("{start:?}"),
    ] {
        for secret in ["owner@example.com", "hunter2", "cccc", "aaaa"] {
            assert!(!text.contains(secret), "{text}");
        }
    }
}
