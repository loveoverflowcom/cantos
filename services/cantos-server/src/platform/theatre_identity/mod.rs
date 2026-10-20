//! Theatre BFF only. Never consumes a Studio or Admin credential.
mod postgres;

use crate::platform::oidc::config::OidcConfig;
pub use crate::platform::oidc::core::AuthError;
use crate::platform::oidc::core::{session_expiry, FLOW_SECONDS, SESSION_SECONDS};
use crate::platform::oidc::provider::Provider;
pub use crate::platform::oidc::trust::IssuerTrust;
use crate::platform::postgres::Store;

pub struct TheatreIdentity {
    config: OidcConfig,
    bearer: crate::platform::bearer::BearerIdentity,
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
}
pub struct LogoutComplete {
    pub provider_revoked: bool,
    pub cookies: Vec<String>,
}
pub enum GuardedLogout {
    Complete(LogoutComplete),
    SessionChanged(String),
}

impl TheatreIdentity {
    pub(crate) fn bearer(&self) -> &crate::platform::bearer::BearerIdentity {
        &self.bearer
    }
    pub fn new(issuer: &str, web_origin: &str, secret: String) -> Result<Self, AuthError> {
        Self::with_trust(issuer, web_origin, secret, None)
    }

    /// `trust` replaces the built-in roots for this issuer's clients only (OIDC and onboarding).
    pub fn with_trust(
        issuer: &str,
        web_origin: &str,
        secret: String,
        trust: Option<IssuerTrust>,
    ) -> Result<Self, AuthError> {
        Self::with_api_origin(issuer, web_origin, secret, trust, None)
    }

    pub fn with_api_origin(
        issuer: &str,
        web_origin: &str,
        secret: String,
        trust: Option<IssuerTrust>,
        api_origin: Option<&str>,
    ) -> Result<Self, AuthError> {
        let mut config = OidcConfig::theatre_web(issuer, web_origin, secret)?;
        if let Some(origin) = api_origin {
            config = config.with_api_callback_origin(origin)?;
        }
        if let Some(trust) = trust {
            config = config.with_trust(trust)?;
        }
        Ok(Self {
            bearer: crate::platform::bearer::BearerIdentity::new(
                config.clone(),
                crate::platform::bearer::client::Client::TheatreWeb,
            )?,
            provider: Provider::new(config.clone())?,
            // At most3 leases in the existing8-connection pool leaves capacity for dispatch.
            admission_slots: tokio::sync::Semaphore::new(3),
            config,
        })
    }
    pub(crate) fn web_return_origin(&self) -> &str {
        self.config.web_return_origin()
    }
    pub fn web_origin(&self) -> &str {
        &self.config.web_origin
    }

    /// The private issuer anchors, so registration's onboarding client trusts exactly the same.
    pub(crate) fn issuer_trust(&self) -> Option<&IssuerTrust> {
        self.config.trust()
    }

    pub(crate) fn registration_issuer_matches(&self, issuer: &str) -> bool {
        self.config.issuer == issuer
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
        // Guarded logout leaves the revoked HttpOnly cookie inert until expiry/replacement.
        // Only a positive local revoked/expired fact permits treating it as anonymous;
        // unknown cookies and provider/protocol uncertainty remain closed.
        let previous = match previous {
            Some(token) if !valid_secret(token) => return Err(AuthError::InvalidToken),
            Some(token) => match store.theatre_previous_is_inert(&self.config, token).await? {
                Some(true) => None,
                Some(false) => {
                    self.authenticate(store, token).await?;
                    Some(token)
                }
                None => return Err(AuthError::Inactive),
            },
            None => None,
        };
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
            cookie: self.cookie(
                &self.flow_cookie_name(&state),
                &browser,
                "Lax",
                FLOW_SECONDS,
            ),
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
        })
    }

    /// Cancels only the exact browser-bound flow, including a callback already in flight.
    /// Any session issued by this flow is revoked locally before best-effort provider cleanup;
    /// another flow/account's cookie and session are never selected or expired.
    pub async fn cancel_callback(
        &self,
        store: &Store,
        state: &str,
        browser: &str,
    ) -> Result<(), AuthError> {
        if !valid_secret(state) || !valid_secret(browser) {
            return Err(AuthError::State);
        }
        let batch = store
            .theatre_cancel_flow(&self.config, state, browser)
            .await?;
        for (hash, access) in batch {
            if self.provider.revoke(&access, now()?).await.is_ok() {
                store.theatre_revocation_hash_completed(&hash).await?;
            }
        }
        Ok(())
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
        let access = store
            .theatre_cancel_and_revoke(&self.config, token, browser)
            .await?;
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

    /// Expected UUID is a rejection fence, never authentication. Stale-only cleanup cannot
    /// revoke a newly valid cookie after another tab changes account.
    pub async fn guarded_logout(
        &self,
        store: &Store,
        token: Option<&str>,
        inventory: &[(String, String)],
        expected: Option<&str>,
        stale_only: bool,
    ) -> Result<GuardedLogout, AuthError> {
        if let Some(token) = token {
            if let Some(current) = store.theatre_session_id(&self.config, token).await? {
                if expected.is_some_and(|v| v != current) {
                    return Ok(GuardedLogout::SessionChanged(current));
                }
                if stale_only {
                    match self.authenticate(store, token).await {
                        Ok(_) => return Ok(GuardedLogout::SessionChanged(current)),
                        Err(AuthError::Inactive | AuthError::Expired) => {}
                        Err(
                            AuthError::Configuration
                            | AuthError::ProviderUnavailable
                            | AuthError::InvalidMetadata
                            | AuthError::Unavailable
                            | AuthError::InvalidToken
                            | AuthError::Algorithm
                            | AuthError::UnknownKey
                            | AuthError::InvalidKey
                            | AuthError::Signature
                            | AuthError::Issuer
                            | AuthError::Audience
                            | AuthError::NotYetValid
                            | AuthError::Nonce
                            | AuthError::Subject
                            | AuthError::State
                            | AuthError::Replay,
                        ) => {
                            // A transport/protocol failure establishes no revocation fact.
                            return Err(AuthError::Unavailable);
                        }
                    }
                }
            }
        }
        if expected.is_none() && !stale_only {
            return Err(AuthError::State);
        }
        let mut provider_revoked = true;
        if let Some(token) = token {
            // Local cancellation/revocation commits before bounded provider effects. Only a
            // still-active expected session may cancel its proven browser inventory; stale
            // A cleanup cannot cancel B's newly intended anonymous flow.
            let batch = store
                .theatre_guarded_revoke(&self.config, token, expected, stale_only, inventory)
                .await?;
            provider_revoked =
                match tokio::time::timeout(std::time::Duration::from_secs(5), async {
                    let mut completed = true;
                    for (hash, access) in batch {
                        if self.provider.revoke(&access, now()?).await.is_ok() {
                            store.theatre_revocation_hash_completed(&hash).await?;
                        } else {
                            completed = false;
                        }
                    }
                    Ok::<bool, AuthError>(completed)
                })
                .await
                {
                    Ok(result) => result?,
                    Err(_) => false,
                };
        }
        Ok(GuardedLogout::Complete(LogoutComplete {
            provider_revoked,
            cookies: Vec::new(),
        }))
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
    pub fn flow_cookie_name(&self, state: &str) -> String {
        format!("{}_{state}", self.flow_name())
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
mod native_session;
pub use native_session::NativeIdentity;
