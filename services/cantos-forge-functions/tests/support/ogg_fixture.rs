//! A tiny Ogg Opus writer for tests: valid streams and deliberately broken ones.
//!
//! Shared by the library's unit tests and the integration tests through `#[path]`. It computes
//! Ogg's CRC-32 bit by bit, independently of the table-driven code under test. Packets are
//! placeholders with a valid Opus table of contents: nothing here decodes, so a stream built
//! here proves framing only, never that audio is intelligible.

#![allow(dead_code)]

pub const CONTINUED: u8 = 1;
pub const BEGIN: u8 = 2;
pub const END: u8 = 4;

/// Ogg's CRC-32 (polynomial 0x04c11db7, no reflection, zero start) over `bytes` as given.
pub fn bitwise_checksum(bytes: &[u8]) -> u32 {
    let mut crc = 0_u32;
    for byte in bytes {
        crc ^= u32::from(*byte) << 24;
        for _ in 0..8 {
            crc = if crc & 0x8000_0000 != 0 {
                (crc << 1) ^ 0x04c1_1db7
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// Recomputes and stores a page's checksum after the page was edited.
pub fn resign(page: &mut [u8]) {
    page[22..26].copy_from_slice(&[0; 4]);
    let crc = bitwise_checksum(page);
    page[22..26].copy_from_slice(&crc.to_le_bytes());
}

/// An Opus packet of 20 ms (960 samples at 48 kHz) padded to `size` bytes.
pub fn packet_20ms(size: usize) -> Vec<u8> {
    // TOC 0x08: config 1 (SILK narrowband, 20 ms), one frame (code 0).
    vec![0x08; size.max(1)]
}

pub fn head(channels: u8, pre_skip: u16) -> Vec<u8> {
    let mut head = b"OpusHead".to_vec();
    head.extend([1, channels]);
    head.extend(pre_skip.to_le_bytes());
    head.extend(48_000_u32.to_le_bytes());
    head.extend([0, 0, 0]); // output gain, mapping family 0
    head
}

pub fn tags() -> Vec<u8> {
    let mut tags = b"OpusTags".to_vec();
    tags.extend(4_u32.to_le_bytes());
    tags.extend(b"test");
    tags.extend(0_u32.to_le_bytes());
    tags
}

pub fn page(flags: u8, granule: u64, serial: u32, sequence: u32, packets: &[Vec<u8>]) -> Vec<u8> {
    let mut laces = Vec::new();
    for packet in packets {
        laces.extend(vec![255; packet.len() / 255]);
        laces.push((packet.len() % 255) as u8);
    }
    page_raw(flags, granule, serial, sequence, &laces, &packets.concat())
}

/// A page with explicit lacing, for packets that span pages.
pub fn page_raw(
    flags: u8,
    granule: u64,
    serial: u32,
    sequence: u32,
    laces: &[u8],
    body: &[u8],
) -> Vec<u8> {
    let mut raw = b"OggS".to_vec();
    raw.extend([0, flags]);
    raw.extend(granule.to_le_bytes());
    raw.extend(serial.to_le_bytes());
    raw.extend(sequence.to_le_bytes());
    raw.extend([0; 4]);
    raw.push(laces.len() as u8);
    raw.extend(laces);
    raw.extend(body);
    resign(&mut raw);
    raw
}

/// A valid stream: 20 ms packets of the given sizes, `per_page` per page, the terminal granule
/// being `pre_skip` plus the coded samples minus `trim_samples`.
pub struct Fixture {
    pub channels: u8,
    pub pre_skip: u16,
    pub packet_sizes: Vec<usize>,
    pub per_page: usize,
    /// Samples trimmed from the last packet (end-of-stream padding), below 960.
    pub trim_samples: u64,
}

impl Fixture {
    pub fn mono(packet_sizes: Vec<usize>) -> Self {
        Self {
            channels: 1,
            pre_skip: 312,
            packet_sizes,
            per_page: 5,
            trim_samples: 0,
        }
    }

    pub fn build(&self) -> Vec<u8> {
        let serial = 0x4341_4e54;
        let mut out = page(BEGIN, 0, serial, 0, &[head(self.channels, self.pre_skip)]);
        out.extend(page(0, 0, serial, 1, &[tags()]));
        let chunks: Vec<&[usize]> = self.packet_sizes.chunks(self.per_page).collect();
        let mut total = 0_u64;
        for (index, chunk) in chunks.iter().enumerate() {
            total += 960 * chunk.len() as u64;
            let last = index + 1 == chunks.len();
            let packets: Vec<Vec<u8>> = chunk.iter().map(|size| packet_20ms(*size)).collect();
            let (flags, granule) = if last {
                (END, total - self.trim_samples)
            } else {
                (0, total)
            };
            out.extend(page(flags, granule, serial, 2 + index as u32, &packets));
        }
        out
    }
}
