use super::{core::AuthError, trust::IssuerTrust};
const STUDIO_CLIENT_ID: &str = "cantos-studio-web-bff";
pub const THEATRE_WEB_CLIENT_ID: &str = "cantos-theatre-web-bff";
pub const THEATRE_NATIVE_CLIENT_ID: &str = "cantos-theatre-native";
pub const THEATRE_NATIVE_REDIRECT: &str = "com.cantos.theatre:/oauth2/callback";
use reqwest::Url;

#[derive(Clone)]
pub(crate) enum ClientAuthentication {
    Confidential(String),
    PublicNative,
}

/// Operator-owned exact configuration. No environment, headers or discovery can choose a client.
#[derive(Clone)]
pub struct OidcConfig {
    pub(crate) client_id: &'static str,
    pub(crate) issuer: String,
    pub(crate) origin: Url,
    pub(crate) web_origin: String,
    pub(crate) redirect: String,
    pub(crate) authentication: ClientAuthentication,
    pub(crate) secure: bool,
    pub(crate) trust: Option<IssuerTrust>,
}

impl OidcConfig {
    pub(crate) fn theatre_native(issuer: &str) -> Result<Self, AuthError> {
        let provider = checked_url(issuer)?;
        if provider.path() != format!("/oauth2/openid/{THEATRE_NATIVE_CLIENT_ID}") {
            return Err(AuthError::Configuration);
        }
        let origin = provider.join("/").map_err(|_| AuthError::Configuration)?;
        Ok(Self {
            client_id: THEATRE_NATIVE_CLIENT_ID,
            issuer: issuer.into(),
            origin,
            web_origin: String::new(),
            redirect: THEATRE_NATIVE_REDIRECT.into(),
            authentication: ClientAuthentication::PublicNative,
            secure: true,
            trust: None,
        })
    }

    pub(crate) fn is_public_native(&self) -> bool {
        matches!(self.authentication, ClientAuthentication::PublicNative)
    }
    /// HTTPS with normal certificate validation; local HTTP is limited to literal loopback.
    /// The host mode (development or production) is decided separately in `platform::host`.
    pub fn new(issuer: &str, web_origin: &str, secret: String) -> Result<Self, AuthError> {
        Self::for_web(issuer, web_origin, secret, STUDIO_CLIENT_ID, "studio")
    }
    pub(crate) fn theatre_web(
        issuer: &str,
        web_origin: &str,
        secret: String,
    ) -> Result<Self, AuthError> {
        Self::for_web(issuer, web_origin, secret, THEATRE_WEB_CLIENT_ID, "theatre")
    }
    fn for_web(
        issuer: &str,
        web_origin: &str,
        secret: String,
        client_id: &'static str,
        surface: &str,
    ) -> Result<Self, AuthError> {
        let provider = checked_url(issuer)?;
        let web = checked_url(web_origin)?;
        if provider.path() != format!("/oauth2/openid/{client_id}")
            || !matches!(web.path(), "" | "/")
            || web_origin.ends_with('/')
            || secret.is_empty()
            || secret.len() > 4096
            || secret.chars().any(char::is_control)
        {
            return Err(AuthError::Configuration);
        }
        let origin = provider.join("/").map_err(|_| AuthError::Configuration)?;
        Ok(Self {
            client_id,
            issuer: issuer.into(),
            origin,
            web_origin: web_origin.into(),
            redirect: format!("{web_origin}/api/v1/{surface}/auth/callback"),
            authentication: ClientAuthentication::Confidential(secret),
            secure: web.scheme() == "https",
            trust: None,
        })
    }

    /// Private anchors only make sense for an HTTPS issuer; unused trust is a configuration error.
    pub fn with_trust(self, trust: IssuerTrust) -> Result<Self, AuthError> {
        if self.origin.scheme() != "https" {
            return Err(AuthError::Configuration);
        }
        Ok(Self {
            trust: Some(trust),
            ..self
        })
    }

    pub(crate) fn trust(&self) -> Option<&IssuerTrust> {
        self.trust.as_ref()
    }

    pub(crate) fn endpoint(&self, value: &str) -> Result<Url, AuthError> {
        let url = checked_url(value).map_err(|_| AuthError::InvalidMetadata)?;
        if url.origin() != self.origin.origin() {
            return Err(AuthError::InvalidMetadata);
        }
        Ok(url)
    }

    pub fn issuer(&self) -> &str {
        &self.issuer
    }
    pub fn redirect_uri(&self) -> &str {
        &self.redirect
    }
}

fn checked_url(value: &str) -> Result<Url, AuthError> {
    let url = Url::parse(value).map_err(|_| AuthError::Configuration)?;
    let local = matches!(url.host_str(), Some("127.0.0.1" | "[::1]"));
    if (url.scheme() != "https" && !(url.scheme() == "http" && local))
        || !url.username().is_empty()
        || url.password().is_some()
        || url.query().is_some()
        || url.fragment().is_some()
        || url.host_str().is_none()
    {
        return Err(AuthError::Configuration);
    }
    // A URL parser must not silently change the exact issuer spelling (host/path/dot segments).
    let normalized = url.as_str().trim_end_matches('/');
    if normalized != value.trim_end_matches('/') {
        return Err(AuthError::Configuration);
    }
    Ok(url)
}
