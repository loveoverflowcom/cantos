//! Build-time nonsecret Theatre origin; absent selects the production API, invalid refuses.
use cantos_contracts::web_api::{api_url, AddressError, WebSurface};
pub(crate) fn endpoint(path: &str) -> Result<String, AddressError> {
    api_url(option_env!("CANTOS_API_ORIGIN"), WebSurface::Theatre, path)
}

#[cfg(target_arch = "wasm32")]
pub(crate) fn auth() -> Result<cantos_web_auth::browser::Client, cantos_web_auth::Failure> {
    cantos_web_auth::browser::client(
        cantos_web_auth::Surface::Theatre,
        option_env!("CANTOS_API_ORIGIN"),
    )
}

/// Configuration is checked before the shell starts network effects.
pub(crate) fn validate() -> Result<(), AddressError> {
    endpoint(cantos_contracts::routes::THEATRE_BASE).map(|_| ())
}
