//! SMTP is an outer adapter; no provider response, email address or bearer link enters errors.
use std::fmt;
use std::time::Duration;

use lettre::message::{header::ContentType, Mailbox};
use lettre::transport::smtp::authentication::{Credentials, Mechanism};
use lettre::transport::smtp::client::{AsyncSmtpConnection, TlsParameters};
use lettre::transport::smtp::extension::ClientId;
use lettre::transport::smtp::Error as SmtpError;
use lettre::Message;

use super::application::{Mailer, RegistrationFuture};
use super::domain::{MailKind, RegistrationError, RegistrationMail};
use super::mail_config::{SmtpConfig, SmtpConfigError, SmtpTls};

const SMTP_DEADLINE: Duration = Duration::from_secs(10);

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
    /// This only validates configuration; it never probes a relay or sends mail at startup.
    pub fn new(config: SmtpConfig) -> Result<Self, SmtpConfigError> {
        let tls_parameters = match config.tls {
            SmtpTls::StartTls | SmtpTls::Implicit => Some(
                TlsParameters::new(config.host.clone()).map_err(|_| SmtpConfigError::Invalid)?,
            ),
            SmtpTls::LoopbackFixture => None,
        };
        Ok(Self {
            config,
            tls_parameters,
        })
    }

    async fn connect(&self) -> Result<AsyncSmtpConnection, RegistrationError> {
        let hello_name = ClientId::Domain(self.config.from.domain().to_owned());
        let implicit = match self.config.tls {
            SmtpTls::Implicit => self.tls_parameters.clone(),
            SmtpTls::StartTls | SmtpTls::LoopbackFixture => None,
        };
        let mut connection = AsyncSmtpConnection::connect_tokio1(
            (self.config.host.as_str(), self.config.port),
            Some(SMTP_DEADLINE),
            &hello_name,
            implicit,
            None,
        )
        .await
        .map_err(|_| RegistrationError::DeliveryRejected)?;
        if self.config.tls == SmtpTls::StartTls {
            let parameters = self
                .tls_parameters
                .clone()
                .ok_or(RegistrationError::DeliveryRejected)?;
            // Required STARTTLS never falls back to plaintext or sends AUTH before TLS.
            connection
                .starttls(parameters, &hello_name)
                .await
                .map_err(|_| RegistrationError::DeliveryRejected)?;
        }
        if let Some((username, password)) = &self.config.authentication {
            connection
                .auth(
                    &[Mechanism::Plain, Mechanism::Login],
                    &Credentials::new(username.clone(), password.clone()),
                )
                .await
                .map_err(|_| RegistrationError::DeliveryRejected)?;
        }
        Ok(connection)
    }

    async fn send_mail(&self, mail: &RegistrationMail) -> Result<(), RegistrationError> {
        let message = self.message(mail)?;
        // Before MAIL/DATA, timeout or connection failure is positive evidence of no send.
        let mut connection = tokio::time::timeout(SMTP_DEADLINE, self.connect())
            .await
            .map_err(|_| RegistrationError::DeliveryRejected)??;
        let result = match tokio::time::timeout(
            SMTP_DEADLINE,
            connection.send(message.envelope(), &message.formatted()),
        )
        .await
        {
            Ok(Ok(response)) if response.has_code(250) => Ok(()),
            Ok(Ok(_)) | Err(_) => Err(RegistrationError::DeliveryUncertain),
            Ok(Err(error)) => Err(classify_send_error(&error)),
        };
        // QUIT is cleanup, not the acceptance boundary. Its loss cannot undo final DATA 250.
        let _ = tokio::time::timeout(Duration::from_secs(1), connection.abort()).await;
        result
    }

    fn message(&self, mail: &RegistrationMail) -> Result<Message, RegistrationError> {
        if mail.message_id().is_nil()
            || mail.link().len() > 8192
            || mail.link().chars().any(char::is_control)
        {
            return Err(RegistrationError::DeliveryRejected);
        }
        let recipient = mail
            .recipient()
            .parse()
            .map_err(|_| RegistrationError::DeliveryRejected)?;
        let (subject, introduction) = match mail.kind() {
            MailKind::Verification => (
                "Xác minh email cho tài khoản Cantos",
                "Để tiếp tục đăng ký Cantos, hãy mở liên kết xác minh email dưới đây.",
            ),
            MailKind::CredentialSetup => (
                "Thiết lập đăng nhập Cantos",
                "Email đã được xác minh. Hãy mở liên kết dưới đây để tự thiết lập thông tin đăng nhập tại hệ thống định danh của Cantos.",
            ),
        };
        let login = mail.login().map_or_else(String::new, |login| format!(
            "Tên đăng nhập của bạn: {login}\r\nHãy lưu tên này để đăng nhập Cantos sau khi thiết lập thông tin đăng nhập.\r\n\r\n"
        ));
        Message::builder()
            .from(Mailbox::new(Some("Cantos".into()), self.config.from.clone()))
            .to(Mailbox::new(None, recipient))
            .message_id(Some(format!(
                "<{}@{}>",
                mail.message_id(),
                self.config.from.domain()
            )))
            .subject(subject)
            .header(ContentType::TEXT_PLAIN)
            .body(format!(
                "{introduction}\r\n\r\n{login}{}\r\n\r\nNếu bạn không yêu cầu tạo tài khoản, hãy bỏ qua email này. Không chia sẻ liên kết này với người khác.\r\n",
                mail.link()
            ))
            .map_err(|_| RegistrationError::DeliveryRejected)
    }
}

impl Mailer for SmtpMailer {
    fn send(&self, mail: RegistrationMail) -> RegistrationFuture<'_, ()> {
        Box::pin(async move { self.send_mail(&mail).await })
    }
}

fn classify_send_error(error: &SmtpError) -> RegistrationError {
    if error.is_transient() || error.is_permanent() || error.is_client() {
        // Explicit negative SMTP replies refuse delivery. Lettre's client errors precede DATA.
        RegistrationError::DeliveryRejected
    } else {
        // EOF, malformed response and network loss can occur after the relay accepted DATA.
        RegistrationError::DeliveryUncertain
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::platform::theatre_registration::domain::{
        CreatedPerson, Email, PendingRegistration, SetupLink,
    };
    use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
    use tokio::net::TcpListener;
    use uuid::Uuid;

    #[derive(Clone, Copy)]
    enum FixtureMode {
        AcceptThenClose,
        RejectRecipient,
        RejectData,
        DropDataAck,
        UnexpectedDataAck,
    }

    struct Transcript {
        commands: Vec<String>,
        message: String,
    }

    async fn fixture(
        mode: FixtureMode,
        tls: SmtpTls,
    ) -> (SmtpMailer, tokio::task::JoinHandle<Transcript>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let port = listener.local_addr().unwrap().port();
        let task = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.unwrap();
            let mut stream = BufReader::new(stream);
            let mut transcript = Transcript {
                commands: Vec::new(),
                message: String::new(),
            };
            stream
                .get_mut()
                .write_all(b"220 local-fixture ESMTP\r\n")
                .await
                .unwrap();
            loop {
                let mut line = String::new();
                if stream.read_line(&mut line).await.unwrap() == 0 {
                    break;
                }
                transcript.commands.push(line.clone());
                let response: &[u8] = if line.starts_with("EHLO ") {
                    b"250-local-fixture\r\n250 8BITMIME\r\n"
                } else if line.starts_with("MAIL FROM:") {
                    b"250 sender accepted\r\n"
                } else if line.starts_with("RCPT TO:") {
                    match mode {
                        FixtureMode::RejectRecipient => b"550 recipient refused\r\n",
                        _ => b"250 recipient accepted\r\n",
                    }
                } else if line == "DATA\r\n" {
                    stream
                        .get_mut()
                        .write_all(b"354 continue\r\n")
                        .await
                        .unwrap();
                    loop {
                        let mut body_line = String::new();
                        assert_ne!(stream.read_line(&mut body_line).await.unwrap(), 0);
                        if body_line == ".\r\n" {
                            break;
                        }
                        transcript.message.push_str(&body_line);
                    }
                    match mode {
                        FixtureMode::DropDataAck => break,
                        FixtureMode::RejectData => b"451 not accepted\r\n",
                        FixtureMode::UnexpectedDataAck => b"354 unexpected continuation\r\n",
                        FixtureMode::AcceptThenClose | FixtureMode::RejectRecipient => {
                            stream.get_mut().write_all(b"250 queued\r\n").await.unwrap();
                            // Lose QUIT cleanup while preserving a confirmed final DATA response.
                            break;
                        }
                    }
                } else if line == "QUIT\r\n" {
                    break;
                } else {
                    panic!("unexpected SMTP fixture command");
                };
                stream.get_mut().write_all(response).await.unwrap();
            }
            transcript
        });
        let config = SmtpConfig::new(
            "127.0.0.1".into(),
            port,
            tls,
            "cantos@example.test".into(),
            None,
            None,
        )
        .unwrap();
        (SmtpMailer::new(config).unwrap(), task)
    }

    fn mail(kind: MailKind) -> RegistrationMail {
        let email = Email::parse("listener@example.test").unwrap();
        let link = "https://cantos.example.test/#verify?token=localfixture".to_owned();
        let id = Uuid::from_u128(42);
        match kind {
            MailKind::Verification => RegistrationMail::verification(email, link, id),
            MailKind::CredentialSetup => {
                let registration = PendingRegistration::new(id, email).unwrap();
                let person = CreatedPerson::confirmed(id, id).unwrap();
                RegistrationMail::credential_setup(
                    &registration,
                    &person,
                    SetupLink::new(link).unwrap(),
                )
                .unwrap()
            }
        }
    }

    #[tokio::test]
    async fn smtp_250_confirms_handoff_even_when_quit_response_is_lost() {
        for kind in [MailKind::Verification, MailKind::CredentialSetup] {
            let (sender, transcript) =
                fixture(FixtureMode::AcceptThenClose, SmtpTls::LoopbackFixture).await;
            assert_eq!(sender.send(mail(kind)).await, Ok(()));
            let transcript = transcript.await.unwrap();
            assert!(transcript
                .commands
                .iter()
                .any(|line| line == "RCPT TO:<listener@example.test>\r\n"));
            assert!(transcript
                .message
                .contains("Message-ID: <00000000-0000-0000-0000-00000000002a@example.test>"));
            let (headers, body) = transcript.message.split_once("\r\n\r\n").unwrap();
            let decoded = if headers
                .to_ascii_lowercase()
                .contains("content-transfer-encoding: base64")
            {
                use base64::Engine;
                String::from_utf8(
                    base64::engine::general_purpose::STANDARD
                        .decode(body.replace("\r\n", ""))
                        .unwrap(),
                )
                .unwrap()
            } else {
                body.replace("=\r\n", "").replace("=3D", "=")
            };
            assert!(decoded.contains("https://cantos.example.test/#verify?token=localfixture"));
            match kind {
                MailKind::CredentialSetup => {
                    assert!(decoded.contains("cantos_0000000000000000000000000000002a"))
                }
                MailKind::Verification => {
                    assert!(!decoded.contains("cantos_0000000000000000000000000000002a"))
                }
            }
        }
    }

    #[tokio::test]
    async fn explicit_rcpt_and_data_refusals_are_confirmed_not_sent() {
        for mode in [FixtureMode::RejectRecipient, FixtureMode::RejectData] {
            let (sender, transcript) = fixture(mode, SmtpTls::LoopbackFixture).await;
            assert_eq!(
                sender.send(mail(MailKind::Verification)).await,
                Err(RegistrationError::DeliveryRejected)
            );
            transcript.await.unwrap();
        }
    }

    #[tokio::test]
    async fn lost_or_ambiguous_data_response_is_never_reported_as_not_sent() {
        for mode in [FixtureMode::DropDataAck, FixtureMode::UnexpectedDataAck] {
            let (sender, transcript) = fixture(mode, SmtpTls::LoopbackFixture).await;
            assert_eq!(
                sender.send(mail(MailKind::CredentialSetup)).await,
                Err(RegistrationError::DeliveryUncertain)
            );
            assert!(!transcript.await.unwrap().message.is_empty());
        }
    }

    #[tokio::test]
    async fn required_starttls_sends_neither_credentials_nor_mail_on_downgrade() {
        let (mut sender, transcript) =
            fixture(FixtureMode::AcceptThenClose, SmtpTls::StartTls).await;
        sender.config.authentication = Some(("fixture-user".into(), "fixture-secret".into()));
        assert_eq!(
            sender.send(mail(MailKind::Verification)).await,
            Err(RegistrationError::DeliveryRejected)
        );
        let transcript = transcript.await.unwrap();
        assert_eq!(transcript.commands.len(), 1);
        assert!(transcript.commands[0].starts_with("EHLO "));
        assert!(transcript.message.is_empty());
    }

    #[test]
    fn mailer_debug_omits_recipient_and_bearer_links() {
        let config = SmtpConfig::new(
            "127.0.0.1".into(),
            2525,
            SmtpTls::LoopbackFixture,
            "cantos@example.test".into(),
            None,
            None,
        )
        .unwrap();
        assert_eq!(
            format!("{:?}", SmtpMailer::new(config).unwrap()),
            "SmtpMailer([REDACTED])"
        );
    }
}
