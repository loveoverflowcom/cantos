//! Actual Library seam adapter. A registry row or raw actor is never sufficient authority.
use crate::{
    contexts::listening::auth::{
        AuthenticatedListener, AuthenticationError, AuthenticationFuture, ListenerAuthenticator,
        ListenerCredentials,
    },
    platform::{identity::domain::ActorId, postgres::Store},
};
/// Identity-issued sessions, Web cookie or native Bearer. The registry mapping never
/// authenticates: a live session row plus a *fresh* Identity security-state read earns the
/// sealed principal, and anything else (mixed, wrong kind, outage) is refused.
pub struct PasswordListenerAuthenticator {
    store: Store,
}
impl PasswordListenerAuthenticator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}
impl ListenerAuthenticator for PasswordListenerAuthenticator {
    fn authenticate(&self, credentials: ListenerCredentials) -> AuthenticationFuture {
        use crate::platform::identity_auth::{credential, AuthFailure};
        let store = self.store.clone();
        Box::pin(async move {
            let token = credentials.expose_to_verifier();
            let channel = credential::access_channel(token)
                .filter(|channel| channel.surface() == credential::Surface::Theatre)
                .ok_or(AuthenticationError::Unauthenticated)?;
            let service = store
                .identity_auth()
                .ok_or(AuthenticationError::Unauthenticated)?
                .clone();
            let record = service
                .authenticate(&store, token, channel)
                .await
                .map_err(|failure| match failure {
                    AuthFailure::Unavailable | AuthFailure::Throttled { .. } => {
                        AuthenticationError::Unavailable
                    }
                    _ => AuthenticationError::Unauthenticated,
                })?;
            let actor =
                ActorId::new(record.actor_id).map_err(|_| AuthenticationError::Unavailable)?;
            Ok(AuthenticatedListener::from_verified_actor(actor))
        })
    }
}
