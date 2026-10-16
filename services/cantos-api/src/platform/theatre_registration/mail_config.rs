//! Explicit, vendor-neutral SMTP configuration. Construction performs no network I/O.
use lettre::Address;
use std::fmt;
use std::net::IpAddr;

const PREFIX: &str = "CANTOS_THEATRE_REGISTRATION_SMTP_";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmtpTls {
    StartTls,
    Implicit,
    /// Isolated fixture transport only; callers must refuse activation outside development.
    LoopbackFixture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SmtpConfigError {
    Incomplete,
    Invalid,
}

/// Secrets deliberately have no serde or derived Debug implementation.
#[derive(Clone)]
pub struct SmtpConfig {
    pub(super) host: String,
    pub(super) port: u16,
    pub(super) tls: SmtpTls,
    pub(super) from: Address,
    pub(super) authentication: Option<(String, String)>,
}

impl fmt::Debug for SmtpConfig {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("SmtpConfig([REDACTED])")
    }
}

impl SmtpConfig {
    /// Absent configuration disables SMTP. A partial configuration is never a working sender.
    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Option<Self>, SmtpConfigError> {
        let host = lookup(&format!("{PREFIX}HOST"));
        let port = lookup(&format!("{PREFIX}PORT"));
        let tls = lookup(&format!("{PREFIX}TLS"));
        let from = lookup(&format!("{PREFIX}FROM"));
        let username = lookup(&format!("{PREFIX}USERNAME"));
        let password = lookup(&format!("{PREFIX}PASSWORD"));
        if [&host, &port, &tls, &from, &username, &password]
            .iter()
            .all(|value| value.is_none())
        {
            return Ok(None);
        }
        let host = host.ok_or(SmtpConfigError::Incomplete)?;
        let port = port.ok_or(SmtpConfigError::Incomplete)?;
        let tls = tls.ok_or(SmtpConfigError::Incomplete)?;
        let from = from.ok_or(SmtpConfigError::Incomplete)?;
        let tls = match tls.as_str() {
            "starttls" => SmtpTls::StartTls,
            "implicit" => SmtpTls::Implicit,
            "loopback" => SmtpTls::LoopbackFixture,
            _ => return Err(SmtpConfigError::Invalid),
        };
        let port = port.parse().map_err(|_| SmtpConfigError::Invalid)?;
        Self::new(host, port, tls, from, username, password).map(Some)
    }

    pub fn new(
        host: String,
        port: u16,
        tls: SmtpTls,
        from: String,
        username: Option<String>,
        password: Option<String>,
    ) -> Result<Self, SmtpConfigError> {
        if !valid_host(&host) || port == 0 || !from.is_ascii() {
            return Err(SmtpConfigError::Invalid);
        }
        let from: Address = from.parse().map_err(|_| SmtpConfigError::Invalid)?;
        let authentication = match (username, password) {
            (Some(username), Some(password)) => {
                if !valid_credential(&username) || !valid_credential(&password) {
                    return Err(SmtpConfigError::Invalid);
                }
                Some((username, password))
            }
            (None, None) => None,
            _ => return Err(SmtpConfigError::Incomplete),
        };
        if tls == SmtpTls::LoopbackFixture && (!literal_loopback(&host) || authentication.is_some())
        {
            return Err(SmtpConfigError::Invalid);
        }
        Ok(Self {
            host,
            port,
            tls,
            from,
            authentication,
        })
    }

    pub fn is_loopback_fixture(&self) -> bool {
        self.tls == SmtpTls::LoopbackFixture
    }
}

fn valid_credential(value: &str) -> bool {
    !value.is_empty() && value.len() <= 4096 && !value.chars().any(char::is_control)
}

fn literal_loopback(host: &str) -> bool {
    host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback())
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
                && part
                    .bytes()
                    .all(|byte| byte.is_ascii_alphanumeric() || byte == b'-')
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(host: &str, tls: SmtpTls) -> Result<SmtpConfig, SmtpConfigError> {
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
    fn missing_configuration_is_disabled_and_partial_configuration_is_rejected() {
        assert!(SmtpConfig::from_lookup(|_| None).unwrap().is_none());
        assert_eq!(
            SmtpConfig::from_lookup(|name| name
                .ends_with("HOST")
                .then(|| "smtp.example.test".into()))
            .unwrap_err(),
            SmtpConfigError::Incomplete
        );
    }

    #[test]
    fn complete_env_configuration_accepts_only_explicit_security_modes() {
        for mode in ["starttls", "implicit", "loopback", "opportunistic", "none"] {
            let result = SmtpConfig::from_lookup(|name| match name.strip_prefix(PREFIX) {
                Some("HOST") => Some("127.0.0.1".into()),
                Some("PORT") => Some("2525".into()),
                Some("TLS") => Some(mode.into()),
                Some("FROM") => Some("cantos@example.test".into()),
                _ => None,
            });
            match mode {
                "starttls" | "implicit" | "loopback" => assert!(result.unwrap().is_some()),
                _ => assert_eq!(result.unwrap_err(), SmtpConfigError::Invalid),
            }
        }
    }

    #[test]
    fn plaintext_is_only_for_literal_loopback_without_authentication() {
        assert!(config("127.0.0.1", SmtpTls::LoopbackFixture).is_ok());
        assert!(config("::1", SmtpTls::LoopbackFixture).is_ok());
        for host in [
            "localhost",
            "smtp.example.test",
            "192.0.2.1",
            "127.0.0.1.example.test",
        ] {
            assert_eq!(
                config(host, SmtpTls::LoopbackFixture).unwrap_err(),
                SmtpConfigError::Invalid
            );
        }
        assert_eq!(
            SmtpConfig::new(
                "127.0.0.1".into(),
                2525,
                SmtpTls::LoopbackFixture,
                "cantos@example.test".into(),
                Some("fixture-user".into()),
                Some("fixture-secret".into()),
            )
            .unwrap_err(),
            SmtpConfigError::Invalid
        );
    }

    #[test]
    fn remote_hosts_require_a_known_tls_mode_and_bare_sender_address() {
        assert!(config("smtp.example.test", SmtpTls::StartTls).is_ok());
        assert!(config("smtp.example.test", SmtpTls::Implicit).is_ok());
        for host in [
            "smtp://smtp.example.test",
            "smtp.example.test\r\nAUTH",
            "user@host",
            "a..test",
        ] {
            assert_eq!(
                config(host, SmtpTls::StartTls).unwrap_err(),
                SmtpConfigError::Invalid
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
                SmtpConfigError::Invalid
            );
        }
    }

    #[test]
    fn debug_never_contains_relay_credentials_or_sender() {
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
    }
}
