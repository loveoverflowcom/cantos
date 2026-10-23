//! URL spaces of the one Cantos API as the Server mounts them, so a surface cannot be mounted
//! under the wrong prefix. Clients keep their own addressing (`cantos-web-auth::address`,
//! Studio Web's route templates) and are checked by their own tests.

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
pub const STUDIO_CONTENT_HEAD: &str = "/api/v1/studio/content-units/{unit_id}/content/head";
pub const STUDIO_CONTENT_REVISIONS: &str =
    "/api/v1/studio/content-units/{unit_id}/content/revisions";
pub const STUDIO_CONTENT_REVISION: &str =
    "/api/v1/studio/content-units/{unit_id}/content/revisions/{revision_id}";
pub const STUDIO_CONTENT_HISTORY: &str = "/api/v1/studio/content-units/{unit_id}/content/history";

/// Owner-only Work delegation management (ADR 0028), authenticated by the Studio session.
pub const STUDIO_WORK_DELEGATIONS: &str = "/api/v1/studio/works/{work_id}/delegations";
pub const STUDIO_WORK_DELEGATION: &str = "/api/v1/studio/works/{work_id}/delegations/{grant_id}";
/// Delegate surface: `Authorization: Bearer cantos_dlg_…` only, never a Studio cookie.
pub const STUDIO_DELEGATED_BASE: &str = "/api/v1/studio/delegated";
pub const STUDIO_DELEGATED_GRANT: &str = "/api/v1/studio/delegated/grant";
pub const STUDIO_DELEGATED_WORK: &str = "/api/v1/studio/delegated/works/{work_id}";
pub const STUDIO_DELEGATED_CONTENT_UNITS: &str =
    "/api/v1/studio/delegated/works/{work_id}/content-units";
pub const STUDIO_DELEGATED_CONTENT_UNIT: &str = "/api/v1/studio/delegated/content-units/{unit_id}";
pub const STUDIO_DELEGATED_CONTENT_HEAD: &str =
    "/api/v1/studio/delegated/content-units/{unit_id}/content/head";
pub const STUDIO_DELEGATED_CONTENT_REVISIONS: &str =
    "/api/v1/studio/delegated/content-units/{unit_id}/content/revisions";
pub const STUDIO_DELEGATED_CONTENT_REVISION: &str =
    "/api/v1/studio/delegated/content-units/{unit_id}/content/revisions/{revision_id}";
pub const STUDIO_DELEGATED_CONTENT_HISTORY: &str =
    "/api/v1/studio/delegated/content-units/{unit_id}/content/history";
