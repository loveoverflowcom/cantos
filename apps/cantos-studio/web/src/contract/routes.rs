//! Studio Web's own copy of the Studio route templates it calls. The Server owns the mounted
//! routes (its `contract::routes`); the Studio route tests keep these templates honest.

/// Creator surface: editing, casting, production and publication.
pub const STUDIO_BASE: &str = "/api/v1/studio";

/// Catalog routes are only mounted in the current Studio space. Full templates are for
/// consumers/documentation; hosts strip `STUDIO_BASE` when nesting the Studio router.
pub const STUDIO_WORKS: &str = "/api/v1/studio/works";
/// Read of one exact saved Work revision as Theatre presentation text (ADR 0054 D03).
pub const STUDIO_WORK_PREVIEW: &str = "/api/v1/studio/works/{work_id}/preview";

/// Owner-only Work delegation management (ADR 0028), authenticated by the Studio session.
pub const STUDIO_WORK_DELEGATIONS: &str = "/api/v1/studio/works/{work_id}/delegations";
pub const STUDIO_WORK_DELEGATION: &str = "/api/v1/studio/works/{work_id}/delegations/{grant_id}";
