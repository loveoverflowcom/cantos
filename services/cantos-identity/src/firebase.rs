//! Verification actions only. Firebase tokens never enter native authentication/session paths.
//! Native proof material is encrypted at rest and never sent to Firebase or a browser.

use crate::{
    config::{ConfigError, MailProvider},
    domain::{email::EmailAddress, ids::OperationId, mailbox_proof::ProofId},
    persistence::{ProofCommandId, ProofSecret},
};
use base64::{
    engine::general_purpose::{STANDARD, URL_SAFE_NO_PAD},
    Engine,
};
use reqwest::Client;
use ring::{aead, hkdf, hmac, rand::SystemRandom, signature};
use serde_json::{json, Value};
use std::{
    fmt,
    io::Read,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};
use url::Url;
use zeroize::Zeroizing;

const MAX_BODY: usize = 32768;
const GOOGLE: &str = "https://identitytoolkit.googleapis.com/v1/";
const TOKEN: &str = "https://oauth2.googleapis.com/token";
const CUSTOM_AUD: &str =
    "https://identitytoolkit.googleapis.com/google.identity.identitytoolkit.v1.IdentityToolkit";

use crate::verification::{
    valid_action_code as valid_code, VerificationActions, VerificationFault as Fault,
    VerificationFuture,
};

pub struct Config {
    project: String,
    action_handler: String,
    base: Url,
    api_key: String,
    envelope_key: Zeroizing<[u8; 32]>,
    credential: Option<Credential>,
}
impl fmt::Debug for Config {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FirebaseConfig([REDACTED])")
    }
}
struct Credential {
    email: String,
    key: signature::RsaKeyPair,
}

fn read_private(path: &Path, limit: usize) -> Result<Zeroizing<Vec<u8>>, Fault> {
    let metadata = std::fs::symlink_metadata(path).map_err(|_| Fault::Configuration)?;
    if !metadata.is_file() || metadata.len() as usize > limit {
        return Err(Fault::Configuration);
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        if metadata.permissions().mode() & 0o037 != 0 {
            return Err(Fault::Configuration);
        }
    }
    let mut bytes = Zeroizing::new(Vec::new());
    std::fs::File::open(path)
        .map_err(|_| Fault::Configuration)?
        .take(limit as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| Fault::Configuration)?;
    if bytes.len() > limit {
        return Err(Fault::Configuration);
    }
    Ok(bytes)
}
fn project_valid(project: &str) -> bool {
    (6..=30).contains(&project.len())
        && project.starts_with(|c: char| c.is_ascii_lowercase())
        && !project.ends_with('-')
        && project
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
}
fn safe_action(handler: &str, origin: &str, local: bool) -> bool {
    Url::parse(handler).ok().is_some_and(|url| {
        let loopback = url.host_str().is_some_and(|host| {
            host.parse::<std::net::IpAddr>()
                .is_ok_and(|ip| ip.is_loopback())
                || host == "localhost"
        });
        url.origin().ascii_serialization() == origin
            && url.query().is_none()
            && url.fragment().is_none()
            && url.username().is_empty()
            && url.password().is_none()
            && if local {
                loopback && matches!(url.scheme(), "http" | "https") && url.port().is_some()
            } else {
                url.scheme() == "https"
                    && !loopback
                    && url.host_str().is_some_and(|host| {
                        host.contains('.') && host.parse::<std::net::IpAddr>().is_err()
                    })
            }
    })
}
impl Config {
    /// All syntax and project/handler approvals are checked before opening either secret file.
    pub fn load(
        provider: MailProvider,
        local: bool,
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        let required = |lookup: &mut dyn FnMut(&str) -> Option<String>, name: &'static str| {
            lookup(name)
                .filter(|value| !value.is_empty())
                .ok_or(ConfigError::Missing(name))
        };
        let project = required(&mut lookup, "CANTOS_IDENTITY_FIREBASE_PROJECT")?;
        let action_handler = required(&mut lookup, "CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER")?;
        let origin = required(&mut lookup, "CANTOS_IDENTITY_FIREBASE_ACTION_ORIGIN")?;
        let key_path = PathBuf::from(required(
            &mut lookup,
            "CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE",
        )?);
        let emulator = provider == MailProvider::FirebaseEmulator;
        if !project_valid(&project)
            || !safe_action(&action_handler, &origin, emulator)
            || !key_path.is_absolute()
        {
            return Err(ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_*"));
        }
        let (base, api_key, credential_path) = if emulator {
            if !local || !cfg!(feature = "local-development") || !project.starts_with("demo-") {
                return Err(ConfigError::Invalid("CANTOS_IDENTITY_MAIL_PROVIDER"));
            }
            for name in [
                "CANTOS_IDENTITY_FIREBASE_API_KEY",
                "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
                "GOOGLE_APPLICATION_CREDENTIALS",
                "FIREBASE_TOKEN",
            ] {
                if lookup(name).is_some() {
                    return Err(ConfigError::Invalid(name));
                }
            }
            let address = required(&mut lookup, "CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT")?
                .parse::<std::net::SocketAddr>()
                .map_err(|_| ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT"))?;
            if !address.ip().is_loopback() || address.port() == 0 {
                return Err(ConfigError::Invalid(
                    "CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT",
                ));
            }
            (
                Url::parse(&format!(
                    "http://{address}/identitytoolkit.googleapis.com/v1/"
                ))
                .map_err(|_| ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT"))?,
                "synthetic-fixture-key".into(),
                None,
            )
        } else {
            for name in [
                "CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT",
                "CANTOS_IDENTITY_FIREBASE_ENDPOINT",
                "FIREBASE_AUTH_EMULATOR_HOST",
                "FIREBASE_AUTH_EMULATOR_URL",
            ] {
                if lookup(name).is_some() {
                    return Err(ConfigError::Invalid(name));
                }
            }
            if local
                || project.starts_with("demo-")
                || required(
                    &mut lookup,
                    "CANTOS_IDENTITY_FIREBASE_PROJECT_BINDING_APPROVED",
                )? != "true"
                || required(
                    &mut lookup,
                    "CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER_APPROVED",
                )? != "true"
            {
                return Err(ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_*"));
            }
            let api_key = required(&mut lookup, "CANTOS_IDENTITY_FIREBASE_API_KEY")?;
            if api_key.len() > 512 || !api_key.bytes().all(|b| b.is_ascii_graphic()) {
                return Err(ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_API_KEY"));
            }
            let path = PathBuf::from(required(
                &mut lookup,
                "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
            )?);
            if !path.is_absolute() {
                return Err(ConfigError::Invalid(
                    "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
                ));
            }
            (
                Url::parse(GOOGLE)
                    .map_err(|_| ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_*"))?,
                api_key,
                Some(path),
            )
        };
        let key = read_private(&key_path, 32)
            .map_err(|_| ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE"))?;
        let envelope_key = Zeroizing::new(
            key.as_slice()
                .try_into()
                .map_err(|_| ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE"))?,
        );
        let credential = credential_path
            .map(|path| Credential::parse(&read_private(&path, MAX_BODY)?, &project))
            .transpose()
            .map_err(|_| ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE"))?;
        Ok(Self {
            project,
            action_handler,
            base,
            api_key,
            envelope_key,
            credential,
        })
    }
}
impl Credential {
    fn parse(bytes: &[u8], project: &str) -> Result<Self, Fault> {
        let raw: Value = serde_json::from_slice(bytes).map_err(|_| Fault::Configuration)?;
        let email = raw["client_email"].as_str().ok_or(Fault::Configuration)?;
        if raw["type"] != "service_account"
            || raw["project_id"] != project
            || !email.ends_with(&format!("@{project}.iam.gserviceaccount.com"))
            || EmailAddress::parse(email).is_err()
        {
            return Err(Fault::Configuration);
        }
        let pem = raw["private_key"]
            .as_str()
            .and_then(|pem| pem.trim().strip_prefix("-----BEGIN PRIVATE KEY-----"))
            .and_then(|pem| pem.strip_suffix("-----END PRIVATE KEY-----"))
            .ok_or(Fault::Configuration)?;
        let encoded: String = pem.chars().filter(|c| !c.is_ascii_whitespace()).collect();
        let der = Zeroizing::new(STANDARD.decode(encoded).map_err(|_| Fault::Configuration)?);
        let key = signature::RsaKeyPair::from_pkcs8(&der).map_err(|_| Fault::Configuration)?;
        Ok(Self {
            email: email.to_owned(),
            key,
        })
    }
    fn sign(&self, claims: Value) -> Result<String, Fault> {
        let unsigned = format!(
            "{}.{}",
            URL_SAFE_NO_PAD.encode(br#"{"alg":"RS256","typ":"JWT"}"#),
            URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).map_err(|_| Fault::Configuration)?)
        );
        let mut output = vec![0; self.key.public().modulus_len()];
        self.key
            .sign(
                &signature::RSA_PKCS1_SHA256,
                &SystemRandom::new(),
                unsigned.as_bytes(),
                &mut output,
            )
            .map_err(|_| Fault::Configuration)?;
        Ok(format!("{unsigned}.{}", URL_SAFE_NO_PAD.encode(output)))
    }
}

pub struct Provider {
    config: Config,
    http: Client,
}
impl fmt::Debug for Provider {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("FirebaseProvider([REDACTED])")
    }
}
impl Provider {
    pub fn new(config: Config) -> Result<Self, Fault> {
        let http = Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .timeout(Duration::from_secs(5))
            .build()
            .map_err(|_| Fault::Configuration)?;
        Ok(Self { config, http })
    }
    pub fn project(&self) -> &str {
        &self.config.project
    }
    fn confirmation_tag(
        &self,
        operation: OperationId,
        proof: ProofId,
        command: ProofCommandId,
        code: &str,
        password: &str,
    ) -> Result<[u8; 32], Fault> {
        // Separate HMAC key from the AES envelope key. DB + known email code must not
        // provide a fast password oracle; only the separately injected runtime key can
        // evaluate this request binding. Password authentication still uses Argon2id.
        let salt = hkdf::Salt::new(hkdf::HKDF_SHA256, b"cantos-firebase-custody-v1");
        let prk = salt.extract(self.config.envelope_key.as_ref());
        let info = [b"theatre-signup-confirmation-request-v1".as_slice()];
        let okm = prk
            .expand(&info, hkdf::HKDF_SHA256)
            .map_err(|_| Fault::Configuration)?;
        let mut derived = Zeroizing::new([0; 32]);
        okm.fill(derived.as_mut())
            .map_err(|_| Fault::Configuration)?;
        let key = hmac::Key::new(hmac::HMAC_SHA256, derived.as_ref());
        let mut context = hmac::Context::with_key(&key);
        for field in [
            b"cantos-firebase-confirm-v2/TheatreSignup".as_slice(),
            self.project().as_bytes(),
            &operation.to_bytes(),
            &proof.to_bytes(),
            &command.to_bytes(),
            code.as_bytes(),
            password.as_bytes(),
        ] {
            context.update(&(field.len() as u64).to_be_bytes());
            context.update(field);
        }
        context
            .sign()
            .as_ref()
            .try_into()
            .map_err(|_| Fault::Configuration)
    }
    pub fn uid(operation: OperationId) -> String {
        format!("cantos-{}", hex(&operation.to_bytes()))
    }
    fn aad(
        &self,
        operation: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
    ) -> Vec<u8> {
        format!(
            "cantos-firebase-envelope-v1\n{}\n{}\n{}\n{}\n{}\n{}\nTheatreSignup",
            self.project(),
            hex(&operation.to_bytes()),
            hex(&proof.to_bytes()),
            email,
            hex(epoch),
            expires
        )
        .into_bytes()
    }
    pub fn seal(
        &self,
        operation: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
        secret: &ProofSecret,
    ) -> Result<Vec<u8>, Fault> {
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, self.config.envelope_key.as_ref())
                .map_err(|_| Fault::Configuration)?,
        );
        let mut nonce = [0; 12];
        getrandom::fill(&mut nonce).map_err(|_| Fault::Configuration)?;
        let mut ciphertext = secret.expose().to_vec();
        key.seal_in_place_append_tag(
            aead::Nonce::assume_unique_for_key(nonce),
            aead::Aad::from(self.aad(operation, proof, email, epoch, expires)),
            &mut ciphertext,
        )
        .map_err(|_| Fault::Configuration)?;
        Ok([nonce.to_vec(), ciphertext].concat())
    }
    pub fn open(
        &self,
        operation: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
        envelope: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, Fault> {
        if envelope.len() != 60 {
            return Err(Fault::Configuration);
        }
        let nonce: [u8; 12] = envelope[..12]
            .try_into()
            .map_err(|_| Fault::Configuration)?;
        let key = aead::LessSafeKey::new(
            aead::UnboundKey::new(&aead::AES_256_GCM, self.config.envelope_key.as_ref())
                .map_err(|_| Fault::Configuration)?,
        );
        let mut bytes = Zeroizing::new(envelope[12..].to_vec());
        let plain = key
            .open_in_place(
                aead::Nonce::assume_unique_for_key(nonce),
                aead::Aad::from(self.aad(operation, proof, email, epoch, expires)),
                &mut bytes,
            )
            .map_err(|_| Fault::Rejected)?;
        Ok(Zeroizing::new(
            plain.try_into().map_err(|_| Fault::Configuration)?,
        ))
    }
    async fn decode(response: reqwest::Response) -> Result<Value, Fault> {
        let status = response.status();
        if status.is_redirection() {
            return Err(Fault::Uncertain);
        }
        if status.is_client_error() {
            return Err(Fault::Rejected);
        }
        if !status.is_success() {
            return Err(Fault::Uncertain);
        }
        if !response
            .headers()
            .get(reqwest::header::CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| {
                value
                    .split(';')
                    .next()
                    .is_some_and(|mime| mime.trim().eq_ignore_ascii_case("application/json"))
            })
        {
            return Err(Fault::Uncertain);
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_BODY as u64)
        {
            return Err(Fault::Uncertain);
        }
        let mut response = response;
        let mut bytes = Zeroizing::new(Vec::new());
        while let Some(chunk) = response.chunk().await.map_err(|_| Fault::Uncertain)? {
            if chunk.len() > MAX_BODY.saturating_sub(bytes.len()) {
                return Err(Fault::Uncertain);
            }
            bytes.extend_from_slice(&chunk);
        }
        serde_json::from_slice(&bytes).map_err(|_| Fault::Uncertain)
    }
    fn endpoint(&self, method: &str) -> Url {
        // REST method names contain ':', so URL::join would interpret them as a new scheme.
        // Only this configured base and an internal method path determine the destination.
        let mut url = self.config.base.clone();
        url.set_path(&format!("{}{method}", self.config.base.path()));
        url.query_pairs_mut()
            .append_pair("key", &self.config.api_key);
        url
    }
    async fn request(&self, method: &str, body: Value, admin: bool) -> Result<Value, Fault> {
        let url = self.endpoint(method);
        let mut request = self
            .http
            .post(url)
            .header("Content-Type", "application/json")
            .header("Cache-Control", "no-store")
            .body(serde_json::to_vec(&body).map_err(|_| Fault::Configuration)?);
        if admin {
            request = request.bearer_auth(self.admin_token().await.map_err(|_| Fault::Rejected)?);
        }
        Self::decode(request.send().await.map_err(|error| {
            if error.is_builder() {
                Fault::Rejected
            } else {
                Fault::Uncertain
            }
        })?)
        .await
    }
    async fn admin_token(&self) -> Result<String, Fault> {
        let Some(credential) = &self.config.credential else {
            return Ok("owner".into());
        };
        let now = now()?;
        let assertion=credential.sign(json!({"iss":credential.email,"sub":credential.email,"aud":TOKEN,"scope":"https://www.googleapis.com/auth/identitytoolkit","iat":now,"exp":now+300}))?;
        let body = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("grant_type", "urn:ietf:params:oauth:grant-type:jwt-bearer")
            .append_pair("assertion", &assertion)
            .finish();
        let response = self
            .http
            .post(TOKEN)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|_| Fault::Rejected)?;
        let token = Self::decode(response).await?;
        token["access_token"]
            .as_str()
            .filter(|token| valid_code(token))
            .map(str::to_owned)
            .ok_or(Fault::Rejected)
    }
    pub async fn create(&self, operation: OperationId, email: &str) -> Result<(), Fault> {
        let uid = Self::uid(operation);
        let method = if self.config.credential.is_some() {
            "accounts:signUp".to_owned()
        } else {
            format!("projects/{}/accounts", self.project())
        };
        let created=self.request(&method,json!({"localId":uid,"email":email,"emailVerified":false,"targetProjectId":self.project()}),true).await?;
        if created["localId"] != uid || created["email"] != email {
            return Err(Fault::Uncertain);
        }
        Ok(())
    }
    /// Requires durable positive-create provenance supplied by the application, never a lookup.
    pub async fn send(
        &self,
        operation: OperationId,
        proof: ProofId,
        email: &str,
    ) -> Result<(), Fault> {
        let uid = Self::uid(operation);
        let now = now()?;
        let token = if let Some(credential) = &self.config.credential {
            credential.sign(json!({"iss":credential.email,"sub":credential.email,"aud":CUSTOM_AUD,"iat":now,"exp":now+300,"uid":uid}))?
        } else {
            json!({"uid":uid}).to_string()
        };
        let exchanged = self
            .request(
                "accounts:signInWithCustomToken",
                json!({"token":token,"returnSecureToken":true}),
                false,
            )
            .await?;
        if exchanged["isNewUser"] == true {
            return Err(Fault::Uncertain);
        }
        let id = exchanged["idToken"]
            .as_str()
            .filter(|value| valid_code(value))
            .ok_or(Fault::Uncertain)?;
        let continuation = format!(
            "{}?operation={}&proof={}",
            self.config.action_handler,
            hex(&operation.to_bytes()),
            hex(&proof.to_bytes())
        );
        let result = self
            .request(
                "accounts:sendOobCode",
                json!({"requestType":"VERIFY_EMAIL","idToken":id,"continueUrl":continuation}),
                false,
            )
            .await?;
        if result["email"] != email {
            return Err(Fault::Uncertain);
        }
        Ok(())
    }
    pub async fn inspect(&self, code: &str, email: &str) -> Result<(), Fault> {
        if !valid_code(code) {
            return Err(Fault::Rejected);
        }
        let value = self
            .request("accounts:resetPassword", json!({"oobCode":code}), false)
            .await?;
        if value["requestType"] != "VERIFY_EMAIL"
            || value["email"] != email
            || value.get("newEmail").is_some()
        {
            return Err(Fault::Rejected);
        }
        Ok(())
    }
    pub async fn consume(
        &self,
        operation: OperationId,
        code: &str,
        email: &str,
    ) -> Result<(), Fault> {
        let value = self
            .request("accounts:update", json!({"oobCode":code}), false)
            .await?;
        if value["localId"] != Self::uid(operation)
            || value["email"] != email
            || value["emailVerified"] != true
        {
            return Err(Fault::Uncertain);
        }
        Ok(())
    }
}
pub fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
fn now() -> Result<u64, Fault> {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|now| now.as_secs())
        .map_err(|_| Fault::Configuration)
}

impl VerificationActions for Provider {
    fn project(&self) -> &str {
        Provider::project(self)
    }
    fn confirmation_tag(
        &self,
        op: OperationId,
        proof: ProofId,
        command: ProofCommandId,
        code: &str,
        password: &str,
    ) -> Result<[u8; 32], Fault> {
        Provider::confirmation_tag(self, op, proof, command, code, password)
    }
    fn seal(
        &self,
        op: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
        secret: &ProofSecret,
    ) -> Result<Vec<u8>, Fault> {
        Provider::seal(self, op, proof, email, epoch, expires, secret)
    }
    fn open(
        &self,
        op: OperationId,
        proof: ProofId,
        email: &str,
        epoch: &[u8; 16],
        expires: u64,
        envelope: &[u8],
    ) -> Result<Zeroizing<[u8; 32]>, Fault> {
        Provider::open(self, op, proof, email, epoch, expires, envelope)
    }
    fn create<'a>(&'a self, op: OperationId, email: &'a str) -> VerificationFuture<'a> {
        Box::pin(Provider::create(self, op, email))
    }
    fn send<'a>(
        &'a self,
        op: OperationId,
        proof: ProofId,
        email: &'a str,
    ) -> VerificationFuture<'a> {
        Box::pin(Provider::send(self, op, proof, email))
    }
    fn inspect<'a>(&'a self, code: &'a str, email: &'a str) -> VerificationFuture<'a> {
        Box::pin(Provider::inspect(self, code, email))
    }
    fn consume<'a>(
        &'a self,
        op: OperationId,
        code: &'a str,
        email: &'a str,
    ) -> VerificationFuture<'a> {
        Box::pin(Provider::consume(self, op, code, email))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture_provider() -> Provider {
        Provider::new(Config {
            project: "demo-cantos-unit".into(),
            action_handler: "http://127.0.0.1:8082/".into(),
            base: Url::parse("http://127.0.0.1:9099/identitytoolkit.googleapis.com/v1/").unwrap(),
            api_key: "synthetic-fixture-key".into(),
            envelope_key: Zeroizing::new([3; 32]),
            credential: None,
        })
        .unwrap()
    }
    #[test]
    fn encrypted_native_proof_is_bound_to_every_native_and_provider_locator() {
        let provider = fixture_provider();
        let operation = OperationId::from_bytes([1; 16]).unwrap();
        let proof = ProofId::from_bytes([2; 16]).unwrap();
        let epoch = [4; 16];
        let expires = 1000;
        let email = "owner@example.test";
        let secret = ProofSecret::from_bytes([7; 32]);
        let encrypted = provider
            .seal(operation, proof, email, &epoch, expires, &secret)
            .unwrap();
        assert_eq!(encrypted.len(), 60);
        assert!(!encrypted.windows(32).any(|bytes| bytes == secret.expose()));
        assert_eq!(
            *provider
                .open(operation, proof, email, &epoch, expires, &encrypted)
                .unwrap(),
            [7; 32]
        );
        for (op, p, mail, e, t) in [
            (
                OperationId::from_bytes([9; 16]).unwrap(),
                proof,
                email,
                epoch,
                expires,
            ),
            (
                operation,
                ProofId::from_bytes([9; 16]).unwrap(),
                email,
                epoch,
                expires,
            ),
            (operation, proof, "other@example.test", epoch, expires),
            (operation, proof, email, [9; 16], expires),
            (operation, proof, email, epoch, expires + 1),
        ] {
            assert_eq!(
                provider.open(op, p, mail, &e, t, &encrypted).unwrap_err(),
                Fault::Rejected
            );
        }
        let mut changed = encrypted.clone();
        changed[20] ^= 1;
        assert_eq!(
            provider
                .open(operation, proof, email, &epoch, expires, &changed)
                .unwrap_err(),
            Fault::Rejected
        );
        let mut other = fixture_provider();
        other.config.project = "demo-other-project".into();
        assert_eq!(
            other
                .open(operation, proof, email, &epoch, expires, &encrypted)
                .unwrap_err(),
            Fault::Rejected
        );
    }
    #[test]
    fn production_actions_require_exact_https_dns_origins_and_local_actions_require_loopback() {
        assert!(safe_action(
            "https://cantos.example/",
            "https://cantos.example",
            false
        ));
        for (url, origin) in [
            ("http://cantos.example/", "http://cantos.example"),
            ("https://localhost/", "https://localhost"),
            ("https://cantos.example/?secret=x", "https://cantos.example"),
            ("https://cantos.example/", "https://other.example"),
            ("https://user@cantos.example/", "https://cantos.example"),
        ] {
            assert!(!safe_action(url, origin, false));
        }
        assert!(safe_action(
            "http://127.0.0.1:8082/",
            "http://127.0.0.1:8082",
            true
        ));
        assert!(!safe_action(
            "http://example.test:8082/",
            "http://example.test:8082",
            true
        ));
    }
    #[test]
    fn confirmation_binding_requires_runtime_key_and_every_exact_request_field() {
        let provider = fixture_provider();
        let op = OperationId::from_bytes([1; 16]).unwrap();
        let proof = ProofId::from_bytes([2; 16]).unwrap();
        let command = ProofCommandId::from_bytes([3; 16]).unwrap();
        let tag = provider
            .confirmation_tag(op, proof, command, "synthetic-action", "synthetic-password")
            .unwrap();
        assert_eq!(
            tag,
            provider
                .confirmation_tag(op, proof, command, "synthetic-action", "synthetic-password")
                .unwrap()
        );
        for (o, p, c, code, password) in [
            (
                OperationId::from_bytes([4; 16]).unwrap(),
                proof,
                command,
                "synthetic-action",
                "synthetic-password",
            ),
            (
                op,
                ProofId::from_bytes([4; 16]).unwrap(),
                command,
                "synthetic-action",
                "synthetic-password",
            ),
            (
                op,
                proof,
                ProofCommandId::from_bytes([4; 16]).unwrap(),
                "synthetic-action",
                "synthetic-password",
            ),
            (op, proof, command, "changed-action", "synthetic-password"),
            (op, proof, command, "synthetic-action", "changed-password"),
        ] {
            assert_ne!(
                tag,
                provider.confirmation_tag(o, p, c, code, password).unwrap()
            );
        }
        let mut other = fixture_provider();
        other.config.envelope_key = Zeroizing::new([9; 32]);
        assert_ne!(
            tag,
            other
                .confirmation_tag(op, proof, command, "synthetic-action", "synthetic-password")
                .unwrap()
        );
        other.config.envelope_key = Zeroizing::new([3; 32]);
        other.config.project = "demo-other-project".into();
        assert_ne!(
            tag,
            other
                .confirmation_tag(op, proof, command, "synthetic-action", "synthetic-password")
                .unwrap()
        );
        assert!(!format!("{provider:?}").contains("synthetic"));
    }
    #[test]
    fn production_binding_and_emulator_overrides_fail_before_secret_reads() {
        use std::collections::BTreeMap;
        let base = BTreeMap::from([
            ("CANTOS_IDENTITY_FIREBASE_PROJECT", "cantos-production"),
            (
                "CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER",
                "https://cantos.example/",
            ),
            (
                "CANTOS_IDENTITY_FIREBASE_ACTION_ORIGIN",
                "https://cantos.example",
            ),
            (
                "CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE",
                "/missing-private-key",
            ),
            ("CANTOS_IDENTITY_FIREBASE_PROJECT_BINDING_APPROVED", "true"),
            ("CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER_APPROVED", "true"),
            ("CANTOS_IDENTITY_FIREBASE_API_KEY", "synthetic-unit-key"),
            (
                "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
                "/missing-service-account",
            ),
        ]);
        let load = |values: &BTreeMap<&str, &str>, provider, local| {
            Config::load(provider, local, |name| {
                values.get(name).map(|value| value.to_string())
            })
        };
        for name in [
            "CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT",
            "CANTOS_IDENTITY_FIREBASE_ENDPOINT",
            "FIREBASE_AUTH_EMULATOR_HOST",
            "FIREBASE_AUTH_EMULATOR_URL",
        ] {
            let mut values = base.clone();
            values.insert(name, "127.0.0.1:9099");
            assert_eq!(
                load(&values, MailProvider::Firebase, false).unwrap_err(),
                ConfigError::Invalid(name)
            );
        }
        for name in [
            "CANTOS_IDENTITY_FIREBASE_PROJECT_BINDING_APPROVED",
            "CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER_APPROVED",
            "CANTOS_IDENTITY_FIREBASE_API_KEY",
            "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
        ] {
            let mut values = base.clone();
            values.remove(name);
            assert_eq!(
                load(&values, MailProvider::Firebase, false).unwrap_err(),
                ConfigError::Missing(name)
            );
        }
        assert_eq!(
            load(&base, MailProvider::Firebase, false).unwrap_err(),
            ConfigError::Invalid("CANTOS_IDENTITY_FIREBASE_ENVELOPE_KEY_FILE")
        );
        let mut local = base.clone();
        local.insert("CANTOS_IDENTITY_FIREBASE_PROJECT", "demo-cantos-unit");
        local.insert(
            "CANTOS_IDENTITY_FIREBASE_ACTION_HANDLER",
            "http://127.0.0.1:8082/",
        );
        local.insert(
            "CANTOS_IDENTITY_FIREBASE_ACTION_ORIGIN",
            "http://127.0.0.1:8082",
        );
        local.insert(
            "CANTOS_IDENTITY_FIREBASE_EMULATOR_ENDPOINT",
            "127.0.0.1:9099",
        );
        local.remove("CANTOS_IDENTITY_FIREBASE_API_KEY");
        local.remove("CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE");
        #[cfg(feature = "local-development")]
        for name in [
            "CANTOS_IDENTITY_FIREBASE_API_KEY",
            "CANTOS_IDENTITY_FIREBASE_SERVICE_ACCOUNT_FILE",
            "GOOGLE_APPLICATION_CREDENTIALS",
            "FIREBASE_TOKEN",
        ] {
            let mut values = local.clone();
            values.insert(name, "inherited-unit-value");
            assert_eq!(
                load(&values, MailProvider::FirebaseEmulator, true).unwrap_err(),
                ConfigError::Invalid(name)
            );
        }
        assert_eq!(
            load(&local, MailProvider::FirebaseEmulator, false).unwrap_err(),
            ConfigError::Invalid("CANTOS_IDENTITY_MAIL_PROVIDER")
        );
    }
    #[tokio::test]
    async fn acknowledged_rejection_is_distinct_from_lost_or_malformed_provider_response() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for (response, expected) in [
            ("HTTP/1.1 400 Bad Request\r\nContent-Length: 0\r\nConnection: close\r\n\r\n", Fault::Rejected),
            ("", Fault::Uncertain),
            ("HTTP/1.1 200 OK\r\nContent-Type: text/html\r\nContent-Length: 2\r\nConnection: close\r\n\r\n{}", Fault::Uncertain),
            ("HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: 1\r\nConnection: close\r\n\r\nx", Fault::Uncertain),
        ] {
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
            let address = listener.local_addr().unwrap();
            let server = tokio::spawn(async move {
                let (mut stream, _) = listener.accept().await.unwrap();
                let mut bytes = [0; 8192];
                let _ = stream.read(&mut bytes).await.unwrap();
                stream.write_all(response.as_bytes()).await.unwrap();
            });
            let mut provider = fixture_provider();
            provider.config.base = Url::parse(&format!("http://{address}/identitytoolkit.googleapis.com/v1/")).unwrap();
            assert_eq!(provider.inspect("synthetic-action", "owner@example.test").await.unwrap_err(), expected);
            server.await.unwrap();
        }
    }
    #[tokio::test]
    async fn provider_timeout_is_uncertain_and_is_not_automatically_retried() {
        use tokio::io::AsyncReadExt;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let server = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.unwrap();
            let mut bytes = [0; 8192];
            let _ = stream.read(&mut bytes).await.unwrap();
            tokio::time::sleep(Duration::from_secs(6)).await;
        });
        let mut provider = fixture_provider();
        provider.config.base = Url::parse(&format!(
            "http://{address}/identitytoolkit.googleapis.com/v1/"
        ))
        .unwrap();
        assert_eq!(
            provider
                .inspect("synthetic-action", "owner@example.test")
                .await
                .unwrap_err(),
            Fault::Uncertain
        );
        server.abort();
        assert!(server.await.unwrap_err().is_cancelled());
    }
    #[test]
    fn rest_method_colons_never_change_the_configured_origin() {
        let provider = fixture_provider();
        for method in [
            "accounts:signInWithCustomToken",
            "accounts:sendOobCode",
            "accounts:resetPassword",
            "accounts:update",
        ] {
            let endpoint = provider.endpoint(method);
            assert_eq!(endpoint.scheme(), "http");
            assert_eq!(endpoint.host_str(), Some("127.0.0.1"));
            assert_eq!(endpoint.port(), Some(9099));
            assert_eq!(
                endpoint.path(),
                format!("/identitytoolkit.googleapis.com/v1/{method}")
            );
        }
    }
}
