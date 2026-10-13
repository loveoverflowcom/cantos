-- Additive registry: existing actor IDs, sessions, ownership and history are unchanged.
-- 0015/0016/0017 are reserved to parallel storage/publication/discovery PRs.
CREATE TABLE actor_external_identities (
    issuer text COLLATE "C" NOT NULL CHECK (
        octet_length(issuer) BETWEEN 1 AND 1024
        AND issuer !~ U&'^[\0020\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*$'
        AND issuer !~ U&'[\0001-\001F\007F-\009F]'
    ),
    subject text COLLATE "C" NOT NULL CHECK (
        octet_length(subject) BETWEEN 1 AND 512
        AND subject !~ U&'^[\0020\00A0\1680\2000-\200A\2028\2029\202F\205F\3000]*$'
        AND subject !~ U&'[\0001-\001F\007F-\009F]'
    ),
    actor_id text NOT NULL REFERENCES actors(id),
    linked_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (issuer, subject)
);
CREATE INDEX actor_external_identities_actor ON actor_external_identities(actor_id);
CREATE TRIGGER actor_external_identities_immutable BEFORE UPDATE OR DELETE ON actor_external_identities
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER actor_external_identities_no_truncate BEFORE TRUNCATE ON actor_external_identities
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON actor_external_identities FROM PUBLIC;
-- No grant to the HTTP role. Only an explicitly authorized operator can currently write.
