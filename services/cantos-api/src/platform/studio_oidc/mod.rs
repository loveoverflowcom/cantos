//! Studio BFF only. No Listener/Admin identity, no production activation, no credential DTO.
mod administrators;
mod config;
use crate::platform::oidc::core;
mod postgres;
mod provider;
#[cfg(test)]
mod tests;

pub use crate::platform::oidc::config::OidcConfig;
pub use crate::platform::oidc::core::{AuthError, CLIENT_ID};
use crate::platform::postgres::Store;
use core::{session_expiry, FLOW_SECONDS, SESSION_SECONDS};
use provider::Provider;

pub const MAX_BROWSER_FLOWS: usize = 8;
pub(super) struct BrowserFlow {
    pub state_hash: Vec<u8>,
    pub browser_hash: Vec<u8>,
}
/// Opaque cookie possession bounds cancellation to this browser's supplied flows. It never
/// verifies an identity or grants authority. Credentials have no Debug/serde representation.
pub struct BrowserInventory {
    pub(super) flows: Vec<BrowserFlow>,
}
impl BrowserInventory {
    pub fn new(values: Vec<(String, String)>) -> Result<Self, AuthError> {
        if values.len() > MAX_BROWSER_FLOWS {
            return Err(AuthError::State);
        }
        let mut seen = std::collections::BTreeSet::new();
        let mut flows = Vec::new();
        for (state, browser) in values {
            if !valid_secret(&state) || !valid_secret(&browser) || !seen.insert(state.clone()) {
                return Err(AuthError::State);
            }
            flows.push(BrowserFlow {
                state_hash: crate::platform::postgres::token_hash(&state),
                browser_hash: crate::platform::postgres::token_hash(&browser),
            });
        }
        Ok(Self { flows })
    }
    pub fn len(&self) -> usize {
        self.flows.len()
    }
    pub fn is_empty(&self) -> bool {
        self.flows.is_empty()
    }
}

pub struct StudioOidc {
    config: OidcConfig,
    bearer: crate::platform::bearer::BearerIdentity,
    provider: Provider,
    administrators: Option<administrators::StudioAdministrators>,
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

impl StudioOidc {
    pub(crate) fn bearer(&self) -> &crate::platform::bearer::BearerIdentity {
        &self.bearer
    }
    pub fn new(config: OidcConfig) -> Result<Self, AuthError> {
        if config.client_id != CLIENT_ID {
            return Err(AuthError::Configuration);
        }
        Ok(Self {
            bearer: crate::platform::bearer::BearerIdentity::new(
                config.clone(),
                crate::platform::bearer::core::Client::Studio,
            )?,
            provider: Provider::new(config.clone())?,
            config,
            administrators: None,
        })
    }

    /// Admit only one or two operator-provisioned exact subjects at this Studio issuer.
    /// Without this roster the client supports metadata inspection only, never login.
    pub fn with_admin_subjects(mut self, subjects: Vec<String>) -> Result<Self, AuthError> {
        self.administrators = Some(administrators::StudioAdministrators::new(
            &self.config.issuer,
            subjects,
        )?);
        Ok(self)
    }

    pub(crate) fn authorize_bearer_subject(&self, subject: &str) -> Result<(), AuthError> {
        let key = crate::platform::identity::domain::ExternalIdentityKey::new(
            &self.config.issuer,
            subject,
        )
        .map_err(|_| AuthError::Subject)?;
        self.administrators()?.authorize(&key)
    }
    fn administrators(&self) -> Result<&administrators::StudioAdministrators, AuthError> {
        self.administrators.as_ref().ok_or(AuthError::Configuration)
    }
    pub(crate) fn web_return_origin(&self) -> &str {
        self.config.web_return_origin()
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
        self.administrators()?;
        let previous = match previous {
            Some(previous)
                if valid_secret(previous)
                    && store.oidc_known_inert(&self.config, previous).await? =>
            {
                None
            }
            Some(previous) => {
                self.authenticate(store, previous).await?;
                Some(previous)
            }
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
            .oidc_begin(&self.config, &state, &browser, &nonce, &verifier, previous)
            .await?;
        Ok(LoginStart {
            url,
            cookie: self.cookie(
                &self.flow_cookie_name(&state)?,
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
        let administrators = self.administrators()?;
        if !valid_secret(state)
            || !valid_secret(browser)
            || code.is_empty()
            || code.len() > 4096
            || code.chars().any(char::is_control)
        {
            return Err(AuthError::State);
        }
        let flow = store.oidc_consume(&self.config, state, browser).await?;
        let exchange = self
            .provider
            .exchange(code, &flow.verifier, &flow.nonce, now)
            .await?;
        // Authentication never enrolls a listener or an arbitrary provider account in Studio.
        // Refuse before registry/session issuance; browser claims cannot edit this roster.
        administrators.authorize(exchange.proof.key())?;
        // Recheck time after network/lock waits. SQL also rechecks against its own commit clock.
        let time = now()?;
        let expires = session_expiry(time, exchange.proof.expires(), exchange.access_expiry)?;
        let token = random_secret()?;
        let actor = store
            .oidc_finish(
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

    pub(crate) async fn authenticate(&self, store: &Store, token: &str) -> Result<(), AuthError> {
        let administrators = self.administrators()?;
        if !valid_secret(token) {
            return Err(AuthError::InvalidToken);
        }
        let credential = store.oidc_credential(&self.config, token).await?;
        let identity = crate::platform::identity::domain::ExternalIdentityKey::new(
            &self.config.issuer,
            &credential.subject,
        )
        .map_err(|_| AuthError::Subject)?;
        administrators.authorize(&identity)?;
        self.provider
            .introspect(&credential.access_token, &credential.subject, now)
            .await?;
        Ok(())
    }

    pub async fn logout(
        &self,
        store: &Store,
        token: Option<&str>,
        browser: Option<&str>,
    ) -> Result<LogoutComplete, AuthError> {
        self.logout_inner(store, token, browser, None).await
    }
    pub async fn logout_in_browser(
        &self,
        store: &Store,
        token: &str,
        inventory: &BrowserInventory,
    ) -> Result<LogoutComplete, AuthError> {
        self.logout_inner(store, Some(token), None, Some(inventory))
            .await
    }
    async fn logout_inner(
        &self,
        store: &Store,
        token: Option<&str>,
        browser: Option<&str>,
        inventory: Option<&BrowserInventory>,
    ) -> Result<LogoutComplete, AuthError> {
        let access = store
            .oidc_cancel_and_revoke(&self.config, token, browser, inventory)
            .await?;
        // Local revocation commits first. Retain durable pending evidence on remote outage.
        let mut provider_revoked = match (access, token) {
            (postgres::RevocationCredential::Current(access), Some(token)) => {
                match self.provider.revoke(&access, now()?).await {
                    Ok(()) => {
                        store.oidc_revocation_completed(token).await?;
                        true
                    }
                    Err(_) => false,
                }
            }
            (postgres::RevocationCredential::ForeignIssuer, _) => false,
            (postgres::RevocationCredential::PendingOnly, _) => false,
            (postgres::RevocationCredential::None, _) => true,
            (postgres::RevocationCredential::Current(_), None) => false,
        };
        if provider_revoked {
            if let Some(token) = token {
                provider_revoked = !store.oidc_pending_siblings(token).await?;
            }
        }
        Ok(LogoutComplete {
            provider_revoked,
            cookies: vec![
                self.cookie(self.session_name(), "", "Strict", 0),
                self.cookie(self.flow_name(), "", "Lax", 0),
                // Retire the two known pre-OIDC Studio mounts. Browser cookie identity includes
                // Path, so clearing only the new Secure name leaves legacy cookies shadowing it.
                "cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1/studio; Max-Age=0".into(),
                "cantos_session=; HttpOnly; SameSite=Strict; Path=/api/v1; Max-Age=0".into(),
                "cantos_oidc=; HttpOnly; SameSite=Lax; Path=/api/v1/studio; Max-Age=0".into(),
            ],
        })
    }

    pub fn session_name(&self) -> &'static str {
        if self.config.secure {
            "__Secure-cantos_studio"
        } else {
            "cantos_session"
        }
    }
    pub fn flow_name(&self) -> &'static str {
        if self.config.secure {
            "__Secure-cantos_oidc"
        } else {
            "cantos_oidc"
        }
    }
    pub fn flow_cookie_name(&self, state: &str) -> Result<String, AuthError> {
        if !valid_secret(state) {
            return Err(AuthError::State);
        }
        Ok(format!("{}_{state}", self.flow_name()))
    }
    fn cookie(&self, name: &str, value: &str, same_site: &str, age: u64) -> String {
        let secure = if self.config.secure { "; Secure" } else { "" };
        format!("{name}={value}; HttpOnly; SameSite={same_site}; Path=/api/v1/studio; Max-Age={age}{secure}")
    }
}

impl Store {
    pub fn with_studio_oidc(mut self, oidc: StudioOidc) -> Self {
        self.studio_oidc = Some(std::sync::Arc::new(oidc));
        self
    }
    pub fn studio_oidc(&self) -> Option<&StudioOidc> {
        self.studio_oidc.as_deref()
    }
}

fn valid_secret(value: &str) -> bool {
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
