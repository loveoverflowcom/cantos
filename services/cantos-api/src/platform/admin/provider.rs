//! Operator-pinned Admin resource client, bounded normal-TLS HTTP and current role observation.
use super::{
    credential,
    domain::{AdminError, RoleEvidence},
    CLIENT_ID,
};
use crate::platform::oidc::core::JwkSet;
use reqwest::{Client, Url};
use serde::Deserialize;
use std::time::Duration;
use tokio::sync::Mutex;

#[derive(Clone)]
pub struct AdminConfig {
    issuer: String,
    origin: Url,
    secret: String,
}
impl AdminConfig {
    pub fn new(issuer: &str, secret: String) -> Result<Self, AdminError> {
        let url = checked_url(issuer)?;
        if url.path() != format!("/oauth2/openid/{CLIENT_ID}")
            || secret.is_empty()
            || secret.len() > 4096
            || secret.chars().any(char::is_control)
        {
            return Err(AdminError::Configuration);
        }
        Ok(Self {
            issuer: issuer.into(),
            origin: url,
            secret,
        })
    }
    fn endpoint(&self, value: &str) -> Result<Url, AdminError> {
        let url = checked_url(value).map_err(|_| AdminError::ProviderUnavailable)?;
        if url.origin() != self.origin.origin() {
            return Err(AdminError::ProviderUnavailable);
        }
        Ok(url)
    }
}
fn checked_url(value: &str) -> Result<Url, AdminError> {
    let url = Url::parse(value).map_err(|_| AdminError::Configuration)?;
    if (url.scheme() != "https"
        && !(url.scheme() == "http" && matches!(url.host_str(), Some("127.0.0.1" | "[::1]"))))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.as_str() != value
    {
        return Err(AdminError::Configuration);
    }
    Ok(url)
}
#[derive(Deserialize)]
struct Metadata {
    issuer: String,
    jwks_uri: String,
    introspection_endpoint: String,
}
struct Cached {
    metadata: Metadata,
    keys: JwkSet,
    loaded: u64,
}
struct Cache {
    entry: Option<Cached>,
    attempted: Option<u64>,
}
#[derive(Deserialize)]
struct Observation {
    active: bool,
    sub: Option<String>,
    iss: Option<String>,
    aud: Option<String>,
    client_id: Option<String>,
    exp: Option<u64>,
    #[serde(default)]
    cantos_admin_groups: Vec<String>,
}
pub(super) struct AdminProvider {
    config: AdminConfig,
    client: Client,
    cache: Mutex<Cache>,
}
impl AdminProvider {
    pub(super) fn new(config: AdminConfig) -> Result<Self, AdminError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| AdminError::Configuration)?;
        Ok(Self {
            config,
            client,
            cache: Mutex::new(Cache {
                entry: None,
                attempted: None,
            }),
        })
    }
    async fn get<T: serde::de::DeserializeOwned>(&self, url: &str) -> Result<T, AdminError> {
        bounded(
            self.client
                .get(self.config.endpoint(url)?)
                .send()
                .await
                .map_err(|_| AdminError::ProviderUnavailable)?,
        )
        .await
    }
    async fn load(&self, now: u64) -> Result<Cached, AdminError> {
        let metadata: Metadata = self
            .get(&format!(
                "{}/.well-known/openid-configuration",
                self.config.issuer
            ))
            .await?;
        if metadata.issuer != self.config.issuer {
            return Err(AdminError::ProviderUnavailable);
        }
        self.config.endpoint(&metadata.introspection_endpoint)?;
        let keys: JwkSet = self.get(&metadata.jwks_uri).await?;
        if keys.keys.is_empty() || keys.keys.len() > 16 {
            return Err(AdminError::ProviderUnavailable);
        }
        Ok(Cached {
            metadata,
            keys,
            loaded: now,
        })
    }
    pub(super) async fn authenticate(&self, token: &str) -> Result<RoleEvidence, AdminError> {
        if token.len() > 16_384 {
            return Err(AdminError::Credential);
        }
        crate::platform::oidc::core::token_kid(token).map_err(|_| AdminError::Credential)?;
        if token.split('.').count() != 3 {
            return Err(AdminError::Credential);
        }
        let now = super::now()?;
        let (proof, endpoint) = {
            let mut cache = self.cache.lock().await;
            if cache
                .entry
                .as_ref()
                .is_none_or(|v| now < v.loaded || now - v.loaded >= 300)
            {
                if cache.attempted.is_some_and(|v| now >= v && now - v < 10) {
                    return Err(AdminError::ProviderUnavailable);
                }
                cache.attempted = Some(now);
                cache.entry = None;
                cache.entry = Some(self.load(now).await?);
            }
            let cached = cache
                .entry
                .as_ref()
                .ok_or(AdminError::ProviderUnavailable)?;
            // Invalid token/unknown key fail closed. An explicit 300s metadata reload observes rotation;
            // no attacker-triggered network retry is permitted for a failed signature.
            (
                credential::verify(token, &cached.keys, &self.config.issuer, super::now()?)?,
                cached.metadata.introspection_endpoint.clone(),
            )
        };
        let observation: Observation = bounded(
            self.client
                .post(self.config.endpoint(&endpoint)?)
                .basic_auth(CLIENT_ID, Some(&self.config.secret))
                .form(&[("token", token)])
                .send()
                .await
                .map_err(|_| AdminError::ProviderUnavailable)?,
        )
        .await?;
        if !observation.active
            || observation.sub.as_deref() != Some(proof.subject())
            || observation.iss.as_deref() != Some(self.config.issuer.as_str())
            || observation.aud.as_deref() != Some(CLIENT_ID)
            || observation.client_id.as_deref() != Some(CLIENT_ID)
        {
            return Err(AdminError::Credential);
        }
        proof.observe(
            &observation.cantos_admin_groups,
            observation.exp.ok_or(AdminError::Credential)?,
            super::now()?,
        )
    }
}
async fn bounded<T: serde::de::DeserializeOwned>(
    mut response: reqwest::Response,
) -> Result<T, AdminError> {
    if !response.status().is_success() || response.content_length().is_some_and(|n| n > 65_536) {
        return Err(AdminError::ProviderUnavailable);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AdminError::ProviderUnavailable)?
    {
        if bytes.len() + chunk.len() > 65_536 {
            return Err(AdminError::ProviderUnavailable);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| AdminError::ProviderUnavailable)
}
