use super::core::{AuthError, CLIENT_ID};
use reqwest::Url;

/// Operator-owned exact configuration. No environment, headers or discovery can choose a client.
#[derive(Clone)]
pub struct OidcConfig {
    pub(super) issuer: String,
    pub(super) origin: Url,
    pub(super) web_origin: String,
    pub(super) redirect: String,
    pub(super) secret: String,
    pub(super) secure: bool,
}

impl OidcConfig {
    /// HTTPS with normal certificate validation; local HTTP is limited to literal loopback.
    /// This does not enable a production host. Cantos-server still requires development/loopback.
    pub fn new(issuer: &str, web_origin: &str, secret: String) -> Result<Self, AuthError> {
        let provider = checked_url(issuer)?;
        let web = checked_url(web_origin)?;
        if provider.path() != format!("/oauth2/openid/{CLIENT_ID}")
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
            issuer: issuer.into(),
            origin,
            web_origin: web_origin.into(),
            redirect: format!("{web_origin}/api/v1/studio/auth/callback"),
            secret,
            secure: web.scheme() == "https",
        })
    }

    pub(super) fn endpoint(&self, value: &str) -> Result<Url, AuthError> {
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
