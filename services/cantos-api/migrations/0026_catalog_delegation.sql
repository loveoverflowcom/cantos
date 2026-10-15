-- Work-scoped delegation (ADR 0028). One Work owner grants a distinct registered delegate
-- actor closed Catalog actions on exactly one Work. Only a purpose-separated SHA-256 lookup of
-- a server-minted 256-bit secret is stored. Grants, revocations and lineage are append-only;
-- a new action needs a new column defaulting to false, so no old grant can widen.
CREATE TABLE catalog_delegation_grants (
    id text PRIMARY KEY CHECK(id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    work_id text NOT NULL,
    grantor_id text NOT NULL,
    -- The grantor is the Work owner at issuance; Catalog ownership itself is immutable.
    FOREIGN KEY(work_id,grantor_id) REFERENCES catalog_works(id,owner_id),
    delegate_id text NOT NULL REFERENCES actors(id),
    CHECK(delegate_id<>grantor_id),
    work_read boolean NOT NULL,
    content_read boolean NOT NULL,
    content_write boolean NOT NULL,
    CHECK(work_read OR content_read OR content_write),
    token_hash bytea NOT NULL UNIQUE CHECK(octet_length(token_hash)=32),
    issued_at bigint NOT NULL CHECK(issued_at>=0),
    expires_at bigint NOT NULL CHECK(expires_at>issued_at AND expires_at-issued_at<=2592000000),
    operation_id text NOT NULL CHECK(operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$'),
    request_digest text NOT NULL CHECK(request_digest ~ '^dg-request1:sha256:[a-f0-9]{64}$'),
    UNIQUE(grantor_id,operation_id),
    UNIQUE(id,delegate_id)
);
CREATE INDEX catalog_delegation_grants_work ON catalog_delegation_grants(work_id,id);
CREATE TABLE catalog_delegation_revocations (
    grant_id text PRIMARY KEY REFERENCES catalog_delegation_grants(id),
    revoked_by text NOT NULL REFERENCES actors(id),
    revoked_at bigint NOT NULL CHECK(revoked_at>=0)
);
-- Lineage of a delegated Catalog receipt: the recorded actor is the grant's delegate.
CREATE TABLE catalog_delegated_operations (
    grant_id text NOT NULL,
    actor_id text NOT NULL,
    operation_id text NOT NULL,
    PRIMARY KEY(actor_id,operation_id),
    FOREIGN KEY(actor_id,operation_id) REFERENCES catalog_operations(actor_id,operation_id),
    FOREIGN KEY(grant_id,actor_id) REFERENCES catalog_delegation_grants(id,delegate_id)
);
CREATE TRIGGER catalog_delegation_grants_immutable BEFORE UPDATE OR DELETE ON catalog_delegation_grants
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_delegation_grants_no_truncate BEFORE TRUNCATE ON catalog_delegation_grants
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_delegation_revocations_immutable BEFORE UPDATE OR DELETE ON catalog_delegation_revocations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_delegation_revocations_no_truncate BEFORE TRUNCATE ON catalog_delegation_revocations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_delegated_operations_immutable BEFORE UPDATE OR DELETE ON catalog_delegated_operations
    FOR EACH ROW EXECUTE FUNCTION reject_settled_change();
CREATE TRIGGER catalog_delegated_operations_no_truncate BEFORE TRUNCATE ON catalog_delegated_operations
    FOR EACH STATEMENT EXECUTE FUNCTION reject_settled_change();
REVOKE ALL ON catalog_delegation_grants,catalog_delegation_revocations,catalog_delegated_operations FROM PUBLIC;
