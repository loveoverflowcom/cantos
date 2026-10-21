//! The private `cantos.identity.v1` adapter. It maps wire messages to the application, maps
//! outcomes to fixed status messages and authorizes every call from the OS peer credential.
//!
//! Nothing inside a request names or elevates the caller: no client id, role or uid field is
//! read. A decoded message is untrusted input until it passes the checks here.

#[cfg(not(unix))]
compile_error!(
    "cantos-identity requires a Unix host: caller authentication uses OS peer credentials"
);

use std::{collections::BTreeSet, net::IpAddr, sync::Arc};

use cantos_identity_rpc::{
    identity_server::Identity, AccountState as WireState, Assurance as WireAssurance,
    AuthenticatePasswordRequest, AuthenticatePasswordResponse, AuthenticationFact, ClientContext,
    CompleteTheatreSignupRequest, CompleteTheatreSignupResponse, GetSecurityStateRequest,
    GetSecurityStateResponse, Purpose as WirePurpose, SetupOutcome, StartTheatreSignupRequest,
    StartTheatreSignupResponse, MAX_CLIENT_IP_BYTES,
};
use tonic::{
    metadata::MetadataValue, transport::server::UdsConnectInfo, Code, Request, Response, Status,
};

use crate::{
    application::{ClientAddress, IdentityService, ServiceError, SignupOutcome},
    domain::{
        admission::{AccountState, Purpose},
        ids::{OperationId, SubjectId},
        mailbox_proof::ProofId,
    },
    events,
    persistence::ProofCommandId,
    policy::{MAX_CALLER_UIDS, PROOF_TTL_SECONDS},
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Method {
    StartTheatreSignup,
    CompleteTheatreSignup,
    AuthenticatePassword,
    GetSecurityState,
}

/// Which OS users may call which methods. The only role in this build is the Theatre Server;
/// operator and Studio methods have no role and therefore no caller.
#[derive(Clone, Debug)]
pub struct CallerPolicy {
    server_uids: BTreeSet<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyError {
    Empty,
    TooMany,
}

impl CallerPolicy {
    pub fn new(server_uids: impl IntoIterator<Item = u32>) -> Result<Self, PolicyError> {
        let server_uids: BTreeSet<u32> = server_uids.into_iter().collect();
        if server_uids.is_empty() {
            return Err(PolicyError::Empty);
        }
        if server_uids.len() > MAX_CALLER_UIDS {
            return Err(PolicyError::TooMany);
        }
        Ok(Self { server_uids })
    }

    pub fn permits_uid(&self, uid: u32) -> bool {
        self.server_uids.contains(&uid)
    }

    fn allows(&self, uid: u32, _method: Method) -> bool {
        // Every implemented method is a Theatre Server method.
        self.permits_uid(uid)
    }

    fn authorize<T>(&self, request: &Request<T>, method: Method) -> Result<(), Status> {
        let uid = request
            .extensions()
            .get::<UdsConnectInfo>()
            .and_then(|info| info.peer_cred)
            .map(|credential| credential.uid());
        match uid {
            Some(uid) if self.allows(uid, method) => Ok(()),
            _ => {
                events::emit("rpc_denied", "caller is not an allowed peer");
                Err(Status::permission_denied("caller is not authorized"))
            }
        }
    }
}

// ACCEPTANCE-PENDING(#335): no Server consumes this service yet. The fact binding/freshness check,
// the subject-to-ActorId binding, session issue and the revocation (security-event) consumer are
// Server work; this adapter is exercised only by this crate's own real UDS client.
pub struct IdentityRpc {
    service: Arc<IdentityService>,
    policy: CallerPolicy,
}

impl IdentityRpc {
    pub fn new(service: Arc<IdentityService>, policy: CallerPolicy) -> Self {
        Self { service, policy }
    }
}

fn id16(bytes: &[u8]) -> Result<[u8; 16], Status> {
    bytes
        .try_into()
        .ok()
        .filter(|id: &[u8; 16]| *id != [0; 16])
        .ok_or_else(|| Status::invalid_argument("invalid request"))
}

fn operation(bytes: &[u8]) -> Result<OperationId, Status> {
    OperationId::from_bytes(id16(bytes)?).map_err(|_| Status::invalid_argument("invalid request"))
}

fn client(context: Option<ClientContext>) -> Result<ClientAddress, Status> {
    let context = context.ok_or_else(|| Status::invalid_argument("invalid request"))?;
    if context.client_ip.len() > MAX_CLIENT_IP_BYTES {
        return Err(Status::invalid_argument("invalid request"));
    }
    context
        .client_ip
        .parse::<IpAddr>()
        .map(ClientAddress)
        .map_err(|_| Status::invalid_argument("invalid request"))
}

fn purpose(raw: i32) -> Result<Purpose, Status> {
    match WirePurpose::try_from(raw) {
        Ok(WirePurpose::Theatre) => Ok(Purpose::Theatre),
        Ok(WirePurpose::Studio) => Ok(Purpose::Studio),
        _ => Err(Status::invalid_argument("invalid request")),
    }
}

/// Fixed messages only: no variant ever interpolates request content or an internal reason.
fn status(error: ServiceError, denied: Code) -> Status {
    match error {
        ServiceError::InvalidRequest => Status::invalid_argument("invalid request"),
        ServiceError::InvalidPassword => {
            Status::invalid_argument("password does not meet the length policy")
        }
        ServiceError::Throttled(throttled) => {
            let mut status = Status::resource_exhausted("too many attempts");
            if let Ok(value) = MetadataValue::try_from(throttled.retry_after_seconds.to_string()) {
                status.metadata_mut().insert("retry-after-seconds", value);
            }
            status
        }
        ServiceError::Denied => Status::new(
            denied,
            if denied == Code::Unauthenticated {
                "authentication failed"
            } else {
                "signup could not be completed"
            },
        ),
        ServiceError::Busy => Status::unavailable("identity is busy; retry later"),
        ServiceError::Unavailable => Status::unavailable("identity unavailable"),
        ServiceError::OutcomeUnknown => Status::new(
            Code::Unknown,
            "outcome unknown: retry the identical command to reconcile it",
        ),
        ServiceError::Conflict => Status::aborted("request conflicts with an earlier request"),
        ServiceError::Unsupported => {
            Status::unimplemented("this purpose is not supported by this identity build")
        }
        ServiceError::NotFound => Status::not_found("subject not found"),
    }
}

#[tonic::async_trait]
impl Identity for IdentityRpc {
    async fn start_theatre_signup(
        &self,
        request: Request<StartTheatreSignupRequest>,
    ) -> Result<Response<StartTheatreSignupResponse>, Status> {
        self.policy
            .authorize(&request, Method::StartTheatreSignup)?;
        let body = request.into_inner();
        let operation = operation(&body.operation_id)?;
        let client = client(body.client)?;
        self.service
            .start_theatre_signup(operation, &body.email, client)
            .await
            .map_err(|error| status(error, Code::PermissionDenied))?;
        // The same answer whatever the mail path did, including "no mail was attempted".
        Ok(Response::new(StartTheatreSignupResponse {
            accepted: true,
            proof_ttl_seconds: PROOF_TTL_SECONDS as u32,
        }))
    }

    async fn complete_theatre_signup(
        &self,
        request: Request<CompleteTheatreSignupRequest>,
    ) -> Result<Response<CompleteTheatreSignupResponse>, Status> {
        self.policy
            .authorize(&request, Method::CompleteTheatreSignup)?;
        let body = request.into_inner();
        let command = ProofCommandId::from_bytes(id16(&body.command_id)?)
            .map_err(|_| Status::invalid_argument("invalid request"))?;
        let operation = operation(&body.operation_id)?;
        let proof = ProofId::from_bytes(id16(&body.proof_id)?)
            .map_err(|_| Status::invalid_argument("invalid request"))?;
        let secret: [u8; 32] = body
            .proof_secret
            .as_slice()
            .try_into()
            .map_err(|_| Status::invalid_argument("invalid request"))?;
        let client = client(body.client)?;
        let outcome = self
            .service
            .complete_theatre_signup(
                command,
                operation,
                proof,
                secret,
                &body.password,
                body.explicit_confirmation,
                client,
            )
            .await
            .map_err(|error| status(error, Code::PermissionDenied))?;
        let (subject, epoch, outcome) = match outcome {
            SignupOutcome::Completed { subject, epoch } => {
                (subject, epoch, SetupOutcome::Completed)
            }
            SignupOutcome::Replayed { subject, epoch } => (subject, epoch, SetupOutcome::Replayed),
        };
        Ok(Response::new(CompleteTheatreSignupResponse {
            subject_id: subject.to_bytes().to_vec(),
            instance_epoch: epoch.to_bytes().to_vec(),
            outcome: outcome as i32,
        }))
    }

    async fn authenticate_password(
        &self,
        request: Request<AuthenticatePasswordRequest>,
    ) -> Result<Response<AuthenticatePasswordResponse>, Status> {
        self.policy
            .authorize(&request, Method::AuthenticatePassword)?;
        let body = request.into_inner();
        let operation = operation(&body.operation_id)?;
        let purpose = purpose(body.purpose)?;
        let client = client(body.client)?;
        let fact = self
            .service
            .authenticate_password(operation, purpose, &body.email, &body.password, client)
            .await
            .map_err(|error| status(error, Code::Unauthenticated))?;
        Ok(Response::new(AuthenticatePasswordResponse {
            fact: Some(AuthenticationFact {
                subject_id: fact.subject.to_bytes().to_vec(),
                purpose: WirePurpose::Theatre as i32,
                assurance: WireAssurance::Password as i32,
                authenticated_at_unix_micros: fact.authenticated_at_micros,
                security_version: fact.security_version.get(),
                instance_epoch: fact.epoch.to_bytes().to_vec(),
                operation_id: fact.operation.to_bytes().to_vec(),
            }),
        }))
    }

    async fn get_security_state(
        &self,
        request: Request<GetSecurityStateRequest>,
    ) -> Result<Response<GetSecurityStateResponse>, Status> {
        self.policy.authorize(&request, Method::GetSecurityState)?;
        let body = request.into_inner();
        let subject = SubjectId::from_bytes(id16(&body.subject_id)?)
            .map_err(|_| Status::invalid_argument("invalid request"))?;
        let state = self
            .service
            .security_state(subject)
            .await
            .map_err(|error| status(error, Code::PermissionDenied))?;
        Ok(Response::new(GetSecurityStateResponse {
            subject_id: state.subject.to_bytes().to_vec(),
            state: match state.state {
                AccountState::Active => WireState::Active,
                // Deleted and tombstoned subjects are not storable yet; fail closed as disabled.
                _ => WireState::Disabled,
            } as i32,
            security_version: state.version.get(),
            instance_epoch: state.epoch.to_bytes().to_vec(),
        }))
    }
}
