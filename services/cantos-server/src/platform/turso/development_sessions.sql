-- Server.db version 3: development Studio session tokens, the `development-only` disposition of
-- the PostgreSQL epoch-2 table `sessions`. A production host refuses every token here; the table
-- exists so a development/test composition can authenticate without Identity. Provider
-- (`studio_oidc`) rows are not ported. Deadlines are microseconds from the trusted clock.

CREATE TABLE development_sessions (
    token_hash BLOB PRIMARY KEY NOT NULL CHECK (length(token_hash) = 32),
    actor_id TEXT NOT NULL REFERENCES actors(id),
    expires_at INTEGER NOT NULL CHECK (expires_at >= 0),
    revoked INTEGER NOT NULL DEFAULT 0 CHECK (revoked IN (0, 1)),
    session_id TEXT NOT NULL UNIQUE CHECK (
        coalesce(session_id REGEXP '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$', 0)
    )
) STRICT;
CREATE INDEX development_sessions_actor ON development_sessions(actor_id);
-- A revoked token never becomes live again and a token never changes hands.
CREATE TRIGGER development_sessions_guard BEFORE UPDATE ON development_sessions
WHEN NEW.token_hash <> OLD.token_hash OR NEW.actor_id <> OLD.actor_id
    OR NEW.session_id <> OLD.session_id OR NEW.expires_at <> OLD.expires_at
    OR (OLD.revoked = 1 AND NEW.revoked = 0)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.development_sessions'); END;
CREATE TRIGGER development_sessions_no_delete BEFORE DELETE ON development_sessions
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.development_sessions'); END;
CREATE TRIGGER development_sessions_no_replace BEFORE INSERT ON development_sessions
WHEN EXISTS (SELECT 1 FROM development_sessions WHERE token_hash = NEW.token_hash OR session_id = NEW.session_id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.development_sessions'); END;
