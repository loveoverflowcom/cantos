use super::*;
use crate::platform::theatre_registration::firebase_bootstrap::{
    tests::{config, ok, server, Captured, Reply},
    BootstrapFailure, BootstrapIdentity, Email, FirebaseBootstrapClient, URL_SAFE_NO_PAD,
};
use base64::Engine;
use ring::signature::{self, KeyPair};
use serde_json::{json, Value};
use std::sync::Arc;

const EPOCH: u64 = 1_800_000_000;

fn grant(token: &str, expires_in: u64) -> Reply {
    ok(json!({"access_token":token,"expires_in":expires_in,"token_type":"Bearer"}))
}

fn error(status: u16, body: Value) -> Reply {
    Reply {
        status,
        content_type: "application/json",
        body: body.to_string(),
    }
}

async fn source(
    replies: Vec<Reply>,
) -> (AccessTokenSource, tokio::task::JoinHandle<Vec<Captured>>) {
    let (address, task) = server(replies).await;
    (
        AccessTokenSource::fixture(address, Duration::from_secs(1)).unwrap(),
        task,
    )
}

fn secret(token: &AdminAccessToken) -> &str {
    &token.0
}

#[test]
fn assertion_has_exact_claims_the_narrow_scope_and_a_verifiable_signature() {
    let config = config();
    let assertion = config.access_assertion(EPOCH).unwrap();
    let parts: Vec<_> = assertion.split('.').collect();
    assert_eq!(parts.len(), 3);
    assert_eq!(
        serde_json::from_slice::<Value>(&URL_SAFE_NO_PAD.decode(parts[0]).unwrap()).unwrap(),
        json!({"alg":"RS256","typ":"JWT"})
    );
    assert_eq!(
        serde_json::from_slice::<Value>(&URL_SAFE_NO_PAD.decode(parts[1]).unwrap()).unwrap(),
        json!({
            "iss":"synthetic@candidate-project.iam.gserviceaccount.com",
            "scope":"https://www.googleapis.com/auth/identitytoolkit",
            "aud":"https://oauth2.googleapis.com/token",
            "iat":EPOCH,
            "exp":EPOCH + 300
        })
    );
    let verifier = signature::UnparsedPublicKey::new(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        config.signing_key.public_key().as_ref(),
    );
    let signature = URL_SAFE_NO_PAD.decode(parts[2]).unwrap();
    verifier
        .verify(format!("{}.{}", parts[0], parts[1]).as_bytes(), &signature)
        .unwrap();
    assert!(verifier.verify(b"altered", &signature).is_err());
    for clock in [0, u64::MAX] {
        assert_eq!(
            config.access_assertion(clock),
            Err(BootstrapError::InvalidClock)
        );
    }
}

#[test]
fn grant_parsing_computes_the_refresh_instant_with_the_safety_margin() {
    let now = Instant::now();
    let cached = parse_grant(
        StatusCode::OK,
        br#"{"access_token":"ya29.synthetic","expires_in":3599,"token_type":"bearer","extra":"ignored"}"#,
        now,
    )
    .unwrap();
    assert_eq!(secret(&cached.token), "ya29.synthetic");
    assert_eq!(cached.refresh_at, now + Duration::from_secs(3599 - 300));
    // Fresh strictly before the refresh instant, stale exactly at and after it.
    assert!(cached.is_fresh(now));
    assert!(cached.is_fresh(cached.refresh_at - Duration::from_millis(1)));
    assert!(!cached.is_fresh(cached.refresh_at));
    assert!(!cached.is_fresh(cached.refresh_at + Duration::from_secs(1)));
}

#[test]
fn grant_parsing_refuses_anything_that_could_be_stale_malformed_or_unbounded() {
    let now = Instant::now();
    for body in [
        json!({"access_token":"t","expires_in":3600,"token_type":"MAC"}),
        json!({"access_token":"","expires_in":3600,"token_type":"Bearer"}),
        json!({"access_token":"has space","expires_in":3600,"token_type":"Bearer"}),
        json!({"access_token":"x".repeat(8193),"expires_in":3600,"token_type":"Bearer"}),
        json!({"access_token":"t","expires_in":300,"token_type":"Bearer"}),
        json!({"access_token":"t","expires_in":0,"token_type":"Bearer"}),
        json!({"access_token":"t","expires_in":3601,"token_type":"Bearer"}),
        json!({"access_token":"t","expires_in":"3600","token_type":"Bearer"}),
        json!({"access_token":"t","expires_in":-1,"token_type":"Bearer"}),
        json!({"expires_in":3600,"token_type":"Bearer"}),
        json!({"access_token":"t","token_type":"Bearer"}),
        json!({}),
    ] {
        assert_eq!(
            parse_grant(StatusCode::OK, body.to_string().as_bytes(), now).err(),
            Some(BootstrapError::Protocol),
            "{body}"
        );
    }
    assert_eq!(
        parse_grant(StatusCode::OK, b"not-json", now).err(),
        Some(BootstrapError::Protocol)
    );
}

#[test]
fn provider_errors_map_to_closed_reasons_without_retaining_detail() {
    let now = Instant::now();
    for (status, body, expected) in [
        (
            400,
            r#"{"error":"invalid_grant"}"#,
            BootstrapError::AccessDenied,
        ),
        (
            401,
            r#"{"error":"invalid_client"}"#,
            BootstrapError::AccessDenied,
        ),
        (
            400,
            r#"{"error":"unauthorized_client"}"#,
            BootstrapError::AccessDenied,
        ),
        (
            400,
            r#"{"error":"invalid_scope"}"#,
            BootstrapError::AccessDenied,
        ),
        (
            403,
            r#"{"error":"access_denied"}"#,
            BootstrapError::AccessDenied,
        ),
        (
            400,
            r#"{"error":"synthetic-private-detail"}"#,
            BootstrapError::Rejected,
        ),
        (400, r#"{"unexpected":true}"#, BootstrapError::Protocol),
        (400, "secret-body", BootstrapError::Protocol),
        (429, "{}", BootstrapError::Quota),
        (500, "{}", BootstrapError::Provider),
        (503, "secret-body", BootstrapError::Provider),
    ] {
        let error = parse_grant(StatusCode::from_u16(status).unwrap(), body.as_bytes(), now)
            .err()
            .unwrap();
        assert_eq!(error, expected, "{status} {body}");
        assert!(!format!("{error:?}").contains("secret"));
    }
}

#[test]
fn production_source_targets_only_the_fixed_google_token_origin() {
    let source = AccessTokenSource::new().unwrap();
    assert_eq!(
        source.endpoint.as_str(),
        "https://oauth2.googleapis.com/token"
    );
    let client = FirebaseBootstrapClient::new(config()).unwrap();
    assert_eq!(client.access.endpoint, source.endpoint);
    // Loopback constructors reject non-loopback targets (and exist only under cfg(test)).
    assert_eq!(
        AccessTokenSource::fixture("192.0.2.1:80".parse().unwrap(), Duration::from_secs(1)).err(),
        Some(BootstrapError::Configuration)
    );
}

#[tokio::test]
async fn mint_sends_one_form_encoded_signed_assertion_and_nothing_else() {
    let (source, task) = source(vec![grant("ya29.minted", 3599)]).await;
    let config = config();
    let token = source
        .token_at(&config, Instant::now(), EPOCH)
        .await
        .unwrap();
    assert_eq!(secret(&token), "ya29.minted");
    let requests = task.await.unwrap();
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].path, "/token");
    assert_eq!(requests[0].authorization, None);
    assert_eq!(
        requests[0].content_type.as_deref(),
        Some("application/x-www-form-urlencoded")
    );
    let form: Vec<(String, String)> = url::form_urlencoded::parse(&requests[0].raw)
        .into_owned()
        .collect();
    assert_eq!(form.len(), 2);
    assert_eq!(
        form[0],
        (
            "grant_type".to_owned(),
            "urn:ietf:params:oauth:grant-type:jwt-bearer".to_owned()
        )
    );
    assert_eq!(form[1].0, "assertion");
    let parts: Vec<_> = form[1].1.split('.').collect();
    assert_eq!(parts.len(), 3);
    signature::UnparsedPublicKey::new(
        &signature::RSA_PKCS1_2048_8192_SHA256,
        config.signing_key.public_key().as_ref(),
    )
    .verify(
        format!("{}.{}", parts[0], parts[1]).as_bytes(),
        &URL_SAFE_NO_PAD.decode(parts[2]).unwrap(),
    )
    .unwrap();
    // Never an endpoint, scope or key taken from the service-account file's token_uri.
    assert!(!String::from_utf8_lossy(&requests[0].raw).contains("attacker"));
}

#[tokio::test]
async fn cached_token_is_reused_until_the_margin_then_replaced() {
    let (source, task) = source(vec![grant("first", 3599), grant("second", 3599)]).await;
    let config = config();
    let start = Instant::now();
    let first = source.token_at(&config, start, EPOCH).await.unwrap();
    assert_eq!(secret(&first), "first");
    // Just before the refresh instant: no second request.
    let almost = start + Duration::from_secs(3599 - 300) - Duration::from_millis(1);
    let again = source.token_at(&config, almost, EPOCH).await.unwrap();
    assert_eq!(secret(&again), "first");
    // At the refresh instant a new token is minted and then cached.
    let due = start + Duration::from_secs(3599 - 300);
    let second = source.token_at(&config, due, EPOCH + 3299).await.unwrap();
    assert_eq!(secret(&second), "second");
    let cached = source.token_at(&config, due, EPOCH + 3299).await.unwrap();
    assert_eq!(secret(&cached), "second");
    assert_eq!(task.await.unwrap().len(), 2);
}

#[tokio::test]
async fn failed_refresh_never_serves_the_stale_token_and_the_next_call_retries() {
    let (source, task) = source(vec![
        grant("first", 3599),
        error(503, json!({})),
        grant("third", 3599),
    ])
    .await;
    let config = config();
    let start = Instant::now();
    source.token_at(&config, start, EPOCH).await.unwrap();
    let due = start + Duration::from_secs(3300);
    assert_eq!(
        source.token_at(&config, due, EPOCH + 3300).await.err(),
        Some(BootstrapError::Provider)
    );
    let recovered = source.token_at(&config, due, EPOCH + 3300).await.unwrap();
    assert_eq!(secret(&recovered), "third");
    assert_eq!(task.await.unwrap().len(), 3);
}

#[tokio::test]
async fn concurrent_callers_share_one_token_request() {
    let (source, task) = source(vec![grant("shared", 3599)]).await;
    let source = Arc::new(source);
    let config = Arc::new(config());
    let start = Instant::now();
    let mut calls = Vec::new();
    for _ in 0..16 {
        let (source, config) = (source.clone(), config.clone());
        calls.push(tokio::spawn(async move {
            source
                .token_at(&config, start, EPOCH)
                .await
                .map(|token| token.0)
        }));
    }
    for call in calls {
        assert_eq!(call.await.unwrap().unwrap(), "shared");
    }
    assert_eq!(task.await.unwrap().len(), 1);
}

#[tokio::test]
async fn transport_and_response_faults_are_closed_and_sanitized() {
    for (reply, expected) in [
        (
            error(
                400,
                json!({"error":"invalid_grant","error_description":"secret-detail"}),
            ),
            BootstrapError::AccessDenied,
        ),
        (error(429, json!({})), BootstrapError::Quota),
        (error(500, json!({})), BootstrapError::Provider),
        (
            Reply {
                status: 302,
                content_type: "application/json",
                body: "{}".into(),
            },
            BootstrapError::Redirect,
        ),
        (
            Reply {
                status: 200,
                content_type: "text/plain",
                body: "secret".into(),
            },
            BootstrapError::NotJson,
        ),
        (
            Reply {
                status: 200,
                content_type: "application/json",
                body: "x".repeat(
                    crate::platform::theatre_registration::firebase_bootstrap::MAX_RESPONSE_BYTES
                        + 1,
                ),
            },
            BootstrapError::TooLarge,
        ),
        (
            Reply {
                status: 200,
                content_type: "application/json",
                body: "not-json-secret".into(),
            },
            BootstrapError::Protocol,
        ),
    ] {
        let (source, task) = source(vec![reply]).await;
        let error = source
            .token_at(&config(), Instant::now(), EPOCH)
            .await
            .err()
            .unwrap();
        assert_eq!(error, expected);
        assert!(!format!("{error:?}").contains("secret"));
        assert_eq!(task.await.unwrap().len(), 1);
    }
}

#[tokio::test]
async fn unresponsive_and_unreachable_endpoints_time_out_or_fail_without_a_fallback() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let hold = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        drop(socket);
    });
    let silent = AccessTokenSource::fixture(address, Duration::from_millis(100)).unwrap();
    assert_eq!(
        silent
            .token_at(&config(), Instant::now(), EPOCH)
            .await
            .err(),
        Some(BootstrapError::Timeout)
    );
    hold.abort();
    let dead = AccessTokenSource::fixture(address, Duration::from_millis(100)).unwrap();
    assert_eq!(
        dead.token_at(&config(), Instant::now(), EPOCH).await.err(),
        Some(BootstrapError::Network)
    );
}

fn identity() -> BootstrapIdentity {
    BootstrapIdentity::new(
        "dedicated-new-uid",
        Email::parse("Victim@example.test").unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn create_user_uses_the_freshly_minted_bearer_and_a_mint_failure_dispatches_nothing() {
    let (token_address, token_task) = server(vec![grant("ya29.for-create", 3599)]).await;
    let (api_address, api_task) = server(vec![ok(
        json!({"localId":"dedicated-new-uid","email":"Victim@example.test"}),
    )])
    .await;
    let client = FirebaseBootstrapClient::fixture_with_access(
        config(),
        api_address,
        AccessTokenSource::fixture(token_address, Duration::from_secs(1)).unwrap(),
        Duration::from_secs(1),
    )
    .unwrap();
    client.create_user(&identity()).await.unwrap();
    let api = api_task.await.unwrap();
    assert_eq!(api.len(), 1);
    assert_eq!(
        api[0].authorization.as_deref(),
        Some("Bearer ya29.for-create")
    );
    assert_eq!(token_task.await.unwrap().len(), 1);

    // Mint refused: typed pre-dispatch failure, and the Auth API is never contacted.
    let (token_address, token_task) =
        server(vec![error(400, json!({"error":"invalid_grant"}))]).await;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let api_address = listener.local_addr().unwrap();
    let client = FirebaseBootstrapClient::fixture_with_access(
        config(),
        api_address,
        AccessTokenSource::fixture(token_address, Duration::from_secs(1)).unwrap(),
        Duration::from_secs(1),
    )
    .unwrap();
    assert_eq!(
        client.create_user(&identity()).await.err(),
        Some(BootstrapFailure::BeforeDispatch(
            BootstrapError::AccessDenied
        ))
    );
    assert!(
        tokio::time::timeout(Duration::from_millis(100), listener.accept())
            .await
            .is_err()
    );
    token_task.await.unwrap();
}
