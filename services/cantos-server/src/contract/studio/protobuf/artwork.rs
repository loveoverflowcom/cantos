//! Exhaustive fallible edge adapters for the reviewed private artwork metadata schema.
use super::{
    generated_artwork as generated, required, scan, ProtobufDto, MAX_ARTWORK_PROTOBUF_BYTES,
    MAX_ARTWORK_RIGHTS_REFS,
};
use crate::contract::codec::{DecodeError, EncodeError};
use crate::contract::studio::{
    CreateImageIntentRequest, ImageAssetDocument, ImageDeclarations, ImageIntentDocument,
    ImageIntentResponse, ImageIntentState, ImageMime, ImageVariantMetadata, SetWorkArtworkRequest,
    WorkArtworkResponse, WorkArtworkSelection,
};
use prost::Message;

trait Bounds {
    fn check_bounds(&self) -> Result<(), EncodeError>;
}
fn texts<'a>(values: impl IntoIterator<Item = &'a str>) -> Result<(), EncodeError> {
    let mut length = 0_usize;
    for value in values {
        length = length.checked_add(value.len()).ok_or(EncodeError)?;
        if length > MAX_ARTWORK_PROTOBUF_BYTES {
            return Err(EncodeError);
        }
    }
    Ok(())
}
fn encode<M: Message>(schema: scan::Schema, value: &M) -> Result<Vec<u8>, EncodeError> {
    if value.encoded_len() > MAX_ARTWORK_PROTOBUF_BYTES {
        return Err(EncodeError);
    }
    let bytes = value.encode_to_vec();
    scan::admit_artwork(schema, &bytes).map_err(|_| EncodeError)?;
    Ok(bytes)
}
fn decode<M: Message + Default>(schema: scan::Schema, bytes: &[u8]) -> Result<M, DecodeError> {
    scan::admit_artwork(schema, bytes)?;
    M::decode(bytes).map_err(|_| DecodeError::Malformed)
}

impl From<ImageMime> for generated::ImageMime {
    fn from(value: ImageMime) -> Self {
        match value {
            ImageMime::Png => Self::Png,
            ImageMime::Jpeg => Self::Jpeg,
        }
    }
}
impl TryFrom<i32> for ImageMime {
    type Error = DecodeError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match generated::ImageMime::try_from(value).map_err(|_| DecodeError::Shape)? {
            generated::ImageMime::Png => Ok(Self::Png),
            generated::ImageMime::Jpeg => Ok(Self::Jpeg),
            generated::ImageMime::Unspecified => Err(DecodeError::Shape),
        }
    }
}
impl From<ImageIntentState> for generated::ImageIntentState {
    fn from(value: ImageIntentState) -> Self {
        match value {
            ImageIntentState::Pending => Self::Pending,
            ImageIntentState::Admitted => Self::Admitted,
            ImageIntentState::Expired => Self::Expired,
        }
    }
}
impl TryFrom<i32> for ImageIntentState {
    type Error = DecodeError;
    fn try_from(value: i32) -> Result<Self, Self::Error> {
        match generated::ImageIntentState::try_from(value).map_err(|_| DecodeError::Shape)? {
            generated::ImageIntentState::Pending => Ok(Self::Pending),
            generated::ImageIntentState::Admitted => Ok(Self::Admitted),
            generated::ImageIntentState::Expired => Ok(Self::Expired),
            generated::ImageIntentState::Unspecified => Err(DecodeError::Shape),
        }
    }
}

impl Bounds for ImageDeclarations {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        if self.rights_refs.len() > MAX_ARTWORK_RIGHTS_REFS {
            return Err(EncodeError);
        }
        texts(
            std::iter::once(self.provenance.as_str())
                .chain(self.rights_refs.iter().map(String::as_str)),
        )
    }
}
impl From<&ImageDeclarations> for generated::ImageDeclarations {
    fn from(value: &ImageDeclarations) -> Self {
        let ImageDeclarations {
            provenance,
            rights_refs,
        } = value;
        Self {
            provenance: Some(provenance.clone()),
            rights_refs: rights_refs.clone(),
        }
    }
}
impl TryFrom<generated::ImageDeclarations> for ImageDeclarations {
    type Error = DecodeError;
    fn try_from(value: generated::ImageDeclarations) -> Result<Self, Self::Error> {
        let generated::ImageDeclarations {
            provenance,
            rights_refs,
        } = value;
        Ok(Self {
            provenance: required(provenance)?,
            rights_refs,
        })
    }
}

impl Bounds for CreateImageIntentRequest {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([
            self.operation_id.as_str(),
            self.id.as_str(),
            self.expected_sha256.as_str(),
        ])?;
        self.declarations.check_bounds()
    }
}
impl From<&CreateImageIntentRequest> for generated::CreateImageIntentRequest {
    fn from(value: &CreateImageIntentRequest) -> Self {
        let CreateImageIntentRequest {
            operation_id,
            id,
            expected_sha256,
            expected_byte_length,
            content_type,
            declarations,
        } = value;
        Self {
            operation_id: Some(operation_id.clone()),
            id: Some(id.clone()),
            expected_sha256: Some(expected_sha256.clone()),
            expected_byte_length: Some(*expected_byte_length),
            content_type: Some(generated::ImageMime::from(*content_type) as i32),
            declarations: Some(declarations.into()),
        }
    }
}
impl TryFrom<generated::CreateImageIntentRequest> for CreateImageIntentRequest {
    type Error = DecodeError;
    fn try_from(value: generated::CreateImageIntentRequest) -> Result<Self, Self::Error> {
        let generated::CreateImageIntentRequest {
            operation_id,
            id,
            expected_sha256,
            expected_byte_length,
            content_type,
            declarations,
        } = value;
        Ok(Self {
            operation_id: required(operation_id)?,
            id: required(id)?,
            expected_sha256: required(expected_sha256)?,
            expected_byte_length: required(expected_byte_length)?,
            content_type: required(content_type)?.try_into()?,
            declarations: required(declarations)?.try_into()?,
        })
    }
}

impl Bounds for ImageIntentDocument {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([
            self.schema_version.as_str(),
            self.id.as_str(),
            self.work_id.as_str(),
            self.owner_id.as_str(),
            self.operation_id.as_str(),
            self.digest.as_str(),
            self.profile_id.as_str(),
        ])?;
        self.request.check_bounds()
    }
}
impl From<&ImageIntentDocument> for generated::ImageIntentDocument {
    fn from(value: &ImageIntentDocument) -> Self {
        let ImageIntentDocument {
            schema_version,
            id,
            work_id,
            owner_id,
            operation_id,
            request,
            created_at_ms,
            expires_at_ms,
            digest,
            profile_id,
        } = value;
        Self {
            schema_version: Some(schema_version.clone()),
            id: Some(id.clone()),
            work_id: Some(work_id.clone()),
            owner_id: Some(owner_id.clone()),
            operation_id: Some(operation_id.clone()),
            request: Some(request.into()),
            created_at_ms: Some(*created_at_ms),
            expires_at_ms: Some(*expires_at_ms),
            digest: Some(digest.clone()),
            profile_id: Some(profile_id.clone()),
        }
    }
}
impl TryFrom<generated::ImageIntentDocument> for ImageIntentDocument {
    type Error = DecodeError;
    fn try_from(value: generated::ImageIntentDocument) -> Result<Self, Self::Error> {
        let generated::ImageIntentDocument {
            schema_version,
            id,
            work_id,
            owner_id,
            operation_id,
            request,
            created_at_ms,
            expires_at_ms,
            digest,
            profile_id,
        } = value;
        Ok(Self {
            schema_version: required(schema_version)?,
            id: required(id)?,
            work_id: required(work_id)?,
            owner_id: required(owner_id)?,
            operation_id: required(operation_id)?,
            request: required(request)?.try_into()?,
            created_at_ms: required(created_at_ms)?,
            expires_at_ms: required(expires_at_ms)?,
            digest: required(digest)?,
            profile_id: required(profile_id)?,
        })
    }
}

impl Bounds for ImageVariantMetadata {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([self.sha256.as_str(), self.profile_id.as_str()])
    }
}
impl From<&ImageVariantMetadata> for generated::ImageVariantMetadata {
    fn from(value: &ImageVariantMetadata) -> Self {
        let ImageVariantMetadata {
            sha256,
            byte_length,
            content_type,
            width_px,
            height_px,
            profile_id,
        } = value;
        Self {
            sha256: Some(sha256.clone()),
            byte_length: Some(*byte_length),
            content_type: Some(generated::ImageMime::from(*content_type) as i32),
            width_px: Some(*width_px),
            height_px: Some(*height_px),
            profile_id: Some(profile_id.clone()),
        }
    }
}
impl TryFrom<generated::ImageVariantMetadata> for ImageVariantMetadata {
    type Error = DecodeError;
    fn try_from(value: generated::ImageVariantMetadata) -> Result<Self, Self::Error> {
        let generated::ImageVariantMetadata {
            sha256,
            byte_length,
            content_type,
            width_px,
            height_px,
            profile_id,
        } = value;
        Ok(Self {
            sha256: required(sha256)?,
            byte_length: required(byte_length)?,
            content_type: required(content_type)?.try_into()?,
            width_px: required(width_px)?,
            height_px: required(height_px)?,
            profile_id: required(profile_id)?,
        })
    }
}

impl Bounds for ImageAssetDocument {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([
            self.schema_version.as_str(),
            self.id.as_str(),
            self.intent_id.as_str(),
            self.work_id.as_str(),
            self.owner_id.as_str(),
            self.digest.as_str(),
        ])?;
        self.verified_media.check_bounds()?;
        self.declarations.check_bounds()
    }
}
impl From<&ImageAssetDocument> for generated::ImageAssetDocument {
    fn from(value: &ImageAssetDocument) -> Self {
        let ImageAssetDocument {
            schema_version,
            id,
            intent_id,
            work_id,
            owner_id,
            verified_media,
            declarations,
            admitted_at_ms,
            digest,
        } = value;
        Self {
            schema_version: Some(schema_version.clone()),
            id: Some(id.clone()),
            intent_id: Some(intent_id.clone()),
            work_id: Some(work_id.clone()),
            owner_id: Some(owner_id.clone()),
            verified_media: Some(verified_media.into()),
            declarations: Some(declarations.into()),
            admitted_at_ms: Some(*admitted_at_ms),
            digest: Some(digest.clone()),
        }
    }
}
impl TryFrom<generated::ImageAssetDocument> for ImageAssetDocument {
    type Error = DecodeError;
    fn try_from(value: generated::ImageAssetDocument) -> Result<Self, Self::Error> {
        let generated::ImageAssetDocument {
            schema_version,
            id,
            intent_id,
            work_id,
            owner_id,
            verified_media,
            declarations,
            admitted_at_ms,
            digest,
        } = value;
        Ok(Self {
            schema_version: required(schema_version)?,
            id: required(id)?,
            intent_id: required(intent_id)?,
            work_id: required(work_id)?,
            owner_id: required(owner_id)?,
            verified_media: required(verified_media)?.try_into()?,
            declarations: required(declarations)?.try_into()?,
            admitted_at_ms: required(admitted_at_ms)?,
            digest: required(digest)?,
        })
    }
}

impl Bounds for ImageIntentResponse {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        self.intent.check_bounds()?;
        if let Some(asset) = &self.asset {
            asset.check_bounds()?;
        }
        Ok(())
    }
}
impl From<&ImageIntentResponse> for generated::ImageIntentResponse {
    fn from(value: &ImageIntentResponse) -> Self {
        let ImageIntentResponse {
            intent,
            asset,
            state,
        } = value;
        Self {
            intent: Some(intent.into()),
            asset: asset.as_ref().map(Into::into),
            state: Some(generated::ImageIntentState::from(*state) as i32),
        }
    }
}
impl TryFrom<generated::ImageIntentResponse> for ImageIntentResponse {
    type Error = DecodeError;
    fn try_from(value: generated::ImageIntentResponse) -> Result<Self, Self::Error> {
        let generated::ImageIntentResponse {
            intent,
            asset,
            state,
        } = value;
        Ok(Self {
            intent: required(intent)?.try_into()?,
            asset: asset.map(TryInto::try_into).transpose()?,
            state: required(state)?.try_into()?,
        })
    }
}

impl Bounds for WorkArtworkSelection {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([
            self.image_asset_id.as_str(),
            self.image_asset_digest.as_str(),
            self.alt_text.as_str(),
        ])
    }
}
impl From<&WorkArtworkSelection> for generated::WorkArtworkSelection {
    fn from(value: &WorkArtworkSelection) -> Self {
        let WorkArtworkSelection {
            image_asset_id,
            image_asset_digest,
            alt_text,
        } = value;
        Self {
            image_asset_id: Some(image_asset_id.clone()),
            image_asset_digest: Some(image_asset_digest.clone()),
            alt_text: Some(alt_text.clone()),
        }
    }
}
impl TryFrom<generated::WorkArtworkSelection> for WorkArtworkSelection {
    type Error = DecodeError;
    fn try_from(value: generated::WorkArtworkSelection) -> Result<Self, Self::Error> {
        let generated::WorkArtworkSelection {
            image_asset_id,
            image_asset_digest,
            alt_text,
        } = value;
        Ok(Self {
            image_asset_id: required(image_asset_id)?,
            image_asset_digest: required(image_asset_digest)?,
            alt_text: required(alt_text)?,
        })
    }
}

impl Bounds for SetWorkArtworkRequest {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([self.operation_id.as_str()])?;
        if let Some(artwork) = &self.artwork {
            artwork.check_bounds()?;
        }
        Ok(())
    }
}
impl From<&SetWorkArtworkRequest> for generated::SetWorkArtworkRequest {
    fn from(value: &SetWorkArtworkRequest) -> Self {
        let SetWorkArtworkRequest {
            operation_id,
            expected_work_revision,
            artwork,
        } = value;
        let change = match artwork {
            Some(artwork) => generated::set_work_artwork_request::Change::Artwork(artwork.into()),
            None => generated::set_work_artwork_request::Change::ClearArtwork(
                generated::ClearArtwork {},
            ),
        };
        Self {
            operation_id: Some(operation_id.clone()),
            expected_work_revision: Some(*expected_work_revision),
            change: Some(change),
        }
    }
}
impl TryFrom<generated::SetWorkArtworkRequest> for SetWorkArtworkRequest {
    type Error = DecodeError;
    fn try_from(value: generated::SetWorkArtworkRequest) -> Result<Self, Self::Error> {
        let generated::SetWorkArtworkRequest {
            operation_id,
            expected_work_revision,
            change,
        } = value;
        let artwork = match required(change)? {
            generated::set_work_artwork_request::Change::Artwork(artwork) => {
                Some(artwork.try_into()?)
            }
            generated::set_work_artwork_request::Change::ClearArtwork(
                generated::ClearArtwork {},
            ) => None,
        };
        Ok(Self {
            operation_id: required(operation_id)?,
            expected_work_revision: required(expected_work_revision)?,
            artwork,
        })
    }
}

impl Bounds for WorkArtworkResponse {
    fn check_bounds(&self) -> Result<(), EncodeError> {
        texts([self.work_id.as_str()])?;
        if let Some(artwork) = &self.artwork {
            artwork.check_bounds()?;
        }
        Ok(())
    }
}
impl From<&WorkArtworkResponse> for generated::WorkArtworkResponse {
    fn from(value: &WorkArtworkResponse) -> Self {
        let WorkArtworkResponse {
            work_id,
            work_revision,
            artwork,
            changed_at_ms,
        } = value;
        Self {
            work_id: Some(work_id.clone()),
            work_revision: Some(*work_revision),
            artwork: artwork.as_ref().map(Into::into),
            changed_at_ms: *changed_at_ms,
        }
    }
}
impl TryFrom<generated::WorkArtworkResponse> for WorkArtworkResponse {
    type Error = DecodeError;
    fn try_from(value: generated::WorkArtworkResponse) -> Result<Self, Self::Error> {
        let generated::WorkArtworkResponse {
            work_id,
            work_revision,
            artwork,
            changed_at_ms,
        } = value;
        Ok(Self {
            work_id: required(work_id)?,
            work_revision: required(work_revision)?,
            artwork: artwork.map(TryInto::try_into).transpose()?,
            changed_at_ms,
        })
    }
}

macro_rules! artwork_codec {
    ($dto:ident, $schema:ident) => {
        impl ProtobufDto for $dto {
            fn encode_protobuf(&self) -> Result<Vec<u8>, EncodeError> {
                self.check_bounds()?;
                encode(scan::Schema::$schema, &generated::$dto::from(self))
            }
            fn decode_protobuf(bytes: &[u8]) -> Result<Self, DecodeError> {
                decode::<generated::$dto>(scan::Schema::$schema, bytes)?.try_into()
            }
        }
    };
}
artwork_codec!(ImageDeclarations, ImageDeclarations);
artwork_codec!(CreateImageIntentRequest, CreateImageIntent);
artwork_codec!(ImageIntentDocument, ImageIntent);
artwork_codec!(ImageVariantMetadata, ImageVariant);
artwork_codec!(ImageAssetDocument, ImageAsset);
artwork_codec!(ImageIntentResponse, ImageResponse);
artwork_codec!(WorkArtworkSelection, ArtworkSelection);
artwork_codec!(SetWorkArtworkRequest, SetArtwork);
artwork_codec!(WorkArtworkResponse, ArtworkResponse);
