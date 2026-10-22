//! Private Work artwork orchestration. Metadata transactions end before inspection/storage,
//! and every mutation/read response reauthorizes through the Catalog owner-work facade.
use std::{future::Future, pin::Pin, sync::Arc};

use bytes::Bytes;
use cantos_contracts as dto;
use futures_util::Stream;
use sha2::{Digest, Sha256};

use super::{
    domain::{
        artwork::{ArtworkChangeValue, ArtworkError, SetWorkArtwork},
        image::{
            ExpectedImage, ImageAssetId, ImageError, ImageIntentId, ImageMetadata, ImageMime,
            PrepareImage, MAX_IMAGE_BYTES,
        },
    },
    image_intake::{ImageAdmission, ImageIntakeError, VerifiedImage},
    postgres::image::StoredImageAsset,
};
use crate::{
    contexts::catalog::{
        domain::{Lifecycle, Revision, Work, WorkId},
        CatalogStoreError,
    },
    platform::store::{Store, StoreError},
};

mod objects;
mod postgres;

#[derive(Debug)]
pub enum ArtworkApplicationError {
    Storage(StoreError),
    Catalog(CatalogStoreError),
    Image(ImageError),
    Artwork(ArtworkError),
    Intake(ImageIntakeError),
    Object(ImageObjectError),
}
impl From<StoreError> for ArtworkApplicationError {
    fn from(value: StoreError) -> Self {
        Self::Storage(value)
    }
}
impl From<CatalogStoreError> for ArtworkApplicationError {
    fn from(value: CatalogStoreError) -> Self {
        Self::Catalog(value)
    }
}
impl From<ImageError> for ArtworkApplicationError {
    fn from(value: ImageError) -> Self {
        Self::Image(value)
    }
}
impl From<ArtworkError> for ArtworkApplicationError {
    fn from(value: ArtworkError) -> Self {
        Self::Artwork(value)
    }
}
impl From<ImageIntakeError> for ArtworkApplicationError {
    fn from(value: ImageIntakeError) -> Self {
        Self::Intake(value)
    }
}
impl From<ImageObjectError> for ArtworkApplicationError {
    fn from(value: ImageObjectError) -> Self {
        Self::Object(value)
    }
}

/// Safe distinctions at the private-object boundary; no paths, provider errors or credentials.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageObjectError {
    NotFound,
    Unavailable,
    Corrupt,
    Storage(super::domain::object_storage::StorageError),
}
impl std::fmt::Display for ImageObjectError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::NotFound => "private image unavailable",
            Self::Unavailable => "private image storage unavailable",
            Self::Corrupt => "private image integrity mismatch",
            Self::Storage(error) => error.code(),
        })
    }
}
impl std::error::Error for ImageObjectError {}

pub(super) type MetadataFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ArtworkApplicationError>> + Send + 'static>>;
pub(super) type ImageObjectFuture<T> =
    Pin<Box<dyn Future<Output = Result<T, ImageObjectError>> + Send + 'static>>;
pub type ImageByteStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, ImageObjectError>> + Send + 'static>>;

/// Exact immutable JSON receipt bytes plus its checked transport-neutral value. JSON replay
/// returns the original bytes; the opt-in Protobuf adapter projects the same accepted value.
#[derive(Clone)]
pub struct ArtworkReceipt<T> {
    pub(crate) value: T,
    pub(crate) json_bytes: Vec<u8>,
}
impl<T> ArtworkReceipt<T> {
    pub fn value(&self) -> &T {
        &self.value
    }
    pub fn json_bytes(&self) -> &[u8] {
        &self.json_bytes
    }
    pub fn into_parts(self) -> (T, Vec<u8>) {
        (self.value, self.json_bytes)
    }
}

#[derive(Clone)]
pub(super) struct OwnerWorkScope {
    token: String,
    work_id: WorkId,
}
impl OwnerWorkScope {
    fn new(token: &str, work: &str) -> Result<Self, ArtworkApplicationError> {
        let work_id = WorkId::parse(work).map_err(|_| ImageError::InvalidWorkId)?;
        Ok(Self {
            token: token.into(),
            work_id,
        })
    }
}

pub(super) struct AdmissionContext {
    pub work: Work,
    pub intent: super::postgres::image::StoredImageIntent,
    pub asset: Option<StoredImageAsset>,
    pub now: i64,
}
pub(super) struct ArtworkContext {
    pub work: Work,
    pub asset: Option<StoredImageAsset>,
    pub replay: Option<ArtworkReceipt<dto::WorkArtworkResponse>>,
}

/// One coarse metadata transaction per call. No Tx, SQL row, or held lock escapes this port.
/// The production Pg repository and deterministic tests exercise the same orchestration.
pub(super) trait ArtworkRepository: Send + Sync {
    fn prepare(
        &self,
        scope: OwnerWorkScope,
        request: PrepareImage,
    ) -> MetadataFuture<ArtworkReceipt<dto::ImageIntentDocument>>;
    fn intent(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
    ) -> MetadataFuture<dto::ImageIntentResponse>;
    fn admission_context(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
    ) -> MetadataFuture<AdmissionContext>;
    fn finalize(
        &self,
        scope: OwnerWorkScope,
        id: ImageIntentId,
        metadata: ImageMetadata,
        location: RecordedImageLocation,
    ) -> MetadataFuture<ArtworkReceipt<dto::ImageAssetDocument>>;
    fn asset(&self, scope: OwnerWorkScope, id: ImageAssetId) -> MetadataFuture<StoredImageAsset>;
    fn artwork(
        &self,
        scope: OwnerWorkScope,
        revision: Option<Revision>,
    ) -> MetadataFuture<dto::WorkArtworkResponse>;
    fn artwork_context(
        &self,
        scope: OwnerWorkScope,
        request: SetWorkArtwork,
    ) -> MetadataFuture<ArtworkContext>;
    fn set_artwork(
        &self,
        scope: OwnerWorkScope,
        request: SetWorkArtwork,
        observed_asset: Option<StoredImageAsset>,
    ) -> MetadataFuture<ArtworkReceipt<dto::WorkArtworkResponse>>;
}

/// Internal location facts returned by the reviewed shared store after actual-byte persistence
/// and readback. These are not public image metadata or an independent storage implementation.
pub(super) struct RecordedImageLocation {
    pub storage_target: String,
    pub object_key: String,
}
pub(super) struct OpenImage {
    pub stream: ImageByteStream,
    pub byte_length: u64,
}

/// Actual storage adapters must consume the sealed inspector witness. There is no raw-byte
/// write, caller key, remote fetch, overwrite or image-to-audio conversion on this port.
pub(super) trait ImageObjects: Send + Sync {
    fn persist(&self, image: VerifiedImage) -> ImageObjectFuture<RecordedImageLocation>;
    fn verify(&self, asset: StoredImageAsset) -> ImageObjectFuture<()>;
    fn open(&self, asset: StoredImageAsset) -> ImageObjectFuture<OpenImage>;
}

#[derive(Clone)]
pub struct ArtworkService {
    repository: Arc<dyn ArtworkRepository>,
    objects: Arc<dyn ImageObjects>,
}
impl ArtworkService {
    pub fn new(store: Store) -> Self {
        let objects = objects::SharedImageObjects::new(store.media_storage.clone());
        Self {
            repository: Arc::new(postgres::PgArtworkRepository(store)),
            objects: Arc::new(objects),
        }
    }
    pub async fn prepare(
        &self,
        token: &str,
        work: &str,
        raw: dto::CreateImageIntentRequest,
    ) -> Result<ArtworkReceipt<dto::ImageIntentDocument>, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        let request = PrepareImage::admit(&raw)?;
        self.repository.prepare(scope, request).await
    }
    pub async fn intent(
        &self,
        token: &str,
        work: &str,
        id: &str,
    ) -> Result<dto::ImageIntentResponse, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        self.repository
            .intent(scope, ImageIntentId::parse(id)?)
            .await
    }
    /// Cheap scope/intent facts settle first; reserve before the handler buffers any body.
    /// Reauthorize after admission even though the current bounded policy refuses busy lanes
    /// immediately, so a future bounded wait cannot accidentally retain stale authority.
    pub async fn begin_upload(
        &self,
        token: &str,
        work: &str,
        id: &str,
        mime: dto::ImageMime,
    ) -> Result<ImageUploadTicket, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        let id = ImageIntentId::parse(id)?;
        let mime = match mime {
            dto::ImageMime::Png => ImageMime::Png,
            dto::ImageMime::Jpeg => ImageMime::Jpeg,
        };
        let initial = self
            .repository
            .admission_context(scope.clone(), id.clone())
            .await?;
        validate_admission_context(&scope, &id, &initial)?;
        if initial.intent.intent.request().expected().content_type() != mime {
            return Err(ImageError::ExpectedMimeMismatch.into());
        }
        let admission = ImageAdmission::try_acquire()?;
        let current = self
            .repository
            .admission_context(scope.clone(), id.clone())
            .await?;
        validate_admission_context(&scope, &id, &current)?;
        if current.intent.intent.document() != initial.intent.intent.document() {
            return Err(StoreError::CorruptRevision.into());
        }
        if current.intent.intent.request().expected().content_type() != mime {
            return Err(ImageError::ExpectedMimeMismatch.into());
        }
        Ok(ImageUploadTicket {
            service: self.clone(),
            scope,
            id,
            mime,
            expected: current.intent.intent.request().expected().clone(),
            admission,
        })
    }
    pub async fn asset(
        &self,
        token: &str,
        work: &str,
        id: &str,
    ) -> Result<ArtworkReceipt<dto::ImageAssetDocument>, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        let stored = self
            .repository
            .asset(scope, ImageAssetId::parse(id)?)
            .await?;
        Ok(ArtworkReceipt {
            value: stored.asset.document().clone(),
            json_bytes: stored.document_bytes,
        })
    }
    pub async fn artwork(
        &self,
        token: &str,
        work: &str,
        revision: Option<u64>,
    ) -> Result<dto::WorkArtworkResponse, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        let revision = revision
            .map(Revision::new)
            .transpose()
            .map_err(ArtworkError::from)?;
        self.repository.artwork(scope, revision).await
    }
    pub async fn set_artwork(
        &self,
        token: &str,
        work: &str,
        raw: dto::SetWorkArtworkRequest,
    ) -> Result<ArtworkReceipt<dto::WorkArtworkResponse>, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        let request = SetWorkArtwork::admit(&raw)?;
        let context = self
            .repository
            .artwork_context(scope.clone(), request.clone())
            .await?;
        if let Some(replay) = context.replay {
            return Ok(replay);
        }
        if let ArtworkChangeValue::Set(selection) = request.change() {
            let asset = context.asset.as_ref().ok_or(ArtworkError::MissingAsset)?;
            selection.require_asset(&context.work, &asset.asset)?;
            self.objects.verify(asset.clone()).await?;
        }
        self.repository
            .set_artwork(scope, request, context.asset)
            .await
    }
    /// Always read the recorded target. No image range/grant/public cache is introduced.
    pub async fn preview(
        &self,
        token: &str,
        work: &str,
        id: &str,
    ) -> Result<PrivateImagePreview, ArtworkApplicationError> {
        let scope = OwnerWorkScope::new(token, work)?;
        let id = ImageAssetId::parse(id)?;
        let stored = self.repository.asset(scope.clone(), id.clone()).await?;
        let metadata = stored.asset.metadata().clone();
        let opened = self.objects.open(stored.clone()).await?;
        if opened.byte_length != metadata.byte_length() {
            return Err(ImageObjectError::Corrupt.into());
        }
        let current = self.repository.asset(scope, id).await?;
        if !same_stored_asset(&stored, &current) {
            return Err(StoreError::CorruptRevision.into());
        }
        Ok(PrivateImagePreview {
            metadata,
            stream: opened.stream,
        })
    }
}

pub struct ImageUploadTicket {
    service: ArtworkService,
    scope: OwnerWorkScope,
    id: ImageIntentId,
    mime: ImageMime,
    expected: ExpectedImage,
    admission: ImageAdmission,
}
impl ImageUploadTicket {
    pub async fn admit(
        self,
        bytes: Vec<u8>,
    ) -> Result<ArtworkReceipt<dto::ImageAssetDocument>, ArtworkApplicationError> {
        if bytes.is_empty() {
            return Err(ImageError::EmptyImage.into());
        }
        if bytes.len() as u64 > MAX_IMAGE_BYTES {
            return Err(ImageError::ImageTooLarge.into());
        }
        // Declarations reject mismatching encoded bodies before expensive decoding. The
        // sealed inspector still independently derives these facts from the actual bytes.
        if format!("{:x}", Sha256::digest(&bytes)) != self.expected.sha256() {
            return Err(ImageError::ExpectedHashMismatch.into());
        }
        if bytes.len() as u64 != self.expected.byte_length() {
            return Err(ImageError::ExpectedLengthMismatch.into());
        }
        let image = self.admission.inspect(bytes, self.mime).await?;
        let metadata = image.metadata().clone();
        // Revalidate declared hash/size before any object write. The repository call ends its
        // transaction before the sealed payload crosses the actual private-object boundary.
        let context = self
            .service
            .repository
            .admission_context(self.scope.clone(), self.id.clone())
            .await?;
        validate_admission_context(&self.scope, &self.id, &context)?;
        context
            .intent
            .intent
            .request()
            .expected()
            .matches(&metadata)?;
        if let Some(asset) = context.asset {
            if asset.asset.metadata() != &metadata {
                return Err(ImageError::BindingMismatch.into());
            }
            self.service.objects.verify(asset.clone()).await?;
            let current = self
                .service
                .repository
                .asset(self.scope, asset.asset.id().clone())
                .await?;
            if !same_stored_asset(&asset, &current) {
                return Err(StoreError::CorruptRevision.into());
            }
            return Ok(ArtworkReceipt {
                value: current.asset.document().clone(),
                json_bytes: current.document_bytes,
            });
        }
        let location = self.service.objects.persist(image).await?;
        let receipt = self
            .service
            .repository
            .finalize(self.scope.clone(), self.id.clone(), metadata, location)
            .await?;
        // Another equal admission may have committed on a different recorded target while
        // this write was in flight. Verify the authoritative accepted target, then reauthorize.
        let id = ImageAssetId::parse(&receipt.value.id).map_err(|_| StoreError::CorruptRevision)?;
        let stored = self
            .service
            .repository
            .asset(self.scope.clone(), id.clone())
            .await?;
        if stored.asset.document() != &receipt.value || stored.document_bytes != receipt.json_bytes
        {
            return Err(StoreError::CorruptRevision.into());
        }
        self.service.objects.verify(stored.clone()).await?;
        let current = self.service.repository.asset(self.scope, id).await?;
        if !same_stored_asset(&stored, &current) {
            return Err(StoreError::CorruptRevision.into());
        }
        Ok(receipt)
    }
}

pub struct PrivateImagePreview {
    pub metadata: ImageMetadata,
    pub stream: ImageByteStream,
}

fn validate_admission_context(
    scope: &OwnerWorkScope,
    id: &ImageIntentId,
    context: &AdmissionContext,
) -> Result<(), ArtworkApplicationError> {
    let intent = &context.intent.intent;
    if context.work.id() != &scope.work_id
        || intent.work_id() != &scope.work_id
        || intent.owner_id() != context.work.owner_id()
        || intent.id() != id
    {
        return Err(StoreError::CorruptRevision.into());
    }
    if let Some(asset) = &context.asset {
        asset
            .asset
            .require_intent(intent)
            .map_err(|_| StoreError::CorruptRevision)?;
        intent
            .state(Some(&asset.asset), context.now)
            .map_err(|_| StoreError::CorruptRevision)?;
    } else {
        if context.work.lifecycle() != Lifecycle::Draft {
            return Err(ArtworkError::from(
                crate::contexts::catalog::domain::CatalogError::WorkArchived,
            )
            .into());
        }
        intent.require_pending(context.now)?;
    }
    Ok(())
}

pub(super) fn same_stored_asset(left: &StoredImageAsset, right: &StoredImageAsset) -> bool {
    left.asset == right.asset
        && left.storage_target == right.storage_target
        && left.object_key == right.object_key
        && left.document_bytes == right.document_bytes
}

#[cfg(test)]
mod tests;
