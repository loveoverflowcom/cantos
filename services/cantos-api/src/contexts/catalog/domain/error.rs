use super::{ContentUnitId, ContentUnitKind, Lifecycle, WorkKind};

/// Invalid input classes never contain private prose or an unbounded caller value.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CatalogError {
    InvalidId(IdKind),
    UnknownWorkKind,
    UnknownContentUnitKind,
    UnknownVisibility,
    UnknownLifecycle,
    BlankField {
        field: &'static str,
    },
    FieldTooLong {
        field: &'static str,
        max_bytes: usize,
    },
    ControlCharacter {
        field: &'static str,
    },
    InvalidLanguage,
    TooManyRightsRefs,
    InvalidRightsRef,
    DuplicateRightsRef,
    InvalidSourceId,
    InvalidSourceDigest,
    InvalidScriptId,
    InvalidScriptDigest,
    InvalidContentDigest,
    InvalidRevision,
    RevisionLimit,
    RevisionConflict {
        current_revision: u64,
        expected_revision: u64,
    },
    InvalidTimestamp,
    TimestampRegression,
    InvalidOrder,
    WrongContentUnitKind {
        work: WorkKind,
        unit: ContentUnitKind,
    },
    UnexpectedLifecycle {
        expected: Lifecycle,
        actual: Lifecycle,
    },
    WorkArchived,
    UnitArchived,
    WrongWork {
        unit: ContentUnitId,
    },
    DuplicateContentUnit {
        unit: ContentUnitId,
    },
    DuplicatePosition {
        position: u32,
    },
    DuplicateReorderUnit {
        unit: ContentUnitId,
    },
    UnknownReorderUnit {
        unit: ContentUnitId,
    },
    ArchivedReorderUnit {
        unit: ContentUnitId,
    },
    MissingReorderUnit {
        unit: ContentUnitId,
    },
}

/// ID types share lexical rules without sharing their meaning.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IdKind {
    Work,
    ContentUnit,
    Owner,
}
