//! Pure registration values and the durable at-most-once effect sequence.
use hmac::{Hmac, Mac};
use sha2::{Digest, Sha256};
use uuid::Uuid;

mod peer;
pub use peer::{PeerPolicy, TrustedProxies};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistrationError {
    InvalidEmail,
    InvalidToken,
    Configuration,
    Unavailable,
    DeliveryRejected,
    DeliveryUncertain,
    IssuerRejected,
    IssuerUncertain,
    Conflict,
    InvalidState,
}

/// Delivery spelling is preserved; the deduplication key is ASCII case-insensitive.
#[derive(Clone, PartialEq, Eq)]
pub struct Email(String);

impl Email {
    pub fn parse(value: &str) -> Result<Self, RegistrationError> {
        let value = value.trim();
        if value.len() > 254 || !value.is_ascii() {
            return Err(RegistrationError::InvalidEmail);
        }
        let mut parts = value.split('@');
        let local = parts.next().ok_or(RegistrationError::InvalidEmail)?;
        let domain = parts.next().ok_or(RegistrationError::InvalidEmail)?;
        let local_char = |v: u8| v.is_ascii_alphanumeric() || b".!#$%&'*+-/=?^_`{|}~".contains(&v);
        if parts.next().is_some()
            || local.is_empty()
            || local.len() > 64
            || local.starts_with('.')
            || local.ends_with('.')
            || local.contains("..")
            || !local.bytes().all(local_char)
            || !domain.contains('.')
            || domain.len() > 253
            || domain.split('.').any(|label| {
                label.is_empty()
                    || label.len() > 63
                    || label.starts_with('-')
                    || label.ends_with('-')
                    || !label
                        .bytes()
                        .all(|v| v.is_ascii_alphanumeric() || v == b'-')
            })
        {
            return Err(RegistrationError::InvalidEmail);
        }
        Ok(Self(format!("{local}@{}", domain.to_ascii_lowercase())))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
    pub fn canonical_key(&self) -> String {
        self.0.to_ascii_lowercase()
    }
}
impl std::fmt::Debug for Email {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Email([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TokenHash([u8; 32]);
impl TokenHash {
    pub fn from_token(value: &str) -> Result<Self, RegistrationError> {
        if value.len() != 64
            || !value
                .bytes()
                .all(|v| v.is_ascii_digit() || (b'a'..=b'f').contains(&v))
        {
            return Err(RegistrationError::InvalidToken);
        }
        Ok(Self(
            Sha256::digest(
                [
                    b"cantos-theatre-registration-proof-v1\0".as_slice(),
                    value.as_bytes(),
                ]
                .concat(),
            )
            .into(),
        ))
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}

/// A quota partition from the actual HTTP peer, never an untrusted forwarded header.
/// A deployment-specific HMAC prevents public address dictionary matching. This remains
/// personal quota metadata and needs the separately accepted retention policy.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct NetworkKey([u8; 32]);
impl NetworkKey {
    pub fn from_peer(peer: std::net::IpAddr, key: &[u8; 32]) -> Result<Self, RegistrationError> {
        let mut hash =
            Hmac::<Sha256>::new_from_slice(key).map_err(|_| RegistrationError::Configuration)?;
        hash.update(b"cantos-theatre-registration-network-v1\0");
        let peer = match peer {
            std::net::IpAddr::V6(ip) => match ip.to_ipv4_mapped() {
                Some(ip) => std::net::IpAddr::V4(ip),
                None => std::net::IpAddr::V6(ip),
            },
            other => other,
        };
        match peer {
            std::net::IpAddr::V4(ip) => {
                hash.update(&[4]);
                hash.update(&ip.octets());
            }
            std::net::IpAddr::V6(ip) => {
                hash.update(&[6]);
                hash.update(&ip.octets());
            }
        }
        Ok(Self(hash.finalize().into_bytes().into()))
    }
    pub fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }
}
impl std::fmt::Debug for NetworkKey {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("NetworkKey([REDACTED])")
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingRegistration {
    id: Uuid,
    email: Email,
}
impl PendingRegistration {
    pub fn new(id: Uuid, email: Email) -> Result<Self, RegistrationError> {
        if id.is_nil() {
            return Err(RegistrationError::InvalidState);
        }
        Ok(Self { id, email })
    }
    pub fn id(&self) -> Uuid {
        self.id
    }
    pub fn email(&self) -> &Email {
        &self.email
    }
    pub fn provider_name(&self) -> String {
        format!("cantos_{}", self.id.simple())
    }
}

#[derive(Clone, Debug)]
pub struct Candidate {
    pub registration: PendingRegistration,
    pub token_hash: TokenHash,
    pub network_key: NetworkKey,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RegistrationPolicy {
    verification_seconds: u32,
    resend_seconds: u32,
    window_seconds: u32,
    max_requests: u32,
    max_network_requests: u32,
}
impl RegistrationPolicy {
    /// Explicit activation configuration; no production policy is silently selected here.
    pub fn new(
        verification_seconds: u32,
        resend_seconds: u32,
        window_seconds: u32,
        max_requests: u32,
        max_network_requests: u32,
    ) -> Result<Self, RegistrationError> {
        if !(60..=3600).contains(&verification_seconds)
            || !(30..=verification_seconds).contains(&resend_seconds)
            || !(30..=86400).contains(&window_seconds)
            || !(1..=10000).contains(&max_requests)
            || !(1..=max_requests).contains(&max_network_requests)
        {
            return Err(RegistrationError::Configuration);
        }
        Ok(Self {
            verification_seconds,
            resend_seconds,
            window_seconds,
            max_requests,
            max_network_requests,
        })
    }
    pub fn verification_seconds(self) -> u32 {
        self.verification_seconds
    }
    pub fn resend_seconds(self) -> u32 {
        self.resend_seconds
    }
    pub fn window_seconds(self) -> u32 {
        self.window_seconds
    }
    pub fn max_requests(self) -> u32 {
        self.max_requests
    }
    pub fn max_network_requests(self) -> u32 {
        self.max_network_requests
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Phase {
    AwaitingVerification,
    Creating,
    Created,
    SetupStarted,
    Delivering,
    Complete,
    Blocked,
    Cancelled,
}
impl Phase {
    pub fn can_advance(self, next: Self) -> bool {
        matches!(
            (self, next),
            (Self::AwaitingVerification, Self::Creating)
                | (Self::Creating, Self::Created)
                | (Self::Created, Self::SetupStarted)
                | (Self::SetupStarted, Self::Delivering)
                | (Self::Delivering, Self::Complete)
                | (Self::AwaitingVerification, Self::Cancelled)
                | (
                    Self::Creating | Self::Created | Self::SetupStarted | Self::Delivering,
                    Self::Blocked
                )
        )
    }
    pub fn may_replace_challenge(self, now: u64, last_sent: u64, cooldown: u32) -> bool {
        matches!(self, Self::AwaitingVerification | Self::Cancelled)
            && now
                .checked_sub(last_sent)
                .is_some_and(|elapsed| elapsed >= u64::from(cooldown))
    }
    pub fn may_claim(self, now: u64, expires: u64) -> bool {
        self == Self::AwaitingVerification && now < expires
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlockReason {
    CreateRejected,
    CreateUncertain,
    SetupRejected,
    SetupUncertain,
    SetupDeliveryRejected,
    SetupDeliveryUncertain,
    BindingMismatch,
}

/// Adapter evidence of a positive create for this exact preselected random person UUID.
/// Existing-person discovery, a conflict or a timeout can never construct this witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CreatedPerson {
    registration_id: Uuid,
    subject: Uuid,
}
impl CreatedPerson {
    pub(crate) fn confirmed(
        registration_id: Uuid,
        subject: Uuid,
    ) -> Result<Self, RegistrationError> {
        if registration_id.is_nil() || registration_id != subject {
            return Err(RegistrationError::InvalidState);
        }
        Ok(Self {
            registration_id,
            subject,
        })
    }
    pub fn registration_id(&self) -> Uuid {
        self.registration_id
    }
    pub fn subject(&self) -> Uuid {
        self.subject
    }
}

#[derive(Clone)]
pub struct SetupLink(String);
impl SetupLink {
    pub(crate) fn new(value: String) -> Result<Self, RegistrationError> {
        let parsed = url::Url::parse(&value).map_err(|_| RegistrationError::IssuerRejected)?;
        if parsed.scheme() != "https"
            || !parsed.username().is_empty()
            || parsed.password().is_some()
        {
            return Err(RegistrationError::IssuerRejected);
        }
        Ok(Self(value))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}
impl std::fmt::Debug for SetupLink {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("SetupLink([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailKind {
    Verification,
    CredentialSetup,
}

#[derive(Clone)]
enum MailPurpose {
    Verification,
    CredentialSetup { login: String },
}

#[derive(Clone)]
pub struct RegistrationMail {
    recipient: Email,
    link: String,
    message_id: Uuid,
    purpose: MailPurpose,
}
impl RegistrationMail {
    pub(crate) fn verification(recipient: Email, link: String, message_id: Uuid) -> Self {
        Self {
            recipient,
            link,
            message_id,
            purpose: MailPurpose::Verification,
        }
    }
    /// The login belongs only to the exact UUID whose creation was confirmed. Email is
    /// delivery contact, not a Kanidm login identifier or permission to select another person.
    pub(crate) fn credential_setup(
        registration: &PendingRegistration,
        person: &CreatedPerson,
        link: SetupLink,
    ) -> Result<Self, RegistrationError> {
        if registration.id() != person.registration_id() || registration.id() != person.subject() {
            return Err(RegistrationError::InvalidState);
        }
        Ok(Self {
            recipient: registration.email().clone(),
            link: link.as_str().to_owned(),
            message_id: registration.id(),
            purpose: MailPurpose::CredentialSetup {
                login: registration.provider_name(),
            },
        })
    }
    pub fn recipient(&self) -> &str {
        self.recipient.as_str()
    }
    pub fn link(&self) -> &str {
        &self.link
    }
    pub fn message_id(&self) -> Uuid {
        self.message_id
    }
    pub fn kind(&self) -> MailKind {
        match self.purpose {
            MailPurpose::Verification => MailKind::Verification,
            MailPurpose::CredentialSetup { .. } => MailKind::CredentialSetup,
        }
    }
    pub fn login(&self) -> Option<&str> {
        match &self.purpose {
            MailPurpose::Verification => None,
            MailPurpose::CredentialSetup { login } => Some(login),
        }
    }
}
impl std::fmt::Debug for RegistrationMail {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RegistrationMail")
            .field("message_id", &self.message_id)
            .field("kind", &self.kind())
            .finish_non_exhaustive()
    }
}

#[cfg(test)]
mod tests;
