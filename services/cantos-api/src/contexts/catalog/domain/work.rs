use serde::Serialize;

use super::revision::Revision;

use super::metadata::{next_revision, validate_timestamps};
use super::{CatalogError, Lifecycle, OwnerId, WorkDetails, WorkId, WorkKind};

/// Caller-supplied creation facts. The application derives `owner_id` from authentication.
pub struct NewWork {
    pub id: WorkId,
    pub kind: WorkKind,
    pub owner_id: OwnerId,
    pub details: WorkDetails,
    /// Authoritative Unix milliseconds, supplied by the transaction shell.
    pub at: i64,
}

/// Storage admission facts. Every restore passes the same validation as a fresh value.
pub struct StoredWork {
    pub id: WorkId,
    pub kind: WorkKind,
    pub owner_id: OwnerId,
    pub details: WorkDetails,
    pub lifecycle: Lifecycle,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Immutable snapshot of a work revision. Identity, owner and category never change.
/// Raw storage facts must pass `from_stored`; serde cannot bypass admission.
/// ```compile_fail,E0277
/// use cantos_api::contexts::catalog::domain::Work;
/// let forged = serde_json::from_str::<Work>("{}");
/// ```
/// ```compile_fail,E0616
/// use cantos_api::contexts::catalog::domain::{Work, WorkId};
/// fn overwrite_identity(work: &mut Work) {
///     work.id = WorkId::parse("00000000-0000-0000-0000-000000000001").unwrap();
/// }
/// ```
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Work {
    id: WorkId,
    kind: WorkKind,
    owner_id: OwnerId,
    details: WorkDetails,
    lifecycle: Lifecycle,
    revision: Revision,
    created_at: i64,
    updated_at: i64,
}

impl Work {
    pub fn new(input: NewWork) -> Result<Self, CatalogError> {
        Self::from_stored(StoredWork {
            id: input.id,
            kind: input.kind,
            owner_id: input.owner_id,
            details: input.details,
            lifecycle: Lifecycle::Draft,
            revision: 1,
            created_at: input.at,
            updated_at: input.at,
        })
    }

    pub fn from_stored(input: StoredWork) -> Result<Self, CatalogError> {
        input.details.validate()?;
        let revision = Revision::new(input.revision)?;
        validate_timestamps(input.created_at, input.updated_at)?;
        Ok(Self {
            id: input.id,
            kind: input.kind,
            owner_id: input.owner_id,
            details: input.details,
            lifecycle: input.lifecycle,
            revision,
            created_at: input.created_at,
            updated_at: input.updated_at,
        })
    }

    pub fn id(&self) -> &WorkId {
        &self.id
    }
    pub fn kind(&self) -> WorkKind {
        self.kind
    }
    pub fn owner_id(&self) -> &OwnerId {
        &self.owner_id
    }
    pub fn details(&self) -> &WorkDetails {
        &self.details
    }
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }
    pub fn revision(&self) -> u64 {
        self.revision.get()
    }
    pub fn created_at(&self) -> i64 {
        self.created_at
    }
    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    pub fn edit(&self, expected: u64, details: WorkDetails, at: i64) -> Result<Self, CatalogError> {
        self.require_draft()?;
        details.validate()?;
        let revision = next_revision(self.revision, expected, self.updated_at, at)?;
        Ok(Self {
            details,
            revision,
            updated_at: at,
            ..self.clone()
        })
    }

    pub fn archive(&self, expected: u64, at: i64) -> Result<Self, CatalogError> {
        self.transition(expected, Lifecycle::Draft, Lifecycle::Archived, at)
    }

    pub fn restore(&self, expected: u64, at: i64) -> Result<Self, CatalogError> {
        self.transition(expected, Lifecycle::Archived, Lifecycle::Draft, at)
    }

    /// Child changes and reorder operations advance the work's catalog revision. The shell
    /// appends this snapshot atomically with child snapshots, giving list cursors one guard.
    pub fn touch(&self, expected: u64, at: i64) -> Result<Self, CatalogError> {
        self.require_draft()?;
        self.advance_revision(expected, at)
    }

    /// Archival retains private history, so owners must still be able to revoke access.
    /// The application owns authorization; this transition changes only the guard/time.
    pub fn with_permission_change(&self, expected: u64, at: i64) -> Result<Self, CatalogError> {
        self.advance_revision(expected, at)
    }

    fn advance_revision(&self, expected: u64, at: i64) -> Result<Self, CatalogError> {
        let revision = next_revision(self.revision, expected, self.updated_at, at)?;
        Ok(Self {
            revision,
            updated_at: at,
            ..self.clone()
        })
    }

    pub(super) fn require_draft(&self) -> Result<(), CatalogError> {
        if self.lifecycle == Lifecycle::Archived {
            return Err(CatalogError::WorkArchived);
        }
        Ok(())
    }

    fn transition(
        &self,
        expected: u64,
        from: Lifecycle,
        to: Lifecycle,
        at: i64,
    ) -> Result<Self, CatalogError> {
        if self.lifecycle != from {
            return Err(CatalogError::UnexpectedLifecycle {
                expected: from,
                actual: self.lifecycle,
            });
        }
        let revision = next_revision(self.revision, expected, self.updated_at, at)?;
        Ok(Self {
            lifecycle: to,
            revision,
            updated_at: at,
            ..self.clone()
        })
    }
}
