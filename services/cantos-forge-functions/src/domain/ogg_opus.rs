//! Strict Ogg Opus framing and the facts it yields, from exact bytes.
//!
//! This is a producer-side technical measurement. It checks page framing, checksums, granule
//! positions and the Opus identification and comment headers, and sums the coded packets. It
//! never decodes audio, so it says nothing about intelligibility, loudness or playback
//! compatibility. The Cantos API inspects the received bytes independently and stays the
//! authority; the rules below follow that inspection so a file Forge refuses is not one the
//! server would admit. A shared pure validator package is a follow-up once a second consumer
//! exists.
//!
//! All arithmetic is integer: durations and bitrates round up, the conservative direction.

const SAMPLE_RATE_HZ: u64 = 48_000;
const PAGE_HEADER_BYTES: usize = 27;
const OPUS_HEAD_BYTES: usize = 19;
const MAX_PACKET_BYTES: usize = 65_536;
const MAX_PACKET_SAMPLES: u64 = 5_760;
/// Header page, comment page and at least one audio packet.
const MIN_PACKETS: u64 = 3;

const FLAG_CONTINUED: u8 = 1;
const FLAG_FIRST_PAGE: u8 = 2;
const FLAG_LAST_PAGE: u8 = 4;
const KNOWN_FLAGS: u8 = FLAG_CONTINUED | FLAG_FIRST_PAGE | FLAG_LAST_PAGE;

/// What the bytes prove about one Ogg Opus stream. Nothing here is a profile verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct OggOpusFacts {
    pub byte_length: u64,
    pub channels: u8,
    /// The rate the producer declared for the source; Opus always decodes at 48 kHz.
    pub input_sample_rate_hz: u32,
    pub pre_skip: u16,
    /// Length on the 48 kHz decoding timeline: terminal granule minus pre-skip.
    pub duration_samples: u64,
    /// Sum of the audio packets only: no page, header or tag bytes.
    pub coded_payload_bytes: u64,
}

impl OggOpusFacts {
    /// Duration in milliseconds, rounded up.
    pub fn duration_ms(&self) -> u64 {
        (self.duration_samples * 1_000).div_ceil(SAMPLE_RATE_HZ)
    }

    /// Average coded audio bitrate in bits per second, rounded up.
    pub fn average_bitrate_bps(&self) -> u64 {
        (self.coded_payload_bytes * 8 * SAMPLE_RATE_HZ).div_ceil(self.duration_samples)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OggOpusError {
    /// The bytes do not start with an Ogg page.
    NotOgg,
    /// A page header or body runs past the end of the bytes.
    Truncated,
    /// Capture pattern, stream structure version or reserved flag bits are wrong.
    BadPageHeader,
    /// Serial changed, sequence skipped, or the first/continued flags contradict the stream.
    PageOutOfOrder,
    ChecksumMismatch,
    GranuleDecreased,
    PacketTooLarge,
    /// The first packet is not an Opus identification header.
    NotOpus,
    /// The identification header is not version 1, 19 bytes, channel mapping family 0.
    OpusHeadInvalid,
    /// The second packet is not an `OpusTags` header.
    MissingTags,
    EmptyAudioPacket,
    /// A packet's table of contents does not describe a valid Opus frame layout.
    InvalidOpusPacket,
    /// A page's granule position contradicts the packets completed so far.
    GranuleMismatch,
    /// Another page follows the page marked as the end of the stream.
    PagesAfterEnd,
    /// The last page is not marked as the end, or leaves a packet unfinished.
    MissingEndOfStream,
    TooFewPackets,
    /// The terminal granule leaves no audio after pre-skip.
    NoAudio,
}

impl OggOpusError {
    /// A stable, machine-readable name for evidence records and CLI results.
    pub fn code(self) -> &'static str {
        match self {
            Self::NotOgg => "not_ogg",
            Self::Truncated => "truncated",
            Self::BadPageHeader => "bad_page_header",
            Self::PageOutOfOrder => "page_out_of_order",
            Self::ChecksumMismatch => "checksum_mismatch",
            Self::GranuleDecreased => "granule_decreased",
            Self::PacketTooLarge => "packet_too_large",
            Self::NotOpus => "not_opus",
            Self::OpusHeadInvalid => "opus_head_invalid",
            Self::MissingTags => "missing_tags",
            Self::EmptyAudioPacket => "empty_audio_packet",
            Self::InvalidOpusPacket => "invalid_opus_packet",
            Self::GranuleMismatch => "granule_mismatch",
            Self::PagesAfterEnd => "pages_after_end",
            Self::MissingEndOfStream => "missing_end_of_stream",
            Self::TooFewPackets => "too_few_packets",
            Self::NoAudio => "no_audio",
        }
    }
}

/// Inspects one Ogg Opus stream. Chained or multiplexed streams are refused.
pub fn inspect(bytes: &[u8]) -> Result<OggOpusFacts, OggOpusError> {
    if !bytes.starts_with(b"OggS") {
        return Err(OggOpusError::NotOgg);
    }
    let mut stream = Stream::default();
    let mut offset = 0;
    while offset < bytes.len() {
        offset = stream.accept_page(bytes, offset)?;
    }
    stream.finish(bytes.len())
}

#[derive(Default)]
struct Stream {
    sequence: u32,
    serial: Option<u32>,
    pending: Vec<u8>,
    packets: u64,
    ended: bool,
    last_granule: u64,
    head: Option<Head>,
    coded_samples: u64,
    last_packet_samples: u64,
    coded_payload_bytes: u64,
}

#[derive(Clone, Copy)]
struct Head {
    channels: u8,
    input_sample_rate_hz: u32,
    pre_skip: u16,
}

impl Stream {
    /// Validates the page at `offset` and returns the offset after it.
    fn accept_page(&mut self, bytes: &[u8], offset: usize) -> Result<usize, OggOpusError> {
        let header = bytes
            .get(offset..offset + PAGE_HEADER_BYTES)
            .ok_or(OggOpusError::Truncated)?;
        if self.ended {
            return Err(OggOpusError::PagesAfterEnd);
        }
        if !header.starts_with(b"OggS") || header[4] != 0 || header[5] & !KNOWN_FLAGS != 0 {
            return Err(OggOpusError::BadPageHeader);
        }
        let flags = header[5];
        let granule = le_u64(&header[6..14]);
        let serial = le_u32(&header[14..18]);
        let sequence = le_u32(&header[18..22]);
        let expected_checksum = le_u32(&header[22..26]);
        let lace_count = usize::from(header[26]);
        let first_page = self.sequence == 0;
        if sequence != self.sequence
            || self.serial.is_some_and(|known| known != serial)
            || (flags & FLAG_FIRST_PAGE != 0) != first_page
            || (flags & FLAG_CONTINUED != 0) == self.pending.is_empty()
        {
            return Err(OggOpusError::PageOutOfOrder);
        }
        let lace_start = offset + PAGE_HEADER_BYTES;
        let laces = bytes
            .get(lace_start..lace_start + lace_count)
            .ok_or(OggOpusError::Truncated)?;
        let body_start = lace_start + lace_count;
        let body_len: usize = laces.iter().map(|lace| usize::from(*lace)).sum();
        let end = body_start + body_len;
        let page = bytes.get(offset..end).ok_or(OggOpusError::Truncated)?;
        if checksum(page) != expected_checksum {
            return Err(OggOpusError::ChecksumMismatch);
        }
        self.serial = Some(serial);
        self.sequence = self
            .sequence
            .checked_add(1)
            .ok_or(OggOpusError::PageOutOfOrder)?;
        if granule != u64::MAX {
            if granule < self.last_granule {
                return Err(OggOpusError::GranuleDecreased);
            }
            self.last_granule = granule;
        }
        self.accept_packets(laces, &bytes[body_start..end])?;
        self.ended = flags & FLAG_LAST_PAGE != 0;
        self.check_granule(granule)?;
        Ok(end)
    }

    fn accept_packets(&mut self, laces: &[u8], body: &[u8]) -> Result<(), OggOpusError> {
        let mut cursor = 0;
        for lace in laces {
            let size = usize::from(*lace);
            self.pending.extend_from_slice(&body[cursor..cursor + size]);
            cursor += size;
            if self.pending.len() > MAX_PACKET_BYTES {
                return Err(OggOpusError::PacketTooLarge);
            }
            // A lace below 255 ends the packet; 255 means it continues in the next lace.
            if size < 255 {
                self.complete_packet()?;
            }
        }
        Ok(())
    }

    fn complete_packet(&mut self) -> Result<(), OggOpusError> {
        let packet = std::mem::take(&mut self.pending);
        match self.packets {
            0 => self.head = Some(parse_head(&packet)?),
            1 if packet.starts_with(b"OpusTags") => {}
            1 => return Err(OggOpusError::MissingTags),
            _ => {
                if packet.is_empty() {
                    return Err(OggOpusError::EmptyAudioPacket);
                }
                self.last_packet_samples = packet_samples(&packet)?;
                self.coded_samples = self
                    .coded_samples
                    .checked_add(self.last_packet_samples)
                    .ok_or(OggOpusError::GranuleMismatch)?;
                self.coded_payload_bytes += packet.len() as u64;
            }
        }
        self.packets += 1;
        Ok(())
    }

    fn check_granule(&self, granule: u64) -> Result<(), OggOpusError> {
        if self.coded_samples > 0 && granule != u64::MAX {
            let valid = if self.ended {
                granule <= self.coded_samples
                    && granule > self.coded_samples - self.last_packet_samples
            } else {
                granule == self.coded_samples
            };
            if !valid {
                return Err(OggOpusError::GranuleMismatch);
            }
        }
        if self.ended && (granule == u64::MAX || granule == 0) {
            return Err(OggOpusError::GranuleMismatch);
        }
        Ok(())
    }

    fn finish(self, byte_length: usize) -> Result<OggOpusFacts, OggOpusError> {
        if !self.ended || !self.pending.is_empty() {
            return Err(OggOpusError::MissingEndOfStream);
        }
        let Some(head) = self.head else {
            return Err(OggOpusError::NotOpus);
        };
        if self.packets < MIN_PACKETS {
            return Err(OggOpusError::TooFewPackets);
        }
        let pre_skip = u64::from(head.pre_skip);
        if self.last_granule <= pre_skip {
            return Err(OggOpusError::NoAudio);
        }
        Ok(OggOpusFacts {
            byte_length: byte_length as u64,
            channels: head.channels,
            input_sample_rate_hz: head.input_sample_rate_hz,
            pre_skip: head.pre_skip,
            duration_samples: self.last_granule - pre_skip,
            coded_payload_bytes: self.coded_payload_bytes,
        })
    }
}

fn parse_head(packet: &[u8]) -> Result<Head, OggOpusError> {
    if !packet.starts_with(b"OpusHead") {
        return Err(OggOpusError::NotOpus);
    }
    let valid =
        packet.len() == OPUS_HEAD_BYTES && packet[8] == 1 && packet[9] != 0 && packet[18] == 0;
    if !valid {
        return Err(OggOpusError::OpusHeadInvalid);
    }
    Ok(Head {
        channels: packet[9],
        pre_skip: u16::from_le_bytes([packet[10], packet[11]]),
        input_sample_rate_hz: le_u32(&packet[12..16]),
    })
}

/// Samples at 48 kHz that one Opus packet decodes to, from its table of contents (RFC 6716 §3.1).
fn packet_samples(packet: &[u8]) -> Result<u64, OggOpusError> {
    let toc = *packet.first().ok_or(OggOpusError::EmptyAudioPacket)?;
    let config = toc >> 3;
    let frame_samples: u64 = if config >= 16 {
        120 << (config & 3)
    } else if config >= 12 {
        480 << (config & 1)
    } else {
        [480, 960, 1_920, 2_880][usize::from(config & 3)]
    };
    let frames: u64 = match toc & 3 {
        0 => 1,
        1 | 2 => 2,
        _ => u64::from(*packet.get(1).ok_or(OggOpusError::InvalidOpusPacket)? & 0x3f),
    };
    let samples = frame_samples * frames;
    if frames == 0 || samples > MAX_PACKET_SAMPLES {
        return Err(OggOpusError::InvalidOpusPacket);
    }
    Ok(samples)
}

fn le_u32(bytes: &[u8]) -> u32 {
    u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]])
}

fn le_u64(bytes: &[u8]) -> u64 {
    u64::from_le_bytes([
        bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5], bytes[6], bytes[7],
    ])
}

const CRC_TABLE: [u32; 256] = crc_table();

const fn crc_table() -> [u32; 256] {
    let mut table = [0_u32; 256];
    let mut index = 0;
    while index < 256 {
        let mut value = (index as u32) << 24;
        let mut bit = 0;
        while bit < 8 {
            value = if value & 0x8000_0000 == 0 {
                value << 1
            } else {
                (value << 1) ^ 0x04c1_1db7
            };
            bit += 1;
        }
        table[index] = value;
        index += 1;
    }
    table
}

/// Ogg's CRC-32 over a page whose own checksum field (bytes 22..26) counts as zero.
fn checksum(page: &[u8]) -> u32 {
    page.iter().enumerate().fold(0_u32, |crc, (index, byte)| {
        let byte = if (22..26).contains(&index) { 0 } else { *byte };
        (crc << 8) ^ CRC_TABLE[usize::from(((crc >> 24) as u8) ^ byte)]
    })
}

/// Builds valid and deliberately broken Ogg Opus streams. The builder lives with the
/// integration tests and carries its own bitwise CRC, so it is an independent oracle for the
/// table-driven checksum above rather than a reuse of it.
#[cfg(test)]
#[path = "../../tests/support/ogg_fixture.rs"]
pub(crate) mod testing;

#[cfg(test)]
mod tests {
    use super::{testing::*, *};
    use proptest::prelude::*;

    proptest! {
        /// The table-driven checksum agrees with the fixture's independent bitwise one.
        #[test]
        fn the_checksum_matches_an_independent_bitwise_implementation(
            body in proptest::collection::vec(any::<u8>(), 26..300),
        ) {
            // `checksum` treats bytes 22..26 (the page's own CRC field) as zero.
            let mut zeroed = body.clone();
            zeroed[22..26].copy_from_slice(&[0; 4]);
            prop_assert_eq!(checksum(&body), bitwise_checksum(&zeroed));
        }
    }

    fn valid() -> Vec<u8> {
        Fixture::mono(vec![60, 61, 62, 63, 64, 65, 66, 67, 68, 69, 70, 71]).build()
    }

    #[test]
    fn a_valid_stream_yields_exact_facts() {
        let facts = inspect(&valid()).expect("valid stream");
        assert_eq!(facts.channels, 1);
        assert_eq!(facts.input_sample_rate_hz, 48_000);
        assert_eq!(facts.pre_skip, 312);
        // 12 packets x 960 samples, terminal granule 11_520, minus pre-skip 312.
        assert_eq!(facts.duration_samples, 11_520 - 312);
        assert_eq!(facts.coded_payload_bytes, (60..=71).sum::<u64>());
        assert_eq!(facts.byte_length, valid().len() as u64);
        assert_eq!(facts.duration_ms(), 234); // 11_208 / 48 = 233.5 rounded up
    }

    #[test]
    fn the_bitrate_rounds_up_from_exact_integer_arithmetic() {
        let facts = OggOpusFacts {
            byte_length: 0,
            channels: 1,
            input_sample_rate_hz: 48_000,
            pre_skip: 0,
            duration_samples: 48_000, // exactly one second
            coded_payload_bytes: 3_000,
        };
        assert_eq!(facts.average_bitrate_bps(), 24_000);
        let one_more = OggOpusFacts {
            coded_payload_bytes: 3_001,
            ..facts
        };
        assert_eq!(one_more.average_bitrate_bps(), 24_008);
        let tiny = OggOpusFacts {
            duration_samples: 47_999,
            ..facts
        };
        assert_eq!(tiny.average_bitrate_bps(), 24_001); // 24_000.5 rounds up
        assert_eq!(tiny.duration_ms(), 1_000);
    }

    #[test]
    fn end_of_stream_trimming_shortens_the_duration_exactly() {
        let mut stream = Fixture::mono(vec![50; 10]);
        stream.trim_samples = 117;
        let facts = inspect(&stream.build()).unwrap();
        assert_eq!(facts.duration_samples, 9_600 - 117 - 312);
    }

    #[test]
    fn stereo_is_reported_as_a_fact_not_rejected_by_framing() {
        let mut stream = Fixture::mono(vec![50; 4]);
        stream.channels = 2;
        assert_eq!(inspect(&stream.build()).unwrap().channels, 2);
    }

    #[test]
    fn non_ogg_input_is_not_ogg() {
        for bytes in [
            &b""[..],
            b"RIFF....WAVE",
            b"ID3\x04",
            &[0, 0, 0, 32, b'f', b't', b'y', b'p'],
        ] {
            assert_eq!(inspect(bytes), Err(OggOpusError::NotOgg));
        }
    }

    #[test]
    fn a_bare_capture_pattern_is_truncated() {
        assert_eq!(inspect(b"OggS"), Err(OggOpusError::Truncated));
    }

    #[test]
    fn every_truncation_of_a_valid_stream_is_refused() {
        let bytes = valid();
        for length in 1..bytes.len() {
            assert!(
                inspect(&bytes[..length]).is_err(),
                "truncated to {length} bytes was accepted"
            );
        }
    }

    #[test]
    fn every_single_byte_corruption_of_a_valid_stream_is_refused() {
        let bytes = valid();
        for index in 0..bytes.len() {
            for mask in [0x01_u8, 0x80, 0xff] {
                let mut corrupt = bytes.clone();
                corrupt[index] ^= mask;
                assert!(
                    inspect(&corrupt).is_err(),
                    "byte {index} ^ {mask:#04x} was accepted"
                );
            }
        }
    }

    #[test]
    fn a_checksum_error_is_reported_as_such() {
        let mut bytes = valid();
        let last = bytes.len() - 1;
        bytes[last] ^= 1;
        assert_eq!(inspect(&bytes), Err(OggOpusError::ChecksumMismatch));
    }

    #[test]
    fn another_codec_in_the_first_packet_is_not_opus() {
        let mut bytes = page(FLAG_FIRST_PAGE, 0, 1, 0, &[b"\x01vorbis".to_vec()]);
        bytes.extend(page(FLAG_LAST_PAGE, 960, 1, 1, &[packet_20ms(40)]));
        assert_eq!(inspect(&bytes), Err(OggOpusError::NotOpus));
    }

    #[test]
    fn malformed_identification_headers_are_refused() {
        type Damage = fn(&mut Vec<u8>);
        let cases: [(&str, Damage); 4] = [
            ("version", |head| head[8] = 2),
            ("zero channels", |head| head[9] = 0),
            ("mapping family", |head| head[18] = 1),
            ("length", |head| head.push(0)),
        ];
        for (name, damage) in cases {
            let mut head = head(1, 312);
            damage(&mut head);
            let mut bytes = page(FLAG_FIRST_PAGE, 0, 1, 0, &[head]);
            bytes.extend(page(0, 0, 1, 1, &[tags()]));
            bytes.extend(page(FLAG_LAST_PAGE, 960, 1, 2, &[packet_20ms(40)]));
            assert_eq!(
                inspect(&bytes),
                Err(OggOpusError::OpusHeadInvalid),
                "{name}"
            );
        }
    }

    #[test]
    fn a_second_packet_that_is_not_tags_is_refused() {
        let mut bytes = page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]);
        bytes.extend(page(0, 0, 1, 1, &[b"NotTags!".to_vec()]));
        bytes.extend(page(FLAG_LAST_PAGE, 960, 1, 2, &[packet_20ms(40)]));
        assert_eq!(inspect(&bytes), Err(OggOpusError::MissingTags));
    }

    #[test]
    fn framing_contradictions_are_page_out_of_order() {
        let build = |pages: Vec<Vec<u8>>| pages.concat();
        // Sequence skips from 1 to 3.
        let skipped = build(vec![
            page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]),
            page(0, 0, 1, 1, &[tags()]),
            page(FLAG_LAST_PAGE, 960, 1, 3, &[packet_20ms(40)]),
        ]);
        assert_eq!(inspect(&skipped), Err(OggOpusError::PageOutOfOrder));
        // The serial number changes mid-stream.
        let reserialed = build(vec![
            page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]),
            page(0, 0, 2, 1, &[tags()]),
            page(FLAG_LAST_PAGE, 960, 1, 2, &[packet_20ms(40)]),
        ]);
        assert_eq!(inspect(&reserialed), Err(OggOpusError::PageOutOfOrder));
        // The first page is missing its beginning-of-stream flag.
        let unflagged = build(vec![
            page(0, 0, 1, 0, &[head(1, 312)]),
            page(0, 0, 1, 1, &[tags()]),
            page(FLAG_LAST_PAGE, 960, 1, 2, &[packet_20ms(40)]),
        ]);
        assert_eq!(inspect(&unflagged), Err(OggOpusError::PageOutOfOrder));
        // A page claims to continue a packet that was never started.
        let continued = build(vec![
            page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]),
            page(FLAG_CONTINUED, 0, 1, 1, &[tags()]),
            page(FLAG_LAST_PAGE, 960, 1, 2, &[packet_20ms(40)]),
        ]);
        assert_eq!(inspect(&continued), Err(OggOpusError::PageOutOfOrder));
    }

    #[test]
    fn reserved_flag_bits_and_versions_are_a_bad_page_header() {
        let mut flagged = page(FLAG_FIRST_PAGE | 8, 0, 1, 0, &[head(1, 312)]);
        resign(&mut flagged); // only the flag is wrong, not the checksum
        assert_eq!(inspect(&flagged), Err(OggOpusError::BadPageHeader));
        let mut versioned = page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]);
        versioned[4] = 1;
        assert_eq!(inspect(&versioned), Err(OggOpusError::BadPageHeader));
    }

    #[test]
    fn a_stream_without_an_end_page_is_missing_its_end() {
        let bytes = [
            page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]),
            page(0, 0, 1, 1, &[tags()]),
            page(0, 960, 1, 2, &[packet_20ms(40)]),
        ]
        .concat();
        assert_eq!(inspect(&bytes), Err(OggOpusError::MissingEndOfStream));
    }

    #[test]
    fn a_page_after_the_end_is_refused() {
        let mut bytes = Fixture::mono(vec![50; 4]).build();
        bytes.extend(page(0, 7_680, 0x4341_4e54, 4, &[packet_20ms(40)]));
        assert_eq!(inspect(&bytes), Err(OggOpusError::PagesAfterEnd));
    }

    #[test]
    fn an_end_page_with_granule_zero_is_a_granule_mismatch() {
        let bytes = [
            page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]),
            page(FLAG_LAST_PAGE, 0, 1, 1, &[tags()]),
        ]
        .concat();
        assert_eq!(inspect(&bytes), Err(OggOpusError::GranuleMismatch));
    }

    #[test]
    fn headers_without_audio_are_too_few_packets() {
        let bytes = [
            page(FLAG_FIRST_PAGE, 0, 1, 0, &[head(1, 312)]),
            page(FLAG_LAST_PAGE, 960, 1, 1, &[tags()]),
        ]
        .concat();
        assert_eq!(inspect(&bytes), Err(OggOpusError::TooFewPackets));
    }

    #[test]
    fn granule_positions_must_equal_the_coded_samples() {
        let serial = 1;
        let wrong_middle = [
            page(FLAG_FIRST_PAGE, 0, serial, 0, &[head(1, 312)]),
            page(0, 0, serial, 1, &[tags()]),
            page(0, 900, serial, 2, &[packet_20ms(40)]),
            page(FLAG_LAST_PAGE, 1_920, serial, 3, &[packet_20ms(40)]),
        ]
        .concat();
        assert_eq!(inspect(&wrong_middle), Err(OggOpusError::GranuleMismatch));
        // The end granule may trim only inside the last packet: 960 leaves it entirely trimmed.
        let over_trim = [
            page(FLAG_FIRST_PAGE, 0, serial, 0, &[head(1, 312)]),
            page(0, 0, serial, 1, &[tags()]),
            page(
                FLAG_LAST_PAGE,
                960,
                serial,
                2,
                &[packet_20ms(40), packet_20ms(40)],
            ),
        ]
        .concat();
        assert_eq!(inspect(&over_trim), Err(OggOpusError::GranuleMismatch));
    }

    #[test]
    fn an_end_granule_that_leaves_no_audio_after_pre_skip_is_no_audio() {
        let mut stream = Fixture::mono(vec![50]);
        stream.pre_skip = 960;
        assert_eq!(inspect(&stream.build()), Err(OggOpusError::NoAudio));
    }

    #[test]
    fn invalid_opus_table_of_contents_are_refused() {
        let serial = 1;
        // Code 3 with a zero frame count.
        let zero_frames = [
            page(FLAG_FIRST_PAGE, 0, serial, 0, &[head(1, 312)]),
            page(0, 0, serial, 1, &[tags()]),
            page(FLAG_LAST_PAGE, 960, serial, 2, &[vec![0x0b, 0x00, 1, 2, 3]]),
        ]
        .concat();
        assert_eq!(inspect(&zero_frames), Err(OggOpusError::InvalidOpusPacket));
        // 120 ms x 48 frames exceeds the 120 ms packet limit.
        let too_long = [
            page(FLAG_FIRST_PAGE, 0, serial, 0, &[head(1, 312)]),
            page(0, 0, serial, 1, &[tags()]),
            page(FLAG_LAST_PAGE, 960, serial, 2, &[vec![0x1b, 0x30, 1, 2, 3]]),
        ]
        .concat();
        assert_eq!(inspect(&too_long), Err(OggOpusError::InvalidOpusPacket));
    }

    #[test]
    fn a_packet_spanning_pages_is_reassembled() {
        let serial = 1;
        let packet = packet_20ms(300); // laces 255 then 45
                                       // Page 2 ends mid-packet (a final lace of 255, no packet completed, granule -1);
                                       // page 3 continues it and ends the stream.
        let bytes = [
            page(FLAG_FIRST_PAGE, 0, serial, 0, &[head(1, 312)]),
            page(0, 0, serial, 1, &[tags()]),
            page_raw(0, u64::MAX, serial, 2, &[255], &packet[..255]),
            page_raw(
                FLAG_CONTINUED | FLAG_LAST_PAGE,
                960,
                serial,
                3,
                &[45],
                &packet[255..],
            ),
        ]
        .concat();
        let facts = inspect(&bytes).unwrap();
        assert_eq!(facts.coded_payload_bytes, 300);
        assert_eq!(facts.duration_samples, 960 - 312);
    }

    proptest! {
        #[test]
        fn duration_and_payload_follow_the_packets_exactly(
            sizes in proptest::collection::vec(1_usize..=250, 1..120),
            per_page in 1_usize..=60,
            pre_skip in 0_u16..=959,
            trim in 0_u64..=959,
        ) {
            let mut stream = Fixture::mono(sizes.clone());
            stream.per_page = per_page;
            stream.pre_skip = pre_skip;
            stream.trim_samples = trim;
            let bytes = stream.build();
            let granule = 960 * sizes.len() as u64 - trim;
            match inspect(&bytes) {
                Ok(facts) => {
                    prop_assert_eq!(facts.duration_samples, granule - u64::from(pre_skip));
                    prop_assert_eq!(facts.coded_payload_bytes, sizes.iter().sum::<usize>() as u64);
                    prop_assert_eq!(facts.byte_length, bytes.len() as u64);
                }
                // The only generated streams a conforming reader may refuse leave no audio.
                Err(error) => {
                    prop_assert_eq!(error, OggOpusError::NoAudio);
                    prop_assert!(granule <= u64::from(pre_skip));
                }
            }
        }

        #[test]
        fn arbitrary_bytes_never_panic(bytes in proptest::collection::vec(any::<u8>(), 0..400)) {
            let _ = inspect(&bytes);
        }

        #[test]
        fn a_stream_with_a_flipped_byte_is_never_accepted_with_other_facts(
            sizes in proptest::collection::vec(1_usize..=120, 2..30),
            index_seed in any::<usize>(),
            mask in 1_u8..=255,
        ) {
            let bytes = Fixture::mono(sizes).build();
            let mut corrupt = bytes.clone();
            let index = index_seed % bytes.len();
            corrupt[index] ^= mask;
            prop_assert!(inspect(&corrupt).is_err());
        }
    }
}
