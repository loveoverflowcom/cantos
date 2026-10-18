//! Publication-class filesystem bytes for origin delivery. This is a separate capability and
//! root from private intake. Promotion copies only an actual-byte inspection witness; neither
//! this adapter nor its object identifiers grant public access. PostgreSQL eligibility does.
use super::{
    audio_intake::{inspect_audio, DeliveryProfile, VerifiedAudio},
    domain::{
        object_storage::{NormalizedByteRange, ObjectIntegrity, StorageError},
        publication::{DeliveryEvidence, DeliveryObservation},
    },
    listener_delivery::{PublicationObjectRef, PublicationObjects},
    publication_delivery::{
        PublicationDelivery, PublicationDeliveryFuture, PublicationDependencyError,
        PublicationObject,
    },
    storage::{
        bounded_filesystem_io,
        port::{StorageFuture, StorageRead},
        storage_error,
    },
};
use std::{
    path::Path,
    sync::{Arc, LazyLock},
};
use tokio::sync::Semaphore;

static PUBLICATION_IO: LazyLock<Arc<Semaphore>> = LazyLock::new(|| Arc::new(Semaphore::new(4)));

/// Immutable approved-audio byte class, with no private-storage port and no direct file URL.
/// Files are content-addressed in this root only. A publication identity additionally binds
/// the rendition; identical approved bytes can be reused without mutable object overwrites.
#[derive(Clone)]
pub struct FilesystemPublicationStorage {
    #[cfg(unix)]
    inner: Arc<super::storage::unix::ObjectDirectories>,
}
impl FilesystemPublicationStorage {
    /// Absolute, owner-only root with no symlink components. Storage-class markers refuse
    /// any private root and refuse adopting an unmarked legacy objects directory.
    pub fn new(root: &Path) -> Result<Self, StorageError> {
        #[cfg(unix)]
        {
            Ok(Self {
                inner: Arc::new(
                    super::storage::unix::ObjectDirectories::publication(root)
                        .map_err(storage_error)?,
                ),
            })
        }
        #[cfg(not(unix))]
        {
            let _ = root;
            Err(StorageError::Unavailable)
        }
    }

    /// Durable conditional promotion of inspected bytes, before the publication transaction.
    /// Replays verify the existing immutable object; interruption can leave only an unreferenced
    /// publication-class object. No release pointer or private permission changes here.
    pub async fn promote(
        &self,
        rendition_id: &str,
        audio: VerifiedAudio,
    ) -> Result<(), StorageError> {
        publication_id(rendition_id, &audio.metadata().sha256)?;
        let storage = self.clone();
        bounded_filesystem_io(PUBLICATION_IO.clone(), move || {
            #[cfg(unix)]
            {
                storage
                    .inner
                    .persist_audio(&audio)
                    .map_err(storage_error)
                    .map(|_| ())
            }
            #[cfg(not(unix))]
            {
                let _ = (storage, audio);
                Err(StorageError::Unavailable)
            }
        })
        .await
    }

    #[cfg(unix)]
    fn verified_file(&self, sha256: &str, length: u64) -> Result<std::fs::File, StorageError> {
        self.inner.verified_file(sha256, sha256, length, None)
    }
}

fn publication_id(rendition: &str, sha256: &str) -> Result<String, StorageError> {
    if crate::platform::ids::canonical_uuid(rendition).as_deref() != Some(rendition)
        || super::domain::publication::Sha256::parse(sha256).is_err()
    {
        return Err(StorageError::InvalidKey);
    }
    Ok(format!("publication-{rendition}-{sha256}"))
}

impl PublicationDelivery for FilesystemPublicationStorage {
    fn observe(&self, object: PublicationObject) -> PublicationDeliveryFuture {
        let storage = self.clone();
        Box::pin(async move {
            let id = publication_id(&object.rendition_id, object.sha256.as_str())
                .map_err(|_| PublicationDependencyError::InvalidObservation)?;
            let result = bounded_filesystem_io(PUBLICATION_IO.clone(), move || {
                #[cfg(unix)]
                {
                    use std::io::{Read, Seek};
                    let mut file =
                        storage.verified_file(object.sha256.as_str(), object.byte_length)?;
                    file.rewind().map_err(|_| StorageError::Unavailable)?;
                    let mut bytes = Vec::with_capacity(object.byte_length as usize);
                    file.take(object.byte_length + 1)
                        .read_to_end(&mut bytes)
                        .map_err(|_| StorageError::Unavailable)?;
                    ObjectIntegrity::new(object.sha256.as_str(), object.byte_length)?
                        .verify_bytes(&bytes)?;
                    let profile = DeliveryProfile::from_id(&object.profile_id)
                        .ok_or(StorageError::InvalidIntegrity)?;
                    // Read-only inspection, never transcode. Its full format/profile check binds
                    // the exact bytes that crossed into the publication class.
                    inspect_audio(&bytes, profile).map_err(|error| {
                        if error.is_unavailable() {
                            StorageError::Unavailable
                        } else {
                            StorageError::InvalidIntegrity
                        }
                    })?;
                    let now = std::time::SystemTime::now()
                        .duration_since(std::time::UNIX_EPOCH)
                        .ok()
                        .and_then(|duration| i64::try_from(duration.as_millis()).ok())
                        .ok_or(StorageError::Unavailable)?;
                    let evidence = DeliveryEvidence::new(
                        id,
                        object.sha256,
                        object.byte_length,
                        now,
                        now.checked_add(120_000).ok_or(StorageError::Unavailable)?,
                    )
                    .map_err(|_| StorageError::InvalidIntegrity)?;
                    Ok(DeliveryObservation::Verified(evidence))
                }
                #[cfg(not(unix))]
                {
                    let _ = (storage, object, id);
                    Err(StorageError::Unavailable)
                }
            })
            .await;
            match result {
                Ok(observation) => Ok(observation),
                Err(StorageError::NotFound) => Ok(DeliveryObservation::Missing),
                Err(
                    StorageError::DigestMismatch
                    | StorageError::LengthMismatch
                    | StorageError::InvalidIntegrity
                    | StorageError::InvalidKey
                    | StorageError::Conflict,
                ) => Ok(DeliveryObservation::IntegrityMismatch),
                Err(_) => Err(PublicationDependencyError::Unavailable),
            }
        })
    }
}

impl PublicationObjects for FilesystemPublicationStorage {
    fn open(
        &self,
        object: PublicationObjectRef,
        range: Option<NormalizedByteRange>,
    ) -> StorageFuture<StorageRead> {
        let storage = self.clone();
        Box::pin(async move {
            let identity = object
                .object_id()
                .strip_prefix("publication-")
                .ok_or(StorageError::InvalidKey)?;
            let (rendition, sha) = identity
                .split_at_checked(36)
                .ok_or(StorageError::InvalidKey)?;
            if sha.strip_prefix('-') != Some(object.sha256())
                || publication_id(rendition, object.sha256())? != object.object_id()
            {
                return Err(StorageError::InvalidKey);
            }
            let range = match range {
                Some(range) if range.total_byte_length() == object.byte_length() => range,
                Some(_) => return Err(StorageError::LengthMismatch),
                None => NormalizedByteRange::full(object.byte_length())
                    .map_err(|_| StorageError::InvalidIntegrity)?,
            };
            #[cfg(unix)]
            {
                use futures_util::StreamExt;
                use std::io::{Seek, SeekFrom};
                use tokio::io::AsyncReadExt;
                let file = bounded_filesystem_io(PUBLICATION_IO.clone(), move || {
                    let mut file = storage.verified_file(object.sha256(), object.byte_length())?;
                    file.seek(SeekFrom::Start(range.first()))
                        .map_err(|_| StorageError::Unavailable)?;
                    Ok(file)
                })
                .await?;
                let reader = tokio::fs::File::from_std(file).take(range.byte_length());
                let stream = tokio_util::io::ReaderStream::with_capacity(reader, 16 * 1024)
                    .map(|chunk| chunk.map_err(|_| StorageError::Unavailable));
                Ok(StorageRead {
                    stream: Box::pin(stream),
                    byte_length: range.byte_length(),
                })
            }
            #[cfg(not(unix))]
            {
                let _ = (storage, object, range);
                Err(StorageError::Unavailable)
            }
        })
    }
}

#[cfg(all(test, unix))]
#[path = "publication_storage_tests.rs"]
mod tests;
