//! Bounded actual-byte PNG/JPEG inspection. Exact encoded bytes remain private and unchanged.
//!
//! Acquire admission after cheap authorization/intent checks and before buffering a body. A
//! cancelled async caller cannot release the lane while its blocking decoder or bytes survive.
use std::{io::Cursor, sync::LazyLock};

use image::{
    codecs::{jpeg::JpegDecoder, png::PngDecoder},
    metadata::Orientation,
    ImageDecoder, ImageError as DecoderImageError, ImageFormat, Limits,
};
use sha2::{Digest, Sha256};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

use super::domain::image::{
    check_image_bounds, ImageMetadata, ImageMime, MAX_DECODED_IMAGE_BYTES, MAX_IMAGE_BYTES,
    MAX_IMAGE_DIMENSION,
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
/// use cantos_api::contexts::media::image_intake::VerifiedImage;
/// let forged = VerifiedImage { bytes: vec![1], metadata: unimplemented!() };
/// ```
///
/// ```compile_fail
/// use cantos_api::contexts::media::{audio_intake::VerifiedAudio, image_intake::VerifiedImage};
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

fn limits() -> Limits {
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_IMAGE_DIMENSION);
    limits.max_image_height = Some(MAX_IMAGE_DIMENSION);
    limits.max_alloc = Some(MAX_DECODED_IMAGE_BYTES);
    limits
}

fn decoder_error(error: DecoderImageError) -> ImageIntakeError {
    match error {
        DecoderImageError::Limits(_) => ImageIntakeError::ImageLimit,
        DecoderImageError::Unsupported(_) => ImageIntakeError::UnsupportedFormat,
        _ => ImageIntakeError::MalformedImage,
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
        _ => return Err(ImageIntakeError::UnsupportedFormat),
    };
    if actual_mime != declared_mime {
        return Err(ImageIntakeError::FormatMismatch);
    }
    let (width, height) = match actual_mime {
        ImageMime::Png => {
            let has_icc_profile = check_png_envelope(bytes)?;
            // Supply limits before PNG header and ancillary metadata parsing, not afterwards.
            let mut decoder =
                PngDecoder::with_limits(Cursor::new(bytes), limits()).map_err(decoder_error)?;
            if decoder.is_apng().map_err(decoder_error)? {
                return Err(ImageIntakeError::AnimatedImage);
            }
            // png0.18 can silently ignore malformed/over-budget iCCP errors. An explicitly
            // supplied profile must actually survive the bounded parser. The image API briefly
            // clones a valid profile here; neither this nor max_alloc is a total-RSS bound.
            if has_icc_profile && decoder.icc_profile().map_err(decoder_error)?.is_none() {
                return Err(ImageIntakeError::MalformedImage);
            }
            decode_pixels(decoder, bytes.len() as u64)?
        }
        ImageMime::Jpeg => {
            check_jpeg_envelope(bytes)?;
            // The constructor copies only this <=8MiB input and parses headers. Output and
            // dimensions are bounded before pixel decode; max_alloc is not an RSS guarantee.
            let mut decoder = JpegDecoder::new(Cursor::new(bytes)).map_err(decoder_error)?;
            decoder.set_limits(limits()).map_err(decoder_error)?;
            if decoder.orientation().map_err(decoder_error)? != Orientation::NoTransforms {
                return Err(ImageIntakeError::NonIdentityOrientation);
            }
            decode_pixels(decoder, bytes.len() as u64)?
        }
    };
    let sha256 = format!("{:x}", Sha256::digest(bytes));
    let metadata = ImageMetadata::new(sha256, bytes.len() as u64, actual_mime, width, height)
        .map_err(|_| ImageIntakeError::MalformedImage)?;
    Ok(VerifiedImage { body, metadata })
}

fn decode_pixels(
    decoder: impl ImageDecoder,
    encoded_length: u64,
) -> Result<(u32, u32), ImageIntakeError> {
    let (width, height) = decoder.dimensions();
    let bounds = check_image_bounds(
        encoded_length,
        width,
        height,
        decoder.color_type().bytes_per_pixel(),
    )
    .map_err(|_| ImageIntakeError::ImageLimit)?;
    if decoder.total_bytes() != bounds.decoded_byte_length() as u64 {
        return Err(ImageIntakeError::MalformedImage);
    }
    let decoded_length = bounds.decoded_byte_length();
    let mut pixels = Vec::new();
    pixels
        .try_reserve_exact(decoded_length)
        .map_err(|_| ImageIntakeError::InspectionUnavailable)?;
    pixels.resize(decoded_length, 0);
    decoder.read_image(&mut pixels).map_err(decoder_error)?;
    // Discard decoded pixels. Retain every original byte, with no rotation or conversion.
    Ok((width, height))
}

// These bounded framing checks supplement actual decoders. They never confer acceptance:
// decode_pixels must still succeed. image's decoders can stop before trailing metadata/EOF;
// reject missing terminators, appended payloads and PNG animation chunks anywhere in the file.
fn check_png_envelope(bytes: &[u8]) -> Result<bool, ImageIntakeError> {
    let mut offset = 8_usize;
    let mut has_idat = false;
    let mut has_icc_profile = false;
    while let Some(header) = bytes.get(offset..).and_then(|tail| tail.get(..8)) {
        let length = u32::from_be_bytes(header[..4].try_into().unwrap()) as usize;
        let end = offset
            .checked_add(12)
            .and_then(|value| value.checked_add(length))
            .filter(|end| *end <= bytes.len())
            .ok_or(ImageIntakeError::MalformedImage)?;
        match &header[4..] {
            b"acTL" | b"fcTL" | b"fdAT" => return Err(ImageIntakeError::AnimatedImage),
            b"IEND" => {
                return if length == 0 && end == bytes.len() {
                    Ok(has_icc_profile)
                } else {
                    Err(ImageIntakeError::MalformedImage)
                };
            }
            b"iCCP" => {
                if has_idat || has_icc_profile {
                    return Err(ImageIntakeError::MalformedImage);
                }
                has_icc_profile = true;
                offset = end;
            }
            b"IDAT" => {
                has_idat = true;
                offset = end;
            }
            _ => offset = end,
        }
    }
    Err(ImageIntakeError::MalformedImage)
}

fn check_jpeg_envelope(bytes: &[u8]) -> Result<(), ImageIntakeError> {
    let mut offset = 2_usize;
    let mut in_scan = false;
    let mut saw_scan = false;
    while offset < bytes.len() {
        if in_scan {
            let next = bytes[offset..]
                .iter()
                .position(|byte| *byte == 0xff)
                .ok_or(ImageIntakeError::MalformedImage)?;
            offset += next;
        } else if bytes[offset] != 0xff {
            return Err(ImageIntakeError::MalformedImage);
        }
        while bytes.get(offset) == Some(&0xff) {
            offset += 1;
        }
        let marker = *bytes.get(offset).ok_or(ImageIntakeError::MalformedImage)?;
        offset += 1;
        if in_scan && (marker == 0 || (0xd0..=0xd7).contains(&marker)) {
            continue;
        }
        in_scan = false;
        match marker {
            0xd9 => {
                return if saw_scan && offset == bytes.len() {
                    Ok(())
                } else {
                    Err(ImageIntakeError::MalformedImage)
                };
            }
            0x01 => continue,
            0x00 | 0xd0..=0xd8 => return Err(ImageIntakeError::MalformedImage),
            _ => {}
        }
        let length_bytes = bytes
            .get(offset..)
            .and_then(|tail| tail.get(..2))
            .ok_or(ImageIntakeError::MalformedImage)?;
        let length = usize::from(u16::from_be_bytes(length_bytes.try_into().unwrap()));
        let end = offset
            .checked_add(length)
            .filter(|end| length >= 2 && *end <= bytes.len())
            .ok_or(ImageIntakeError::MalformedImage)?;
        if marker == 0xe1 {
            if let Some(exif) = bytes[offset + 2..end].strip_prefix(b"Exif\0\0") {
                // Check every EXIF segment so a later identity segment cannot hide a rotation.
                if Orientation::from_exif_chunk(exif)
                    .is_some_and(|orientation| orientation != Orientation::NoTransforms)
                {
                    return Err(ImageIntakeError::NonIdentityOrientation);
                }
            }
        }
        offset = end;
        if marker == 0xda {
            in_scan = true;
            saw_scan = true;
        }
    }
    Err(ImageIntakeError::MalformedImage)
}

#[cfg(test)]
mod tests;
