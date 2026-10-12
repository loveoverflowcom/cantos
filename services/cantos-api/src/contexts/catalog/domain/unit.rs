use serde::Serialize;

use super::metadata::{
    next_revision, validate_position, validate_revision, validate_timestamps, validate_unit_kind,
};
use super::{
    CatalogError, ContentUnitId, ContentUnitKind, Lifecycle, UnitDetails, Work, WorkId, WorkKind,
};

/// Unit creation facts. Its one parent and inherited ownership come from the work snapshot.
pub struct NewContentUnit {
    pub id: ContentUnitId,
    pub kind: ContentUnitKind,
    pub details: UnitDetails,
    pub position: u32,
    pub at: i64,
}

/// Storage admission facts; the shell supplies the immutable parent's declared category.
pub struct StoredContentUnit {
    pub id: ContentUnitId,
    pub work_id: WorkId,
    pub kind: ContentUnitKind,
    pub details: UnitDetails,
    pub position: u32,
    pub lifecycle: Lifecycle,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Immutable content-unit revision. Reordering changes position, never identity or parent.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct ContentUnit {
    id: ContentUnitId,
    work_id: WorkId,
    kind: ContentUnitKind,
    details: UnitDetails,
    position: u32,
    lifecycle: Lifecycle,
    revision: u64,
    created_at: i64,
    updated_at: i64,
}

impl ContentUnit {
    pub fn new(parent: &Work, input: NewContentUnit) -> Result<Self, CatalogError> {
        parent.require_draft()?;
        if input.at < parent.created_at() {
            return Err(CatalogError::TimestampRegression);
        }
        Self::from_stored(
            StoredContentUnit {
                id: input.id,
                work_id: parent.id().clone(),
                kind: input.kind,
                details: input.details,
                position: input.position,
                lifecycle: Lifecycle::Draft,
                revision: 1,
                created_at: input.at,
                updated_at: input.at,
            },
            parent.kind(),
        )
    }

    pub fn from_stored(
        input: StoredContentUnit,
        parent_kind: WorkKind,
    ) -> Result<Self, CatalogError> {
        validate_unit_kind(parent_kind, input.kind)?;
        input.details.validate()?;
        validate_position(input.position)?;
        validate_revision(input.revision)?;
        validate_timestamps(input.created_at, input.updated_at)?;
        Ok(Self {
            id: input.id,
            work_id: input.work_id,
            kind: input.kind,
            details: input.details,
            position: input.position,
            lifecycle: input.lifecycle,
            revision: input.revision,
            created_at: input.created_at,
            updated_at: input.updated_at,
        })
    }

    pub fn id(&self) -> &ContentUnitId {
        &self.id
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub fn kind(&self) -> ContentUnitKind {
        self.kind
    }
    pub fn details(&self) -> &UnitDetails {
        &self.details
    }
    pub fn position(&self) -> u32 {
        self.position
    }
    pub fn lifecycle(&self) -> Lifecycle {
        self.lifecycle
    }
    pub fn revision(&self) -> u64 {
        self.revision
    }
    pub fn created_at(&self) -> i64 {
        self.created_at
    }
    pub fn updated_at(&self) -> i64 {
        self.updated_at
    }

    pub fn edit(
        &self,
        parent: &Work,
        expected: u64,
        details: UnitDetails,
        at: i64,
    ) -> Result<Self, CatalogError> {
        self.require_parent(parent)?;
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

    pub fn with_position(
        &self,
        parent: &Work,
        expected: u64,
        position: u32,
        at: i64,
    ) -> Result<Self, CatalogError> {
        self.require_parent(parent)?;
        self.require_draft()?;
        validate_position(position)?;
        let revision = next_revision(self.revision, expected, self.updated_at, at)?;
        Ok(Self {
            position,
            revision,
            updated_at: at,
            ..self.clone()
        })
    }

    pub fn archive(&self, parent: &Work, expected: u64, at: i64) -> Result<Self, CatalogError> {
        self.transition(parent, expected, Lifecycle::Draft, Lifecycle::Archived, at)
    }

    pub fn restore(&self, parent: &Work, expected: u64, at: i64) -> Result<Self, CatalogError> {
        self.transition(parent, expected, Lifecycle::Archived, Lifecycle::Draft, at)
    }

    fn require_draft(&self) -> Result<(), CatalogError> {
        if self.lifecycle == Lifecycle::Archived {
            return Err(CatalogError::UnitArchived);
        }
        Ok(())
    }

    fn require_parent(&self, parent: &Work) -> Result<(), CatalogError> {
        if self.work_id != *parent.id() {
            return Err(CatalogError::WrongWork {
                unit: self.id.clone(),
            });
        }
        parent.require_draft()?;
        validate_unit_kind(parent.kind(), self.kind)
    }

    fn transition(
        &self,
        parent: &Work,
        expected: u64,
        from: Lifecycle,
        to: Lifecycle,
        at: i64,
    ) -> Result<Self, CatalogError> {
        self.require_parent(parent)?;
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
