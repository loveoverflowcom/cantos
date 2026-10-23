//! Immutable upload intents and acceptance receipts. PostgreSQL owns their binding and status;
//! verified private object bytes remain outside the database.
use crate::platform::store::Row;
use crate::contract::studio::{AudioIntentResponse, AudioReceiptResponse};
use sha2::{Digest, Sha256};

use super::audio_intake::DeliveryProfile;
use super::domain::object_storage::{
    ObjectIntegrity, ObjectKey, PrivateObjectRef, StorageTargetId,
};
use super::storage::registry::RegistryStoredObject;
use crate::platform::store::{normalized_uuid, StoreError, Tx};

const INTENT_COLUMNS: &str =
    "id,script_id,script_revision,content_digest,profile_id,operation_id,created_by,created_at";

fn decode_intent(row: &Row) -> Result<AudioIntentResponse, StoreError> {
    let id: String = row.get("id");
    let script_id: String = row.get("script_id");
    let operation_id: String = row.get("operation_id");
    for value in [&id, &script_id, &operation_id] {
        if normalized_uuid(value).map_err(|_| StoreError::CorruptRevision)? != *value {
            return Err(StoreError::CorruptRevision);
        }
    }
    let profile_id: String = row.get("profile_id");
    if DeliveryProfile::from_id(&profile_id).is_none() {
        return Err(StoreError::CorruptRevision);
    }
    let script_revision = u64::try_from(row.get::<_, i64>("script_revision"))
        .map_err(|_| StoreError::CorruptRevision)?;
    if script_revision == 0 {
        return Err(StoreError::CorruptRevision);
    }
    let status_path = format!("/api/v1/studio/scripts/{script_id}/audio-intents/{id}");
    Ok(AudioIntentResponse {
        id,
        script_id,
        script_revision,
        content_digest: row.get("content_digest"),
        profile_id,
        operation_id,
        created_by: row.get("created_by"),
        created_at: crate::platform::store::utc_timestamp(row.try_get("created_at")?)?,
        upload_path: format!("{status_path}/bytes"),
        status_path,
    })
}

pub(super) async fn actor_active(tx: &Tx<'_>, actor: &str) -> Result<bool, StoreError> {
    crate::platform::store::actor_active(tx, actor).await
}

pub(super) async fn by_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<AudioIntentResponse>, StoreError> {
    let query = format!(
        "SELECT {INTENT_COLUMNS} FROM audio_upload_intents WHERE created_by=?1 AND operation_id=?2"
    );
    tx.query_opt(&query, &[&actor, &operation])
        .await?
        .as_ref()
        .map(decode_intent)
        .transpose()
}

pub(super) async fn find_intent(
    tx: &Tx<'_>,
    actor: &str,
    script: &str,
    id: &str,
) -> Result<Option<AudioIntentResponse>, StoreError> {
    let query = format!("SELECT {INTENT_COLUMNS} FROM audio_upload_intents WHERE created_by=?1 AND script_id=?2 AND id=?3");
    tx.query_opt(&query, &[&actor, &script, &id])
        .await?
        .as_ref()
        .map(decode_intent)
        .transpose()
}

pub(super) async fn insert_intent(
    tx: &Tx<'_>,
    actor: &str,
    binding: super::domain::intake::IntentBinding<'_>,
    operation: &str,
    id: &str,
) -> Result<AudioIntentResponse, StoreError> {
    let script = binding.script_id;
    let revision = crate::platform::store::checked_number(binding.revision)?;
    let digest = binding.content_digest;
    let profile = binding.profile_id;
    let query = format!("INSERT INTO audio_upload_intents(id,script_id,script_revision,content_digest,profile_id,operation_id,created_by,created_at) VALUES(?1,?2,?3,?4,?5,?6,?7,?8) RETURNING {INTENT_COLUMNS}");
    let row = tx
        .query_one(
            &query,
            &[
                &id,
                &script,
                &revision,
                &digest,
                &profile,
                &operation,
                &actor,
                &tx.now_micros(),
            ],
        )
        .await
        .map_err(|error| {
            if error.constraint() == Some(cantos_store_turso::error::ConstraintKind::Unique) {
                StoreError::OperationReused
            } else {
                StoreError::from(error)
            }
        })?;
    decode_intent(&row)
}

pub(super) async fn now(tx: &Tx<'_>) -> Result<String, StoreError> {
    crate::platform::store::utc_timestamp(tx.now_micros())
}

pub(super) fn receipt_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"cantos/audio-intake-receipt/r1\n");
    digest.update(bytes);
    format!("audio-intake-r1:sha256:{:x}", digest.finalize())
}

pub(super) struct StoredReceipt {
    pub receipt: AudioReceiptResponse,
    object: PrivateObjectRef,
}

impl StoredReceipt {
    pub(super) fn location(&self) -> &PrivateObjectRef {
        &self.object
    }
}

pub(super) async fn receipt(
    tx: &Tx<'_>,
    intent: &AudioIntentResponse,
) -> Result<Option<StoredReceipt>, StoreError> {
    let row = tx.query_opt("SELECT id,intent_id,script_id,script_revision,content_digest,profile_id,operation_id,accepted_by,accepted_at,object_key,storage_target_id,receipt_bytes,receipt_digest FROM audio_intake_receipts WHERE intent_id=?1", &[&intent.id]).await?;
    let Some(row) = row else {
        return Ok(None);
    };
    let bytes: Vec<u8> = row.get("receipt_bytes");
    if receipt_digest(&bytes) != row.get::<_, String>("receipt_digest") {
        return Err(StoreError::CorruptRevision);
    }
    let receipt: AudioReceiptResponse =
        serde_json::from_slice(&bytes).map_err(|_| StoreError::CorruptRevision)?;
    let object_key: String = row.get("object_key");
    let profile =
        DeliveryProfile::from_id(&receipt.profile_id).ok_or(StoreError::CorruptRevision)?;
    if serde_json::to_vec(&receipt).map_err(|_| StoreError::CorruptRevision)? != bytes
        || normalized_uuid(&receipt.id).map_err(|_| StoreError::CorruptRevision)? != receipt.id
        || receipt.id != row.get::<_, String>("id")
        || receipt.intent_id != intent.id
        || receipt.intent_id != row.get::<_, String>("intent_id")
        || receipt.script_id != intent.script_id
        || receipt.script_id != row.get::<_, String>("script_id")
        || receipt.script_revision != intent.script_revision
        || crate::platform::store::checked_number(receipt.script_revision)
            .map_err(|_| StoreError::CorruptRevision)?
            != row.get::<_, i64>("script_revision")
        || receipt.content_digest != intent.content_digest
        || receipt.content_digest != row.get::<_, String>("content_digest")
        || receipt.profile_id != intent.profile_id
        || receipt.profile_id != row.get::<_, String>("profile_id")
        || receipt.operation_id != intent.operation_id
        || receipt.operation_id != row.get::<_, String>("operation_id")
        || receipt.accepted_by != intent.created_by
        || receipt.accepted_by != row.get::<_, String>("accepted_by")
        || receipt.accepted_at
            != crate::platform::store::utc_timestamp(row.try_get("accepted_at")?)?
        || receipt.media.profile_id != intent.profile_id
        || receipt.media.sha256 != object_key
        || receipt.media.sha256.len() != 64
        || !receipt
            .media
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
        || receipt.media.container != profile.container()
        || receipt.media.codec != profile.codec()
        || receipt.media.audio_bitrate_bps == 0
        || receipt.media.audio_bitrate_bps > profile.maximum_bitrate_bps()
        || receipt.media.byte_length == 0
        || usize::try_from(receipt.media.byte_length).map_err(|_| StoreError::CorruptRevision)?
            > super::audio_intake::MAX_AUDIO_BYTES
        || receipt.media.channels != 1
        || receipt.media.sample_rate_hz != 48_000
        || receipt.media.duration_ms == 0
        || receipt.media.duration_ms > 3_600_000
    {
        return Err(StoreError::CorruptRevision);
    }
    let target: String = row
        .try_get("storage_target_id")
        .map_err(|_| StoreError::CorruptRevision)?;
    let target = StorageTargetId::parse(&target).map_err(|_| StoreError::CorruptRevision)?;
    let key = ObjectKey::parse(&object_key).map_err(|_| StoreError::CorruptRevision)?;
    let integrity = ObjectIntegrity::new(&receipt.media.sha256, receipt.media.byte_length)
        .map_err(|_| StoreError::CorruptRevision)?;
    let object =
        PrivateObjectRef::new(target, key, integrity).map_err(|_| StoreError::CorruptRevision)?;
    Ok(Some(StoredReceipt { receipt, object }))
}

pub(super) async fn insert_receipt(
    tx: &Tx<'_>,
    receipt: &AudioReceiptResponse,
    stored: &RegistryStoredObject,
) -> Result<(), StoreError> {
    if stored.object_key != receipt.media.sha256 {
        return Err(StoreError::CorruptRevision);
    }
    let object_key = stored.object_key.as_str();
    let target = stored.storage_target.as_str();
    let bytes = serde_json::to_vec(receipt).map_err(|_| StoreError::CorruptRevision)?;
    let digest = receipt_digest(&bytes);
    let revision =
        i64::try_from(receipt.script_revision).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute("INSERT INTO audio_intake_receipts(id,intent_id,script_id,script_revision,content_digest,profile_id,operation_id,accepted_by,accepted_at,object_key,storage_target_id,receipt_bytes,receipt_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)", &[&receipt.id, &receipt.intent_id, &receipt.script_id, &revision, &receipt.content_digest, &receipt.profile_id, &receipt.operation_id, &receipt.accepted_by, &tx.now_micros(), &object_key, &target, &bytes, &digest]).await?;
    Ok(())
}

pub(super) mod lifecycle;

pub(super) mod publication;

pub(in crate::contexts::media) mod artwork;
pub(in crate::contexts::media) mod image;

pub(super) mod listener;

#[cfg(test)]
mod tests;
