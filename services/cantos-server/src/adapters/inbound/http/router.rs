//! Route mounting/layers consume already assembled capabilities; no service construction.
use super::{
    admin, assembly::HttpServices, bearer, errors::failure, policy, studio, theatre, theatre_auth,
    theatre_library, theatre_native_auth, AppState,
};
use axum::{extract::DefaultBodyLimit, http::StatusCode, middleware, response::Response, Router};
use cantos_contracts::{routes, ErrorCode};
use tower_http::services::ServeDir;

pub(super) fn routes(state: AppState, dist: Option<&str>, services: HttpServices) -> Router {
    let HttpServices {
        publication,
        listener,
        library,
        authenticator,
        artwork,
        theatre_web,
        theatre_native,
    } = services;
    // Surfaces are separate route groups with their own permission context; shared code is the
    // middleware below and the domain/store modules, never a handler.
    let api = Router::new()
        .nest(
            "/studio",
            studio::routes_with_artwork(state.clone(), studio::Mount::Current, artwork.is_some())
                .fallback(not_found),
        )
        .nest(
            "/theatre",
            theatre::routes()
                .merge(theatre_auth::routes(theatre_web))
                .merge(theatre_native_auth::routes(theatre_native))
                .merge(bearer::routes(
                    crate::platform::bearer::client::Client::TheatreWeb,
                ))
                .merge(bearer::routes(
                    crate::platform::bearer::client::Client::TheatreNative,
                ))
                .merge(theatre_library::routes())
                .layer(middleware::from_fn_with_state(
                    state.clone(),
                    theatre_auth::bff_identity,
                )),
        )
        .nest("/admin", admin::routes())
        // Deprecated pre-split Studio paths (`/api/v1/scripts/...`); same handlers as `/studio`.
        .merge(studio::routes(state.clone(), studio::Mount::Legacy))
        .fallback(not_found)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            policy::origin_and_cache,
        ));
    let host = Router::new().nest(routes::API_BASE, api);
    let host = match dist {
        Some(dist) => host.fallback_service(ServeDir::new(dist)),
        _ => host.fallback(not_found),
    };
    let host = if let Some(artwork) = artwork {
        host.layer(axum::Extension(artwork))
    } else {
        host
    };
    host.with_state(state)
        .layer(axum::Extension(publication))
        .layer(axum::Extension(listener))
        .layer(axum::Extension(library))
        .layer(axum::Extension(authenticator))
}

pub(super) async fn not_found() -> Response {
    failure(ErrorCode::NotFound, StatusCode::NOT_FOUND)
}

#[cfg(test)]
pub(crate) fn playback_contract_router(
    listener: crate::contexts::media::ListenerService,
) -> Router {
    theatre::playback_contract_routes().layer(axum::extract::Extension(listener))
}
