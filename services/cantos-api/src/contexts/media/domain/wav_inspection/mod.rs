//! Standalone structural inspection of RIFF/WAVE integer PCM bytes.
//!
//! [`inspect_wav`] is a pure function of the input bytes: no file system, clock, process,
//! provider or audio conversion. It reports the chunk table, declared `fmt ` fields, SHA-256
//! checksums and every structural error it can reach. Frame count, duration and per-channel
//! sample peak are measured only when the structure is valid, so a damaged file never yields a
//! number that looks like a measurement of the whole file.
//!
//! This is groundwork for technical QC, not QC: it measures no loudness or true peak, applies no
//! audio profile or target, and approves neither listening quality nor publication.

mod chunks;
mod fmt_chunk;
mod sample_peak;

use std::fmt;

use serde::{Serialize, Serializer};
use sha2::{Digest, Sha256};

pub use chunks::ChunkFacts;
pub use fmt_chunk::{ExtensibleFacts, FmtFacts, Guid};
pub use sample_peak::{ChannelPeak, PeakLevel, SamplePeak};

/// Names the report layout and its rules; it changes whenever a field or error meaning changes.
pub const INSPECTOR_VERSION: &str = "cantos-wav-inspection-1";
/// Bounds the chunk table so a file made of tiny chunks cannot grow the report without limit.
pub const MAX_CHUNKS: usize = 1024;
/// Largest file a 32-bit RIFF size field can describe.
pub const MAX_RIFF_FILE_BYTES: u64 = u32::MAX as u64 + 8;
/// Judgements this inspection deliberately leaves to versioned profiles and named reviewers.
pub const NOT_ASSESSED: [&str; 5] = [
    "integrated_loudness",
    "true_peak",
    "clipping",
    "listening_quality",
    "publication_readiness",
];

/// A four-byte RIFF identifier, rendered as ASCII with `\xNN` escapes for anything else.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ChunkId(pub [u8; 4]);

impl ChunkId {
    pub const FMT: ChunkId = ChunkId(*b"fmt ");
    pub const DATA: ChunkId = ChunkId(*b"data");

    fn is_printable(self) -> bool {
        self.0.iter().all(|byte| (0x20..=0x7e).contains(byte))
    }
}

impl fmt::Display for ChunkId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        for &byte in &self.0 {
            if (0x20..=0x7e).contains(&byte) && byte != b'\\' {
                write!(formatter, "{}", char::from(byte))?;
            } else {
                write!(formatter, "\\x{byte:02x}")?;
            }
        }
        Ok(())
    }
}

impl Serialize for ChunkId {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// Every defect is reported; offsets are byte positions in the inspected file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum StructuralError {
    TruncatedRiffHeader {
        file_bytes: u64,
    },
    NotRiff {
        found: ChunkId,
    },
    UnsupportedContainer {
        found: ChunkId,
    },
    NotWave {
        found: ChunkId,
    },
    RiffSizeTooSmall {
        declared_size: u32,
    },
    RiffSizeExceedsFile {
        riff_end: u64,
        file_bytes: u64,
    },
    TrailingBytes {
        riff_end: u64,
        file_bytes: u64,
    },
    TruncatedChunkHeader {
        offset: u64,
        available: u64,
    },
    InvalidChunkId {
        offset: u64,
        id: ChunkId,
    },
    ChunkOverrun {
        id: ChunkId,
        offset: u64,
        declared_size: u32,
        available: u64,
    },
    MissingPadByte {
        id: ChunkId,
        offset: u64,
    },
    ChunkLimitExceeded {
        limit: usize,
    },
    DuplicateChunk {
        id: ChunkId,
        offset: u64,
    },
    MissingChunk {
        id: ChunkId,
    },
    DataBeforeFmt {
        data_offset: u64,
        fmt_offset: u64,
    },
    FmtTooShort {
        declared_size: u32,
        minimum: u32,
    },
    InvalidExtensionSize {
        cb_size: u16,
        minimum: u16,
        available: u32,
    },
    UnsupportedFormatTag {
        format_tag: u16,
    },
    UnsupportedSubformat {
        subformat: Guid,
    },
    ZeroChannels,
    ZeroSampleRate,
    UnsupportedBitsPerSample {
        bits_per_sample: u16,
    },
    InvalidValidBits {
        valid_bits_per_sample: u16,
        bits_per_sample: u16,
    },
    BlockAlignMismatch {
        declared: u16,
        expected: u32,
    },
    ByteRateMismatch {
        declared: u32,
        expected: u64,
    },
    PartialFrame {
        data_bytes: u32,
        block_align: u16,
    },
}

/// A deviation from RIFF that leaves every byte accounted for; it does not invalidate the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "code", rename_all = "snake_case")]
pub enum StructuralWarning {
    UnpaddedFinalChunk { id: ChunkId, offset: u64 },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Structure {
    Valid,
    Invalid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct FileFacts {
    pub bytes: u64,
    pub sha256: String,
}

/// The complete `data` payload; `offset` is where its first sample byte sits in the file.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct DataFacts {
    pub offset: u64,
    pub bytes: u32,
    pub sha256: String,
}

/// Present only for a structurally valid file. `sample_peak` is `None` when it has no frames.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Measurements {
    pub frames: u64,
    pub duration_seconds: f64,
    pub sample_peak: Option<SamplePeak>,
}

/// Built only by [`inspect_wav`], so measurements cannot coexist with structural errors.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct WavReport {
    inspector: &'static str,
    file: FileFacts,
    structure: Structure,
    errors: Vec<StructuralError>,
    warnings: Vec<StructuralWarning>,
    chunks: Vec<ChunkFacts>,
    fmt: Option<FmtFacts>,
    data: Option<DataFacts>,
    measurements: Option<Measurements>,
    not_assessed: [&'static str; 5],
}

impl WavReport {
    pub fn structure(&self) -> Structure {
        self.structure
    }

    pub fn file(&self) -> &FileFacts {
        &self.file
    }

    pub fn errors(&self) -> &[StructuralError] {
        &self.errors
    }

    pub fn warnings(&self) -> &[StructuralWarning] {
        &self.warnings
    }

    pub fn chunks(&self) -> &[ChunkFacts] {
        &self.chunks
    }

    pub fn fmt(&self) -> Option<&FmtFacts> {
        self.fmt.as_ref()
    }

    pub fn data(&self) -> Option<&DataFacts> {
        self.data.as_ref()
    }

    pub fn measurements(&self) -> Option<&Measurements> {
        self.measurements.as_ref()
    }
}

/// Inspect untrusted bytes. Never panics, and allocates in proportion to the chunk table and
/// channel count rather than to any declared size.
pub fn inspect_wav(bytes: &[u8]) -> WavReport {
    let mut errors = Vec::new();
    let layout = chunks::walk(bytes, &mut errors);
    let fmt = layout
        .fmt
        .and_then(|chunk| chunk.payload(bytes))
        .and_then(|payload| fmt_chunk::read_fmt(payload, &mut errors));
    let pcm = fmt
        .as_ref()
        .and_then(|fmt| fmt_chunk::pcm_layout(fmt, &mut errors));
    let data = layout
        .data
        .and_then(|chunk| Some((chunk, chunk.payload(bytes)?)));
    if let (Some(pcm), Some((chunk, _))) = (pcm, data) {
        if chunk.size % u32::from(pcm.block_align()) != 0 {
            errors.push(StructuralError::PartialFrame {
                data_bytes: chunk.size,
                block_align: pcm.block_align(),
            });
        }
    }
    let measurements = match (errors.is_empty(), pcm, data) {
        (true, Some(pcm), Some((_, payload))) => Some(measure(payload, pcm)),
        _ => None,
    };
    WavReport {
        inspector: INSPECTOR_VERSION,
        file: FileFacts {
            bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        },
        structure: if errors.is_empty() {
            Structure::Valid
        } else {
            Structure::Invalid
        },
        errors,
        warnings: layout.warnings,
        chunks: layout.chunks,
        fmt,
        data: data.map(|(chunk, payload)| DataFacts {
            offset: chunk.offset + chunks::CHUNK_HEADER_BYTES,
            bytes: chunk.size,
            sha256: sha256_hex(payload),
        }),
        measurements,
        not_assessed: NOT_ASSESSED,
    }
}

fn measure(payload: &[u8], pcm: fmt_chunk::PcmLayout) -> Measurements {
    let frames = (payload.len() / usize::from(pcm.block_align())) as u64;
    Measurements {
        frames,
        // A correctly rounded quotient for display; `frames` and the rate remain the exact facts.
        duration_seconds: frames as f64 / f64::from(pcm.sample_rate_hz()),
        sample_peak: sample_peak::measure(payload, pcm),
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn le_array<const N: usize>(bytes: &[u8], offset: usize) -> Option<[u8; N]> {
    bytes.get(offset..offset.checked_add(N)?)?.try_into().ok()
}

fn le_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    le_array(bytes, offset).map(u16::from_le_bytes)
}

fn le_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    le_array(bytes, offset).map(u32::from_le_bytes)
}
