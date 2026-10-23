//! Bounded immutable image receipt admission. SQL is a second barrier; every document is
//! restored through Media constructors and every indexed fact is cross-checked on read.
use crate::contract::studio as dto;
use crate::platform::store::Row;
use serde::{de::DeserializeOwned, Serialize};

use crate::contexts::catalog::domain::{OwnerId, WorkId};
use crate::contexts::media::domain::image::{
    sha256_valid, ImageAsset, ImageAssetId, ImageIntent, ImageIntentId, MAX_IMAGE_DOCUMENT_BYTES,
};
use crate::platform::store::{checked_number, StoreError, Tx};

#[derive(Clone)]
pub(in crate::contexts::media) struct StoredImageIntent {
    pub intent: ImageIntent,
    pub document_bytes: Vec<u8>,
}
#[derive(Clone)]
pub(in crate::contexts::media) struct StoredImageAsset {
    pub asset: ImageAsset,
    /// These internal strings are checked against the final storage target/key grammar. They
    /// are not a new storage reference capability and never appear in client documents.
    pub storage_target: String,
    pub object_key: String,
    pub document_bytes: Vec<u8>,
}

pub(super) fn corrupt<T>(
    result: Result<T, crate::platform::store::Error>,
) -> Result<T, StoreError> {
    result.map_err(|_| StoreError::CorruptRevision)
}
pub(super) fn document_bytes<T: Serialize>(document: &T) -> Result<Vec<u8>, StoreError> {
    let bytes = serde_json::to_vec(document).map_err(|_| StoreError::CorruptRevision)?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_DOCUMENT_BYTES {
        return Err(StoreError::CorruptRevision);
    }
    Ok(bytes)
}
pub(super) fn decode_document<T: DeserializeOwned + Serialize>(
    bytes: &[u8],
) -> Result<T, StoreError> {
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_DOCUMENT_BYTES {
        return Err(StoreError::CorruptRevision);
    }
    let raw: T = serde_json::from_slice(bytes).map_err(|_| StoreError::CorruptRevision)?;
    if document_bytes(&raw)? != bytes {
        return Err(StoreError::CorruptRevision);
    }
    Ok(raw)
}
pub(super) fn matches(row: &Row, column: &str, value: &str) -> Result<(), StoreError> {
    if corrupt(row.try_get::<_, String>(column))? != value {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}
fn number_matches(row: &Row, column: &str, value: u64) -> Result<(), StoreError> {
    if corrupt(row.try_get::<_, i64>(column))?
        != checked_number(value).map_err(|_| StoreError::CorruptRevision)?
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}
/// Restore the final reviewed shared storage identity; no parallel target grammar.
pub(super) fn target_valid(value: &str) -> bool {
    crate::contexts::media::domain::object_storage::StorageTargetId::parse(value).is_ok()
}
fn insert_error(error: crate::platform::store::Error) -> StoreError {
    if error.constraint() == Some(cantos_store_turso::error::ConstraintKind::Unique) {
        StoreError::OperationReused
    } else {
        StoreError::from(error)
    }
}
fn intent_row(row: &Row) -> Result<StoredImageIntent, StoreError> {
    let bytes: Vec<u8> = corrupt(row.try_get("document_bytes"))?;
    let raw: dto::ImageIntentDocument = decode_document(&bytes)?;
    let intent = ImageIntent::restore(raw).map_err(|_| StoreError::CorruptRevision)?;
    let d = intent.document();
    for (column, value) in [
        ("id", d.id.as_str()),
        ("work_id", d.work_id.as_str()),
        ("owner_id", d.owner_id.as_str()),
        ("operation_id", d.operation_id.as_str()),
        ("expected_sha256", d.request.expected_sha256.as_str()),
        (
            "content_type",
            intent.request().expected().content_type().as_str(),
        ),
        ("profile_id", d.profile_id.as_str()),
        ("digest", d.digest.as_str()),
    ] {
        matches(row, column, value)?;
    }
    number_matches(row, "expected_byte_length", d.request.expected_byte_length)?;
    number_matches(row, "created_at_ms", d.created_at_ms)?;
    number_matches(row, "expires_at_ms", d.expires_at_ms)?;
    Ok(StoredImageIntent {
        intent,
        document_bytes: bytes,
    })
}
fn asset_row(row: &Row) -> Result<StoredImageAsset, StoreError> {
    let bytes: Vec<u8> = corrupt(row.try_get("document_bytes"))?;
    let raw: dto::ImageAssetDocument = decode_document(&bytes)?;
    let asset = ImageAsset::restore(raw).map_err(|_| StoreError::CorruptRevision)?;
    let d = asset.document();
    for (column, value) in [
        ("id", d.id.as_str()),
        ("intent_id", d.intent_id.as_str()),
        ("work_id", d.work_id.as_str()),
        ("owner_id", d.owner_id.as_str()),
        ("sha256", d.verified_media.sha256.as_str()),
        ("content_type", asset.metadata().content_type().as_str()),
        ("profile_id", d.verified_media.profile_id.as_str()),
        ("digest", d.digest.as_str()),
    ] {
        matches(row, column, value)?;
    }
    number_matches(row, "byte_length", d.verified_media.byte_length)?;
    number_matches(row, "admitted_at_ms", d.admitted_at_ms)?;
    for (column, value) in [
        ("width_px", d.verified_media.width_px),
        ("height_px", d.verified_media.height_px),
    ] {
        if corrupt(row.try_get::<_, i32>(column))? != value as i32 {
            return Err(StoreError::CorruptRevision);
        }
    }
    let storage_target: String = corrupt(row.try_get("storage_target"))?;
    let object_key: String = corrupt(row.try_get("object_key"))?;
    if !target_valid(&storage_target)
        || !sha256_valid(&object_key)
        || object_key != asset.metadata().sha256()
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(StoredImageAsset {
        asset,
        storage_target,
        object_key,
        document_bytes: bytes,
    })
}

pub(in crate::contexts::media) async fn load_intent(
    tx: &Tx<'_>,
    owner: &OwnerId,
    work: &WorkId,
    id: &ImageIntentId,
) -> Result<Option<StoredImageIntent>, StoreError> {
    tx.query_opt(
        "SELECT * FROM media_image_intents WHERE owner_id=$1 AND work_id=$2 AND id=$3",
        &[&owner.as_str(), &work.as_str(), &id.as_str()],
    )
    .await?
    .as_ref()
    .map(intent_row)
    .transpose()
}
pub(in crate::contexts::media) async fn intent_id_exists(
    tx: &Tx<'_>,
    id: &ImageIntentId,
) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt(
            "SELECT id FROM media_image_intents WHERE id=$1",
            &[&id.as_str()],
        )
        .await?
        .is_some())
}
pub(in crate::contexts::media) async fn insert_intent(
    tx: &Tx<'_>,
    value: &ImageIntent,
) -> Result<Vec<u8>, StoreError> {
    let d = value.document();
    let bytes = document_bytes(d)?;
    tx.execute(
        "INSERT INTO media_image_intents(id,work_id,owner_id,operation_id,expected_sha256,expected_byte_length,content_type,profile_id,created_at_ms,expires_at_ms,document_bytes,digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12)",
        &[&d.id,&d.work_id,&d.owner_id,&d.operation_id,&d.request.expected_sha256,
          &checked_number(d.request.expected_byte_length)?,&value.request().expected().content_type().as_str(),
          &d.profile_id,&value.created_at_ms(),&value.expires_at_ms(),&bytes,&d.digest],
    ).await.map_err(insert_error)?;
    Ok(bytes)
}
pub(in crate::contexts::media) async fn load_asset(
    tx: &Tx<'_>,
    intent: &ImageIntent,
) -> Result<Option<StoredImageAsset>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT * FROM media_image_assets WHERE intent_id=$1 AND work_id=$2 AND owner_id=$3",
            &[
                &intent.id().as_str(),
                &intent.work_id().as_str(),
                &intent.owner_id().as_str(),
            ],
        )
        .await?;
    row.as_ref()
        .map(|row| {
            let asset = asset_row(row)?;
            asset
                .asset
                .require_intent(intent)
                .map_err(|_| StoreError::CorruptRevision)?;
            Ok(asset)
        })
        .transpose()
}
pub(in crate::contexts::media) async fn load_asset_by_id(
    tx: &Tx<'_>,
    owner: &OwnerId,
    work: &WorkId,
    id: &ImageAssetId,
) -> Result<Option<StoredImageAsset>, StoreError> {
    let row = tx
        .query_opt(
            "SELECT * FROM media_image_assets WHERE owner_id=$1 AND work_id=$2 AND id=$3",
            &[&owner.as_str(), &work.as_str(), &id.as_str()],
        )
        .await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let asset = asset_row(&row)?;
    let intent = load_intent(tx, owner, work, asset.asset.intent_id())
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    asset
        .asset
        .require_intent(&intent.intent)
        .map_err(|_| StoreError::CorruptRevision)?;
    Ok(Some(asset))
}
pub(in crate::contexts::media) async fn insert_asset(
    tx: &Tx<'_>,
    value: &ImageAsset,
    storage_target: &str,
    object_key: &str,
) -> Result<Vec<u8>, StoreError> {
    if !target_valid(storage_target)
        || !sha256_valid(object_key)
        || object_key != value.metadata().sha256()
    {
        return Err(StoreError::CorruptRevision);
    }
    let d = value.document();
    let bytes = document_bytes(d)?;
    tx.execute(
        "INSERT INTO media_image_assets(id,intent_id,work_id,owner_id,storage_target,object_key,sha256,byte_length,content_type,width_px,height_px,profile_id,admitted_at_ms,document_bytes,digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15)",
        &[&d.id,&d.intent_id,&d.work_id,&d.owner_id,&storage_target,&object_key,&d.verified_media.sha256,
          &checked_number(d.verified_media.byte_length)?,&value.metadata().content_type().as_str(),
          &(d.verified_media.width_px as i32),&(d.verified_media.height_px as i32),
          &d.verified_media.profile_id,&value.admitted_at_ms(),&bytes,&d.digest],
    ).await.map_err(insert_error)?;
    Ok(bytes)
}
