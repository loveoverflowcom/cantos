-- Candidate server.db version 1: the actor registry slice (#333.4) of the PostgreSQL epoch-2
-- tables `actors` and `actor_external_identities`. Every other server-owned table is pending.
-- The invariant mapping lives in docs/architecture/server-turso-schema.md.

CREATE TABLE actors (
    id TEXT PRIMARY KEY NOT NULL CHECK (coalesce(id REGEXP '^[a-z0-9_-]{1,64}$', 0)),
    active INTEGER NOT NULL DEFAULT 1 CHECK (active IN (0, 1))
) STRICT;
CREATE TRIGGER actors_id_immutable BEFORE UPDATE ON actors
WHEN NEW.id <> OLD.id
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.actor_id'); END;
CREATE TRIGGER actors_no_delete BEFORE DELETE ON actors
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.actor_id'); END;
-- REPLACE deletes a conflicting row without firing delete triggers, so an insert over an existing
-- key is refused before conflict resolution.
CREATE TRIGGER actors_no_replace BEFORE INSERT ON actors
WHEN EXISTS (SELECT 1 FROM actors WHERE id = NEW.id)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.actor_id'); END;

-- Exact byte keys (BINARY is PostgreSQL COLLATE "C"). The Unicode classes are those of the
-- PostgreSQL CHECKs; together they admit exactly what `ExternalIdentityKey::new` admits.
CREATE TABLE actor_external_identities (
    issuer TEXT NOT NULL COLLATE BINARY CHECK (
        octet_length(issuer) BETWEEN 1 AND 1024
        AND NOT coalesce(issuer REGEXP '^[\x{20}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]*$', 1)
        AND NOT coalesce(issuer REGEXP '[\x{0}-\x{1F}\x{7F}-\x{9F}]', 1)
    ),
    subject TEXT NOT NULL COLLATE BINARY CHECK (
        octet_length(subject) BETWEEN 1 AND 512
        AND NOT coalesce(subject REGEXP '^[\x{20}\x{A0}\x{1680}\x{2000}-\x{200A}\x{2028}\x{2029}\x{202F}\x{205F}\x{3000}]*$', 1)
        AND NOT coalesce(subject REGEXP '[\x{0}-\x{1F}\x{7F}-\x{9F}]', 1)
    ),
    actor_id TEXT NOT NULL REFERENCES actors(id),
    linked_at INTEGER NOT NULL CHECK (linked_at >= 0),
    PRIMARY KEY (issuer, subject)
) STRICT;
CREATE INDEX actor_external_identities_actor ON actor_external_identities(actor_id);
CREATE TRIGGER actor_external_identities_no_update BEFORE UPDATE ON actor_external_identities
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.actor_external_identities'); END;
CREATE TRIGGER actor_external_identities_no_delete BEFORE DELETE ON actor_external_identities
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.actor_external_identities'); END;
CREATE TRIGGER actor_external_identities_no_replace BEFORE INSERT ON actor_external_identities
WHEN EXISTS (SELECT 1 FROM actor_external_identities WHERE issuer = NEW.issuer AND subject = NEW.subject)
BEGIN SELECT RAISE(ABORT, 'cantos:immutable.actor_external_identities'); END;
