//! Owner-session private audio intake. Inspection and new object writes finish outside any
//! database transaction; the final transaction rechecks the session, owner and exact revision.
use crate::contract::studio::{
    AudioIntentResponse, AudioIntentStatusResponse, AudioMediaMetadata, AudioReceiptResponse,
    CreateAudioIntentRequest, FieldIssue,
};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use tokio::sync::Semaphore;
use uuid::Uuid;

use super::audio_intake::{
    inspect_audio, AudioIntakeError, DeliveryProfile, VerifiedAudio, MAX_AUDIO_BYTES,
};
use super::domain::intake::{decide_prepare, IntentBinding, PrepareDecision};
use super::domain::object_storage::{decide_owner_read, PrivateObjectRef};
use super::postgres;
use super::storage::registry::{RegistryStoredObject, StorageRegistry};
use crate::contexts::catalog::domain::OwnerId;
use crate::contexts::editorial::{authorize, find_revision, Action};
use crate::platform::store::{
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
    let revision = find_revision(
        tx,
        &intent.script_id,
        checked_number(intent.script_revision)?,
    )
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

pub(super) fn storage(store: &Store) -> Result<Arc<StorageRegistry>, StoreError> {
    store.media_storage.clone().ok_or(StoreError::Unavailable)
}

async fn reauthorize_intent(
    store: &Store,
    token: &str,
    intent: &AudioIntentResponse,
    accepted: Option<&postgres::StoredReceipt>,
    require_current: bool,
) -> Result<(), StoreError> {
    let mut client = store.connection().await?;
    let tx = client.transaction().await?;
    let (actor, head) = owner(&tx, token, &intent.script_id, LockMode::Share).await?;
    let locked = postgres::find_intent(&tx, &actor, &intent.script_id, &intent.id)
        .await?
        .ok_or(StoreError::NotFound)?;
    if locked != *intent {
        return Err(StoreError::CorruptRevision);
    }
    bound_revision(&tx, &locked).await?;
    if require_current {
        current(&locked, head)?;
    }
    if let Some(expected) = accepted {
        let settled = postgres::receipt(&tx, &locked)
            .await?
            .ok_or(StoreError::CorruptRevision)?;
        if settled.receipt != expected.receipt || settled.location() != expected.location() {
            return Err(StoreError::CorruptRevision);
        }
    }
    authenticate(&tx, token).await?;
    tx.commit().await?;
    Ok(())
}

/// A settled replay resolves its recorded target, verifies outside SQL, then rechecks authority.
async fn verified_receipt(
    store: &Store,
    token: &str,
    intent: &AudioIntentResponse,
    accepted: postgres::StoredReceipt,
) -> Result<AudioReceiptResponse, StoreError> {
    storage(store)?.verify(accepted.location()).await?;
    reauthorize_intent(store, token, intent, Some(&accepted), true).await?;
    Ok(accepted.receipt)
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
        self.media_storage = Some(Arc::new(StorageRegistry::filesystem(root)?));
        Ok(self)
    }

    /// Checked topology and secret-variable references select adapters only at composition.
    pub fn with_storage_config(mut self, path: &std::path::Path) -> Result<Self, StoreError> {
        self.media_storage = Some(Arc::new(StorageRegistry::from_config(path)?));
        Ok(self)
    }

    pub fn audio_upload_body_limit() -> usize {
        MAX_AUDIO_BYTES
    }

    /// Enabling private storage makes the development API host API-only. Static Web files
    /// must be served by a separate host whose public tree cannot reference private objects.
    pub fn audio_storage_is_enabled(&self) -> bool {
        self.media_storage.is_some()
    }

    /// Resolve an owner's exact accepted historical receipt. This grants no publication or
    /// producer access and performs no object I/O while the authorization transaction is open.
    pub(crate) async fn audio_intent_object(
        &self,
        token: &str,
        script: &str,
        id: &str,
    ) -> Result<PrivateObjectRef, StoreError> {
        let script = normalized_uuid(script)?;
        let id = normalized_uuid(id)?;
        let mut client = self.connection().await?;
        let tx = client.transaction().await?;
        let (actor, _) = owner(&tx, token, &script, LockMode::Share).await?;
        let intent = postgres::find_intent(&tx, &actor, &script, &id)
            .await?
            .ok_or(StoreError::NotFound)?;
        let actor = OwnerId::parse(&actor).map_err(|_| StoreError::CorruptRevision)?;
        let resource_owner =
            OwnerId::parse(&intent.created_by).map_err(|_| StoreError::CorruptRevision)?;
        decide_owner_read(&actor, &resource_owner).map_err(StoreError::Storage)?;
        bound_revision(&tx, &intent).await?;
        let receipt = postgres::receipt(&tx, &intent)
            .await?
            .ok_or(StoreError::NotFound)?;
        let object = receipt.location().clone();
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(object)
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
        if checked_number(request.expected_revision)? < 1 {
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
            requested,
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
        authenticate(&tx, token).await?;
        tx.commit().await?;
        drop(client);
        if let Some(receipt) = &stored {
            storage(self)?.verify(receipt.location()).await?;
        }
        reauthorize_intent(self, token, &intent, stored.as_ref(), false).await?;
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
        // A settled replay needs no decoder. Release SQL locks before storage verification,
        // then recheck the same immutable intent and current session/revision before returning.
        if let Some(prior) = postgres::receipt(&tx, &intent).await? {
            let sha256 = format!("{:x}", Sha256::digest(&bytes));
            if prior.receipt.media.sha256 != sha256
                || prior.receipt.media.byte_length != bytes.len() as u64
            {
                return Err(StoreError::OperationReused);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            drop(client);
            return verified_receipt(self, token, &intent, prior).await;
        }
        tx.commit().await?;
        drop(client);

        let profile =
            DeliveryProfile::from_id(&intent.profile_id).ok_or(StoreError::CorruptRevision)?;
        let (media, stored) = inspect_and_store(objects, bytes, profile).await?;

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
            if prior.receipt.media != media || prior.location().key().as_str() != stored.object_key
            {
                return Err(StoreError::OperationReused);
            }
            authenticate(&tx, token).await?;
            tx.commit().await?;
            drop(client);
            return verified_receipt(self, token, &locked, prior).await;
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
        postgres::insert_receipt(&tx, &receipt, &stored).await?;
        authenticate(&tx, token).await?;
        tx.commit().await?;
        Ok(receipt)
    }
}

/// One inspector/storage path shared by legacy Script intake and Content/Brief admission.
pub(super) async fn inspect_and_store(
    objects: Arc<StorageRegistry>,
    bytes: Vec<u8>,
    profile: DeliveryProfile,
) -> Result<(AudioMediaMetadata, RegistryStoredObject), StoreError> {
    let permit = INSPECTION_LANES
        .try_acquire()
        .map_err(|_| StoreError::Unavailable)?;
    let (verified, permit) = tokio::task::spawn_blocking(move || {
        // Returning the permit keeps canceled requests from releasing a decoder lane early.
        let verified = inspect_audio(&bytes, profile).map_err(rejected)?;
        Ok::<_, StoreError>((verified, permit))
    })
    .await
    .map_err(|_| StoreError::Unavailable)??;
    // Filesystem persistence may continue in a blocking task after HTTP cancellation. This
    // owned operation retains the lane until the bounded adapter finishes, just as inspection
    // does. Cancellation can leave a private orphan, but cannot accept metadata or free a lane
    // while its object I/O still runs.
    tokio::spawn(async move {
        let _permit = permit;
        let metadata = media(&verified);
        let stored = objects.persist_audio(verified).await?;
        Ok((metadata, stored))
    })
    .await
    .map_err(|_| StoreError::Unavailable)?
}
