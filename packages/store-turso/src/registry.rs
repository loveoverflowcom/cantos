//! The versioned, checksummed schema registry of one Turso file, and the pure decision of what
//! a process may do with the file it finds.
//!
//! Nothing here touches I/O. The shell observes the file read-only, asks [`plan_startup`] and
//! then executes exactly the returned plan, so every refusal below is a plain `#[test]`.
//!
//! The registry is independent of the retired PostgreSQL checksum policy: a Turso file has its
//! own `application_id`, its own epoch in `user_version` and its own history table. PostgreSQL
//! SQL and checksums are never replayed here.

use std::fmt;

use sha2::{Digest, Sha256};

pub type Checksum = [u8; 32];

/// Table that records every applied migration. Created by the first migration plan, never by
/// migration SQL, so a migration cannot forge or omit its own history row.
pub const HISTORY_TABLE: &str = "cantos_schema_history";

const CHECKSUM_DOMAIN: &[u8] = b"cantos-turso-migration/1\0";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Migration {
    pub version: u32,
    pub name: &'static str,
    pub sql: &'static str,
}

impl Migration {
    /// Binds the version and name as well as the SQL, so renumbering or renaming an applied
    /// migration is detected even when its SQL text is unchanged.
    pub fn checksum(&self) -> Checksum {
        let mut hasher = Sha256::new();
        hasher.update(CHECKSUM_DOMAIN);
        hasher.update(self.version.to_be_bytes());
        hasher.update((self.name.len() as u64).to_be_bytes());
        hasher.update(self.name.as_bytes());
        hasher.update(self.sql.as_bytes());
        hasher.finalize().into()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RegistryError {
    /// Zero denotes an unmarked file and cannot identify a database owner.
    UnmarkedApplicationId,
    /// Epoch 0 means "unmarked"; the header field is a signed 32-bit integer.
    EpochOutOfRange,
    EmptyHistory,
    /// Versions must be exactly `1..=n` in order.
    NonContiguousVersions {
        expected: u32,
    },
    InvalidName {
        version: u32,
    },
    EmptySql {
        version: u32,
    },
    /// Migration SQL controls the transaction, the header or the file set. The runner owns those.
    ForbiddenStatement {
        version: u32,
    },
}

impl fmt::Display for RegistryError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "invalid schema registry: {self:?}")
    }
}

impl std::error::Error for RegistryError {}

#[derive(Clone, Copy, Debug)]
pub struct Registry {
    application_id: i32,
    epoch: u32,
    migrations: &'static [Migration],
}

impl Registry {
    /// Bind a consumer-selected, nonzero application marker to its schema history.
    /// The caller owns marker uniqueness, epoch and migration definitions.
    pub fn new(
        application_id: i32,
        epoch: u32,
        migrations: &'static [Migration],
    ) -> Result<Self, RegistryError> {
        if application_id == 0 {
            return Err(RegistryError::UnmarkedApplicationId);
        }
        if epoch == 0 || i32::try_from(epoch).is_err() {
            return Err(RegistryError::EpochOutOfRange);
        }
        if migrations.is_empty() {
            return Err(RegistryError::EmptyHistory);
        }
        for (index, migration) in migrations.iter().enumerate() {
            let expected = u32::try_from(index + 1).map_err(|_| RegistryError::EmptyHistory)?;
            if migration.version != expected {
                return Err(RegistryError::NonContiguousVersions { expected });
            }
            let name_ok = !migration.name.is_empty()
                && migration.name.len() <= 64
                && migration
                    .name
                    .bytes()
                    .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_');
            if !name_ok {
                return Err(RegistryError::InvalidName {
                    version: migration.version,
                });
            }
            if migration.sql.trim().is_empty() {
                return Err(RegistryError::EmptySql {
                    version: migration.version,
                });
            }
            if forbidden_statement(migration.sql).is_some() {
                return Err(RegistryError::ForbiddenStatement {
                    version: migration.version,
                });
            }
        }
        Ok(Self {
            application_id,
            epoch,
            migrations,
        })
    }

    /// The consumer-owned marker stored verbatim in the signed file-header field.
    pub const fn application_id(&self) -> i32 {
        self.application_id
    }

    pub const fn epoch(&self) -> u32 {
        self.epoch
    }

    pub const fn migrations(&self) -> &'static [Migration] {
        self.migrations
    }

    pub fn latest(&self) -> u32 {
        self.migrations.last().map_or(0, |m| m.version)
    }
}

/// A row of the history table as read back from the file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AppliedMigration {
    pub version: u32,
    pub name: String,
    pub checksum: Checksum,
}

/// What a read-only look at the path found.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Observation {
    /// No file, or a zero-length file.
    Absent,
    Present(PresentFile),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PresentFile {
    pub application_id: i32,
    pub user_version: i32,
    /// Schema objects other than the engine's own `sqlite_%` ones.
    pub user_objects: u64,
    /// `None` when the history table does not exist.
    pub applied: Option<Vec<AppliedMigration>>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Intent {
    /// `cantos-migrate`: create an absent or empty file, or apply pending migrations.
    Migrate,
    /// Ordinary startup: the file must already be exactly current. Serving never mutates schema.
    Serve,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Plan {
    /// Create the file: header marker, history table and every migration, in one transaction.
    Create(&'static [Migration]),
    /// Apply the pending suffix, one transaction per migration.
    Upgrade(&'static [Migration]),
    Ready,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Refusal {
    /// `Serve` found no file.
    Missing,
    /// `Serve` found an empty, unmarked file.
    NotInitialized,
    /// The header belongs to another application (or another Cantos service).
    ForeignApplication { found: i32 },
    /// Objects exist but no Cantos marker: not ours, or damaged. Never adopted silently.
    UnmarkedNonEmpty,
    /// The schema generation differs; epochs only change through a controlled cutover.
    EpochMismatch { found: i32, expected: u32 },
    /// Marked as ours but the history table is missing.
    HistoryMissing,
    /// An applied row does not match the binary's migration at that version.
    HistoryDiverged { version: u32 },
    /// The file was migrated by a newer binary.
    NewerThanBinary { applied: u32, known: u32 },
    /// `Serve` needs migrations that have not been applied.
    PendingMigrations { applied: u32, latest: u32 },
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("the database file does not exist"),
            Self::NotInitialized => f.write_str("the database file is empty and unmarked"),
            Self::ForeignApplication { .. } => {
                f.write_str("the file belongs to another application")
            }
            Self::UnmarkedNonEmpty => {
                f.write_str("the file has schema objects but no Cantos marker")
            }
            Self::EpochMismatch { .. } => {
                f.write_str("the schema epoch does not match this binary")
            }
            Self::HistoryMissing => f.write_str("the schema history table is missing"),
            Self::HistoryDiverged { version } => {
                write!(f, "applied migration {version} differs from this binary")
            }
            Self::NewerThanBinary { .. } => f.write_str("the file was migrated by a newer binary"),
            Self::PendingMigrations { .. } => f.write_str("migrations are pending"),
        }
    }
}

impl std::error::Error for Refusal {}

/// Decide what the process may do with the observed file. Pure and total.
pub fn plan_startup(
    registry: &Registry,
    observed: &Observation,
    intent: Intent,
) -> Result<Plan, Refusal> {
    let present = match observed {
        Observation::Absent => {
            return match intent {
                Intent::Migrate => Ok(Plan::Create(registry.migrations)),
                Intent::Serve => Err(Refusal::Missing),
            }
        }
        Observation::Present(present) => present,
    };
    if present.application_id == 0 {
        if present.user_objects > 0 || present.user_version != 0 || present.applied.is_some() {
            return Err(Refusal::UnmarkedNonEmpty);
        }
        return match intent {
            Intent::Migrate => Ok(Plan::Create(registry.migrations)),
            Intent::Serve => Err(Refusal::NotInitialized),
        };
    }
    if present.application_id != registry.application_id {
        return Err(Refusal::ForeignApplication {
            found: present.application_id,
        });
    }
    if u32::try_from(present.user_version) != Ok(registry.epoch) {
        return Err(Refusal::EpochMismatch {
            found: present.user_version,
            expected: registry.epoch,
        });
    }
    let applied = present.applied.as_deref().ok_or(Refusal::HistoryMissing)?;
    let known = registry.migrations.len();
    if applied.len() > known {
        return Err(Refusal::NewerThanBinary {
            applied: u32::try_from(applied.len()).unwrap_or(u32::MAX),
            known: registry.latest(),
        });
    }
    for (row, migration) in applied.iter().zip(registry.migrations) {
        let same = row.version == migration.version
            && row.name == migration.name
            && row.checksum == migration.checksum();
        if !same {
            return Err(Refusal::HistoryDiverged {
                version: migration.version,
            });
        }
    }
    if applied.len() == known {
        return Ok(Plan::Ready);
    }
    match intent {
        Intent::Migrate => Ok(Plan::Upgrade(&registry.migrations[applied.len()..])),
        Intent::Serve => Err(Refusal::PendingMigrations {
            applied: u32::try_from(applied.len()).unwrap_or(u32::MAX),
            latest: registry.latest(),
        }),
    }
}

/// First statement keyword that a migration must not contain, if any. Best-effort scanner over
/// reviewed source, not a SQL parser: it skips comments and quoted text, keeps `CREATE TRIGGER`
/// bodies (and their `CASE … END`) as one statement, and fails closed on what it cannot tell.
pub fn forbidden_statement(sql: &str) -> Option<&'static str> {
    const FORBIDDEN: [&str; 10] = [
        "BEGIN",
        "COMMIT",
        "END",
        "ROLLBACK",
        "SAVEPOINT",
        "RELEASE",
        "PRAGMA",
        "VACUUM",
        "ATTACH",
        "DETACH",
    ];
    let tokens = tokenize(sql);
    let mut index = 0;
    while index < tokens.len() {
        if tokens[index] == Token::Semi {
            index += 1;
            continue;
        }
        if let Token::Word(first) = &tokens[index] {
            if let Some(hit) = FORBIDDEN.iter().find(|word| **word == first) {
                return Some(hit);
            }
        }
        if starts_trigger(&tokens[index..]) {
            index = skip_trigger(&tokens, index);
        } else {
            while index < tokens.len() && tokens[index] != Token::Semi {
                index += 1;
            }
        }
    }
    None
}

#[derive(Debug, PartialEq, Eq)]
enum Token {
    Word(String),
    Semi,
    Other,
}

fn starts_trigger(tokens: &[Token]) -> bool {
    let word = |i: usize| match tokens.get(i) {
        Some(Token::Word(w)) => Some(w.as_str()),
        _ => None,
    };
    word(0) == Some("CREATE")
        && (word(1) == Some("TRIGGER")
            || (matches!(word(1), Some("TEMP" | "TEMPORARY")) && word(2) == Some("TRIGGER")))
}

/// Index just past the `END ;` that closes the trigger starting at `from`.
fn skip_trigger(tokens: &[Token], from: usize) -> usize {
    let mut depth = 0_u32;
    let mut index = from;
    while index < tokens.len() {
        if let Token::Word(word) = &tokens[index] {
            match word.as_str() {
                "CASE" => depth += 1,
                "END" if depth > 0 => depth -= 1,
                "END" if tokens.get(index + 1).is_none_or(|t| *t == Token::Semi) => {
                    return index + 2;
                }
                _ => {}
            }
        }
        index += 1;
    }
    tokens.len()
}

fn tokenize(sql: &str) -> Vec<Token> {
    let bytes = sql.as_bytes();
    let mut tokens = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        let b = bytes[i];
        match b {
            b'-' if bytes.get(i + 1) == Some(&b'-') => {
                while i < bytes.len() && bytes[i] != b'\n' {
                    i += 1;
                }
            }
            b'/' if bytes.get(i + 1) == Some(&b'*') => {
                i += 2;
                while i < bytes.len() && !(bytes[i] == b'*' && bytes.get(i + 1) == Some(&b'/')) {
                    i += 1;
                }
                i += 2;
            }
            b'\'' | b'"' | b'`' => {
                i += 1;
                while i < bytes.len() {
                    if bytes[i] == b {
                        if bytes.get(i + 1) == Some(&b) {
                            i += 2;
                            continue;
                        }
                        break;
                    }
                    i += 1;
                }
                i += 1;
                tokens.push(Token::Other);
            }
            b'[' => {
                while i < bytes.len() && bytes[i] != b']' {
                    i += 1;
                }
                i += 1;
                tokens.push(Token::Other);
            }
            b';' => {
                tokens.push(Token::Semi);
                i += 1;
            }
            _ if b.is_ascii_alphabetic() || b == b'_' => {
                let start = i;
                while i < bytes.len() && (bytes[i].is_ascii_alphanumeric() || bytes[i] == b'_') {
                    i += 1;
                }
                tokens.push(Token::Word(sql[start..i].to_ascii_uppercase()));
            }
            _ => i += 1,
        }
    }
    tokens
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::testing::{OTHER_APPLICATION_ID, TEST_APPLICATION_ID};

    const M1: Migration = Migration {
        version: 1,
        name: "create_items",
        sql: "CREATE TABLE items(id INTEGER PRIMARY KEY) STRICT;",
    };
    const M2: Migration = Migration {
        version: 2,
        name: "add_label",
        sql: "ALTER TABLE items ADD COLUMN label TEXT;",
    };
    const MIGRATIONS: [Migration; 2] = [M1, M2];

    fn registry() -> Registry {
        Registry::new(TEST_APPLICATION_ID, 1, &MIGRATIONS).unwrap()
    }

    fn row(migration: &Migration) -> AppliedMigration {
        AppliedMigration {
            version: migration.version,
            name: migration.name.to_owned(),
            checksum: migration.checksum(),
        }
    }

    fn present(applied: Option<Vec<AppliedMigration>>) -> Observation {
        Observation::Present(PresentFile {
            application_id: TEST_APPLICATION_ID,
            user_version: 1,
            user_objects: 4,
            applied,
        })
    }

    #[test]
    fn registry_construction_rejects_each_invalid_shape() {
        let bad =
            |migrations: &'static [Migration]| Registry::new(TEST_APPLICATION_ID, 1, migrations);
        assert_eq!(
            Registry::new(TEST_APPLICATION_ID, 0, &MIGRATIONS).unwrap_err(),
            RegistryError::EpochOutOfRange
        );
        assert_eq!(
            Registry::new(TEST_APPLICATION_ID, 1 << 31, &MIGRATIONS).unwrap_err(),
            RegistryError::EpochOutOfRange
        );
        assert_eq!(bad(&[]).unwrap_err(), RegistryError::EmptyHistory);
        assert_eq!(
            bad(&[M2]).unwrap_err(),
            RegistryError::NonContiguousVersions { expected: 1 }
        );
        assert_eq!(
            bad(&[M1, M1]).unwrap_err(),
            RegistryError::NonContiguousVersions { expected: 2 }
        );
        const RENAMED: Migration = Migration {
            name: "Has Space",
            ..M1
        };
        assert_eq!(
            bad(&[RENAMED]).unwrap_err(),
            RegistryError::InvalidName { version: 1 }
        );
        const BLANK: Migration = Migration { sql: "  \n", ..M1 };
        assert_eq!(
            bad(&[BLANK]).unwrap_err(),
            RegistryError::EmptySql { version: 1 }
        );
        const COMMITS: Migration = Migration {
            sql: "CREATE TABLE a(x); COMMIT;",
            ..M1
        };
        assert_eq!(
            bad(&[COMMITS]).unwrap_err(),
            RegistryError::ForbiddenStatement { version: 1 }
        );
    }

    #[test]
    fn any_nonzero_signed_marker_is_accepted_and_zero_is_reserved_for_unmarked_files() {
        for application_id in [i32::MIN, -1, 1, TEST_APPLICATION_ID, i32::MAX] {
            let registry = Registry::new(application_id, 1, &MIGRATIONS).unwrap();
            assert_eq!(registry.application_id(), application_id);
            let mut file = match present(Some(vec![row(&M1), row(&M2)])) {
                Observation::Present(file) => file,
                Observation::Absent => unreachable!(),
            };
            file.application_id = application_id;
            assert_eq!(
                plan_startup(&registry, &Observation::Present(file), Intent::Serve),
                Ok(Plan::Ready)
            );
        }
        assert_eq!(
            Registry::new(0, 1, &MIGRATIONS).unwrap_err(),
            RegistryError::UnmarkedApplicationId
        );
    }

    #[test]
    fn checksum_binds_version_name_and_sql() {
        let base = M1.checksum();
        assert_eq!(base, M1.checksum());
        // Frozen pre-refactor digest: changing the framing/domain would invalidate existing files.
        assert_eq!(
            base,
            [
                0x12, 0x61, 0x31, 0x99, 0xe6, 0xda, 0x52, 0x0d, 0x6d, 0x95, 0x16, 0x48, 0x68, 0x0a,
                0x2c, 0xb4, 0x50, 0x7d, 0xc8, 0x57, 0xb1, 0x7c, 0x01, 0x35, 0x8e, 0x81, 0x33, 0x61,
                0x11, 0x93, 0xed, 0x42
            ]
        );
        assert_ne!(base, Migration { version: 2, ..M1 }.checksum());
        assert_ne!(
            base,
            Migration {
                name: "other",
                ..M1
            }
            .checksum()
        );
        assert_ne!(
            base,
            Migration {
                sql: "CREATE TABLE items(id INTEGER) STRICT;",
                ..M1
            }
            .checksum()
        );
        // Length framing: moving a byte between name and sql must change the digest.
        let a = Migration {
            version: 1,
            name: "ab",
            sql: "c",
        };
        let b = Migration {
            version: 1,
            name: "a",
            sql: "bc",
        };
        assert_ne!(a.checksum(), b.checksum());
    }

    #[test]
    fn absent_and_empty_files_may_only_be_created_by_a_migrate_intent() {
        let r = registry();
        let empty = Observation::Present(PresentFile {
            application_id: 0,
            user_version: 0,
            user_objects: 0,
            applied: None,
        });
        for observed in [Observation::Absent, empty.clone()] {
            assert_eq!(
                plan_startup(&r, &observed, Intent::Migrate),
                Ok(Plan::Create(&MIGRATIONS))
            );
        }
        assert_eq!(
            plan_startup(&r, &Observation::Absent, Intent::Serve),
            Err(Refusal::Missing)
        );
        assert_eq!(
            plan_startup(&r, &empty, Intent::Serve),
            Err(Refusal::NotInitialized)
        );
    }

    #[test]
    fn unmarked_or_foreign_files_are_never_adopted() {
        let r = registry();
        let unmarked = |objects, version, history: Option<Vec<AppliedMigration>>| {
            Observation::Present(PresentFile {
                application_id: 0,
                user_version: version,
                user_objects: objects,
                applied: history,
            })
        };
        for observed in [
            unmarked(1, 0, None),
            unmarked(0, 3, None),
            unmarked(0, 0, Some(vec![])),
        ] {
            for intent in [Intent::Migrate, Intent::Serve] {
                assert_eq!(
                    plan_startup(&r, &observed, intent),
                    Err(Refusal::UnmarkedNonEmpty)
                );
            }
        }
        for found in [OTHER_APPLICATION_ID, 1, -1, i32::MAX] {
            let foreign = Observation::Present(PresentFile {
                application_id: found,
                user_version: 1,
                user_objects: 4,
                applied: Some(vec![row(&M1), row(&M2)]),
            });
            for intent in [Intent::Migrate, Intent::Serve] {
                assert_eq!(
                    plan_startup(&r, &foreign, intent),
                    Err(Refusal::ForeignApplication { found })
                );
            }
        }
    }

    #[test]
    fn a_different_epoch_is_refused_in_both_directions() {
        let r = registry();
        for version in [0, 2, -1, i32::MAX] {
            let observed = Observation::Present(PresentFile {
                application_id: TEST_APPLICATION_ID,
                user_version: version,
                user_objects: 4,
                applied: Some(vec![row(&M1), row(&M2)]),
            });
            assert_eq!(
                plan_startup(&r, &observed, Intent::Migrate),
                Err(Refusal::EpochMismatch {
                    found: version,
                    expected: 1
                })
            );
        }
    }

    #[test]
    fn current_files_are_ready_and_pending_suffixes_need_a_migrate_intent() {
        let r = registry();
        let current = present(Some(vec![row(&M1), row(&M2)]));
        assert_eq!(plan_startup(&r, &current, Intent::Serve), Ok(Plan::Ready));
        assert_eq!(plan_startup(&r, &current, Intent::Migrate), Ok(Plan::Ready));
        let behind = present(Some(vec![row(&M1)]));
        assert_eq!(
            plan_startup(&r, &behind, Intent::Migrate),
            Ok(Plan::Upgrade(&MIGRATIONS[1..]))
        );
        assert_eq!(
            plan_startup(&r, &behind, Intent::Serve),
            Err(Refusal::PendingMigrations {
                applied: 1,
                latest: 2
            })
        );
        assert_eq!(
            plan_startup(&r, &present(Some(vec![])), Intent::Migrate),
            Ok(Plan::Upgrade(&MIGRATIONS[..]))
        );
        assert_eq!(
            plan_startup(&r, &present(None), Intent::Migrate),
            Err(Refusal::HistoryMissing)
        );
    }

    #[test]
    fn divergent_or_newer_history_is_refused_before_any_write() {
        let r = registry();
        let mut tampered = row(&M1);
        tampered.checksum[0] ^= 1;
        let renamed = AppliedMigration {
            name: "other".into(),
            ..row(&M1)
        };
        let skipped = AppliedMigration {
            version: 2,
            ..row(&M2)
        };
        for history in [vec![tampered], vec![renamed], vec![skipped]] {
            assert_eq!(
                plan_startup(&r, &present(Some(history)), Intent::Migrate),
                Err(Refusal::HistoryDiverged { version: 1 })
            );
        }
        let extra = AppliedMigration {
            version: 3,
            name: "future".into(),
            checksum: [7; 32],
        };
        assert_eq!(
            plan_startup(
                &r,
                &present(Some(vec![row(&M1), row(&M2), extra])),
                Intent::Serve
            ),
            Err(Refusal::NewerThanBinary {
                applied: 3,
                known: 2
            })
        );
    }

    #[test]
    fn the_scanner_finds_transaction_and_header_control_but_keeps_trigger_bodies() {
        for sql in [
            "COMMIT;",
            "create table a(x); begin;",
            "PRAGMA user_version = 2;",
            "VACUUM;",
            "ATTACH 'x' AS y;",
            "ROLLBACK",
            "SAVEPOINT s;",
            "/* c */ -- c\n END;",
        ] {
            assert!(forbidden_statement(sql).is_some(), "{sql}");
        }
        for sql in [
            "CREATE TABLE a(x TEXT DEFAULT 'COMMIT; BEGIN');",
            "-- COMMIT;\nCREATE TABLE b(\"end\" INTEGER);",
            "CREATE TRIGGER g BEFORE UPDATE ON t BEGIN SELECT CASE WHEN OLD.v <> NEW.v THEN RAISE(ABORT,'cantos:g') END; END;",
            "CREATE TEMP TRIGGER g BEFORE DELETE ON t BEGIN SELECT RAISE(ABORT,'cantos:g'); END; CREATE TABLE c(x);",
            "CREATE TRIGGER a BEFORE UPDATE ON t BEGIN SELECT 1; SELECT 2; END;\nCREATE TRIGGER b BEFORE DELETE ON t BEGIN SELECT 3; END;",
        ] {
            assert_eq!(forbidden_statement(sql), None, "{sql}");
        }
        // A statement after a trigger is still scanned.
        assert_eq!(
            forbidden_statement("CREATE TRIGGER g BEFORE UPDATE ON t BEGIN SELECT 1; END; COMMIT;"),
            Some("COMMIT")
        );
    }
}
