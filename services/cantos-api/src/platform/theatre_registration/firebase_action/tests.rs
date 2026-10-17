use super::*;
use crate::platform::theatre_registration::domain::Email;
use uuid::Uuid;

fn binding() -> ExpectedActionBinding {
    ExpectedActionBinding::new(
        PendingRegistration::new(
            Uuid::from_u128(1),
            Email::parse("Victim@EXAMPLE.test").unwrap(),
        )
        .unwrap(),
        FirebaseProjectId::parse("candidate-project").unwrap(),
        DedicatedUid::parse("dedicated-new-uid").unwrap(),
    )
}

#[test]
fn project_uid_key_and_code_factories_reject_invalid_inputs_without_retaining_them() {
    for invalid in [
        "",
        "short",
        "UPPERCASE",
        "123456",
        "project-",
        "project/id",
        " has-space",
        "project.with.dot",
        "this-project-id-is-longer-than-thirty-characters",
    ] {
        assert_eq!(
            FirebaseProjectId::parse(invalid),
            Err(ActionError::InvalidBinding(BindingField::Project))
        );
    }
    for valid in ["cantos", "candidate-project", "a23456", "project-012345"] {
        assert!(FirebaseProjectId::parse(valid).is_ok());
    }
    for invalid in ["", "line\nbreak", "space here", "Việt"] {
        assert_eq!(
            ApiKey::parse(invalid).err(),
            Some(ActionError::InvalidApiKey)
        );
        assert_eq!(
            ActionCode::parse(invalid).err(),
            Some(ActionError::InvalidCode)
        );
        assert_eq!(
            DedicatedUid::parse(invalid),
            Err(ActionError::InvalidBinding(BindingField::Uid))
        );
    }
    assert_eq!(
        ApiKey::parse(&"x".repeat(513)).err(),
        Some(ActionError::InvalidApiKey)
    );
    assert_eq!(
        ActionCode::parse(&"x".repeat(4097)).err(),
        Some(ActionError::InvalidCode)
    );
    assert_eq!(
        DedicatedUid::parse(&"x".repeat(129)),
        Err(ActionError::InvalidBinding(BindingField::Uid))
    );
    assert!(ApiKey::parse(&"x".repeat(512)).is_ok());
    assert!(ActionCode::parse(&"x".repeat(4096)).is_ok());
    assert!(DedicatedUid::parse(&"x".repeat(128)).is_ok());
}

#[test]
fn fixed_tls_origin_and_redacted_debug_never_expose_credentials_or_binding_pii() {
    let client = FirebaseActionClient::new(
        FirebaseProjectId::parse("candidate-project").unwrap(),
        ApiKey::parse("synthetic-private-api-key").unwrap(),
    )
    .unwrap();
    assert_eq!(client.base.as_str(), PROVIDER_ENDPOINT);
    let endpoint = client.endpoint(false);
    assert_eq!(endpoint.scheme(), "https");
    assert_eq!(endpoint.host_str(), Some("identitytoolkit.googleapis.com"));
    assert_eq!(endpoint.path(), "/v1/accounts:resetPassword");
    assert_eq!(client.endpoint(true).path(), "/v1/accounts:update");
    assert_eq!(format!("{client:?}"), "FirebaseActionClient([REDACTED])");
    assert_eq!(format!("{:?}", client.key), "ApiKey([REDACTED])");
    assert_eq!(
        format!("{:?}", binding()),
        "ExpectedActionBinding([REDACTED])"
    );
    assert_eq!(
        format!("{:?}", ActionCode::parse("synthetic-secret-code").unwrap()),
        "ActionCode([REDACTED])"
    );
    let inspected = InspectedAction {
        client: &client,
        binding: binding(),
        code: ActionCode::parse("synthetic-secret-code").unwrap(),
    };
    assert_eq!(format!("{inspected:?}"), "InspectedAction([REDACTED])");
    assert_eq!(
        format!("{:?}", ConsumedAction { binding: binding() }),
        "ConsumedAction([REDACTED])"
    );
}

#[test]
fn only_exact_verification_purpose_and_expected_email_pass_inspection() {
    let expected = binding();
    let valid: Inspection =
        serde_json::from_str(r#"{"email":"Victim@example.test","requestType":"VERIFY_EMAIL"}"#)
            .unwrap();
    assert_eq!(valid.check(&expected), Ok(()));
    for purpose in [
        "PASSWORD_RESET",
        "EMAIL_SIGNIN",
        "RECOVER_EMAIL",
        "VERIFY_AND_CHANGE_EMAIL",
        "verify_email",
        "",
    ] {
        let invalid = Inspection {
            request_type: purpose.into(),
            ..valid_for_inspection()
        };
        assert_eq!(invalid.check(&expected), Err(ActionError::WrongPurpose));
    }
    for email in [
        "victim@example.test",
        "Victim@EXAMPLE.test",
        "Victim+alias@example.test",
        " Victim@example.test",
        "other@example.test",
    ] {
        let invalid = Inspection {
            email: email.into(),
            ..valid_for_inspection()
        };
        assert_eq!(
            invalid.check(&expected),
            Err(ActionError::InvalidBinding(BindingField::Email))
        );
        let invalid = Inspection {
            new_email: Some(email.into()),
            ..valid_for_inspection()
        };
        assert_eq!(
            invalid.check(&expected),
            Err(ActionError::InvalidBinding(BindingField::Email))
        );
    }
    let matching = Inspection {
        new_email: Some("Victim@example.test".into()),
        ..valid_for_inspection()
    };
    assert_eq!(matching.check(&expected), Ok(()));
    assert!(serde_json::from_str::<Inspection>(
        r#"{"email":"Victim@example.test","emailVerified":true}"#
    )
    .is_err());
}

fn valid_for_inspection() -> Inspection {
    Inspection {
        email: "Victim@example.test".into(),
        request_type: "VERIFY_EMAIL".into(),
        new_email: None,
    }
}

#[test]
fn successful_consumption_requires_exact_uid_email_and_refuses_conflicting_claims() {
    let expected = binding();
    let literal = r#"{"localId":"dedicated-new-uid","email":"Victim@example.test"}"#;
    let valid: Consumption = serde_json::from_str(literal).unwrap();
    assert_eq!(valid.check(&expected), Ok(()));
    for (reply, error) in [
        (
            r#"{"localId":"other-uid","email":"Victim@example.test","emailVerified":true}"#,
            ActionError::InvalidBinding(BindingField::Uid),
        ),
        (
            r#"{"localId":"dedicated-new-uid","email":"victim@example.test","emailVerified":true}"#,
            ActionError::InvalidBinding(BindingField::Email),
        ),
        (
            r#"{"localId":"dedicated-new-uid","email":"Victim@example.test","newEmail":"other@example.test"}"#,
            ActionError::InvalidBinding(BindingField::Email),
        ),
        (
            r#"{"localId":"dedicated-new-uid","email":"Victim@example.test","emailVerified":false}"#,
            ActionError::Unverified,
        ),
    ] {
        let value: Consumption = serde_json::from_str(reply).unwrap();
        assert_eq!(value.check(&expected), Err(error));
    }
    assert!(serde_json::from_str::<Consumption>(
        r#"{"email":"Victim@example.test","emailVerified":true}"#
    )
    .is_err());
}

#[tokio::test]
async fn wrong_project_is_rejected_locally_and_inspected_values_are_bound_to_exact_client() {
    let client =
        FirebaseActionClient::fixture("127.0.0.1:1".parse().unwrap(), Duration::from_millis(20))
            .unwrap();
    let mut expected = binding();
    expected.project = FirebaseProjectId::parse("different-project").unwrap();
    assert_eq!(
        client
            .inspect(expected, ActionCode::parse("code").unwrap())
            .await
            .err(),
        Some(ActionError::InvalidBinding(BindingField::Project))
    );
    let equivalent =
        FirebaseActionClient::fixture("127.0.0.1:1".parse().unwrap(), Duration::from_millis(20))
            .unwrap();
    let mut different_key =
        FirebaseActionClient::fixture("127.0.0.1:1".parse().unwrap(), Duration::from_millis(20))
            .unwrap();
    different_key.key = ApiKey::parse("another-key").unwrap();
    let mut different_project =
        FirebaseActionClient::fixture("127.0.0.1:1".parse().unwrap(), Duration::from_millis(20))
            .unwrap();
    different_project.project = FirebaseProjectId::parse("different-project").unwrap();
    for other in [equivalent, different_key, different_project] {
        // Constructed in the child test only; production callers have no such constructor.
        let inspected = InspectedAction {
            client: &client,
            binding: binding(),
            code: ActionCode::parse("code").unwrap(),
        };
        assert_eq!(
            other.consume(inspected).await,
            Err(ConsumptionError::BeforeConsume(ActionError::WrongClient))
        );
    }
}

#[test]
fn all_test_seams_refuse_non_loopback_and_use_only_fixed_prefixes() {
    for address in ["192.0.2.1:1234", "127.0.0.1:0", "[2001:db8::1]:1234"] {
        let address = address.parse().unwrap();
        assert_eq!(
            FirebaseActionClient::fixture(address, Duration::from_secs(1)).err(),
            Some(ActionError::Configuration)
        );
        assert_eq!(
            FirebaseActionClient::auth_emulator_fixture(address, Duration::from_secs(1)).err(),
            Some(ActionError::Configuration)
        );
    }
    let client = FirebaseActionClient::auth_emulator_fixture(
        "127.0.0.1:9099".parse().unwrap(),
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        client.endpoint(false).path(),
        "/identitytoolkit.googleapis.com/v1/accounts:resetPassword"
    );
    assert_eq!(
        client.project,
        FirebaseProjectId::parse("demo-cantos-issue206").unwrap()
    );
}

#[test]
fn owned_emulator_action_seam_is_demo_project_literal_loopback_only() {
    for (address, project) in [
        ("192.0.2.1:9199", "demo-cantos-242"),
        ("127.0.0.1:0", "demo-cantos-242"),
        ("127.0.0.1:9199", "real-project"),
    ] {
        assert_eq!(
            FirebaseActionClient::owned_auth_emulator_fixture(
                address.parse().unwrap(),
                project,
                Duration::from_secs(1),
            )
            .err(),
            Some(ActionError::Configuration),
        );
    }
}
