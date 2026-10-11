-- Additive intake on the existing immutable source registry. Legacy rows are not rewritten.
-- Manuscript bytes are bounded local intake documents, not audio/media objects.
ALTER TABLE source_records ALTER COLUMN original_text DROP NOT NULL;
ALTER TABLE source_records
    ADD COLUMN original_bytes bytea,
    ADD COLUMN import_metadata jsonb,
    ADD COLUMN import_outcome jsonb,
    ADD COLUMN import_digest text,
    ADD COLUMN imported_by text REFERENCES actors(id),
    ADD COLUMN import_operation_id text,
    ADD CONSTRAINT source_original_exists CHECK (
        original_text IS NOT NULL OR original_bytes IS NOT NULL
    ),
    ADD CONSTRAINT imported_source_complete CHECK (
        (original_bytes IS NULL AND import_metadata IS NULL AND import_outcome IS NULL AND import_digest IS NULL
            AND imported_by IS NULL AND import_operation_id IS NULL)
        OR
        (original_bytes IS NOT NULL AND import_metadata IS NOT NULL AND import_outcome IS NOT NULL
            AND imported_by IS NOT NULL AND import_operation_id IS NOT NULL
            AND import_digest IS NOT NULL AND import_digest ~ '^[a-f0-9]{64}$'
            AND imported_by = owner_id
            AND octet_length(original_bytes) <= 1048576
            AND jsonb_typeof(import_metadata) = 'object'
            AND jsonb_typeof(import_outcome) = 'object'
            AND import_operation_id ~ '^[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}$')
    );
CREATE UNIQUE INDEX source_import_operations
    ON source_records(imported_by, import_operation_id)
    WHERE import_operation_id IS NOT NULL;
