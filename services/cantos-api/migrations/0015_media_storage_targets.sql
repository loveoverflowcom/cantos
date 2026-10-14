-- Storage locations are private infrastructure metadata, outside immutable receipt/rendition
-- snapshots and their established digest schemes. Existing filesystem bytes are not moved.
-- Keep the default for compatibility with the preceding filesystem-only application version;
-- the current application always supplies the actual configured write target explicitly.
ALTER TABLE audio_intake_receipts
    ADD COLUMN storage_target_id text NOT NULL DEFAULT 'legacy-filesystem'
    CHECK (storage_target_id ~ '^[a-z][a-z0-9-]{0,62}$' AND storage_target_id !~ '-$');
ALTER TABLE audio_renditions
    ADD COLUMN storage_target_id text NOT NULL DEFAULT 'legacy-filesystem'
    CHECK (storage_target_id ~ '^[a-z][a-z0-9-]{0,62}$' AND storage_target_id !~ '-$');
