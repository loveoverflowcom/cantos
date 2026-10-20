//! Typed store outcomes. The engine reports text; callers receive a closed vocabulary that never
//! carries SQL, row data or file paths.

use std::fmt;
use std::io;

use crate::registry::Refusal;
use crate::value::ValueError;

/// A `RAISE(ABORT, 'cantos:<code>')` message from a guard trigger carries this prefix. Anything
/// else the engine raises as a plain error is an `Engine` fault, never a domain conflict.
pub const GUARD_PREFIX: &str = "cantos:";

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConstraintKind {
    Unique,
    ForeignKey,
    Check,
    NotNull,
    Other,
}

/// A stable, lowercase guard identifier such as `immutable.catalog_revisions`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GuardCode(String);

impl GuardCode {
    pub fn new(code: &str) -> Option<Self> {
        let valid = !code.is_empty()
            && code.len() <= 64
            && code.bytes().all(|b| {
                b.is_ascii_lowercase() || b.is_ascii_digit() || matches!(b, b'_' | b'.' | b'-')
            });
        valid.then(|| Self(code.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StoreFault {
    /// Another connection holds the write lock; nothing was changed.
    Busy,
    /// A deferred transaction read an older snapshot than the current writer; retry whole.
    StaleSnapshot,
    /// `target` is the engine's `table.column` list for UNIQUE and NOT NULL, otherwise `None`.
    Constraint {
        kind: ConstraintKind,
        target: Option<String>,
    },
    Guard(GuardCode),
    ReadOnly,
    DatabaseFull,
    Corrupt,
    NotDatabase,
    Io(io::ErrorKind),
    Interrupted,
    Misuse,
    NoRows,
    Engine,
    Clock,
    Value(ValueError),
}

impl fmt::Display for StoreFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Busy => "the database write lock is busy",
            Self::StaleSnapshot => "the transaction snapshot is stale",
            Self::Constraint { .. } => "a database constraint rejected the change",
            Self::Guard(_) => "a database guard rejected the change",
            Self::ReadOnly => "the database is read-only",
            Self::DatabaseFull => "the database or its volume is full",
            Self::Corrupt => "the database is corrupt",
            Self::NotDatabase => "the file is not a database",
            Self::Io(_) => "a database file I/O error occurred",
            Self::Interrupted => "the database operation was interrupted",
            Self::Misuse => "the database API was misused",
            Self::NoRows => "the query returned no rows",
            Self::Engine => "the database engine reported an error",
            Self::Clock => "the trusted clock is unavailable",
            Self::Value(_) => "a value is outside the storage range",
        })
    }
}

impl std::error::Error for StoreFault {}

impl From<turso::Error> for StoreFault {
    fn from(error: turso::Error) -> Self {
        use turso::Error as E;
        match error {
            E::Busy(_) => Self::Busy,
            E::BusySnapshot(_) => Self::StaleSnapshot,
            E::Interrupt(_) => Self::Interrupted,
            E::Misuse(_) | E::ConversionFailure(_) | E::ToSqlConversionFailure(_) => Self::Misuse,
            E::QueryReturnedNoRows => Self::NoRows,
            E::Constraint(message) => classify_constraint(&message),
            E::Readonly(_) => Self::ReadOnly,
            E::DatabaseFull(_) => Self::DatabaseFull,
            E::NotAdb(_) => Self::NotDatabase,
            E::Corrupt(_) => Self::Corrupt,
            E::IoError(kind, _) => Self::Io(kind),
            E::Error(message) => message
                .strip_prefix(GUARD_PREFIX)
                .and_then(GuardCode::new)
                .map_or(Self::Engine, Self::Guard),
            E::BatchStatementFailed { error, .. } => Self::from(*error),
            E::BatchRollbackFailed { .. } => Self::Engine,
        }
    }
}

impl From<ValueError> for StoreFault {
    fn from(error: ValueError) -> Self {
        Self::Value(error)
    }
}

pub(crate) fn classify_constraint(message: &str) -> StoreFault {
    let target = |text: &str| {
        let (_, tail) = text.split_once(": ")?;
        let valid = !tail.is_empty()
            && tail.len() <= 128
            && tail
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'.' | b',' | b' '));
        valid.then(|| tail.to_owned())
    };
    let (kind, target) = if message.starts_with("UNIQUE constraint failed")
        || message.starts_with("PRIMARY KEY constraint failed")
    {
        (ConstraintKind::Unique, target(message))
    } else if message.starts_with("NOT NULL constraint failed") {
        (ConstraintKind::NotNull, target(message))
    } else if message.starts_with("FOREIGN KEY constraint failed") {
        (ConstraintKind::ForeignKey, None)
    } else if message.starts_with("CHECK constraint failed") {
        (ConstraintKind::Check, None)
    } else {
        (ConstraintKind::Other, None)
    };
    StoreFault::Constraint { kind, target }
}

/// What a failed `COMMIT` proves. A caller must never treat `Unknown` as failed-with-no-effect:
/// the engine may have made the transaction durable before the acknowledgement was lost.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CommitFault {
    /// Definitely not committed; the transaction was rolled back.
    Rejected(StoreFault),
    /// The outcome is not known; reconcile through the operation receipt before any retry.
    Unknown(StoreFault),
}

impl CommitFault {
    pub(crate) fn classify(error: turso::Error) -> Self {
        let definite = matches!(
            error,
            turso::Error::Constraint(_)
                | turso::Error::Busy(_)
                | turso::Error::BusySnapshot(_)
                | turso::Error::Readonly(_)
        );
        let fault = StoreFault::from(error);
        if definite {
            Self::Rejected(fault)
        } else {
            Self::Unknown(fault)
        }
    }

    pub fn fault(&self) -> &StoreFault {
        match self {
            Self::Rejected(fault) | Self::Unknown(fault) => fault,
        }
    }
}

impl fmt::Display for CommitFault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Rejected(_) => f.write_str("the commit was rejected and rolled back"),
            Self::Unknown(_) => f.write_str("the commit outcome is unknown"),
        }
    }
}

impl std::error::Error for CommitFault {}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DirectoryFault {
    Missing,
    NotDirectory,
    /// The directory grants access to its group or others; mode must be `0700`-like.
    TooPermissive,
    /// The directory belongs to a different user than the process.
    ForeignOwner,
    /// The database or lock path is a symbolic link.
    SymlinkRefused,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum OpenError {
    Directory(DirectoryFault),
    /// Another process or handle already owns this file.
    AlreadyOwned,
    /// The file is not what the registry allows this process to open.
    Refused(Refusal),
    Store(StoreFault),
    /// A migration failed and was rolled back; the file is unchanged.
    MigrationFailed {
        version: u32,
        fault: StoreFault,
    },
    /// COMMIT may have become durable. Reconcile the header/history on the next startup;
    /// do not retry the migration on the assumption that it was rolled back.
    MigrationCommitUnknown {
        version: u32,
        fault: StoreFault,
    },
    Io(io::ErrorKind),
}

impl fmt::Display for OpenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Directory(fault) => write!(f, "database directory refused: {fault:?}"),
            Self::AlreadyOwned => f.write_str("the database file already has an owner"),
            Self::Refused(refusal) => write!(f, "database file refused: {refusal}"),
            Self::Store(fault) => write!(f, "database store fault: {fault}"),
            Self::MigrationFailed { version, fault } => {
                write!(f, "migration {version} failed and was rolled back: {fault}")
            }
            Self::MigrationCommitUnknown { version, fault } => {
                write!(f, "migration {version} commit outcome is unknown: {fault}")
            }
            Self::Io(kind) => write!(f, "database file I/O failed: {kind}"),
        }
    }
}

impl std::error::Error for OpenError {}

impl From<StoreFault> for OpenError {
    fn from(fault: StoreFault) -> Self {
        Self::Store(fault)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn engine_constraint_text_maps_to_a_closed_kind_without_leaking_expressions() {
        let unique = StoreFault::from(turso::Error::Constraint(
            "UNIQUE constraint failed: pi.k".into(),
        ));
        assert_eq!(
            unique,
            StoreFault::Constraint {
                kind: ConstraintKind::Unique,
                target: Some("pi.k".into())
            }
        );
        let check = StoreFault::from(turso::Error::Constraint(
            "CHECK constraint failed: octet_length(a) <= 4".into(),
        ));
        assert_eq!(
            check,
            StoreFault::Constraint {
                kind: ConstraintKind::Check,
                target: None
            }
        );
        let hostile = StoreFault::from(turso::Error::Constraint(
            "UNIQUE constraint failed: t.a'; DROP TABLE x;--".into(),
        ));
        assert_eq!(
            hostile,
            StoreFault::Constraint {
                kind: ConstraintKind::Unique,
                target: None
            }
        );
    }

    #[test]
    fn only_prefixed_trigger_messages_are_guards() {
        let guard = StoreFault::from(turso::Error::Error("cantos:immutable.revisions".into()));
        assert_eq!(
            guard,
            StoreFault::Guard(GuardCode::new("immutable.revisions").unwrap())
        );
        for plain in [
            "immutable-row",
            "cantos:",
            "cantos:UPPER",
            "cantos:has space",
        ] {
            assert_eq!(
                StoreFault::from(turso::Error::Error(plain.into())),
                StoreFault::Engine,
                "{plain}"
            );
        }
    }

    #[test]
    fn a_lost_acknowledgement_is_never_classified_as_no_effect() {
        let rejected = [
            turso::Error::Constraint("FOREIGN KEY constraint failed".into()),
            turso::Error::Busy("database is locked".into()),
            turso::Error::BusySnapshot("stale".into()),
            turso::Error::Readonly("readonly".into()),
        ];
        for error in rejected {
            assert!(matches!(
                CommitFault::classify(error),
                CommitFault::Rejected(_)
            ));
        }
        let unknown = [
            turso::Error::IoError(io::ErrorKind::TimedOut, "commit"),
            turso::Error::Corrupt("bad page".into()),
            turso::Error::DatabaseFull("full".into()),
            turso::Error::Interrupt("interrupted".into()),
            turso::Error::Error("anything else".into()),
        ];
        for error in unknown {
            assert!(matches!(
                CommitFault::classify(error),
                CommitFault::Unknown(_)
            ));
        }
    }
}
