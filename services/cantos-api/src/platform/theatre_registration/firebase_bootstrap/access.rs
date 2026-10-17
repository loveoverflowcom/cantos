//! In-RAM OAuth access token for the one Auth call that needs a bearer (`accounts:signUp`).
//! The service-account key signs a JWT-bearer assertion that is exchanged at the fixed Google
//! token origin. The token is cached only in this process, renewed lazily before expiry with a
//! safety margin and never logged, persisted or accepted from configuration. Every failure is a
//! closed, sanitized `BootstrapError`; there is no fallback credential, emulator or endpoint.

use std::time::{Duration, Instant};

use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

use super::{
    fixed_origin_client, read_json_body, transport_failure, valid_opaque_token, AdminAccessToken,
    BootstrapError, Client, FirebaseBootstrapConfig, StatusCode, Url,
};

const TOKEN_ENDPOINT: &str = "https://oauth2.googleapis.com/token";
const GRANT_TYPE: &str = "urn:ietf:params:oauth:grant-type:jwt-bearer";
/// Narrowest documented scope for `accounts:signUp`; the other accepted one is cloud-platform.
const ACCESS_SCOPE: &str = "https://www.googleapis.com/auth/identitytoolkit";
const ASSERTION_SECONDS: u64 = 300;
/// A cached token is replaced this long before Google's stated expiry.
const REFRESH_MARGIN: Duration = Duration::from_secs(300);
const MAX_LIFETIME_SECONDS: u64 = 3600;
const MINT_TIMEOUT: Duration = Duration::from_secs(5);

struct Cached {
    token: AdminAccessToken,
    refresh_at: Instant,
}

impl Cached {
    fn is_fresh(&self, now: Instant) -> bool {
        now < self.refresh_at
    }
}

/// One process-wide holder. The async mutex is held across a refresh, so concurrent callers
/// share a single token request (single flight) and never observe a half-written cache.
pub(super) struct AccessTokenSource {
    client: Client,
    endpoint: Url,
    cache: Mutex<Option<Cached>>,
}

impl AccessTokenSource {
    pub(super) fn new() -> Result<Self, BootstrapError> {
        Self::build(
            Url::parse(TOKEN_ENDPOINT).map_err(|_| BootstrapError::Configuration)?,
            MINT_TIMEOUT,
            None,
        )
    }

    fn build(
        endpoint: Url,
        timeout: Duration,
        seed: Option<Cached>,
    ) -> Result<Self, BootstrapError> {
        Ok(Self {
            client: fixed_origin_client(timeout)?,
            endpoint,
            cache: Mutex::new(seed),
        })
    }

    pub(super) async fn token(
        &self,
        config: &FirebaseBootstrapConfig,
    ) -> Result<AdminAccessToken, BootstrapError> {
        let epoch = u64::try_from(chrono::Utc::now().timestamp())
            .map_err(|_| BootstrapError::InvalidClock)?;
        self.token_at(config, Instant::now(), epoch).await
    }

    async fn token_at(
        &self,
        config: &FirebaseBootstrapConfig,
        now: Instant,
        epoch_seconds: u64,
    ) -> Result<AdminAccessToken, BootstrapError> {
        let mut cache = self.cache.lock().await;
        if let Some(cached) = cache.as_ref().filter(|cached| cached.is_fresh(now)) {
            return Ok(cached.token.clone());
        }
        // Drop the stale value before any await so a failed refresh never serves it again.
        *cache = None;
        let minted = self.mint(config, now, epoch_seconds).await?;
        let token = minted.token.clone();
        *cache = Some(minted);
        Ok(token)
    }

    async fn mint(
        &self,
        config: &FirebaseBootstrapConfig,
        now: Instant,
        epoch_seconds: u64,
    ) -> Result<Cached, BootstrapError> {
        let assertion = config.access_assertion(epoch_seconds)?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", GRANT_TYPE)
            .append_pair("assertion", &assertion)
            .finish();
        let response = self
            .client
            .post(self.endpoint.clone())
            .header(reqwest::header::ACCEPT, "application/json")
            .header(
                reqwest::header::CONTENT_TYPE,
                "application/x-www-form-urlencoded",
            )
            .header(reqwest::header::CACHE_CONTROL, "no-store")
            .body(body)
            .send()
            .await
            .map_err(transport_failure)?;
        let (status, bytes) = read_json_body(response).await?;
        parse_grant(status, &bytes, now)
    }

    /// Test-only loopback constructors; neither exists in a production build.
    #[cfg(test)]
    pub(super) fn fixture(
        address: std::net::SocketAddr,
        timeout: Duration,
    ) -> Result<Self, BootstrapError> {
        Self::fixture_inner(address, timeout, None)
    }

    #[cfg(test)]
    pub(super) fn fixture_seeded(
        address: std::net::SocketAddr,
        token: AdminAccessToken,
        timeout: Duration,
    ) -> Result<Self, BootstrapError> {
        let far = Instant::now() + Duration::from_secs(10 * 365 * 24 * 3600);
        Self::fixture_inner(
            address,
            timeout,
            Some(Cached {
                token,
                refresh_at: far,
            }),
        )
    }

    #[cfg(test)]
    fn fixture_inner(
        address: std::net::SocketAddr,
        timeout: Duration,
        seed: Option<Cached>,
    ) -> Result<Self, BootstrapError> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(BootstrapError::Configuration);
        }
        Self::build(
            Url::parse(&format!("http://{address}/token"))
                .map_err(|_| BootstrapError::Configuration)?,
            timeout,
            seed,
        )
    }
}

impl FirebaseBootstrapConfig {
    pub(super) fn access_assertion(
        &self,
        now_epoch_seconds: u64,
    ) -> Result<String, BootstrapError> {
        let expires = now_epoch_seconds
            .checked_add(ASSERTION_SECONDS)
            .filter(|_| now_epoch_seconds > 0)
            .ok_or(BootstrapError::InvalidClock)?;
        #[derive(Serialize)]
        struct Claims<'a> {
            iss: &'a str,
            scope: &'static str,
            aud: &'static str,
            iat: u64,
            exp: u64,
        }
        let claims = serde_json::to_vec(&Claims {
            iss: &self.client_email,
            scope: ACCESS_SCOPE,
            aud: TOKEN_ENDPOINT,
            iat: now_epoch_seconds,
            exp: expires,
        })
        .map_err(|_| BootstrapError::Signing)?;
        self.sign_claims(&claims)
    }
}

/// Pure response interpretation. `now` is the monotonic instant the request was started at, which
/// is conservative: the cached lifetime can only be shorter than the provider's.
fn parse_grant(status: StatusCode, body: &[u8], now: Instant) -> Result<Cached, BootstrapError> {
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(BootstrapError::Quota);
    }
    if status.is_server_error() {
        return Err(BootstrapError::Provider);
    }
    if !status.is_success() {
        #[derive(Deserialize)]
        struct OAuthError {
            error: String,
        }
        let error: OAuthError =
            serde_json::from_slice(body).map_err(|_| BootstrapError::Protocol)?;
        return Err(match error.error.as_str() {
            "invalid_grant"
            | "invalid_client"
            | "unauthorized_client"
            | "invalid_scope"
            | "access_denied"
            | "disabled_client" => BootstrapError::AccessDenied,
            _ => BootstrapError::Rejected,
        });
    }
    #[derive(Deserialize)]
    struct Grant {
        access_token: String,
        token_type: String,
        expires_in: u64,
    }
    let grant: Grant = serde_json::from_slice(body).map_err(|_| BootstrapError::Protocol)?;
    // A token that would be stale on arrival is a protocol fault, never cached.
    if !grant.token_type.eq_ignore_ascii_case("bearer")
        || !valid_opaque_token(&grant.access_token)
        || grant.expires_in <= REFRESH_MARGIN.as_secs()
        || grant.expires_in > MAX_LIFETIME_SECONDS
    {
        return Err(BootstrapError::Protocol);
    }
    let lifetime = Duration::from_secs(grant.expires_in) - REFRESH_MARGIN;
    Ok(Cached {
        token: AdminAccessToken::parse(&grant.access_token)?,
        refresh_at: now
            .checked_add(lifetime)
            .ok_or(BootstrapError::InvalidClock)?,
    })
}

#[cfg(test)]
mod tests;
