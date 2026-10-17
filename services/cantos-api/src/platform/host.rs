//! Host mode decision for `cantos-server`: a pure function of the process environment.
//!
//! `development` keeps the historical local contract (loopback bind, derived HTTP origin,
//! development Studio tokens, optional static Studio files). `production` is the deployed
//! contract: an explicit bind inside the private container network, one explicit HTTPS Web
//! origin, no static serving, no development Studio credential and no development-only
//! storage, trust or mail fixtures. PostgreSQL stays local (loopback or a Unix socket) in both
//! modes; see [`crate::platform::postgres::local_config`].
use std::net::SocketAddr;

/// The closed set of host modes. Anything else in `CANTOS_ENV` refuses to start.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostMode {
    Development,
    Production,
}

/// Validated, immutable host settings. Construction is the only proof of a valid mode.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct HostSettings {
    mode: HostMode,
    bind: SocketAddr,
    web_origin: String,
    web_dist: Option<String>,
}

impl HostSettings {
    pub fn mode(&self) -> HostMode {
        self.mode
    }
    pub fn bind(&self) -> SocketAddr {
        self.bind
    }
    pub fn web_origin(&self) -> &str {
        &self.web_origin
    }
    /// Static Studio files for the development host; production never serves files.
    pub fn web_dist(&self) -> Option<&str> {
        self.web_dist.as_deref()
    }
    /// Development Studio session tokens are honored only by a development host.
    pub fn development_sessions(&self) -> bool {
        self.mode == HostMode::Development
    }
}

/// Every refusal names the offending input; none of them carries a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum HostConfigError {
    UnknownMode,
    InvalidBind,
    DevelopmentBindNotLoopback,
    ProductionBindRequired,
    ProductionOriginRequired,
    ProductionOriginNotHttps,
    ProductionStaticFiles,
    ProductionDevelopmentAudioRoot,
    ProductionPrivateIssuerTrust,
}

impl HostConfigError {
    pub fn message(self) -> &'static str {
        match self {
            Self::UnknownMode => "CANTOS_ENV must be exactly development or production",
            Self::InvalidBind => "CANTOS_BIND_ADDRESS must be an IP:port socket address",
            Self::DevelopmentBindNotLoopback => "development host must bind loopback",
            Self::ProductionBindRequired => "production requires an explicit CANTOS_BIND_ADDRESS",
            Self::ProductionOriginRequired => "production requires an explicit CANTOS_WEB_ORIGIN",
            Self::ProductionOriginNotHttps => {
                "production CANTOS_WEB_ORIGIN must be an exact https origin without path"
            }
            Self::ProductionStaticFiles => {
                "production serves no static files; unset CANTOS_WEB_DIST"
            }
            Self::ProductionDevelopmentAudioRoot => {
                "production refuses the development CANTOS_AUDIO_ROOT; use CANTOS_STORAGE_CONFIG"
            }
            Self::ProductionPrivateIssuerTrust => {
                "production uses public issuer trust only; unset CANTOS_THEATRE_OIDC_CA_FILE"
            }
        }
    }
}

const DEVELOPMENT_BIND: &str = "127.0.0.1:8080";
const DEVELOPMENT_DIST: &str = "apps/cantos-studio/web/dist";

pub fn host_settings(
    lookup: impl Fn(&str) -> Option<String>,
) -> Result<HostSettings, HostConfigError> {
    let mode = match lookup("CANTOS_ENV").as_deref() {
        Some("development") => HostMode::Development,
        Some("production") => HostMode::Production,
        _ => return Err(HostConfigError::UnknownMode),
    };
    match mode {
        HostMode::Development => {
            let bind: SocketAddr = lookup("CANTOS_BIND_ADDRESS")
                .unwrap_or_else(|| DEVELOPMENT_BIND.into())
                .parse()
                .map_err(|_| HostConfigError::InvalidBind)?;
            if !bind.ip().is_loopback() {
                return Err(HostConfigError::DevelopmentBindNotLoopback);
            }
            Ok(HostSettings {
                mode,
                bind,
                web_origin: lookup("CANTOS_WEB_ORIGIN").unwrap_or_else(|| format!("http://{bind}")),
                web_dist: Some(
                    lookup("CANTOS_WEB_DIST").unwrap_or_else(|| DEVELOPMENT_DIST.into()),
                ),
            })
        }
        HostMode::Production => {
            let bind: SocketAddr = lookup("CANTOS_BIND_ADDRESS")
                .ok_or(HostConfigError::ProductionBindRequired)?
                .parse()
                .map_err(|_| HostConfigError::InvalidBind)?;
            if bind.port() == 0 {
                return Err(HostConfigError::InvalidBind);
            }
            let origin =
                lookup("CANTOS_WEB_ORIGIN").ok_or(HostConfigError::ProductionOriginRequired)?;
            if !exact_https_origin(&origin) {
                return Err(HostConfigError::ProductionOriginNotHttps);
            }
            if lookup("CANTOS_WEB_DIST").is_some() {
                return Err(HostConfigError::ProductionStaticFiles);
            }
            if lookup("CANTOS_AUDIO_ROOT").is_some() {
                return Err(HostConfigError::ProductionDevelopmentAudioRoot);
            }
            if lookup("CANTOS_THEATRE_OIDC_CA_FILE").is_some() {
                return Err(HostConfigError::ProductionPrivateIssuerTrust);
            }
            Ok(HostSettings {
                mode,
                bind,
                web_origin: origin,
                web_dist: None,
            })
        }
    }
}

/// `https://host[:port]` exactly as the browser serializes the origin: no path, query,
/// fragment, credentials or trailing slash, and not an IP literal.
fn exact_https_origin(value: &str) -> bool {
    let Ok(url) = url::Url::parse(value) else {
        return false;
    };
    url.scheme() == "https"
        && matches!(url.host(), Some(url::Host::Domain(_)))
        && url.origin().ascii_serialization() == value
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn settings(pairs: &[(&str, &str)]) -> Result<HostSettings, HostConfigError> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| ((*k).to_owned(), (*v).to_owned()))
            .collect();
        host_settings(|name| map.get(name).cloned())
    }

    const PRODUCTION: [(&str, &str); 3] = [
        ("CANTOS_ENV", "production"),
        ("CANTOS_BIND_ADDRESS", "0.0.0.0:8080"),
        ("CANTOS_WEB_ORIGIN", "https://studio.cantoss.duckdns.org"),
    ];

    fn production_with(extra: (&str, &str)) -> Result<HostSettings, HostConfigError> {
        let mut pairs = PRODUCTION.to_vec();
        pairs.retain(|(name, _)| *name != extra.0);
        pairs.push(extra);
        settings(&pairs)
    }

    #[test]
    fn mode_is_a_closed_exact_set() {
        for value in ["", "Production", "prod", "development ", "staging"] {
            assert_eq!(
                settings(&[("CANTOS_ENV", value)]),
                Err(HostConfigError::UnknownMode)
            );
        }
        assert_eq!(settings(&[]), Err(HostConfigError::UnknownMode));
    }

    #[test]
    fn development_keeps_the_local_contract() {
        let host = settings(&[("CANTOS_ENV", "development")]).unwrap();
        assert_eq!(host.mode(), HostMode::Development);
        assert_eq!(host.bind(), "127.0.0.1:8080".parse().unwrap());
        assert_eq!(host.web_origin(), "http://127.0.0.1:8080");
        assert_eq!(host.web_dist(), Some(DEVELOPMENT_DIST));
        assert!(host.development_sessions());
        assert_eq!(
            settings(&[
                ("CANTOS_ENV", "development"),
                ("CANTOS_BIND_ADDRESS", "0.0.0.0:8080")
            ]),
            Err(HostConfigError::DevelopmentBindNotLoopback)
        );
    }

    #[test]
    fn production_requires_explicit_bind_and_https_origin() {
        let host = settings(&PRODUCTION).unwrap();
        assert_eq!(host.mode(), HostMode::Production);
        assert_eq!(host.bind(), "0.0.0.0:8080".parse().unwrap());
        assert_eq!(host.web_origin(), "https://studio.cantoss.duckdns.org");
        assert_eq!(host.web_dist(), None);
        assert!(!host.development_sessions());
        assert_eq!(
            settings(&PRODUCTION[..1]),
            Err(HostConfigError::ProductionBindRequired)
        );
        assert_eq!(
            settings(&PRODUCTION[..2]),
            Err(HostConfigError::ProductionOriginRequired)
        );
        assert_eq!(
            production_with(("CANTOS_BIND_ADDRESS", "0.0.0.0:0")),
            Err(HostConfigError::InvalidBind)
        );
        assert_eq!(
            production_with(("CANTOS_BIND_ADDRESS", "api:8080")),
            Err(HostConfigError::InvalidBind)
        );
    }

    #[test]
    fn production_origin_must_be_the_exact_browser_origin() {
        for origin in [
            "http://studio.cantoss.duckdns.org",
            "https://studio.cantoss.duckdns.org/",
            "https://studio.cantoss.duckdns.org/app",
            "https://studio.cantoss.duckdns.org?x=1",
            "https://user@studio.cantoss.duckdns.org",
            "https://STUDIO.cantoss.duckdns.org",
            "https://127.0.0.1",
            "https://[::1]:8443",
            "studio.cantoss.duckdns.org",
        ] {
            assert_eq!(
                production_with(("CANTOS_WEB_ORIGIN", origin)),
                Err(HostConfigError::ProductionOriginNotHttps),
                "{origin}"
            );
        }
        assert!(production_with(("CANTOS_WEB_ORIGIN", "https://cantoss.duckdns.org:8443")).is_ok());
    }

    #[test]
    fn production_refuses_development_only_inputs() {
        assert_eq!(
            production_with(("CANTOS_WEB_DIST", "apps/cantos-studio/web/dist")),
            Err(HostConfigError::ProductionStaticFiles)
        );
        assert_eq!(
            production_with(("CANTOS_AUDIO_ROOT", "/var/lib/cantos/audio")),
            Err(HostConfigError::ProductionDevelopmentAudioRoot)
        );
        assert_eq!(
            production_with(("CANTOS_THEATRE_OIDC_CA_FILE", "/etc/cantos/ca.pem")),
            Err(HostConfigError::ProductionPrivateIssuerTrust)
        );
    }
}
