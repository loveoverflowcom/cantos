CREATE TABLE cantos_recovery_state (
    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
    instance BLOB NOT NULL CHECK (length(instance) = 16 AND instance <> x'00000000000000000000000000000000'),
    revision INTEGER NOT NULL CHECK (revision >= 1)
) STRICT;
CREATE TRIGGER cantos_recovery_no_delete BEFORE DELETE ON cantos_recovery_state
BEGIN SELECT RAISE(ABORT, 'cantos:recovery.immutable_instance'); END;
CREATE TRIGGER cantos_recovery_monotonic BEFORE UPDATE ON cantos_recovery_state
WHEN NEW.singleton <> OLD.singleton OR NEW.instance <> OLD.instance OR NEW.revision <> OLD.revision + 1
BEGIN SELECT RAISE(ABORT, 'cantos:recovery.monotonic'); END;
