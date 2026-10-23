//! Handler-level caller authorization: a request that carries no OS peer credential is never
//! served, whatever else it contains. The process test covers real credentials end to end.
#![cfg(feature = "service")]

mod support;

use std::{sync::Arc, time::Duration};

use cantos_identity::rpc::generated::{
    identity_server::Identity, AuthenticatePasswordRequest, ClientContext,
    CompleteTheatreSignupRequest, GetSecurityStateRequest, Purpose, StartTheatreSignupRequest,
};
use cantos_identity::{
    application::{IdentityService, Limits, Settings},
    credentials::KdfPool,
    mail::{SignupLinkBase, UnconfiguredMailer},
    persistence::IdentityStore,
    rpc::{CallerPolicy, IdentityRpc, PolicyError},
};
use cantos_store_turso::{registry::Intent, testing::FixedClock};
use tonic::{Code, Request};

async fn rpc(run: &support::RunDir) -> IdentityRpc {
    let clock = FixedClock::new(1_700_000_000_000_000);
    let store = Arc::new(
        IdentityStore::open(run.path("a.db"), Intent::Migrate, clock.clone())
            .await
            .unwrap(),
    );
    let service = IdentityService::new(
        store,
        clock,
        Arc::new(UnconfiguredMailer),
        Settings {
            link: SignupLinkBase::parse("https://theatre.example/signup").unwrap(),
            kdf: KdfPool::new(1, 1, Duration::from_secs(1)),
            limits: Limits::standard(),
        },
    )
    .unwrap();
    IdentityRpc::new(Arc::new(service), CallerPolicy::new([1000]).unwrap())
}

fn client() -> Option<ClientContext> {
    Some(ClientContext {
        client_ip: "203.0.113.1".into(),
    })
}

fn code<T>(result: Result<T, tonic::Status>) -> Code {
    match result {
        Ok(_) => panic!("served without a peer credential"),
        Err(status) => status.code(),
    }
}

#[tokio::test]
async fn every_method_refuses_a_request_without_a_peer_credential() {
    let run = support::RunDir::new();
    let rpc = rpc(&run).await;
    assert_eq!(
        code(
            rpc.start_theatre_signup(Request::new(StartTheatreSignupRequest {
                operation_id: vec![1; 16],
                email: "a@example.com".into(),
                client: client(),
            }))
            .await
        ),
        Code::PermissionDenied
    );
    assert_eq!(
        code(
            rpc.complete_theatre_signup(Request::new(CompleteTheatreSignupRequest {
                command_id: vec![1; 16],
                operation_id: vec![1; 16],
                proof_id: vec![1; 16],
                proof_secret: vec![1; 32],
                password: "long enough password".into(),
                explicit_confirmation: true,
                client: client(),
            }))
            .await
        ),
        Code::PermissionDenied
    );
    assert_eq!(
        code(
            rpc.authenticate_password(Request::new(AuthenticatePasswordRequest {
                operation_id: vec![1; 16],
                purpose: Purpose::Theatre as i32,
                email: "a@example.com".into(),
                password: "long enough password".into(),
                client: client(),
            }))
            .await
        ),
        Code::PermissionDenied
    );
    assert_eq!(
        code(
            rpc.get_security_state(Request::new(GetSecurityStateRequest {
                subject_id: vec![1; 16]
            }))
            .await
        ),
        Code::PermissionDenied
    );
}

#[test]
fn the_caller_allowlist_is_never_empty_and_never_unbounded() {
    assert_eq!(CallerPolicy::new([]).unwrap_err(), PolicyError::Empty);
    assert_eq!(CallerPolicy::new(0..9).unwrap_err(), PolicyError::TooMany);
    let policy = CallerPolicy::new([0, 501]).unwrap();
    assert!(policy.permits_uid(501) && !policy.permits_uid(502));
}
