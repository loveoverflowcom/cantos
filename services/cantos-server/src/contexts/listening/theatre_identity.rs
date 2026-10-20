//! Actual Library seam adapter. A registry row or raw actor is never sufficient authority.
use crate::platform::theatre_identity::AuthError;
use crate::{
    contexts::listening::auth::{
        AuthenticatedListener, AuthenticationError, AuthenticationFuture, ListenerAuthenticator,
        ListenerCredentials,
    },
    platform::{identity::domain::ActorId, postgres::Store},
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
