//! Cantos Admin (operations and permissions) surface, mounted at `/api/v1/admin`.
//!
//! Reserved: no administration endpoint is implemented yet, so this router has no routes and
//! answers every request with `not_found`. When implemented it needs its own administrator
//! authentication and authorization (a Studio or Theatre session never grants it), DTOs in
//! `cantos_contracts::admin`, and an audit record for every mutation.
use axum::Router;

use super::{not_found, AppState};

pub(super) fn routes() -> Router<AppState> {
    Router::new().fallback(not_found)
}
