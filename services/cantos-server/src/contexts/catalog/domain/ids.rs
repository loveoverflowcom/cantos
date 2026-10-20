use serde::Serialize;

use crate::platform::ids::canonical_uuid;

use super::{CatalogError, IdKind};

/// Immutable work identity, using the existing canonical entity UUID spelling.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct WorkId(String);

impl WorkId {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        canonical_uuid(value)
            .map(Self)
            .ok_or(CatalogError::InvalidId(IdKind::Work))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Immutable chapter or episode identity; cannot be substituted for a work ID.
/// ```compile_fail,E0308
/// use cantos_server::contexts::catalog::domain::{ContentUnitId, WorkId};
/// let unit: ContentUnitId = WorkId::parse("00000000-0000-0000-0000-000000000001").unwrap();
/// ```
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct ContentUnitId(String);

impl ContentUnitId {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        canonical_uuid(value)
            .map(Self)
            .ok_or(CatalogError::InvalidId(IdKind::ContentUnit))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Existing actor identity. Parsing does not establish that an actor exists or is active.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
#[serde(transparent)]
pub struct OwnerId(String);

impl OwnerId {
    pub fn parse(value: &str) -> Result<Self, CatalogError> {
        if !token_id_valid(value) {
            return Err(CatalogError::InvalidId(IdKind::Owner));
        }
        Ok(Self(value.to_owned()))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

pub(super) fn token_id_valid(value: &str) -> bool {
    (1..=64).contains(&value.len())
        && value
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte))
}
