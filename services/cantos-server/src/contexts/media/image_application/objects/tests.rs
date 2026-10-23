//! The production image-to-registry bridge crosses real synthetic filesystem bytes here.
//! No database, owner-session HTTP or live S3 provider is part of these tests.
use super::*;
use crate::contexts::{
    catalog::domain::{OwnerId, WorkId},
    media::{
        domain::image::{ImageAsset, ImageIntent, ImageMime, PrepareImage},
        image_intake::ImageAdmission,
    },
};
use crate::contract::studio as dto;

const PNG: &[u8] = include_bytes!("../../image_intake/fixtures/python-rgb-2x3.png");
const PNG_SHA256: &str = "7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276";

async fn fixture() -> (VerifiedImage, StoredImageAsset) {
    let image = ImageAdmission::from_test_lanes(Arc::new(tokio::sync::Semaphore::new(1)))
        .unwrap()
        .inspect(PNG.to_vec(), ImageMime::Png)
        .await
        .unwrap();
    assert_eq!(image.metadata().sha256(), PNG_SHA256);
    let intent = ImageIntent::create(
        PrepareImage::admit(&dto::CreateImageIntentRequest {
            operation_id: "00000000-0000-4000-8000-000000000001".into(),
            id: "00000000-0000-4000-8000-000000000002".into(),
            expected_sha256: PNG_SHA256.into(),
            expected_byte_length: PNG.len() as u64,
            content_type: dto::ImageMime::Png,
            declarations: dto::ImageDeclarations {
                provenance: "Independent synthetic Python PNG, no rights approval".into(),
                rights_refs: vec!["synthetic-rights".into()],
            },
        })
        .unwrap(),
        &WorkId::parse("00000000-0000-4000-8000-000000000003").unwrap(),
        &OwnerId::parse("synthetic-owner").unwrap(),
        0,
    )
    .unwrap();
    let asset = ImageAsset::create(&intent, image.metadata().clone(), 1).unwrap();
    let document_bytes = serde_json::to_vec(asset.document()).unwrap();
    (
        image,
        StoredImageAsset {
            asset,
            storage_target: "legacy-filesystem".into(),
            object_key: PNG_SHA256.into(),
            document_bytes,
        },
    )
}

#[test]
fn shared_storage_errors_preserve_typed_effect_outcomes_and_classify_integrity_as_corrupt() {
    for error in [
        StorageError::NotFound,
        StorageError::InvalidTarget,
        StorageError::InvalidKey,
        StorageError::InvalidIntegrity,
        StorageError::DigestMismatch,
        StorageError::LengthMismatch,
    ] {
        assert_eq!(storage_failure(error), ImageObjectError::Corrupt);
    }
    for error in [
        StorageError::Conflict,
        StorageError::Forbidden,
        StorageError::Timeout,
        StorageError::Cancelled,
        StorageError::Unavailable,
        StorageError::UnsupportedCapability(
            crate::contexts::media::domain::object_storage::StorageCapability::ConditionalCreate,
        ),
    ] {
        assert_eq!(storage_failure(error), ImageObjectError::Storage(error));
    }
    assert_eq!(
        registry_failure(StoreError::CorruptRevision),
        ImageObjectError::Corrupt
    );
    assert_eq!(
        registry_failure(StoreError::Unavailable),
        ImageObjectError::Unavailable
    );
    assert_eq!(
        registry_failure(StoreError::Forbidden),
        ImageObjectError::Corrupt
    );
}

#[tokio::test]
async fn restored_location_is_exact_and_never_falls_back_to_an_active_target() {
    let (image, asset) = fixture().await;
    drop(image);
    let reference = recorded_object(&asset).unwrap();
    assert_eq!(reference.target().as_str(), "legacy-filesystem");
    assert_eq!(reference.key().as_str(), PNG_SHA256);
    assert_eq!(reference.integrity().byte_length(), PNG.len() as u64);
    for (target, key) in [
        ("https://foreign.example", PNG_SHA256.to_owned()),
        ("legacy-filesystem", "../image".into()),
        ("legacy-filesystem", "f".repeat(64)),
    ] {
        let mut corrupt = asset.clone();
        corrupt.storage_target = target.into();
        corrupt.object_key = key;
        assert_eq!(recorded_object(&corrupt), Err(ImageObjectError::Corrupt));
    }
}

#[tokio::test]
async fn missing_composition_refuses_write_verify_and_open_without_fabricating_storage() {
    let (image, asset) = fixture().await;
    let objects = SharedImageObjects::new(None);
    assert!(matches!(
        objects.persist(image).await,
        Err(ImageObjectError::Unavailable)
    ));
    assert_eq!(
        objects.verify(asset.clone()).await,
        Err(ImageObjectError::Unavailable)
    );
    assert!(matches!(
        objects.open(asset).await,
        Err(ImageObjectError::Unavailable)
    ));
}

#[cfg(unix)]
struct SyntheticRoot(std::path::PathBuf);
#[cfg(unix)]
impl SyntheticRoot {
    fn new() -> Self {
        use std::os::unix::fs::DirBuilderExt;
        let path =
            std::env::temp_dir().join(format!("cantos-image-bridge-{}", uuid::Uuid::new_v4()));
        std::fs::DirBuilder::new()
            .mode(0o700)
            .create(&path)
            .unwrap();
        Self(path)
    }
}
#[cfg(unix)]
impl Drop for SyntheticRoot {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

#[cfg(unix)]
#[tokio::test]
async fn production_image_bridge_persists_and_reopens_exact_real_filesystem_bytes() {
    let root = SyntheticRoot::new();
    let registry = Arc::new(StorageRegistry::filesystem(&root.0).unwrap());
    let objects = SharedImageObjects::new(Some(registry));
    let (image, mut asset) = fixture().await;
    let location = objects.persist(image).await.unwrap();
    assert_eq!(location.storage_target, "legacy-filesystem");
    assert_eq!(location.object_key, PNG_SHA256);
    asset.storage_target = location.storage_target;
    asset.object_key = location.object_key;
    objects.verify(asset.clone()).await.unwrap();
    let mut opened = objects.open(asset.clone()).await.unwrap();
    assert_eq!(opened.byte_length, PNG.len() as u64);
    let mut actual = Vec::new();
    while let Some(chunk) = opened.stream.next().await {
        actual.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(actual, PNG);
    drop(opened);
    drop(objects);
    let reopened = SharedImageObjects::new(Some(Arc::new(
        StorageRegistry::filesystem(&root.0).unwrap(),
    )));
    reopened.verify(asset.clone()).await.unwrap();
    let mut opened = reopened.open(asset).await.unwrap();
    let mut actual = Vec::new();
    while let Some(chunk) = opened.stream.next().await {
        actual.extend_from_slice(&chunk.unwrap());
    }
    assert_eq!(actual, PNG);
}

#[cfg(unix)]
#[tokio::test]
async fn accepted_location_with_missing_bytes_is_corruption_on_verify_and_open() {
    let root = SyntheticRoot::new();
    let objects = SharedImageObjects::new(Some(Arc::new(
        StorageRegistry::filesystem(&root.0).unwrap(),
    )));
    let (image, asset) = fixture().await;
    drop(image);
    assert_eq!(
        objects.verify(asset.clone()).await,
        Err(ImageObjectError::Corrupt)
    );
    assert!(matches!(
        objects.open(asset).await,
        Err(ImageObjectError::Corrupt)
    ));
}
