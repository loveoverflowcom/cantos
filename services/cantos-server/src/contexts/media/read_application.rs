//! Owner-authorized private delivery. SQL authorization and object I/O are separate stages;
//! every delivered stream/grant rechecks the exact immutable location after storage work.
use super::{
    domain::object_storage::{NormalizedByteRange, PrivateObjectRef},
    storage::port::{StorageRead, TemporaryRead},
    storage::registry::StorageRegistry,
};
use crate::platform::store::{Store, StoreError};

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
    fn new(
        object: PrivateObjectRef,
        scope: AudioReadScope,
        storage: Option<&StorageRegistry>,
    ) -> Result<Self, StoreError> {
        storage
            .ok_or(StoreError::Unavailable)?
            .ensure_read_target(&object)?;
        Ok(Self { object, scope })
    }

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
        AuthorizedAudioRead::new(object, scope, self.media_storage.as_deref())
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

#[cfg(all(test, unix))]
mod tests {
    use super::*;
    use crate::contexts::media::domain::object_storage::{
        ObjectIntegrity, ObjectKey, StorageError, StorageTargetId,
    };

    struct Scratch(std::path::PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn scope() -> AudioReadScope {
        AudioReadScope::ScriptIntent {
            script: "script".into(),
            intent: "intent".into(),
        }
    }

    fn object(target: &str) -> PrivateObjectRef {
        PrivateObjectRef::new(
            StorageTargetId::parse(target).unwrap(),
            ObjectKey::for_bytes(b"accepted historical bytes"),
            ObjectIntegrity::for_bytes(b"accepted historical bytes").unwrap(),
        )
        .unwrap()
    }

    #[test]
    fn missing_target_cannot_construct_an_authorized_conditional_audio_read() {
        let scratch = Scratch(
            std::env::temp_dir().join(format!("cantos-read-readiness-{}", uuid::Uuid::new_v4())),
        );
        let registry = StorageRegistry::filesystem(&scratch.0).unwrap();
        assert!(matches!(
            AuthorizedAudioRead::new(
                object("removed-historical-target"),
                scope(),
                Some(&registry)
            ),
            Err(StoreError::Storage(StorageError::Unavailable))
        ));
        assert!(matches!(
            AuthorizedAudioRead::new(object("legacy-filesystem"), scope(), None),
            Err(StoreError::Unavailable)
        ));
        // Readiness asserts configured routing only. These bytes have never been written;
        // opening/verifying them remains a later storage effect, not this factory's claim.
        let object = object("legacy-filesystem");
        let authorized =
            AuthorizedAudioRead::new(object.clone(), scope(), Some(&registry)).unwrap();
        assert_eq!(authorized.sha256(), object.integrity().sha256());
        assert_eq!(authorized.byte_length(), object.integrity().byte_length());
    }
}
