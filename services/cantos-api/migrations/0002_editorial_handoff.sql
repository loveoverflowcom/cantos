-- Additive extension: retain migration 0001 and every accepted revision byte-for-byte.
CREATE TABLE source_records (
    owner_id text NOT NULL,
    kind text NOT NULL DEFAULT 'source' CHECK (kind = 'source'),
    id text NOT NULL,
    reference text NOT NULL CHECK (octet_length(reference) BETWEEN 1 AND 2048),
    original_text text NOT NULL CHECK (octet_length(original_text) BETWEEN 1 AND 1048576),
    sha256 text NOT NULL CHECK (sha256 ~ '^[a-f0-9]{64}$'),
    recorded_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (owner_id, id),
    FOREIGN KEY (owner_id, kind, id) REFERENCES script_evidence(owner_id, kind, id)
);
CREATE TABLE script_reviews (
    id text NOT NULL UNIQUE,
    script_id text NOT NULL,
    revision bigint NOT NULL,
    operation_id text NOT NULL,
    reviewed_by text NOT NULL REFERENCES actors(id),
    reviewed_at timestamptz NOT NULL DEFAULT CURRENT_TIMESTAMP,
    PRIMARY KEY (script_id, revision),
    FOREIGN KEY (script_id, revision) REFERENCES script_revisions(script_id, revision)
);
CREATE TABLE script_review_operations (
    script_id text NOT NULL,
    actor_id text NOT NULL REFERENCES actors(id),
    operation_id text NOT NULL,
    revision bigint NOT NULL,
    PRIMARY KEY (script_id, actor_id, operation_id),
    FOREIGN KEY (script_id, revision) REFERENCES script_reviews(script_id, revision)
);
CREATE TRIGGER sources_immutable BEFORE UPDATE OR DELETE ON source_records
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER sources_no_truncate BEFORE TRUNCATE ON source_records
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER reviews_immutable BEFORE UPDATE OR DELETE ON script_reviews
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER reviews_no_truncate BEFORE TRUNCATE ON script_reviews
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER review_operations_immutable BEFORE UPDATE OR DELETE ON script_review_operations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER review_operations_no_truncate BEFORE TRUNCATE ON script_review_operations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON source_records,script_reviews,script_review_operations FROM PUBLIC;
