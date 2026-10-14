//! Composition-selected targets. Reads always resolve the immutable recorded target.
use super::super::{
    audio_intake::VerifiedAudio,
    domain::object_storage::{
        ObjectIntegrity, ObjectKey, PrivateObjectRef, StorageError, StorageTargetId,
    },
    image_intake::VerifiedImage,
};
use super::{
    config::{StorageConfig, TargetConfig},
    s3::S3Storage,
};
use super::{
    port::{PrivateStorage, VerifiedPayload},
    FilesystemStorage,
};
use crate::platform::postgres::StoreError;
use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, LazyLock},
};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

static STORAGE_REQUESTS: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));
fn storage_permit() -> Result<OwnedSemaphorePermit, StoreError> {
    STORAGE_REQUESTS
        .clone()
        .try_acquire_owned()
        .map_err(|_| StoreError::Storage(StorageError::Unavailable))
}

pub(crate) struct RegistryStoredObject {
    pub(crate) object_key: String,
    pub(crate) storage_target: StorageTargetId,
}
#[derive(Clone)]
pub(crate) struct StorageRegistry {
    active: StorageTargetId,
    targets: BTreeMap<StorageTargetId, Arc<dyn PrivateStorage>>,
}
impl StorageRegistry {
    pub(crate) fn filesystem(root: &Path) -> Result<Self, StoreError> {
        let active = StorageTargetId::parse("legacy-filesystem").map_err(storage_error)?;
        let adapter = FilesystemStorage::new(root)?;
        Ok(Self {
            active: active.clone(),
            targets: BTreeMap::from([(active, Arc::new(adapter) as Arc<dyn PrivateStorage>)]),
        })
    }
    pub(crate) fn from_config(path: &Path) -> Result<Self, StoreError> {
        Self::configured(StorageConfig::load(path).map_err(storage_error)?, |name| {
            std::env::var(name).ok()
        })
    }
    fn configured(
        config: StorageConfig,
        secret: impl Fn(&str) -> Option<String>,
    ) -> Result<Self, StoreError> {
        let mut targets: BTreeMap<StorageTargetId, Arc<dyn PrivateStorage>> = BTreeMap::new();
        for (id, target) in config.targets {
            let adapter: Arc<dyn PrivateStorage> = match target {
                TargetConfig::Filesystem(root) => Arc::new(FilesystemStorage::new(&root)?),
                TargetConfig::S3 {
                    settings,
                    access_key_env,
                    secret_key_env,
                    session_token_env,
                } => {
                    let access = secret(&access_key_env).ok_or(StoreError::InvalidRequest)?;
                    let key = secret(&secret_key_env).ok_or(StoreError::InvalidRequest)?;
                    let session = session_token_env
                        .map(|name| secret(&name).ok_or(StoreError::InvalidRequest))
                        .transpose()?;
                    Arc::new(S3Storage::new(settings, access, key, session).map_err(storage_error)?)
                }
            };
            targets.insert(id, adapter);
        }
        Ok(Self {
            active: config.active,
            targets,
        })
    }
    fn resolve(&self, target: &StorageTargetId) -> Result<Arc<dyn PrivateStorage>, StoreError> {
        self.targets
            .get(target)
            .cloned()
            .ok_or(StoreError::Storage(StorageError::Unavailable))
    }
    pub(crate) async fn persist_audio(
        &self,
        verified: VerifiedAudio,
    ) -> Result<RegistryStoredObject, StoreError> {
        self.persist_payload(VerifiedPayload::Audio(verified)).await
    }
    pub(crate) async fn persist_image(
        &self,
        verified: VerifiedImage,
    ) -> Result<RegistryStoredObject, StoreError> {
        self.persist_payload(VerifiedPayload::Image(verified)).await
    }
    async fn persist_payload(
        &self,
        verified: VerifiedPayload,
    ) -> Result<RegistryStoredObject, StoreError> {
        let _permit = storage_permit()?;
        let object_key = ObjectKey::for_bytes(verified.bytes());
        ObjectIntegrity::new(verified.sha256(), verified.byte_length())
            .and_then(|i| i.verify_bytes(verified.bytes()))
            .map_err(storage_error)?;
        self.resolve(&self.active)?
            .put_if_absent(verified)
            .await
            .map_err(storage_error)?;
        Ok(RegistryStoredObject {
            object_key: object_key.as_str().to_owned(),
            storage_target: self.active.clone(),
        })
    }
    pub(crate) async fn verify(&self, object: &PrivateObjectRef) -> Result<(), StoreError> {
        let _permit = storage_permit()?;
        self.resolve(object.target())?
            .verify(object.key().clone(), object.integrity().clone())
            .await
            .map_err(verification_error)
    }
    pub(crate) async fn open_verified(
        &self,
        object: &PrivateObjectRef,
        range: Option<super::super::domain::object_storage::NormalizedByteRange>,
    ) -> Result<super::port::StorageRead, StoreError> {
        use futures_util::StreamExt;
        let permit = storage_permit()?;
        let read = self
            .resolve(object.target())?
            .open_verified(object.key().clone(), object.integrity().clone(), range)
            .await
            .map_err(storage_error)?;
        let stream = read.stream.map(move |chunk| {
            let _keep_permit = &permit;
            chunk
        });
        Ok(super::port::StorageRead {
            stream: Box::pin(stream),
            byte_length: read.byte_length,
        })
    }
    pub(crate) async fn temporary_read(
        &self,
        object: &PrivateObjectRef,
        ttl: u32,
    ) -> Result<Option<super::port::TemporaryRead>, StoreError> {
        let _permit = storage_permit()?;
        self.resolve(object.target())?
            .temporary_read(object.key().clone(), object.integrity().clone(), ttl)
            .await
            .map_err(storage_error)
    }
}
pub(crate) fn storage_error(error: StorageError) -> StoreError {
    StoreError::Storage(error)
}

fn verification_error(error: StorageError) -> StoreError {
    match error {
        StorageError::NotFound
        | StorageError::InvalidKey
        | StorageError::InvalidIntegrity
        | StorageError::DigestMismatch
        | StorageError::LengthMismatch => StoreError::CorruptRevision,
        StorageError::InvalidTarget
        | StorageError::Conflict
        | StorageError::Forbidden
        | StorageError::Timeout
        | StorageError::Cancelled
        | StorageError::Unavailable
        | StorageError::UnsupportedCapability(_) => StoreError::Storage(error),
    }
}

#[cfg(test)]
#[path = "registry_tests.rs"]
mod tests;
