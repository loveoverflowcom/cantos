//! Listener-specific authentication seam. Registry mapping never authenticates a request.
use super::domain::ListenerId;
use crate::contexts::catalog::domain::OwnerId;
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
/// Sealed account scope. Parsing actor IDs or receiving a Studio cookie cannot construct it.
/// ```compile_fail,E0624
/// use cantos_api::contexts::{catalog::domain::OwnerId, listening::auth::AuthenticatedListener};
/// let forged = AuthenticatedListener::from_verified_actor(OwnerId::parse("alice").unwrap());
/// ```
/// ```compile_fail,E0277
/// use cantos_api::contexts::listening::auth::AuthenticatedListener;
/// let forged = serde_json::from_str::<AuthenticatedListener>("{\"actor_id\":\"alice\"}");
/// ```
#[derive(Clone, Debug)]
pub struct AuthenticatedListener(ListenerId);
impl AuthenticatedListener {
    // Trusted issuer adapter seam, deliberately inaccessible to callers. The default adapter
    // cannot mint a principal. The identity successor must earn all issuer/session checks first.
    #[allow(dead_code)]
    pub(crate) fn from_verified_actor(actor: OwnerId) -> Self {
        Self(ListenerId::from_actor(actor))
    }
    pub fn listener(&self) -> &ListenerId {
        &self.0
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
