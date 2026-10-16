//! Privileged Firebase bootstrap and verification-mail transport, never a session issuer.
//! The application commits a durable one-winner fence before each separate remote effect.
//! No retry, account adoption, credential refresh, endpoint override or file access occurs here.

use std::time::Duration;

use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use reqwest::{header, Client, Response, StatusCode};
use ring::{rand::SystemRandom, signature};
use serde::{de::DeserializeOwned, Deserialize, Serialize};
use url::Url;

use super::{
    domain::{Email, RegistrationError},
    firebase_action::{ApiKey, DedicatedUid, FirebaseProjectId},
    firebase_enrollment::{
        application::{EnrollmentFuture, MailboxProvider},
        domain::{Enrollment, VerificationCredential},
    },
};

const PROVIDER_ENDPOINT: &str = "https://identitytoolkit.googleapis.com/v1/";
const CUSTOM_TOKEN_AUDIENCE: &str =
    "https://identitytoolkit.googleapis.com/google.identity.identitytoolkit.v1.IdentityToolkit";
const MAX_RESPONSE_BYTES: usize = 16_384;
const MAX_SERVICE_ACCOUNT_BYTES: usize = 32_768;

/// Closed, sanitized reasons. No provider body, URL, email or credential is retained.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapError {
    Configuration,
    InvalidIdentity,
    InvalidCredential,
    InvalidClock,
    WrongBinding,
    WrongClient,
    Signing,
    Conflict,
    Disabled,
    Rejected,
    Protocol,
    TooLarge,
    NotJson,
    Redirect,
    Timeout,
    Network,
    Quota,
    Provider,
}

/// Even a definitive provider rejection is terminal for the reserved create/send attempt.
/// Lost, malformed or mismatched responses after dispatch are uncertain; callers cannot
/// infer that nothing happened. An explicit provider rejection is kept separately.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BootstrapFailure {
    BeforeDispatch(BootstrapError),
    Rejected(BootstrapError),
    OutcomeUncertain(BootstrapError),
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

/// Operator-supplied existing OAuth bearer. Its expiry and renewal are operator concerns.
/// This adapter neither exchanges private keys for OAuth credentials nor auto-renews them.
pub struct AdminAccessToken(String);

impl AdminAccessToken {
    pub fn parse(value: &str) -> Result<Self, BootstrapError> {
        if value.is_empty() || value.len() > 8192 || !value.bytes().all(|b| b.is_ascii_graphic()) {
            return Err(BootstrapError::InvalidCredential);
        }
        Ok(Self(value.to_owned()))
    }
}

/// A dedicated never-reused UID and expected delivery spelling, not creation provenance.
#[derive(Clone, PartialEq, Eq)]
pub struct BootstrapIdentity {
    uid: String,
    email: Email,
}

impl BootstrapIdentity {
    pub fn new(uid: &str, email: Email) -> Result<Self, BootstrapError> {
        DedicatedUid::parse(uid).map_err(|_| BootstrapError::InvalidIdentity)?;
        Ok(Self {
            uid: uid.to_owned(),
            email,
        })
    }

    pub fn uid(&self) -> &str {
        &self.uid
    }

    pub fn email(&self) -> &Email {
        &self.email
    }
}

/// Parsed only from bytes supplied by composition; this constructor performs no file I/O.
/// Extra standard service-account fields are ignored, including token_uri: endpoints are fixed.
pub struct FirebaseBootstrapConfig {
    project: String,
    api_key: String,
    client_email: String,
    signing_key: signature::RsaKeyPair,
}

impl FirebaseBootstrapConfig {
    pub fn parse(
        project_id: &str,
        api_key: &str,
        service_account_json: &[u8],
    ) -> Result<Self, BootstrapError> {
        FirebaseProjectId::parse(project_id).map_err(|_| BootstrapError::Configuration)?;
        ApiKey::parse(api_key).map_err(|_| BootstrapError::Configuration)?;
        if service_account_json.len() > MAX_SERVICE_ACCOUNT_BYTES {
            return Err(BootstrapError::Configuration);
        }
        #[derive(Deserialize)]
        struct RawServiceAccount {
            #[serde(rename = "type")]
            account_type: String,
            project_id: String,
            client_email: String,
            private_key: String,
        }
        let raw: RawServiceAccount = serde_json::from_slice(service_account_json)
            .map_err(|_| BootstrapError::Configuration)?;
        let email = Email::parse(&raw.client_email).map_err(|_| BootstrapError::Configuration)?;
        if raw.account_type != "service_account"
            || raw.project_id != project_id
            || email.as_str() != raw.client_email
            || !raw
                .client_email
                .ends_with(&format!("@{project_id}.iam.gserviceaccount.com"))
        {
            return Err(BootstrapError::Configuration);
        }
        let der = decode_private_key(&raw.private_key)?;
        let signing_key =
            signature::RsaKeyPair::from_pkcs8(&der).map_err(|_| BootstrapError::Configuration)?;
        Ok(Self {
            project: project_id.to_owned(),
            api_key: api_key.to_owned(),
            client_email: raw.client_email,
            signing_key,
        })
    }

    pub fn project_id(&self) -> &str {
        &self.project
    }

    fn custom_token(
        &self,
        identity: &BootstrapIdentity,
        now_epoch_seconds: u64,
    ) -> Result<String, BootstrapError> {
        // A bounded lifetime does not authorize a stale caller; application owns fresh clock.
        let expires = now_epoch_seconds
            .checked_add(3600)
            .filter(|_| now_epoch_seconds > 0)
            .ok_or(BootstrapError::InvalidClock)?;
        #[derive(Serialize)]
        struct Claims<'a> {
            iss: &'a str,
            sub: &'a str,
            aud: &'static str,
            iat: u64,
            exp: u64,
            uid: &'a str,
        }
        let claims = serde_json::to_vec(&Claims {
            iss: &self.client_email,
            sub: &self.client_email,
            aud: CUSTOM_TOKEN_AUDIENCE,
            iat: now_epoch_seconds,
            exp: expires,
            uid: identity.uid(),
        })
        .map_err(|_| BootstrapError::Signing)?;
        let unsigned = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#),
            URL_SAFE_NO_PAD.encode(claims)
        );
        let mut signature = vec![0; self.signing_key.public().modulus_len()];
        self.signing_key
            .sign(
                &signature::RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                unsigned.as_bytes(),
                &mut signature,
            )
            .map_err(|_| BootstrapError::Signing)?;
        Ok(format!("{unsigned}.{}", URL_SAFE_NO_PAD.encode(signature)))
    }
}

fn decode_private_key(pem: &str) -> Result<Vec<u8>, BootstrapError> {
    let content = pem
        .trim()
        .strip_prefix("-----BEGIN PRIVATE KEY-----")
        .and_then(|s| s.strip_suffix("-----END PRIVATE KEY-----"))
        .ok_or(BootstrapError::Configuration)?;
    // Do not accept unrelated PEM labels or multiple keys; DER validity is checked by ring.
    let encoded: String = content
        .chars()
        .filter(|c| !c.is_ascii_whitespace())
        .collect();
    STANDARD
        .decode(encoded)
        .map_err(|_| BootstrapError::Configuration)
}

/// Positive exact-identity acknowledgment, never reconstructed from email/verified lookup.
pub struct CreatedFirebaseUser {
    project: String,
    identity: BootstrapIdentity,
}

impl CreatedFirebaseUser {
    pub fn identity(&self) -> &BootstrapIdentity {
        &self.identity
    }

    /// Trusted persistence boundary only: caller must load the exact immutable binding from
    /// a positively acknowledged durable create receipt, not a requested/uncertain attempt.
    pub(super) fn from_durable_ack(
        project_id: &str,
        identity: BootstrapIdentity,
    ) -> Result<Self, BootstrapError> {
        FirebaseProjectId::parse(project_id).map_err(|_| BootstrapError::Configuration)?;
        Ok(Self {
            project: project_id.to_owned(),
            identity,
        })
    }
}

pub struct SendCredential<'client> {
    client: &'client FirebaseBootstrapClient,
    identity: BootstrapIdentity,
    id_token: String,
}

impl std::fmt::Debug for SendCredential<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SendCredential([REDACTED])")
    }
}

pub struct VerificationSent {
    identity: BootstrapIdentity,
}

/// Constructed only from this provider's configured origin and a leased enrollment.
pub struct VerificationContinuation {
    identity: BootstrapIdentity,
    url: String,
}

impl VerificationSent {
    pub fn identity(&self) -> &BootstrapIdentity {
        &self.identity
    }
}

redacted_debug!(
    AdminAccessToken,
    BootstrapIdentity,
    FirebaseBootstrapConfig,
    CreatedFirebaseUser,
    VerificationSent,
    VerificationContinuation,
    FirebaseBootstrapClient,
    FirebaseMailboxProvider,
);

pub struct FirebaseBootstrapClient {
    client: Client,
    base: Url,
    config: FirebaseBootstrapConfig,
}

impl FirebaseBootstrapClient {
    pub fn new(config: FirebaseBootstrapConfig) -> Result<Self, BootstrapError> {
        Self::build(
            config,
            Url::parse(PROVIDER_ENDPOINT).map_err(|_| BootstrapError::Configuration)?,
            Duration::from_secs(5),
        )
    }

    fn build(
        config: FirebaseBootstrapConfig,
        base: Url,
        timeout: Duration,
    ) -> Result<Self, BootstrapError> {
        let client = Client::builder()
            .redirect(reqwest::redirect::Policy::none())
            .no_proxy()
            // reqwest's HTTP/2 GOAWAY retry can transparently repeat a mutation.
            .http1_only()
            .pool_max_idle_per_host(0)
            .connect_timeout(Duration::from_secs(2))
            .timeout(timeout)
            .build()
            .map_err(|_| BootstrapError::Configuration)?;
        Ok(Self {
            client,
            base,
            config,
        })
    }

    /// Administrative create: no password, ID token, provider linking or adoption request.
    /// The caller must commit CreateStarted before awaiting this non-idempotent operation.
    pub async fn create_user(
        &self,
        identity: &BootstrapIdentity,
        access: &AdminAccessToken,
    ) -> Result<CreatedFirebaseUser, BootstrapFailure> {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            local_id: &'a str,
            email: &'a str,
            email_verified: bool,
            target_project_id: &'a str,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Created {
            local_id: String,
            email: String,
        }
        let created: Created = self
            .request(
                "accounts:signUp",
                &Request {
                    local_id: identity.uid(),
                    email: identity.email().as_str(),
                    email_verified: false,
                    target_project_id: &self.config.project,
                },
                Some(access),
            )
            .await?;
        if created.local_id != identity.uid() || created.email != identity.email().as_str() {
            return Err(BootstrapFailure::OutcomeUncertain(
                BootstrapError::WrongBinding,
            ));
        }
        Ok(CreatedFirebaseUser {
            project: self.config.project.clone(),
            identity: identity.clone(),
        })
    }

    /// An acknowledged create receipt is required. Exchange may create a missing Firebase
    /// UID; it can never restore create provenance after an uncertain outcome.
    pub async fn credential_exchange(
        &self,
        created: &CreatedFirebaseUser,
        now_epoch_seconds: u64,
    ) -> Result<SendCredential<'_>, BootstrapFailure> {
        self.check_created(created)
            .map_err(BootstrapFailure::BeforeDispatch)?;
        let token = self
            .config
            .custom_token(&created.identity, now_epoch_seconds)
            .map_err(BootstrapFailure::BeforeDispatch)?;
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            token: &'a str,
            return_secure_token: bool,
        }
        #[derive(Deserialize)]
        #[serde(rename_all = "camelCase")]
        struct Exchanged {
            id_token: String,
            expires_in: String,
            is_new_user: Option<bool>,
        }
        let response: Exchanged = self
            .request(
                "accounts:signInWithCustomToken",
                &Request {
                    token: &token,
                    return_secure_token: true,
                },
                None,
            )
            .await?;
        if response.is_new_user == Some(true)
            || !valid_opaque_token(&response.id_token)
            || !matches!(response.expires_in.parse::<u32>(), Ok(1..=3600))
        {
            return Err(BootstrapFailure::OutcomeUncertain(BootstrapError::Protocol));
        }
        // Refresh tokens are neither deserialized nor retained. This credential is RAM-only.
        Ok(SendCredential {
            client: self,
            identity: created.identity.clone(),
            id_token: response.id_token,
        })
    }

    /// Built-in verification mail only. Returns no action code/link and no setup authority.
    /// The caller commits SendStarted before this effect; no send is retried automatically.
    pub async fn send_verification(
        &self,
        created: &CreatedFirebaseUser,
        credential: SendCredential<'_>,
        continuation: VerificationContinuation,
    ) -> Result<VerificationSent, BootstrapFailure> {
        self.check_created(created)
            .map_err(BootstrapFailure::BeforeDispatch)?;
        if !std::ptr::eq(self, credential.client) {
            return Err(BootstrapFailure::BeforeDispatch(
                BootstrapError::WrongClient,
            ));
        }
        if created.identity != credential.identity || created.identity != continuation.identity {
            return Err(BootstrapFailure::BeforeDispatch(
                BootstrapError::WrongBinding,
            ));
        }
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct Request<'a> {
            request_type: &'static str,
            email: &'a str,
            id_token: &'a str,
            continue_url: &'a str,
        }
        #[derive(Deserialize)]
        struct Sent {
            email: String,
        }
        let sent: Sent = self
            .request(
                "accounts:sendOobCode",
                &Request {
                    request_type: "VERIFY_EMAIL",
                    email: created.identity.email().as_str(),
                    id_token: &credential.id_token,
                    continue_url: &continuation.url,
                },
                None,
            )
            .await?;
        if sent.email != created.identity.email().as_str() {
            return Err(BootstrapFailure::OutcomeUncertain(
                BootstrapError::WrongBinding,
            ));
        }
        Ok(VerificationSent {
            identity: created.identity.clone(),
        })
    }

    fn check_created(&self, created: &CreatedFirebaseUser) -> Result<(), BootstrapError> {
        if created.project != self.config.project {
            return Err(BootstrapError::WrongBinding);
        }
        Ok(())
    }

    async fn request<T: DeserializeOwned>(
        &self,
        method: &'static str,
        body: &impl Serialize,
        access: Option<&AdminAccessToken>,
    ) -> Result<T, BootstrapFailure> {
        let body = serde_json::to_vec(body)
            .map_err(|_| BootstrapFailure::BeforeDispatch(BootstrapError::Protocol))?;
        let mut endpoint = self.base.clone();
        endpoint.set_path(&format!("/v1/{method}"));
        endpoint
            .query_pairs_mut()
            .append_pair("key", &self.config.api_key);
        let mut request = self
            .client
            .post(endpoint)
            .header(header::ACCEPT, "application/json")
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::CACHE_CONTROL, "no-store")
            .body(body);
        if let Some(access) = access {
            request = request.bearer_auth(&access.0);
        }
        let response = request
            .send()
            .await
            .map_err(|e| BootstrapFailure::OutcomeUncertain(transport_failure(e)))?;
        let client_error = response.status().is_client_error();
        decode(response).await.map_err(|error| {
            if client_error
                && matches!(
                    error,
                    BootstrapError::Conflict
                        | BootstrapError::Disabled
                        | BootstrapError::Rejected
                        | BootstrapError::Quota
                )
            {
                BootstrapFailure::Rejected(error)
            } else {
                BootstrapFailure::OutcomeUncertain(error)
            }
        })
    }

    #[cfg(test)]
    fn fixture(
        config: FirebaseBootstrapConfig,
        address: std::net::SocketAddr,
        timeout: Duration,
    ) -> Result<Self, BootstrapError> {
        if !address.ip().is_loopback() || address.port() == 0 {
            return Err(BootstrapError::Configuration);
        }
        Self::build(
            config,
            Url::parse(&format!("http://{address}/v1/"))
                .map_err(|_| BootstrapError::Configuration)?,
            timeout,
        )
    }
}

/// Concrete application port. Existing OAuth credential lifetime is supplied by the operator;
/// no credential minting/renewal, secret storage, user adoption or hidden retries occur here.
pub struct FirebaseMailboxProvider {
    client: FirebaseBootstrapClient,
    access: AdminAccessToken,
    web_origin: Url,
}

impl FirebaseMailboxProvider {
    pub fn new(
        client: FirebaseBootstrapClient,
        access: AdminAccessToken,
        web_origin: &str,
    ) -> Result<Self, RegistrationError> {
        let origin = Url::parse(web_origin).map_err(|_| RegistrationError::Configuration)?;
        let loopback = matches!(origin.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if (origin.scheme() != "https" && !(origin.scheme() == "http" && loopback))
            || origin.host_str().is_none()
            || !origin.username().is_empty()
            || origin.password().is_some()
            || origin.path() != "/"
            || origin.query().is_some()
            || origin.fragment().is_some()
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            client,
            access,
            web_origin: origin,
        })
    }

    fn identity(&self, enrollment: &Enrollment) -> Result<BootstrapIdentity, RegistrationError> {
        if enrollment.project() != self.client.config.project {
            return Err(RegistrationError::InvalidState);
        }
        BootstrapIdentity::new(enrollment.uid(), enrollment.registration().email().clone())
            .map_err(|_| RegistrationError::InvalidState)
    }

    fn continuation(
        &self,
        enrollment: &Enrollment,
        identity: BootstrapIdentity,
    ) -> VerificationContinuation {
        let mut url = self.web_origin.clone();
        url.query_pairs_mut()
            .append_pair("firebaseEnrollment", &enrollment.id().to_string());
        VerificationContinuation {
            identity,
            url: url.into(),
        }
    }
}

impl MailboxProvider for FirebaseMailboxProvider {
    fn create_unverified(&self, enrollment: Enrollment) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            let identity = self.identity(&enrollment)?;
            self.client
                .create_user(&identity, &self.access)
                .await
                .map(|_| ())
                .map_err(|failure| match failure {
                    BootstrapFailure::OutcomeUncertain(_) => RegistrationError::IssuerUncertain,
                    BootstrapFailure::BeforeDispatch(_) | BootstrapFailure::Rejected(_) => {
                        RegistrationError::IssuerRejected
                    }
                })
        })
    }

    fn prepare_verification(
        &self,
        enrollment: Enrollment,
    ) -> EnrollmentFuture<'_, VerificationCredential> {
        Box::pin(async move {
            // Application supplies this enrollment only after a durable Ready acknowledgement
            // and SendStarted lease. Public status or provider lookup cannot call this port.
            let identity = self.identity(&enrollment)?;
            let created = CreatedFirebaseUser::from_durable_ack(enrollment.project(), identity)
                .map_err(|_| RegistrationError::InvalidState)?;
            let now = u64::try_from(chrono::Utc::now().timestamp())
                .map_err(|_| RegistrationError::Configuration)?;
            let credential = self
                .client
                .credential_exchange(&created, now)
                .await
                .map_err(delivery_failure)?;
            VerificationCredential::new(enrollment, credential.id_token)
        })
    }

    fn send_verification(
        &self,
        enrollment: Enrollment,
        credential: VerificationCredential,
    ) -> EnrollmentFuture<'_, ()> {
        Box::pin(async move {
            // The application performs a fresh durable SendStarted -> MailStarted CAS
            // after the exchange await and before invoking this final remote write.
            let (binding, id_token) = credential.into_parts();
            if binding != enrollment || !valid_opaque_token(&id_token) {
                return Err(RegistrationError::InvalidState);
            }
            let identity = self.identity(&enrollment)?;
            let continuation = self.continuation(&enrollment, identity.clone());
            let created =
                CreatedFirebaseUser::from_durable_ack(enrollment.project(), identity.clone())
                    .map_err(|_| RegistrationError::InvalidState)?;
            let credential = SendCredential {
                client: &self.client,
                identity,
                id_token,
            };
            self.client
                .send_verification(&created, credential, continuation)
                .await
                .map(|_| ())
                .map_err(delivery_failure)
        })
    }
}

fn delivery_failure(failure: BootstrapFailure) -> RegistrationError {
    match failure {
        BootstrapFailure::OutcomeUncertain(_) => RegistrationError::DeliveryUncertain,
        BootstrapFailure::BeforeDispatch(_) | BootstrapFailure::Rejected(_) => {
            RegistrationError::DeliveryRejected
        }
    }
}

fn valid_opaque_token(value: &str) -> bool {
    !value.is_empty() && value.len() <= 8192 && value.bytes().all(|b| b.is_ascii_graphic())
}

fn transport_failure(error: reqwest::Error) -> BootstrapError {
    // reqwest errors retain the API-key URL; never format or keep the foreign error.
    if error.is_timeout() {
        BootstrapError::Timeout
    } else {
        BootstrapError::Network
    }
}

async fn decode<T: DeserializeOwned>(mut response: Response) -> Result<T, BootstrapError> {
    let status = response.status();
    if status.is_redirection() {
        return Err(BootstrapError::Redirect);
    }
    if response
        .content_length()
        .is_some_and(|n| n > MAX_RESPONSE_BYTES as u64)
    {
        return Err(BootstrapError::TooLarge);
    }
    if !response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.split(';').next())
        .is_some_and(|v| v.trim().eq_ignore_ascii_case("application/json"))
    {
        return Err(BootstrapError::NotJson);
    }
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(transport_failure)? {
        if chunk.len() > MAX_RESPONSE_BYTES.saturating_sub(bytes.len()) {
            return Err(BootstrapError::TooLarge);
        }
        bytes.extend_from_slice(&chunk);
    }
    if status == StatusCode::TOO_MANY_REQUESTS {
        return Err(BootstrapError::Quota);
    }
    if status.is_server_error() {
        return Err(BootstrapError::Provider);
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
        let envelope: Envelope =
            serde_json::from_slice(&bytes).map_err(|_| BootstrapError::Protocol)?;
        return Err(match envelope.error.message.as_str() {
            "EMAIL_EXISTS" | "DUPLICATE_LOCAL_ID" | "UID_ALREADY_EXISTS" => {
                BootstrapError::Conflict
            }
            "USER_DISABLED" => BootstrapError::Disabled,
            "TOO_MANY_ATTEMPTS_TRY_LATER" | "QUOTA_EXCEEDED" => BootstrapError::Quota,
            _ => BootstrapError::Rejected,
        });
    }
    serde_json::from_slice(&bytes).map_err(|_| BootstrapError::Protocol)
}

#[cfg(test)]
pub(crate) mod tests;
