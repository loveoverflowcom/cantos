//! Email/password sessions issued from a verified `cantos-identity` fact (#335).
//!
//! Identity proves a mailbox and a password and answers with a fact. This module turns a *fresh*
//! fact into a Server session bound to the exact Identity subject's ActorId, and re-reads the
//! subject's security state at every issue, renewal and protected admission. It reuses the
//! existing `SessionPolicy`, rotation/replay core and token hashing; it adds no second engine
//! and never grants Studio, Admin, producer or membership authority on its own.
pub mod client;
pub mod config;
pub mod credential;
pub mod decision;
pub mod session;
pub mod turso;

pub use self::session::{SessionError, SessionRecord, SessionStore};
use self::{
    client::{IdentityClient, RpcFault},
    credential::{Channel, Secrets, Surface},
    decision::{decide_admission, decide_issue, Admission, IssueRefusal, RevokeReason, Subject},
};
use crate::platform::{
    bearer::core::Lifetime, store::Store, theatre_registration::domain::PeerPolicy,
};
use cantos_identity_rpc::Purpose;
use sha2::{Digest, Sha256};
use std::{net::IpAddr, sync::Arc, time::Duration};

/// Closed failure vocabulary of this module. The HTTP boundary projects it to generic statuses;
/// no variant says which of password, account, proof or security check failed. `Forbidden` is the
/// one exception, reached only after the password verified: it names the Studio roster.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthFailure {
    Unauthenticated,
    /// Correct credentials, but the subject is not on the Studio roster.
    Forbidden,
    InvalidRequest,
    Throttled {
        retry_after_seconds: u32,
    },
    Unavailable,
}

fn rpc(fault: RpcFault) -> AuthFailure {
    match fault {
        RpcFault::Denied | RpcFault::NotFound => AuthFailure::Unauthenticated,
        RpcFault::InvalidArgument => AuthFailure::InvalidRequest,
        RpcFault::Throttled {
            retry_after_seconds,
        } => AuthFailure::Throttled {
            retry_after_seconds,
        },
        // A protocol violation fails closed: it is never evidence for a session.
        RpcFault::Unavailable
        | RpcFault::UnknownEffect
        | RpcFault::Conflict
        | RpcFault::Protocol => AuthFailure::Unavailable,
    }
}

fn session(error: SessionError) -> AuthFailure {
    match error {
        SessionError::Inactive | SessionError::Expired | SessionError::Replay => {
            AuthFailure::Unauthenticated
        }
        // An unknown commit is never success; the person retries, which an adapter reconciles.
        SessionError::Unavailable | SessionError::Unknown => AuthFailure::Unavailable,
    }
}

pub struct IdentityAuth {
    client: IdentityClient,
    peer_policy: PeerPolicy,
    studio: Vec<Subject>,
    secure: bool,
    /// Admission holds a pooled connection through dispatch; leave capacity for everything else,
    /// exactly like the existing credential kinds.
    slots: tokio::sync::Semaphore,
}

/// A freshly issued session. Secrets are shown once, to the transport that must deliver them.
pub struct Login {
    pub actor_id: String,
    pub session_id: String,
    pub lifetime: Lifetime,
    pub authenticated_at: u64,
    pub secrets: Secrets,
}

pub struct SignupCommand {
    pub operation: [u8; 16],
    pub proof: [u8; 16],
    pub secret: [u8; 32],
    pub password: String,
}

impl IdentityAuth {
    /// `secure` is true when every Web origin served by this process is `https`: the cookie then
    /// carries `Secure` and the `__Secure-` name.
    pub fn new(config: config::IdentityAuthConfig, secure: bool) -> Self {
        Self {
            client: IdentityClient::lazy(config.socket, config.peer_uid),
            peer_policy: config.peer_policy,
            studio: config.studio_subjects,
            secure,
            slots: tokio::sync::Semaphore::new(3),
        }
    }

    pub fn secure(&self) -> bool {
        self.secure
    }

    pub fn studio_enabled(&self) -> bool {
        !self.studio.is_empty()
    }

    /// The address that owns an anonymous attempt's budget: the socket peer, or the single
    /// forwarded hop a configured proxy appended. Anything else fails closed.
    pub fn client_ip(&self, peer: IpAddr, forwarded: &[&str]) -> Result<IpAddr, AuthFailure> {
        self.peer_policy
            .client_ip(peer, forwarded)
            .map_err(|_| AuthFailure::Unavailable)
    }

    pub async fn start_signup(&self, email: &str, client: IpAddr) -> Result<u32, AuthFailure> {
        let operation = random_id().ok_or(AuthFailure::Unavailable)?;
        self.client
            .start_signup(operation, email, client)
            .await
            .map_err(rpc)
    }

    /// The command id is derived from the operation and proof, so an identical retry of a lost
    /// response is the identical command (Identity then answers `REPLAYED`), while a different
    /// proof or password is refused. Binding the subject to its ActorId is idempotent and runs
    /// on replay too, so a half-finished completion resumes instead of stranding the account.
    pub async fn complete_signup<S: SessionStore>(
        &self,
        store: &S,
        command: SignupCommand,
        client: IpAddr,
    ) -> Result<bool, AuthFailure> {
        let done = self
            .client
            .complete_signup(
                command_id(&command.operation, &command.proof),
                command.operation,
                command.proof,
                command.secret,
                &command.password,
                client,
            )
            .await
            .map_err(rpc)?;
        store.bind(done.subject).await.map_err(session)?;
        Ok(done.replayed)
    }

    /// One attempt, never retried. Success needs: a verified fact bound to this operation and
    /// purpose, the Studio roster for Studio, then a *fresh* security state that still matches.
    pub async fn login<S: SessionStore>(
        &self,
        store: &S,
        channel: Channel,
        email: &str,
        password: &str,
        client: IpAddr,
    ) -> Result<Login, AuthFailure> {
        let purpose = match channel.surface() {
            Surface::Theatre => Purpose::Theatre,
            Surface::Studio => Purpose::Studio,
        };
        let operation = random_id().ok_or(AuthFailure::Unavailable)?;
        let fact = self
            .client
            .authenticate(operation, purpose, email, password, client)
            .await
            .map_err(rpc)?;
        let subject = Subject::from_bytes(fact.subject).ok_or(AuthFailure::Unavailable)?;
        if purpose == Purpose::Studio && !self.studio.contains(&subject) {
            return Err(AuthFailure::Forbidden);
        }
        let current = self.client.security_state(subject).await.map_err(rpc)?;
        let observed =
            decide_issue(&fact, &current, now_micros()?).map_err(|refusal| match refusal {
                IssueRefusal::FactStale | IssueRefusal::FactFromFuture => AuthFailure::Unavailable,
                IssueRefusal::SubjectChanged
                | IssueRefusal::Inactive
                | IssueRefusal::EpochChanged
                | IssueRefusal::VersionChanged => AuthFailure::Unauthenticated,
            })?;
        let authenticated_at = u64::try_from(fact.authenticated_at_unix_micros / 1_000_000)
            .map_err(|_| AuthFailure::Unavailable)?;
        let secrets = Secrets::new(channel).ok_or(AuthFailure::Unavailable)?;
        let issued = store
            .issue(channel, &observed, authenticated_at, &secrets)
            .await
            .map_err(session)?;
        Ok(Login {
            actor_id: issued.actor_id,
            session_id: issued.session_id,
            lifetime: issued.lifetime,
            authenticated_at,
            secrets,
        })
    }

    /// Live session plus a **fresh** Identity read. Any change in subject, account state,
    /// instance epoch or security version revokes the whole family durably; Identity being
    /// unreachable fails closed *without* revoking (an outage is not a security change).
    pub async fn authenticate<S: SessionStore>(
        &self,
        store: &S,
        token: &str,
        channel: Channel,
    ) -> Result<SessionRecord, AuthFailure> {
        if credential::access_channel(token) != Some(channel) {
            return Err(AuthFailure::Unauthenticated);
        }
        let record = store.access(token, channel).await.map_err(session)?;
        self.confirm(store, &record).await?;
        Ok(record)
    }

    async fn confirm<S: SessionStore>(
        &self,
        store: &S,
        record: &SessionRecord,
    ) -> Result<(), AuthFailure> {
        if record.channel.surface() == Surface::Studio
            && !self.studio.contains(&record.observed.subject)
        {
            return Err(AuthFailure::Unauthenticated);
        }
        let current = match self.client.security_state(record.observed.subject).await {
            Ok(current) => current,
            // An unknown subject is a security fact (a restored older file), not an outage.
            Err(RpcFault::NotFound) => {
                let _ = store
                    .revoke(&record.session_id, RevokeReason::SubjectMissing)
                    .await;
                return Err(AuthFailure::Unauthenticated);
            }
            Err(fault) => return Err(rpc(fault)),
        };
        match decide_admission(&record.observed, &current) {
            Admission::Admit => Ok(()),
            Admission::Revoke(reason) => {
                // Best effort: the refusal below does not depend on this write succeeding.
                let _ = store.revoke(&record.session_id, reason).await;
                Err(AuthFailure::Unauthenticated)
            }
        }
    }

    /// Protected-request admission: fresh Identity read first (no lock held), then the live
    /// session at the Server's snapshot before `dispatch` runs. A business write re-admits the
    /// session inside its own transaction, which linearizes it against logout and disable. A
    /// disable committed in Identity *during* dispatch is seen by the next admission; the bound
    /// is one request, capped by the host's 15 s deadline.
    pub async fn admit<F, Fut, T>(
        &self,
        store: &Store,
        token: &str,
        channel: Channel,
        dispatch: F,
    ) -> Result<T, AuthFailure>
    where
        F: FnOnce(SessionRecord) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        self.authenticate(store, token, channel).await?;
        self.lease(store, token, channel, dispatch).await
    }

    /// The admission lease alone, for a caller that has just authenticated the same secret (the
    /// listener port does, to earn its sealed principal) and must not ask Identity twice.
    pub async fn lease<F, Fut, T>(
        &self,
        store: &Store,
        token: &str,
        channel: Channel,
        dispatch: F,
    ) -> Result<T, AuthFailure>
    where
        F: FnOnce(SessionRecord) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _permit = tokio::time::timeout(Duration::from_secs(5), self.slots.acquire())
            .await
            .map_err(|_| AuthFailure::Unavailable)?
            .map_err(|_| AuthFailure::Unavailable)?;
        // No lock is held across `dispatch`: the serialized writer orders a logout or
        // deactivation against the unit of any business write, which re-admits the session
        // itself (`turso::admit_in`). A read-only dispatch is admitted at this snapshot only.
        let record = store.access(token, channel).await.map_err(session)?;
        Ok(dispatch(record).await)
    }

    /// Nonsecret fence input for guarded logout; `None` for an unknown secret.
    pub async fn session_id_of<S: SessionStore>(
        &self,
        store: &S,
        token: &str,
        channel: Channel,
    ) -> Result<Option<String>, AuthFailure> {
        if credential::access_channel(token) != Some(channel) {
            return Ok(None);
        }
        store.session_id_of(token, channel).await.map_err(session)
    }

    /// Local, durable and independent of Identity being online.
    pub async fn logout<S: SessionStore>(
        &self,
        store: &S,
        token: &str,
        channel: Channel,
    ) -> Result<(), AuthFailure> {
        if credential::access_channel(token) != Some(channel) {
            return Err(AuthFailure::Unauthenticated);
        }
        store.logout(token, channel).await.map_err(session)
    }

    /// Extends the idle deadline of a Web cookie session (never past its absolute deadline).
    pub async fn renew_web<S: SessionStore>(
        &self,
        store: &S,
        token: &str,
        channel: Channel,
    ) -> Result<(SessionRecord, Lifetime), AuthFailure> {
        self.authenticate(store, token, channel).await?;
        store.renew_web(token, channel).await.map_err(session)
    }

    /// Native renewal. A spent secret revokes the family before Identity is consulted; the
    /// subject's state is then re-read (outside any transaction) before the next pair is cut.
    pub async fn refresh_native<S: SessionStore>(
        &self,
        store: &S,
        refresh: &str,
        session_id: &str,
    ) -> Result<(String, u64, Lifetime, Secrets), AuthFailure> {
        if !credential::is_refresh(refresh) || uuid::Uuid::parse_str(session_id).is_err() {
            return Err(AuthFailure::Unauthenticated);
        }
        let peek = store
            .peek_refresh(refresh, session_id)
            .await
            .map_err(session)?;
        self.confirm(store, &peek).await?;
        let secrets = Secrets::new(Channel::Native).ok_or(AuthFailure::Unavailable)?;
        let rotated = store
            .rotate(refresh, session_id, &secrets)
            .await
            .map_err(session)?;
        Ok((
            rotated.actor_id,
            rotated.authenticated_at,
            rotated.lifetime,
            secrets,
        ))
    }

    pub async fn revoke_refresh<S: SessionStore>(
        &self,
        store: &S,
        refresh: &str,
        session_id: &str,
    ) -> Result<(), AuthFailure> {
        if !credential::is_refresh(refresh) || uuid::Uuid::parse_str(session_id).is_err() {
            return Err(AuthFailure::Unauthenticated);
        }
        store
            .revoke_refresh(refresh, session_id)
            .await
            .map_err(session)
    }
}

fn random_id() -> Option<[u8; 16]> {
    let mut bytes = [0u8; 16];
    for _ in 0..4 {
        getrandom::fill(&mut bytes).ok()?;
        if bytes != [0; 16] {
            return Some(bytes);
        }
    }
    None
}

fn now_micros() -> Result<i64, AuthFailure> {
    let elapsed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_err(|_| AuthFailure::Unavailable)?;
    i64::try_from(elapsed.as_micros()).map_err(|_| AuthFailure::Unavailable)
}

/// Deterministic, domain-separated and nonzero: the same operation and proof always name the
/// same command, which is what makes retrying a lost completion response safe.
pub(crate) fn command_id(operation: &[u8; 16], proof: &[u8; 16]) -> [u8; 16] {
    let digest = Sha256::new()
        .chain_update(b"cantos-identity-signup-command-v1")
        .chain_update(operation)
        .chain_update(proof)
        .finalize();
    let mut id = [0u8; 16];
    id.copy_from_slice(&digest[..16]);
    if id == [0; 16] {
        id[15] = 1;
    }
    id
}

impl Store {
    pub fn with_identity_auth(mut self, identity: IdentityAuth) -> Self {
        self.identity_auth = Some(Arc::new(identity));
        self
    }
    pub fn identity_auth(&self) -> Option<&Arc<IdentityAuth>> {
        self.identity_auth.as_ref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn command_ids_are_deterministic_distinct_and_nonzero() {
        let a = command_id(&[1; 16], &[2; 16]);
        assert_eq!(a, command_id(&[1; 16], &[2; 16]));
        assert_ne!(a, command_id(&[1; 16], &[3; 16]));
        assert_ne!(a, command_id(&[2; 16], &[2; 16]));
        // Operation and proof are not interchangeable.
        assert_ne!(
            command_id(&[1; 16], &[2; 16]),
            command_id(&[2; 16], &[1; 16])
        );
        assert_ne!(a, [0; 16]);
    }

    #[test]
    fn rpc_faults_project_to_generic_failures_and_never_to_success() {
        for (fault, expected) in [
            (RpcFault::Denied, AuthFailure::Unauthenticated),
            (RpcFault::NotFound, AuthFailure::Unauthenticated),
            (RpcFault::InvalidArgument, AuthFailure::InvalidRequest),
            (
                RpcFault::Throttled {
                    retry_after_seconds: 7,
                },
                AuthFailure::Throttled {
                    retry_after_seconds: 7,
                },
            ),
            (RpcFault::Unavailable, AuthFailure::Unavailable),
            (RpcFault::UnknownEffect, AuthFailure::Unavailable),
            (RpcFault::Conflict, AuthFailure::Unavailable),
            (RpcFault::Protocol, AuthFailure::Unavailable),
        ] {
            assert_eq!(rpc(fault), expected, "{fault:?}");
        }
        for (error, expected) in [
            (SessionError::Inactive, AuthFailure::Unauthenticated),
            (SessionError::Expired, AuthFailure::Unauthenticated),
            (SessionError::Replay, AuthFailure::Unauthenticated),
            (SessionError::Unavailable, AuthFailure::Unavailable),
            (SessionError::Unknown, AuthFailure::Unavailable),
        ] {
            assert_eq!(session(error), expected);
        }
    }
}
