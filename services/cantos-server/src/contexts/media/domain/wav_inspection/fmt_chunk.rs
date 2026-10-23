//! Declared `fmt ` fields and the integer PCM frame layout they must describe consistently.
use std::fmt;

use serde::{Serialize, Serializer};

use super::{le_array, le_u16, le_u32, StructuralError};

const WAVE_FORMAT_PCM: u16 = 0x0001;
const WAVE_FORMAT_EXTENSIBLE: u16 = 0xfffe;
const BASE_FMT_BYTES: u32 = 16;
const EXTENSIBLE_FMT_BYTES: u32 = 40;
const EXTENSION_OFFSET: u32 = 18;
const MIN_EXTENSION_BYTES: u16 = 22;
/// KSDATAFORMAT_SUBTYPE_PCM (00000001-0000-0010-8000-00aa00389b71) in stored byte order.
const PCM_SUBFORMAT: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];

/// A GUID as stored in a WAVE_FORMAT_EXTENSIBLE header, rendered in registry form.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Guid(pub [u8; 16]);

impl fmt::Display for Guid {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let [a0, a1, a2, a3, b0, b1, c0, c1, rest @ ..] = self.0;
        write!(
            formatter,
            "{:08x}-{:04x}-{:04x}-",
            u32::from_le_bytes([a0, a1, a2, a3]),
            u16::from_le_bytes([b0, b1]),
            u16::from_le_bytes([c0, c1]),
        )?;
        for (index, byte) in rest.iter().enumerate() {
            if index == 2 {
                formatter.write_str("-")?;
            }
            write!(formatter, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl Serialize for Guid {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Fields exactly as declared, reported even when they are inconsistent.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FmtFacts {
    pub format_tag: u16,
    pub channels: u16,
    pub sample_rate_hz: u32,
    pub byte_rate: u32,
    pub block_align: u16,
    pub bits_per_sample: u16,
    pub extensible: Option<ExtensibleFacts>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExtensibleFacts {
    pub cb_size: u16,
    pub valid_bits_per_sample: u16,
    pub channel_mask: u32,
    pub subformat: Guid,
}

/// Stored sample container. 8-bit PCM is unsigned around 128; wider PCM is signed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SampleWidth {
    U8,
    I16,
    I24,
    I32,
}

impl SampleWidth {
    fn from_bits(bits_per_sample: u16) -> Option<Self> {
        match bits_per_sample {
            8 => Some(Self::U8),
            16 => Some(Self::I16),
            24 => Some(Self::I24),
            32 => Some(Self::I32),
            _ => None,
        }
    }

    pub(super) fn bytes(self) -> u16 {
        match self {
            Self::U8 => 1,
            Self::I16 => 2,
            Self::I24 => 3,
            Self::I32 => 4,
        }
    }

    /// The magnitude of the most negative code, the 0 dBFS reference of the container.
    pub(super) fn full_scale(self) -> u32 {
        1 << (8 * u32::from(self.bytes()) - 1)
    }
}

/// Proof that the declared format is consistent integer PCM; only this reaches sample decoding.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct PcmLayout {
    channels: u16,
    sample_rate_hz: u32,
    width: SampleWidth,
    block_align: u16,
}

impl PcmLayout {
    pub(super) fn channels(self) -> u16 {
        self.channels
    }

    pub(super) fn sample_rate_hz(self) -> u32 {
        self.sample_rate_hz
    }

    pub(super) fn width(self) -> SampleWidth {
        self.width
    }

    pub(super) fn block_align(self) -> u16 {
        self.block_align
    }
}

pub(super) fn read_fmt(payload: &[u8], errors: &mut Vec<StructuralError>) -> Option<FmtFacts> {
    let declared_size = payload.len() as u32;
    let fields = (
        le_u16(payload, 0),
        le_u16(payload, 2),
        le_u32(payload, 4),
        le_u32(payload, 8),
        le_u16(payload, 12),
        le_u16(payload, 14),
    );
    let (
        Some(format_tag),
        Some(channels),
        Some(sample_rate_hz),
        Some(byte_rate),
        Some(block_align),
        Some(bits_per_sample),
    ) = fields
    else {
        errors.push(StructuralError::FmtTooShort {
            declared_size,
            minimum: BASE_FMT_BYTES,
        });
        return None;
    };
    let extensible = if format_tag == WAVE_FORMAT_EXTENSIBLE {
        read_extension(payload, errors)
    } else {
        None
    };
    Some(FmtFacts {
        format_tag,
        channels,
        sample_rate_hz,
        byte_rate,
        block_align,
        bits_per_sample,
        extensible,
    })
}

fn read_extension(payload: &[u8], errors: &mut Vec<StructuralError>) -> Option<ExtensibleFacts> {
    let declared_size = payload.len() as u32;
    let fields = (
        le_u16(payload, 16),
        le_u16(payload, 18),
        le_u32(payload, 20),
        le_array(payload, 24),
    );
    let (Some(cb_size), Some(valid_bits_per_sample), Some(channel_mask), Some(subformat)) = fields
    else {
        errors.push(StructuralError::FmtTooShort {
            declared_size,
            minimum: EXTENSIBLE_FMT_BYTES,
        });
        return None;
    };
    let available = declared_size - EXTENSION_OFFSET;
    if cb_size < MIN_EXTENSION_BYTES || u32::from(cb_size) > available {
        errors.push(StructuralError::InvalidExtensionSize {
            cb_size,
            minimum: MIN_EXTENSION_BYTES,
            available,
        });
    }
    Some(ExtensibleFacts {
        cb_size,
        valid_bits_per_sample,
        channel_mask,
        subformat: Guid(subformat),
    })
}

/// Checks every declared field against integer PCM and the others, reporting all mismatches.
pub(super) fn pcm_layout(fmt: &FmtFacts, errors: &mut Vec<StructuralError>) -> Option<PcmLayout> {
    let reported = errors.len();
    let encoding_known = match (fmt.format_tag, &fmt.extensible) {
        (WAVE_FORMAT_PCM, _) => true,
        (WAVE_FORMAT_EXTENSIBLE, Some(extension)) => {
            check_extension(fmt, extension, errors);
            true
        }
        // The missing extension was already reported as `FmtTooShort`.
        (WAVE_FORMAT_EXTENSIBLE, None) => false,
        (format_tag, _) => {
            errors.push(StructuralError::UnsupportedFormatTag { format_tag });
            false
        }
    };
    if fmt.channels == 0 {
        errors.push(StructuralError::ZeroChannels);
    }
    if fmt.sample_rate_hz == 0 {
        errors.push(StructuralError::ZeroSampleRate);
    }
    let width = SampleWidth::from_bits(fmt.bits_per_sample);
    if width.is_none() {
        errors.push(StructuralError::UnsupportedBitsPerSample {
            bits_per_sample: fmt.bits_per_sample,
        });
    }
    if let Some(width) = width.filter(|_| fmt.channels > 0) {
        check_frame_rates(fmt, width, errors);
    }
    let width = width.filter(|_| encoding_known && errors.len() == reported)?;
    Some(PcmLayout {
        channels: fmt.channels,
        sample_rate_hz: fmt.sample_rate_hz,
        width,
        block_align: fmt.block_align,
    })
}

fn check_extension(fmt: &FmtFacts, extension: &ExtensibleFacts, errors: &mut Vec<StructuralError>) {
    if extension.subformat.0 != PCM_SUBFORMAT {
        errors.push(StructuralError::UnsupportedSubformat {
            subformat: extension.subformat,
        });
    }
    let valid = extension.valid_bits_per_sample;
    if valid == 0 || valid > fmt.bits_per_sample {
        errors.push(StructuralError::InvalidValidBits {
            valid_bits_per_sample: valid,
            bits_per_sample: fmt.bits_per_sample,
        });
    }
}

fn check_frame_rates(fmt: &FmtFacts, width: SampleWidth, errors: &mut Vec<StructuralError>) {
    let expected_align = u32::from(fmt.channels) * u32::from(width.bytes());
    if u32::from(fmt.block_align) != expected_align {
        errors.push(StructuralError::BlockAlignMismatch {
            declared: fmt.block_align,
            expected: expected_align,
        });
    }
    let expected_rate = u64::from(fmt.sample_rate_hz) * u64::from(expected_align);
    if u64::from(fmt.byte_rate) != expected_rate {
        errors.push(StructuralError::ByteRateMismatch {
            declared: fmt.byte_rate,
            expected: expected_rate,
        });
    }
}
