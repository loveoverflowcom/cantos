//! Build-time nonsecret API origin; unset keeps the local same-origin proxy.
use cantos_contracts::web_api::{api_url, AddressError, WebSurface};
pub(crate) fn endpoint(path: &str) -> Result<String, AddressError> {
    api_url(option_env!("CANTOS_API_ORIGIN"), WebSurface::Studio, path)
}
