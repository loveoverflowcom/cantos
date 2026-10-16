//! Kanidm 1.10.5 onboarding adapter. Never discover or reset an existing person.
//!
//! POST /v1/person accepts an explicit UUID and returns JSON null. The UUID plugin
//! preserves the supplied UUID and rejects duplicates, including recycled entries.
//! Only a positive response creates the witness required for the credential intent.
//! See the pinned server/core/src/https/v1.rs and server/lib/src/plugins/base.rs.
use reqwest::{header, Client, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize};
use serde_json::json;
use std::time::{Duration, SystemTime, UNIX_EPOCH};
use url::Url;

use super::{
    application::{Issuer, RegistrationFuture},
    domain::{CreatedPerson, PendingRegistration, RegistrationError, SetupLink},
};

const MAX_RESPONSE_BYTES: usize = 16_384;
const THEATRE_ISSUER_PATH: &str = "/oauth2/openid/cantos-theatre-web-bff";

/// Operator-supplied service credential; never browser input, logging data or a debug value.
pub struct KanidmConfig {
    origin: Url,
    bearer: String,
    setup_ttl_seconds: u32,
}

impl KanidmConfig {
    pub fn new(
        issuer: &str,
        bearer: String,
        setup_ttl_seconds: u32,
    ) -> Result<Self, RegistrationError> {
        let parsed = Url::parse(issuer).map_err(|_| RegistrationError::Configuration)?;
        // Registration transfers a privileged service credential and sends an issuer
        // credential-setup URL. Unlike local OIDC discovery, both require HTTPS.
        if parsed.scheme() != "https"
            || parsed.host_str().is_none()
            || !parsed.username().is_empty()
            || parsed.password().is_some()
            || parsed.query().is_some()
            || parsed.fragment().is_some()
            || parsed.path() != THEATRE_ISSUER_PATH
            || parsed.as_str() != issuer
            || bearer.is_empty()
            || bearer.len() > 16_384
            || !bearer.bytes().all(|value| value.is_ascii_graphic())
            // The pinned provider clamps to this range. Refuse instead of silently
            // extending an operator-requested shorter lifetime.
            || !(300..=86_400).contains(&setup_ttl_seconds)
        {
            return Err(RegistrationError::Configuration);
        }
        let origin = parsed
            .join("/")
            .map_err(|_| RegistrationError::Configuration)?;
        Ok(Self {
            origin,
            bearer,
            setup_ttl_seconds,
        })
    }
}

pub struct KanidmIssuer {
    config: KanidmConfig,
    client: Client,
    api_origin: Url,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Intent {
    token: String,
    expiry_time: u64,
}

impl KanidmIssuer {
    pub fn new(config: KanidmConfig) -> Result<Self, RegistrationError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| RegistrationError::Configuration)?;
        let api_origin = config.origin.clone();
        Ok(Self {
            config,
            client,
            api_origin,
        })
    }

    fn endpoint(&self, path: &str) -> Result<Url, RegistrationError> {
        let value = self
            .api_origin
            .join(path)
            .map_err(|_| RegistrationError::Configuration)?;
        if value.origin() != self.api_origin.origin() {
            return Err(RegistrationError::Configuration);
        }
        Ok(value)
    }

    async fn create(
        &self,
        registration: PendingRegistration,
    ) -> Result<CreatedPerson, RegistrationError> {
        let id = registration.id();
        let body = serde_json::to_vec(&json!({"attrs": {
            "uuid": [id.to_string()],
            "name": [registration.provider_name()],
            "displayname": ["Cantos listener"],
            "mail": [registration.email().as_str()]
        }}))
        .map_err(|_| RegistrationError::Configuration)?;
        let response = self
            .client
            .post(self.endpoint("/v1/person")?)
            .bearer_auth(&self.config.bearer)
            .header(header::ACCEPT, "application/json")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CACHE_CONTROL, "no-store")
            .body(body)
            .send()
            .await
            .map_err(|_| RegistrationError::IssuerUncertain)?;
        // Neither a conflict nor a lost/malformed success is ownership evidence.
        // No lookup, modification, deletion or automatic retry follows an error.
        bounded::<()>(response).await?;
        CreatedPerson::confirmed(id, id)
    }

    async fn request_intent(&self, person: CreatedPerson) -> Result<Intent, RegistrationError> {
        if person.registration_id() != person.subject() || person.subject().is_nil() {
            return Err(RegistrationError::InvalidState);
        }
        let response = self
            .client
            // Kanidm's published GET mutates state. The application fences this
            // effect durably; this adapter never retries it or follows redirects.
            .get(self.endpoint(&format!(
                "/v1/person/{}/_credential/_update_intent/{}",
                person.subject(),
                self.config.setup_ttl_seconds
            ))?)
            .bearer_auth(&self.config.bearer)
            .header(header::ACCEPT, "application/json")
            .header(header::CACHE_CONTROL, "no-store")
            .send()
            .await
            .map_err(|_| RegistrationError::IssuerUncertain)?;
        bounded(response).await
    }

    fn setup_link(&self, intent: Intent, now: u64) -> Result<SetupLink, RegistrationError> {
        if intent.token.is_empty()
            || intent.token.len() > 4096
            || !intent.token.bytes().all(|value| value.is_ascii_graphic())
            || intent.expiry_time <= now
            || now
                .checked_add(u64::from(self.config.setup_ttl_seconds))
                .is_none_or(|latest| intent.expiry_time > latest)
        {
            return Err(RegistrationError::IssuerUncertain);
        }
        let mut link = self
            .config
            .origin
            .join("/ui/reset")
            .map_err(|_| RegistrationError::Configuration)?;
        link.query_pairs_mut().append_pair("token", &intent.token);
        SetupLink::new(link.into())
    }
}

impl Issuer for KanidmIssuer {
    fn create_new(
        &self,
        registration: PendingRegistration,
    ) -> RegistrationFuture<'_, CreatedPerson> {
        Box::pin(self.create(registration))
    }

    fn setup_intent(&self, person: CreatedPerson) -> RegistrationFuture<'_, SetupLink> {
        Box::pin(async move {
            let intent = self.request_intent(person).await?;
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map_err(|_| RegistrationError::Unavailable)?
                .as_secs();
            self.setup_link(intent, now)
        })
    }
}

async fn bounded<T: DeserializeOwned>(mut response: Response) -> Result<T, RegistrationError> {
    if response.status() != StatusCode::OK {
        return Err(
            if response.status().is_client_error()
                && response.status() != StatusCode::REQUEST_TIMEOUT
            {
                RegistrationError::IssuerRejected
            } else {
                RegistrationError::IssuerUncertain
            },
        );
    }
    if response
        .content_length()
        .is_some_and(|value| value > MAX_RESPONSE_BYTES as u64)
        || response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .and_then(|value| value.split(';').next())
            .map(str::trim)
            != Some("application/json")
    {
        return Err(RegistrationError::IssuerUncertain);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|_| RegistrationError::IssuerUncertain)?
    {
        if bytes.len().saturating_add(chunk.len()) > MAX_RESPONSE_BYTES {
            return Err(RegistrationError::IssuerUncertain);
        }
        bytes.extend_from_slice(&chunk);
    }
    serde_json::from_slice(&bytes).map_err(|_| RegistrationError::IssuerUncertain)
}

#[cfg(test)]
mod tests;
