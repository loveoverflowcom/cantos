//! Only synthetic in-memory WAV fixtures; no provider output or committed audio.

use cantos_api::{
    tts::audio::{
        inspect_output, inspect_output_for_profile, AudioError, OutputProfile,
        ESPEAK_OUTPUT_PROFILE_VERSION, ESPEAK_OUTPUT_SAMPLE_RATE_HZ, MAX_OUTPUT_BYTES,
        MAX_OUTPUT_SECONDS, OUTPUT_BITS_PER_SAMPLE, OUTPUT_CHANNELS, OUTPUT_PROFILE_VERSION,
        OUTPUT_SAMPLE_RATE_HZ,
    },
    wav_inspection::{ChunkId, StructuralError, INSPECTOR_VERSION},
};
use serde_json::json;
use sha2::{Digest, Sha256};

fn sha256(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn pcm_wav(rate: u32, channels: u16, bits: u16, samples: &[u8]) -> Vec<u8> {
    let block_align = channels * bits / 8;
    let padded_length = samples.len() + samples.len() % 2;
    let mut bytes = b"RIFF".to_vec();
    bytes.extend((36 + padded_length as u32).to_le_bytes());
    bytes.extend(b"WAVEfmt ");
    bytes.extend(16_u32.to_le_bytes());
    bytes.extend(1_u16.to_le_bytes());
    bytes.extend(channels.to_le_bytes());
    bytes.extend(rate.to_le_bytes());
    bytes.extend((rate * u32::from(block_align)).to_le_bytes());
    bytes.extend(block_align.to_le_bytes());
    bytes.extend(bits.to_le_bytes());
    bytes.extend(b"data");
    bytes.extend((samples.len() as u32).to_le_bytes());
    bytes.extend(samples);
    if samples.len() % 2 == 1 {
        bytes.push(0);
    }
    bytes
}

fn output_wav(samples: &[i16]) -> Vec<u8> {
    let payload: Vec<_> = samples
        .iter()
        .flat_map(|sample| sample.to_le_bytes())
        .collect();
    pcm_wav(24_000, 1, 16, &payload)
}

#[test]
fn admission_carries_exact_byte_and_decoded_frame_facts() {
    let bytes = output_wav(&[-32768, -1, 0, 32767]);
    let digest = sha256(&bytes);
    let admitted = inspect_output(&bytes, &digest).expect("synthetic PCM should fit the profile");

    assert_eq!(admitted.file_sha256(), digest);
    assert_eq!(admitted.frames(), 4);
    assert_eq!(admitted.sample_rate_hz(), OUTPUT_SAMPLE_RATE_HZ);
    assert_eq!(admitted.channels(), OUTPUT_CHANNELS);
    assert_eq!(admitted.bits_per_sample(), OUTPUT_BITS_PER_SAMPLE);
    assert_eq!(admitted.inspector_version(), INSPECTOR_VERSION);
    assert_eq!(admitted.output_profile_version(), OUTPUT_PROFILE_VERSION);
    assert_eq!(
        serde_json::to_value(&admitted).expect("facts serialize"),
        json!({
            "file_sha256": digest,
            "frames": 4,
            "sample_rate_hz": 24_000,
            "channels": 1,
            "bits_per_sample": 16,
            "inspector_version": INSPECTOR_VERSION,
            "output_profile_version": OUTPUT_PROFILE_VERSION,
        })
    );
}

#[test]
fn espeak_profile_admits_exact_output_rate_and_preserves_the_profile_identity() {
    let bytes = pcm_wav(22_050, 1, 16, &[0, 0, 1, 0, 255, 255]);
    let digest = sha256(&bytes);
    let admitted = inspect_output_for_profile(&bytes, &digest, OutputProfile::Espeak22Khz)
        .expect("eSpeak mono16 output fits its explicit profile");
    assert_eq!(admitted.file_sha256(), digest);
    assert_eq!(admitted.frames(), 3);
    assert_eq!(admitted.sample_rate_hz(), ESPEAK_OUTPUT_SAMPLE_RATE_HZ);
    assert_eq!(admitted.channels(), 1);
    assert_eq!(admitted.bits_per_sample(), 16);
    assert_eq!(admitted.inspector_version(), INSPECTOR_VERSION);
    assert_eq!(
        admitted.output_profile_version(),
        ESPEAK_OUTPUT_PROFILE_VERSION
    );
}

#[test]
fn profiles_reject_each_others_rates_without_conversion_or_fallback() {
    let offline = output_wav(&[1]);
    assert_eq!(
        inspect_output_for_profile(&offline, &sha256(&offline), OutputProfile::Espeak22Khz),
        Err(AudioError::UnsupportedProfile {
            format_tag: 1,
            sample_rate_hz: 24_000,
            channels: 1,
            bits_per_sample: 16,
        })
    );

    let espeak = pcm_wav(22_050, 1, 16, &[1, 0]);
    assert_eq!(
        inspect_output(&espeak, &sha256(&espeak)),
        Err(AudioError::UnsupportedProfile {
            format_tag: 1,
            sample_rate_hz: 22_050,
            channels: 1,
            bits_per_sample: 16,
        })
    );
}

#[test]
fn espeak_duration_bound_uses_its_exact_sample_rate() {
    let bytes = pcm_wav(22_050, 1, 16, &vec![0; 2_646_000 * 2]);
    assert_eq!(
        inspect_output_for_profile(&bytes, &sha256(&bytes), OutputProfile::Espeak22Khz)
            .expect("eSpeak output fits exactly 120 seconds")
            .frames(),
        2_646_000
    );

    let bytes = pcm_wav(22_050, 1, 16, &vec![0; 2_646_001 * 2]);
    assert_eq!(
        inspect_output_for_profile(&bytes, &sha256(&bytes), OutputProfile::Espeak22Khz),
        Err(AudioError::DurationExceeded {
            frames: 2_646_001,
            maximum_frames: 2_646_000,
        })
    );
}

#[test]
fn no_bytes_and_no_decoded_frames_are_distinct_defects() {
    assert_eq!(
        inspect_output(&[], &sha256(&[])),
        Err(AudioError::InvalidWav {
            errors: vec![StructuralError::TruncatedRiffHeader { file_bytes: 0 }],
        })
    );
    let bytes = output_wav(&[]);
    assert_eq!(
        inspect_output(&bytes, &sha256(&bytes)),
        Err(AudioError::EmptyFrames)
    );
}

#[test]
fn actual_byte_limit_rejects_before_digest_or_riff_inspection() {
    let bytes = vec![0; MAX_OUTPUT_BYTES + 1];
    assert_eq!(
        inspect_output(&bytes, "invalid"),
        Err(AudioError::OutputTooLarge {
            actual_bytes: 16 * 1024 * 1024 + 1,
            maximum_bytes: 16 * 1024 * 1024,
        })
    );
    drop(bytes);

    let bytes = vec![0; MAX_OUTPUT_BYTES];
    assert_eq!(
        inspect_output(&bytes, &sha256(&bytes)),
        Err(AudioError::InvalidWav {
            errors: vec![StructuralError::NotRiff {
                found: ChunkId([0; 4]),
            }],
        })
    );
}

#[test]
fn admission_requires_exact_lowercase_sha256_and_matching_bytes() {
    let bytes = output_wav(&[1]);
    let digest = sha256(&bytes);
    for malformed in [
        String::new(),
        "f".repeat(63),
        "f".repeat(65),
        "g".repeat(64),
        "F".repeat(64),
        "é".repeat(32),
        format!(" {digest}"),
    ] {
        assert_eq!(
            inspect_output(&bytes, &malformed),
            Err(AudioError::InvalidExpectedSha256),
            "malformed digest {malformed:?}"
        );
    }
    assert_eq!(
        inspect_output(&bytes, &"0".repeat(64)),
        Err(AudioError::Sha256Mismatch {
            expected: "0".repeat(64),
            actual: digest,
        })
    );
}

#[test]
fn partial_decoded_frame_preserves_the_inspector_defect() {
    let bytes = pcm_wav(24_000, 1, 16, &[1, 2, 3]);
    assert_eq!(
        inspect_output(&bytes, &sha256(&bytes)),
        Err(AudioError::InvalidWav {
            errors: vec![StructuralError::PartialFrame {
                data_bytes: 3,
                block_align: 2,
            }],
        })
    );
}

#[test]
fn inconsistent_pcm_header_and_non_pcm_encoding_are_rejected() {
    let mut wrong_byte_rate = output_wav(&[1]);
    wrong_byte_rate[28..32].copy_from_slice(&1_u32.to_le_bytes());
    assert_eq!(
        inspect_output(&wrong_byte_rate, &sha256(&wrong_byte_rate)),
        Err(AudioError::InvalidWav {
            errors: vec![StructuralError::ByteRateMismatch {
                declared: 1,
                expected: 48_000,
            }],
        })
    );

    let mut unsupported = output_wav(&[1]);
    unsupported[20..22].copy_from_slice(&3_u16.to_le_bytes());
    assert_eq!(
        inspect_output(&unsupported, &sha256(&unsupported)),
        Err(AudioError::InvalidWav {
            errors: vec![StructuralError::UnsupportedFormatTag { format_tag: 3 }],
        })
    );
}

#[test]
fn structurally_valid_pcm_outside_the_offline_profile_is_rejected() {
    for (rate, channels, bits, payload) in [
        (8_000, 1, 16, vec![0, 0]),
        (48_000, 1, 16, vec![0, 0]),
        (24_000, 2, 16, vec![0, 0, 0, 0]),
        (24_000, 1, 8, vec![128]),
        (24_000, 1, 24, vec![0, 0, 0]),
    ] {
        let bytes = pcm_wav(rate, channels, bits, &payload);
        assert_eq!(
            inspect_output(&bytes, &sha256(&bytes)),
            Err(AudioError::UnsupportedProfile {
                format_tag: 1,
                sample_rate_hz: rate,
                channels,
                bits_per_sample: bits,
            })
        );
    }
}

#[test]
fn extensible_pcm_requires_an_explicit_future_profile_version() {
    let mut bytes = output_wav(&[1]);
    bytes[16..20].copy_from_slice(&40_u32.to_le_bytes());
    bytes[20..22].copy_from_slice(&0xfffe_u16.to_le_bytes());
    let extension = [
        &22_u16.to_le_bytes()[..],
        &16_u16.to_le_bytes(),
        &0_u32.to_le_bytes(),
        &[
            0x01, 0x00, 0x00, 0x00, 0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0xaa, 0x00, 0x38,
            0x9b, 0x71,
        ],
    ]
    .concat();
    bytes.splice(36..36, extension);
    let riff_size = bytes.len() as u32 - 8;
    bytes[4..8].copy_from_slice(&riff_size.to_le_bytes());
    assert_eq!(
        inspect_output(&bytes, &sha256(&bytes)),
        Err(AudioError::UnsupportedProfile {
            format_tag: 0xfffe,
            sample_rate_hz: 24_000,
            channels: 1,
            bits_per_sample: 16,
        })
    );
}

#[test]
fn duration_bound_uses_exact_frames_and_admits_the_boundary() {
    let maximum_frames = u64::from(OUTPUT_SAMPLE_RATE_HZ) * MAX_OUTPUT_SECONDS;
    assert_eq!(maximum_frames, 2_880_000);
    let bytes = pcm_wav(24_000, 1, 16, &vec![0; 2_880_000 * 2]);
    let admitted = inspect_output(&bytes, &sha256(&bytes)).expect("120 seconds fits exactly");
    assert_eq!(admitted.frames(), 2_880_000);

    let bytes = pcm_wav(24_000, 1, 16, &vec![0; 2_880_001 * 2]);
    assert_eq!(
        inspect_output(&bytes, &sha256(&bytes)),
        Err(AudioError::DurationExceeded {
            frames: 2_880_001,
            maximum_frames: 2_880_000,
        })
    );
}

#[test]
fn digital_silence_is_structurally_admissible_without_quality_approval() {
    let bytes = output_wav(&[0, 0, 0]);
    assert_eq!(
        inspect_output(&bytes, &sha256(&bytes))
            .expect("silence has complete non-empty frames")
            .frames(),
        3
    );
}

#[test]
fn every_truncated_prefix_is_rejected_without_an_admission_value() {
    let bytes = output_wav(&[-1, 0, 1]);
    for length in 0..bytes.len() {
        let prefix = &bytes[..length];
        let result = inspect_output(prefix, &sha256(prefix));
        match result {
            Err(AudioError::InvalidWav { errors }) => {
                assert!(!errors.is_empty(), "prefix length {length}");
            }
            other => panic!("unexpected admission at prefix length {length}: {other:?}"),
        }
    }
}
