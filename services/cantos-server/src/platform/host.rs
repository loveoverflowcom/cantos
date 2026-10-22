//! Host mode decision for `cantos-server`: a pure function of the process environment.
//!
//! `development` keeps the historical local contract (loopback bind, derived HTTP origin,
//! development Studio tokens, optional static Studio files). `production` is the deployed
//! contract: an explicit bind, separate exact HTTPS origins for each Web surface,
//! no static serving, no development Studio credential and no development-only
//! storage, trust or mail fixtures. PostgreSQL stays local (loopback or a Unix socket) in both
//! modes; see [`crate::platform::store::local_config`].
use std::net::SocketAddr;

/// Trusted browser origins, kept separate even when two development apps share an origin.
/// Admin browser transport is disabled unless explicitly configured; it grants no capability.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SurfaceOrigins {
    studio: String,
    theatre: String,
    admin: Option<String>,
}

impl SurfaceOrigins {
    pub fn new(studio: &str, theatre: &str, admin: Option<&str>) -> Result<Self, HostConfigError> {
        for origin in [Some(studio), Some(theatre), admin].into_iter().flatten() {
            cantos_contracts::web_api::ApiOrigin::parse(origin)
                .map_err(|_| HostConfigError::InvalidSurfaceOrigin)?;
        }
        Ok(Self {
            studio: studio.into(),
            theatre: theatre.into(),
            admin: admin.map(str::to_owned),
        })
    }
    pub fn studio(&self) -> &str {
        &self.studio
    }
    pub fn theatre(&self) -> &str {
        &self.theatre
    }
    pub fn admin(&self) -> Option<&str> {
        self.admin.as_deref()
    }
}

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
    web_dist: Option<String>,
    origins: SurfaceOrigins,
}

impl HostSettings {
    pub fn mode(&self) -> HostMode {
        self.mode
    }
    pub fn bind(&self) -> SocketAddr {
        self.bind
    }
    pub fn web_origin(&self) -> &str {
        self.origins.studio()
    }
    pub fn origins(&self) -> &SurfaceOrigins {
        &self.origins
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
    ProductionOriginNotHttps,
    ProductionStaticFiles,
    ProductionDevelopmentAudioRoot,
    SurfaceOriginsRequired,
    InvalidSurfaceOrigin,
    LegacySurfaceConfiguration,
}

impl HostConfigError {
    pub fn message(self) -> &'static str {
        match self {
            Self::SurfaceOriginsRequired => "production requires CANTOS_STUDIO_WEB_ORIGIN and CANTOS_THEATRE_WEB_ORIGIN",
            Self::InvalidSurfaceOrigin => "each surface requires an exact browser origin (HTTPS, or development loopback HTTP)",
            Self::LegacySurfaceConfiguration => "unset CANTOS_BROWSER_API_SURFACE; production CANTOS_WEB_ORIGIN is replaced by per-surface origins",
            Self::UnknownMode => "CANTOS_ENV must be exactly development or production",
            Self::InvalidBind => "CANTOS_BIND_ADDRESS must be an IP:port socket address",
            Self::DevelopmentBindNotLoopback => "development host must bind loopback",
            Self::ProductionBindRequired => "production requires an explicit CANTOS_BIND_ADDRESS",
            Self::ProductionOriginNotHttps => {
                "production surface origins must be exact https origins without paths"
            }
            Self::ProductionStaticFiles => {
                "production serves no static files; unset CANTOS_WEB_DIST"
            }
            Self::ProductionDevelopmentAudioRoot => {
                "production refuses the development CANTOS_AUDIO_ROOT; use CANTOS_STORAGE_CONFIG"
            }
        }
    }
}

/// Inputs of the retired provider-based authentication paths (Kanidm/OIDC, the legacy native
/// public client, Firebase/SMTP registration). They configure nothing any more; a host that still
/// receives one refuses to start rather than letting an operator believe the path is active.
/// Identity v1 (`CANTOS_IDENTITY_*`) is the only session authority.
const RETIRED_PROVIDER_INPUTS: &[&str] = &[
    "CANTOS_STUDIO_OIDC_ISSUER",
    "CANTOS_STUDIO_OIDC_SECRET",
    "CANTOS_STUDIO_ADMIN_SUBJECTS",
    "CANTOS_THEATRE_OIDC_ISSUER",
    "CANTOS_THEATRE_OIDC_SECRET",
    "CANTOS_THEATRE_OIDC_CA_FILE",
    "CANTOS_THEATRE_NATIVE_ISSUER",
    "CANTOS_THEATRE_NATIVE_DEVELOPMENT_CALLBACK",
    "CANTOS_ADMIN_OIDC_ISSUER",
    "CANTOS_ADMIN_OIDC_SECRET",
];
const RETIRED_PROVIDER_PREFIXES: &[&str] = &["CANTOS_THEATRE_REGISTRATION_"];

/// The first name among `names` that configured a retired provider path, if any.
pub fn retired_provider_input<S: AsRef<str>>(names: impl IntoIterator<Item = S>) -> Option<String> {
    names.into_iter().find_map(|name| {
        let name = name.as_ref();
        (RETIRED_PROVIDER_INPUTS.contains(&name)
            || RETIRED_PROVIDER_PREFIXES
                .iter()
                .any(|prefix| name.starts_with(prefix)))
        .then(|| name.to_owned())
    })
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
    if lookup("CANTOS_BROWSER_API_SURFACE").is_some()
        || mode == HostMode::Production && lookup("CANTOS_WEB_ORIGIN").is_some()
    {
        return Err(HostConfigError::LegacySurfaceConfiguration);
    }
    match mode {
        HostMode::Development => {
            let bind: SocketAddr = lookup("CANTOS_BIND_ADDRESS")
                .unwrap_or_else(|| DEVELOPMENT_BIND.into())
                .parse()
                .map_err(|_| HostConfigError::InvalidBind)?;
            if !bind.ip().is_loopback() {
                return Err(HostConfigError::DevelopmentBindNotLoopback);
            }
            let default = lookup("CANTOS_WEB_ORIGIN").unwrap_or_else(|| format!("http://{bind}"));
            let origins = SurfaceOrigins::new(
                &lookup("CANTOS_STUDIO_WEB_ORIGIN").unwrap_or_else(|| default.clone()),
                &lookup("CANTOS_THEATRE_WEB_ORIGIN").unwrap_or(default),
                lookup("CANTOS_ADMIN_WEB_ORIGIN").as_deref(),
            )?;
            Ok(HostSettings {
                mode,
                bind,
                origins,
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
            let studio = lookup("CANTOS_STUDIO_WEB_ORIGIN")
                .ok_or(HostConfigError::SurfaceOriginsRequired)?;
            let theatre = lookup("CANTOS_THEATRE_WEB_ORIGIN")
                .ok_or(HostConfigError::SurfaceOriginsRequired)?;
            let admin = lookup("CANTOS_ADMIN_WEB_ORIGIN");
            if ![
                Some(studio.as_str()),
                Some(theatre.as_str()),
                admin.as_deref(),
            ]
            .into_iter()
            .flatten()
            .all(exact_https_origin)
            {
                return Err(HostConfigError::ProductionOriginNotHttps);
            }
            let origins = SurfaceOrigins::new(&studio, &theatre, admin.as_deref())?;
            if lookup("CANTOS_WEB_DIST").is_some() {
                return Err(HostConfigError::ProductionStaticFiles);
            }
            if lookup("CANTOS_AUDIO_ROOT").is_some() {
                return Err(HostConfigError::ProductionDevelopmentAudioRoot);
            }
            Ok(HostSettings {
                mode,
                bind,
                origins,
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

    const PRODUCTION: [(&str, &str); 4] = [
        ("CANTOS_ENV", "production"),
        ("CANTOS_BIND_ADDRESS", "0.0.0.0:8080"),
        (
            "CANTOS_STUDIO_WEB_ORIGIN",
            "https://studio.cantoss.duckdns.org",
        ),
        ("CANTOS_THEATRE_WEB_ORIGIN", "https://cantoss.duckdns.org"),
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
        assert_eq!(host.bind(), "127.0.0.1:8080".parse::<SocketAddr>().unwrap());
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
        assert_eq!(host.bind(), "0.0.0.0:8080".parse::<SocketAddr>().unwrap());
        assert_eq!(host.web_origin(), "https://studio.cantoss.duckdns.org");
        assert_eq!(host.web_dist(), None);
        assert!(!host.development_sessions());
        assert_eq!(
            settings(&PRODUCTION[..1]),
            Err(HostConfigError::ProductionBindRequired)
        );
        assert_eq!(
            settings(&PRODUCTION[..2]),
            Err(HostConfigError::SurfaceOriginsRequired)
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
                production_with(("CANTOS_STUDIO_WEB_ORIGIN", origin)),
                Err(HostConfigError::ProductionOriginNotHttps),
                "{origin}"
            );
        }
        assert!(production_with((
            "CANTOS_STUDIO_WEB_ORIGIN",
            "https://cantoss.duckdns.org:8443"
        ))
        .is_ok());
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
    }

    #[test]
    fn surface_configuration_is_exact_separate_and_admin_is_opt_in() {
        let host = settings(&PRODUCTION).unwrap();
        assert_eq!(
            host.origins().studio(),
            "https://studio.cantoss.duckdns.org"
        );
        assert_eq!(host.origins().theatre(), "https://cantoss.duckdns.org");
        assert_eq!(host.origins().admin(), None);
        assert_eq!(
            production_with(("CANTOS_ADMIN_WEB_ORIGIN", "https://admin.example.org"))
                .unwrap()
                .origins()
                .admin(),
            Some("https://admin.example.org")
        );
        for name in [
            "CANTOS_STUDIO_WEB_ORIGIN",
            "CANTOS_THEATRE_WEB_ORIGIN",
            "CANTOS_ADMIN_WEB_ORIGIN",
        ] {
            for value in [
                "*",
                "null",
                "https://example.org/",
                "https://EXAMPLE.org",
                "http://example.org",
                "https://user@example.org",
                "https://example.org:443",
            ] {
                assert_eq!(
                    production_with((name, value)),
                    Err(HostConfigError::ProductionOriginNotHttps),
                    "{name} {value}"
                );
            }
        }
        for name in ["CANTOS_BROWSER_API_SURFACE", "CANTOS_WEB_ORIGIN"] {
            assert_eq!(
                production_with((name, "studio")),
                Err(HostConfigError::LegacySurfaceConfiguration)
            );
        }
        assert_eq!(
            settings(&[
                ("CANTOS_ENV", "development"),
                ("CANTOS_THEATRE_WEB_ORIGIN", "null")
            ]),
            Err(HostConfigError::InvalidSurfaceOrigin)
        );
    }

    #[test]
    fn a_retired_provider_input_is_named_and_identity_inputs_are_not() {
        for name in [
            "CANTOS_STUDIO_OIDC_ISSUER",
            "CANTOS_STUDIO_ADMIN_SUBJECTS",
            "CANTOS_THEATRE_OIDC_CA_FILE",
            "CANTOS_THEATRE_NATIVE_ISSUER",
            "CANTOS_ADMIN_OIDC_SECRET",
            "CANTOS_THEATRE_REGISTRATION_SMTP_HOST",
            "CANTOS_THEATRE_REGISTRATION_ENABLED",
        ] {
            assert_eq!(
                retired_provider_input(["PATH", name, "CANTOS_ENV"]),
                Some(name.to_owned())
            );
        }
        assert_eq!(
            retired_provider_input([
                "CANTOS_ENV",
                "CANTOS_IDENTITY_SOCKET",
                "CANTOS_IDENTITY_STUDIO_SUBJECTS",
                "CANTOS_THEATRE_WEB_ORIGIN",
                "CANTOS_STUDIO_WEB_ORIGIN",
            ]),
            None
        );
    }
}
