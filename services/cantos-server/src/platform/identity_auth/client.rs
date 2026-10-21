//! The Server's private-RPC client to `cantos-identity` over its Unix socket.
//!
//! Every connection is checked against the configured peer uid before a byte is sent, every call
//! has a deadline and an in-flight ceiling, and a response never leaves this module except as a
//! typed fact. Nothing here retries: a lost or late answer is an *unknown effect*, never proof
//! that Identity rolled back, and login/signup attempts are repeated only by the person.
use super::decision::{Epoch, MalformedState, SecurityState, Subject};
use cantos_identity_rpc::{
    identity_client::IdentityClient as Rpc, validate_fact, AuthenticatePasswordRequest,
    ClientContext, CompleteTheatreSignupRequest, GetSecurityStateRequest, Purpose, SetupOutcome,
    StartTheatreSignupRequest, ValidatedFact, ID_BYTES, MAX_MESSAGE_BYTES,
};
use hyper_util::rt::TokioIo;
use std::{net::IpAddr, path::PathBuf, sync::Arc, time::Duration};
use tokio::{net::UnixStream, sync::Semaphore};
use tonic::{
    transport::{Channel, Endpoint, Uri},
    Code, Request,
};
use tower::service_fn;

const CONNECT_TIMEOUT: Duration = Duration::from_secs(2);
const STATE_DEADLINE: Duration = Duration::from_secs(3);
const LOGIN_DEADLINE: Duration = Duration::from_secs(12);
const START_DEADLINE: Duration = Duration::from_secs(10);
const COMPLETE_DEADLINE: Duration = Duration::from_secs(15);
/// Identity serves 16 connections x 4 streams; stay well inside it and fail closed beyond.
const MAX_IN_FLIGHT: usize = 32;

/// Closed, content-free outcome of one RPC. No variant carries a status message, request field,
/// credential, address or internal reason.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RpcFault {
    /// Generic denial: wrong password, unknown account, expired/consumed proof, unauthorized.
    Denied,
    InvalidArgument,
    Throttled {
        retry_after_seconds: u32,
    },
    /// Refused before any effect (saturated, or Identity said so itself).
    Unavailable,
    /// Deadline, cancellation or an unclassifiable failure: the effect may have happened.
    UnknownEffect,
    Conflict,
    NotFound,
    /// The answer decoded but violated the contract (shape, operation, purpose, assurance).
    Protocol,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SignupDone {
    pub subject: Subject,
    pub epoch: Epoch,
    pub replayed: bool,
}

#[derive(Clone)]
pub struct IdentityClient {
    rpc: Rpc<Channel>,
    slots: Arc<Semaphore>,
}

impl IdentityClient {
    /// A long-lived channel that connects on first use and again after any break, so the Server
    /// starts and keeps serving public reads while Identity is down (protected work fails closed).
    pub fn lazy(socket: PathBuf, peer_uid: u32) -> Self {
        let connector = service_fn(move |_: Uri| {
            let path = socket.clone();
            async move {
                let stream = UnixStream::connect(&path).await?;
                if stream.peer_cred()?.uid() != peer_uid {
                    return Err(std::io::Error::from(std::io::ErrorKind::PermissionDenied));
                }
                Ok::<_, std::io::Error>(TokioIo::new(stream))
            }
        });
        let channel = Endpoint::from_static("http://identity.invalid")
            .connect_timeout(CONNECT_TIMEOUT)
            .connect_with_connector_lazy(connector);
        Self {
            rpc: Rpc::new(channel)
                .max_decoding_message_size(MAX_MESSAGE_BYTES)
                .max_encoding_message_size(MAX_MESSAGE_BYTES),
            slots: Arc::new(Semaphore::new(MAX_IN_FLIGHT)),
        }
    }

    async fn call<T, F, Fut>(&self, deadline: Duration, call: F) -> Result<T, RpcFault>
    where
        F: FnOnce(Rpc<Channel>) -> Fut,
        Fut: std::future::Future<Output = Result<tonic::Response<T>, tonic::Status>>,
    {
        let _permit = self
            .slots
            .clone()
            .try_acquire_owned()
            .map_err(|_| RpcFault::Unavailable)?;
        match tokio::time::timeout(deadline, call(self.rpc.clone())).await {
            Ok(Ok(response)) => Ok(response.into_inner()),
            Ok(Err(status)) => Err(fault(&status)),
            Err(_) => Err(RpcFault::UnknownEffect),
        }
    }

    pub async fn start_signup(
        &self,
        operation: [u8; ID_BYTES],
        email: &str,
        client: IpAddr,
    ) -> Result<u32, RpcFault> {
        let request = StartTheatreSignupRequest {
            operation_id: operation.to_vec(),
            email: email.to_owned(),
            client: Some(context(client)),
        };
        self.call(START_DEADLINE, |mut rpc| async move {
            rpc.start_theatre_signup(timed(request, START_DEADLINE))
                .await
        })
        .await
        .map(|response| response.proof_ttl_seconds)
    }

    #[allow(clippy::too_many_arguments)]
    pub async fn complete_signup(
        &self,
        command: [u8; ID_BYTES],
        operation: [u8; ID_BYTES],
        proof: [u8; ID_BYTES],
        secret: [u8; 32],
        password: &str,
        client: IpAddr,
    ) -> Result<SignupDone, RpcFault> {
        let request = CompleteTheatreSignupRequest {
            command_id: command.to_vec(),
            operation_id: operation.to_vec(),
            proof_id: proof.to_vec(),
            proof_secret: secret.to_vec(),
            password: password.to_owned(),
            explicit_confirmation: true,
            client: Some(context(client)),
        };
        let response = self
            .call(COMPLETE_DEADLINE, |mut rpc| async move {
                rpc.complete_theatre_signup(timed(request, COMPLETE_DEADLINE))
                    .await
            })
            .await
            // A transport break may follow the commit: only Identity's own fixed
            // "unavailable" answers promise that nothing changed.
            .map_err(|fault| match fault {
                RpcFault::Unavailable => RpcFault::UnknownEffect,
                other => other,
            })?;
        let replayed = match SetupOutcome::try_from(response.outcome) {
            Ok(SetupOutcome::Completed) => false,
            Ok(SetupOutcome::Replayed) => true,
            Ok(SetupOutcome::Unspecified) | Err(_) => return Err(RpcFault::Protocol),
        };
        Ok(SignupDone {
            subject: Subject::from_slice(&response.subject_id).ok_or(RpcFault::Protocol)?,
            epoch: Epoch::from_slice(&response.instance_epoch).ok_or(RpcFault::Protocol)?,
            replayed,
        })
    }

    /// One attempt; never retried. The returned fact is shape-, operation- and purpose-checked,
    /// and is not a session: the caller still applies freshness and issues nothing on its own.
    pub async fn authenticate(
        &self,
        operation: [u8; ID_BYTES],
        purpose: Purpose,
        email: &str,
        password: &str,
        client: IpAddr,
    ) -> Result<ValidatedFact, RpcFault> {
        let request = AuthenticatePasswordRequest {
            operation_id: operation.to_vec(),
            purpose: purpose as i32,
            email: email.to_owned(),
            password: password.to_owned(),
            client: Some(context(client)),
        };
        let response = self
            .call(LOGIN_DEADLINE, |mut rpc| async move {
                rpc.authenticate_password(timed(request, LOGIN_DEADLINE))
                    .await
            })
            .await?;
        let fact = response.fact.ok_or(RpcFault::Protocol)?;
        validate_fact(&fact, operation, purpose).map_err(|_| RpcFault::Protocol)
    }

    pub async fn security_state(&self, subject: Subject) -> Result<SecurityState, RpcFault> {
        let request = GetSecurityStateRequest {
            subject_id: subject.bytes().to_vec(),
        };
        let response = self
            .call(STATE_DEADLINE, |mut rpc| async move {
                rpc.get_security_state(timed(request, STATE_DEADLINE)).await
            })
            .await?;
        SecurityState::from_response(&response, subject)
            .map_err(|MalformedState| RpcFault::Protocol)
    }
}

fn timed<T>(message: T, deadline: Duration) -> Request<T> {
    let mut request = Request::new(message);
    request.set_timeout(deadline);
    request
}

fn context(client: IpAddr) -> ClientContext {
    ClientContext {
        client_ip: client.to_string(),
    }
}

fn fault(status: &tonic::Status) -> RpcFault {
    match status.code() {
        Code::Unauthenticated | Code::PermissionDenied => RpcFault::Denied,
        Code::InvalidArgument | Code::FailedPrecondition | Code::OutOfRange => {
            RpcFault::InvalidArgument
        }
        Code::ResourceExhausted => RpcFault::Throttled {
            retry_after_seconds: status
                .metadata()
                .get("retry-after-seconds")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u32>().ok())
                .map_or(60, |v| v.clamp(1, 86_400)),
        },
        Code::Aborted | Code::AlreadyExists => RpcFault::Conflict,
        Code::NotFound => RpcFault::NotFound,
        // A refused or unreachable socket and Identity's own "busy/unavailable" both mean the
        // request did not run; `complete_signup` alone distrusts transport-level breaks.
        Code::Unavailable => RpcFault::Unavailable,
        Code::Unimplemented => RpcFault::Protocol,
        _ => RpcFault::UnknownEffect,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tonic::Status;

    #[test]
    fn status_codes_map_to_closed_content_free_faults() {
        for (status, expected) in [
            (Status::unauthenticated("x"), RpcFault::Denied),
            (Status::permission_denied("x"), RpcFault::Denied),
            (Status::invalid_argument("x"), RpcFault::InvalidArgument),
            (Status::aborted("x"), RpcFault::Conflict),
            (Status::not_found("x"), RpcFault::NotFound),
            (Status::unimplemented("x"), RpcFault::Protocol),
            (Status::deadline_exceeded("x"), RpcFault::UnknownEffect),
            (Status::cancelled("x"), RpcFault::UnknownEffect),
            (Status::unknown("x"), RpcFault::UnknownEffect),
            (Status::internal("x"), RpcFault::UnknownEffect),
            (
                Status::unavailable("identity unavailable"),
                RpcFault::Unavailable,
            ),
            (
                Status::unavailable("transport error"),
                RpcFault::Unavailable,
            ),
        ] {
            assert_eq!(fault(&status), expected, "{status:?}");
        }
    }

    #[test]
    fn throttle_retry_after_is_bounded_and_defaults_safely() {
        let mut limited = Status::resource_exhausted("too many attempts");
        assert_eq!(
            fault(&limited),
            RpcFault::Throttled {
                retry_after_seconds: 60
            }
        );
        for (value, expected) in [("5", 5), ("0", 1), ("999999", 86_400), ("junk", 60)] {
            limited
                .metadata_mut()
                .insert("retry-after-seconds", value.parse().unwrap());
            assert_eq!(
                fault(&limited),
                RpcFault::Throttled {
                    retry_after_seconds: expected
                },
                "{value}"
            );
        }
    }
}
