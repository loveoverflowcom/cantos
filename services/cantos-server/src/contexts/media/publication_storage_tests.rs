//! Real filesystem fault/read tests over arbitrary transport bytes. Full Ogg/Opus promotion is
//! exercised by the PostgreSQL seed acceptance, where decoder prerequisites are explicit.
use super::*;
use crate::contexts::media::{
    domain::{object_storage::ByteRange, publication::Sha256},
    listener_delivery::PlaybackPermit,
};
use futures_util::StreamExt;
use sha2::Digest;
use std::{fs, path::PathBuf};
const RENDITION: &str = "60000000-0000-4000-8000-000000000006";
struct Root(PathBuf);
impl Root {
    fn new() -> Self {
        Self(
            fs::canonicalize(std::env::temp_dir())
                .unwrap()
                .join(format!("cantos-publication-{}", uuid::Uuid::new_v4())),
        )
    }
}
impl Drop for Root {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
fn hash(bytes: &[u8]) -> String {
    format!("{:x}", sha2::Sha256::digest(bytes))
}
fn object(bytes: &[u8]) -> PublicationObjectRef {
    PublicationObjectRef::for_permit(&PlaybackPermit {
        publication_id: "80000000-0000-4000-8000-000000000008".into(),
        rendition_id: RENDITION.into(),
        manifest_digest: format!("manifest-v1:sha256:{}", "b".repeat(64)),
        sha256: hash(bytes),
        object_id: format!("publication-{RENDITION}-{}", hash(bytes)),
        byte_length: bytes.len() as u64,
        valid_until: 10_000,
        access_scope: None,
    })
}
async fn body(mut read: StorageRead) -> Vec<u8> {
    let mut bytes = Vec::new();
    while let Some(chunk) = read.stream.next().await {
        bytes.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(bytes.len() as u64, read.byte_length);
    bytes
}
#[test]
fn private_and_publication_classes_cannot_share_or_adopt_each_others_roots() {
    let private = Root::new();
    crate::contexts::media::storage::FilesystemStorage::new(&private.0).unwrap();
    assert!(matches!(
        FilesystemPublicationStorage::new(&private.0),
        Err(StorageError::InvalidIntegrity)
    ));
    let public = Root::new();
    FilesystemPublicationStorage::new(&public.0).unwrap();
    assert!(crate::contexts::media::storage::FilesystemStorage::new(&public.0).is_err());
    assert!(matches!(
        crate::contexts::media::storage::validate_publication_separation(
            &private.0.join("child"),
            Some(&private.0),
            None
        ),
        Err(StorageError::InvalidTarget)
    ));
    let legacy = Root::new();
    fs::create_dir(&legacy.0).unwrap();
    use std::os::unix::fs::PermissionsExt;
    fs::set_permissions(&legacy.0, fs::Permissions::from_mode(0o700)).unwrap();
    fs::create_dir(legacy.0.join("objects")).unwrap();
    assert!(matches!(
        FilesystemPublicationStorage::new(&legacy.0),
        Err(StorageError::InvalidIntegrity)
    ));
}
#[tokio::test]
async fn exact_bytes_ranges_and_integrity_survive_restart_without_private_authority() {
    let root = Root::new();
    let bytes: Vec<_> = (0..=255_u8).collect();
    let storage = FilesystemPublicationStorage::new(&root.0).unwrap();
    storage
        .inner
        .inject_test_bytes(&bytes, &hash(&bytes))
        .unwrap();
    drop(storage);
    let storage = FilesystemPublicationStorage::new(&root.0).unwrap();
    assert_eq!(
        body(storage.open(object(&bytes), None).await.unwrap()).await,
        bytes
    );
    let range = ByteRange::parse("bytes=27-112")
        .unwrap()
        .normalize(256)
        .unwrap();
    assert_eq!(
        body(storage.open(object(&bytes), Some(range)).await.unwrap()).await,
        &bytes[27..113]
    );
    let mut wrong = object(&bytes);
    // Refuse a correct object ID paired with a contradictory exact length.
    let permit = PlaybackPermit {
        publication_id: "80000000-0000-4000-8000-000000000008".into(),
        rendition_id: RENDITION.into(),
        manifest_digest: "unused".into(),
        sha256: hash(&bytes),
        object_id: wrong.object_id().into(),
        byte_length: 255,
        valid_until: 1000,
        access_scope: None,
    };
    wrong = PublicationObjectRef::for_permit(&permit);
    assert!(matches!(
        storage.open(wrong, None).await,
        Err(StorageError::LengthMismatch)
    ));
    let missing = object(b"missing");
    assert!(matches!(
        storage.open(missing, None).await,
        Err(StorageError::NotFound)
    ));
    // Corruption after promotion is refused on the next open, rather than returning bytes.
    use std::os::unix::fs::PermissionsExt;
    let path = root.0.join("objects").join(hash(&bytes));
    fs::set_permissions(&path, fs::Permissions::from_mode(0o600)).unwrap();
    fs::write(&path, vec![0; 256]).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o400)).unwrap();
    assert!(matches!(
        storage.open(object(&bytes), None).await,
        Err(StorageError::DigestMismatch)
    ));
}
#[tokio::test]
async fn observed_publication_requires_actual_media_format_and_missing_bytes_fail_closed() {
    let root = Root::new();
    let storage = FilesystemPublicationStorage::new(&root.0).unwrap();
    let bytes = b"renamed audio.ogg";
    let request = || PublicationObject {
        rendition_id: RENDITION.into(),
        sha256: Sha256::parse(&hash(bytes)).unwrap(),
        byte_length: bytes.len() as u64,
        profile_id: "opus-mono-24-v1".into(),
    };
    assert_eq!(
        storage.observe(request()).await.unwrap(),
        DeliveryObservation::Missing
    );
    storage
        .inner
        .inject_test_bytes(bytes, &hash(bytes))
        .unwrap();
    assert_eq!(
        storage.observe(request()).await.unwrap(),
        DeliveryObservation::IntegrityMismatch
    );
}

#[test]
fn concurrent_open_cannot_admit_one_root_into_both_storage_classes() {
    let root = Root::new();
    let barrier = Arc::new(std::sync::Barrier::new(2));
    let private_path = root.0.clone();
    let private_barrier = barrier.clone();
    let private = std::thread::spawn(move || {
        private_barrier.wait();
        crate::contexts::media::storage::FilesystemStorage::new(&private_path).is_ok()
    });
    let public_path = root.0.clone();
    let public = std::thread::spawn(move || {
        barrier.wait();
        FilesystemPublicationStorage::new(&public_path).is_ok()
    });
    let private = private.join().unwrap();
    let public = public.join().unwrap();
    assert_ne!(private, public);
}

/// Studio readiness/active-release/publish observations inspect bytes with external decoders for
/// seconds. Holding every admission lane must not refuse a listener read, and listener reads
/// holding every read lane must not fail an admission observation. Each pool keeps its bound.
#[tokio::test]
async fn admission_inspection_and_listener_reads_cannot_exhaust_each_others_lanes() {
    let root = Root::new();
    let bytes: Vec<_> = (0..=255_u8).collect();
    let storage = FilesystemPublicationStorage::new(&root.0).unwrap();
    storage
        .inner
        .inject_test_bytes(&bytes, &hash(&bytes))
        .unwrap();
    // The host shares one value between publication admission and origin playback.
    let listener = storage.clone();
    let admission = storage
        .admission
        .clone()
        .acquire_many_owned(ADMISSION_LANES as u32)
        .await
        .unwrap();
    assert_eq!(
        body(listener.open(object(&bytes), None).await.unwrap()).await,
        bytes
    );
    let missing = PublicationObject {
        rendition_id: RENDITION.into(),
        sha256: Sha256::parse(&hash(b"missing")).unwrap(),
        byte_length: 7,
        profile_id: "opus-mono-24-v1".into(),
    };
    assert_eq!(
        storage.observe(missing.clone()).await,
        Err(PublicationDependencyError::Unavailable)
    );
    drop(admission);
    let reads = listener
        .reads
        .clone()
        .acquire_many_owned(READ_LANES as u32)
        .await
        .unwrap();
    assert_eq!(
        storage.observe(missing).await.unwrap(),
        DeliveryObservation::Missing
    );
    assert!(matches!(
        listener.open(object(&bytes), None).await,
        Err(StorageError::Unavailable)
    ));
    drop(reads);
}
