//! Private Work artwork metadata. Raw DTOs are not authorization, decoded-image,
//! storage, rights or publication witnesses. Image bytes never enter these messages.
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub enum ImageMime {
    #[serde(rename = "image/png")]
    Png,
    #[serde(rename = "image/jpeg")]
    Jpeg,
    #[serde(rename = "image/avif")]
    Avif,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ImageIntentState {
    Pending,
    Admitted,
    Expired,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImageDeclarations {
    pub provenance: String,
    pub rights_refs: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct CreateImageIntentRequest {
    pub operation_id: String,
    /// Caller-minted immutable identity allows recovery after a lost prepare response.
    pub id: String,
    pub expected_sha256: String,
    pub expected_byte_length: u64,
    pub content_type: ImageMime,
    pub declarations: ImageDeclarations,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImageIntentDocument {
    pub schema_version: String,
    pub id: String,
    pub work_id: String,
    pub owner_id: String,
    pub operation_id: String,
    pub request: CreateImageIntentRequest,
    pub created_at_ms: u64,
    pub expires_at_ms: u64,
    pub digest: String,
    pub profile_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImageVariantMetadata {
    pub sha256: String,
    pub byte_length: u64,
    pub content_type: ImageMime,
    pub width_px: u32,
    pub height_px: u32,
    pub profile_id: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImageAssetDocument {
    pub schema_version: String,
    pub id: String,
    pub intent_id: String,
    pub work_id: String,
    pub owner_id: String,
    pub verified_media: ImageVariantMetadata,
    pub declarations: ImageDeclarations,
    pub admitted_at_ms: u64,
    pub digest: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ImageIntentResponse {
    pub intent: ImageIntentDocument,
    pub asset: Option<ImageAssetDocument>,
    pub state: ImageIntentState,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkArtworkSelection {
    pub image_asset_id: String,
    pub image_asset_digest: String,
    pub alt_text: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct SetWorkArtworkRequest {
    pub operation_id: String,
    pub expected_work_revision: u64,
    /// Explicit null clears; omission is rejected. Missing intent must never clear a cover.
    #[serde(deserialize_with = "required_nullable")]
    pub artwork: Option<WorkArtworkSelection>,
}

fn required_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct WorkArtworkResponse {
    pub work_id: String,
    pub work_revision: u64,
    pub artwork: Option<WorkArtworkSelection>,
    pub changed_at_ms: Option<u64>,
}
