//! Studio catalog use cases. The existing session is checked inside each transaction;
//! pure domain transitions decide changes, and the adapter alone owns SQL.
use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::domain::{
    CatalogError, ContentUnit, ContentUnitId, ContentUnitKind, OwnerId, UnitDetails, UnitPatch,
    Work, WorkDetails, WorkId, WorkKind, WorkPatch,
};
use super::turso;
use crate::contexts::editorial::{self, permits, Access, Action};
use crate::contexts::ingestion;
use crate::platform::store::{authenticate, StoreError, Tx};

mod sample_metadata;
pub(super) mod sample_metadata_read;
mod sample_metadata_rights;

pub(super) const MAX_UNITS: usize = 1000;

#[derive(Debug)]
pub enum CatalogStoreError {
    Storage(StoreError),
    Domain(CatalogError),
    DuplicateId,
    StaleCursor,
    OperationReused,
    InvalidCursor,
    Limit,
}
impl From<StoreError> for CatalogStoreError {
    fn from(error: StoreError) -> Self {
        Self::Storage(error)
    }
}
impl From<crate::platform::store::Error> for CatalogStoreError {
    fn from(error: crate::platform::store::Error) -> Self {
        Self::Storage(StoreError::from(error))
    }
}
impl From<CatalogError> for CatalogStoreError {
    fn from(error: CatalogError) -> Self {
        Self::Domain(error)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum MemberRole {
    Reader,
    Editor,
}
impl MemberRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Reader => "reader",
            Self::Editor => "editor",
        }
    }

    /// The stored spelling back to a role; `None` for a role this table does not know.
    pub fn parse(stored: &str) -> Option<Self> {
        [Self::Reader, Self::Editor]
            .into_iter()
            .find(|role| role.as_str() == stored)
    }
}

/// Owner is always derived from the authenticated actor, never from this input.
#[derive(Clone, Debug, Serialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum CatalogCommand {
    CreateWork {
        id: WorkId,
        kind: WorkKind,
        details: WorkDetails,
    },
    EditWork {
        id: WorkId,
        expected_revision: u64,
        details: WorkDetails,
    },
    PatchWork {
        id: WorkId,
        expected_revision: u64,
        patch: WorkPatch,
    },
    ArchiveWork {
        id: WorkId,
        expected_revision: u64,
    },
    CreateUnit {
        work_id: WorkId,
        expected_work_revision: u64,
        id: ContentUnitId,
        kind: ContentUnitKind,
        details: UnitDetails,
    },
    EditUnit {
        id: ContentUnitId,
        expected_revision: u64,
        expected_work_revision: u64,
        details: UnitDetails,
    },
    PatchUnit {
        id: ContentUnitId,
        expected_revision: u64,
        expected_work_revision: u64,
        patch: UnitPatch,
    },
    ArchiveUnit {
        id: ContentUnitId,
        expected_revision: u64,
        expected_work_revision: u64,
    },
    Reorder {
        work_id: WorkId,
        expected_revision: u64,
        ordered: Vec<ContentUnitId>,
    },
    SetMember {
        work_id: WorkId,
        expected_revision: u64,
        actor_id: OwnerId,
        role: Option<MemberRole>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct CatalogOutcome {
    pub work: Work,
    pub units: Vec<ContentUnit>,
}

/// Internal cursor encoding, bound to actor, query and the transactional catalog epoch.
/// Every catalog write invalidates it; no snapshot or access survives a permission change.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatalogCursor {
    scope: String,
    epoch: u64,
    after: String,
}
impl CatalogCursor {
    pub fn encode(&self) -> Result<String, CatalogStoreError> {
        let bytes = serde_json::to_vec(self).map_err(|_| CatalogStoreError::InvalidCursor)?;
        Ok(bytes.iter().map(|byte| format!("{byte:02x}")).collect())
    }
    pub fn decode(encoded: &str) -> Result<Self, CatalogStoreError> {
        if encoded.is_empty() || encoded.len() > 2048 || !encoded.len().is_multiple_of(2) {
            return Err(CatalogStoreError::InvalidCursor);
        }
        let bytes = encoded
            .as_bytes()
            .as_chunks::<2>()
            .0
            .iter()
            .map(|pair| {
                let text =
                    std::str::from_utf8(pair).map_err(|_| CatalogStoreError::InvalidCursor)?;
                u8::from_str_radix(text, 16).map_err(|_| CatalogStoreError::InvalidCursor)
            })
            .collect::<Result<Vec<_>, _>>()?;
        serde_json::from_slice(&bytes).map_err(|_| CatalogStoreError::InvalidCursor)
    }
}
#[derive(Debug)]
pub struct CatalogPage<T> {
    pub items: Vec<T>,
    pub next: Option<CatalogCursor>,
}

pub(super) fn require(access: Access, action: Action) -> Result<(), CatalogStoreError> {
    if permits(access, action) {
        return Ok(());
    }
    Err(StoreError::Forbidden.into())
}
pub(super) async fn authorize(
    tx: &Tx<'_>,
    actor: &str,
    work: &Work,
    action: Action,
) -> Result<Access, CatalogStoreError> {
    let access = if work.owner_id().as_str() == actor {
        Access::Owner
    } else {
        match turso::member_role(tx, work.id().as_str(), actor).await? {
            Some(role) => Access::from_member_role(&role).ok_or(StoreError::CorruptRevision)?,
            None => Access::None,
        }
    };
    if access == Access::None {
        return Err(StoreError::NotFound.into());
    }
    require(access, action)?;
    Ok(access)
}
pub(super) async fn current_actor(tx: &Tx<'_>, token: &str) -> Result<String, CatalogStoreError> {
    let actor = authenticate(tx, token).await?;
    if !turso::lock_actor(tx, &actor).await? {
        return Err(StoreError::Unauthenticated.into());
    }
    // The actor lock can wait, and time-based expiry is not held by a row lock.
    authenticate(tx, token).await?;
    Ok(actor)
}
pub(super) async fn work(tx: &Tx<'_>, id: &WorkId) -> Result<Work, CatalogStoreError> {
    turso::load_work(tx, id.as_str())
        .await?
        .ok_or_else(|| StoreError::NotFound.into())
}
pub(super) async fn unit(
    tx: &Tx<'_>,
    id: &ContentUnitId,
) -> Result<ContentUnit, CatalogStoreError> {
    turso::load_unit(tx, id.as_str())
        .await?
        .ok_or_else(|| StoreError::NotFound.into())
}
pub(super) async fn rights(
    tx: &Tx<'_>,
    owner: &OwnerId,
    details: &WorkDetails,
) -> Result<(), CatalogStoreError> {
    for id in &details.rights_refs {
        if !editorial::evidence_exists(tx, owner.as_str(), "rights", id).await? {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    Ok(())
}
pub(super) async fn links(
    tx: &Tx<'_>,
    owner: &OwnerId,
    details: &UnitDetails,
) -> Result<(), CatalogStoreError> {
    if let Some(source) = &details.source {
        let actual = ingestion::source_checksum(tx, owner.as_str(), source.id()).await?;
        if actual.as_deref() != Some(source.sha256()) {
            return Err(StoreError::EvidenceUnavailable.into());
        }
        if details.script.is_none()
            && details
                .content_digest
                .as_deref()
                .is_some_and(|digest| digest != source.sha256())
        {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    if let Some(script) = &details.script {
        let actual = editorial::owned_revision(
            tx,
            owner.as_str(),
            script.id(),
            i64::try_from(script.revision()).map_err(|_| StoreError::InvalidRequest)?,
        )
        .await?
        .ok_or(StoreError::EvidenceUnavailable)?;
        if actual.export_digest != script.export_digest()
            || details
                .content_digest
                .as_deref()
                .is_some_and(|digest| digest != actual.content_digest)
        {
            return Err(StoreError::EvidenceUnavailable.into());
        }
    }
    Ok(())
}
pub(super) fn decode_outcome(bytes: &[u8]) -> Result<CatalogOutcome, CatalogStoreError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRevision)?;
    let work = turso::decode_work(
        &serde_json::to_vec(&value["work"]).map_err(|_| StoreError::CorruptRevision)?,
    )?;
    let units = value["units"]
        .as_array()
        .ok_or(StoreError::CorruptRevision)?
        .iter()
        .map(|value| {
            let bytes = serde_json::to_vec(value).map_err(|_| StoreError::CorruptRevision)?;
            let unit = turso::decode_unit(&bytes, work.kind())?;
            if unit.work_id() != work.id() {
                return Err(StoreError::CorruptRevision);
            }
            Ok(unit)
        })
        .collect::<Result<Vec<_>, StoreError>>()?;
    Ok(CatalogOutcome { work, units })
}

mod commands;
pub mod content;
pub mod delegation;
pub(crate) mod owner_work;
pub(crate) mod production_content;
mod queries;
pub mod wire;

pub(super) fn page_limit(limit: u64) -> Result<i64, CatalogStoreError> {
    if !(1..=50).contains(&limit) {
        return Err(CatalogStoreError::Limit);
    }
    Ok(limit as i64)
}
pub(super) fn cursor_after<'a>(
    cursor: Option<&'a CatalogCursor>,
    scope: &str,
    epoch: u64,
) -> Result<Option<&'a str>, CatalogStoreError> {
    match cursor {
        Some(cursor) if cursor.scope != scope => Err(CatalogStoreError::InvalidCursor),
        Some(cursor) if cursor.epoch != epoch => Err(CatalogStoreError::StaleCursor),
        Some(cursor) => Ok(Some(&cursor.after)),
        None => Ok(None),
    }
}

#[cfg(test)]
mod tests {
    use super::{cursor_after, CatalogCursor, CatalogStoreError};
    #[test]
    fn cursor_requires_matching_actor_query_and_epoch() {
        let cursor = CatalogCursor {
            scope: "works:alice:false".into(),
            epoch: 3,
            after: "id".into(),
        };
        let decoded = CatalogCursor::decode(&cursor.encode().unwrap()).unwrap();
        assert_eq!(
            cursor_after(Some(&decoded), "works:alice:false", 3).unwrap(),
            Some("id")
        );
        assert!(matches!(
            cursor_after(Some(&decoded), "works:bob:false", 3),
            Err(CatalogStoreError::InvalidCursor)
        ));
        assert!(matches!(
            cursor_after(Some(&decoded), "works:alice:false", 4),
            Err(CatalogStoreError::StaleCursor)
        ));
        for raw in ["", "f", "zz", "7b7d"] {
            assert!(matches!(
                CatalogCursor::decode(raw),
                Err(CatalogStoreError::InvalidCursor)
            ));
        }
    }
}
