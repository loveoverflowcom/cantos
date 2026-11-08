//! Image orchestration's concrete bridge to the reviewed shared private-storage registry.
//! This module selects no provider, implements no storage transport and holds no SQL handle.
use std::sync::Arc;

use futures_util::StreamExt;

use super::{ImageObjectError, ImageObjectFuture, ImageObjects, OpenImage, RecordedImageLocation};
use crate::contexts::media::{
    domain::image::ImageMetadata,
    domain::object_storage::{
        ObjectIntegrity, ObjectKey, PrivateObjectRef, StorageError, StorageTargetId,
    },
    image_intake::VerifiedImage,
    storage::registry::StorageRegistry,
    turso::image::StoredImageAsset,
};
use crate::platform::store::StoreError;

pub(in crate::contexts::media) struct SharedImageObjects {
    registry: Option<Arc<StorageRegistry>>,
}

impl SharedImageObjects {
    pub(in crate::contexts::media) fn new(registry: Option<Arc<StorageRegistry>>) -> Self {
        Self { registry }
    }
}

fn recorded_object(asset: &StoredImageAsset) -> Result<PrivateObjectRef, ImageObjectError> {
    image_object(
        &asset.storage_target,
        &asset.object_key,
        asset.asset.metadata(),
    )
}

fn image_object(
    target: &str,
    key: &str,
    metadata: &ImageMetadata,
) -> Result<PrivateObjectRef, ImageObjectError> {
    // These are persisted accepted facts, so malformed location/integrity is corruption,
    // never bad caller input or an invitation to use the current active write target.
    let target = StorageTargetId::parse(target).map_err(|_| ImageObjectError::Corrupt)?;
    let key = ObjectKey::parse(key).map_err(|_| ImageObjectError::Corrupt)?;
    let integrity = ObjectIntegrity::new(metadata.sha256(), metadata.byte_length())
        .map_err(|_| ImageObjectError::Corrupt)?;
    PrivateObjectRef::new(target, key, integrity).map_err(|_| ImageObjectError::Corrupt)
}

fn storage_failure(error: StorageError) -> ImageObjectError {
    match error {
        // An admitted asset whose exact bytes disappeared or changed is corrupt.
        StorageError::NotFound
        | StorageError::InvalidTarget
        | StorageError::InvalidKey
        | StorageError::InvalidIntegrity
        | StorageError::DigestMismatch
        | StorageError::LengthMismatch => ImageObjectError::Corrupt,
        StorageError::Conflict
        | StorageError::Forbidden
        | StorageError::Timeout
        | StorageError::Cancelled
        | StorageError::Unavailable
        | StorageError::UnsupportedCapability(_) => ImageObjectError::Storage(error),
    }
}

fn registry_failure(error: StoreError) -> ImageObjectError {
    match error {
        StoreError::Storage(error) => storage_failure(error),
        StoreError::CorruptRevision => ImageObjectError::Corrupt,
        // The registry only emits infrastructure/corruption outcomes. A future unexpected
        // composition refusal still cannot authorize, synthesize bytes or expose internals.
        StoreError::Unauthenticated
        | StoreError::NotFound
        | StoreError::Forbidden
        | StoreError::InvalidRequest
        | StoreError::InvalidRequestFields(_)
        | StoreError::InvalidScript(_)
        | StoreError::EvidenceUnavailable
        | StoreError::StaleRevision(_)
        | StoreError::OperationReused
        | StoreError::ProposalAlreadySubmitted
        | StoreError::StaleProductionInputs
        | StoreError::ProductionBlocked(_)
        | StoreError::AudioRejected(_)
        | StoreError::AudioInspectionUnavailable(_) => ImageObjectError::Corrupt,
        StoreError::Unavailable => ImageObjectError::Unavailable,
    }
}

impl ImageObjects for SharedImageObjects {
    fn persist(&self, image: VerifiedImage) -> ImageObjectFuture<RecordedImageLocation> {
        let registry = self.registry.clone();
        Box::pin(async move {
            let registry = registry.ok_or(ImageObjectError::Unavailable)?;
            let stored = registry
                .persist_image(image)
                .await
                .map_err(registry_failure)?;
            Ok(RecordedImageLocation {
                storage_target: stored.storage_target.as_str().to_owned(),
                object_key: stored.object_key,
            })
        })
    }

    fn verify(&self, asset: StoredImageAsset) -> ImageObjectFuture<()> {
        let registry = self.registry.clone();
        Box::pin(async move {
            let object = recorded_object(&asset)?;
            let registry = registry.ok_or(ImageObjectError::Unavailable)?;
            registry.verify(&object).await.map_err(registry_failure)
        })
    }

    fn open(&self, asset: StoredImageAsset) -> ImageObjectFuture<OpenImage> {
        let registry = self.registry.clone();
        Box::pin(async move {
            let object = recorded_object(&asset)?;
            open_image(registry, object).await
        })
    }
}

impl SharedImageObjects {
    pub(in crate::contexts::media) async fn open_rendition(
        &self,
        asset: &StoredImageAsset,
        metadata: &ImageMetadata,
    ) -> Result<OpenImage, ImageObjectError> {
        let object = image_object(&asset.storage_target, metadata.sha256(), metadata)?;
        open_image(self.registry.clone(), object).await
    }
}

async fn open_image(
    registry: Option<Arc<StorageRegistry>>,
    object: PrivateObjectRef,
) -> Result<OpenImage, ImageObjectError> {
    let registry = registry.ok_or(ImageObjectError::Unavailable)?;
    // The registry verifies bytes on a retained filesystem handle or pins the S3 read.
    let read = registry
        .open_verified(&object, None)
        .await
        .map_err(registry_failure)?;
    Ok(OpenImage {
        stream: Box::pin(read.stream.map(|chunk| chunk.map_err(storage_failure))),
        byte_length: read.byte_length,
    })
}

#[cfg(test)]
mod tests;
