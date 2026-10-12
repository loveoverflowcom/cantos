//! URL spaces of the one Cantos API. Hosts and clients take their base paths from here so a
//! surface cannot be mounted or called under the wrong prefix.

/// Root of every versioned API path.
pub const API_BASE: &str = "/api/v1";
/// Creator surface: editing, casting, production and publication.
pub const STUDIO_BASE: &str = "/api/v1/studio";
/// Listener surface: catalog, playback, library and progress. Reserved; no endpoint yet.
pub const THEATRE_BASE: &str = "/api/v1/theatre";
/// Operations surface: system administration and permissions. Reserved; no endpoint yet.
pub const ADMIN_BASE: &str = "/api/v1/admin";
