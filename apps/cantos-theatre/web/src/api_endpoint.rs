//! Build-time nonsecret API origin; unset keeps the local same-origin proxy.
use cantos_contracts::web_api::{api_url, AddressError, WebSurface};
pub(crate) fn endpoint(path: &str) -> Result<String, AddressError> {
    api_url(option_env!("CANTOS_API_ORIGIN"), WebSurface::Theatre, path)
}

pub(crate) fn auth() -> Result<cantos_web_auth::browser::Client, cantos_web_auth::Failure> {
    cantos_web_auth::browser::client(
        cantos_web_auth::Surface::Theatre,
        option_env!("CANTOS_API_ORIGIN"),
    )
}
