//! Bounded actual-byte PNG/JPEG/AVIF inspection. Exact encoded bytes remain private and unchanged.
//!
//! Acquire admission after cheap authorization/intent checks and before buffering a body. A
//! cancelled async caller cannot release the lane while its blocking decoder or bytes survive.
mod avif;
use std::sync::LazyLock;

use image::ImageFormat;
use sha2::{Digest, Sha256};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::domain::image::{ImageMetadata, ImageMime, MAX_IMAGE_BYTES};

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
        self.run_blocking(bytes, move |body| inspect_body(body, declared_mime))
    }

    fn run_blocking(
        self,
        bytes: Vec<u8>,
        inspect: impl FnOnce(AdmittedBody) -> Result<VerifiedImage, ImageIntakeError> + Send + 'static,
    ) -> impl std::future::Future<Output = Result<VerifiedImage, ImageIntakeError>> + Send {
        // Bind the body and permit synchronously, even if the returned future is never polled.
        let body = AdmittedBody {
            bytes,
            _admission: self,
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
    _admission: ImageAdmission,
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
) -> Result<VerifiedImage, ImageIntakeError> {
    let bytes = &body.bytes;
    if bytes.is_empty() {
        return Err(ImageIntakeError::EmptyImage);
    }
    if bytes.len() as u64 > MAX_IMAGE_BYTES {
        return Err(ImageIntakeError::ImageTooLarge);
    }
    let actual_mime = match image::guess_format(bytes) {
        Ok(ImageFormat::Png) => ImageMime::Png,
        Ok(ImageFormat::Jpeg) => ImageMime::Jpeg,
        Ok(ImageFormat::Avif) => ImageMime::Avif,
        _ if is_isobmff_avif_or_avis(bytes) => ImageMime::Avif,
        _ => return Err(ImageIntakeError::UnsupportedFormat),
    };
    if actual_mime != declared_mime {
        return Err(ImageIntakeError::FormatMismatch);
    }
    if actual_mime != ImageMime::Avif {
        return Err(ImageIntakeError::UnsupportedFormat);
    }
    let (width, height) = avif::inspect_avif(bytes)?;
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    let metadata = ImageMetadata::new(sha256, bytes.len() as u64, actual_mime, width, height)
        .map_err(|_| ImageIntakeError::MalformedImage)?;
    Ok(VerifiedImage { body, metadata })
}

fn is_isobmff_avif_or_avis(bytes: &[u8]) -> bool {
    if bytes.len() < 12 || &bytes[4..8] != b"ftyp" {
        return false;
    }
    let size = u32::from_be_bytes(bytes[..4].try_into().unwrap()) as usize;
    let end = size.min(bytes.len());
    if end < 12 {
        return false;
    }
    matches!(&bytes[8..12], b"avif" | b"avis")
        || bytes.get(16..end).is_some_and(|brands| {
            brands
                .as_chunks::<4>()
                .0
                .iter()
                .any(|b| b == b"avif" || b == b"avis")
        })
}

#[cfg(test)]
mod tests;
