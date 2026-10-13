//! Bounded HTTP shell. One exact issuer, same-origin endpoints, no redirects or TLS bypass.
use reqwest::{Client, Response};
use serde::{de::DeserializeOwned, Deserialize};
use std::time::Duration;
use tokio::sync::Mutex;

use super::{
    config::OidcConfig,
    core::{self, AuthError, JwkSet, VerifiedIdentity, CLIENT_ID},
};

#[derive(Clone, Deserialize)]
struct Discovery {
    issuer: String,
    authorization_endpoint: String,
    token_endpoint: String,
    jwks_uri: String,
    introspection_endpoint: String,
    revocation_endpoint: String,
    code_challenge_methods_supported: Vec<String>,
    id_token_signing_alg_values_supported: Vec<String>,
    token_endpoint_auth_methods_supported: Vec<String>,
    response_types_supported: Vec<String>,
}

struct Cached {
    discovery: Discovery,
    keys: JwkSet,
    loaded: u64,
    last_refresh: u64,
}

#[derive(Default)]
struct ProviderCache {
    entry: Option<Cached>,
    last_load: Option<u64>,
}

pub(super) struct Provider {
    config: OidcConfig,
    client: Client,
    cache: Mutex<ProviderCache>,
}

#[derive(Deserialize)]
struct Tokens {
    id_token: String,
    access_token: String,
    token_type: String,
}

#[derive(Deserialize)]
struct Introspection {
    active: bool,
    sub: Option<String>,
    aud: Option<String>,
    iss: Option<String>,
    client_id: Option<String>,
    exp: Option<u64>,
}

pub(super) struct Exchange {
    pub proof: VerifiedIdentity,
    pub access_token: String,
    pub access_expiry: u64,
}

impl Provider {
    pub fn new(config: OidcConfig) -> Result<Self, AuthError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| AuthError::Configuration)?;
        Ok(Self {
            config,
            client,
            cache: Mutex::new(ProviderCache::default()),
        })
    }

    async fn load(&self, now: u64) -> Result<Cached, AuthError> {
        let discovery: Discovery = bounded(
            self.client
                .get(format!(
                    "{}/.well-known/openid-configuration",
                    self.config.issuer
                ))
                .send()
                .await
                .map_err(|_| AuthError::ProviderUnavailable)?,
        )
        .await?;
        if discovery.issuer != self.config.issuer
            || !discovery
                .code_challenge_methods_supported
                .iter()
                .any(|v| v == "S256")
            || !discovery
                .id_token_signing_alg_values_supported
                .iter()
                .any(|v| v == "ES256")
            || !discovery
                .token_endpoint_auth_methods_supported
                .iter()
                .any(|v| v == "client_secret_basic")
            || !discovery
                .response_types_supported
                .iter()
                .any(|v| v == "code")
        {
            return Err(AuthError::InvalidMetadata);
        }
        for endpoint in [
            &discovery.authorization_endpoint,
            &discovery.token_endpoint,
            &discovery.jwks_uri,
            &discovery.introspection_endpoint,
            &discovery.revocation_endpoint,
        ] {
            self.config.endpoint(endpoint)?;
        }
        let keys = self.fetch_keys(&discovery).await?;
        Ok(Cached {
            discovery,
            keys,
            loaded: now,
            last_refresh: now,
        })
    }

    async fn fetch_keys(&self, discovery: &Discovery) -> Result<JwkSet, AuthError> {
        let keys: JwkSet = bounded(
            self.client
                .get(&discovery.jwks_uri)
                .send()
                .await
                .map_err(|_| AuthError::ProviderUnavailable)?,
        )
        .await?;
        if keys.keys.is_empty() || keys.keys.len() > 16 {
            return Err(AuthError::InvalidKey);
        }
        Ok(keys)
    }

    async fn discovery(&self, now: u64) -> Result<Discovery, AuthError> {
        let mut cache = self.cache.lock().await;
        if cache
            .entry
            .as_ref()
            .is_none_or(|v| now < v.loaded || now.saturating_sub(v.loaded) >= 300)
        {
            // Invalidate before I/O. An outage cannot silently extend expired discovery/keys.
            if cache.last_load.is_some_and(|v| now >= v && now - v < 10) {
                return Err(AuthError::ProviderUnavailable);
            }
            cache.last_load = Some(now);
            cache.entry = None;
            cache.entry = Some(self.load(now).await?);
        }
        cache
            .entry
            .as_ref()
            .map(|v| v.discovery.clone())
            .ok_or(AuthError::ProviderUnavailable)
    }

    pub async fn authorization_url(
        &self,
        state: &str,
        nonce: &str,
        verifier: &str,
        now: u64,
    ) -> Result<String, AuthError> {
        let discovery = self.discovery(now).await?;
        let mut url = self.config.endpoint(&discovery.authorization_endpoint)?;
        url.query_pairs_mut().extend_pairs([
            ("client_id", CLIENT_ID),
            ("response_type", "code"),
            ("scope", "openid"),
            ("redirect_uri", self.config.redirect.as_str()),
            ("state", state),
            ("nonce", nonce),
            ("code_challenge_method", "S256"),
            ("code_challenge", core::pkce_challenge(verifier).as_str()),
        ]);
        Ok(url.into())
    }

    pub async fn exchange(
        &self,
        code: &str,
        verifier: &str,
        nonce: &str,
        clock: impl Fn() -> Result<u64, AuthError>,
    ) -> Result<Exchange, AuthError> {
        let discovery = self.discovery(clock()?).await?;
        // Never retry: the provider may have consumed the code even if this response is lost.
        let tokens: Tokens = bounded(
            self.client
                .post(&discovery.token_endpoint)
                .basic_auth(
                    oauth_component(CLIENT_ID),
                    Some(oauth_component(&self.config.secret)),
                )
                .form(&[
                    ("grant_type", "authorization_code"),
                    ("code", code),
                    ("redirect_uri", self.config.redirect.as_str()),
                    ("code_verifier", verifier),
                ])
                .send()
                .await
                .map_err(|_| AuthError::ProviderUnavailable)?,
        )
        .await?;
        if !tokens.token_type.eq_ignore_ascii_case("bearer")
            || tokens.access_token.is_empty()
            || tokens.access_token.len() > 16_384
        {
            return Err(AuthError::InvalidToken);
        }
        core::token_kid(&tokens.id_token)?;
        let proof = {
            let mut cache = self.cache.lock().await;
            let cached = cache.entry.as_mut().ok_or(AuthError::ProviderUnavailable)?;
            let now = clock()?;
            let result = core::verify(
                &tokens.id_token,
                &cached.keys,
                &self.config.issuer,
                nonce,
                now,
            );
            if matches!(result, Err(AuthError::UnknownKey | AuthError::Signature))
                && now.saturating_sub(cached.last_refresh) >= 10
            {
                // Serialized, at most one refresh per 10 seconds including failed refreshes.
                cached.last_refresh = now;
                cached.keys = self.fetch_keys(&cached.discovery).await?;
                core::verify(
                    &tokens.id_token,
                    &cached.keys,
                    &self.config.issuer,
                    nonce,
                    clock()?,
                )?
            } else {
                result?
            }
        };
        let access_expiry = self
            .introspect(&tokens.access_token, proof.key().subject(), &clock)
            .await?;
        Ok(Exchange {
            proof,
            access_token: tokens.access_token,
            access_expiry,
        })
    }

    pub async fn introspect(
        &self,
        token: &str,
        subject: &str,
        clock: impl Fn() -> Result<u64, AuthError>,
    ) -> Result<u64, AuthError> {
        let discovery = self.discovery(clock()?).await?;
        let facts: Introspection = bounded(
            self.client
                .post(&discovery.introspection_endpoint)
                .basic_auth(
                    oauth_component(CLIENT_ID),
                    Some(oauth_component(&self.config.secret)),
                )
                .form(&[("token", token)])
                .send()
                .await
                .map_err(|_| AuthError::ProviderUnavailable)?,
        )
        .await?;
        if !facts.active {
            return Err(AuthError::Inactive);
        }
        if facts.sub.as_deref() != Some(subject)
            || facts.aud.as_deref() != Some(CLIENT_ID)
            || facts
                .iss
                .as_deref()
                .is_some_and(|v| v != self.config.issuer)
            || facts.client_id.as_deref().is_some_and(|v| v != CLIENT_ID)
        {
            return Err(AuthError::Audience);
        }
        let expires = facts.exp.ok_or(AuthError::InvalidToken)?;
        if clock()? >= expires {
            return Err(AuthError::Expired);
        }
        Ok(expires)
    }

    pub async fn revoke(&self, token: &str, now: u64) -> Result<(), AuthError> {
        let discovery = self.discovery(now).await?;
        let response = self
            .client
            .post(&discovery.revocation_endpoint)
            .basic_auth(
                oauth_component(CLIENT_ID),
                Some(oauth_component(&self.config.secret)),
            )
            .form(&[("token", token), ("token_type_hint", "access_token")])
            .send()
            .await
            .map_err(|_| AuthError::ProviderUnavailable)?;
        if !response.status().is_success() {
            return Err(AuthError::ProviderUnavailable);
        }
        Ok(())
    }

    pub async fn verify_metadata(&self, now: u64) -> Result<(), AuthError> {
        self.discovery(now).await.map(|_| ())
    }
}

// RFC6749§2.3.1 applies form encoding to each credential before the Basic encoding.
// reqwest's generic HTTP Basic method deliberately does not perform OAuth encoding.
fn oauth_component(value: &str) -> String {
    url::form_urlencoded::byte_serialize(value.as_bytes()).collect()
}

async fn bounded<T: DeserializeOwned>(mut response: Response) -> Result<T, AuthError> {
    if !response.status().is_success() || response.content_length().is_some_and(|v| v > 65_536) {
        return Err(AuthError::ProviderUnavailable);
    }
    let mut body = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| AuthError::ProviderUnavailable)?
    {
        if body.len() + chunk.len() > 65_536 {
            return Err(AuthError::ProviderUnavailable);
        }
        body.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&body).map_err(|_| AuthError::InvalidMetadata)
}
