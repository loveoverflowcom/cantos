-- Candidate server.db version 2: the provider-free Identity sessions (#335) of the PostgreSQL
-- epoch-2 tables `identity_sessions` and `identity_session_tokens` (0004_identity_sessions.sql).
-- Deadlines are Unix seconds, the unit of `SessionPolicy`. The invariant mapping lives in
-- docs/architecture/server-turso-schema.md.

CREATE TABLE identity_sessions (
    id TEXT PRIMARY KEY NOT NULL CHECK (
        coalesce(id REGEXP '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', 0)
    ),
    actor_id TEXT NOT NULL REFERENCES actors(id),
    -- Carries the surface; a Web cookie never doubles as a native Bearer and Studio is Web-only.
    channel TEXT NOT NULL CHECK (channel IN ('theatre-web', 'studio-web', 'theatre-native')),
    issuer TEXT NOT NULL COLLATE BINARY CHECK (issuer = 'cantos-identity'),
    subject TEXT NOT NULL COLLATE BINARY CHECK (
        coalesce(subject REGEXP '^[0-9a-f]{32}$', 0)
        AND subject <> '00000000000000000000000000000000'
    ),
    assurance TEXT NOT NULL CHECK (assurance = 'password'),
    authenticated_at INTEGER NOT NULL CHECK (authenticated_at > 0),
    -- What Identity reported when the session was issued; admission requires them unchanged.
    observed_security_version INTEGER NOT NULL CHECK (observed_security_version > 0),
    observed_instance_epoch BLOB NOT NULL CHECK (
        length(observed_instance_epoch) = 16
        AND observed_instance_epoch <> x'00000000000000000000000000000000'
    ),
    absolute_expires INTEGER NOT NULL CHECK (absolute_expires >= 0),
    idle_expires INTEGER NOT NULL CHECK (idle_expires >= 0 AND idle_expires <= absolute_expires),
    revoked INTEGER NOT NULL DEFAULT 0 CHECK (revoked IN (0, 1)),
    revoked_reason TEXT CHECK (revoked_reason IN (
        'logout', 'replay', 'subject-mismatch', 'subject-missing', 'disabled',
        'epoch-changed', 'version-advanced', 'version-regressed'
    )),
    CHECK (revoked = (revoked_reason IS NOT NULL)),
    FOREIGN KEY (issuer, subject) REFERENCES actor_external_identities(issuer, subject)
) STRICT;
CREATE INDEX identity_sessions_subject ON identity_sessions(subject) WHERE revoked = 0;
CREATE INDEX identity_sessions_actor ON identity_sessions(actor_id);
-- Added over PostgreSQL: the session's actor is the one its exact issuer/subject is bound to.
CREATE TRIGGER identity_sessions_bound_actor BEFORE INSERT ON identity_sessions
WHEN NOT EXISTS (
    SELECT 1 FROM actor_external_identities
    WHERE issuer = NEW.issuer AND subject = NEW.subject AND actor_id = NEW.actor_id
)
BEGIN SELECT RAISE(ABORT, 'cantos:identity_sessions.binding'); END;
-- REPLACE deletes a conflicting row without firing delete triggers, so an insert over an existing
-- key is refused before conflict resolution.
CREATE TRIGGER identity_sessions_no_replace BEFORE INSERT ON identity_sessions
WHEN EXISTS (SELECT 1 FROM identity_sessions WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_sessions'); END;
CREATE TRIGGER identity_sessions_no_delete BEFORE DELETE ON identity_sessions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_sessions'); END;
-- Revocation is permanent: never cleared, re-reasoned or extended.
CREATE TRIGGER identity_sessions_revocation_permanent BEFORE UPDATE ON identity_sessions
WHEN OLD.revoked = 1 AND (
    NEW.revoked <> 1 OR NEW.revoked_reason IS NOT OLD.revoked_reason
    OR NEW.idle_expires <> OLD.idle_expires
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_sessions.revocation'); END;
-- The binding, authentication time, observed security and absolute deadline never move; the idle
-- deadline never decreases.
CREATE TRIGGER identity_sessions_binding_immutable BEFORE UPDATE ON identity_sessions
WHEN NEW.id IS NOT OLD.id OR NEW.actor_id IS NOT OLD.actor_id OR NEW.channel IS NOT OLD.channel
    OR NEW.issuer IS NOT OLD.issuer OR NEW.subject IS NOT OLD.subject
    OR NEW.assurance IS NOT OLD.assurance OR NEW.authenticated_at IS NOT OLD.authenticated_at
    OR NEW.observed_security_version IS NOT OLD.observed_security_version
    OR NEW.observed_instance_epoch IS NOT OLD.observed_instance_epoch
    OR NEW.absolute_expires IS NOT OLD.absolute_expires
    OR NEW.idle_expires < OLD.idle_expires
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_sessions'); END;

-- Only SHA-256 digests of random 256-bit secrets are stored. A Web cookie has no refresh digest.
CREATE TABLE identity_session_tokens (
    access_hash BLOB PRIMARY KEY NOT NULL CHECK (length(access_hash) = 32),
    refresh_hash BLOB UNIQUE CHECK (refresh_hash IS NULL OR length(refresh_hash) = 32),
    session_id TEXT NOT NULL REFERENCES identity_sessions(id),
    expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
    used INTEGER NOT NULL DEFAULT 0 CHECK (used IN (0, 1)),
    CHECK (used = 0 OR refresh_hash IS NOT NULL)
) STRICT;
CREATE INDEX identity_session_tokens_session ON identity_session_tokens(session_id);
CREATE TRIGGER identity_session_tokens_no_replace BEFORE INSERT ON identity_session_tokens
WHEN EXISTS (
    SELECT 1 FROM identity_session_tokens
    WHERE access_hash = NEW.access_hash
        OR (NEW.refresh_hash IS NOT NULL AND refresh_hash = NEW.refresh_hash)
)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_session_tokens'); END;
-- Added over PostgreSQL: a new secret belongs to a live session of its own channel shape (native
-- pairs carry a refresh digest, Web cookies never do) and starts unspent.
CREATE TRIGGER identity_session_tokens_issue BEFORE INSERT ON identity_session_tokens
WHEN NEW.used <> 0 OR NOT EXISTS (
    SELECT 1 FROM identity_sessions
    WHERE id = NEW.session_id AND revoked = 0
        AND (channel = 'theatre-native') = (NEW.refresh_hash IS NOT NULL)
)
BEGIN SELECT RAISE(ABORT, 'cantos:identity_session_tokens.issue'); END;
CREATE TRIGGER identity_session_tokens_no_delete BEFORE DELETE ON identity_session_tokens
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_session_tokens'); END;
-- Consumption is permanent, a secret never moves to another session and its expiry never shrinks.
CREATE TRIGGER identity_session_tokens_immutable BEFORE UPDATE ON identity_session_tokens
WHEN NEW.access_hash IS NOT OLD.access_hash OR NEW.refresh_hash IS NOT OLD.refresh_hash
    OR NEW.session_id IS NOT OLD.session_id OR (OLD.used = 1 AND NEW.used <> 1)
    OR NEW.expires_at < OLD.expires_at
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.identity_session_tokens'); END;
