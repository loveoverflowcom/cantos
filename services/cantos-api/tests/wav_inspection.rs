//! WAV inspection fixtures are built in code from explicit parts: no recording, provider audio or
//! committed media. Good fixtures have analytic metadata and peaks; each defect fixture names the
//! exact structural errors it must produce.
use std::{
    fs,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use cantos_api::contexts::media::domain::wav_inspection::{
    inspect_wav, ChunkId, Guid, StructuralError, Structure, WavReport, INSPECTOR_VERSION,
    MAX_CHUNKS, NOT_ASSESSED,
};
use serde_json::{json, Value};

const PCM_GUID: [u8; 16] = [
    0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
const FLOAT_GUID: [u8; 16] = [
    0x03, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38, 0x9b, 0x71,
];
/// Interleaved stereo frames shared with the independent Python `wave` oracle below.
const REFERENCE_SAMPLES: [i16; 16] = [
    0, 0, 1000, -1000, 32767, -32768, -12000, 12000, 5, -5, 0, 0, -1, 1, 300, -300,
];
// Independent oracle (Python 3.14 stdlib `wave`, then `sha256sum`):
//   wave.open(path, "wb"): nchannels=2, sampwidth=2, framerate=8000,
//   writeframes(struct.pack("<16h", *REFERENCE_SAMPLES))
const REFERENCE_FILE_SHA256: &str =
    "5fac9253f1767af6af5a816a452cb563f6325de4d074f238dc7b3c74ad5dcfa0";
const REFERENCE_DATA_SHA256: &str =
    "a39a3c7fa2356d37608db514d078ff2d2afddb4d57806b823a85114b5ff8c6a4";
/// Python `20 * math.log10(32767 / 32768)` and `20 * math.log10(0.5)`.
const DBFS_32767: f64 = -0.000_265_076_360_379_619_15;
const DBFS_HALF: f64 = -6.020_599_913_279_624;

#[derive(Clone, Copy)]
struct Fmt {
    tag: u16,
    channels: u16,
    rate: u32,
    byte_rate: u32,
    block_align: u16,
    bits: u16,
}

impl Fmt {
    fn pcm(channels: u16, rate: u32, bits: u16) -> Self {
        let block_align = channels * bits / 8;
        Self {
            tag: 1,
            channels,
            rate,
            byte_rate: rate * u32::from(block_align),
            block_align,
            bits,
        }
    }

    fn bytes(self) -> Vec<u8> {
        [
            &self.tag.to_le_bytes()[..],
            &self.channels.to_le_bytes(),
            &self.rate.to_le_bytes(),
            &self.byte_rate.to_le_bytes(),
            &self.block_align.to_le_bytes(),
            &self.bits.to_le_bytes(),
        ]
        .concat()
    }

    fn extensible(self, cb_size: u16, valid_bits: u16, subformat: [u8; 16]) -> Vec<u8> {
        let base = Self {
            tag: 0xfffe,
            ..self
        }
        .bytes();
        [
            &base[..],
            &cb_size.to_le_bytes(),
            &valid_bits.to_le_bytes(),
            &0x3_u32.to_le_bytes(),
            &subformat,
        ]
        .concat()
    }
}

fn chunk(id: &[u8; 4], payload: &[u8]) -> Vec<u8> {
    let mut bytes = id.to_vec();
    bytes.extend((payload.len() as u32).to_le_bytes());
    bytes.extend(payload);
    if payload.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn riff(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body = chunks.concat();
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((body.len() as u32 + 4).to_le_bytes());
    bytes.extend(b"WAVE");
    bytes.extend(body);
    bytes
}

fn wav(fmt: Fmt, samples: &[u8]) -> Vec<u8> {
    riff(&[chunk(b"fmt ", &fmt.bytes()), chunk(b"data", samples)])
}

fn i16_samples(values: &[i16]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn i24_samples(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes().into_iter().take(3))
        .collect()
}

fn i32_samples(values: &[i32]) -> Vec<u8> {
    values
        .iter()
        .flat_map(|value| value.to_le_bytes())
        .collect()
}

fn reference() -> Vec<u8> {
    wav(Fmt::pcm(2, 8000, 16), &i16_samples(&REFERENCE_SAMPLES))
}

fn fmt_chunk(fmt: Fmt) -> Vec<u8> {
    chunk(b"fmt ", &fmt.bytes())
}

fn reference_data() -> Vec<u8> {
    chunk(b"data", &i16_samples(&REFERENCE_SAMPLES))
}

fn with_u32(mut bytes: Vec<u8>, offset: usize, value: u32) -> Vec<u8> {
    bytes[offset..offset + 4].copy_from_slice(&value.to_le_bytes());
    bytes
}

fn with_prefix(mut bytes: Vec<u8>, prefix: &[u8; 4]) -> Vec<u8> {
    bytes[..4].copy_from_slice(prefix);
    bytes
}

fn report_json(report: &WavReport) -> Value {
    serde_json::to_value(report).unwrap()
}

/// Floats are compared within `1e-12`; every other JSON value and every key must match exactly.
fn assert_json_close(actual: &Value, expected: &Value, path: &str) {
    match (actual, expected) {
        (Value::Number(left), Value::Number(right)) if left.is_f64() || right.is_f64() => {
            let (left, right) = (left.as_f64().unwrap(), right.as_f64().unwrap());
            assert!((left - right).abs() <= 1e-12, "{path}: {left} != {right}");
        }
        (Value::Object(left), Value::Object(right)) => {
            assert_eq!(
                left.keys().collect::<Vec<_>>(),
                right.keys().collect::<Vec<_>>(),
                "{path}"
            );
            for (key, value) in right {
                assert_json_close(&left[key], value, &format!("{path}.{key}"));
            }
        }
        (Value::Array(left), Value::Array(right)) => {
            assert_eq!(left.len(), right.len(), "{path}");
            for (index, (left, right)) in left.iter().zip(right).enumerate() {
                assert_json_close(left, right, &format!("{path}[{index}]"));
            }
        }
        _ => assert_eq!(actual, expected, "{path}"),
    }
}

/// The typed accessors are the library interface; they must say exactly what the JSON says.
fn assert_accessors_match_json(report: &WavReport) {
    let json = report_json(report);
    let typed = json!({
        "structure": report.structure(),
        "file": report.file(),
        "errors": report.errors(),
        "warnings": report.warnings(),
        "chunks": report.chunks(),
        "fmt": report.fmt(),
        "data": report.data(),
        "measurements": report.measurements(),
    });
    for (key, value) in typed.as_object().unwrap() {
        assert_eq!(&json[key], value, "{key}");
    }
}

fn channel_peaks(report: &WavReport) -> Vec<(u32, u64)> {
    let peak = report
        .measurements()
        .and_then(|measurements| measurements.sample_peak.as_ref())
        .expect("a valid file with frames has a sample peak");
    peak.channels
        .iter()
        .map(|channel| (channel.magnitude, channel.first_frame))
        .collect()
}

#[test]
fn reference_file_matches_python_wave_bytes_and_reports_exact_metadata_checksums_and_peaks() {
    let report = inspect_wav(&reference());
    assert_eq!(report.structure(), Structure::Valid);
    assert_json_close(
        &report_json(&report),
        &json!({
            "inspector": INSPECTOR_VERSION,
            "file": { "bytes": 76, "sha256": REFERENCE_FILE_SHA256 },
            "structure": "valid",
            "errors": [],
            "warnings": [],
            "chunks": [
                { "id": "fmt ", "offset": 12, "size": 16 },
                { "id": "data", "offset": 36, "size": 32 }
            ],
            "fmt": {
                "format_tag": 1,
                "channels": 2,
                "sample_rate_hz": 8000,
                "byte_rate": 32000,
                "block_align": 4,
                "bits_per_sample": 16,
                "extensible": null
            },
            "data": { "offset": 44, "bytes": 32, "sha256": REFERENCE_DATA_SHA256 },
            "measurements": {
                "frames": 8,
                "duration_seconds": 0.001,
                "sample_peak": {
                    "full_scale": 32768,
                    "overall": { "magnitude": 32768, "dbfs": 0.0 },
                    "channels": [
                        { "channel": 0, "magnitude": 32767, "dbfs": DBFS_32767, "first_frame": 2 },
                        { "channel": 1, "magnitude": 32768, "dbfs": 0.0, "first_frame": 2 }
                    ]
                }
            },
            "not_assessed": [
                "integrated_loudness",
                "true_peak",
                "clipping",
                "listening_quality",
                "publication_readiness"
            ]
        }),
        "$",
    );
    assert_accessors_match_json(&report);
    assert_eq!(report, inspect_wav(&reference()));
}

#[test]
fn sine_tone_reports_analytic_peak_level_first_peak_frame_and_duration() {
    // 1 kHz at 48 kHz peaks at frame 12 (a quarter period); channel 1 is inverted at half level.
    let samples: Vec<i16> = (0..48_000)
        .flat_map(|frame| {
            let phase = 2.0 * std::f64::consts::PI * 1000.0 * f64::from(frame) / 48_000.0;
            let sine = phase.sin();
            [
                (16384.0 * sine).round() as i16,
                (-8192.0 * sine).round() as i16,
            ]
        })
        .collect();
    let report = inspect_wav(&wav(Fmt::pcm(2, 48_000, 16), &i16_samples(&samples)));
    let measurements = report.measurements().expect("valid tone is measured");
    assert_eq!(measurements.frames, 48_000);
    assert_eq!(measurements.duration_seconds, 1.0);
    assert_eq!(channel_peaks(&report), [(16384, 12), (8192, 12)]);
    let peak = measurements.sample_peak.as_ref().unwrap();
    assert!((peak.overall.dbfs.unwrap() - DBFS_HALF).abs() <= 1e-12);
    assert!((peak.channels[1].dbfs.unwrap() - 2.0 * DBFS_HALF).abs() <= 1e-12);
}

#[test]
fn each_supported_width_decodes_its_extremes_signs_and_eight_bit_center() {
    let cases = [
        // 8-bit PCM is unsigned around 128: code 0 is full scale, 255 is one step below it.
        (
            wav(Fmt::pcm(1, 8000, 8), &[128, 200, 0, 255, 128]),
            128,
            vec![(128, 2)],
        ),
        (
            wav(Fmt::pcm(1, 8000, 16), &i16_samples(&[-32768, 32767])),
            32768,
            vec![(32768, 0)],
        ),
        (
            wav(
                Fmt::pcm(1, 8000, 24),
                &i24_samples(&[0x7f_ffff, -0x80_0000, -1]),
            ),
            8_388_608,
            vec![(8_388_608, 1)],
        ),
        // -1 must sign-extend to magnitude 1, never decode as 0xffffff.
        (
            wav(Fmt::pcm(1, 8000, 24), &i24_samples(&[-1, 1, -1])),
            8_388_608,
            vec![(1, 0)],
        ),
        (
            wav(
                Fmt::pcm(2, 8000, 32),
                &i32_samples(&[i32::MIN, i32::MAX, 7, -7]),
            ),
            2_147_483_648,
            vec![(2_147_483_648, 0), (2_147_483_647, 0)],
        ),
    ];
    for (bytes, full_scale, expected) in cases {
        let report = inspect_wav(&bytes);
        assert_eq!(report.errors(), []);
        let peak = report.measurements().unwrap().sample_peak.as_ref().unwrap();
        assert_eq!(peak.full_scale, full_scale);
        assert_eq!(channel_peaks(&report), expected);
        let loudest = expected.iter().map(|(magnitude, _)| *magnitude).max();
        assert_eq!(Some(peak.overall.magnitude), loudest);
    }
}

#[test]
fn extensible_pcm_with_metadata_chunks_keeps_the_data_checksum_of_the_plain_file() {
    // Twenty valid bits left-justified in 24-bit containers, the convention of the format.
    let samples = i24_samples(&[0x12_3450, -0x12_3450, 0x7f_fff0, -0x80_0000]);
    let fmt = Fmt::pcm(2, 48_000, 24);
    let decorated = riff(&[
        chunk(b"JUNK", &[0; 4]),
        chunk(b"fmt ", &fmt.extensible(22, 20, PCM_GUID)),
        chunk(b"LIST", b"INFOx"),
        chunk(b"data", &samples),
    ]);
    let plain = inspect_wav(&wav(fmt, &samples));
    let report = inspect_wav(&decorated);
    assert_eq!(report.errors(), []);
    let json = report_json(&report);
    assert_eq!(
        json["chunks"],
        json!([
            { "id": "JUNK", "offset": 12, "size": 4 },
            { "id": "fmt ", "offset": 24, "size": 40 },
            { "id": "LIST", "offset": 72, "size": 5 },
            { "id": "data", "offset": 86, "size": 12 }
        ])
    );
    assert_eq!(
        json["fmt"]["extensible"],
        json!({
            "cb_size": 22,
            "valid_bits_per_sample": 20,
            "channel_mask": 3,
            "subformat": "00000001-0000-0010-8000-00aa00389b71"
        })
    );
    assert_eq!(report.data().unwrap().offset, 94);
    assert_eq!(report.data().unwrap().sha256, plain.data().unwrap().sha256);
    assert_ne!(report.file().sha256, plain.file().sha256);
    assert_eq!(channel_peaks(&report), [(8_388_592, 1), (8_388_608, 1)]);
    assert_eq!(report.measurements(), plain.measurements());
}

#[test]
fn digital_silence_has_zero_magnitude_and_no_decibel_level() {
    let report = inspect_wav(&wav(Fmt::pcm(2, 8000, 16), &[0; 16]));
    let json = report_json(&report);
    assert_eq!(
        json["measurements"]["sample_peak"],
        json!({
            "full_scale": 32768,
            "overall": { "magnitude": 0, "dbfs": null },
            "channels": [
                { "channel": 0, "magnitude": 0, "dbfs": null, "first_frame": 0 },
                { "channel": 1, "magnitude": 0, "dbfs": null, "first_frame": 0 }
            ]
        })
    );
    // JSON also renders an infinite level as null, so check the typed value is absent.
    let peak = report.measurements().unwrap().sample_peak.as_ref().unwrap();
    assert_eq!(peak.overall.dbfs, None);
    assert!(peak.channels.iter().all(|channel| channel.dbfs.is_none()));
}

#[test]
fn empty_data_chunk_is_structurally_valid_with_no_sample_to_measure() {
    let report = inspect_wav(&wav(Fmt::pcm(1, 44_100, 16), &[]));
    assert_eq!(report.structure(), Structure::Valid);
    assert_eq!(
        report_json(&report)["measurements"],
        json!({ "frames": 0, "duration_seconds": 0.0, "sample_peak": null })
    );
}

#[test]
fn a_full_chunk_table_at_the_limit_is_still_inspected() {
    let mut chunks = vec![fmt_chunk(Fmt::pcm(2, 8000, 16))];
    chunks.extend((2..MAX_CHUNKS).map(|_| chunk(b"JUNK", &[])));
    chunks.push(reference_data());
    let report = inspect_wav(&riff(&chunks));
    assert_eq!(report.errors(), []);
    assert_eq!(report.chunks().len(), MAX_CHUNKS);
}

fn defect_cases() -> Vec<(&'static str, Vec<u8>, Vec<StructuralError>)> {
    use StructuralError::*;
    let pcm = Fmt::pcm(2, 8000, 16);
    let fmt = fmt_chunk(pcm);
    let data = reference_data();
    let stereo_24 = Fmt::pcm(2, 8000, 24);
    let data_24 = chunk(b"data", &[0; 24]);
    let extensible = |payload: Vec<u8>| riff(&[chunk(b"fmt ", &payload), data_24.clone()]);
    let mut too_many = vec![fmt.clone()];
    too_many.extend((0..MAX_CHUNKS).map(|_| chunk(b"JUNK", &[])));
    too_many.push(data.clone());
    let mut pad_cut = riff(&[fmt.clone(), data.clone(), chunk(b"LIST", b"abc")]);
    pad_cut.pop();
    vec![
        (
            "empty file",
            vec![],
            vec![TruncatedRiffHeader { file_bytes: 0 }],
        ),
        (
            "header cut at 11 bytes",
            reference()[..11].to_vec(),
            vec![TruncatedRiffHeader { file_bytes: 11 }],
        ),
        (
            "not a RIFF file",
            with_prefix(reference(), b"OggS"),
            vec![NotRiff {
                found: ChunkId(*b"OggS"),
            }],
        ),
        (
            "big-endian RIFX",
            with_prefix(reference(), b"RIFX"),
            vec![UnsupportedContainer {
                found: ChunkId(*b"RIFX"),
            }],
        ),
        (
            "64-bit RF64",
            with_prefix(reference(), b"RF64"),
            vec![UnsupportedContainer {
                found: ChunkId(*b"RF64"),
            }],
        ),
        (
            "RIFF form is not WAVE",
            {
                let mut bytes = reference();
                bytes[8..12].copy_from_slice(b"AVI ");
                bytes
            },
            vec![NotWave {
                found: ChunkId(*b"AVI "),
            }],
        ),
        (
            "RIFF size cannot hold the form type",
            with_u32(reference(), 4, 2),
            vec![RiffSizeTooSmall { declared_size: 2 }],
        ),
        (
            "RIFF body holds only the form type",
            riff(&[]),
            vec![
                MissingChunk { id: ChunkId::FMT },
                MissingChunk { id: ChunkId::DATA },
            ],
        ),
        (
            "data truncated by ten bytes",
            reference()[..66].to_vec(),
            vec![
                RiffSizeExceedsFile {
                    riff_end: 76,
                    file_bytes: 66,
                },
                ChunkOverrun {
                    id: ChunkId::DATA,
                    offset: 36,
                    declared_size: 32,
                    available: 22,
                },
            ],
        ),
        (
            "bytes after the RIFF chunk",
            [reference(), vec![0; 3]].concat(),
            vec![TrailingBytes {
                riff_end: 76,
                file_bytes: 79,
            }],
        ),
        (
            "declared sizes far beyond the file",
            with_u32(with_u32(reference(), 4, u32::MAX), 40, u32::MAX),
            vec![
                RiffSizeExceedsFile {
                    riff_end: u64::from(u32::MAX) + 8,
                    file_bytes: 76,
                },
                ChunkOverrun {
                    id: ChunkId::DATA,
                    offset: 36,
                    declared_size: u32::MAX,
                    available: 32,
                },
            ],
        ),
        (
            "no fmt chunk",
            riff(&[data.clone()]),
            vec![MissingChunk { id: ChunkId::FMT }],
        ),
        (
            "no data chunk",
            riff(&[fmt.clone()]),
            vec![MissingChunk { id: ChunkId::DATA }],
        ),
        (
            "data precedes fmt",
            riff(&[data.clone(), fmt.clone()]),
            vec![DataBeforeFmt {
                data_offset: 12,
                fmt_offset: 52,
            }],
        ),
        (
            "second fmt chunk",
            riff(&[fmt.clone(), fmt.clone(), data.clone()]),
            vec![DuplicateChunk {
                id: ChunkId::FMT,
                offset: 36,
            }],
        ),
        (
            "second data chunk",
            riff(&[fmt.clone(), data.clone(), data.clone()]),
            vec![DuplicateChunk {
                id: ChunkId::DATA,
                offset: 76,
            }],
        ),
        (
            "pad byte counted by the RIFF size but cut off",
            pad_cut,
            vec![
                RiffSizeExceedsFile {
                    riff_end: 88,
                    file_bytes: 87,
                },
                MissingPadByte {
                    id: ChunkId(*b"LIST"),
                    offset: 76,
                },
            ],
        ),
        (
            "unprintable chunk identifier",
            riff(&[
                fmt.clone(),
                chunk(&[0, b'a', 0x7f, b'\\'], &[]),
                data.clone(),
            ]),
            vec![InvalidChunkId {
                offset: 36,
                id: ChunkId([0, b'a', 0x7f, b'\\']),
            }],
        ),
        (
            "stray bytes shorter than a chunk header",
            riff(&[fmt.clone(), data.clone(), b"JUNK".to_vec()]),
            vec![TruncatedChunkHeader {
                offset: 76,
                available: 4,
            }],
        ),
        (
            "more chunks than the inspection limit",
            riff(&too_many),
            vec![ChunkLimitExceeded { limit: MAX_CHUNKS }],
        ),
        (
            "fmt shorter than its base fields",
            riff(&[chunk(b"fmt ", &pcm.bytes()[..14]), data.clone()]),
            vec![FmtTooShort {
                declared_size: 14,
                minimum: 16,
            }],
        ),
        (
            "IEEE float format tag",
            riff(&[
                fmt_chunk(Fmt {
                    tag: 3,
                    ..Fmt::pcm(2, 8000, 32)
                }),
                data.clone(),
            ]),
            vec![UnsupportedFormatTag { format_tag: 3 }],
        ),
        (
            "extensible float subformat",
            extensible(stereo_24.extensible(22, 24, FLOAT_GUID)),
            vec![UnsupportedSubformat {
                subformat: Guid(FLOAT_GUID),
            }],
        ),
        (
            "extensible header without its extension",
            extensible(stereo_24.extensible(0, 24, PCM_GUID)[..18].to_vec()),
            vec![FmtTooShort {
                declared_size: 18,
                minimum: 40,
            }],
        ),
        (
            "extensible cbSize below the extension",
            extensible(stereo_24.extensible(0, 24, PCM_GUID)),
            vec![InvalidExtensionSize {
                cb_size: 0,
                minimum: 22,
                available: 22,
            }],
        ),
        (
            "extensible cbSize beyond the chunk",
            extensible(stereo_24.extensible(40, 24, PCM_GUID)),
            vec![InvalidExtensionSize {
                cb_size: 40,
                minimum: 22,
                available: 22,
            }],
        ),
        (
            "more valid bits than the container",
            extensible(stereo_24.extensible(22, 25, PCM_GUID)),
            vec![InvalidValidBits {
                valid_bits_per_sample: 25,
                bits_per_sample: 24,
            }],
        ),
        (
            "zero valid bits",
            extensible(stereo_24.extensible(22, 0, PCM_GUID)),
            vec![InvalidValidBits {
                valid_bits_per_sample: 0,
                bits_per_sample: 24,
            }],
        ),
        (
            "zero channels",
            riff(&[
                fmt_chunk(Fmt {
                    channels: 0,
                    block_align: 0,
                    byte_rate: 0,
                    ..pcm
                }),
                data.clone(),
            ]),
            vec![ZeroChannels],
        ),
        (
            // Without channels the block align and byte rate have nothing to agree with.
            "zero channels beside stereo frame fields",
            riff(&[fmt_chunk(Fmt { channels: 0, ..pcm }), data.clone()]),
            vec![ZeroChannels],
        ),
        (
            "zero sample rate",
            riff(&[
                fmt_chunk(Fmt {
                    rate: 0,
                    byte_rate: 0,
                    ..pcm
                }),
                data.clone(),
            ]),
            vec![ZeroSampleRate],
        ),
        (
            "12-bit samples",
            riff(&[fmt_chunk(Fmt { bits: 12, ..pcm }), data.clone()]),
            vec![UnsupportedBitsPerSample {
                bits_per_sample: 12,
            }],
        ),
        (
            "block align disagrees with channels and width",
            riff(&[
                fmt_chunk(Fmt {
                    block_align: 3,
                    ..pcm
                }),
                data.clone(),
            ]),
            vec![BlockAlignMismatch {
                declared: 3,
                expected: 4,
            }],
        ),
        (
            "byte rate disagrees with rate and block align",
            riff(&[
                fmt_chunk(Fmt {
                    byte_rate: 1,
                    ..pcm
                }),
                data.clone(),
            ]),
            vec![ByteRateMismatch {
                declared: 1,
                expected: 32000,
            }],
        ),
        (
            "data ends inside a frame",
            riff(&[fmt.clone(), chunk(b"data", &[0; 30])]),
            vec![PartialFrame {
                data_bytes: 30,
                block_align: 4,
            }],
        ),
        (
            "several independent defects",
            [
                riff(&[
                    fmt_chunk(Fmt {
                        byte_rate: 1,
                        ..pcm
                    }),
                    data.clone(),
                    data.clone(),
                ]),
                vec![0; 2],
            ]
            .concat(),
            vec![
                TrailingBytes {
                    riff_end: 116,
                    file_bytes: 118,
                },
                DuplicateChunk {
                    id: ChunkId::DATA,
                    offset: 76,
                },
                ByteRateMismatch {
                    declared: 1,
                    expected: 32000,
                },
            ],
        ),
    ]
}

#[test]
fn each_defect_fixture_reports_exactly_its_structural_errors_and_no_measurement() {
    for (name, bytes, expected) in defect_cases() {
        let report = inspect_wav(&bytes);
        assert_eq!(report.errors(), expected, "{name}");
        assert_eq!(report.warnings(), [], "{name}");
        assert_eq!(report.structure(), Structure::Invalid, "{name}");
        assert_eq!(report.measurements(), None, "{name}");
        assert_eq!(report.file().bytes, bytes.len() as u64, "{name}");
        assert_eq!(report, inspect_wav(&bytes), "{name}");
        assert_accessors_match_json(&report);
    }
}

#[test]
fn an_unpadded_final_chunk_sized_consistently_is_a_warning_not_an_error() {
    // Python's `wave` writes odd-sized data this way: no pad byte, RIFF size without it.
    let mut unpadded = riff(&[
        fmt_chunk(Fmt::pcm(1, 8000, 8)),
        chunk(b"data", &[0, 128, 255]),
    ]);
    unpadded.pop();
    let unpadded = with_u32(unpadded.clone(), 4, unpadded.len() as u32 - 8);
    let warning = json!([{ "code": "unpadded_final_chunk", "id": "data", "offset": 36 }]);
    let report = inspect_wav(&unpadded);
    assert_eq!(report.structure(), Structure::Valid);
    assert_eq!(report_json(&report)["warnings"], warning);
    assert_eq!(report.measurements().unwrap().frames, 3);
    assert_eq!(channel_peaks(&report), [(128, 0)]);
    assert_accessors_match_json(&report);

    // A pad byte written after the declared RIFF end is still a byte outside the RIFF chunk.
    let padded_outside = inspect_wav(&[unpadded, vec![0]].concat());
    assert_eq!(
        padded_outside.errors(),
        [StructuralError::TrailingBytes {
            riff_end: 47,
            file_bytes: 48,
        }]
    );
    assert_eq!(report_json(&padded_outside)["warnings"], warning);
    assert_eq!(padded_outside.measurements(), None);
}

#[test]
fn defects_serialize_as_stable_codes_with_escaped_identifiers() {
    let report = inspect_wav(&riff(&[
        fmt_chunk(Fmt {
            channels: 0,
            block_align: 0,
            byte_rate: 0,
            ..Fmt::pcm(2, 8000, 16)
        }),
        chunk(&[0, b'a', 0x7f, b'\\'], &[]),
        reference_data(),
    ]));
    let json = report_json(&report);
    assert_eq!(
        json["errors"],
        json!([
            { "code": "invalid_chunk_id", "offset": 36, "id": "\\x00a\\x7f\\x5c" },
            { "code": "zero_channels" }
        ])
    );
    assert_eq!(json["structure"], "invalid");
    assert_eq!(json["measurements"], Value::Null);
    assert_eq!(json["not_assessed"], json!(NOT_ASSESSED));
    let truncated = report_json(&inspect_wav(&reference()[..66]));
    assert_eq!(truncated["data"], Value::Null);
    assert_eq!(
        truncated["errors"][1],
        json!({
            "code": "chunk_overrun",
            "id": "data",
            "offset": 36,
            "declared_size": 32,
            "available": 22
        })
    );
}

#[test]
fn every_strict_prefix_of_a_valid_file_is_invalid_and_unmeasured() {
    let decorated = riff(&[
        chunk(
            b"fmt ",
            &Fmt::pcm(2, 48_000, 24).extensible(22, 24, PCM_GUID),
        ),
        chunk(b"LIST", b"INFOx"),
        chunk(b"data", &[0x11; 18]),
    ]);
    for valid in [reference(), decorated] {
        assert_eq!(inspect_wav(&valid).structure(), Structure::Valid);
        for end in 0..valid.len() {
            let report = inspect_wav(&valid[..end]);
            assert_eq!(report.structure(), Structure::Invalid, "prefix {end}");
            assert_eq!(report.measurements(), None, "prefix {end}");
        }
    }
}

#[test]
fn single_byte_corruption_never_panics_and_sample_bytes_change_only_data_facts() {
    let original_bytes = reference();
    let original = inspect_wav(&original_bytes);
    let data_start = original.data().unwrap().offset as usize;
    for offset in 0..original_bytes.len() {
        for mask in [0x01, 0x80, 0xff] {
            let mut corrupted = original_bytes.clone();
            corrupted[offset] ^= mask;
            let report = inspect_wav(&corrupted);
            assert_eq!(report, inspect_wav(&corrupted), "offset {offset}");
            assert_ne!(report.file().sha256, original.file().sha256);
            if offset >= data_start {
                assert_eq!(report.structure(), Structure::Valid, "offset {offset}");
                assert_eq!(report.chunks(), original.chunks());
                assert_eq!(report.fmt(), original.fmt());
                assert_ne!(
                    report.data().unwrap().sha256,
                    original.data().unwrap().sha256
                );
            }
        }
    }
}

struct Scratch(PathBuf);

impl Scratch {
    fn new(name: &str) -> Self {
        let path =
            std::env::temp_dir().join(format!("cantos-inspect-wav-{}-{name}", std::process::id()));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }

    fn file(&self, name: &str, bytes: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, bytes).unwrap();
        path
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

fn inspect_cli(args: &[&std::ffi::OsStr]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_inspect-wav"))
        .args(args)
        .output()
        .unwrap()
}

fn cli_with_path(path: &Path, extra: &[&str]) -> Output {
    let mut args = vec![path.as_os_str()];
    args.extend(extra.iter().map(std::ffi::OsStr::new));
    inspect_cli(&args)
}

#[test]
fn cli_prints_the_library_report_and_exits_by_structure() {
    let scratch = Scratch::new("report");
    let valid = scratch.file("reference.wav", &reference());
    let output = cli_with_path(&valid, &[]);
    assert_eq!(output.status.code(), Some(0));
    assert!(output.stderr.is_empty());
    let printed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(printed, report_json(&inspect_wav(&reference())));
    assert_eq!(printed["file"]["sha256"], REFERENCE_FILE_SHA256);

    let truncated = &reference()[..66];
    let invalid = scratch.file("truncated.wav", truncated);
    let output = cli_with_path(&invalid, &["--max-bytes", "66"]);
    assert_eq!(output.status.code(), Some(1));
    let printed: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(printed, report_json(&inspect_wav(truncated)));
}

#[test]
fn cli_refuses_bad_usage_unreadable_and_oversized_input_without_a_report() {
    let scratch = Scratch::new("refusals");
    let valid = scratch.file("reference.wav", &reference());
    let missing = scratch.0.join("missing.wav");
    let refusals: Vec<(Output, &str)> = vec![
        (inspect_cli(&[]), "missing input file"),
        (
            cli_with_path(&valid, &["extra.wav"]),
            "exactly one input file",
        ),
        (cli_with_path(&valid, &["--loudness"]), "unexpected option"),
        (
            cli_with_path(&valid, &["--max-bytes", "80", "--max-bytes", "90"]),
            "--max-bytes given more than once",
        ),
        (
            cli_with_path(&valid, &["--max-bytes"]),
            "--max-bytes expects",
        ),
        (
            cli_with_path(&valid, &["--max-bytes", "0"]),
            "--max-bytes expects",
        ),
        (
            cli_with_path(&valid, &["--max-bytes", "abc"]),
            "--max-bytes expects",
        ),
        (
            cli_with_path(&valid, &["--max-bytes", "4294967304"]),
            "--max-bytes expects",
        ),
        (cli_with_path(&missing, &[]), "input could not be read"),
        (cli_with_path(&scratch.0, &[]), "not a regular file"),
        (
            cli_with_path(&valid, &["--max-bytes", "75"]),
            "exceeds the 75-byte limit",
        ),
    ];
    for (output, message) in refusals {
        assert_eq!(output.status.code(), Some(2), "{message}");
        assert!(output.stdout.is_empty(), "{message}");
        assert!(
            String::from_utf8_lossy(&output.stderr).contains(message),
            "{message}: {}",
            String::from_utf8_lossy(&output.stderr)
        );
    }
    for limit in ["76", "4294967303"] {
        let accepted = cli_with_path(&valid, &["--max-bytes", limit]);
        assert_eq!(accepted.status.code(), Some(0), "{limit}");
    }
    let help = inspect_cli(&[std::ffi::OsStr::new("--help")]);
    assert_eq!(help.status.code(), Some(0));
    assert!(String::from_utf8_lossy(&help.stdout).starts_with("usage: inspect-wav"));
}
