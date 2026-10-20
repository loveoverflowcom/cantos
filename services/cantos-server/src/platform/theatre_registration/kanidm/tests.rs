//! Isolated HTTP doubles for the production adapter, never a Kanidm live claim.
use std::{collections::VecDeque, sync::Arc};

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Request, Response},
    routing::any,
    Router,
};
use serde_json::{json, Value};
use tokio::{net::TcpListener, sync::Mutex, task::JoinHandle};
use uuid::Uuid;

use super::*;
use crate::platform::theatre_registration::domain::Email;

const ISSUER: &str = "https://idm.example/oauth2/openid/cantos-theatre-web-bff";
const TOKEN: &str = "fixture-service-bearer-never-live";

#[derive(Debug)]
struct Capture {
    method: String,
    path: String,
    authorization: String,
    cache_control: String,
    body: Value,
}

struct Reply {
    status: StatusCode,
    body: String,
    content_type: &'static str,
    location: Option<String>,
}
impl Reply {
    fn json(status: StatusCode, value: Value) -> Self {
        Self {
            status,
            body: value.to_string(),
            content_type: "application/json",
            location: None,
        }
    }
}

#[derive(Clone)]
struct FixtureState {
    calls: Arc<Mutex<Vec<Capture>>>,
    replies: Arc<Mutex<VecDeque<Reply>>>,
}
struct Fixture {
    issuer: KanidmIssuer,
    calls: Arc<Mutex<Vec<Capture>>>,
    task: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn handler(State(state): State<FixtureState>, request: Request<Body>) -> Response<Body> {
    let (parts, body) = request.into_parts();
    let body = to_bytes(body, 16_384).await.unwrap();
    state.calls.lock().await.push(Capture {
        method: parts.method.to_string(),
        path: parts.uri.to_string(),
        authorization: parts.headers[header::AUTHORIZATION]
            .to_str()
            .unwrap()
            .to_owned(),
        cache_control: parts.headers[header::CACHE_CONTROL]
            .to_str()
            .unwrap()
            .to_owned(),
        body: if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        },
    });
    let reply = state.replies.lock().await.pop_front().unwrap();
    let mut builder = Response::builder()
        .status(reply.status)
        .header(header::CONTENT_TYPE, reply.content_type);
    if let Some(location) = reply.location {
        builder = builder.header(header::LOCATION, location);
    }
    builder.body(Body::from(reply.body)).unwrap()
}

async fn fixture(replies: Vec<Reply>) -> Fixture {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let calls = Arc::new(Mutex::new(Vec::new()));
    let state = FixtureState {
        calls: calls.clone(),
        replies: Arc::new(Mutex::new(replies.into())),
    };
    let router = Router::new().fallback(any(handler)).with_state(state);
    let task = tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    let mut issuer =
        KanidmIssuer::new(KanidmConfig::new(ISSUER, TOKEN.into(), 600).unwrap()).unwrap();
    // Test-only transport substitution preserves the production HTTPS reset origin.
    // Neither production configuration nor applicant input can set api_origin.
    issuer.api_origin = Url::parse(&format!("http://{address}/")).unwrap();
    Fixture {
        issuer,
        calls,
        task,
    }
}

fn pending() -> PendingRegistration {
    PendingRegistration::new(
        Uuid::parse_str("4a455a1e-9c3c-4328-8cf2-c00ad8c99c68").unwrap(),
        Email::parse("nguoi-nghe@example.test").unwrap(),
    )
    .unwrap()
}

#[test]
fn configuration_requires_exact_https_theatre_issuer_and_explicit_provider_supported_ttl() {
    for issuer in [
        "http://127.0.0.1:8443/oauth2/openid/cantos-theatre-web-bff",
        "https://idm.example/oauth2/openid/cantos-studio-web-bff",
        "https://idm.example/oauth2/openid/cantos-theatre-web-bff/",
        "https://idm.example/oauth2/openid/cantos-theatre-web-bff?redirect=elsewhere",
        "https://idm.example/oauth2/openid/cantos-theatre-web-bff#fragment",
        "https://user:password@idm.example/oauth2/openid/cantos-theatre-web-bff",
        "https://idm.example/oauth2/openid/../openid/cantos-theatre-web-bff",
    ] {
        assert_eq!(
            KanidmConfig::new(issuer, TOKEN.into(), 600).err(),
            Some(RegistrationError::Configuration)
        );
    }
    for ttl in [0, 299, 86_401, u32::MAX] {
        assert_eq!(
            KanidmConfig::new(ISSUER, TOKEN.into(), ttl).err(),
            Some(RegistrationError::Configuration)
        );
    }
    for token in ["", "secret\r\nHeader: value", "secret with spaces"] {
        assert_eq!(
            KanidmConfig::new(ISSUER, token.into(), 600).err(),
            Some(RegistrationError::Configuration)
        );
    }
    for ttl in [300, 86_400] {
        assert!(KanidmConfig::new(ISSUER, TOKEN.into(), ttl).is_ok());
    }
}

#[tokio::test]
async fn successful_create_is_atomic_generated_uuid_name_and_verified_mail_without_grants_or_password(
) {
    let fixture = fixture(vec![Reply::json(StatusCode::OK, Value::Null)]).await;
    let registration = pending();
    let person = fixture
        .issuer
        .create_new(registration.clone())
        .await
        .unwrap();
    assert_eq!(person.registration_id(), registration.id());
    assert_eq!(person.subject(), registration.id());
    let calls = fixture.calls.lock().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "POST");
    assert_eq!(calls[0].path, "/v1/person");
    assert_eq!(calls[0].authorization, format!("Bearer {TOKEN}"));
    assert_eq!(calls[0].cache_control, "no-store");
    assert_eq!(
        calls[0].body,
        json!({"attrs": {
            "uuid": ["4a455a1e-9c3c-4328-8cf2-c00ad8c99c68"],
            "name": ["cantos_4a455a1e9c3c43288cf2c00ad8c99c68"],
            "displayname": ["Cantos listener"],
            "mail": ["nguoi-nghe@example.test"]
        }})
    );
}

#[tokio::test]
async fn create_rejection_or_conflict_cannot_discover_or_reset_a_preexisting_studio_person() {
    for status in [
        StatusCode::FORBIDDEN,
        StatusCode::CONFLICT,
        StatusCode::BAD_REQUEST,
    ] {
        let fixture = fixture(vec![Reply::json(
            status,
            json!({"existing_person": "studio-admin"}),
        )])
        .await;
        assert_eq!(
            fixture.issuer.create_new(pending()).await.err(),
            Some(RegistrationError::IssuerRejected)
        );
        let calls = fixture.calls.lock().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].path, "/v1/person");
        assert_eq!(calls[0].method, "POST");
    }
}

#[tokio::test]
async fn unknown_create_response_never_becomes_creation_evidence_or_retries() {
    for reply in [
        Reply::json(StatusCode::OK, json!({"uuid": pending().id().to_string()})),
        Reply::json(StatusCode::CREATED, Value::Null),
        Reply::json(StatusCode::SERVICE_UNAVAILABLE, Value::Null),
        Reply::json(StatusCode::REQUEST_TIMEOUT, Value::Null),
        Reply {
            status: StatusCode::OK,
            body: "<html>null</html>".into(),
            content_type: "text/html",
            location: None,
        },
        Reply {
            status: StatusCode::OK,
            body: " ".repeat(MAX_RESPONSE_BYTES + 1),
            content_type: "application/json",
            location: None,
        },
        Reply {
            status: StatusCode::TEMPORARY_REDIRECT,
            body: "null".into(),
            content_type: "application/json",
            location: Some("https://other.example/v1/person".into()),
        },
    ] {
        let fixture = fixture(vec![reply]).await;
        assert_eq!(
            fixture.issuer.create_new(pending()).await.err(),
            Some(RegistrationError::IssuerUncertain)
        );
        assert_eq!(fixture.calls.lock().await.len(), 1);
    }
}

#[tokio::test]
async fn setup_intent_targets_only_the_confirmed_new_uuid_and_keeps_setup_on_the_pinned_origin() {
    let fixture = fixture(vec![Reply::json(
        StatusCode::OK,
        json!({"token": "abcde-fghjk-pqrst-uvwxy", "expiry_time": 700}),
    )])
    .await;
    let id = pending().id();
    let person = CreatedPerson::confirmed(id, id).unwrap();
    let intent = fixture.issuer.request_intent(person).await.unwrap();
    let link = fixture.issuer.setup_link(intent, 100).unwrap();
    assert_eq!(
        link.as_str(),
        "https://idm.example/ui/reset?token=abcde-fghjk-pqrst-uvwxy"
    );
    assert_eq!(format!("{link:?}"), "SetupLink([REDACTED])");
    let calls = fixture.calls.lock().await;
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].method, "GET");
    assert_eq!(
        calls[0].path,
        "/v1/person/4a455a1e-9c3c-4328-8cf2-c00ad8c99c68/_credential/_update_intent/600"
    );
    assert_eq!(calls[0].body, Value::Null);
    assert_eq!(calls[0].cache_control, "no-store");
}

#[tokio::test]
async fn failed_or_unknown_setup_intent_is_not_repeated_or_replaced_with_a_different_person() {
    let id = pending().id();
    for (reply, error) in [
        (
            Reply::json(StatusCode::FORBIDDEN, Value::Null),
            RegistrationError::IssuerRejected,
        ),
        (
            Reply::json(StatusCode::SERVICE_UNAVAILABLE, Value::Null),
            RegistrationError::IssuerUncertain,
        ),
        (
            Reply::json(StatusCode::OK, json!({"token": "fixture-without-expiry"})),
            RegistrationError::IssuerUncertain,
        ),
        (
            Reply::json(
                StatusCode::OK,
                json!({"token": "fixture", "expiry_time": 200, "redirect": "https://other.example"}),
            ),
            RegistrationError::IssuerUncertain,
        ),
    ] {
        let fixture = fixture(vec![reply]).await;
        let person = CreatedPerson::confirmed(id, id).unwrap();
        assert_eq!(
            fixture.issuer.request_intent(person).await.err(),
            Some(error)
        );
        let calls = fixture.calls.lock().await;
        assert_eq!(calls.len(), 1);
        assert_eq!(
            calls[0].path,
            "/v1/person/4a455a1e-9c3c-4328-8cf2-c00ad8c99c68/_credential/_update_intent/600"
        );
    }
}

#[test]
fn setup_link_rejects_expired_excess_lifetime_or_malformed_intents_and_encodes_the_token_as_data() {
    let issuer = KanidmIssuer::new(KanidmConfig::new(ISSUER, TOKEN.into(), 600).unwrap()).unwrap();
    for (token, expires) in [
        ("token", 100),
        ("token", 99),
        ("token", 701),
        ("", 200),
        ("token\n", 200),
    ] {
        assert_eq!(
            issuer
                .setup_link(
                    Intent {
                        token: token.into(),
                        expiry_time: expires
                    },
                    100
                )
                .err(),
            Some(RegistrationError::IssuerUncertain)
        );
    }
    let link = issuer
        .setup_link(
            Intent {
                token: "https://other.example/?x=y&z=1#part".into(),
                expiry_time: 200,
            },
            100,
        )
        .unwrap();
    let parsed = Url::parse(link.as_str()).unwrap();
    assert_eq!(parsed.origin().ascii_serialization(), "https://idm.example");
    assert_eq!(parsed.path(), "/ui/reset");
    assert_eq!(
        parsed.query_pairs().collect::<Vec<_>>(),
        vec![("token".into(), "https://other.example/?x=y&z=1#part".into())]
    );
    assert_eq!(parsed.fragment(), None);
}

#[tokio::test]
async fn lost_create_response_is_uncertain_without_a_second_http_request() {
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let server = tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut bytes = vec![0; 16_384];
        let count = stream.read(&mut bytes).await.unwrap();
        assert!(std::str::from_utf8(&bytes[..count])
            .unwrap()
            .starts_with("POST /v1/person HTTP/1.1"));
        // Simulate a committed creation whose response body was lost in transit.
        stream.write_all(b"HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 4\r\nConnection: close\r\n\r\n").await.unwrap();
        stream.shutdown().await.unwrap();
        assert!(
            tokio::time::timeout(Duration::from_millis(100), listener.accept())
                .await
                .is_err()
        );
    });
    let mut issuer =
        KanidmIssuer::new(KanidmConfig::new(ISSUER, TOKEN.into(), 600).unwrap()).unwrap();
    issuer.api_origin = Url::parse(&format!("http://{address}/")).unwrap();
    assert_eq!(
        issuer.create_new(pending()).await.err(),
        Some(RegistrationError::IssuerUncertain)
    );
    server.await.unwrap();
}
