//! Route mounting/layers consume already assembled capabilities; no service construction.
use super::{
    admin, assembly::HttpServices, bearer, cors, errors::failure, identity_auth, policy, studio,
    theatre, theatre_auth, theatre_library, theatre_native_auth, AppState,
};
use axum::{extract::DefaultBodyLimit, http::StatusCode, middleware, response::Response, Router};
use cantos_contracts::{routes, ErrorCode};
use tower_http::services::ServeDir;

pub(super) fn routes(
    state: AppState,
    dist: Option<&str>,
    services: HttpServices,
    origins: Option<&crate::platform::host::SurfaceOrigins>,
) -> Router {
    let HttpServices {
        publication,
        listener,
        library,
        authenticator,
        artwork,
        theatre_web,
        theatre_native,
    } = services;
    // Cloning the application handle shares the one pool and every configured capability.
    // Only the route's trusted browser origin changes; handlers cannot choose another surface.
    let studio_state = AppState {
        web_origin: origins
            .map_or(state.web_origin.as_str(), |o| o.studio())
            .to_owned(),
        store: state.store.clone(),
    };
    let theatre_state = AppState {
        web_origin: origins
            .map_or(state.web_origin.as_str(), |o| o.theatre())
            .to_owned(),
        store: state.store.clone(),
    };
    let admin_state = AppState {
        web_origin: origins
            .map_or(Some(state.web_origin.as_str()), |o| o.admin())
            .unwrap_or("")
            .to_owned(),
        store: state.store.clone(),
    };
    let scope = |app: Router<AppState>, state: AppState, surface| {
        let app = app.layer(middleware::from_fn_with_state(
            policy::Boundary {
                state: state.clone(),
                surface,
            },
            policy::origin_and_cache,
        ));
        let app = if origins.is_some() {
            cors::boundary(
                app,
                (!state.web_origin.is_empty()).then_some(state.web_origin.as_str()),
                surface,
            )
        } else {
            app
        };
        app.with_state(state)
    };
    let studio = scope(
        studio::routes_with_artwork(
            studio_state.clone(),
            studio::Mount::Current,
            artwork.is_some(),
        )
        .fallback(not_found),
        studio_state.clone(),
        policy::Surface::Studio,
    );
    let theatre = scope(
        theatre::routes()
            .merge(theatre_auth::routes(theatre_web))
            .merge(theatre_native_auth::routes(theatre_native))
            .merge(bearer::routes(
                crate::platform::bearer::client::Client::TheatreWeb,
            ))
            .merge(bearer::routes(
                crate::platform::bearer::client::Client::TheatreNative,
            ))
            .merge(identity_auth::theatre_routes())
            .merge(theatre_library::routes())
            .layer(middleware::from_fn_with_state(
                theatre_state.clone(),
                theatre_auth::bff_identity,
            )),
        theatre_state,
        policy::Surface::Theatre,
    );
    let admin = scope(admin::routes(), admin_state, policy::Surface::Admin);
    // Deprecated pre-split Studio paths retain the same handlers and Studio boundary.
    let legacy = scope(
        studio::routes(studio_state.clone(), studio::Mount::Legacy).fallback(not_found),
        studio_state,
        policy::Surface::Studio,
    );
    let api = Router::new()
        .nest("/studio", studio)
        .nest("/theatre", theatre)
        .nest("/admin", admin)
        .merge(legacy)
        .layer(DefaultBodyLimit::max(8 * 1024 * 1024));
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
    host.layer(axum::Extension(publication))
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
