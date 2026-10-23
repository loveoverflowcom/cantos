//! Validated runtime configuration. Every value is explicit: there are no defaults for a path,
//! a caller or a mail relay, and an error names the variable, never its value.

use std::{collections::BTreeSet, path::PathBuf};

use crate::{
    mail::{MailConfigError, SignupLinkBase, SmtpConfig},
    policy::MAX_CALLER_UIDS,
};

pub const DATABASE: &str = "CANTOS_IDENTITY_DATABASE";
pub const SOCKET: &str = "CANTOS_IDENTITY_SOCKET";
pub const SOCKET_MODE: &str = "CANTOS_IDENTITY_SOCKET_MODE";
pub const SERVER_UIDS: &str = "CANTOS_IDENTITY_SERVER_UIDS";
pub const SIGNUP_LINK_BASE: &str = "CANTOS_IDENTITY_SIGNUP_LINK_BASE";
pub const MAIL_PROVIDER: &str = "CANTOS_IDENTITY_MAIL_PROVIDER";
pub const LOCAL_DEVELOPMENT: &str = "CANTOS_IDENTITY_LOCAL_DEVELOPMENT";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Missing(&'static str),
    Invalid(&'static str),
    UnsupportedProvider,
}

impl std::fmt::Display for ConfigError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Missing(name) => write!(f, "{name} is required"),
            Self::Invalid(name) => write!(f, "{name} is invalid"),
            Self::UnsupportedProvider => f.write_str("Firebase verification is not implemented: a durable native proof bridge is required; no SMTP fallback was selected"),
        }
    }
}

impl std::error::Error for ConfigError {}

/// Who may open the socket besides its owner. Group access still needs a matching uid in the
/// caller allowlist; the directory permission check is what keeps others out.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SocketMode {
    OwnerOnly,
    OwnerAndGroup,
}

impl SocketMode {
    pub const fn bits(self) -> u32 {
        match self {
            Self::OwnerOnly => 0o600,
            Self::OwnerAndGroup => 0o660,
        }
    }
}

#[derive(Clone, Debug)]
pub struct MigrateConfig {
    pub database: PathBuf,
}

#[derive(Clone, Debug)]
pub struct ServeConfig {
    pub database: PathBuf,
    pub socket: PathBuf,
    pub socket_mode: SocketMode,
    pub server_uids: BTreeSet<u32>,
    pub link: SignupLinkBase,
    pub smtp: Option<SmtpConfig>,
}

fn absolute_path(
    name: &'static str,
    lookup: &mut impl FnMut(&str) -> Option<String>,
) -> Result<PathBuf, ConfigError> {
    let value = lookup(name).ok_or(ConfigError::Missing(name))?;
    let path = PathBuf::from(value);
    if !path.is_absolute() || path.file_name().is_none() || path.to_str().is_none() {
        return Err(ConfigError::Invalid(name));
    }
    Ok(path)
}

impl MigrateConfig {
    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        Ok(Self {
            database: absolute_path(DATABASE, &mut lookup)?,
        })
    }
}

impl ServeConfig {
    pub fn from_lookup(
        mut lookup: impl FnMut(&str) -> Option<String>,
    ) -> Result<Self, ConfigError> {
        // Validate the provider before paths or any credential file could be opened. The
        // current SMTP port cannot safely carry Firebase's distinct email-action capability.
        let local = match lookup(LOCAL_DEVELOPMENT).as_deref() {
            None => false,
            Some("1") if cfg!(feature = "local-development") => true,
            Some(_) => return Err(ConfigError::Invalid(LOCAL_DEVELOPMENT)),
        };
        let provider = lookup(MAIL_PROVIDER);
        match provider.as_deref() {
            None | Some("smtp" | "disabled") => {}
            Some("firebase" | "firebase-emulator") => return Err(ConfigError::UnsupportedProvider),
            Some(_) => return Err(ConfigError::Invalid(MAIL_PROVIDER)),
        }
        // Never interpret SDK emulator/credential exports as native configuration. A future
        // adapter needs its own project/credential/action binding, not ambient ADC discovery.
        for name in [
            "FIREBASE_AUTH_EMULATOR_HOST",
            "FIREBASE_AUTH_EMULATOR_URL",
            "CANTOS_IDENTITY_FIREBASE_PROJECT",
            "CANTOS_IDENTITY_FIREBASE_API_KEY",
            "CANTOS_IDENTITY_FIREBASE_CREDENTIALS",
            "CANTOS_IDENTITY_FIREBASE_ENDPOINT",
        ] {
            if lookup(name).is_some() {
                return Err(ConfigError::Invalid(name));
            }
        }
        let database = absolute_path(DATABASE, &mut lookup)?;
        let socket = absolute_path(SOCKET, &mut lookup)?;
        let socket_mode = match lookup(SOCKET_MODE).as_deref() {
            None | Some("0600") => SocketMode::OwnerOnly,
            Some("0660") => SocketMode::OwnerAndGroup,
            Some(_) => return Err(ConfigError::Invalid(SOCKET_MODE)),
        };
        let uids = lookup(SERVER_UIDS).ok_or(ConfigError::Missing(SERVER_UIDS))?;
        let mut server_uids = BTreeSet::new();
        for part in uids.split(',') {
            let valid = !part.is_empty() && part.bytes().all(|b| b.is_ascii_digit());
            let uid = part
                .parse::<u32>()
                .ok()
                .filter(|_| valid)
                .ok_or(ConfigError::Invalid(SERVER_UIDS))?;
            server_uids.insert(uid);
        }
        if server_uids.is_empty() || server_uids.len() > MAX_CALLER_UIDS {
            return Err(ConfigError::Invalid(SERVER_UIDS));
        }
        let link_text = lookup(SIGNUP_LINK_BASE).ok_or(ConfigError::Missing(SIGNUP_LINK_BASE))?;
        let parse_link = || SignupLinkBase::parse(&link_text);
        #[cfg(feature = "local-development")]
        let link = if local {
            SignupLinkBase::parse_development(&link_text)
        } else {
            parse_link()
        };
        #[cfg(not(feature = "local-development"))]
        let link = {
            let _ = local;
            parse_link()
        };
        let link = link.map_err(|_| ConfigError::Invalid(SIGNUP_LINK_BASE))?;
        let smtp = SmtpConfig::from_lookup(&mut lookup).map_err(|error| match error {
            MailConfigError::Incomplete => ConfigError::Missing("CANTOS_IDENTITY_SMTP_*"),
            MailConfigError::Invalid => ConfigError::Invalid("CANTOS_IDENTITY_SMTP_*"),
        })?;
        if provider.as_deref() == Some("disabled") && smtp.is_some() {
            return Err(ConfigError::Invalid(MAIL_PROVIDER));
        }
        Ok(Self {
            database,
            socket,
            socket_mode,
            server_uids,
            link,
            smtp,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn lookup<'a>(entries: &'a [(&'a str, &'a str)]) -> impl FnMut(&str) -> Option<String> + 'a {
        move |name| {
            entries
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| (*value).to_owned())
        }
    }

    const BASE: [(&str, &str); 4] = [
        (DATABASE, "/var/lib/cantos/identity.db"),
        (SOCKET, "/run/cantos/identity.sock"),
        (SERVER_UIDS, "1001"),
        (SIGNUP_LINK_BASE, "https://theatre.example/signup"),
    ];

    #[test]
    fn a_complete_configuration_parses_and_mail_is_optional() {
        let config = ServeConfig::from_lookup(lookup(&BASE)).unwrap();
        assert_eq!(config.socket_mode, SocketMode::OwnerOnly);
        assert_eq!(config.server_uids.into_iter().collect::<Vec<_>>(), [1001]);
        assert!(config.smtp.is_none());
    }

    #[test]
    fn each_required_value_is_reported_by_name_when_absent() {
        for missing in [DATABASE, SOCKET, SERVER_UIDS, SIGNUP_LINK_BASE] {
            let entries: Vec<_> = BASE
                .iter()
                .filter(|(key, _)| *key != missing)
                .copied()
                .collect();
            assert_eq!(
                ServeConfig::from_lookup(lookup(&entries)).unwrap_err(),
                ConfigError::Missing(missing)
            );
        }
        assert_eq!(
            MigrateConfig::from_lookup(lookup(&[])).unwrap_err(),
            ConfigError::Missing(DATABASE)
        );
    }

    #[test]
    fn paths_must_be_absolute_and_uids_plain_bounded_decimals() {
        let with = |key: &'static str, value: &'static str| {
            let mut entries: Vec<_> = BASE.iter().filter(|(k, _)| *k != key).copied().collect();
            entries.push((key, value));
            ServeConfig::from_lookup(lookup(&entries))
        };
        assert_eq!(
            with(DATABASE, "identity.db").unwrap_err(),
            ConfigError::Invalid(DATABASE)
        );
        assert_eq!(
            with(SOCKET, "relative/x.sock").unwrap_err(),
            ConfigError::Invalid(SOCKET)
        );
        assert_eq!(with(SOCKET, "/").unwrap_err(), ConfigError::Invalid(SOCKET));
        for uids in [
            "",
            "x",
            "1,,2",
            "-1",
            "1 ",
            "4294967296",
            "1,2,3,4,5,6,7,8,9",
        ] {
            assert_eq!(
                with(SERVER_UIDS, uids).unwrap_err(),
                ConfigError::Invalid(SERVER_UIDS),
                "{uids:?}"
            );
        }
        assert!(with(SERVER_UIDS, "0,501,4294967295").is_ok());
        assert_eq!(
            with(SOCKET_MODE, "0666").unwrap_err(),
            ConfigError::Invalid(SOCKET_MODE)
        );
        assert_eq!(
            with(SOCKET_MODE, "0660").unwrap().socket_mode,
            SocketMode::OwnerAndGroup
        );
        assert_eq!(
            with(SIGNUP_LINK_BASE, "http://theatre.example").unwrap_err(),
            ConfigError::Invalid(SIGNUP_LINK_BASE)
        );
    }

    #[test]
    fn errors_name_the_variable_and_never_the_value() {
        let entries = [
            (DATABASE, "relative-secret-path"),
            (SOCKET, "/x/s"),
            (SERVER_UIDS, "1"),
            (SIGNUP_LINK_BASE, "https://a.example"),
        ];
        let error = ServeConfig::from_lookup(lookup(&entries)).unwrap_err();
        let text = format!("{error} {error:?}");
        assert!(text.contains(DATABASE) && !text.contains("relative-secret-path"));
    }

    #[test]
    fn partial_mail_configuration_refuses_to_start() {
        let mut entries = BASE.to_vec();
        entries.push(("CANTOS_IDENTITY_SMTP_HOST", "127.0.0.1"));
        assert!(matches!(
            ServeConfig::from_lookup(lookup(&entries)).unwrap_err(),
            ConfigError::Missing(_)
        ));
        entries.extend([
            ("CANTOS_IDENTITY_SMTP_PORT", "2525"),
            ("CANTOS_IDENTITY_SMTP_TLS", "loopback-plaintext"),
            ("CANTOS_IDENTITY_SMTP_FROM", "cantos@example.test"),
        ]);
        assert!(ServeConfig::from_lookup(lookup(&entries))
            .unwrap()
            .smtp
            .is_some());
    }

    #[test]
    fn firebase_selection_never_falls_back_to_smtp_or_reads_a_credential() {
        for provider in ["firebase", "firebase-emulator"] {
            let mut observed = Vec::new();
            let result = ServeConfig::from_lookup(|name| {
                observed.push(name.to_owned());
                (name == MAIL_PROVIDER).then(|| provider.to_owned())
            });
            assert_eq!(result.unwrap_err(), ConfigError::UnsupportedProvider);
            assert_eq!(observed, [LOCAL_DEVELOPMENT, MAIL_PROVIDER]);
        }
    }

    #[test]
    fn sdk_emulator_overrides_and_mixed_disabled_mail_are_rejected() {
        for name in [
            "FIREBASE_AUTH_EMULATOR_HOST",
            "FIREBASE_AUTH_EMULATOR_URL",
            "CANTOS_IDENTITY_FIREBASE_PROJECT",
            "CANTOS_IDENTITY_FIREBASE_ENDPOINT",
        ] {
            let mut entries = BASE.to_vec();
            entries.push((name, "sensitive-invalid-value"));
            let error = ServeConfig::from_lookup(lookup(&entries)).unwrap_err();
            assert_eq!(error, ConfigError::Invalid(name));
            assert!(!error.to_string().contains("sensitive-invalid-value"));
        }
        let mut entries = BASE.to_vec();
        entries.extend([
            (MAIL_PROVIDER, "disabled"),
            ("CANTOS_IDENTITY_SMTP_HOST", "127.0.0.1"),
            ("CANTOS_IDENTITY_SMTP_PORT", "2525"),
            ("CANTOS_IDENTITY_SMTP_TLS", "loopback-plaintext"),
            ("CANTOS_IDENTITY_SMTP_FROM", "cantos@example.test"),
        ]);
        assert_eq!(
            ServeConfig::from_lookup(lookup(&entries)).unwrap_err(),
            ConfigError::Invalid(MAIL_PROVIDER)
        );
    }

    #[test]
    fn local_http_requires_both_the_development_build_and_explicit_composition() {
        let mut entries: Vec<_> = BASE
            .iter()
            .filter(|(name, _)| *name != SIGNUP_LINK_BASE)
            .copied()
            .collect();
        entries.push((SIGNUP_LINK_BASE, "http://127.0.0.1:8082/"));
        assert_eq!(
            ServeConfig::from_lookup(lookup(&entries)).unwrap_err(),
            ConfigError::Invalid(SIGNUP_LINK_BASE)
        );
        entries.push((LOCAL_DEVELOPMENT, "1"));
        #[cfg(feature = "local-development")]
        assert!(ServeConfig::from_lookup(lookup(&entries)).is_ok());
        #[cfg(not(feature = "local-development"))]
        assert_eq!(
            ServeConfig::from_lookup(lookup(&entries)).unwrap_err(),
            ConfigError::Invalid(LOCAL_DEVELOPMENT)
        );
    }
}
