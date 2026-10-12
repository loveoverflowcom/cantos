//! Cantos Theatre (listener) surface, mounted at `/api/v1/theatre`.
//!
//! Reserved: no listener endpoint is implemented yet, so this router has no routes and answers
//! every request with `not_found`. It must not serve fake catalog, library or progress data.
//! When listener work arrives, add routes here with their own DTOs (`cantos_contracts::theatre`),
//! their own authentication and permission context, and public-delivery reads that only see
//! immutable publication manifests. Never reuse a Studio route, session or `Store` editor method.
use axum::Router;

use super::{not_found, AppState};

pub(super) fn routes() -> Router<AppState> {
    Router::new().fallback(not_found)
}
