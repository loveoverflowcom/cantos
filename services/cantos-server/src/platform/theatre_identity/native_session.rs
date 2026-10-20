//! Native public-client shell. Provider credentials never become Web sessions or client DTOs.
mod postgres;

use super::{now, random_secret, valid_secret, AuthError};
use crate::platform::{
    oidc::{
        config::OidcConfig,
        core::{pkce_challenge, session_expiry},
        provider::Provider,
    },
    postgres::Store,
};
use cantos_contracts::theatre_identity::{
    NativeExchange, NativeExchangeRequest, NativeFlow, NativeFlowRequest, NativeSession,
};

pub struct NativeIdentity {
    config: OidcConfig,
    bearer: crate::platform::bearer::BearerIdentity,
    provider: Provider,
    admission_slots: tokio::sync::Semaphore,
}
impl NativeIdentity {
    pub(crate) fn bearer(&self) -> &crate::platform::bearer::BearerIdentity {
        &self.bearer
    }
    /// Explicit development composition only. No secret, dynamic client, redirect or issuer.
    pub fn new(issuer: &str) -> Result<Self, AuthError> {
        Self::with_callbacks(issuer, false)
    }
    /// `admit_development_callback` adds only the exact Android debug callback; the default
    /// production callback stays admitted and no other URI can be configured.
    pub fn with_callbacks(
        issuer: &str,
        admit_development_callback: bool,
    ) -> Result<Self, AuthError> {
        let mut config = OidcConfig::theatre_native(issuer)?;
        if admit_development_callback {
            config = config.with_development_callback();
        }
        Ok(Self {
            bearer: crate::platform::bearer::BearerIdentity::new(
                config.clone(),
                crate::platform::bearer::client::Client::TheatreNative,
            )?,
            provider: Provider::new(config.clone())?,
            config,
            admission_slots: tokio::sync::Semaphore::new(3),
        })
    }
    pub async fn begin(
        &self,
        store: &Store,
        request: NativeFlowRequest,
    ) -> Result<NativeFlow, AuthError> {
        if !protocol_secret(&request.state)
            || !protocol_secret(&request.nonce)
            || !challenge(&request.code_challenge)
        {
            return Err(AuthError::State);
        }
        let authorization_url = self
            .provider
            .authorization_url_with_challenge(
                &request.state,
                &request.nonce,
                &request.code_challenge,
                now()?,
            )
            .await?;
        let flow_id = uuid::Uuid::new_v4().to_string();
        store.native_begin(&self.config, &flow_id, &request).await?;
        Ok(NativeFlow {
            flow_id,
            authorization_url,
        })
    }
    pub async fn exchange(
        &self,
        store: &Store,
        flow_id: &str,
        request: NativeExchangeRequest,
    ) -> Result<NativeExchange, AuthError> {
        if uuid::Uuid::parse_str(flow_id).is_err()
            || !protocol_secret(&request.state)
            || !protocol_secret(&request.code_verifier)
            || request.code.is_empty()
            || request.code.len() > 4096
            || !request.code.bytes().all(|v| v.is_ascii_graphic())
        {
            return Err(AuthError::State);
        }
        // Durable consume commits before the sole remote exchange. Timeout/cancellation must
        // start a fresh flow, never redispatch the same code or infer remote rejection.
        let nonce = store
            .native_consume(
                &self.config,
                flow_id,
                &request.state,
                &pkce_challenge(&request.code_verifier),
            )
            .await?;
        let exchange = self
            .provider
            .exchange(&request.code, &request.code_verifier, &nonce, now)
            .await?;
        let expires = session_expiry(now()?, exchange.proof.expires(), exchange.access_expiry)?;
        let credential = random_secret()?;
        let view = store
            .native_finish(
                &self.config,
                flow_id,
                exchange.proof,
                &exchange.access_token,
                &credential,
                expires,
            )
            .await?;
        Ok(NativeExchange {
            actor_id: view.actor_id,
            session_id: view.session_id,
            access_token: credential,
            expires_at: view.expires_at,
        })
    }
    pub(crate) async fn authenticate(
        &self,
        store: &Store,
        token: &str,
    ) -> Result<NativeSession, AuthError> {
        if !valid_secret(token) {
            return Err(AuthError::InvalidToken);
        }
        let credential = store.native_credential(&self.config, token).await?;
        let access = self
            .provider
            .verify_native_resource(&credential.access_token, now)
            .await?;
        if access.key().issuer() != self.config.issuer
            || access.key().subject() != credential.subject
        {
            return Err(AuthError::Subject);
        }
        self.provider
            .introspect(&credential.access_token, &credential.subject, now)
            .await?;
        // Current provider revocation is observed per request; local revoke/disable during I/O
        // is refused by this reread and again under admission locks before dispatch.
        let current = store.native_credential(&self.config, token).await?;
        if current.view.expires_at > access.expires() {
            return Err(AuthError::Expired);
        }
        Ok(current.view)
    }
    pub async fn with_session_admission<F, Fut, T>(
        &self,
        store: &Store,
        token: &str,
        actor: &str,
        dispatch: F,
    ) -> Result<T, AuthError>
    where
        F: FnOnce(NativeSession) -> Fut,
        Fut: std::future::Future<Output = T>,
    {
        let _permit = tokio::time::timeout(
            std::time::Duration::from_secs(5),
            self.admission_slots.acquire(),
        )
        .await
        .map_err(|_| AuthError::Unavailable)?
        .map_err(|_| AuthError::Unavailable)?;
        store
            .native_admission(&self.config, token, actor, dispatch)
            .await
    }
    pub async fn logout(&self, store: &Store, token: &str) -> Result<bool, AuthError> {
        if !valid_secret(token) {
            return Err(AuthError::InvalidToken);
        }
        let access = store.native_revoke(&self.config, token).await?;
        match access {
            Some(access) => match self.provider.revoke(&access, now()?).await {
                Ok(()) => {
                    store.native_revocation_completed(token).await?;
                    Ok(true)
                }
                Err(_) => Ok(false),
            },
            None => Ok(true),
        }
    }
}
impl Store {
    pub fn with_theatre_native(mut self, native: NativeIdentity) -> Self {
        self.theatre_native = Some(std::sync::Arc::new(native));
        self
    }
    pub fn theatre_native(&self) -> Option<&NativeIdentity> {
        self.theatre_native.as_deref()
    }
}
fn protocol_secret(value: &str) -> bool {
    (43..=128).contains(&value.len())
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"-._~".contains(&v))
}
fn challenge(value: &str) -> bool {
    value.len() == 43
        && value
            .bytes()
            .all(|v| v.is_ascii_alphanumeric() || b"-_".contains(&v))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn native_protocol_inputs_have_exact_ascii_pkce_bounds() {
        for value in [
            "",
            "short",
            &"a".repeat(129),
            &"á".repeat(43),
            &" ".repeat(43),
        ] {
            assert!(!protocol_secret(value));
        }
        assert!(protocol_secret(&"a".repeat(43)));
        assert!(protocol_secret(&"~".repeat(128)));
        assert!(challenge(&"_".repeat(43)));
        assert!(!challenge(&"~".repeat(43)));
    }
}
