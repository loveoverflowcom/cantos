//! Immutable private-image declarations and receipts. Metadata admission checks facts; only
//! the outer image inspector can establish that actual bytes decoded successfully. No storage,
//! decoder, HTTP, clock or authorization implementation enters this module.
use std::collections::BTreeSet;

use crate::contract::studio as dto;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::contexts::catalog::domain::{OwnerId, WorkId};
use crate::platform::ids::canonical_uuid;

pub const MAX_IMAGE_BYTES: u64 = 8_388_608;
pub const MAX_IMAGE_DIMENSION: u32 = 4096;
pub const MAX_IMAGE_PIXELS: u64 = 16_777_216;
pub const MAX_DECODED_IMAGE_BYTES: u64 = 67_108_864;
pub const MAX_IMAGE_DOCUMENT_BYTES: usize = 32_768;
pub const IMAGE_INTENT_LIFETIME_MS: i64 = 1_800_000;
pub const MAX_PROVENANCE_BYTES: usize = 4096;
pub const MAX_IMAGE_RIGHTS_REFS: usize = 64;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageError {
    InvalidIntentId,
    InvalidAssetId,
    InvalidOperationId,
    InvalidWorkId,
    InvalidOwnerId,
    InvalidHash,
    InvalidDigest,
    UnsupportedMime,
    UnsupportedProfile,
    UnsupportedSchema,
    EmptyImage,
    ImageTooLarge,
    InvalidDimensions,
    UnsupportedLayout,
    DecodedImageTooLarge,
    OutputSizeOverflow,
    BlankProvenance,
    ProvenanceTooLong,
    ControlCharacter,
    MissingRightsRefs,
    TooManyRightsRefs,
    InvalidRightsRef,
    DuplicateRightsRef,
    NoncanonicalRightsRefs,
    InvalidTimestamp,
    InvalidExpiry,
    IntentNotYetCreated,
    IntentExpired,
    ExpectedHashMismatch,
    ExpectedLengthMismatch,
    ExpectedMimeMismatch,
    BindingMismatch,
    OperationReused,
    DuplicateIntent,
    DocumentTooLarge,
    Encoding,
}

macro_rules! identity {
    ($(#[$meta:meta])* $name:ident, $error:ident) => {
        $(#[$meta])*
        #[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize)]
        #[serde(transparent)]
        pub struct $name(String);
        impl $name {
            pub fn parse(value: &str) -> Result<Self, ImageError> {
                canonical_uuid(value).map(Self).ok_or(ImageError::$error)
            }
            pub fn as_str(&self) -> &str {
                &self.0
            }
        }
    };
}
identity!(ImageIntentId, InvalidIntentId);
identity!(
    /// An image asset is not an audio rendition or its upload intent.
    /// ```compile_fail,E0308
    /// use cantos_server::contexts::media::domain::image::{ImageAssetId, ImageIntentId};
    /// let asset: ImageAssetId = ImageIntentId::parse("00000000-0000-0000-0000-000000000001").unwrap();
    /// ```
    ImageAssetId, InvalidAssetId
);
identity!(ImageOperationId, InvalidOperationId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageMime {
    Png,
    Jpeg,
}
impl ImageMime {
    pub fn parse(value: &str) -> Result<Self, ImageError> {
        match value {
            "image/png" => Ok(Self::Png),
            "image/jpeg" => Ok(Self::Jpeg),
            _ => Err(ImageError::UnsupportedMime),
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Png => "image/png",
            Self::Jpeg => "image/jpeg",
        }
    }
    pub const fn document(self) -> dto::ImageMime {
        match self {
            Self::Png => dto::ImageMime::Png,
            Self::Jpeg => dto::ImageMime::Jpeg,
        }
    }
}
impl From<dto::ImageMime> for ImageMime {
    fn from(value: dto::ImageMime) -> Self {
        match value {
            dto::ImageMime::Png => Self::Png,
            dto::ImageMime::Jpeg => Self::Jpeg,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageProfile {
    CoverImageV1,
}
impl ImageProfile {
    pub fn parse(value: &str) -> Result<Self, ImageError> {
        match value {
            "cover-image-v1" => Ok(Self::CoverImageV1),
            _ => Err(ImageError::UnsupportedProfile),
        }
    }
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CoverImageV1 => "cover-image-v1",
        }
    }
}

/// Checked allocation facts, never a claim that a decoder ran or bytes are safe to publish.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ImageBounds {
    pixel_count: u64,
    decoded_byte_length: usize,
}
impl ImageBounds {
    pub const fn pixel_count(self) -> u64 {
        self.pixel_count
    }
    pub const fn decoded_byte_length(self) -> usize {
        self.decoded_byte_length
    }
}

/// The production numeric admission kernel. The decoder supplies its actual sample layout;
/// no implicit RGBA conversion or assumed four-byte pixel enters the calculation.
pub const fn check_image_bounds(
    encoded_length: u64,
    width: u32,
    height: u32,
    bytes_per_pixel: u8,
) -> Result<ImageBounds, ImageError> {
    if encoded_length == 0 {
        return Err(ImageError::EmptyImage);
    }
    if encoded_length > MAX_IMAGE_BYTES {
        return Err(ImageError::ImageTooLarge);
    }
    if width == 0 || height == 0 || width > MAX_IMAGE_DIMENSION || height > MAX_IMAGE_DIMENSION {
        return Err(ImageError::InvalidDimensions);
    }
    if !matches!(bytes_per_pixel, 1 | 2 | 3 | 4 | 6 | 8) {
        return Err(ImageError::UnsupportedLayout);
    }
    let pixels = (width as u64) * (height as u64);
    if pixels > MAX_IMAGE_PIXELS {
        return Err(ImageError::InvalidDimensions);
    }
    let output = match pixels.checked_mul(bytes_per_pixel as u64) {
        Some(output) => output,
        None => return Err(ImageError::OutputSizeOverflow),
    };
    if output > MAX_DECODED_IMAGE_BYTES {
        return Err(ImageError::DecodedImageTooLarge);
    }
    if output > usize::MAX as u64 {
        return Err(ImageError::OutputSizeOverflow);
    }
    Ok(ImageBounds {
        pixel_count: pixels,
        decoded_byte_length: output as usize,
    })
}

pub fn sha256_valid(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}
pub(super) fn scheme_digest_valid(value: &str, scheme: &str) -> bool {
    value
        .strip_prefix(scheme)
        .and_then(|suffix| suffix.strip_prefix(":sha256:"))
        .is_some_and(sha256_valid)
}

/// Expected upload facts are declarations, not a VerifiedImage witness.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ExpectedImage {
    sha256: String,
    byte_length: u64,
    content_type: ImageMime,
}
impl ExpectedImage {
    pub fn new(
        sha256: String,
        byte_length: u64,
        content_type: ImageMime,
    ) -> Result<Self, ImageError> {
        if !sha256_valid(&sha256) {
            return Err(ImageError::InvalidHash);
        }
        check_image_bounds(byte_length, 1, 1, 1)?;
        Ok(Self {
            sha256,
            byte_length,
            content_type,
        })
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub const fn content_type(&self) -> ImageMime {
        self.content_type
    }
    pub fn matches(&self, inspected: &ImageMetadata) -> Result<(), ImageError> {
        if self.sha256 != inspected.sha256 {
            return Err(ImageError::ExpectedHashMismatch);
        }
        if self.byte_length != inspected.byte_length {
            return Err(ImageError::ExpectedLengthMismatch);
        }
        if self.content_type != inspected.content_type {
            return Err(ImageError::ExpectedMimeMismatch);
        }
        Ok(())
    }
}

/// Intrinsically valid image facts. It has no Deserialize impl and does not prove inspection.
/// Successful full decoding is represented by the separate outer-edge VerifiedImage type.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageMetadata {
    sha256: String,
    byte_length: u64,
    content_type: ImageMime,
    width_px: u32,
    height_px: u32,
}
impl ImageMetadata {
    pub fn new(
        sha256: String,
        byte_length: u64,
        content_type: ImageMime,
        width_px: u32,
        height_px: u32,
    ) -> Result<Self, ImageError> {
        ExpectedImage::new(sha256.clone(), byte_length, content_type)?;
        // The intrinsic metadata receipt does not contain a decoded layout. Full-layout
        // bounds must already have passed in the inspector; restoration checks pixels here.
        check_image_bounds(byte_length, width_px, height_px, 1)?;
        Ok(Self {
            sha256,
            byte_length,
            content_type,
            width_px,
            height_px,
        })
    }
    pub fn restore(raw: &dto::ImageVariantMetadata) -> Result<Self, ImageError> {
        ImageProfile::parse(&raw.profile_id)?;
        Self::new(
            raw.sha256.clone(),
            raw.byte_length,
            raw.content_type.into(),
            raw.width_px,
            raw.height_px,
        )
    }
    pub fn sha256(&self) -> &str {
        &self.sha256
    }
    pub const fn byte_length(&self) -> u64 {
        self.byte_length
    }
    pub const fn content_type(&self) -> ImageMime {
        self.content_type
    }
    pub const fn width_px(&self) -> u32 {
        self.width_px
    }
    pub const fn height_px(&self) -> u32 {
        self.height_px
    }
    pub const fn profile(&self) -> ImageProfile {
        ImageProfile::CoverImageV1
    }
    pub fn document(&self) -> dto::ImageVariantMetadata {
        dto::ImageVariantMetadata {
            sha256: self.sha256.clone(),
            byte_length: self.byte_length,
            content_type: self.content_type.document(),
            width_px: self.width_px,
            height_px: self.height_px,
            profile_id: self.profile().as_str().into(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageDeclarations {
    provenance: String,
    rights_refs: Vec<String>,
}
impl ImageDeclarations {
    /// New writes canonicalize an unordered set only after rejecting duplicate declarations.
    pub fn new(provenance: String, mut rights_refs: Vec<String>) -> Result<Self, ImageError> {
        validate_declarations(&provenance, &rights_refs)?;
        rights_refs.sort();
        Ok(Self {
            provenance,
            rights_refs,
        })
    }
    /// Historical receipts must already be canonical. Restoration never repairs stored data.
    pub fn restore(raw: &dto::ImageDeclarations) -> Result<Self, ImageError> {
        let admitted = Self::new(raw.provenance.clone(), raw.rights_refs.clone())?;
        if admitted.rights_refs != raw.rights_refs {
            return Err(ImageError::NoncanonicalRightsRefs);
        }
        Ok(admitted)
    }
    pub fn provenance(&self) -> &str {
        &self.provenance
    }
    pub fn rights_refs(&self) -> &[String] {
        &self.rights_refs
    }
    pub fn document(&self) -> dto::ImageDeclarations {
        dto::ImageDeclarations {
            provenance: self.provenance.clone(),
            rights_refs: self.rights_refs.clone(),
        }
    }
}
fn validate_declarations(provenance: &str, rights_refs: &[String]) -> Result<(), ImageError> {
    if provenance.trim().is_empty() {
        return Err(ImageError::BlankProvenance);
    }
    if provenance.len() > MAX_PROVENANCE_BYTES {
        return Err(ImageError::ProvenanceTooLong);
    }
    if provenance
        .chars()
        .any(|character| character.is_control() && !matches!(character, '\n' | '\t'))
    {
        return Err(ImageError::ControlCharacter);
    }
    if rights_refs.is_empty() {
        return Err(ImageError::MissingRightsRefs);
    }
    if rights_refs.len() > MAX_IMAGE_RIGHTS_REFS {
        return Err(ImageError::TooManyRightsRefs);
    }
    let mut distinct = BTreeSet::new();
    for id in rights_refs {
        if !(1..=64).contains(&id.len())
            || !id.bytes().all(|byte| {
                byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-".contains(&byte)
            })
        {
            return Err(ImageError::InvalidRightsRef);
        }
        if !distinct.insert(id) {
            return Err(ImageError::DuplicateRightsRef);
        }
    }
    Ok(())
}

/// A normalized explicit command. The application still resolves owner rights evidence.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PrepareImage {
    id: ImageIntentId,
    operation_id: ImageOperationId,
    expected: ExpectedImage,
    declarations: ImageDeclarations,
    raw: dto::CreateImageIntentRequest,
}
impl PrepareImage {
    pub fn admit(raw: &dto::CreateImageIntentRequest) -> Result<Self, ImageError> {
        let id = ImageIntentId::parse(&raw.id)?;
        let operation_id = ImageOperationId::parse(&raw.operation_id)?;
        let expected = ExpectedImage::new(
            raw.expected_sha256.clone(),
            raw.expected_byte_length,
            raw.content_type.into(),
        )?;
        let declarations = ImageDeclarations::new(
            raw.declarations.provenance.clone(),
            raw.declarations.rights_refs.clone(),
        )?;
        let raw = dto::CreateImageIntentRequest {
            declarations: declarations.document(),
            ..raw.clone()
        };
        bounded_document(&raw)?;
        Ok(Self {
            id,
            operation_id,
            expected,
            declarations,
            raw,
        })
    }
    pub fn id(&self) -> &ImageIntentId {
        &self.id
    }
    pub fn operation_id(&self) -> &ImageOperationId {
        &self.operation_id
    }
    pub fn expected(&self) -> &ExpectedImage {
        &self.expected
    }
    pub fn declarations(&self) -> &ImageDeclarations {
        &self.declarations
    }
    pub fn document(&self) -> &dto::CreateImageIntentRequest {
        &self.raw
    }
    pub fn fingerprint(&self, work: &WorkId) -> Result<String, ImageError> {
        operation_fingerprint("prepare_image", work, &self.raw)
    }
}

#[derive(Serialize)]
struct IntentPreimage<'a> {
    schema_version: &'a str,
    id: &'a str,
    work_id: &'a str,
    owner_id: &'a str,
    operation_id: &'a str,
    request: &'a dto::CreateImageIntentRequest,
    profile_id: &'a str,
    created_at_ms: u64,
    expires_at_ms: u64,
}
fn intent_digest(raw: &dto::ImageIntentDocument) -> Result<String, ImageError> {
    document_digest(
        "ii-r1",
        &IntentPreimage {
            schema_version: &raw.schema_version,
            id: &raw.id,
            work_id: &raw.work_id,
            owner_id: &raw.owner_id,
            operation_id: &raw.operation_id,
            request: &raw.request,
            profile_id: &raw.profile_id,
            created_at_ms: raw.created_at_ms,
            expires_at_ms: raw.expires_at_ms,
        },
    )
}

/// Immutable intent. Serde cannot construct it or bypass restoration checks.
/// ```compile_fail,E0277
/// use cantos_server::contexts::media::domain::image::ImageIntent;
/// let forged = serde_json::from_str::<ImageIntent>("{}");
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageIntent {
    id: ImageIntentId,
    work_id: WorkId,
    owner_id: OwnerId,
    request: PrepareImage,
    created_at_ms: i64,
    expires_at_ms: i64,
    raw: dto::ImageIntentDocument,
}
impl ImageIntent {
    pub fn create(
        request: PrepareImage,
        work_id: &WorkId,
        owner_id: &OwnerId,
        now: i64,
    ) -> Result<Self, ImageError> {
        if now < 0 {
            return Err(ImageError::InvalidTimestamp);
        }
        let expiry = now
            .checked_add(IMAGE_INTENT_LIFETIME_MS)
            .ok_or(ImageError::InvalidExpiry)?;
        let mut raw = dto::ImageIntentDocument {
            schema_version: "image-intent-v1".into(),
            id: request.id.as_str().into(),
            work_id: work_id.as_str().into(),
            owner_id: owner_id.as_str().into(),
            operation_id: request.operation_id.as_str().into(),
            request: request.raw,
            profile_id: ImageProfile::CoverImageV1.as_str().into(),
            created_at_ms: now as u64,
            expires_at_ms: expiry as u64,
            digest: String::new(),
        };
        raw.digest = intent_digest(&raw)?;
        Self::restore(raw)
    }
    pub fn restore(raw: dto::ImageIntentDocument) -> Result<Self, ImageError> {
        bounded_document(&raw)?;
        if raw.schema_version != "image-intent-v1" {
            return Err(ImageError::UnsupportedSchema);
        }
        let id = ImageIntentId::parse(&raw.id)?;
        let work_id = WorkId::parse(&raw.work_id).map_err(|_| ImageError::InvalidWorkId)?;
        let owner_id = OwnerId::parse(&raw.owner_id).map_err(|_| ImageError::InvalidOwnerId)?;
        let request = PrepareImage::admit(&raw.request)?;
        if request.raw != raw.request {
            return Err(ImageError::NoncanonicalRightsRefs);
        }
        if id != request.id || raw.operation_id != request.operation_id.as_str() {
            return Err(ImageError::BindingMismatch);
        }
        ImageProfile::parse(&raw.profile_id)?;
        let created_at_ms = storage_time(raw.created_at_ms)?;
        let expires_at_ms = storage_time(raw.expires_at_ms)?;
        if created_at_ms.checked_add(IMAGE_INTENT_LIFETIME_MS) != Some(expires_at_ms) {
            return Err(ImageError::InvalidExpiry);
        }
        if !scheme_digest_valid(&raw.digest, "ii-r1") || intent_digest(&raw)? != raw.digest {
            return Err(ImageError::InvalidDigest);
        }
        Ok(Self {
            id,
            work_id,
            owner_id,
            request,
            created_at_ms,
            expires_at_ms,
            raw,
        })
    }
    pub fn id(&self) -> &ImageIntentId {
        &self.id
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub fn owner_id(&self) -> &OwnerId {
        &self.owner_id
    }
    pub fn request(&self) -> &PrepareImage {
        &self.request
    }
    pub const fn created_at_ms(&self) -> i64 {
        self.created_at_ms
    }
    pub const fn expires_at_ms(&self) -> i64 {
        self.expires_at_ms
    }
    pub fn document(&self) -> &dto::ImageIntentDocument {
        &self.raw
    }
    pub fn require_pending(&self, now: i64) -> Result<(), ImageError> {
        if now < 0 {
            return Err(ImageError::InvalidTimestamp);
        }
        if now < self.created_at_ms {
            return Err(ImageError::IntentNotYetCreated);
        }
        if now >= self.expires_at_ms {
            return Err(ImageError::IntentExpired);
        }
        Ok(())
    }
    pub fn state(
        &self,
        accepted: Option<&ImageAsset>,
        now: i64,
    ) -> Result<dto::ImageIntentState, ImageError> {
        if let Some(asset) = accepted {
            asset.require_intent(self)?;
            if now < asset.admitted_at_ms {
                return Err(ImageError::InvalidTimestamp);
            }
            return Ok(dto::ImageIntentState::Admitted);
        }
        match self.require_pending(now) {
            Ok(()) => Ok(dto::ImageIntentState::Pending),
            Err(ImageError::IntentExpired) => Ok(dto::ImageIntentState::Expired),
            Err(error) => Err(error),
        }
    }
}

#[derive(Serialize)]
struct AssetPreimage<'a> {
    schema_version: &'a str,
    id: &'a str,
    intent_id: &'a str,
    work_id: &'a str,
    owner_id: &'a str,
    verified_media: &'a dto::ImageVariantMetadata,
    declarations: &'a dto::ImageDeclarations,
    admitted_at_ms: u64,
}
fn asset_digest(raw: &dto::ImageAssetDocument) -> Result<String, ImageError> {
    document_digest(
        "ia-r1",
        &AssetPreimage {
            schema_version: &raw.schema_version,
            id: &raw.id,
            intent_id: &raw.intent_id,
            work_id: &raw.work_id,
            owner_id: &raw.owner_id,
            verified_media: &raw.verified_media,
            declarations: &raw.declarations,
            admitted_at_ms: raw.admitted_at_ms,
        },
    )
}

/// Settled image receipt. Location is a separate checked storage fact and never a client field.
/// Construction assumes the application has inspected and read back the exact immutable object;
/// this pure receipt constructor cannot establish those external effects.
/// ```compile_fail,E0616
/// use cantos_server::contexts::media::domain::image::ImageAsset;
/// fn overwrite(asset: &mut ImageAsset) { asset.raw.digest = String::new(); }
/// ```
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ImageAsset {
    id: ImageAssetId,
    intent_id: ImageIntentId,
    work_id: WorkId,
    owner_id: OwnerId,
    metadata: ImageMetadata,
    declarations: ImageDeclarations,
    admitted_at_ms: i64,
    raw: dto::ImageAssetDocument,
}
impl ImageAsset {
    pub fn create(
        intent: &ImageIntent,
        metadata: ImageMetadata,
        now: i64,
    ) -> Result<Self, ImageError> {
        intent.require_pending(now)?;
        intent.request.expected.matches(&metadata)?;
        let mut raw = dto::ImageAssetDocument {
            schema_version: "image-asset-v1".into(),
            id: intent.id.as_str().into(),
            intent_id: intent.id.as_str().into(),
            work_id: intent.work_id.as_str().into(),
            owner_id: intent.owner_id.as_str().into(),
            verified_media: metadata.document(),
            declarations: intent.request.declarations.document(),
            admitted_at_ms: now as u64,
            digest: String::new(),
        };
        raw.digest = asset_digest(&raw)?;
        Self::restore(raw)
    }
    pub fn restore(raw: dto::ImageAssetDocument) -> Result<Self, ImageError> {
        bounded_document(&raw)?;
        if raw.schema_version != "image-asset-v1" {
            return Err(ImageError::UnsupportedSchema);
        }
        let id = ImageAssetId::parse(&raw.id)?;
        let intent_id = ImageIntentId::parse(&raw.intent_id)?;
        if id.as_str() != intent_id.as_str() {
            return Err(ImageError::BindingMismatch);
        }
        let work_id = WorkId::parse(&raw.work_id).map_err(|_| ImageError::InvalidWorkId)?;
        let owner_id = OwnerId::parse(&raw.owner_id).map_err(|_| ImageError::InvalidOwnerId)?;
        let metadata = ImageMetadata::restore(&raw.verified_media)?;
        let declarations = ImageDeclarations::restore(&raw.declarations)?;
        let admitted_at_ms = storage_time(raw.admitted_at_ms)?;
        if !scheme_digest_valid(&raw.digest, "ia-r1") || asset_digest(&raw)? != raw.digest {
            return Err(ImageError::InvalidDigest);
        }
        Ok(Self {
            id,
            intent_id,
            work_id,
            owner_id,
            metadata,
            declarations,
            admitted_at_ms,
            raw,
        })
    }
    pub fn id(&self) -> &ImageAssetId {
        &self.id
    }
    pub fn intent_id(&self) -> &ImageIntentId {
        &self.intent_id
    }
    pub fn work_id(&self) -> &WorkId {
        &self.work_id
    }
    pub fn owner_id(&self) -> &OwnerId {
        &self.owner_id
    }
    pub fn metadata(&self) -> &ImageMetadata {
        &self.metadata
    }
    pub fn declarations(&self) -> &ImageDeclarations {
        &self.declarations
    }
    pub const fn admitted_at_ms(&self) -> i64 {
        self.admitted_at_ms
    }
    pub fn digest(&self) -> &str {
        &self.raw.digest
    }
    pub fn document(&self) -> &dto::ImageAssetDocument {
        &self.raw
    }
    pub fn require_intent(&self, intent: &ImageIntent) -> Result<(), ImageError> {
        if self.intent_id != intent.id
            || self.work_id != intent.work_id
            || self.owner_id != intent.owner_id
            || self.declarations != intent.request.declarations
        {
            return Err(ImageError::BindingMismatch);
        }
        intent.request.expected.matches(&self.metadata)?;
        intent.require_pending(self.admitted_at_ms)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageAdmissionDecision {
    Create,
    Replay,
}
/// Replays keep their original receipt/time even after expiry. Current authorization and
/// object-integrity rechecks remain application obligations before returning this decision.
pub fn decide_image_admission(
    intent: &ImageIntent,
    metadata: &ImageMetadata,
    accepted: Option<&ImageAsset>,
    now: i64,
) -> Result<ImageAdmissionDecision, ImageError> {
    intent.request.expected.matches(metadata)?;
    if let Some(asset) = accepted {
        asset.require_intent(intent)?;
        if now < asset.admitted_at_ms {
            return Err(ImageError::InvalidTimestamp);
        }
        if asset.metadata != *metadata {
            return Err(ImageError::BindingMismatch);
        }
        return Ok(ImageAdmissionDecision::Replay);
    }
    intent.require_pending(now)?;
    Ok(ImageAdmissionDecision::Create)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImagePrepareDecision {
    Create,
    Replay,
}
pub fn decide_image_prepare(
    request: &PrepareImage,
    work: &WorkId,
    prior_operation_fingerprint: Option<&str>,
    existing_intent: Option<&ImageIntent>,
) -> Result<ImagePrepareDecision, ImageError> {
    let requested = request.fingerprint(work)?;
    if let Some(prior) = prior_operation_fingerprint {
        return if prior == requested {
            Ok(ImagePrepareDecision::Replay)
        } else {
            Err(ImageError::OperationReused)
        };
    }
    if existing_intent.is_some() {
        return Err(ImageError::DuplicateIntent);
    }
    Ok(ImagePrepareDecision::Create)
}

pub(super) fn storage_time(raw: u64) -> Result<i64, ImageError> {
    i64::try_from(raw).map_err(|_| ImageError::InvalidTimestamp)
}
pub(super) fn bounded_document<T: Serialize>(value: &T) -> Result<(), ImageError> {
    let bytes = serde_json::to_vec(value).map_err(|_| ImageError::Encoding)?;
    if bytes.is_empty() || bytes.len() > MAX_IMAGE_DOCUMENT_BYTES {
        return Err(ImageError::DocumentTooLarge);
    }
    Ok(())
}
/// Serialize the reviewed ordered preimage directly, without a map round trip or digest field.
pub(super) fn document_digest<T: Serialize>(
    scheme: &str,
    preimage: &T,
) -> Result<String, ImageError> {
    let bytes = serde_json::to_vec(preimage).map_err(|_| ImageError::Encoding)?;
    let mut hash = Sha256::new();
    hash.update(format!("cantos/{scheme}\n"));
    hash.update(bytes);
    Ok(format!("{scheme}:sha256:{:x}", hash.finalize()))
}
pub(super) fn operation_fingerprint<T: Serialize>(
    kind: &'static str,
    work: &WorkId,
    request: &T,
) -> Result<String, ImageError> {
    #[derive(Serialize)]
    struct OperationPreimage<'a, T> {
        kind: &'static str,
        work_id: &'a str,
        request: &'a T,
    }
    document_digest(
        "image-operation1",
        &OperationPreimage {
            kind,
            work_id: work.as_str(),
            request,
        },
    )
}

#[cfg(test)]
#[path = "image_tests.rs"]
mod tests;

#[cfg(kani)]
mod kani_proofs {
    use super::check_image_bounds;

    /// Full symbolic u64 length/u32 dimensions/u8 sample size, no assumptions or stubs.
    /// Ordinary production numeric kernel only: no decoder, allocation, storage or authority.
    #[kani::proof]
    #[kani::unwind(2)]
    fn image_bounds_admission() {
        let length: u64 = kani::any();
        let width: u32 = kani::any();
        let height: u32 = kani::any();
        let layout: u8 = kani::any();
        let pixels = (width as u128) * (height as u128);
        let output = pixels * (layout as u128);
        let supported = matches!(layout, 1 | 2 | 3 | 4 | 6 | 8);
        // Independent widened specification: generate invalid raw inputs as well as valid.
        let admitted = length >= 1
            && length <= 8_388_608
            && width >= 1
            && width <= 4096
            && height >= 1
            && height <= 4096
            && supported
            && pixels <= 16_777_216
            && output <= 67_108_864
            && output <= usize::MAX as u128;
        let production = check_image_bounds(length, width, height, layout);
        match production {
            Ok(bounds) => {
                assert!(admitted);
                assert!(bounds.pixel_count() as u128 == pixels);
                assert!(bounds.decoded_byte_length() as u128 == output);
            }
            Err(_) => assert!(!admitted),
        }
        kani::cover!(
            length == 1 && width == 1 && height == 1 && layout == 1,
            "minimum accepted image"
        );
        kani::cover!(
            length == 8_388_608 && width == 4096 && height == 4096 && layout == 4,
            "exact encoded dimension pixel and output maxima accepted"
        );
        kani::cover!(
            length == 1 && width == 2048 && height == 4096 && layout == 8,
            "sixteen bit layout exact output maximum accepted"
        );
        kani::cover!(
            length == 0 && width == 1 && height == 1 && layout == 1,
            "zero encoded length rejected"
        );
        kani::cover!(
            length == 8_388_609 && width == 1 && height == 1 && layout == 1,
            "encoded length above maximum rejected"
        );
        kani::cover!(
            length == 1 && width == 0 && height == 1 && layout == 1,
            "zero width rejected"
        );
        kani::cover!(
            length == 1 && width == 1 && height == 0 && layout == 1,
            "zero height rejected"
        );
        kani::cover!(
            length == 1 && width == 4097 && height == 1 && layout == 1,
            "width above maximum rejected"
        );
        kani::cover!(
            length == 1 && width == 1 && height == 4097 && layout == 1,
            "height above maximum rejected"
        );
        kani::cover!(
            length == 1 && width == 1 && height == 1 && layout == 0,
            "zero layout rejected"
        );
        kani::cover!(
            length == 1 && width == 1 && height == 1 && layout == 5,
            "unsupported layout rejected"
        );
        kani::cover!(
            length == 1 && width == 2049 && height == 4096 && layout == 8,
            "decoded output above maximum rejected"
        );
        kani::cover!(
            length == u64::MAX && width == u32::MAX && height == u32::MAX && layout == u8::MAX,
            "full machine width maxima rejected"
        );
    }
}
