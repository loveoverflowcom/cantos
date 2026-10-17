use super::*;
use ring::signature::KeyPair;
use serde_json::{json, Value};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use uuid::Uuid;

include!("synthetic_key.rs");

pub(crate) fn service_account() -> Value {
    json!({
        "type": "service_account",
        "project_id": "candidate-project",
        "client_email": "synthetic@candidate-project.iam.gserviceaccount.com",
        "private_key": format!("-----BEGIN PRIVATE KEY-----\n{SYNTHETIC_KEY_DER_B64}\n-----END PRIVATE KEY-----\n"),
        // Ignored, never used to select a network endpoint.
        "token_uri": "https://attacker.invalid/credential-exfiltration"
    })
}

pub(super) fn config() -> FirebaseBootstrapConfig {
    FirebaseBootstrapConfig::parse(
        "candidate-project",
        "synthetic-api-key",
        &serde_json::to_vec(&service_account()).unwrap(),
    )
    .unwrap()
}

fn identity() -> BootstrapIdentity {
    BootstrapIdentity::new(
        "dedicated-new-uid",
        Email::parse("Victim@example.test").unwrap(),
    )
    .unwrap()
}

fn acknowledged(identity: BootstrapIdentity) -> CreatedFirebaseUser {
    CreatedFirebaseUser::from_durable_ack("candidate-project", identity).unwrap()
}

fn enrollment() -> Enrollment {
    Enrollment::new(
        super::super::domain::PendingRegistration::new(
            Uuid::from_u128(1),
            Email::parse("Victim@example.test").unwrap(),
        )
        .unwrap(),
        "candidate-project",
    )
    .unwrap()
}

#[test]
fn config_refuses_malformed_cross_project_or_non_rsa_credentials() {
    for (field, value) in [
        ("type", "authorized_user"),
        ("project_id", "different-project"),
        (
            "client_email",
            "synthetic@different-project.iam.gserviceaccount.com",
        ),
        (
            "client_email",
            " synthetic@candidate-project.iam.gserviceaccount.com",
        ),
        (
            "private_key",
            "-----BEGIN RSA PRIVATE KEY-----\nAQID\n-----END RSA PRIVATE KEY-----",
        ),
        (
            "private_key",
            "-----BEGIN PRIVATE KEY-----\nAQID\n-----END PRIVATE KEY-----",
        ),
    ] {
        let mut raw = service_account();
        raw[field] = Value::String(value.into());
        assert_eq!(
            FirebaseBootstrapConfig::parse(
                "candidate-project",
                "synthetic-api-key",
                &serde_json::to_vec(&raw).unwrap(),
            )
            .err(),
            Some(BootstrapError::Configuration)
        );
    }
    assert_eq!(
        FirebaseBootstrapConfig::parse("candidate-project", "key", b"{}").err(),
        Some(BootstrapError::Configuration)
    );
    assert_eq!(
        FirebaseBootstrapConfig::parse(
            "candidate-project",
            "key",
            &vec![b' '; MAX_SERVICE_ACCOUNT_BYTES + 1]
        )
        .err(),
        Some(BootstrapError::Configuration)
    );
    assert_eq!(
        FirebaseBootstrapConfig::parse(
            "other-project",
            "key",
            &serde_json::to_vec(&service_account()).unwrap()
        )
        .err(),
        Some(BootstrapError::Configuration)
    );
    assert_eq!(
        FirebaseBootstrapConfig::parse(
            "candidate-project",
            "has space",
            &serde_json::to_vec(&service_account()).unwrap()
        )
        .err(),
        Some(BootstrapError::Configuration)
    );
    for invalid in ["", "header\r\ninjection", "white space"] {
        assert_eq!(
            AdminAccessToken::parse(invalid).err(),
            Some(BootstrapError::InvalidCredential)
        );
    }
    assert_eq!(
        BootstrapIdentity::new("", Email::parse("a@example.test").unwrap()).err(),
        Some(BootstrapError::InvalidIdentity)
    );
}

#[test]
fn custom_token_signature_and_exact_claims_are_independently_checked() {
    let config = config();
    let token = config.custom_token(&identity(), 1_800_000_000).unwrap();
    let parts: Vec<_> = token.split('.').collect();
    assert_eq!(parts.len(), 3);
    let header: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap();
    let claims: Value = serde_json::from_slice(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap();
    assert_eq!(header, json!({"alg":"RS256","typ":"JWT"}));
    assert_eq!(
        claims,
        json!({
            "iss":"synthetic@candidate-project.iam.gserviceaccount.com",
            "sub":"synthetic@candidate-project.iam.gserviceaccount.com",
            "aud":"https://identitytoolkit.googleapis.com/google.identity.identitytoolkit.v1.IdentityToolkit",
            "iat":1_800_000_000u64,
            "exp":1_800_003_600u64,
            "uid":"dedicated-new-uid"
        })
    );
    let verifier = signature::UnparsedPublicKey::new(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        config.signing_key.public_key().as_ref(),
    );
    verifier
        .verify(
            format!("{}.{}", parts[0], parts[1]).as_bytes(),
            &URL_SAFE_NO_PAD.decode(parts[2]).unwrap(),
        )
        .unwrap();
    assert!(verifier
        .verify(
            b"altered-payload",
            &URL_SAFE_NO_PAD.decode(parts[2]).unwrap()
        )
        .is_err());
    assert_eq!(
        config.custom_token(&identity(), 0),
        Err(BootstrapError::InvalidClock)
    );
    assert_eq!(
        config.custom_token(&identity(), u64::MAX),
        Err(BootstrapError::InvalidClock)
    );
}

#[test]
fn fixed_origin_and_debug_never_expose_service_key_tokens_or_recipient() {
    let client = FirebaseBootstrapClient::new(config()).unwrap();
    assert_eq!(client.base.as_str(), PROVIDER_ENDPOINT);
    for value in [
        format!("{client:?}"),
        format!("{:?}", client.config),
        format!(
            "{:?}",
            AdminAccessToken::parse("synthetic-access-token").unwrap()
        ),
        format!("{:?}", acknowledged(identity())),
    ] {
        assert!(value.contains("[REDACTED]"));
        for secret in [
            "synthetic-api-key",
            "synthetic-access-token",
            "Victim@",
            SYNTHETIC_KEY_DER_B64,
        ] {
            assert!(!value.contains(secret));
        }
    }
    for invalid in [
        "http://remote.example.test",
        "https://user:pass@example.test",
        "https://example.test/path",
        "https://example.test/?query=1",
        "https://example.test/#fragment",
    ] {
        assert_eq!(
            FirebaseMailboxProvider::new(FirebaseBootstrapClient::new(config()).unwrap(), invalid,)
                .err(),
            Some(RegistrationError::Configuration)
        );
    }
}

#[derive(Debug)]
pub(super) struct Captured {
    pub(super) path: String,
    pub(super) authorization: Option<String>,
    pub(super) content_type: Option<String>,
    pub(super) raw: Vec<u8>,
    /// JSON body; `Null` for a non-JSON (form) request body.
    pub(super) body: Value,
}

pub(super) struct Reply {
    pub(super) status: u16,
    pub(super) content_type: &'static str,
    pub(super) body: String,
}

pub(super) fn ok(body: Value) -> Reply {
    Reply {
        status: 200,
        content_type: "application/json",
        body: body.to_string(),
    }
}

pub(super) async fn server(
    replies: Vec<Reply>,
) -> (std::net::SocketAddr, tokio::task::JoinHandle<Vec<Captured>>) {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let mut captured = Vec::new();
        for reply in replies {
            let (mut socket, _) = tokio::time::timeout(Duration::from_secs(3), listener.accept())
                .await
                .unwrap()
                .unwrap();
            let request = read_request(&mut socket).await;
            captured.push(request);
            let response = format!(
                "HTTP/1.1 {} fixture\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{}",
                reply.status, reply.content_type, reply.body.len(), reply.body,
            );
            socket.write_all(response.as_bytes()).await.unwrap();
        }
        captured
    });
    (address, task)
}

async fn read_request(socket: &mut tokio::net::TcpStream) -> Captured {
    let mut bytes = Vec::new();
    let header_end = loop {
        let mut chunk = [0; 2048];
        let count = socket.read(&mut chunk).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&chunk[..count]);
        assert!(bytes.len() < 32768);
        if let Some(pos) = bytes.windows(4).position(|w| w == b"\r\n\r\n") {
            break pos + 4;
        }
    };
    let headers = String::from_utf8(bytes[..header_end].to_vec()).unwrap();
    let length = headers
        .lines()
        .find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-length")
                .then(|| value.trim().parse::<usize>().unwrap())
        })
        .unwrap();
    while bytes.len() - header_end < length {
        let mut chunk = [0; 2048];
        let count = socket.read(&mut chunk).await.unwrap();
        assert!(count > 0);
        bytes.extend_from_slice(&chunk[..count]);
    }
    Captured {
        path: headers
            .lines()
            .next()
            .unwrap()
            .split_whitespace()
            .nth(1)
            .unwrap()
            .to_owned(),
        authorization: headers.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("authorization")
                .then(|| value.trim().to_owned())
        }),
        content_type: headers.lines().find_map(|line| {
            let (key, value) = line.split_once(':')?;
            key.eq_ignore_ascii_case("content-type")
                .then(|| value.trim().to_owned())
        }),
        raw: bytes[header_end..header_end + length].to_vec(),
        body: serde_json::from_slice(&bytes[header_end..header_end + length])
            .unwrap_or(Value::Null),
    }
}

#[tokio::test]
async fn bootstrap_exchange_send_use_exact_minimal_requests_and_server_only_tokens() {
    let operation = enrollment();
    let (address, task) = server(vec![
        ok(json!({"localId":operation.uid(),"email":"Victim@example.test"})),
        ok(json!({"idToken":"synthetic-id-token","refreshToken":"discard-me","expiresIn":"3600","isNewUser":false})),
        ok(json!({"email":"Victim@example.test"})),
    ]).await;
    let provider = FirebaseMailboxProvider::new(
        FirebaseBootstrapClient::fixture(config(), address, Duration::from_secs(1)).unwrap(),
        "https://cantos.example.test",
    )
    .unwrap();
    provider.create_unverified(operation.clone()).await.unwrap();
    let credential = provider
        .prepare_verification(operation.clone())
        .await
        .unwrap();
    provider
        .send_verification(operation.clone(), credential)
        .await
        .unwrap();
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 3);
    assert_eq!(
        requests[0].path,
        "/v1/accounts:signUp?key=synthetic-api-key"
    );
    assert_eq!(
        requests[0].authorization.as_deref(),
        Some("Bearer synthetic-access-token")
    );
    assert_eq!(
        requests[0].body,
        json!({
            "localId":operation.uid(),"email":"Victim@example.test","emailVerified":false,"targetProjectId":"candidate-project"
        })
    );
    assert_eq!(
        requests[1].path,
        "/v1/accounts:signInWithCustomToken?key=synthetic-api-key"
    );
    assert_eq!(requests[1].authorization, None);
    assert_eq!(requests[1].body.as_object().unwrap().len(), 2);
    assert_eq!(requests[1].body["returnSecureToken"], true);
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(
                requests[1].body["token"]
                    .as_str()
                    .unwrap()
                    .split('.')
                    .nth(1)
                    .unwrap(),
            )
            .unwrap(),
    )
    .unwrap();
    assert_eq!(claims["uid"], operation.uid());
    assert_eq!(
        requests[2].path,
        "/v1/accounts:sendOobCode?key=synthetic-api-key"
    );
    assert_eq!(requests[2].authorization, None);
    assert_eq!(
        requests[2].body,
        json!({
            "requestType":"VERIFY_EMAIL","email":"Victim@example.test","idToken":"synthetic-id-token",
            "continueUrl":format!("https://cantos.example.test/?firebaseEnrollment={}",operation.id())
        })
    );
}

#[tokio::test]
async fn create_mismatch_and_conflict_never_produce_a_create_witness() {
    for (reply, expected) in [
        (
            ok(json!({"localId":"other-uid","email":"Victim@example.test"})),
            BootstrapFailure::OutcomeUncertain(BootstrapError::WrongBinding),
        ),
        (
            ok(json!({"localId":"dedicated-new-uid","email":"other@example.test"})),
            BootstrapFailure::OutcomeUncertain(BootstrapError::WrongBinding),
        ),
        (
            ok(json!({"email":"Victim@example.test"})),
            BootstrapFailure::OutcomeUncertain(BootstrapError::Protocol),
        ),
        (
            Reply {
                status: 400,
                content_type: "application/json",
                body: json!({"error":{"message":"EMAIL_EXISTS"}}).to_string(),
            },
            BootstrapFailure::Rejected(BootstrapError::Conflict),
        ),
    ] {
        let (address, task) = server(vec![reply]).await;
        let client =
            FirebaseBootstrapClient::fixture(config(), address, Duration::from_secs(1)).unwrap();
        assert_eq!(client.create_user(&identity()).await.err(), Some(expected));
        assert_eq!(task.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn exchange_refuses_recreated_account_empty_token_and_bad_lifetime() {
    for body in [
        json!({"idToken":"token","expiresIn":"3600","isNewUser":true}),
        json!({"idToken":"","expiresIn":"3600","isNewUser":false}),
        json!({"idToken":"token","expiresIn":"0","isNewUser":false}),
        json!({"idToken":"token","expiresIn":"3601","isNewUser":false}),
    ] {
        let (address, task) = server(vec![ok(body)]).await;
        let client =
            FirebaseBootstrapClient::fixture(config(), address, Duration::from_secs(1)).unwrap();
        assert_eq!(
            client
                .credential_exchange(&acknowledged(identity()), 1_800_000_000)
                .await
                .err(),
            Some(BootstrapFailure::OutcomeUncertain(BootstrapError::Protocol))
        );
        assert_eq!(task.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn cross_client_binding_and_project_mismatches_are_rejected_before_dispatch() {
    let client = FirebaseBootstrapClient::fixture(
        config(),
        "127.0.0.1:1".parse().unwrap(),
        Duration::from_millis(20),
    )
    .unwrap();
    let other = FirebaseBootstrapClient::fixture(
        config(),
        "127.0.0.1:1".parse().unwrap(),
        Duration::from_millis(20),
    )
    .unwrap();
    let created = acknowledged(identity());
    let continuation = || VerificationContinuation {
        identity: identity(),
        url: "https://cantos.example.test/".into(),
    };
    let credential = SendCredential {
        client: &other,
        identity: identity(),
        id_token: "token".into(),
    };
    assert_eq!(
        client
            .send_verification(&created, credential, continuation())
            .await
            .err(),
        Some(BootstrapFailure::BeforeDispatch(
            BootstrapError::WrongClient
        ))
    );
    let credential = SendCredential {
        client: &client,
        identity: BootstrapIdentity::new("other-uid", Email::parse("Victim@example.test").unwrap())
            .unwrap(),
        id_token: "token".into(),
    };
    assert_eq!(
        client
            .send_verification(&created, credential, continuation())
            .await
            .err(),
        Some(BootstrapFailure::BeforeDispatch(
            BootstrapError::WrongBinding
        ))
    );
    let mismatched =
        CreatedFirebaseUser::from_durable_ack("different-project", identity()).unwrap();
    assert_eq!(
        client
            .credential_exchange(&mismatched, 1_800_000_000)
            .await
            .err(),
        Some(BootstrapFailure::BeforeDispatch(
            BootstrapError::WrongBinding
        ))
    );
}

#[tokio::test]
async fn prepared_mail_credential_is_ram_only_and_bound_to_the_exact_enrollment() {
    let operation = enrollment();
    let (address, task) = server(vec![ok(json!({
        "idToken":"synthetic-id-token", "expiresIn":"3600", "isNewUser":false
    }))])
    .await;
    let provider = FirebaseMailboxProvider::new(
        FirebaseBootstrapClient::fixture(config(), address, Duration::from_secs(1)).unwrap(),
        "https://cantos.example.test",
    )
    .unwrap();
    let credential = provider.prepare_verification(operation).await.unwrap();
    assert_eq!(
        format!("{credential:?}"),
        "VerificationCredential([REDACTED])"
    );
    let other = Enrollment::new(
        super::super::domain::PendingRegistration::new(
            Uuid::from_u128(2),
            Email::parse("Victim@example.test").unwrap(),
        )
        .unwrap(),
        "candidate-project",
    )
    .unwrap();
    assert_eq!(
        provider.send_verification(other, credential).await,
        Err(RegistrationError::InvalidState)
    );
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert!(requests[0]
        .path
        .starts_with("/v1/accounts:signInWithCustomToken?"));
}

#[tokio::test]
async fn malformed_oversized_redirect_and_remote_failures_remain_sanitized() {
    for (reply, expected) in [
        (
            Reply {
                status: 200,
                content_type: "application/json",
                body: "not-json-secret".into(),
            },
            BootstrapFailure::OutcomeUncertain(BootstrapError::Protocol),
        ),
        (
            Reply {
                status: 200,
                content_type: "text/plain",
                body: "secret".into(),
            },
            BootstrapFailure::OutcomeUncertain(BootstrapError::NotJson),
        ),
        (
            Reply {
                status: 200,
                content_type: "application/json",
                body: "x".repeat(MAX_RESPONSE_BYTES + 1),
            },
            BootstrapFailure::OutcomeUncertain(BootstrapError::TooLarge),
        ),
        (
            Reply {
                status: 302,
                content_type: "application/json",
                body: "{}".into(),
            },
            BootstrapFailure::OutcomeUncertain(BootstrapError::Redirect),
        ),
        (
            Reply {
                status: 503,
                content_type: "application/json",
                body: "{}".into(),
            },
            BootstrapFailure::OutcomeUncertain(BootstrapError::Provider),
        ),
        (
            Reply {
                status: 429,
                content_type: "application/json",
                body: "{}".into(),
            },
            BootstrapFailure::Rejected(BootstrapError::Quota),
        ),
        (
            Reply {
                status: 400,
                content_type: "application/json",
                body: json!({"error":{"message":"synthetic-unrecognized-private-detail"}})
                    .to_string(),
            },
            BootstrapFailure::Rejected(BootstrapError::Rejected),
        ),
    ] {
        let (address, task) = server(vec![reply]).await;
        let client =
            FirebaseBootstrapClient::fixture(config(), address, Duration::from_secs(1)).unwrap();
        let error = client.create_user(&identity()).await.err().unwrap();
        assert_eq!(error, expected);
        assert!(!format!("{error:?}").contains("synthetic-unrecognized-private-detail"));
        assert_eq!(task.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn lost_send_response_is_uncertain_and_has_no_automatic_replay() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let task = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.unwrap();
        let request = read_request(&mut socket).await;
        drop(socket);
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
        request
    });
    let client =
        FirebaseBootstrapClient::fixture(config(), address, Duration::from_secs(1)).unwrap();
    let created = acknowledged(identity());
    let credential = SendCredential {
        client: &client,
        identity: identity(),
        id_token: "synthetic-id-token".into(),
    };
    let continuation = VerificationContinuation {
        identity: identity(),
        url: "https://cantos.example.test/?firebaseEnrollment=00000000-0000-0000-0000-000000000001"
            .into(),
    };
    assert_eq!(
        client
            .send_verification(&created, credential, continuation)
            .await
            .err(),
        Some(BootstrapFailure::OutcomeUncertain(BootstrapError::Network))
    );
    let request = task.await.unwrap();
    assert!(request.path.starts_with("/v1/accounts:sendOobCode?"));
}

fn emulator_config() -> FirebaseBootstrapConfig {
    let mut account = service_account();
    account["project_id"] = json!("demo-cantos-242");
    account["client_email"] = json!("synthetic@demo-cantos-242.iam.gserviceaccount.com");
    FirebaseBootstrapConfig::parse(
        "demo-cantos-242",
        "synthetic-fixture-key",
        &serde_json::to_vec(&account).unwrap(),
    )
    .unwrap()
}

#[test]
fn emulator_bootstrap_seam_requires_a_demo_project_and_literal_nonzero_loopback() {
    for address in ["192.0.2.1:9199", "127.0.0.1:0", "[::]:9199"] {
        assert_eq!(
            FirebaseBootstrapClient::auth_emulator_fixture(
                emulator_config(),
                address.parse().unwrap(),
                Duration::from_secs(1),
            )
            .err(),
            Some(BootstrapError::Configuration),
        );
    }
    assert_eq!(
        FirebaseBootstrapClient::auth_emulator_fixture(
            config(),
            "127.0.0.1:9199".parse().unwrap(),
            Duration::from_secs(1),
        )
        .err(),
        Some(BootstrapError::Configuration),
    );
    let production = FirebaseBootstrapClient::new(emulator_config()).unwrap();
    assert_eq!(production.base.as_str(), PROVIDER_ENDPOINT);
}

#[tokio::test]
async fn emulator_transport_reuses_signup_body_parser_and_signed_custom_token() {
    let identity = identity();
    let (address, task) = server(vec![
        ok(json!({"localId":identity.uid(),"email":identity.email().as_str()})),
        ok(json!({"idToken":"synthetic-id-token","expiresIn":"3600","isNewUser":false})),
    ])
    .await;
    let client = FirebaseBootstrapClient::auth_emulator_fixture(
        emulator_config(),
        address,
        Duration::from_secs(1),
    )
    .unwrap();
    let created = client.create_user(&identity).await.unwrap();
    let _credential = client
        .credential_exchange(&created, 1_800_000_000)
        .await
        .unwrap();
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 2);
    assert_eq!(
        requests[0].path,
        "/identitytoolkit.googleapis.com/v1/accounts:signUp?key=synthetic-fixture-key"
    );
    assert_eq!(requests[0].authorization.as_deref(), Some("Bearer owner"));
    assert_eq!(
        requests[0].body,
        json!({
            "localId":"dedicated-new-uid", "email":"Victim@example.test",
            "emailVerified":false, "targetProjectId":"demo-cantos-242",
        })
    );
    assert_eq!(requests[1].path, "/identitytoolkit.googleapis.com/v1/accounts:signInWithCustomToken?key=synthetic-fixture-key");
    let token = requests[1].body["token"].as_str().unwrap();
    assert_eq!(
        token.split('.').count(),
        3,
        "production RSA token construction remains active"
    );
    let claims: Value = serde_json::from_slice(
        &URL_SAFE_NO_PAD
            .decode(token.split('.').nth(1).unwrap())
            .unwrap(),
    )
    .unwrap();
    assert_eq!(claims["uid"], "dedicated-new-uid");
    assert_eq!(
        claims["iss"],
        "synthetic@demo-cantos-242.iam.gserviceaccount.com"
    );
}
