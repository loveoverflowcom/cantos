//! Test-harness-only: a scripted `cantos.identity.v1` server on a Unix socket. It exists to
//! inject answers the real executable never gives (a stale fact, a mismatched operation, a
//! restored epoch, a slow reply). Evidence from it is mock-integration, never a login proof.
#![allow(dead_code)]

/// The scripted peer's own server-side binding of Identity's proto (`tools/proto-codegen`),
/// independent of the Server's client binding: only bytes cross the socket.
#[allow(clippy::all, clippy::pedantic, dead_code)]
#[rustfmt::skip]
#[path = "identity_rpc_peer.rs"]
mod identity_rpc_peer;

use identity_rpc_peer::{
    identity_server::{Identity, IdentityServer},
    AccountState, AuthenticatePasswordRequest, AuthenticatePasswordResponse, AuthenticationFact,
    CompleteTheatreSignupRequest, CompleteTheatreSignupResponse, GetSecurityStateRequest,
    GetSecurityStateResponse, StartTheatreSignupRequest, StartTheatreSignupResponse,
};
use std::{
    path::Path,
    sync::{Arc, Mutex},
    time::Duration,
};
use tokio::{net::UnixListener, task::JoinHandle};
use tokio_stream::wrappers::UnixListenerStream;
use tonic::{Request, Response, Status};

#[derive(Clone)]
pub struct Script {
    pub subject: [u8; 16],
    pub epoch: [u8; 16],
    pub version: u64,
    pub active: bool,
    /// What `AuthenticatePassword` reports; independent of the state read that follows.
    pub fact_version: u64,
    pub fact_epoch: [u8; 16],
    pub fact_age_micros: i64,
    pub echo_operation: Option<Vec<u8>>,
    pub fact_purpose: Option<i32>,
    pub fact_assurance: i32,
    pub auth_delay: Duration,
    pub state_delay: Duration,
    pub state_subject: Option<Vec<u8>>,
    pub state_value: Option<i32>,
    pub state_not_found: bool,
    pub state_calls: u32,
}

impl Script {
    pub fn healthy() -> Self {
        Self {
            subject: [0x11; 16],
            epoch: [0x22; 16],
            version: 3,
            active: true,
            fact_version: 3,
            fact_epoch: [0x22; 16],
            fact_age_micros: 0,
            echo_operation: None,
            fact_purpose: None,
            fact_assurance: 1,
            auth_delay: Duration::ZERO,
            state_delay: Duration::ZERO,
            state_subject: None,
            state_value: None,
            state_not_found: false,
            state_calls: 0,
        }
    }
}

#[derive(Clone)]
pub struct Fake(pub Arc<Mutex<Script>>);

fn now_micros() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_micros() as i64
}

#[tonic::async_trait]
impl Identity for Fake {
    async fn start_theatre_signup(
        &self,
        _: Request<StartTheatreSignupRequest>,
    ) -> Result<Response<StartTheatreSignupResponse>, Status> {
        Err(Status::unimplemented("scripted"))
    }

    async fn complete_theatre_signup(
        &self,
        _: Request<CompleteTheatreSignupRequest>,
    ) -> Result<Response<CompleteTheatreSignupResponse>, Status> {
        Err(Status::unimplemented("scripted"))
    }

    async fn authenticate_password(
        &self,
        request: Request<AuthenticatePasswordRequest>,
    ) -> Result<Response<AuthenticatePasswordResponse>, Status> {
        let script = self.0.lock().unwrap().clone();
        tokio::time::sleep(script.auth_delay).await;
        let body = request.into_inner();
        Ok(Response::new(AuthenticatePasswordResponse {
            fact: Some(AuthenticationFact {
                subject_id: script.subject.to_vec(),
                purpose: script.fact_purpose.unwrap_or(body.purpose),
                assurance: script.fact_assurance,
                authenticated_at_unix_micros: now_micros() - script.fact_age_micros,
                security_version: script.fact_version,
                instance_epoch: script.fact_epoch.to_vec(),
                operation_id: script.echo_operation.unwrap_or(body.operation_id),
            }),
        }))
    }

    async fn get_security_state(
        &self,
        request: Request<GetSecurityStateRequest>,
    ) -> Result<Response<GetSecurityStateResponse>, Status> {
        let script = {
            let mut guard = self.0.lock().unwrap();
            guard.state_calls += 1;
            guard.clone()
        };
        tokio::time::sleep(script.state_delay).await;
        if script.state_not_found {
            return Err(Status::not_found("scripted"));
        }
        let body = request.into_inner();
        Ok(Response::new(GetSecurityStateResponse {
            subject_id: script.state_subject.unwrap_or(body.subject_id),
            state: script.state_value.unwrap_or(if script.active {
                AccountState::Active as i32
            } else {
                AccountState::Disabled as i32
            }),
            security_version: script.version,
            instance_epoch: script.epoch.to_vec(),
        }))
    }
}

/// Serves until dropped.
pub struct Serving {
    pub script: Arc<Mutex<Script>>,
    task: JoinHandle<()>,
}

impl Serving {
    pub fn start(socket: &Path, script: Script) -> Self {
        let script = Arc::new(Mutex::new(script));
        let listener = UnixListener::bind(socket).unwrap();
        let service = IdentityServer::new(Fake(Arc::clone(&script)));
        let task = tokio::spawn(async move {
            let _ = tonic::transport::Server::builder()
                .add_service(service)
                .serve_with_incoming(UnixListenerStream::new(listener))
                .await;
        });
        Self { script, task }
    }

    pub fn set(&self, change: impl FnOnce(&mut Script)) {
        change(&mut self.script.lock().unwrap());
    }

    pub fn state_calls(&self) -> u32 {
        self.script.lock().unwrap().state_calls
    }
}

impl Drop for Serving {
    fn drop(&mut self) {
        self.task.abort();
    }
}
