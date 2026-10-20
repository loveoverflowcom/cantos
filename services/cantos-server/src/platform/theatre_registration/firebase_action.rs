//! Bounded Firebase email-action REST adapter for the explicitly selected enrollment mode.
//!
//! Action facts are not enrollment, mailbox/session authority or bootstrap provenance.
//! Before calling `consume`, the enrollment application must durably claim the operation and
//! recheck freshness/cancellation. A dropped consumption future may have changed Firebase;
//! callers must fence the operation and reconcile rather than automatically replay it.
//! Neither REST response proves the locally configured project/API-key association,
//! dedicated-UID exclusivity, per-send provenance or invalidation of older resend codes.

use std::time::Duration;

use reqwest::{header, Client, Response, StatusCode};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use url::Url;

use super::domain::PendingRegistration;

const PROVIDER_ENDPOINT: &str = "https://identitytoolkit.googleapis.com/v1/";
pub(super) const MAX_RESPONSE_BYTES: usize = 16_384;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingField {
    Project,
    Uid,
    Email,
}

/// Sanitized failures only: provider bodies, request URLs and credentials are never kept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ActionError {
    InvalidBinding(BindingField),
    InvalidApiKey,
    InvalidCode,
    WrongClient,
    WrongPurpose,
    Unverified,
    Expired,
    Configuration,
    Protocol,
    TooLarge,
    NotJson,
    Redirect,
    Timeout,
    Network,
    Quota,
    Provider,
    /// The project has not enabled the Email/Password provider that action inspection needs.
    /// An operator fault, never evidence about the caller's link.
    ProviderDisabled,
    Rejected,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConsumptionError {
    /// Refused locally, before dispatching the consuming request.
    BeforeConsume(ActionError),
    /// Includes malformed success, identity mismatch and all remote/transport failures.
    /// It never means that the provider did not consume the code.
    ConsumeUncertain(ActionError),
}

/// Syntactically checked project ID, never proof of an API key's project association.
#[derive(Clone, PartialEq, Eq)]
pub struct FirebaseProjectId(String);

impl FirebaseProjectId {
    pub fn parse(value: &str) -> Result<Self, ActionError> {
        if !(6..=30).contains(&value.len())
            || !value.starts_with(|c: char| c.is_ascii_lowercase())
            || value.ends_with('-')
            || !value
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
        {
            return Err(ActionError::InvalidBinding(BindingField::Project));
        }
        Ok(Self(value.to_owned()))
    }
}

pub struct ApiKey(String);

impl ApiKey {
    pub fn parse(value: &str) -> Result<Self, ActionError> {
        if value.is_empty() || value.len() > 512 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ActionError::InvalidApiKey);
        }
        Ok(Self(value.to_owned()))
    }
}

pub struct ActionCode(String);

impl ActionCode {
    pub fn parse(value: &str) -> Result<Self, ActionError> {
        if value.is_empty() || value.len() > 4096 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ActionError::InvalidCode);
        }
        Ok(Self(value.to_owned()))
    }
}

/// Expected dedicated UID; syntax validation establishes no creation or ownership fact.
#[derive(Clone, PartialEq, Eq)]
pub struct DedicatedUid(String);

impl DedicatedUid {
    pub fn parse(value: &str) -> Result<Self, ActionError> {
        if value.is_empty() || value.len() > 128 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(ActionError::InvalidBinding(BindingField::Uid));
        }
        Ok(Self(value.to_owned()))
    }
}

macro_rules! redacted_debug {
    ($($name:ident),+ $(,)?) => {$ (
        impl std::fmt::Debug for $name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str(concat!(stringify!($name), "([REDACTED])"))
            }
        }
    )+};
}

redacted_debug!(FirebaseProjectId, ApiKey, ActionCode, DedicatedUid);

/// Caller-supplied local expectations. This value is not verified enrollment evidence.
#[derive(Clone, PartialEq, Eq)]
pub struct ExpectedActionBinding {
    registration: PendingRegistration,
    project: FirebaseProjectId,
    uid: DedicatedUid,
}

impl ExpectedActionBinding {
    pub fn new(
        registration: PendingRegistration,
        project: FirebaseProjectId,
        uid: DedicatedUid,
    ) -> Self {
        Self {
            registration,
            project,
            uid,
        }
    }

    pub fn registration(&self) -> &PendingRegistration {
        &self.registration
    }

    fn matches_email(&self, email: &str) -> Result<(), ActionError> {
        // Email's input normalization and deduplication key confer no action authority.
        // Require exact provider spelling; never fold local parts, dots or plus aliases.
        if email != self.registration.email().as_str() {
            return Err(ActionError::InvalidBinding(BindingField::Email));
        }
        Ok(())
    }

    fn matches_new_email(&self, email: Option<&str>) -> Result<(), ActionError> {
        if let Some(email) = email {
            self.matches_email(email)?;
        }
        Ok(())
    }
}

redacted_debug!(ExpectedActionBinding);

/// Sealed facts from non-consuming inspection, retaining the exact code and client.
/// It cannot be cloned or deserialized, and provides no setup or authenticated principal.
///
/// ```compile_fail
/// use cantos_server::platform::theatre_registration::firebase_action::InspectedAction;
/// fn duplicate(action: InspectedAction<'_>) {
///     let _replay = action.clone();
/// }
/// ```
pub struct InspectedAction<'client> {
    client: &'client FirebaseActionClient,
    binding: ExpectedActionBinding,
    code: ActionCode,
}

impl std::fmt::Debug for InspectedAction<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("InspectedAction([REDACTED])")
    }
}

/// Factual successful consumption of this inspected code for these local expectations.
/// No conversion to CreatedPerson, application challenge, setup capability or session exists.
///
/// ```compile_fail
/// use cantos_server::platform::theatre_registration::firebase_action::{
///     ConsumedAction, ExpectedActionBinding,
/// };
/// fn forge(binding: ExpectedActionBinding) -> ConsumedAction {
///     ConsumedAction { binding }
/// }
/// ```
#[derive(PartialEq, Eq)]
pub struct ConsumedAction {
    binding: ExpectedActionBinding,
}

impl ConsumedAction {
    pub fn binding(&self) -> &ExpectedActionBinding {
        &self.binding
    }
}

redacted_debug!(ConsumedAction);

/// Fixed-origin, bounded REST client. Constructing it performs no network operation.
pub struct FirebaseActionClient {
    client: Client,
    base: Url,
    key: ApiKey,
    project: FirebaseProjectId,
}

impl std::fmt::Debug for FirebaseActionClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FirebaseActionClient([REDACTED])")
    }
}

impl FirebaseActionClient {
    /// No endpoint/environment/emulator override is accepted. Default rustls certificate
    /// and hostname validation remains enabled; redirects and inherited proxies are disabled.
    pub fn new(project: FirebaseProjectId, key: ApiKey) -> Result<Self, ActionError> {
        Self::build(
            Url::parse(PROVIDER_ENDPOINT).map_err(|_| ActionError::Configuration)?,
            project,
            key,
            Duration::from_secs(5),
        )
    }

    fn build(
        base: Url,
        project: FirebaseProjectId,
        key: ApiKey,
        timeout: Duration,
    ) -> Result<Self, ActionError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            // Pinned reqwest may transparently replay HTTP/2 stream failures. A
            // consuming action uses a fresh HTTP/1 connection and no retry loop.
            .http1_only()
            .pool_max_idle_per_host(0)
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            .build()
            .map_err(|_| ActionError::Configuration)?;
        Ok(Self {
            client,
            base,
            key,
            project,
        })
    }

    /// Non-consuming request: the serialized request includes only `oobCode`, never a
    /// password, ID token or caller-supplied mutation. It proves no operation/project echo.
    pub async fn inspect(
        &self,
        binding: ExpectedActionBinding,
        code: ActionCode,
    ) -> Result<InspectedAction<'_>, ActionError> {
        if binding.project != self.project {
            return Err(ActionError::InvalidBinding(BindingField::Project));
        }
        let response = self.request::<Inspection>(false, &code).await?;
        response.check(&binding)?;
        Ok(InspectedAction {
            client: self,
            binding,
            code,
        })
    }

    /// Ownership prevents reusing this inspected value, but is not a durable replay fence.
    /// The application must claim/recheck before calling; remote failure or cancellation
    /// after dispatch requires reconciliation. This client never retries either request.
    pub async fn consume(
        &self,
        action: InspectedAction<'_>,
    ) -> Result<ConsumedAction, ConsumptionError> {
        if !std::ptr::eq(self, action.client) {
            return Err(ConsumptionError::BeforeConsume(ActionError::WrongClient));
        }
        let response = self
            .request::<Consumption>(true, &action.code)
            .await
            .map_err(ConsumptionError::ConsumeUncertain)?;
        response
            .check(&action.binding)
            .map_err(ConsumptionError::ConsumeUncertain)?;
        Ok(ConsumedAction {
            binding: action.binding,
        })
    }

    fn endpoint(&self, consuming: bool) -> Url {
        let mut url = self.base.clone();
        let method = if consuming {
            "accounts:update"
        } else {
            "accounts:resetPassword"
        };
        url.set_path(&format!("{}{method}", self.base.path()));
        url.query_pairs_mut().append_pair("key", &self.key.0);
        url
    }

    async fn request<T: DeserializeOwned>(
        &self,
        consuming: bool,
        code: &ActionCode,
    ) -> Result<T, ActionError> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            oob_code: &'a str,
        }
        let body = serde_json::to_vec(&Request { oob_code: &code.0 })
            .map_err(|_| ActionError::Protocol)?;
        let response = self
            .client
            .post(self.endpoint(consuming))
            .header(header::ACCEPT, "application/json")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CACHE_CONTROL, "no-store")
            .body(body)
            .send()
            .await
            .map_err(transport_failure)?;
        decode(response).await
    }

    /// Test-only seam takes a parsed literal loopback socket, never a URL or environment.
    #[cfg(test)]
    pub(super) fn fixture(
        address: std::net::SocketAddr,
        timeout: Duration,
    ) -> Result<Self, ActionError> {
        Self::fixture_at(address, timeout, false)
    }

    #[cfg(test)]
    pub(super) fn auth_emulator_fixture(
        address: std::net::SocketAddr,
        timeout: Duration,
    ) -> Result<Self, ActionError> {
        Self::fixture_at(address, timeout, true)
    }

    #[cfg(test)]
    pub(super) fn owned_auth_emulator_fixture(
        address: std::net::SocketAddr,
        project: &str,
        timeout: Duration,
    ) -> Result<Self, ActionError> {
        if !address.ip().is_loopback() || address.port() == 0 || !project.starts_with("demo-") {
            return Err(ActionError::Configuration);
        }
        Self::build(
            Url::parse(&format!(
                "http://{address}/identitytoolkit.googleapis.com/v1/"
            ))
            .map_err(|_| ActionError::Configuration)?,
            FirebaseProjectId::parse(project)?,
            ApiKey::parse("synthetic-fixture-key")?,
            timeout,
        )
    }

    #[cfg(test)]
    fn fixture_at(
        address: std::net::SocketAddr,
        timeout: Duration,
        emulator: bool,
    ) -> Result<Self, ActionError> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(ActionError::Configuration);
        }
        let (prefix, project) = if emulator {
            (
                "/identitytoolkit.googleapis.com/v1/",
                "demo-cantos-issue206",
            )
        } else {
            ("/v1/", "candidate-project")
        };
        Self::build(
            Url::parse(&format!("http://{address}{prefix}"))
                .map_err(|_| ActionError::Configuration)?,
            FirebaseProjectId::parse(project)?,
            ApiKey::parse("synthetic-fixture-key")?,
            timeout,
        )
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Inspection {
    email: String,
    request_type: String,
    new_email: Option<String>,
}

impl Inspection {
    fn check(&self, binding: &ExpectedActionBinding) -> Result<(), ActionError> {
        if self.request_type != "VERIFY_EMAIL" {
            return Err(ActionError::WrongPurpose);
        }
        binding.matches_email(&self.email)?;
        binding.matches_new_email(self.new_email.as_deref())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Consumption {
    local_id: String,
    email: String,
    email_verified: Option<bool>,
    new_email: Option<String>,
}

impl Consumption {
    fn check(&self, binding: &ExpectedActionBinding) -> Result<(), ActionError> {
        binding.matches_email(&self.email)?;
        binding.matches_new_email(self.new_email.as_deref())?;
        if self.local_id != binding.uid.0 {
            return Err(ActionError::InvalidBinding(BindingField::Uid));
        }
        // Official confirmation examples omit this field. True alone never constructs
        // an inspected action; an explicit false is refused after consumption.
        if self.email_verified == Some(false) {
            return Err(ActionError::Unverified);
        }
        Ok(())
    }
}

pub(super) fn transport_failure(error: reqwest::Error) -> ActionError {
    // Never format reqwest::Error: its URL includes the API key.
    if error.is_timeout() {
        ActionError::Timeout
    } else {
        ActionError::Network
    }
}

pub(super) async fn decode<T: DeserializeOwned>(mut response: Response) -> Result<T, ActionError> {
    let status = response.status();
    if status.is_redirection() {
        return Err(ActionError::Redirect);
    }
    if response
        .content_length()
        .is_some_and(|size| size > MAX_RESPONSE_BYTES as u64)
    {
        return Err(ActionError::TooLarge);
    }
    let json_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"));
    if !json_type {
        return Err(ActionError::NotJson);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_failure)? {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err(ActionError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(ActionError::Quota);
    }
    if status.is_server_error() {
        return Err(ActionError::Provider);
    }
    if !status.is_success() {
        #[derive(Deserialize)]
        struct Envelope {
            error: ProviderError,
        }
        #[derive(Deserialize)]
        struct ProviderError {
            message: String,
        }
        let error: Envelope = serde_json::from_slice(&bytes).map_err(|_| ActionError::Protocol)?;
        return Err(match error.error.message.as_str() {
            "INVALID_OOB_CODE" => ActionError::InvalidCode,
            "EXPIRED_OOB_CODE" => ActionError::Expired,
            "PASSWORD_LOGIN_DISABLED" => ActionError::ProviderDisabled,
            "TOO_MANY_ATTEMPTS_TRY_LATER" | "QUOTA_EXCEEDED" => ActionError::Quota,
            _ => ActionError::Rejected,
        });
    }
    serde_json::from_slice(&bytes).map_err(|_| ActionError::Protocol)
}

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) mod emulator_tests;
