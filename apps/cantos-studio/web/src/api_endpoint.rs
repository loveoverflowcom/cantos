//! Build-time nonsecret API origin; unset keeps the local same-origin proxy.
use cantos_contracts::web_api::{api_url, AddressError, WebSurface};
pub(crate) fn endpoint(path: &str) -> Result<String, AddressError> {
    api_url(option_env!("CANTOS_API_ORIGIN"), WebSurface::Studio, path)
}

/// The configured nonsecret API origin; `None` keeps the same-origin proxy. Credentialed
/// transports validate their target against it.
pub(crate) fn api_origin() -> Option<&'static str> {
    option_env!("CANTOS_API_ORIGIN")
}

pub(crate) fn auth() -> Result<cantos_web_auth::browser::Client, cantos_web_auth::Failure> {
    cantos_web_auth::browser::client(cantos_web_auth::Surface::Studio, api_origin())
}
