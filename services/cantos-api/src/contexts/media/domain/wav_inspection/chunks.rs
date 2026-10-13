//! RIFF header and chunk walk. Declared sizes are compared with the bytes present, never trusted.
use serde::Serialize;

use super::{le_array, le_u32, ChunkId, StructuralError, StructuralWarning, MAX_CHUNKS};

pub(super) const CHUNK_HEADER_BYTES: u64 = 8;
const RIFF_HEADER_BYTES: u64 = 12;
const RIFF: ChunkId = ChunkId(*b"RIFF");
const WAVE: ChunkId = ChunkId(*b"WAVE");
/// Recognized relatives (big-endian RIFX, 64-bit RF64/BW64) that this inspection does not read.
const OTHER_CONTAINERS: [ChunkId; 3] = [ChunkId(*b"RIFX"), ChunkId(*b"RF64"), ChunkId(*b"BW64")];

/// Raw chunk facts. The walker emits complete chunks, but public fields alone are not a
/// bounds witness; payload access re-admits their range against the actual buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct ChunkFacts {
    pub id: ChunkId,
    pub offset: u64,
    pub size: u32,
}

impl ChunkFacts {
    pub(super) fn payload(self, bytes: &[u8]) -> Option<&[u8]> {
        bytes.get(payload_range(self.offset, self.size, bytes.len())?)
    }
}

/// The actual payload-access kernel: both RIFF arithmetic and platform-sized indices are
/// checked before slicing. No buffer allocation or RIFF-walker precondition is required.
fn payload_range(offset: u64, size: u32, length: usize) -> Option<std::ops::Range<usize>> {
    let start = offset.checked_add(CHUNK_HEADER_BYTES)?;
    let end = start.checked_add(u64::from(size))?;
    let range = usize::try_from(start).ok()?..usize::try_from(end).ok()?;
    (range.end <= length).then_some(range)
}

#[derive(Debug, Default)]
pub(super) struct Layout {
    pub chunks: Vec<ChunkFacts>,
    pub fmt: Option<ChunkFacts>,
    pub data: Option<ChunkFacts>,
    pub warnings: Vec<StructuralWarning>,
}

/// Where the declared RIFF chunk ends, and how much of it the file actually holds.
struct RiffBody {
    declared_end: u64,
    readable_end: u64,
}

impl Layout {
    /// The first `fmt ` and `data` chunks are authoritative; later copies are defects.
    fn record(&mut self, chunk: ChunkFacts, errors: &mut Vec<StructuralError>) {
        self.chunks.push(chunk);
        let slot = match chunk.id {
            ChunkId::FMT => &mut self.fmt,
            ChunkId::DATA => &mut self.data,
            _ => return,
        };
        match slot {
            Some(_) => errors.push(StructuralError::DuplicateChunk {
                id: chunk.id,
                offset: chunk.offset,
            }),
            None => *slot = Some(chunk),
        }
    }

    /// Absence is claimed only after the walk has covered the whole RIFF body.
    fn require(&self, walk_complete: bool, errors: &mut Vec<StructuralError>) {
        match (self.fmt, self.data) {
            (Some(fmt), Some(data)) if data.offset < fmt.offset => {
                errors.push(StructuralError::DataBeforeFmt {
                    data_offset: data.offset,
                    fmt_offset: fmt.offset,
                });
            }
            (fmt, data) if walk_complete => {
                for (found, id) in [(fmt, ChunkId::FMT), (data, ChunkId::DATA)] {
                    if found.is_none() {
                        errors.push(StructuralError::MissingChunk { id });
                    }
                }
            }
            _ => {}
        }
    }
}

pub(super) fn walk(bytes: &[u8], errors: &mut Vec<StructuralError>) -> Layout {
    let mut layout = Layout::default();
    let Some(body) = riff_body(bytes, errors) else {
        return layout;
    };
    let body_end = body.readable_end;
    let mut offset = RIFF_HEADER_BYTES;
    let walk_complete = loop {
        if offset >= body_end {
            break true;
        }
        if layout.chunks.len() == MAX_CHUNKS {
            errors.push(StructuralError::ChunkLimitExceeded { limit: MAX_CHUNKS });
            break false;
        }
        let available = body_end - offset;
        let header = usize::try_from(offset)
            .ok()
            .filter(|_| available >= CHUNK_HEADER_BYTES)
            .and_then(|at| Some((ChunkId(le_array(bytes, at)?), le_u32(bytes, at + 4)?)));
        let Some((id, size)) = header else {
            errors.push(StructuralError::TruncatedChunkHeader { offset, available });
            break false;
        };
        if !id.is_printable() {
            errors.push(StructuralError::InvalidChunkId { offset, id });
        }
        let payload_available = available - CHUNK_HEADER_BYTES;
        if u64::from(size) > payload_available {
            errors.push(StructuralError::ChunkOverrun {
                id,
                offset,
                declared_size: size,
                available: payload_available,
            });
            break false;
        }
        layout.record(ChunkFacts { id, offset, size }, errors);
        // RIFF pads every odd-sized payload with one byte that the size field does not count.
        let padded = u64::from(size) + u64::from(size % 2);
        if padded > payload_available {
            // Some writers (including Python's `wave`) omit the final pad and size the RIFF chunk
            // without it. Every byte is still accounted for, so that alone is only a warning.
            if body_end == body.declared_end {
                layout
                    .warnings
                    .push(StructuralWarning::UnpaddedFinalChunk { id, offset });
                break true;
            }
            errors.push(StructuralError::MissingPadByte { id, offset });
            break false;
        }
        offset += CHUNK_HEADER_BYTES + padded;
    };
    layout.require(walk_complete, errors);
    layout
}

/// Returns the RIFF body extent, or `None` when no WAVE body can be walked.
fn riff_body(bytes: &[u8], errors: &mut Vec<StructuralError>) -> Option<RiffBody> {
    let file_bytes = bytes.len() as u64;
    let header = (le_array(bytes, 0), le_u32(bytes, 4), le_array(bytes, 8));
    let (Some(tag), Some(size), Some(form)) = header else {
        errors.push(StructuralError::TruncatedRiffHeader { file_bytes });
        return None;
    };
    let (tag, form) = (ChunkId(tag), ChunkId(form));
    if tag != RIFF {
        errors.push(if OTHER_CONTAINERS.contains(&tag) {
            StructuralError::UnsupportedContainer { found: tag }
        } else {
            StructuralError::NotRiff { found: tag }
        });
        return None;
    }
    if form != WAVE {
        errors.push(StructuralError::NotWave { found: form });
        return None;
    }
    // The size counts the form type, so anything below four cannot describe a WAVE body.
    if size < 4 {
        errors.push(StructuralError::RiffSizeTooSmall {
            declared_size: size,
        });
        return None;
    }
    let riff_end = CHUNK_HEADER_BYTES + u64::from(size);
    if riff_end > file_bytes {
        errors.push(StructuralError::RiffSizeExceedsFile {
            riff_end,
            file_bytes,
        });
    } else if riff_end < file_bytes {
        errors.push(StructuralError::TrailingBytes {
            riff_end,
            file_bytes,
        });
    }
    Some(RiffBody {
        declared_end: riff_end,
        readable_end: riff_end.min(file_bytes),
    })
}

#[cfg(test)]
mod range_tests {
    use super::*;
    use proptest::prelude::*;

    #[test]
    fn admitted_payload_ranges_pin_empty_exact_end_and_overrun_edges() {
        assert_eq!(payload_range(0, 0, 8), Some(8..8));
        assert_eq!(payload_range(0, 1, 9), Some(8..9));
        assert_eq!(payload_range(0, 1, 8), None);
        assert_eq!(payload_range(u64::MAX - 8, 1, usize::MAX), None);
        let end = u128::from(u32::MAX) + 8;
        let expected = if end <= usize::MAX as u128 {
            Some(8..end as usize)
        } else {
            None
        };
        assert_eq!(payload_range(0, u32::MAX, usize::MAX), expected);
    }

    proptest! {
        #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_013), ..ProptestConfig::default() })]
        #[test]
        fn payload_range_matches_an_independent_wider_integer_oracle(
            offset in any::<u64>(), size in any::<u32>(), length in any::<usize>(),
        ) {
            let start = u128::from(offset) + 8;
            let end = start + u128::from(size);
            let expected = if end <= u128::from(u64::MAX) && end <= length as u128 {
                Some(start as usize..end as usize)
            } else { None };
            prop_assert_eq!(payload_range(offset, size, length), expected);
        }
    }

    #[test]
    fn raw_chunk_offsets_are_checked_before_payload_access() {
        for offset in [u64::MAX, u64::MAX - 7] {
            let raw = ChunkFacts {
                id: ChunkId::DATA,
                offset,
                size: 0,
            };
            assert_eq!(raw.payload(&[]), None);
        }
    }
}

#[cfg(kani)]
mod kani_proofs {
    use super::payload_range;

    /// Proposition: a range exists iff widened header/payload ends fit u64 and the actual
    /// platform-sized buffer length, and its endpoints equal those widened byte positions.
    /// Domain: full u64 offset, u32 payload size and target usize length. Assumptions/stubs:
    /// none. Unwind: 2; no business loop. Calls the real kernel used by ChunkFacts::payload.
    /// This establishes range arithmetic, not RIFF structure, PCM correctness or audio QC.
    #[kani::proof]
    #[kani::unwind(2)]
    fn wav_payload_range_is_checked() {
        let offset: u64 = kani::any();
        let size: u32 = kani::any();
        let length: usize = kani::any();
        let start = u128::from(offset) + 8;
        let end = start + u128::from(size);
        let admissible = end <= u128::from(u64::MAX) && end <= length as u128;
        let range = payload_range(offset, size, length);
        assert_eq!(range.is_some(), admissible);
        if let Some(range) = range {
            assert_eq!(range.start as u128, start);
            assert_eq!(range.end as u128, end);
            assert!(range.start <= range.end && range.end <= length);
        }
        kani::cover!(admissible && size > 0, "nonempty accepted range");
        kani::cover!(admissible && size == 0, "empty accepted range");
        kani::cover!(
            admissible && end == length as u128,
            "exact buffer end accepted"
        );
        kani::cover!(start > u128::from(u64::MAX), "header overflow rejected");
        kani::cover!(
            start <= u128::from(u64::MAX) && end > u128::from(u64::MAX),
            "payload overflow rejected"
        );
        kani::cover!(
            end <= u128::from(u64::MAX) && end > length as u128,
            "buffer overrun rejected"
        );
        kani::cover!(
            admissible && size == u32::MAX,
            "maximum payload admitted when it fits"
        );
    }
}
