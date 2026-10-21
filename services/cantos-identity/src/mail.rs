//! Outbound mailbox-proof delivery. The port reports only what the transport knows: a message
//! was refused before DATA, accepted by the relay, or left uncertain. SMTP acceptance is not
//! mailbox delivery, and neither says anything about whether the proof was consumed.
//!
//! No proof, address or relay credential enters an error, a Debug rendering or a log line.

use std::{fmt, future::Future, net::IpAddr, pin::Pin};

use lettre::{
    message::{header::ContentType, Mailbox},
    transport::smtp::{
        authentication::{Credentials, Mechanism},
        client::{AsyncSmtpConnection, TlsParameters},
        extension::ClientId,
        Error as SmtpError,
    },
    Address, Message,
};

use crate::{
    domain::{email::EmailAddress, ids::OperationId, mailbox_proof::ProofId},
    persistence::ProofSecret,
    policy::{PROOF_TTL_SECONDS, SMTP_DEADLINE},
};

const PREFIX: &str = "CANTOS_IDENTITY_SMTP_";
const MAX_LINK_BYTES: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailFault {
    /// Definitely not sent: no SMTP session was opened or the relay refused before DATA.
    Rejected,
    /// DATA was sent but the relay's final answer was lost or malformed: it may be delivered.
    Uncertain,
    /// No mail transport is configured, so signup fails closed.
    NotConfigured,
}

pub type MailFuture<'a> = Pin<Box<dyn Future<Output = Result<(), MailFault>> + Send + 'a>>;

pub trait Mailer: Send + Sync {
    fn send<'a>(&'a self, mail: &'a ProofMail) -> MailFuture<'a>;
}

/// A mailbox-proof message. The link carries the proof secret, so Debug prints nothing.
pub struct ProofMail {
    recipient: EmailAddress,
    operation: OperationId,
    proof: ProofId,
    link: String,
}

impl ProofMail {
    pub fn new(
        recipient: EmailAddress,
        operation: OperationId,
        proof: ProofId,
        link: String,
    ) -> Self {
        Self {
            recipient,
            operation,
            proof,
            link,
        }
    }

    pub fn recipient(&self) -> &EmailAddress {
        &self.recipient
    }

    pub fn link(&self) -> &str {
        &self.link
    }
}

impl fmt::Debug for ProofMail {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ProofMail([REDACTED])")
    }
}

fn hex(bytes: &[u8]) -> String {
    use std::fmt::Write as _;
    bytes
        .iter()
        .fold(String::with_capacity(bytes.len() * 2), |mut out, byte| {
            let _ = write!(out, "{byte:02x}");
            out
        })
}

/// The front-end address the mailed link opens. It must be `https`, bare of query and
/// fragment, and short. The front end is a #335 deliverable; this link format is a draft.
#[derive(Clone, PartialEq, Eq)]
pub struct SignupLinkBase(String);

impl SignupLinkBase {
    pub fn parse(text: &str) -> Result<Self, MailConfigError> {
        let after = text
            .strip_prefix("https://")
            .ok_or(MailConfigError::Invalid)?;
        let authority = after.split('/').next().unwrap_or_default();
        let host = authority.split(':').next().unwrap_or_default();
        let valid = !host.is_empty()
            && !authority.contains('@')
            && text.len() <= 512
            && text.bytes().all(|b| b.is_ascii_graphic())
            && !text.contains(['#', '?'])
            && host.split('.').all(|label| {
                !label.is_empty()
                    && !label.starts_with('-')
                    && !label.ends_with('-')
                    && label
                        .bytes()
                        .all(|b| b.is_ascii_alphanumeric() || b == b'-')
            });
        if valid {
            Ok(Self(text.to_owned()))
        } else {
            Err(MailConfigError::Invalid)
        }
    }

    // ACCEPTANCE-PENDING(#335): no front end implements this link; the page that reads the
    // fragment and sends the explicit confirmation through the Server does not exist yet.
    /// The secret rides in the URL fragment so a GET, prefetch or link scanner never sends it
    /// to any server: only the page's own script reads it and POSTs it on explicit action.
    pub fn link(&self, operation: OperationId, proof: ProofId, secret: &ProofSecret) -> String {
        format!(
            "{}#operation={}&proof={}&secret={}",
            self.0,
            hex(&operation.to_bytes()),
            hex(&proof.to_bytes()),
            hex(secret.expose())
        )
    }
}

impl fmt::Debug for SignupLinkBase {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SignupLinkBase")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmtpTls {
    StartTls,
    Implicit,
    /// Plaintext to a literal loopback address without authentication. A relay on another
    /// host is never reachable in this mode; it exists so a run-owned local sink can be used.
    LoopbackPlaintext,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MailConfigError {
    Incomplete,
    Invalid,
}

impl fmt::Display for MailConfigError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Incomplete => "mail configuration is incomplete",
            Self::Invalid => "mail configuration is invalid",
        })
    }
}

impl std::error::Error for MailConfigError {}

#[derive(Clone)]
pub struct SmtpConfig {
    host: String,
    port: u16,
    tls: SmtpTls,
    from: Address,
    authentication: Option<(String, String)>,
}

impl fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SmtpConfig([REDACTED])")
    }
}

impl SmtpConfig {
    /// Absent configuration disables mail; a partial one is an error, never a half-working sender.
    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Option<Self>, MailConfigError> {
        let mut get = |name: &str| lookup(&format!("{PREFIX}{name}"));
        let (host, port, tls, from) = (get("HOST"), get("PORT"), get("TLS"), get("FROM"));
        let (username, password) = (get("USERNAME"), get("PASSWORD"));
        if [&host, &port, &tls, &from, &username, &password]
            .iter()
            .all(|value| value.is_none())
        {
            return Ok(None);
        }
        let tls = match tls.ok_or(MailConfigError::Incomplete)?.as_str() {
            "starttls" => SmtpTls::StartTls,
            "implicit" => SmtpTls::Implicit,
            "loopback-plaintext" => SmtpTls::LoopbackPlaintext,
            _ => return Err(MailConfigError::Invalid),
        };
        let port = port
            .ok_or(MailConfigError::Incomplete)?
            .parse()
            .map_err(|_| MailConfigError::Invalid)?;
        Self::new(
            host.ok_or(MailConfigError::Incomplete)?,
            port,
            tls,
            from.ok_or(MailConfigError::Incomplete)?,
            username,
            password,
        )
        .map(Some)
    }

    pub fn new(
        host: String,
        port: u16,
        tls: SmtpTls,
        from: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, MailConfigError> {
        if !valid_host(&host) || port == 0 || !from.is_ascii() {
            return Err(MailConfigError::Invalid);
        }
        let from: Address = from.parse().map_err(|_| MailConfigError::Invalid)?;
        let authentication = match (username, password) {
            (Some(user), Some(secret)) => {
                if !valid_credential(&user) || !valid_credential(&secret) {
                    return Err(MailConfigError::Invalid);
                }
                Some((user, secret))
            }
            (None, None) => None,
            _ => return Err(MailConfigError::Incomplete),
        };
        if tls == SmtpTls::LoopbackPlaintext
            && (!host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
                || authentication.is_some())
        {
            return Err(MailConfigError::Invalid);
        }
        Ok(Self {
            host,
            port,
            tls,
            from,
            authentication,
        })
    }

    pub fn is_loopback_plaintext(&self) -> bool {
        self.tls == SmtpTls::LoopbackPlaintext
    }
}

fn valid_credential(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}

fn valid_host(host: &str) -> bool {
    if host.parse::<IpAddr>().is_ok() {
        return true;
    }
    !host.is_empty()
        && host.len() <= 253
        && host.split('.').all(|part| {
            !part.is_empty()
                && part.len() <= 63
                && !part.starts_with('-')
                && !part.ends_with('-')
                && part.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-')
        })
}

/// What a deployment without mail configuration sends with: nothing, and signup fails closed.
pub struct UnconfiguredMailer;

impl Mailer for UnconfiguredMailer {
    fn send<'a>(&'a self, _mail: &'a ProofMail) -> MailFuture<'a> {
        Box::pin(async { Err(MailFault::NotConfigured) })
    }
}

// ACCEPTANCE-PENDING(production mail): only `loopback-plaintext` into a run-owned sink has run.
// STARTTLS/implicit TLS against a real relay, relay credentials, sender-domain authentication and
// actual mailbox delivery are not exercised and need an approved delivery path.
pub struct SmtpMailer {
    config: SmtpConfig,
    tls_parameters: Option<TlsParameters>,
}

impl fmt::Debug for SmtpMailer {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SmtpMailer([REDACTED])")
    }
}

impl SmtpMailer {
    /// Validates configuration only. It never probes a relay or sends mail at startup.
    pub fn new(config: SmtpConfig) -> Result<Self, MailConfigError> {
        let tls_parameters = match config.tls {
            SmtpTls::StartTls | SmtpTls::Implicit => Some(
                TlsParameters::new(config.host.clone()).map_err(|_| MailConfigError::Invalid)?,
            ),
            SmtpTls::LoopbackPlaintext => None,
        };
        Ok(Self {
            config,
            tls_parameters,
        })
    }

    async fn connect(&self) -> Result<AsyncSmtpConnection, MailFault> {
        let hello = ClientId::Domain(self.config.from.domain().to_owned());
        let implicit = match self.config.tls {
            SmtpTls::Implicit => self.tls_parameters.clone(),
            SmtpTls::StartTls | SmtpTls::LoopbackPlaintext => None,
        };
        let mut connection = AsyncSmtpConnection::connect_tokio1(
            (self.config.host.as_str(), self.config.port),
            Some(SMTP_DEADLINE),
            &hello,
            implicit,
            None,
        )
        .await
        .map_err(|_| MailFault::Rejected)?;
        if self.config.tls == SmtpTls::StartTls {
            // Required STARTTLS never falls back to plaintext or authenticates before TLS.
            let parameters = self.tls_parameters.clone().ok_or(MailFault::Rejected)?;
            connection
                .starttls(parameters, &hello)
                .await
                .map_err(|_| MailFault::Rejected)?;
        }
        if let Some((user, secret)) = &self.config.authentication {
            connection
                .auth(
                    &[Mechanism::Plain, Mechanism::Login],
                    &Credentials::new(user.clone(), secret.clone()),
                )
                .await
                .map_err(|_| MailFault::Rejected)?;
        }
        Ok(connection)
    }

    fn message(&self, mail: &ProofMail) -> Result<Message, MailFault> {
        if mail.link.len() > MAX_LINK_BYTES || mail.link.chars().any(char::is_control) {
            return Err(MailFault::Rejected);
        }
        let recipient = mail
            .recipient
            .as_str()
            .parse()
            .map_err(|_| MailFault::Rejected)?;
        Message::builder()
            .from(Mailbox::new(Some("Cantos".into()), self.config.from.clone()))
            .to(Mailbox::new(None, recipient))
            .message_id(Some(format!(
                "<{}.{}@{}>",
                hex(&mail.operation.to_bytes()),
                hex(&mail.proof.to_bytes()),
                self.config.from.domain()
            )))
            .subject("Xác nhận email để tạo tài khoản Cantos")
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "Ai đó đã dùng địa chỉ email này để bắt đầu tạo tài khoản Cantos.\r\n\r\n\
                 Nếu là bạn, hãy mở liên kết dưới đây, chọn mật khẩu và xác nhận. Liên kết có hiệu lực {} phút và chỉ dùng được một lần. Chỉ mở liên kết không tạo tài khoản.\r\n\r\n{}\r\n\r\n\
                 Nếu không phải bạn, hãy bỏ qua email này và không chia sẻ liên kết cho người khác.\r\n",
                PROOF_TTL_SECONDS / 60,
                mail.link
            ))
            .map_err(|_| MailFault::Rejected)
    }

    async fn deliver(&self, mail: &ProofMail) -> Result<(), MailFault> {
        let message = self.message(mail)?;
        // Before MAIL/DATA, a timeout or connection failure is positive evidence of no send.
        let mut connection = tokio::time::timeout(SMTP_DEADLINE, self.connect())
            .await
            .map_err(|_| MailFault::Rejected)??;
        let result = match tokio::time::timeout(
            SMTP_DEADLINE,
            connection.send(message.envelope(), &message.formatted()),
        )
        .await
        {
            Ok(Ok(response)) if response.has_code(250) => Ok(()),
            Ok(Ok(_)) | Err(_) => Err(MailFault::Uncertain),
            Ok(Err(error)) => Err(classify(&error)),
        };
        // QUIT is cleanup, not the acceptance boundary. Losing it cannot undo a final 250.
        let _ = tokio::time::timeout(std::time::Duration::from_secs(1), connection.abort()).await;
        result
    }
}

fn classify(error: &SmtpError) -> MailFault {
    if error.is_transient() || error.is_permanent() || error.is_client() {
        // Explicit negative SMTP replies refuse delivery; lettre's client errors precede DATA.
        MailFault::Rejected
    } else {
        // EOF, a malformed answer or network loss can follow a relay accepting DATA.
        MailFault::Uncertain
    }
}

impl Mailer for SmtpMailer {
    fn send<'a>(&'a self, mail: &'a ProofMail) -> MailFuture<'a> {
        Box::pin(self.deliver(mail))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(host: &str, tls: SmtpTls) -> Result<SmtpConfig, MailConfigError> {
        SmtpConfig::new(
            host.into(),
            2525,
            tls,
            "cantos@example.test".into(),
            None,
            None,
        )
    }

    #[test]
    fn absent_configuration_is_disabled_and_partial_configuration_is_rejected() {
        assert!(SmtpConfig::from_lookup(|_| None).unwrap().is_none());
        assert_eq!(
            SmtpConfig::from_lookup(|name| name
                .ends_with("HOST")
                .then(|| "smtp.example.test".into()))
            .unwrap_err(),
            MailConfigError::Incomplete
        );
    }

    #[test]
    fn only_explicit_security_modes_are_accepted() {
        for mode in [
            "starttls",
            "implicit",
            "loopback-plaintext",
            "opportunistic",
            "none",
            "",
        ] {
            let result = SmtpConfig::from_lookup(|name| match name.strip_prefix(PREFIX) {
                Some("HOST") => Some("127.0.0.1".into()),
                Some("PORT") => Some("2525".into()),
                Some("TLS") => Some(mode.into()),
                Some("FROM") => Some("cantos@example.test".into()),
                _ => None,
            });
            match mode {
                "starttls" | "implicit" | "loopback-plaintext" => {
                    assert!(result.unwrap().is_some())
                }
                _ => assert_eq!(result.unwrap_err(), MailConfigError::Invalid, "{mode:?}"),
            }
        }
    }

    #[test]
    fn plaintext_is_only_for_a_literal_loopback_without_authentication() {
        assert!(config("127.0.0.1", SmtpTls::LoopbackPlaintext).is_ok());
        assert!(config("::1", SmtpTls::LoopbackPlaintext).is_ok());
        for host in [
            "localhost",
            "smtp.example.test",
            "192.0.2.1",
            "127.0.0.1.example.test",
        ] {
            assert_eq!(
                config(host, SmtpTls::LoopbackPlaintext).unwrap_err(),
                MailConfigError::Invalid,
                "{host}"
            );
        }
        assert_eq!(
            SmtpConfig::new(
                "127.0.0.1".into(),
                2525,
                SmtpTls::LoopbackPlaintext,
                "cantos@example.test".into(),
                Some("user".into()),
                Some("secret".into()),
            )
            .unwrap_err(),
            MailConfigError::Invalid
        );
        assert!(!config("smtp.example.test", SmtpTls::StartTls)
            .unwrap()
            .is_loopback_plaintext());
    }

    #[test]
    fn hosts_senders_and_credentials_cannot_inject_protocol_text() {
        for host in [
            "smtp://smtp.example.test",
            "smtp.example.test\r\nAUTH",
            "user@host",
            "a..test",
            "",
        ] {
            assert_eq!(
                config(host, SmtpTls::StartTls).unwrap_err(),
                MailConfigError::Invalid
            );
        }
        for from in [
            "Cantos <cantos@example.test>",
            "cantos@example.test\r\nBcc:x@example.test",
        ] {
            assert_eq!(
                SmtpConfig::new(
                    "smtp.example.test".into(),
                    587,
                    SmtpTls::StartTls,
                    from.into(),
                    None,
                    None
                )
                .unwrap_err(),
                MailConfigError::Invalid
            );
        }
        assert_eq!(
            SmtpConfig::new(
                "smtp.example.test".into(),
                587,
                SmtpTls::StartTls,
                "cantos@example.test".into(),
                Some("user".into()),
                None,
            )
            .unwrap_err(),
            MailConfigError::Incomplete
        );
    }

    #[test]
    fn debug_never_contains_relay_secrets_sender_or_proof_link() {
        let value = SmtpConfig::new(
            "smtp.example.test".into(),
            465,
            SmtpTls::Implicit,
            "private@example.test".into(),
            Some("private-user".into()),
            Some("private-secret".into()),
        )
        .unwrap();
        assert_eq!(format!("{value:?}"), "SmtpConfig([REDACTED])");
        let mail = ProofMail::new(
            EmailAddress::parse("who@example.com").unwrap(),
            OperationId::from_bytes([1; 16]).unwrap(),
            ProofId::from_bytes([2; 16]).unwrap(),
            "https://theatre.example/x#secret=abc".into(),
        );
        assert_eq!(format!("{mail:?}"), "ProofMail([REDACTED])");
    }

    #[test]
    fn the_link_base_is_https_bare_and_places_the_secret_in_the_fragment() {
        let base = SignupLinkBase::parse("https://theatre.example/signup").unwrap();
        let secret = ProofSecret::from_bytes([0xAB; 32]);
        let link = base.link(
            OperationId::from_bytes([1; 16]).unwrap(),
            ProofId::from_bytes([2; 16]).unwrap(),
            &secret,
        );
        assert_eq!(
            link,
            format!(
                "https://theatre.example/signup#operation={}&proof={}&secret={}",
                "01".repeat(16),
                "02".repeat(16),
                "ab".repeat(32)
            )
        );
        assert!(!link.split('#').next().unwrap().contains("ab"));
        for bad in [
            "http://theatre.example/signup",
            "https://theatre.example/signup?next=/x",
            "https://theatre.example/signup#frag",
            "https://",
            "https:///path",
            "https://theatre.example:x@evil.example/signup",
            "https://user@theatre.example/signup",
            "https://bad host/x",
            "ftp://theatre.example",
            "",
        ] {
            assert!(SignupLinkBase::parse(bad).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn the_message_carries_the_link_only_in_the_body_and_binds_the_recipient() {
        let mailer =
            SmtpMailer::new(config("127.0.0.1", SmtpTls::LoopbackPlaintext).unwrap()).unwrap();
        let mail = ProofMail::new(
            EmailAddress::parse("Owner@Example.com").unwrap(),
            OperationId::from_bytes([1; 16]).unwrap(),
            ProofId::from_bytes([2; 16]).unwrap(),
            "https://theatre.example/signup#s3cret".into(),
        );
        let message = mailer.message(&mail).unwrap();
        let text = String::from_utf8(message.formatted()).unwrap();
        assert!(text.contains("To: Owner@example.com"));
        // The body is transfer-encoded (non-ASCII text): its decoded content is asserted by the
        // process journey, which reads the link out of a real captured message.
        let subject_line = text.lines().find(|l| l.starts_with("Subject:")).unwrap();
        assert!(!subject_line.contains("s3cret"));
        let injected = ProofMail::new(
            EmailAddress::parse("a@example.com").unwrap(),
            OperationId::from_bytes([1; 16]).unwrap(),
            ProofId::from_bytes([2; 16]).unwrap(),
            "https://x/\r\nBcc: v@example.com".into(),
        );
        assert_eq!(mailer.message(&injected).err(), Some(MailFault::Rejected));
    }
}
