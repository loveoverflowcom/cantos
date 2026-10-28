//! Trusted outer assembly for the HTTP host. No application/context imports this module.
//! Constructors remain source-compatible; every route graph receives one concrete bundle.
use axum::Router;
use std::sync::Arc;

use super::{router::routes as mount_routes, AppState};
use crate::contexts::{
    listening::{
        auth::{DenyListenerAuthentication, ListenerAuthenticator},
        LibraryService,
    },
    media::{
        ArtworkService, CoverDisclosureService, ListenerService, OriginPlayback, PlaybackDelivery,
        PublicationDelivery, PublicationService,
    },
};

pub(super) struct HttpServices {
    pub publication: PublicationService,
    pub listener: ListenerService,
    pub library: LibraryService,
    pub authenticator: Arc<dyn ListenerAuthenticator>,
    pub artwork: Option<ArtworkService>,
    /// Mounted with the private artwork routes: a decision names one of their admitted images.
    pub cover_disclosures: Option<CoverDisclosureService>,
}

struct Assembly<'a> {
    state: AppState,
    dist: Option<&'a str>,
    services: HttpServices,
    origins: Option<&'a crate::platform::host::SurfaceOrigins>,
}

impl<'a> Assembly<'a> {
    fn new(
        state: AppState,
        dist: Option<&'a str>,
        publication: Option<PublicationService>,
    ) -> Self {
        let store = &state.store;
        // Identity v1 sessions are admitted by the Theatre route middleware before a handler runs.
        // The port below only serves a trusted host override; by default nothing else mints a
        // listener principal.
        let authenticator: Arc<dyn ListenerAuthenticator> = Arc::new(DenyListenerAuthentication);
        let services = HttpServices {
            publication: publication.unwrap_or_else(|| PublicationService::new(store.clone())),
            listener: ListenerService::new(store.clone()),
            library: LibraryService::new(store.clone()),
            authenticator,
            artwork: store
                .audio_storage_is_enabled()
                .then(|| ArtworkService::new(store.clone())),
            cover_disclosures: None,
        };
        Self {
            state,
            dist,
            services,
            origins: None,
        }
    }

    fn build(mut self) -> Router {
        self.services.cover_disclosures = self
            .services
            .artwork
            .as_ref()
            .map(|_| CoverDisclosureService::new(self.state.store.clone()));
        // ServeDir follows nested symlinks. Private storage/artwork disables every static
        // fallback, including development and trusted capability overrides.
        let dist = self.dist.filter(|_| {
            !self.state.store.audio_storage_is_enabled() && self.services.artwork.is_none()
        });
        mount_routes(self.state, dist, self.services, self.origins)
    }
}

impl Assembly<'_> {
    /// Origin playback and cover delivery share the host's one explicit HTTPS origin. Covers are
    /// composed only when private media storage is configured; otherwise they stay `null`.
    fn compose_public_media(&mut self, origin: OriginPlayback) {
        let cover_origin = origin.origin().clone();
        self.services.listener = self
            .services
            .listener
            .clone()
            .with_origin_playback(origin)
            .with_cover_delivery(cover_origin, &self.state.store);
    }
}

pub fn router(state: AppState, dist: &str) -> Router {
    Assembly::new(state, Some(dist), None).build()
}

/// Production API host: Web apps have separate hosting; non-API paths are JSON not_found.
pub fn api_router(state: AppState) -> Router {
    Assembly::new(state, None, None).build()
}

/// Disposable test composition only; no executable or request can select the proposed policy.
#[cfg(test)]
pub(crate) fn metadata_cover_test_router(state: AppState, listener: ListenerService) -> Router {
    let mut assembly = Assembly::new(state, None, None);
    assembly.services.listener = listener;
    assembly.build()
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
    assembly.compose_public_media(origin);
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

/// Preserve this explicit private-artwork seam's denied listener identity. Ordinary hosts admit
/// Identity v1 sessions in the Theatre route middleware.
pub fn router_with_private_artwork(state: AppState, dist: &str, artwork: ArtworkService) -> Router {
    let mut assembly = Assembly::new(state, Some(dist), None);
    assembly.services.artwork = Some(artwork);
    assembly.services.authenticator = Arc::new(DenyListenerAuthentication);
    assembly.build()
}

/// Canonical executable composition: one application pool, one listener, route-local policies.
/// Public-media capabilities use the same assembly and cannot be selected by a request.
pub fn server_router(
    state: AppState,
    origins: &crate::platform::host::SurfaceOrigins,
    dist: Option<&str>,
    public_media: Option<(PublicationService, OriginPlayback)>,
) -> Router {
    let mut assembly = Assembly::new(state, dist, None);
    assembly.origins = Some(origins);
    if let Some((publication, playback)) = public_media {
        assembly.services.publication = publication;
        assembly.compose_public_media(playback);
        assembly.dist = None;
    }
    assembly.build()
}
