CREATE TABLE identity_restore_epochs (
    operation_id BLOB PRIMARY KEY NOT NULL CHECK (length(operation_id) = 16 AND operation_id <> x'00000000000000000000000000000000'),
    previous_epoch BLOB NOT NULL UNIQUE CHECK (length(previous_epoch) = 16 AND previous_epoch <> x'00000000000000000000000000000000'),
    new_epoch BLOB NOT NULL UNIQUE CHECK (length(new_epoch) = 16 AND new_epoch <> x'00000000000000000000000000000000' AND new_epoch <> previous_epoch)
) STRICT;
CREATE TRIGGER identity_restore_epochs_no_update BEFORE UPDATE ON identity_restore_epochs
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_restore_receipt'); END;
CREATE TRIGGER identity_restore_epochs_no_delete BEFORE DELETE ON identity_restore_epochs
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_restore_receipt'); END;
DROP TRIGGER identity_instance_no_update;
CREATE TRIGGER identity_instance_no_update BEFORE UPDATE ON identity_instance
WHEN NEW.singleton <> OLD.singleton OR NOT EXISTS (
    SELECT 1 FROM identity_restore_epochs
    WHERE previous_epoch = OLD.instance_epoch AND new_epoch = NEW.instance_epoch
)
BEGIN SELECT RAISE(ABORT, 'cantos:identity.immutable_instance'); END;
