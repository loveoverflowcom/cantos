//! JSON-only Studio catalog transport. These raw values do not establish ownership,
//! admitted metadata, source integrity, rights clearance or publication authority.
use serde::{Deserialize, Deserializer, Serialize, Serializer};

/// PATCH distinguishes an omitted field from an explicit clear and a replacement value.
/// Only optional unit links/checksums may be cleared; admission rejects `Null` elsewhere.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PatchField<T> {
    Missing,
    Null,
    Value(T),
}

impl<T> Default for PatchField<T> {
    fn default() -> Self {
        Self::Missing
    }
}

impl<T> PatchField<T> {
    pub fn is_missing(&self) -> bool {
        matches!(self, Self::Missing)
    }
}

impl<'de, T: Deserialize<'de>> Deserialize<'de> for PatchField<T> {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        Option::<T>::deserialize(deserializer).map(|value| value.map_or(Self::Null, Self::Value))
    }
}

impl<T: Serialize> Serialize for PatchField<T> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        match self {
            Self::Value(value) => value.serialize(serializer),
            Self::Missing | Self::Null => serializer.serialize_none(),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkDetailsDto {
    pub title: String,
    pub synopsis: String,
    pub language: String,
    pub visibility: String,
    pub rights_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogSourceRef {
    pub id: String,
    pub sha256: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogScriptRef {
    pub id: String,
    pub revision: u64,
    pub export_digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitDetailsDto {
    pub title: String,
    pub description: String,
    pub source: Option<CatalogSourceRef>,
    pub script: Option<CatalogScriptRef>,
    pub content_digest: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkPatchDto {
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub title: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub synopsis: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub language: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub visibility: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub rights_refs: PatchField<Vec<String>>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct UnitPatchDto {
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub title: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub description: PatchField<String>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub source: PatchField<CatalogSourceRef>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub script: PatchField<CatalogScriptRef>,
    #[serde(default, skip_serializing_if = "PatchField::is_missing")]
    pub content_digest: PatchField<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateWorkRequest {
    pub operation_id: String,
    pub id: String,
    pub kind: String,
    pub details: WorkDetailsDto,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchWorkRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub details: WorkPatchDto,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArchiveWorkRequest {
    pub operation_id: String,
    pub expected_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateContentUnitRequest {
    pub operation_id: String,
    pub id: String,
    pub kind: String,
    pub expected_work_revision: u64,
    pub details: UnitDetailsDto,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct PatchContentUnitRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub expected_work_revision: u64,
    pub details: UnitPatchDto,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ArchiveContentUnitRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub expected_work_revision: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ReorderContentUnitsRequest {
    pub operation_id: String,
    pub expected_revision: u64,
    pub ordered: Vec<String>,
}

/// Current or historical immutable Work snapshot. Times are nonnegative Unix milliseconds.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogWork {
    pub id: String,
    pub kind: String,
    pub owner_id: String,
    pub details: WorkDetailsDto,
    pub lifecycle: String,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

/// Ownership is inherited from its immutable Work parent and is not repeated on the wire.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogContentUnit {
    pub id: String,
    pub work_id: String,
    pub kind: String,
    pub details: UnitDetailsDto,
    pub position: u32,
    pub lifecycle: String,
    pub revision: u64,
    pub created_at: i64,
    pub updated_at: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogMutationResponse {
    pub work: CatalogWork,
    pub units: Vec<CatalogContentUnit>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CatalogPage<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<String>,
}
