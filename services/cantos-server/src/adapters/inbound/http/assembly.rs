//! Trusted outer assembly for the HTTP host. No application/context imports this module.
//! Constructors remain source-compatible; every route graph receives one concrete bundle.
use axum::Router;
use std::sync::Arc;

use super::{router::routes as mount_routes, AppState};
use crate::contexts::{
    listening::{
        auth::{DenyListenerAuthentication, ListenerAuthenticator, TheatreListenerAuthenticator},
        LibraryService,
    },
    media::{
        ArtworkService, ListenerService, OriginPlayback, PlaybackDelivery, PublicationDelivery,
        PublicationService,
    },
};

pub(super) struct HttpServices {
    pub publication: PublicationService,
    pub listener: ListenerService,
    pub library: LibraryService,
    pub authenticator: Arc<dyn ListenerAuthenticator>,
    pub artwork: Option<ArtworkService>,
    pub theatre_web: bool,
    pub theatre_native: bool,
}

struct Assembly<'a> {
    state: AppState,
    dist: Option<&'a str>,
    services: HttpServices,
}

impl<'a> Assembly<'a> {
    fn new(
        state: AppState,
        dist: Option<&'a str>,
        publication: Option<PublicationService>,
    ) -> Self {
        let store = &state.store;
        let theatre_web = store.theatre_identity().is_some();
        let authenticator: Arc<dyn ListenerAuthenticator> = if theatre_web {
            Arc::new(TheatreListenerAuthenticator::new(store.clone()))
        } else {
            Arc::new(DenyListenerAuthentication)
        };
        let services = HttpServices {
            publication: publication.unwrap_or_else(|| PublicationService::new(store.clone())),
            listener: ListenerService::new(store.clone()),
            library: LibraryService::new(store.clone()),
            authenticator,
            artwork: store
                .audio_storage_is_enabled()
                .then(|| ArtworkService::new(store.clone())),
            theatre_web,
            theatre_native: store.theatre_native().is_some(),
        };
        Self {
            state,
            dist,
            services,
        }
    }

    fn build(self) -> Router {
        // ServeDir follows nested symlinks. Private storage/artwork disables every static
        // fallback, including development and trusted capability overrides.
        let dist = self.dist.filter(|_| {
            !self.state.store.audio_storage_is_enabled() && self.services.artwork.is_none()
        });
        mount_routes(self.state, dist, self.services)
    }
}

pub fn router(state: AppState, dist: &str) -> Router {
    Assembly::new(state, Some(dist), None).build()
}

/// Production API host: Web apps have separate hosting; non-API paths are JSON not_found.
pub fn api_router(state: AppState) -> Router {
    Assembly::new(state, None, None).build()
}

/// Trusted host override; requests cannot replace publication delivery.
pub fn router_with_publication_delivery(
    state: AppState,
    dist: &str,
    delivery: Arc<dyn PublicationDelivery>,
) -> Router {
    let mut assembly = Assembly::new(state, Some(dist), None);
    assembly.services.publication = assembly
        .services
        .publication
        .with_publication_delivery(delivery);
    assembly.build()
}

/// Trusted public-delivery override; the default host remains fail-closed.
pub fn router_with_theatre_delivery(
    state: AppState,
    dist: &str,
    delivery: Arc<dyn PlaybackDelivery>,
) -> Router {
    let mut assembly = Assembly::new(state, Some(dist), None);
    assembly.services.listener = assembly.services.listener.with_playback_delivery(delivery);
    assembly.build()
}

/// One value enables both grant issuance and the exact Theatre media reader (ADR 0032).
pub fn router_with_origin_playback(state: AppState, dist: &str, origin: OriginPlayback) -> Router {
    let mut assembly = Assembly::new(state, Some(dist), None);
    assembly.services.listener = assembly.services.listener.with_origin_playback(origin);
    assembly.build()
}

/// Publication eligibility and origin playback share the host's separately configured bytes.
/// A public-media host is API-only, including in development.
pub fn router_with_public_media(
    state: AppState,
    publication: PublicationService,
    origin: OriginPlayback,
) -> Router {
    let mut assembly = Assembly::new(state, None, Some(publication));
    assembly.services.listener = assembly.services.listener.with_origin_playback(origin);
    assembly.build()
}

/// Trusted host override; the supplied issuer adapter must mint an authenticated principal.
pub fn router_with_listener_identity(
    state: AppState,
    dist: &str,
    authenticator: Arc<dyn ListenerAuthenticator>,
) -> Router {
    let mut assembly = Assembly::new(state, Some(dist), None);
    assembly.services.authenticator = authenticator;
    assembly.build()
}

/// Preserve this explicit private-artwork seam's denied listener identity, even when the
/// store has Theatre identity configured. Ordinary hosts select identity from their store.
pub fn router_with_private_artwork(state: AppState, dist: &str, artwork: ArtworkService) -> Router {
    let mut assembly = Assembly::new(state, Some(dist), None);
    assembly.services.artwork = Some(artwork);
    assembly.services.authenticator = Arc::new(DenyListenerAuthentication);
    assembly.build()
}
