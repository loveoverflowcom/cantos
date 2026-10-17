//! Nonsecret browser-to-API addressing shared by the two Web clients (ADR 0041).
//! This selects a destination, never an identity provider or an authorization capability.
use url::Url;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WebSurface {
    Studio,
    Theatre,
}
impl WebSurface {
    pub fn base(self) -> &'static str {
        match self {
            Self::Studio => crate::routes::STUDIO_BASE,
            Self::Theatre => crate::routes::THEATRE_BASE,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ApiOrigin(String);
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AddressError {
    InvalidOrigin,
    WrongSurface,
}
impl ApiOrigin {
    pub fn parse(value: &str) -> Result<Self, AddressError> {
        let url = Url::parse(value).map_err(|_| AddressError::InvalidOrigin)?;
        let local = matches!(url.host_str(), Some("localhost" | "127.0.0.1" | "[::1]"));
        if !(url.scheme() == "https" || url.scheme() == "http" && local)
            || url.origin().ascii_serialization() != value
            || url.host_str().is_none()
        {
            return Err(AddressError::InvalidOrigin);
        }
        Ok(Self(value.to_owned()))
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Absent configuration retains the local Trunk proxy. Invalid configuration never falls back.
pub fn api_url(
    configured_origin: Option<&str>,
    surface: WebSurface,
    path: &str,
) -> Result<String, AddressError> {
    let base = surface.base();
    let route = path.split('?').next().unwrap_or(path);
    if !(route == base || route.starts_with(&format!("{base}/")))
        || route.contains('\\')
        || route.contains('%')
        || route.split('/').any(|part| matches!(part, "." | ".."))
        || path.contains('#')
        || path.chars().any(char::is_control)
    {
        return Err(AddressError::WrongSurface);
    }
    let origin = configured_origin.map(ApiOrigin::parse).transpose()?;
    Ok(format!(
        "{}{path}",
        origin.as_ref().map_or("", ApiOrigin::as_str)
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn configured_api_routes_keep_the_surface_and_query_and_local_proxy() {
        let path = "/api/v1/theatre/works?limit=20&query=Long%20V%C3%A2n";
        assert_eq!(api_url(None, WebSurface::Theatre, path), Ok(path.into()));
        assert_eq!(
            api_url(Some("https://api.example.org"), WebSurface::Theatre, path),
            Ok(format!("https://api.example.org{path}"))
        );
        assert_eq!(
            api_url(
                Some("http://127.0.0.1:8080"),
                WebSurface::Studio,
                "/api/v1/studio/session"
            ),
            Ok("http://127.0.0.1:8080/api/v1/studio/session".into())
        );
    }
    #[test]
    fn origins_are_exact_and_never_contain_credentials_or_endpoint_paths() {
        for origin in [
            "",
            "https://api.example.org/",
            "https://API.example.org",
            "https://api.example.org:443",
            "http://api.example.org",
            "https://user:secret@api.example.org",
            "https://api.example.org/path",
            "https://api.example.org?x=1",
            "https://api.example.org#fragment",
            " https://api.example.org",
            "https://api.example.org\n",
        ] {
            assert_eq!(
                ApiOrigin::parse(origin),
                Err(AddressError::InvalidOrigin),
                "{origin:?}"
            );
        }
    }
    #[test]
    fn a_surface_cannot_resolve_another_surface_or_escape_to_a_provider() {
        for path in [
            "//evil.example/api/v1/studio/session",
            "https://idm.example/v1/auth",
            "/api/v1/theatre/session",
            "/api/v1/studioevil/session",
            "/api/v1/studio/../theatre/session",
            "/api/v1/studio/%2e%2e/theatre",
            "/api/v1/studio/session#fragment",
        ] {
            assert_eq!(
                api_url(None, WebSurface::Studio, path),
                Err(AddressError::WrongSurface),
                "{path}"
            );
        }
        assert_eq!(
            api_url(
                Some("invalid"),
                WebSurface::Studio,
                "/api/v1/studio/session"
            ),
            Err(AddressError::InvalidOrigin)
        );
    }
}
