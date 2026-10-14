//! Owner-authorized private delivery. SQL authorization and object I/O are separate stages;
//! every delivered stream/grant rechecks the exact immutable location after storage work.
use super::{
    domain::object_storage::{NormalizedByteRange, PrivateObjectRef},
    storage::port::{StorageRead, TemporaryRead},
};
use crate::platform::postgres::{Store, StoreError};

pub(crate) enum AudioReadScope {
    ScriptIntent {
        script: String,
        intent: String,
    },
    Rendition {
        unit: String,
        submission: String,
        rendition: String,
    },
}

pub(crate) struct AuthorizedAudioRead {
    object: PrivateObjectRef,
    scope: AudioReadScope,
}
impl AuthorizedAudioRead {
    pub(crate) fn sha256(&self) -> &str {
        self.object.integrity().sha256()
    }

    pub(crate) fn byte_length(&self) -> u64 {
        self.object.integrity().byte_length()
    }
}
impl Store {
    async fn resolve_audio_read(
        &self,
        token: &str,
        scope: &AudioReadScope,
    ) -> Result<PrivateObjectRef, StoreError> {
        match scope {
            AudioReadScope::ScriptIntent { script, intent } => {
                self.audio_intent_object(token, script, intent).await
            }
            AudioReadScope::Rendition {
                unit,
                submission,
                rendition,
            } => {
                self.audio_rendition_object(token, unit, submission, rendition)
                    .await
            }
        }
    }
    pub(crate) async fn authorize_audio_read(
        &self,
        token: &str,
        scope: AudioReadScope,
    ) -> Result<AuthorizedAudioRead, StoreError> {
        let object = self.resolve_audio_read(token, &scope).await?;
        // Resolve owner authority first, then reject an already-known unavailable reader before
        // HTTP validators/ranges can answer 304/412/416 for an unservable representation.
        // This is configured capability readiness, not proof of remote object availability.
        self.media_storage.as_ref().ok_or(StoreError::Unavailable)?;
        Ok(AuthorizedAudioRead { object, scope })
    }
    async fn reauthorize_audio_read(
        &self,
        token: &str,
        authorized: &AuthorizedAudioRead,
    ) -> Result<(), StoreError> {
        if self.resolve_audio_read(token, &authorized.scope).await? != authorized.object {
            return Err(StoreError::CorruptRevision);
        }
        Ok(())
    }
    pub(crate) async fn open_audio_read(
        &self,
        token: &str,
        authorized: &AuthorizedAudioRead,
        range: Option<NormalizedByteRange>,
    ) -> Result<StorageRead, StoreError> {
        let objects = self.media_storage.as_ref().ok_or(StoreError::Unavailable)?;
        let read = objects.open_verified(&authorized.object, range).await?;
        self.reauthorize_audio_read(token, authorized).await?;
        Ok(read)
    }
    pub(crate) async fn audio_read_grant(
        &self,
        token: &str,
        authorized: &AuthorizedAudioRead,
    ) -> Result<Option<TemporaryRead>, StoreError> {
        let objects = self.media_storage.as_ref().ok_or(StoreError::Unavailable)?;
        let grant = objects.temporary_read(&authorized.object, 60).await?;
        self.reauthorize_audio_read(token, authorized).await?;
        Ok(grant)
    }
}
