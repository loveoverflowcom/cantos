//! Real, owned synthetic signals generated at test time; no audio is committed to Git.
//! This suite requires the bounded read-only ffmpeg/ffprobe inspection adapter.
use std::{fs, path::PathBuf, process::Command};

use cantos_api::contexts::media::audio_intake::{inspect_audio, AudioIntakeError, DeliveryProfile};
use sha2::{Digest, Sha256};
use uuid::Uuid;

struct Fixtures(PathBuf);
impl Fixtures {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("cantos-audio-test-{}", Uuid::new_v4().simple()));
        fs::create_dir(&directory).unwrap();
        Self(directory)
    }
    fn encode(&self, filename: &str, codec: &str, bitrate: &str, channels: &str) -> Vec<u8> {
        let path = self.0.join(filename);
        let result = Command::new("ffmpeg")
            .args([
                "-hide_banner",
                "-loglevel",
                "error",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=440:sample_rate=48000:duration=2",
                "-ac",
                channels,
                "-c:a",
                codec,
                "-b:a",
                bitrate,
            ])
            .args(if codec == "vorbis" {
                vec!["-strict", "experimental"]
            } else {
                vec![]
            })
            .args(if codec == "libopus" {
                vec!["-vbr", "off"]
            } else {
                vec![]
            })
            .arg(&path)
            .output()
            .expect("ffmpeg prerequisite is missing");
        assert!(result.status.success(), "owned fixture encoding failed");
        fs::read(path).unwrap()
    }
}
impl Drop for Fixtures {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
#[ignore = "requires installed trusted ffmpeg/ffprobe; run through xtask audio inspection gate"]
fn four_profiles_accept_actual_mono_ogg_opus_and_derive_exact_byte_identity() {
    let fixtures = Fixtures::new();
    for (profile, bitrate) in [
        (DeliveryProfile::OpusMono20V1, "20k"),
        (DeliveryProfile::OpusMono24V1, "24k"),
        (DeliveryProfile::OpusMono32V1, "32k"),
        (DeliveryProfile::OpusMono40V1, "40k"),
    ] {
        let bytes = fixtures.encode(&format!("{bitrate}.opus"), "libopus", bitrate, "1");
        let verified = inspect_audio(&bytes, profile).unwrap();
        assert_eq!(verified.bytes(), bytes);
        let metadata = verified.metadata();
        assert_eq!(metadata.sha256, format!("{:x}", Sha256::digest(&bytes)));
        assert_eq!(metadata.byte_length, bytes.len() as u64);
        assert_eq!(metadata.profile_id, profile.id());
        assert_eq!(
            (
                &metadata.container[..],
                &metadata.codec[..],
                metadata.channels,
                metadata.sample_rate_hz
            ),
            ("ogg", "opus", 1, 48_000)
        );
        assert!((1900..=2100).contains(&metadata.duration_ms));
        assert!(metadata.audio_bitrate_bps <= profile.maximum_bitrate_bps());
    }
}

#[test]
#[ignore = "requires installed trusted ffmpeg/ffprobe; run through xtask audio inspection gate"]
fn actual_wrong_codec_container_channels_profile_and_corruption_are_rejected() {
    let fixtures = Fixtures::new();
    let profile = DeliveryProfile::OpusMono24V1;
    let valid = fixtures.encode("valid.opus", "libopus", "24k", "1");
    for (bytes, expected) in [
        (
            fixtures.encode("wrong.m4a", "aac", "40k", "1"),
            AudioIntakeError::UnsupportedContainer,
        ),
        (
            fixtures.encode("wrong.ogg", "vorbis", "96k", "2"),
            AudioIntakeError::CodecMismatch,
        ),
        (
            fixtures.encode("stereo.opus", "libopus", "24k", "2"),
            AudioIntakeError::ChannelMismatch,
        ),
        (
            fixtures.encode("high.opus", "libopus", "96k", "1"),
            AudioIntakeError::BitrateLimit,
        ),
        (
            valid[..valid.len() - 1].to_vec(),
            AudioIntakeError::MalformedAudio,
        ),
        (
            [valid.as_slice(), valid.as_slice()].concat(),
            AudioIntakeError::MalformedAudio,
        ),
        (
            corrupt_packet_with_correct_crc(&valid),
            AudioIntakeError::MalformedAudio,
        ),
    ] {
        assert_eq!(inspect_audio(&bytes, profile).unwrap_err(), expected);
    }
    let mut damaged = valid;
    let last = damaged.len() - 1;
    damaged[last] ^= 0x40;
    assert_eq!(
        inspect_audio(&damaged, profile).unwrap_err(),
        AudioIntakeError::MalformedAudio
    );
}

fn corrupt_packet_with_correct_crc(bytes: &[u8]) -> Vec<u8> {
    let mut broken = bytes.to_vec();
    let mut offset = 0;
    loop {
        let laces = usize::from(broken[offset + 26]);
        let body = offset + 27 + laces;
        let size: usize = broken[offset + 27..body]
            .iter()
            .map(|size| usize::from(*size))
            .sum();
        let end = body + size;
        if offset > 0 && !broken[body..end].starts_with(b"OpusTags") {
            // Two 10ms hybrid frames preserve the valid 20ms granule, but code-2 declares
            // a first frame larger than this packet. Only actual decoding rejects this.
            broken[body] = (12 << 3) | 2;
            broken[body + 1] = 255;
            broken[body + 2] = 255;
            broken[offset + 22..offset + 26].fill(0);
            let mut crc = 0_u32;
            for byte in &broken[offset..end] {
                crc ^= u32::from(*byte) << 24;
                for _ in 0..8 {
                    crc = if crc & 0x8000_0000 == 0 {
                        crc << 1
                    } else {
                        (crc << 1) ^ 0x04c1_1db7
                    };
                }
            }
            broken[offset + 22..offset + 26].copy_from_slice(&crc.to_le_bytes());
            return broken;
        }
        offset = end;
    }
}

#[test]
fn profiles_and_byte_admission_do_not_use_filenames_or_mime_claims() {
    assert_eq!(DeliveryProfile::from_id("aac-lc-mono-40-v1"), None);
    assert_eq!(DeliveryProfile::from_id("../../opus-mono-24-v1"), None);
    assert_eq!(
        inspect_audio(b"Content-Type: audio/opus", DeliveryProfile::OpusMono24V1).unwrap_err(),
        AudioIntakeError::UnsupportedContainer
    );
}
