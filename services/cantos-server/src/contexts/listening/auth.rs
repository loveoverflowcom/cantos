//! Listener-specific authentication seam. Registry mapping never authenticates a request.
use super::domain::ListenerId;
use crate::platform::{identity::domain::ActorId, identity_auth::credential::Channel};
use std::{fmt, future::Future, pin::Pin};
/// Bearer-only input, redacted in diagnostics. Cookies never enter this port.
pub struct ListenerCredentials(String);
impl ListenerCredentials {
    pub fn bearer(raw: &str) -> Result<Self, AuthenticationError> {
        if raw.is_empty() || raw.len() > 8192 || !raw.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(AuthenticationError::Unauthenticated);
        }
        Ok(Self(raw.into()))
    }
    pub fn expose_to_verifier(&self) -> &str {
        &self.0
    }
}
impl fmt::Debug for ListenerCredentials {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ListenerCredentials([redacted])")
    }
}
/// What the Server itself established about the session behind a sealed principal. It is created
/// only by the Identity-backed authenticator, never from a request body or header, and it lets
/// the Library re-admit that exact session inside the business transaction that writes (a logout
/// or revocation either committed before the write, which is then refused, or waits behind it).
/// The access secret is held only to repeat that lookup and is redacted from diagnostics.
#[derive(Clone)]
pub(crate) struct ListenerAdmission {
    token: String,
    channel: Channel,
    session_id: String,
}
impl ListenerAdmission {
    pub(crate) fn new(token: &str, channel: Channel, session_id: String) -> Self {
        Self {
            token: token.to_owned(),
            channel,
            session_id,
        }
    }
    pub(crate) fn token(&self) -> &str {
        &self.token
    }
    pub(crate) fn channel(&self) -> Channel {
        self.channel
    }
    pub(crate) fn session_id(&self) -> &str {
        &self.session_id
    }
}
impl fmt::Debug for ListenerAdmission {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ListenerAdmission")
            .field("channel", &self.channel)
            .field("session_id", &self.session_id)
            .field("token", &"[redacted]")
            .finish()
    }
}
/// Sealed account scope. Parsing actor IDs or receiving a Studio cookie cannot construct it.
/// ```compile_fail,E0624
/// use cantos_server::{contexts::listening::auth::AuthenticatedListener, platform::identity::domain::ActorId};
/// let forged = AuthenticatedListener::from_verified_actor(ActorId::new("alice").unwrap());
/// ```
/// ```compile_fail,E0277
/// use cantos_server::contexts::listening::auth::AuthenticatedListener;
/// let forged = serde_json::from_str::<AuthenticatedListener>("{\"actor_id\":\"alice\"}");
/// ```
#[derive(Clone, Debug)]
pub struct AuthenticatedListener {
    listener: ListenerId,
    admission: Option<ListenerAdmission>,
}
impl AuthenticatedListener {
    // Trusted issuer adapter seam, deliberately inaccessible to callers. The default adapter
    // cannot mint a principal. A principal without an admission was vouched for by a trusted host
    // override and is checked against the active actor only.
    #[allow(dead_code)]
    pub(crate) fn from_verified_actor(actor: ActorId) -> Self {
        Self {
            listener: ListenerId::from_actor(actor),
            admission: None,
        }
    }
    /// The principal earned by a live Identity session; the Library re-admits `admission` in the
    /// transaction of every write.
    pub(crate) fn from_admitted_session(actor: ActorId, admission: ListenerAdmission) -> Self {
        Self {
            listener: ListenerId::from_actor(actor),
            admission: Some(admission),
        }
    }
    pub fn listener(&self) -> &ListenerId {
        &self.listener
    }
    pub(super) fn admission(&self) -> Option<&ListenerAdmission> {
        self.admission.as_ref()
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AuthenticationError {
    Unauthenticated,
    Unavailable,
}
pub type AuthenticationFuture = Pin<
    Box<dyn Future<Output = Result<AuthenticatedListener, AuthenticationError>> + Send + 'static>,
>;
pub trait ListenerAuthenticator: Send + Sync {
    fn authenticate(&self, credentials: ListenerCredentials) -> AuthenticationFuture;
}
pub struct DenyListenerAuthentication;
impl ListenerAuthenticator for DenyListenerAuthentication {
    fn authenticate(&self, _credentials: ListenerCredentials) -> AuthenticationFuture {
        Box::pin(async { Err(AuthenticationError::Unauthenticated) })
    }
}
pub use super::theatre_identity::PasswordListenerAuthenticator;

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn default_listener_adapter_never_trusts_raw_studio_development_or_actor_tokens() {
        for raw in [
            "alice",
            "cantos_session=dev",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "fake.oidc.token",
        ] {
            assert_eq!(
                format!("{:?}", ListenerCredentials::bearer(raw).unwrap()),
                "ListenerCredentials([redacted])"
            );
            assert!(matches!(
                DenyListenerAuthentication
                    .authenticate(ListenerCredentials::bearer(raw).unwrap())
                    .await,
                Err(AuthenticationError::Unauthenticated)
            ));
        }
    }
}
