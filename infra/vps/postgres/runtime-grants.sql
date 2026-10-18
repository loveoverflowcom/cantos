-- Least-privilege runtime grants for `cantos_app`, applied by `cantos-vps deploy` as
-- `cantos_owner` after every current-schema migration run. Every statement is idempotent.
--
-- Source of truth: each statement is the union of the narrow grants that the PostgreSQL
-- integration tests give the same role for the route groups this deployment enables
-- (services/cantos-api/tests/*_postgres.rs and the services/kanidm operator contract).
-- Deliberately absent: DDL, TRUNCATE, actor/registry writes, retired TTS tables,
-- Admin functions/tables (Admin stays unconfigured), Theatre native
-- identity, Theatre registration (SMTP mode has no block). Firebase enrollment has its own
-- optional block, theatre-registration-grants.sql, applied only when registration is enabled
-- (cantos-vps apply_grants); a block for any other feature must be reviewed before it is set.
-- A new migration that adds a table read or written at runtime must extend this file.

GRANT USAGE ON SCHEMA public TO cantos_app;

-- Startup checks the schema epoch and migration checksums without owner privileges.
GRANT SELECT ON cantos_schema, cantos_migrations TO cantos_app;

-- Editorial: scripts, revisions, evidence, reviews, imports.
GRANT SELECT ON actors, sessions, scripts, script_members, script_evidence, script_revisions,
    revision_evidence, source_records, script_reviews, script_review_operations TO cantos_app;
GRANT INSERT ON scripts, script_revisions, revision_evidence, script_evidence, source_records,
    script_reviews, script_review_operations TO cantos_app;
GRANT UPDATE (head_revision) ON scripts TO cantos_app;
-- Studio sessions: development rows are inert on a production host; OIDC rows are revocable.
GRANT UPDATE ON sessions TO cantos_app;

-- Adaptation proposals supplied by external callers.
GRANT SELECT, INSERT ON adaptation_runs, adaptation_proposals, adaptation_cancellations, adaptation_acceptances,
    adaptation_submissions TO cantos_app;
GRANT UPDATE (status, updated_at) ON adaptation_runs TO cantos_app;

-- Production planning inputs and briefs.
GRANT SELECT, INSERT ON production_settings, production_rights_claims, production_snapshots,
    production_approvals TO cantos_app;
GRANT SELECT, INSERT ON production_briefs, production_brief_heads, production_producer_grants,
    production_producer_revocations TO cantos_app;
GRANT UPDATE (brief_id, version) ON production_brief_heads TO cantos_app;
GRANT EXECUTE ON FUNCTION production_lock_actor(text), production_lock_member(text, text) TO cantos_app;

-- Catalog, content identity and Work-scoped delegation.
GRANT SELECT ON catalog_clock, catalog_works, catalog_work_revisions, catalog_units,
    catalog_unit_revisions, catalog_members, catalog_operations, catalog_content_revisions,
    catalog_content_heads TO cantos_app;
GRANT INSERT ON catalog_works, catalog_work_revisions, catalog_units, catalog_unit_revisions,
    catalog_members, catalog_operations, catalog_content_revisions, catalog_content_heads TO cantos_app;
GRANT UPDATE (revision) ON catalog_clock TO cantos_app;
GRANT UPDATE (head_revision, lifecycle) ON catalog_works TO cantos_app;
GRANT UPDATE (head_revision, position, lifecycle) ON catalog_units TO cantos_app;
GRANT UPDATE (revision_id, revision) ON catalog_content_heads TO cantos_app;
GRANT UPDATE (role), DELETE ON catalog_members TO cantos_app;
GRANT EXECUTE ON FUNCTION catalog_lock_actor(text), catalog_lock_member(text, text) TO cantos_app;
GRANT SELECT, INSERT ON catalog_delegation_grants, catalog_delegation_revocations,
    catalog_delegated_operations TO cantos_app;

-- Private audio intake, lifecycle, publication and the public listener projection.
GRANT SELECT, INSERT ON audio_upload_intents, audio_intake_receipts TO cantos_app;
GRANT SELECT, INSERT ON audio_submissions, audio_submission_revocations, audio_renditions,
    audio_reviews, audio_release_candidates, audio_distribution_clearances,
    audio_publication_approvals, audio_releases, audio_release_withdrawals,
    audio_publication_operations, audio_publication_events, audio_publication_pointers,
    audio_listener_works, audio_listener_units, audio_listener_releases TO cantos_app;
GRANT UPDATE (active_release_id, version) ON audio_publication_pointers TO cantos_app;
GRANT EXECUTE ON FUNCTION catalog_listener_discoverable(text, text), listener_actor_active(text) TO cantos_app;

-- Private Work artwork.
GRANT SELECT, INSERT ON media_image_intents, media_image_assets, media_work_artwork_changes,
    media_artwork_operations TO cantos_app;

-- Studio OIDC BFF (services/kanidm README, migrations 0020/0025).
GRANT SELECT ON studio_oidc_sessions TO cantos_app;
GRANT SELECT, INSERT, UPDATE ON studio_oidc_flows TO cantos_app;
GRANT UPDATE (revocation_pending, revoked_by_token_hash) ON studio_oidc_sessions TO cantos_app;
GRANT EXECUTE ON FUNCTION
    studio_oidc_finish(bytea, text, text, text, text, bytea, bytea, text, bigint) TO cantos_app;

-- Theatre Web identity (migration 0021) and the listener Library/Progress it unlocks.
GRANT SELECT ON theatre_identity_sessions TO cantos_app;
GRANT SELECT, INSERT, UPDATE ON theatre_identity_flows TO cantos_app;
GRANT UPDATE (revoked, revocation_pending, revoked_by_token_hash) ON theatre_identity_sessions TO cantos_app;
GRANT EXECUTE ON FUNCTION
    theatre_identity_finish(bytea, text, text, text, text, bytea, bytea, text, bigint),
    listener_lock_actor(text) TO cantos_app;
GRANT SELECT, INSERT ON listener_state, listener_saved, listener_progress, listener_history,
    listener_playback_sessions, listener_device_sequences, listener_operations TO cantos_app;
GRANT UPDATE (revision, history_revision, history_epoch) ON listener_state TO cantos_app;
GRANT UPDATE (saved, revision, listener_order, accepted_at, snapshot, snapshot_digest)
    ON listener_saved TO cantos_app;
GRANT UPDATE (position_ms, completion, revision, listener_order, accepted_at, snapshot,
    snapshot_digest) ON listener_progress TO cantos_app;
GRANT UPDATE (publication_id, unit_id, listener_order, started_at, snapshot, snapshot_digest),
    DELETE ON listener_history TO cantos_app;
GRANT UPDATE (sequence) ON listener_device_sequences TO cantos_app;

-- Cantos-issued surface-scoped Bearer families. No direct registry write privilege.
GRANT SELECT, INSERT ON bearer_flows, bearer_families, bearer_tokens TO cantos_app;
GRANT UPDATE (consumed, cancelled, completed, handoff_hash, subject, proof_hash, provider_access,
    authority_expires, authenticated_at, verifier, nonce) ON bearer_flows TO cantos_app;
GRANT UPDATE (revoked, revocation_pending, idle_expires) ON bearer_families TO cantos_app;
GRANT UPDATE (used) ON bearer_tokens TO cantos_app;
GRANT EXECUTE ON FUNCTION bearer_register(uuid,text,text,text) TO cantos_app;
