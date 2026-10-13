//! Theatre BFF only. Never consumes a Studio or Admin credential.
mod postgres;

use crate::platform::oidc::config::OidcConfig;
pub use crate::platform::oidc::core::AuthError;
use crate::platform::oidc::core::{session_expiry, FLOW_SECONDS, SESSION_SECONDS};
use crate::platform::oidc::provider::Provider;
use crate::platform::postgres::Store;

pub struct TheatreIdentity {
    config: OidcConfig,
    provider: Provider,
    admission_slots: tokio::sync::Semaphore,
}

/// Browser receives only random opaque values; no provider bearer or verified proof is returned.
pub struct LoginStart {
    pub url: String,
    pub cookie: String,
}
pub struct LoginComplete {
    pub actor_id: String,
    pub cookie: String,
    pub clear_flow: String,
}
pub struct LogoutComplete {
    pub provider_revoked: bool,
    pub cookies: Vec<String>,
}

impl TheatreIdentity {
    pub fn new(issuer: &str, web_origin: &str, secret: String) -> Result<Self, AuthError> {
        let config = OidcConfig::theatre_web(issuer, web_origin, secret)?;
        Ok(Self {
            provider: Provider::new(config.clone())?,
            // At most3 leases in the existing8-connection pool leaves capacity for dispatch.
            admission_slots: tokio::sync::Semaphore::new(3),
            config,
        })
    }
    pub fn web_origin(&self) -> &str {
        &self.config.web_origin
    }

    /// Read-only discovery/JWKS contract check; success is metadata evidence, never a live login.
    pub async fn verify_metadata(&self) -> Result<(), AuthError> {
        self.provider.verify_metadata(now()?).await
    }

    pub async fn begin(
        &self,
        store: &Store,
        previous: Option<&str>,
    ) -> Result<LoginStart, AuthError> {
        if let Some(previous) = previous {
            self.authenticate(store, previous).await?;
        }
        let state = random_secret()?;
        let browser = random_secret()?;
        let nonce = random_secret()?;
        let verifier = random_secret()?;
        let url = self
            .provider
            .authorization_url(&state, &nonce, &verifier, now()?)
            .await?;
        store
            .theatre_begin(&self.config, &state, &browser, &nonce, &verifier, previous)
            .await?;
        Ok(LoginStart {
            url,
            cookie: self.cookie(self.flow_name(), &browser, "Lax", FLOW_SECONDS),
        })
    }

    pub async fn callback(
        &self,
        store: &Store,
        state: &str,
        browser: &str,
        code: &str,
    ) -> Result<LoginComplete, AuthError> {
        if !valid_secret(state)
            || !valid_secret(browser)
            || code.is_empty()
            || code.len() > 4096
            || code.chars().any(char::is_control)
        {
            return Err(AuthError::State);
        }
        let flow = store.theatre_consume(&self.config, state, browser).await?;
        let exchange = self
            .provider
            .exchange(code, &flow.verifier, &flow.nonce, now)
            .await?;
        // Recheck time after network/lock waits. SQL also rechecks against its own commit clock.
        let time = now()?;
        let expires = session_expiry(time, exchange.proof.expires(), exchange.access_expiry)?;
        let token = random_secret()?;
        let actor = store
            .theatre_finish(
                &self.config,
                state,
                exchange.proof,
                &exchange.access_token,
                &token,
                expires,
            )
            .await?;
        Ok(LoginComplete {
            actor_id: actor,
            cookie: self.cookie(
                self.session_name(),
                &token,
                "Strict",
                expires.saturating_sub(time).min(SESSION_SECONDS),
            ),
            clear_flow: self.cookie(self.flow_name(), "", "Lax", 0),
        })
    }

    pub(crate) async fn authenticate(
        &self,
        store: &Store,
        token: &str,
    ) -> Result<String, AuthError> {
        if !valid_secret(token) {
            return Err(AuthError::InvalidToken);
        }
        let credential = store.theatre_credential(&self.config, token).await?;
        self.provider
            .introspect(&credential.access_token, &credential.subject, now)
            .await?;
        // Re-read after network: logout/disable/expiry during introspection must refuse.
        Ok(store.theatre_credential(&self.config, token).await?.actor)
    }

    /// Owns the admission lease across the actual account dispatch; no provider I/O inside.
    /// This concrete BFF consumer keeps transaction/session mechanics outside HTTP.
    pub async fn with_session_admission<F, Fut, T>(
        &self,
        store: &Store,
        token: &str,
        actor: &str,
        dispatch: F,
    ) -> Result<T, AuthError>
    where
        F: FnOnce(String) -> Fut,
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
            .theatre_admission(&self.config, token, actor, dispatch)
            .await
    }

    pub async fn logout(
        &self,
        store: &Store,
        token: Option<&str>,
        browser: Option<&str>,
    ) -> Result<LogoutComplete, AuthError> {
        let access = store.theatre_cancel_and_revoke(token, browser).await?;
        // Local revocation commits first. Retain durable pending evidence on remote outage.
        let provider_revoked = match (access, token) {
            (Some(access), Some(token)) => match self.provider.revoke(&access, now()?).await {
                Ok(()) => {
                    store.theatre_revocation_completed(token).await?;
                    true
                }
                Err(_) => false,
            },
            _ => true,
        };
        Ok(LogoutComplete {
            provider_revoked,
            cookies: vec![
                self.cookie(self.session_name(), "", "Strict", 0),
                self.cookie(self.flow_name(), "", "Lax", 0),
            ],
        })
    }

    pub fn session_name(&self) -> &'static str {
        if self.config.secure {
            "__Secure-cantos_theatre"
        } else {
            "cantos_theatre"
        }
    }
    pub fn flow_name(&self) -> &'static str {
        if self.config.secure {
            "__Secure-cantos_theatre_flow"
        } else {
            "cantos_theatre_flow"
        }
    }
    fn cookie(&self, name: &str, value: &str, same_site: &str, age: u64) -> String {
        let secure = if self.config.secure { "; Secure" } else { "" };
        format!("{name}={value}; HttpOnly; SameSite={same_site}; Path=/api/v1/theatre; Max-Age={age}{secure}")
    }
}

impl Store {
    pub fn with_theatre_identity(mut self, oidc: TheatreIdentity) -> Self {
        self.theatre_identity = Some(std::sync::Arc::new(oidc));
        self
    }
    pub fn theatre_identity(&self) -> Option<&TheatreIdentity> {
        self.theatre_identity.as_deref()
    }
}

pub(crate) fn valid_secret(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
}
fn random_secret() -> Result<String, AuthError> {
    let mut bytes = [0u8; 32];
    getrandom::fill(&mut bytes).map_err(|_| AuthError::Unavailable)?;
    Ok(bytes.iter().map(|v| format!("{v:02x}")).collect())
}
fn now() -> Result<u64, AuthError> {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|v| v.as_secs())
        .map_err(|_| AuthError::Unavailable)
}

#[cfg(test)]
mod tests;

pub mod native;
