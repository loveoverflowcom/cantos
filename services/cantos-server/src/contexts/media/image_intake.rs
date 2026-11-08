//! Bounded actual-byte WebP inspection and rendition encoding. Source bytes stay unchanged.
//!
//! Acquire admission after cheap authorization/intent checks and before buffering a body. A
//! cancelled async caller cannot release the lane while its blocking decoder or bytes survive.
use std::sync::{Arc, LazyLock};

use image::{imageops, RgbImage};
use sha2::{Digest, Sha256};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::domain::image::{
    CoverVariant, ImageMetadata, ImageMime, MAX_IMAGE_BYTES, MAX_IMAGE_DIMENSION, MAX_IMAGE_PIXELS,
};

static IMAGE_LANES: LazyLock<std::sync::Arc<Semaphore>> =
    LazyLock::new(|| std::sync::Arc::new(Semaphore::new(2)));

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ImageIntakeError {
    EmptyImage,
    ImageTooLarge,
    UnsupportedFormat,
    FormatMismatch,
    MalformedImage,
    AnimatedImage,
    NonIdentityOrientation,
    ImageLimit,
    AdmissionBusy,
    InspectionUnavailable,
}

impl ImageIntakeError {
    pub fn code(self) -> &'static str {
        match self {
            Self::EmptyImage => "empty_image",
            Self::ImageTooLarge => "image_too_large",
            Self::UnsupportedFormat => "unsupported_image_format",
            Self::FormatMismatch => "image_format_mismatch",
            Self::MalformedImage => "malformed_image",
            Self::AnimatedImage => "animated_image",
            Self::NonIdentityOrientation => "non_identity_image_orientation",
            Self::ImageLimit => "image_limit",
            Self::AdmissionBusy => "image_admission_busy",
            Self::InspectionUnavailable => "image_inspection_unavailable",
        }
    }
}

/// An owned reservation for one of the two process-wide admission/decode lanes.
///
/// Busy admission is refused immediately, matching the existing bounded upload gate. Callers
/// recheck authorization and intent expiry after admission, before reading the body.
#[derive(Debug)]
pub struct ImageAdmission {
    _permit: OwnedSemaphorePermit,
}

impl ImageAdmission {
    pub fn try_acquire() -> Result<Self, ImageIntakeError> {
        Self::from_lanes(IMAGE_LANES.clone())
    }

    /// Independent test lanes exercise the actual decoder and permit lifetime without
    /// competing with application tests that intentionally exhaust production admission.
    #[cfg(test)]
    pub(crate) fn from_test_lanes(
        lanes: std::sync::Arc<Semaphore>,
    ) -> Result<Self, ImageIntakeError> {
        Self::from_lanes(lanes)
    }

    fn from_lanes(lanes: std::sync::Arc<Semaphore>) -> Result<Self, ImageIntakeError> {
        let permit = lanes
            .try_acquire_owned()
            .map_err(|_| ImageIntakeError::AdmissionBusy)?;
        Ok(Self { _permit: permit })
    }

    /// Inspection consumes the reservation. The witness retains it through storage/readback
    /// and until its original encoded body is dropped. No unbounded blocking work is spawned.
    pub fn inspect(
        self,
        bytes: Vec<u8>,
        declared_mime: ImageMime,
    ) -> impl std::future::Future<Output = Result<VerifiedImage, ImageIntakeError>> + Send {
        self.run_blocking(bytes, move |body| {
            inspect_body(body, declared_mime).map(|(image, _)| image)
        })
    }

    pub(super) fn inspect_cover(
        self,
        bytes: Vec<u8>,
        declared_mime: ImageMime,
    ) -> impl std::future::Future<
        Output = Result<(VerifiedImage, [VerifiedImage; 2]), ImageIntakeError>,
    > + Send {
        self.run_blocking(bytes, move |body| {
            let (source, rgb) = inspect_body(body, declared_mime)?;
            let [thumbnail, full] = CoverVariant::ALL;
            let variants = [
                encode_variant(&source, &rgb, thumbnail)?,
                encode_variant(&source, &rgb, full)?,
            ];
            Ok((source, variants))
        })
    }

    fn run_blocking<T: Send + 'static>(
        self,
        bytes: Vec<u8>,
        inspect: impl FnOnce(AdmittedBody) -> Result<T, ImageIntakeError> + Send + 'static,
    ) -> impl std::future::Future<Output = Result<T, ImageIntakeError>> + Send {
        // Bind the body and permit synchronously, even if the returned future is never polled.
        let body = AdmittedBody {
            bytes,
            admission: Arc::new(self),
        };
        async move {
            tokio::task::spawn_blocking(move || inspect(body))
                .await
                .map_err(|_| ImageIntakeError::InspectionUnavailable)?
        }
    }
}

// Declaration order matters: bytes are dropped before admission releases its lane.
struct AdmittedBody {
    bytes: Vec<u8>,
    admission: Arc<ImageAdmission>,
}

impl std::fmt::Debug for AdmittedBody {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AdmittedBody")
            .field("byte_length", &self.bytes.len())
            .finish_non_exhaustive()
    }
}

/// Only successful complete pixel decoding can construct this witness.
/// There is no raw/serde/default constructor, mutable accessor, or audio conversion.
///
/// ```compile_fail
/// use cantos_server::contexts::media::image_intake::VerifiedImage;
/// let forged = VerifiedImage { bytes: vec![1], metadata: unimplemented!() };
/// ```
///
/// ```compile_fail
/// use cantos_server::contexts::media::{audio_intake::VerifiedAudio, image_intake::VerifiedImage};
/// fn audio_only(_: &VerifiedAudio) {}
/// fn forged_audio(image: &VerifiedImage) { audio_only(image); }
/// ```
#[derive(Debug)]
pub struct VerifiedImage {
    body: AdmittedBody,
    metadata: ImageMetadata,
}

impl VerifiedImage {
    pub fn bytes(&self) -> &[u8] {
        &self.body.bytes
    }

    pub fn metadata(&self) -> &ImageMetadata {
        &self.metadata
    }
}

fn inspect_body(
    body: AdmittedBody,
    declared_mime: ImageMime,
) -> Result<(VerifiedImage, RgbImage), ImageIntakeError> {
    if body.bytes.is_empty() {
        return Err(ImageIntakeError::EmptyImage);
    }
    if body.bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(ImageIntakeError::ImageTooLarge);
    }
    if declared_mime != ImageMime::Webp {
        return Err(ImageIntakeError::UnsupportedFormat);
    }
    let dimensions = vp8_dimensions(&body.bytes, None)?;
    let pixels = decode(&body.bytes, dimensions)?;
    Ok((verified(body, dimensions)?, pixels))
}

/// Admit the simple opaque VP8 container. Derivatives add one private-data-free RIFF chunk
/// after VP8 to distinguish the two profiles even when tiny source images need no resize.
fn vp8_dimensions(
    bytes: &[u8],
    variant: Option<CoverVariant>,
) -> Result<(u32, u32), ImageIntakeError> {
    use ImageIntakeError::{AnimatedImage, FormatMismatch, MalformedImage, UnsupportedFormat};
    if !bytes.starts_with(b"RIFF") || bytes.get(8..12) != Some(b"WEBP") {
        return Err(FormatMismatch);
    }
    let riff_len = le_u32(bytes.get(4..8))?;
    if bytes.len().checked_sub(8) != Some(riff_len) {
        return Err(MalformedImage);
    }
    match bytes.get(12..16) {
        Some(b"VP8 ") => {}
        Some(b"VP8X") if bytes.get(20).is_some_and(|flags| flags & 2 != 0) => {
            return Err(AnimatedImage);
        }
        _ => return Err(UnsupportedFormat),
    }
    let chunk_len = le_u32(bytes.get(16..20))?;
    let end = 20usize.checked_add(chunk_len).ok_or(MalformedImage)?;
    let padded = end.checked_add(chunk_len % 2).ok_or(MalformedImage)?;
    let frame = bytes.get(20..end).ok_or(MalformedImage)?;
    let trailer = bytes.get(padded..).ok_or(MalformedImage)?;
    if (end != padded && bytes.get(end) != Some(&0))
        || trailer
            != variant
                .map(variant_chunk)
                .as_ref()
                .map_or(&[][..], |chunk| chunk.as_slice())
        || frame.len() < 10
        || frame[0] & 1 != 0
        || frame[3..6] != [0x9d, 0x01, 0x2a]
    {
        return Err(MalformedImage);
    }
    let width = u32::from(u16::from_le_bytes([frame[6], frame[7]]) & 0x3fff);
    let height = u32::from(u16::from_le_bytes([frame[8], frame[9]]) & 0x3fff);
    if width == 0
        || height == 0
        || width > MAX_IMAGE_DIMENSION
        || height > MAX_IMAGE_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_IMAGE_PIXELS
    {
        return Err(ImageIntakeError::ImageLimit);
    }
    Ok((width, height))
}

fn le_u32(bytes: Option<&[u8]>) -> Result<usize, ImageIntakeError> {
    bytes
        .and_then(|bytes| bytes.try_into().ok())
        .map(|bytes| u32::from_le_bytes(bytes) as usize)
        .ok_or(ImageIntakeError::MalformedImage)
}

fn decode(bytes: &[u8], (width, height): (u32, u32)) -> Result<RgbImage, ImageIntakeError> {
    let image = webp::Decoder::new(bytes)
        .decode()
        .filter(|image| image.width() == width && image.height() == height && !image.is_alpha())
        .ok_or(ImageIntakeError::MalformedImage)?;
    RgbImage::from_raw(width, height, image.to_vec()).ok_or(ImageIntakeError::MalformedImage)
}

fn verified(
    body: AdmittedBody,
    (width, height): (u32, u32),
) -> Result<VerifiedImage, ImageIntakeError> {
    let bytes = &body.bytes;
    let metadata = ImageMetadata::new(
        format!("{:x}", Sha256::digest(bytes)),
        bytes.len() as u64,
        ImageMime::Webp,
        width,
        height,
    )
    .map_err(|_| ImageIntakeError::MalformedImage)?;
    Ok(VerifiedImage { body, metadata })
}

fn variant_chunk(variant: CoverVariant) -> [u8; 10] {
    let profile = match variant {
        CoverVariant::Thumbnail => b'T',
        CoverVariant::Full => b'F',
    };
    [b'C', b'T', b'V', b'R', 2, 0, 0, 0, 1, profile]
}

fn encode_variant(
    source: &VerifiedImage,
    pixels: &RgbImage,
    variant: CoverVariant,
) -> Result<VerifiedImage, ImageIntakeError> {
    let (width, height) = variant.dimensions(source.metadata());
    let resized;
    let pixels = if pixels.dimensions() == (width, height) {
        pixels
    } else {
        resized = imageops::resize(pixels, width, height, imageops::FilterType::Triangle);
        &resized
    };
    let quality = match variant {
        CoverVariant::Thumbnail => 78.0,
        CoverVariant::Full => 82.0,
    };
    let mut bytes = webp::Encoder::from_rgb(pixels.as_raw(), width, height)
        .encode_simple(false, quality)
        .map_err(|_| ImageIntakeError::InspectionUnavailable)?
        .to_vec();
    bytes.extend_from_slice(&variant_chunk(variant));
    let size = u32::try_from(bytes.len() - 8).map_err(|_| ImageIntakeError::ImageLimit)?;
    bytes[4..8].copy_from_slice(&size.to_le_bytes());
    if bytes.len() as u64 > variant.max_bytes() {
        return Err(ImageIntakeError::ImageLimit);
    }
    let dimensions = vp8_dimensions(&bytes, Some(variant))?;
    decode(&bytes, dimensions)?;
    let encoded = verified(
        AdmittedBody {
            bytes,
            admission: source.body.admission.clone(),
        },
        dimensions,
    )?;
    if !variant.matches(source.metadata(), encoded.metadata()) {
        return Err(ImageIntakeError::MalformedImage);
    }
    Ok(encoded)
}

#[cfg(test)]
mod tests;
