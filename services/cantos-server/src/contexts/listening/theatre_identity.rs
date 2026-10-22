//! Actual Library seam adapter. A registry row or raw actor is never sufficient authority.
use crate::platform::theatre_identity::AuthError;
use crate::{
    contexts::listening::auth::{
        AuthenticatedListener, AuthenticationError, AuthenticationFuture, ListenerAuthenticator,
        ListenerCredentials,
    },
    platform::{identity::domain::ActorId, store::Store},
};
pub struct TheatreListenerAuthenticator {
    store: Store,
}
pub struct NativeListenerAuthenticator {
    store: Store,
}
impl NativeListenerAuthenticator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}
impl ListenerAuthenticator for NativeListenerAuthenticator {
    fn authenticate(&self, credentials: ListenerCredentials) -> AuthenticationFuture {
        let store = self.store.clone();
        Box::pin(async move {
            let native = store
                .theatre_native()
                .ok_or(AuthenticationError::Unauthenticated)?;
            let view = native
                .authenticate(&store, credentials.expose_to_verifier())
                .await
                .map_err(authentication_error)?;
            let actor =
                ActorId::new(view.actor_id).map_err(|_| AuthenticationError::Unavailable)?;
            Ok(AuthenticatedListener::from_verified_actor(actor))
        })
    }
}
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
impl TheatreListenerAuthenticator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}
pub(crate) fn authentication_error(error: AuthError) -> AuthenticationError {
    match error {
        AuthError::Configuration
        | AuthError::ProviderUnavailable
        | AuthError::InvalidMetadata
        | AuthError::Unavailable => AuthenticationError::Unavailable,
        AuthError::InvalidToken
        | AuthError::Algorithm
        | AuthError::UnknownKey
        | AuthError::InvalidKey
        | AuthError::Signature
        | AuthError::Issuer
        | AuthError::Audience
        | AuthError::Expired
        | AuthError::NotYetValid
        | AuthError::Nonce
        | AuthError::Subject
        | AuthError::State
        | AuthError::Replay
        | AuthError::Inactive => AuthenticationError::Unauthenticated,
    }
}
impl ListenerAuthenticator for TheatreListenerAuthenticator {
    fn authenticate(&self, credentials: ListenerCredentials) -> AuthenticationFuture {
        let store = self.store.clone();
        Box::pin(async move {
            let oidc = store
                .theatre_identity()
                .ok_or(AuthenticationError::Unauthenticated)?;
            let actor = oidc
                .authenticate(&store, credentials.expose_to_verifier())
                .await
                .map_err(authentication_error)?;
            let actor = ActorId::new(actor).map_err(|_| AuthenticationError::Unavailable)?;
            Ok(AuthenticatedListener::from_verified_actor(actor))
        })
    }
}

/// The unified Cantos Bearer transport still earns the sealed listener principal here.
pub struct BearerListenerAuthenticator {
    store: Store,
}
impl BearerListenerAuthenticator {
    pub fn new(store: Store) -> Self {
        Self { store }
    }
}
impl ListenerAuthenticator for BearerListenerAuthenticator {
    fn authenticate(&self, credentials: ListenerCredentials) -> AuthenticationFuture {
        let store = self.store.clone();
        Box::pin(async move {
            let token = credentials.expose_to_verifier();
            let client = crate::platform::bearer::credential::access_client(token)
                .filter(|c| c.surface() == "theatre")
                .ok_or(AuthenticationError::Unauthenticated)?;
            let service = store
                .bearer(client)
                .ok_or(AuthenticationError::Unavailable)?;
            let view = service
                .authenticate(&store, token)
                .await
                .map_err(authentication_error)?;
            let actor =
                ActorId::new(view.actor_id).map_err(|_| AuthenticationError::Unavailable)?;
            Ok(AuthenticatedListener::from_verified_actor(actor))
        })
    }
}
