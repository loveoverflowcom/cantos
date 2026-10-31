//! Axum inbound adapter. Assembly owns concrete wiring; surface modules own transport.
mod admin;
mod assembly;
pub(crate) mod audio_transport;
mod cookies;
mod cors;
mod errors;
mod identity_auth;
mod policy;
mod router;
mod state;
mod studio;
mod theatre;
mod theatre_library;
mod viewer_access;
mod wire;

#[cfg(test)]
pub(crate) use assembly::metadata_cover_test_router;
pub use assembly::{
    api_router, router, router_with_listener_identity, router_with_origin_playback,
    router_with_private_artwork, router_with_public_media, router_with_publication_delivery,
    router_with_theatre_delivery, server_router,
};
pub use cors::{with_browser_cors, WebSurface};
pub use state::AppState;

#[cfg(test)]
pub(crate) use router::playback_contract_router;
#[cfg(test)]
mod tests;
