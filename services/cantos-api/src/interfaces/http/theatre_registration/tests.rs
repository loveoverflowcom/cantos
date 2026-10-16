//! Mounted production HTTP handlers with explicitly isolated application ports, no database,
//! real issuer, mail relay, provisioned account or authenticated session.
use std::{net::SocketAddr, sync::Arc, time::Duration};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;

use crate::{
    interfaces::http::{router, AppState},
    platform::{
        theatre_identity::TheatreIdentity,
        theatre_registration::{
            application::test_support::{fixture_store, token, Harness},
            domain::{MailKind, NetworkKey, RegistrationError},
            runtime::{AttemptPolicy, RegistrationRuntime},
        },
    },
};

const ORIGIN: &str = "https://cantos.example";
const BASE: &str = "/api/v1/theatre/auth/registration";

fn fixture(
    harness: Option<&Harness>,
    identity: bool,
) -> (Router, Option<Arc<RegistrationRuntime>>) {
    let mut store = fixture_store();
    if identity {
        store = store.with_theatre_identity(
            TheatreIdentity::new(
                "https://issuer.example/oauth2/openid/cantos-theatre-web-bff",
                ORIGIN,
                "isolated-fixture".into(),
            )
            .unwrap(),
        );
    }
    let runtime = harness.map(|harness| {
        Arc::new(
            RegistrationRuntime::new(
                Arc::new(harness.service()),
                ORIGIN,
                [1; 32],
                4,
                AttemptPolicy::new(60, 100, 100).unwrap(),
            )
            .unwrap(),
        )
    });
    store.theatre_registration = runtime.clone();
    (
        router(
            AppState {
                store,
                web_origin: ORIGIN.into(),
            },
            "/tmp/cantos-registration-no-static-fixture",
        ),
        runtime,
    )
}

fn request(method: Method, path: &str, value: Value, with_peer: bool) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap();
    if with_peer {
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 1], 4321))));
    }
    request
}

async fn receipt(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    assert!(response.headers().get(header::SET_COOKIE).is_none());
    assert!(response.headers().get(header::LOCATION).is_none());
    assert!(response.headers().get("x-cantos-theatre-session").is_none());
    assert_eq!(
        response.headers().get(header::CACHE_CONTROL).unwrap(),
        "no-store"
    );
    assert_eq!(
        response.headers().get(header::REFERRER_POLICY).unwrap(),
        "no-referrer"
    );
    let status = response.status();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap()
    };
    (status, value)
}

#[tokio::test]
async fn mounted_flow_verifies_email_then_emails_issuer_setup_and_never_issues_a_session() {
    let h = Harness::new();
    let (app, _) = fixture(Some(&h), true);
    let (status, body) = receipt(
        app.clone(),
        request(
            Method::POST,
            BASE,
            json!({"email":"Reader@example.com"}),
            true,
        ),
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::ACCEPTED, json!({"status":"accepted"}))
    );
    assert_eq!(h.count("create"), 0);
    assert_eq!(h.count("setup"), 0);
    let mail = tokio::time::timeout(
        Duration::from_secs(1),
        h.wait_for_mail(MailKind::Verification),
    )
    .await
    .unwrap();
    assert_eq!(mail.recipient(), "Reader@example.com");
    let (status, body) = receipt(
        app.clone(),
        request(
            Method::POST,
            &format!("{BASE}/verify"),
            json!({"token":token(&mail)}),
            true,
        ),
    )
    .await;
    assert_eq!(
        (status, body),
        (StatusCode::OK, json!({"status":"setup_link_sent"}))
    );
    assert_eq!(h.count("create"), 1);
    assert_eq!(h.count("setup"), 1);
    let setup = h.captured_mail(MailKind::CredentialSetup).unwrap();
    assert_eq!(setup.recipient(), "Reader@example.com");
    assert_eq!(
        setup.link(),
        "https://issuer.example/ui/reset?token=fixture"
    );
    let replay = receipt(
        app.clone(),
        request(
            Method::POST,
            &format!("{BASE}/verify"),
            json!({"token":token(&mail)}),
            true,
        ),
    )
    .await;
    assert_eq!(replay, (StatusCode::OK, json!({"status":"unavailable"})));
    assert_eq!(h.count("create"), 1);
    assert_eq!(h.count("setup"), 1);
    for path in ["/api/v1/theatre/session", "/api/v1/studio/works"] {
        let (status, _) = receipt(app.clone(), request(Method::GET, path, Value::Null, true)).await;
        assert_eq!(
            status,
            StatusCode::UNAUTHORIZED,
            "registration cannot authenticate {path}"
        );
    }
}

#[tokio::test]
async fn first_duplicate_completed_and_delivery_failure_starts_have_the_same_public_receipt() {
    for delivery_error in [
        None,
        Some(RegistrationError::DeliveryRejected),
        Some(RegistrationError::DeliveryUncertain),
    ] {
        let mut h = Harness::new();
        Arc::get_mut(&mut h.mailer).unwrap().verification_error = delivery_error;
        let (app, _) = fixture(Some(&h), true);
        let first = receipt(
            app.clone(),
            request(
                Method::POST,
                BASE,
                json!({"email":"Reader@example.com"}),
                true,
            ),
        )
        .await;
        let duplicate = receipt(
            app.clone(),
            request(
                Method::POST,
                BASE,
                json!({"email":"reader@example.com"}),
                true,
            ),
        )
        .await;
        assert_eq!(first, (StatusCode::ACCEPTED, json!({"status":"accepted"})));
        assert_eq!(first, duplicate);
        let mail = tokio::time::timeout(
            Duration::from_secs(1),
            h.wait_for_mail(MailKind::Verification),
        )
        .await
        .unwrap();
        assert_eq!(h.count("verification_mail"), 1);
        assert_eq!(h.count("create"), 0);
        let verified = receipt(
            app.clone(),
            request(
                Method::POST,
                &format!("{BASE}/verify"),
                json!({"token":token(&mail)}),
                true,
            ),
        )
        .await;
        assert_eq!(verified.0, StatusCode::OK);
        let completed = receipt(
            app.clone(),
            request(
                Method::POST,
                BASE,
                json!({"email":"Reader@example.com"}),
                true,
            ),
        )
        .await;
        assert_eq!(first, completed);
        assert_eq!(h.count("create"), 1);
    }
}

#[tokio::test]
async fn cancel_invalidates_the_unconsumed_proof_without_touching_an_account() {
    let h = Harness::new();
    let (app, _) = fixture(Some(&h), true);
    receipt(
        app.clone(),
        request(
            Method::POST,
            BASE,
            json!({"email":"Reader@example.com"}),
            true,
        ),
    )
    .await;
    let mail = tokio::time::timeout(
        Duration::from_secs(1),
        h.wait_for_mail(MailKind::Verification),
    )
    .await
    .unwrap();
    for _ in 0..2 {
        let cancelled = receipt(
            app.clone(),
            request(
                Method::POST,
                &format!("{BASE}/cancel"),
                json!({"token":token(&mail)}),
                true,
            ),
        )
        .await;
        assert_eq!(
            cancelled,
            (StatusCode::ACCEPTED, json!({"status":"accepted"}))
        );
    }
    let verified = receipt(
        app.clone(),
        request(
            Method::POST,
            &format!("{BASE}/verify"),
            json!({"token":token(&mail)}),
            true,
        ),
    )
    .await;
    assert_eq!(verified, (StatusCode::OK, json!({"status":"unavailable"})));
    assert_eq!(h.count("create"), 0);
    assert_eq!(h.count("setup"), 0);
}

#[tokio::test]
async fn disabled_registration_or_absent_theatre_identity_never_dispatches_onboarding() {
    let h = Harness::new();
    for (runtime, identity) in [(None, true), (Some(&h), false), (None, false)] {
        let (app, _) = fixture(runtime, identity);
        for suffix in ["", "/verify", "/cancel"] {
            let response = receipt(
                app.clone(),
                request(
                    Method::POST,
                    &format!("{BASE}{suffix}"),
                    json!({"token":"a".repeat(64), "email":"Reader@example.com"}),
                    true,
                ),
            )
            .await;
            assert_eq!(response.0, StatusCode::NOT_FOUND);
        }
    }
    assert!(h.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn same_origin_and_fetch_metadata_are_enforced_before_any_external_effect() {
    let h = Harness::new();
    let (app, _) = fixture(Some(&h), true);
    for suffix in ["", "/verify", "/cancel"] {
        for mode in ["missing", "foreign", "cross-site"] {
            let mut req = request(
                Method::POST,
                &format!("{BASE}{suffix}"),
                if suffix.is_empty() {
                    json!({"email":"Reader@example.com"})
                } else {
                    json!({"token":"a".repeat(64)})
                },
                true,
            );
            match mode {
                "missing" => {
                    req.headers_mut().remove(header::ORIGIN);
                }
                "foreign" => {
                    req.headers_mut()
                        .insert(header::ORIGIN, "https://other.example".parse().unwrap());
                }
                "cross-site" => {
                    req.headers_mut()
                        .insert("sec-fetch-site", "cross-site".parse().unwrap());
                }
                _ => unreachable!(),
            }
            assert_eq!(receipt(app.clone(), req).await.0, StatusCode::FORBIDDEN);
        }
    }
    assert!(h.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn untrusted_body_fields_cannot_select_a_person_provider_or_password() {
    let h = Harness::new();
    let (app, _) = fixture(Some(&h), true);
    for body in [
        json!({}),
        json!({"email":"not-an-email"}),
        json!({"email":"Reader@example.com","person_id":"studio"}),
        json!({"email":"Reader@example.com","password":"forbidden"}),
        json!({"email":"Reader@example.com","issuer":"https://other.example"}),
        json!({"email":"x".repeat(3000)}),
    ] {
        assert_eq!(
            receipt(app.clone(), request(Method::POST, BASE, body, true))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
    }
    for suffix in ["/verify", "/cancel"] {
        let injected = json!({"token":"a".repeat(64),"person_id":"studio"});
        assert_eq!(
            receipt(
                app.clone(),
                request(Method::POST, &format!("{BASE}{suffix}"), injected, true)
            )
            .await
            .0,
            StatusCode::BAD_REQUEST
        );
    }
    assert_eq!(h.count("create"), 0);
    assert_eq!(h.count("verification_mail"), 0);
}

#[tokio::test]
async fn get_never_activates_an_email_proof_and_malformed_tokens_do_not_dispatch() {
    let h = Harness::new();
    let (app, _) = fixture(Some(&h), true);
    for suffix in ["", "/verify", "/cancel"] {
        assert_eq!(
            receipt(
                app.clone(),
                request(Method::GET, &format!("{BASE}{suffix}"), Value::Null, true)
            )
            .await
            .0,
            StatusCode::METHOD_NOT_ALLOWED
        );
    }
    for proof in ["bad".to_owned(), "a".repeat(64), "A".repeat(64)] {
        let response = receipt(
            app.clone(),
            request(
                Method::POST,
                &format!("{BASE}/verify"),
                json!({"token":proof}),
                true,
            ),
        )
        .await;
        assert_eq!(response, (StatusCode::OK, json!({"status":"unavailable"})));
    }
    assert!(h.events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn absent_socket_peer_cannot_be_replaced_by_forwarded_headers_or_json() {
    let h = Harness::new();
    let (app, runtime) = fixture(Some(&h), true);
    for suffix in ["", "/verify", "/cancel"] {
        let mut req = request(
            Method::POST,
            &format!("{BASE}{suffix}"),
            if suffix.is_empty() {
                json!({"email":"Reader@example.com"})
            } else {
                json!({"token":"a".repeat(64)})
            },
            false,
        );
        req.headers_mut()
            .insert("x-forwarded-for", "192.0.2.1".parse().unwrap());
        req.headers_mut()
            .insert("forwarded", "for=192.0.2.1".parse().unwrap());
        assert_eq!(
            receipt(app.clone(), req).await.0,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    assert!(h.events.lock().unwrap().is_empty());
    assert_eq!(
        runtime
            .unwrap()
            .network_key("192.0.2.1".parse().unwrap())
            .unwrap(),
        NetworkKey::from_peer("192.0.2.1".parse().unwrap(), &[1; 32]).unwrap()
    );
}
