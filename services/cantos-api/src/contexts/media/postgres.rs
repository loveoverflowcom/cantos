//! Immutable upload intents and acceptance receipts. PostgreSQL owns their binding and status;
//! verified private object bytes remain outside the database.
use cantos_contracts::{AudioIntentResponse, AudioReceiptResponse};
use sha2::{Digest, Sha256};
use tokio_postgres::{error::SqlState, Row};

use super::audio_intake::DeliveryProfile;
use crate::platform::postgres::{normalized_uuid, StoreError, Tx};

const INTENT_COLUMNS: &str = "id,script_id,script_revision,content_digest,profile_id,operation_id,created_by,to_char(created_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS created_at";

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
        created_at: row.get("created_at"),
        upload_path: format!("{status_path}/bytes"),
        status_path,
    })
}

pub(super) async fn actor_active(tx: &Tx<'_>, actor: &str) -> Result<bool, StoreError> {
    Ok(tx
        .query_opt("SELECT production_lock_actor($1) AS active", &[&actor])
        .await?
        .and_then(|row| row.get::<_, Option<bool>>("active"))
        .unwrap_or(false))
}

pub(super) async fn by_operation(
    tx: &Tx<'_>,
    actor: &str,
    operation: &str,
) -> Result<Option<AudioIntentResponse>, StoreError> {
    let query = format!("SELECT {INTENT_COLUMNS} FROM audio_upload_intents WHERE created_by=$1 AND operation_id=$2");
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
    let query = format!("SELECT {INTENT_COLUMNS} FROM audio_upload_intents WHERE created_by=$1 AND script_id=$2 AND id=$3");
    tx.query_opt(&query, &[&actor, &script, &id])
        .await?
        .as_ref()
        .map(decode_intent)
        .transpose()
}

pub(super) async fn insert_intent(
    tx: &Tx<'_>,
    actor: &str,
    script: &str,
    revision: i64,
    digest: &str,
    profile: &str,
    operation: &str,
    id: &str,
) -> Result<AudioIntentResponse, StoreError> {
    let query = format!("INSERT INTO audio_upload_intents(id,script_id,script_revision,content_digest,profile_id,operation_id,created_by) VALUES($1,$2,$3,$4,$5,$6,$7) RETURNING {INTENT_COLUMNS}");
    let row = tx
        .query_one(&query, &[&id, &script, &revision, &digest, &profile, &operation, &actor])
        .await
        .map_err(|error| {
            if error.code() == Some(&SqlState::UNIQUE_VIOLATION) {
                StoreError::OperationReused
            } else {
                StoreError::from(error)
            }
        })?;
    decode_intent(&row)
}

pub(super) async fn now(tx: &Tx<'_>) -> Result<String, StoreError> {
    Ok(tx.query_one("SELECT to_char(clock_timestamp() AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"')", &[]).await?.get(0))
}

pub(super) fn receipt_digest(bytes: &[u8]) -> String {
    let mut digest = Sha256::new();
    digest.update(b"cantos/audio-intake-receipt/r1\n");
    digest.update(bytes);
    format!("audio-intake-r1:sha256:{:x}", digest.finalize())
}

pub(super) struct StoredReceipt {
    pub receipt: AudioReceiptResponse,
    pub object_key: String,
}

pub(super) async fn receipt(
    tx: &Tx<'_>,
    intent: &AudioIntentResponse,
) -> Result<Option<StoredReceipt>, StoreError> {
    let row = tx.query_opt("SELECT id,intent_id,script_id,script_revision,content_digest,profile_id,operation_id,accepted_by,to_char(accepted_at AT TIME ZONE 'UTC', 'YYYY-MM-DD\"T\"HH24:MI:SS.US\"Z\"') AS accepted_at,object_key,receipt_bytes,receipt_digest FROM audio_intake_receipts WHERE intent_id=$1", &[&intent.id]).await?;
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
    let profile = DeliveryProfile::from_id(&receipt.profile_id)
        .ok_or(StoreError::CorruptRevision)?;
    if serde_json::to_vec(&receipt).map_err(|_| StoreError::CorruptRevision)? != bytes
        || normalized_uuid(&receipt.id).map_err(|_| StoreError::CorruptRevision)? != receipt.id
        || receipt.id != row.get::<_, String>("id")
        || receipt.intent_id != intent.id
        || receipt.intent_id != row.get::<_, String>("intent_id")
        || receipt.script_id != intent.script_id
        || receipt.script_id != row.get::<_, String>("script_id")
        || receipt.script_revision != intent.script_revision
        || receipt.script_revision as i64 != row.get::<_, i64>("script_revision")
        || receipt.content_digest != intent.content_digest
        || receipt.content_digest != row.get::<_, String>("content_digest")
        || receipt.profile_id != intent.profile_id
        || receipt.profile_id != row.get::<_, String>("profile_id")
        || receipt.operation_id != intent.operation_id
        || receipt.operation_id != row.get::<_, String>("operation_id")
        || receipt.accepted_by != intent.created_by
        || receipt.accepted_by != row.get::<_, String>("accepted_by")
        || receipt.accepted_at != row.get::<_, String>("accepted_at")
        || receipt.media.profile_id != intent.profile_id
        || receipt.media.sha256 != object_key
        || receipt.media.sha256.len() != 64
        || !receipt.media.sha256.bytes().all(|byte| {
            byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)
        })
        || receipt.media.container != profile.container()
        || receipt.media.codec != profile.codec()
        || receipt.media.audio_bitrate_bps == 0
        || receipt.media.audio_bitrate_bps > profile.maximum_bitrate_bps()
        || receipt.media.byte_length == 0
        || receipt.media.byte_length > super::audio_intake::MAX_AUDIO_BYTES as u64
        || receipt.media.channels != 1
        || receipt.media.sample_rate_hz != 48_000
        || receipt.media.duration_ms == 0
        || receipt.media.duration_ms > 3_600_000
    {
        return Err(StoreError::CorruptRevision);
    }
    Ok(Some(StoredReceipt {
        receipt,
        object_key,
    }))
}

pub(super) async fn insert_receipt(
    tx: &Tx<'_>,
    receipt: &AudioReceiptResponse,
    object_key: &str,
) -> Result<(), StoreError> {
    let bytes = serde_json::to_vec(receipt).map_err(|_| StoreError::CorruptRevision)?;
    let digest = receipt_digest(&bytes);
    let revision = i64::try_from(receipt.script_revision).map_err(|_| StoreError::CorruptRevision)?;
    tx.execute("INSERT INTO audio_intake_receipts(id,intent_id,script_id,script_revision,content_digest,profile_id,operation_id,accepted_by,accepted_at,object_key,receipt_bytes,receipt_digest) VALUES($1,$2,$3,$4,$5,$6,$7,$8,$9::text::timestamptz,$10,$11,$12)", &[&receipt.id, &receipt.intent_id, &receipt.script_id, &revision, &receipt.content_digest, &receipt.profile_id, &receipt.operation_id, &receipt.accepted_by, &receipt.accepted_at, &object_key, &bytes, &digest]).await?;
    Ok(())
}
