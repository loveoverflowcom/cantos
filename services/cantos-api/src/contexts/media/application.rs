//! Owner-session private audio intake. Inspection and new object writes finish outside any
//! database transaction; the final transaction rechecks the session, owner and exact revision.
use cantos_contracts::{
    AudioIntentResponse, AudioIntentStatusResponse, AudioMediaMetadata, AudioReceiptResponse,
    CreateAudioIntentRequest, FieldIssue,
};
use sha2::{Digest, Sha256};
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::audio_intake::{
    inspect_audio, AudioIntakeError, DeliveryProfile, VerifiedAudio, MAX_AUDIO_BYTES,
};
use super::domain::intake::{decide_prepare, IntentBinding, PrepareDecision};
use super::postgres;
use super::storage::AudioStorage;
use crate::contexts::editorial::{authorize, find_revision, Action};
use crate::platform::postgres::{
    authenticate, checked_number, normalized_uuid, LockMode, Store, StoreError, Tx,
};

// Fail fast when the bounded inspector lanes are occupied. The permit is owned by the blocking
// task, so HTTP cancellation cannot release a lane while its subprocess still runs.
static INSPECTION_LANES: Semaphore = Semaphore::const_new(2);

async fn owner(
    tx: &Tx<'_>,
    token: &str,
    script: &str,
    lock: LockMode,
) -> Result<(String, u64), StoreError> {
    let actor = authenticate(tx, token).await?;
    let (_, head) = authorize(tx, &actor, script, Action::Review, lock).await?;
    if !postgres::actor_active(tx, &actor).await? {
        return Err(StoreError::Unauthenticated);
    }
    Ok((actor, head))
}

async fn bound_revision(tx: &Tx<'_>, intent: &AudioIntentResponse) -> Result<(), StoreError> {
    let revision = find_revision(tx, &intent.script_id, checked_number(intent.script_revision)?)
        .await?
        .ok_or(StoreError::CorruptRevision)?;
    if revision.content_digest != intent.content_digest {
        return Err(StoreError::CorruptRevision);
    }
    Ok(())
}

fn current(intent: &AudioIntentResponse, head: u64) -> Result<(), StoreError> {
    if intent.script_revision != head {
        return Err(StoreError::StaleRevision(head));
    }
    Ok(())
}

fn storage(store: &Store) -> Result<AudioStorage, StoreError> {
    AudioStorage::new(store.audio_root.as_deref().ok_or(StoreError::Unavailable)?)
}

async fn verify_object(
    storage: AudioStorage,
    receipt: &postgres::StoredReceipt,
) -> Result<(), StoreError> {
    let key = receipt.object_key.clone();
    let digest = receipt.receipt.media.sha256.clone();
    let length = receipt.receipt.media.byte_length;
    tokio::task::spawn_blocking(move || storage.verify(&key, &digest, length))
        .await
        .map_err(|_| StoreError::Unavailable)?
}

fn media(verified: &VerifiedAudio) -> AudioMediaMetadata {
    let facts = verified.metadata();
    AudioMediaMetadata {
        sha256: facts.sha256.clone(),
        byte_length: facts.byte_length,
        container: facts.container.clone(),
        codec: facts.codec.clone(),
        channels: facts.channels,
        sample_rate_hz: facts.sample_rate_hz,
        duration_ms: facts.duration_ms,
        audio_bitrate_bps: facts.audio_bitrate_bps,
        profile_id: facts.profile_id.clone(),
    }
}

fn rejected(error: AudioIntakeError) -> StoreError {
    if error.is_unavailable() {
        StoreError::AudioInspectionUnavailable(error.code())
    } else {
        StoreError::AudioRejected(error.code())
    }
}

impl Store {
    /// Opt in to an owned, private development object root. Existing editorial APIs do not
    /// require one; private intake writes fail closed when it is absent.
    pub fn with_audio_storage(mut self, root: &std::path::Path) -> Result<Self, StoreError> {
        AudioStorage::new(root)?;
        self.audio_root = Some(root.to_path_buf());
        Ok(self)
    }

    pub fn audio_upload_body_limit() -> usize {
        MAX_AUDIO_BYTES
    }

    /// Enabling private storage makes the development API host API-only. Static Web files
    /// must be served by a separate host whose public tree cannot reference private objects.
    pub fn audio_storage_is_enabled(&self) -> bool {
        self.audio_root.is_some()
    }

    /// The script owner pins one immutable, canonical profile separately from its byte upload.
    /// No producer session, caller-authored policy or default profile is accepted.
    pub async fn create_audio_intent(
        &self,
        token: &str,
        script: &str,
        request: CreateAudioIntentRequest,
    ) -> Result<AudioIntentResponse, StoreError> {
        storage(self)?;
        let script = normalized_uuid(script)?;
        let operation = normalized_uuid(&request.operation_id)?;
        let revision = checked_number(request.expected_revision)?;
        if revision < 1 {
            return Err(StoreError::InvalidRequest);
        }
        let profile = DeliveryProfile::from_id(&request.profile_id).ok_or_else(|| {
            StoreError::InvalidRequestFields(vec![FieldIssue {
                path: "profile_id".into(),
                rule: "unknown_delivery_profile".into(),
            }])
        })?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (actor, head) = owner(&tx, token, &script, LockMode::Update).await?;
        let prior = postgres::by_operation(&tx, &actor, &operation).await?;
        let pinned = find_revision(&tx, &script, checked_number(head)?)
            .await?
            .ok_or(StoreError::CorruptRevision)?;
        let requested = IntentBinding {
            script_id: &script,
            revision: request.expected_revision,
            content_digest: &request.expected_content_digest,
            profile_id: profile.id(),
        };
        let prior_binding = prior.as_ref().map(|prior| IntentBinding {
            script_id: &prior.script_id,
            revision: prior.script_revision,
            content_digest: &prior.content_digest,
            profile_id: &prior.profile_id,
        });
        match decide_prepare(head, &pinned.content_digest, requested, prior_binding) {
            PrepareDecision::Replay => {
                let prior = prior.ok_or(StoreError::CorruptRevision)?;
                bound_revision(&tx, &prior).await?;
                authenticate(&tx, token).await?;
                tx.commit().await?;
                return Ok(prior);
            }
            PrepareDecision::Create => (),
            PrepareDecision::OperationReused => return Err(StoreError::OperationReused),
            PrepareDecision::StaleRevision(head) => return Err(StoreError::StaleRevision(head)),
            PrepareDecision::ContentDigestMismatch => {
                return Err(StoreError::InvalidRequestFields(vec![FieldIssue {
                    path: "expected_content_digest".into(),
                    rule: "content_digest_mismatch".into(),
                }]))
            }
        }
        let result = postgres::insert_intent(
            &tx,
            &actor,
            &script,
            revision,
            &pinned.content_digest,
            profile.id(),
            &operation,
            &Uuid::new_v4().to_string(),
        )
        .await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(result)
    }

    /// Status always reapplies current owner/session access, but can describe a historical
    /// accepted revision. It never exposes an object path, signed URL or private media bytes.
    pub async fn load_audio_intent(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<AudioIntentStatusResponse, StoreError> {
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (actor, _) = owner(&tx, token, &script, LockMode::Share).await?;
        let intent = postgres::find_intent(&tx, &actor, &script, &id)
            .await?
            .ok_or(StoreError::NotFound)?;
        bound_revision(&tx, &intent).await?;
        let stored = postgres::receipt(&tx, &intent).await?;
        if let Some(receipt) = &stored {
            verify_object(storage(self)?, receipt).await?;
        }
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(AudioIntentStatusResponse {
            intent,
            receipt: stored.map(|stored| stored.receipt),
        })
    }

    /// Acceptance is a single immutable metadata commit only after actual bytes passed the
    /// pinned inspector and were durably published in private storage. A failed metadata commit
    /// may leave a private orphan; it never deletes a concurrently referenced content object.
    pub async fn upload_audio_intent(
        &self,
        token: &str,
        script: &str,
        id: &str,
        bytes: Vec<u8>,
    ) -> Result<AudioReceiptResponse, StoreError> {
        let objects = storage(self)?;
        if bytes.len() > MAX_AUDIO_BYTES {
            return Err(rejected(AudioIntakeError::AudioTooLarge));
        }
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (actor, head) = owner(&tx, token, &script, LockMode::Share).await?;
        let intent = postgres::find_intent(&tx, &actor, &script, &id)
            .await?
            .ok_or(StoreError::NotFound)?;
        bound_revision(&tx, &intent).await?;
        current(&intent, head)?;
        // Replaying settled bytes needs no decoder invocation, but still verifies the stored
        // object and current authority under the same locked script/session facts.
        if let Some(prior) = postgres::receipt(&tx, &intent).await? {
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            if prior.receipt.media.sha256 != sha256
                || prior.receipt.media.byte_length != bytes.len() as u64
            {
                return Err(StoreError::OperationReused);
            }
            verify_object(objects, &prior).await?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.receipt);
        }
        tx.commit().await?;
        drop(client);

        let profile = DeliveryProfile::from_id(&intent.profile_id)
            .ok_or(StoreError::CorruptRevision)?;
        let permit = INSPECTION_LANES
            .try_acquire()
            .map_err(|_| StoreError::Unavailable)?;
        let (media, stored) = tokio::task::spawn_blocking(move || {
            let _permit = permit;
            let verified = inspect_audio(&bytes, profile).map_err(rejected)?;
            let media = media(&verified);
            let stored = objects.persist(&verified)?;
            Ok::<_, StoreError>((media, stored))
        })
        .await
        .map_err(|_| StoreError::Unavailable)??;

        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (current_actor, head) = owner(&tx, token, &script, LockMode::Update).await?;
        if current_actor != actor {
            return Err(StoreError::Unauthenticated);
        }
        let locked = postgres::find_intent(&tx, &current_actor, &script, &id)
            .await?
            .ok_or(StoreError::NotFound)?;
        if locked != intent {
            return Err(StoreError::CorruptRevision);
        }
        bound_revision(&tx, &locked).await?;
        current(&locked, head)?;
        if let Some(prior) = postgres::receipt(&tx, &locked).await? {
            if prior.receipt.media != media || prior.object_key != stored.object_key {
                return Err(StoreError::OperationReused);
            }
            verify_object(storage(self)?, &prior).await?;
            authenticate(&tx, token).await?;
            tx.commit().await?;
            return Ok(prior.receipt);
        }
        let receipt = AudioReceiptResponse {
            id: Uuid::new_v4().to_string(),
            intent_id: locked.id,
            script_id: locked.script_id,
            script_revision: locked.script_revision,
            content_digest: locked.content_digest,
            profile_id: locked.profile_id,
            operation_id: locked.operation_id,
            accepted_by: current_actor,
            accepted_at: postgres::now(&tx).await?,
            media,
        };
        postgres::insert_receipt(&tx, &receipt, &stored.object_key).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(receipt)
    }
}
