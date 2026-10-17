//! Mounted default-selected Firebase handlers, production orchestration and real action
//! HTTP client. Bootstrap/Kanidm and repository are isolated doubles, never live acceptance.
use std::{
    net::SocketAddr,
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::Body,
    extract::ConnectInfo,
    http::{header, Method, Request, StatusCode},
    Router,
};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use crate::{
    interfaces::http::{router, AppState},
    platform::{
        theatre_identity::TheatreIdentity,
        theatre_registration::{
            application::test_support::fixture_store,
            domain::{PeerPolicy, TrustedProxies},
            firebase_enrollment::{
                domain::Phase,
                runtime::FirebaseEnrollmentRuntime,
                tests::{self as support, ActionServer, Events, Memory},
            },
            runtime::AttemptPolicy,
        },
    },
};

const ORIGIN: &str = "https://cantos.example";
const BASE: &str = "/api/v1/theatre/auth/registration";
const CONFIRM: &str = "/api/v1/theatre/auth/registration/firebase/confirm";
const CANCEL: &str = "/api/v1/theatre/auth/registration/firebase/cancel";

async fn fixture(awaiting: bool) -> (Router, Arc<Memory>, ActionServer, Events) {
    fixture_with_peers(awaiting, PeerPolicy::DirectPeer, 100).await
}

async fn fixture_with_peers(
    awaiting: bool,
    peers: PeerPolicy,
    per_network: u32,
) -> (Router, Arc<Memory>, ActionServer, Events) {
    let events = Arc::new(Mutex::new(Vec::new()));
    let memory = Arc::new(if awaiting {
        Memory::awaiting(events.clone())
    } else {
        Memory::new(events.clone())
    });
    let server = ActionServer::start(memory.clone(), None).await;
    let service = support::service(memory.clone(), &server, None, None);
    let store = fixture_store()
        .with_theatre_identity(
            TheatreIdentity::new(
                "https://issuer.example/oauth2/openid/cantos-theatre-web-bff",
                ORIGIN,
                "isolated-fixture".into(),
            )
            .unwrap(),
        )
        .with_theatre_firebase_enrollment(
            FirebaseEnrollmentRuntime::new(
                service,
                ORIGIN,
                [1; 32],
                4,
                AttemptPolicy::new(60, 100, per_network).unwrap(),
            )
            .unwrap()
            .with_peer_policy(peers),
        );
    (
        router(
            AppState {
                store,
                web_origin: ORIGIN.into(),
            },
            "/tmp/cantos-firebase-no-static-fixture",
        ),
        memory,
        server,
        events,
    )
}

fn request(method: Method, path: &str, value: Value, peer: bool) -> Request<Body> {
    let mut request = Request::builder()
        .method(method)
        .uri(path)
        .header(header::ORIGIN, ORIGIN)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(serde_json::to_vec(&value).unwrap()))
        .unwrap();
    if peer {
        request
            .extensions_mut()
            .insert(ConnectInfo(SocketAddr::from(([192, 0, 2, 1], 4321))));
    }
    request
}

async fn receipt(app: Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.oneshot(request).await.unwrap();
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    assert!(!response.headers().contains_key(header::LOCATION));
    assert!(!response.headers().contains_key("x-cantos-theatre-session"));
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert_eq!(response.headers()[header::REFERRER_POLICY], "no-referrer");
    let status = response.status();
    let body = response.into_body().collect().await.unwrap().to_bytes();
    (
        status,
        if body.is_empty() {
            Value::Null
        } else {
            serde_json::from_slice(&body).unwrap()
        },
    )
}

fn action() -> Value {
    json!({"operation_id":Uuid::from_u128(1).to_string(),"oob_code":"actual-action"})
}

#[tokio::test]
async fn firebase_capability_and_start_receipts_disclose_no_operation_setup_or_session() {
    let (app, memory, server, events) = fixture(false).await;
    let configuration = receipt(
        app.clone(),
        request(
            Method::GET,
            "/api/v1/theatre/auth/configuration",
            Value::Null,
            false,
        ),
    )
    .await;
    assert_eq!(
        configuration,
        (
            StatusCode::OK,
            json!({"login":"oidc","registration":{"status":"firebase_email_verification"}})
        )
    );
    for _ in 0..2 {
        let started = receipt(
            app.clone(),
            request(
                Method::POST,
                BASE,
                json!({"email":"Victim@example.test"}),
                true,
            ),
        )
        .await;
        assert_eq!(
            started,
            (StatusCode::ACCEPTED, json!({"status":"accepted"}))
        );
    }
    tokio::time::timeout(Duration::from_secs(1), async {
        while memory.phase() != Some(Phase::AwaitingAction) {
            tokio::task::yield_now().await;
        }
    })
    .await
    .unwrap();
    assert!(memory.enrollment().is_some());
    assert!(server.bodies.lock().unwrap().is_empty());
    assert!(!events.lock().unwrap().contains(&"create"));
    assert!(!events.lock().unwrap().contains(&"setup"));
}

#[tokio::test]
async fn explicit_firebase_action_delivers_setup_once_and_authenticates_no_surface() {
    let (app, memory, server, events) = fixture(true).await;
    let confirmed = receipt(app.clone(), request(Method::POST, CONFIRM, action(), true)).await;
    assert_eq!(
        confirmed,
        (
            StatusCode::OK,
            json!({"status":"setup_ready","login":"cantos_00000000000000000000000000000001","setup_url":"https://issuer.example/ui/reset?token=fixture"})
        )
    );
    assert_eq!(memory.phase(), Some(Phase::HandedOff));
    let replay = receipt(app.clone(), request(Method::POST, CONFIRM, action(), true)).await;
    assert_eq!(replay, (StatusCode::OK, json!({"status":"unavailable"})));
    for path in ["/api/v1/theatre/session", "/api/v1/studio/works"] {
        let denied = receipt(app.clone(), request(Method::GET, path, Value::Null, true)).await;
        assert!(
            matches!(
                denied.0,
                StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN | StatusCode::NOT_FOUND
            ),
            "registration must not authenticate {path}"
        );
    }
    // This fixture deliberately has no dedicated Admin capability configured. Its exact
    // unavailable response proves no activation, not live Admin authorization acceptance.
    let admin = receipt(
        app.clone(),
        request(Method::GET, "/api/v1/admin/capabilities", Value::Null, true),
    )
    .await;
    assert_eq!(admin.0, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(admin.1["code"], "unavailable");
    for suffix in ["/verify", "/cancel"] {
        assert_eq!(
            receipt(
                app.clone(),
                request(
                    Method::POST,
                    &format!("{BASE}{suffix}"),
                    json!({"token":"a".repeat(64)}),
                    true
                )
            )
            .await
            .0,
            StatusCode::NOT_FOUND
        );
    }
    assert_eq!(server.bodies.lock().unwrap().len(), 2);
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| **event == "create")
            .count(),
        1
    );
    assert_eq!(
        events
            .lock()
            .unwrap()
            .iter()
            .filter(|event| **event == "setup")
            .count(),
        1
    );
}

#[tokio::test]
async fn get_scanners_and_untrusted_origin_peer_or_body_cannot_dispatch_firebase_effects() {
    let (app, memory, server, events) = fixture(true).await;
    for path in [BASE, CONFIRM, CANCEL] {
        assert_eq!(
            receipt(app.clone(), request(Method::GET, path, Value::Null, true))
                .await
                .0,
            StatusCode::METHOD_NOT_ALLOWED
        );
        for mode in ["missing-origin", "foreign-origin", "cross-site", "no-peer"] {
            let body = if path == BASE {
                json!({"email":"Victim@example.test"})
            } else {
                action()
            };
            let mut req = request(Method::POST, path, body, mode != "no-peer");
            match mode {
                "missing-origin" => {
                    req.headers_mut().remove(header::ORIGIN);
                }
                "foreign-origin" => {
                    req.headers_mut()
                        .insert(header::ORIGIN, "https://foreign.example".parse().unwrap());
                }
                "cross-site" => {
                    req.headers_mut()
                        .insert("sec-fetch-site", "cross-site".parse().unwrap());
                }
                "no-peer" => {
                    req.headers_mut()
                        .insert("x-forwarded-for", "192.0.2.1".parse().unwrap());
                }
                _ => unreachable!(),
            }
            let expected = if mode == "no-peer" {
                StatusCode::SERVICE_UNAVAILABLE
            } else {
                StatusCode::FORBIDDEN
            };
            assert_eq!(receipt(app.clone(), req).await.0, expected);
        }
    }
    for path in [CONFIRM, CANCEL] {
        for body in [
            json!({}),
            json!({"operation_id":"invalid","oob_code":"code"}),
            json!({"operation_id":Uuid::nil().to_string(),"oob_code":"code"}),
            json!({"operation_id":Uuid::from_u128(1).to_string(),"oob_code":"code","person_id":"studio"}),
            json!({"operation_id":Uuid::from_u128(1).to_string(),"oob_code":"code","password":"forbidden"}),
        ] {
            assert_eq!(
                receipt(app.clone(), request(Method::POST, path, body, true))
                    .await
                    .0,
                StatusCode::BAD_REQUEST
            );
        }
    }
    assert_eq!(memory.phase(), Some(Phase::AwaitingAction));
    assert!(server.bodies.lock().unwrap().is_empty());
    assert!(events.lock().unwrap().is_empty());
}

#[tokio::test]
async fn action_bound_cancel_returns_generic_receipt_and_never_provisions() {
    let (app, memory, server, events) = fixture(true).await;
    assert_eq!(
        receipt(app.clone(), request(Method::POST, CANCEL, action(), true)).await,
        (StatusCode::ACCEPTED, json!({"status":"accepted"}))
    );
    assert_eq!(memory.phase(), Some(Phase::Cancelled));
    assert_eq!(
        receipt(app.clone(), request(Method::POST, CONFIRM, action(), true)).await,
        (StatusCode::OK, json!({"status":"unavailable"}))
    );
    assert_eq!(
        receipt(app.clone(), request(Method::POST, CANCEL, action(), true)).await,
        (StatusCode::ACCEPTED, json!({"status":"accepted"}))
    );
    assert_eq!(server.bodies.lock().unwrap().len(), 2);
    assert!(!events.lock().unwrap().contains(&"create"));
    assert!(!events.lock().unwrap().contains(&"setup"));
}

fn unknown_operation() -> Value {
    json!({"operation_id":Uuid::from_u128(99).to_string(),"oob_code":"code"})
}

fn from_peer(peer: [u8; 4], forwarded: &[&str]) -> Request<Body> {
    let mut req = request(Method::POST, CONFIRM, unknown_operation(), false);
    req.extensions_mut()
        .insert(ConnectInfo(SocketAddr::from((peer, 4321))));
    for line in forwarded {
        req.headers_mut()
            .append("x-forwarded-for", line.parse().unwrap());
    }
    req
}

#[tokio::test]
async fn trusted_edge_partitions_quota_by_client_while_direct_access_cannot_spoof_it() {
    let edge = PeerPolicy::TrustedProxy(TrustedProxies::parse("10.89.0.0/24").unwrap());
    let (app, ..) = fixture_with_peers(false, edge, 1).await;
    let admitted = (StatusCode::OK, json!({"status":"unavailable"}));
    let refused = |app: Router, peer, lines: &[&str]| {
        let req = from_peer(peer, lines);
        async move { receipt(app, req).await.0 }
    };
    // Two users behind the same edge address have separate budgets.
    assert_eq!(
        receipt(app.clone(), from_peer([10, 89, 0, 2], &["198.51.100.1"])).await,
        admitted
    );
    assert_eq!(
        refused(app.clone(), [10, 89, 0, 2], &["198.51.100.1"]).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    assert_eq!(
        receipt(app.clone(), from_peer([10, 89, 0, 2], &["203.0.113.8"])).await,
        admitted
    );
    // A client-written left entry cannot choose the identity the edge appended.
    assert_eq!(
        refused(app.clone(), [10, 89, 0, 2], &["9.9.9.9, 198.51.100.1"]).await,
        StatusCode::SERVICE_UNAVAILABLE
    );
    // The edge without exactly one usable header fails closed instead of pooling users.
    for lines in [&[][..], &["198.51.100.1", "203.0.113.8"][..], &["junk"][..]] {
        assert_eq!(
            refused(app.clone(), [10, 89, 0, 2], lines).await,
            StatusCode::SERVICE_UNAVAILABLE
        );
    }
    // Direct API access is its own identity: rotating the header buys no new budget.
    assert_eq!(
        receipt(app.clone(), from_peer([192, 0, 2, 7], &["198.51.100.50"])).await,
        admitted
    );
    for spoof in ["198.51.100.51", "203.0.113.99", "10.89.0.2"] {
        assert_eq!(
            refused(app.clone(), [192, 0, 2, 7], &[spoof]).await,
            StatusCode::SERVICE_UNAVAILABLE,
            "{spoof}"
        );
    }
}

#[tokio::test]
async fn direct_peer_policy_ignores_forwarded_headers_entirely() {
    let (app, ..) = fixture_with_peers(false, PeerPolicy::DirectPeer, 1).await;
    assert_eq!(
        receipt(app.clone(), from_peer([192, 0, 2, 7], &["198.51.100.1"]))
            .await
            .0,
        StatusCode::OK
    );
    assert_eq!(
        receipt(app, from_peer([192, 0, 2, 7], &["203.0.113.8"]))
            .await
            .0,
        StatusCode::SERVICE_UNAVAILABLE
    );
}
