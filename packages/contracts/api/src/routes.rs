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

/// Catalog routes are only mounted in the current Studio space. Full templates are for
/// consumers/documentation; hosts strip `STUDIO_BASE` when nesting the Studio router.
pub const STUDIO_WORKS: &str = "/api/v1/studio/works";
pub const STUDIO_WORK: &str = "/api/v1/studio/works/{work_id}";
pub const STUDIO_WORK_HISTORY: &str = "/api/v1/studio/works/{work_id}/history";
pub const STUDIO_CONTENT_UNITS: &str = "/api/v1/studio/works/{work_id}/content-units";
pub const STUDIO_CONTENT_UNITS_REORDER: &str =
    "/api/v1/studio/works/{work_id}/content-units/reorder";
pub const STUDIO_CONTENT_UNIT: &str = "/api/v1/studio/content-units/{unit_id}";
