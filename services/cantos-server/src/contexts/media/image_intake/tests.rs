use std::{io::Write, sync::Arc, time::Duration};

use flate2::{write::ZlibEncoder, Compression};
use proptest::prelude::*;

use super::*;

const PNG: &[u8] = include_bytes!("fixtures/python-rgb-2x3.png");
const PNG16: &[u8] = include_bytes!("fixtures/python-rgba16-2x3.png");
const JPEG: &[u8] = include_bytes!("fixtures/pillow-rgb-2x3.jpg");
const AVIF: &[u8] = include_bytes!("fixtures/ffmpeg-yuv420p-2x3.avif");

fn lanes() -> Arc<Semaphore> {
    Arc::new(Semaphore::new(2))
}

fn inspect(bytes: &[u8], mime: ImageMime) -> Result<VerifiedImage, ImageIntakeError> {
    inspect_body(
        AdmittedBody {
            bytes: bytes.to_vec(),
            _admission: ImageAdmission::from_lanes(lanes()).unwrap(),
        },
        mime,
    )
}

#[test]
fn independent_png_jpeg_and_sixteen_bit_fixtures_decode_exactly() {
    // PNGs are Python struct/zlib, JPEG is Pillow12.3.0/libjpeg. SHA256 expectations
    // are independently computed by Python hashlib, not by the Rust acceptance code.
    for (bytes, mime, hash) in [
        (
            PNG,
            ImageMime::Png,
            "7b222c7332a2d0a325b482be5ff001918ff8470f52405b99d4c8351856bed276",
        ),
        (
            PNG16,
            ImageMime::Png,
            "1e2725064b73e8bdc0c7056ae8bf2d7f17dbcae5a7dac2d5f13fc398076c357d",
        ),
        (
            JPEG,
            ImageMime::Jpeg,
            "a259cc25075434554da937e24a8056b5f9db0608a448c06effac4ba9c950eb46",
        ),
        (
            AVIF,
            ImageMime::Avif,
            "7c2306b2967d74b97c08809e1aa73b4923ab830a71cdecdf7acf2f0ba6873257",
        ),
    ] {
        let verified = inspect(bytes, mime).unwrap();
        assert_eq!(verified.bytes(), bytes);
        assert_eq!(verified.metadata().sha256(), hash);
        assert_eq!(verified.metadata().byte_length(), bytes.len() as u64);
        assert_eq!(verified.metadata().content_type(), mime);
        assert_eq!(verified.metadata().width_px(), 2);
        assert_eq!(verified.metadata().height_px(), 3);
    }
}

#[test]
fn empty_oversize_unsupported_and_spoofed_formats_are_closed_errors() {
    assert_eq!(
        inspect(&[], ImageMime::Png).unwrap_err(),
        ImageIntakeError::EmptyImage
    );
    assert_eq!(
        inspect(&vec![0; MAX_IMAGE_BYTES as usize + 1], ImageMime::Png).unwrap_err(),
        ImageIntakeError::ImageTooLarge,
    );
    for bytes in [
        b"<svg></svg>".as_slice(),
        b"<html></html>",
        b"GIF89a",
        b"RIFFxxxxWEBP",
        b"OggS",
        &[0; 20],
    ] {
        assert_eq!(
            inspect(bytes, ImageMime::Png).unwrap_err(),
            ImageIntakeError::UnsupportedFormat
        );
    }
    assert_eq!(
        inspect(PNG, ImageMime::Jpeg).unwrap_err(),
        ImageIntakeError::FormatMismatch
    );
    assert_eq!(
        inspect(JPEG, ImageMime::Png).unwrap_err(),
        ImageIntakeError::FormatMismatch
    );
    assert_eq!(
        inspect(PNG, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::FormatMismatch
    );
    assert_eq!(
        inspect(AVIF, ImageMime::Png).unwrap_err(),
        ImageIntakeError::FormatMismatch
    );
    assert_eq!(
        inspect(AVIF, ImageMime::Jpeg).unwrap_err(),
        ImageIntakeError::FormatMismatch
    );
}

#[test]
fn exact_encoded_limit_preserves_private_ancillary_metadata() {
    let extra_length = MAX_IMAGE_BYTES as usize - PNG.len() - 12;
    let mut text = vec![b'x'; extra_length];
    text[..5].copy_from_slice(b"note\0");
    let encoded = [
        &PNG[..PNG.len() - 12],
        &chunk(b"tEXt", &text),
        &PNG[PNG.len() - 12..],
    ]
    .concat();
    assert_eq!(encoded.len() as u64, MAX_IMAGE_BYTES);
    let verified = inspect(&encoded, ImageMime::Png).unwrap();
    assert_eq!(verified.bytes(), encoded);
    assert_eq!(verified.metadata().byte_length(), MAX_IMAGE_BYTES);
}

#[test]
fn every_truncated_fixture_and_header_only_input_is_rejected() {
    for (fixture, mime) in [
        (PNG, ImageMime::Png),
        (PNG16, ImageMime::Png),
        (JPEG, ImageMime::Jpeg),
        (AVIF, ImageMime::Avif),
    ] {
        for length in 0..fixture.len() {
            assert!(
                inspect(&fixture[..length], mime).is_err(),
                "accepted prefix {length} of {}",
                fixture.len()
            );
        }
    }
    assert_eq!(
        inspect(&PNG[..33], ImageMime::Png).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
    assert_eq!(
        inspect(&[0xff, 0xd8, 0xff, 0xd9], ImageMime::Jpeg).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
}

#[test]
fn appended_payloads_and_second_images_are_rejected() {
    for (fixture, mime) in [
        (PNG, ImageMime::Png),
        (JPEG, ImageMime::Jpeg),
        (AVIF, ImageMime::Avif),
    ] {
        for suffix in [b"<script>untrusted</script>".as_slice(), fixture] {
            let bytes = [fixture, suffix].concat();
            assert_eq!(
                inspect(&bytes, mime).unwrap_err(),
                ImageIntakeError::MalformedImage
            );
        }
    }
}

// Test-only encoding helpers synthesize hostile envelopes without depending on image's encoder.
fn chunk(kind: &[u8; 4], data: &[u8]) -> Vec<u8> {
    let mut bytes = (data.len() as u32).to_be_bytes().to_vec();
    bytes.extend_from_slice(kind);
    bytes.extend_from_slice(data);
    let mut crc = !0_u32;
    for byte in bytes[4..].iter().copied() {
        crc ^= u32::from(byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & 0_u32.wrapping_sub(crc & 1));
        }
    }
    bytes.extend_from_slice(&(!crc).to_be_bytes());
    bytes
}

fn compressed_zeros(length: usize) -> Vec<u8> {
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    let zeros = [0; 16 * 1024];
    let mut remaining = length;
    while remaining > 0 {
        let count = remaining.min(zeros.len());
        encoder.write_all(&zeros[..count]).unwrap();
        remaining -= count;
    }
    encoder.finish().unwrap()
}

fn png_header(width: u32, height: u32, depth: u8, color: u8) -> Vec<u8> {
    let mut header = width.to_be_bytes().to_vec();
    header.extend_from_slice(&height.to_be_bytes());
    header.extend_from_slice(&[depth, color, 0, 0, 0]);
    [b"\x89PNG\r\n\x1a\n".as_slice(), &chunk(b"IHDR", &header)].concat()
}

fn png_with_header(width: u32, height: u32, depth: u8, color: u8) -> Vec<u8> {
    [
        png_header(width, height, depth, color),
        chunk(b"IDAT", &compressed_zeros(1)),
        chunk(b"IEND", &[]),
    ]
    .concat()
}

#[test]
fn apng_is_rejected_even_when_animation_chunks_follow_pixels() {
    for (kind, payload) in [
        (b"acTL", vec![0, 0, 0, 1, 0, 0, 0, 0]),
        (b"fcTL", vec![0; 26]),
        (b"fdAT", vec![0; 4]),
    ] {
        for offset in [33, PNG.len() - 12] {
            let bytes = [&PNG[..offset], &chunk(kind, &payload), &PNG[offset..]].concat();
            assert_eq!(
                inspect(&bytes, ImageMime::Png).unwrap_err(),
                ImageIntakeError::AnimatedImage
            );
        }
    }
}

#[test]
fn hostile_dimensions_and_real_sixteen_bit_sample_layout_are_bounded_before_decode() {
    for (width, height) in [(4097, 1), (1, 4097), (u32::MAX, u32::MAX)] {
        assert_eq!(
            inspect(&png_with_header(width, height, 8, 6), ImageMime::Png).unwrap_err(),
            ImageIntakeError::ImageLimit
        );
    }
    // Valid dimensions/pixel product would fit RGBA8, but actual RGBA16 output is 128MiB.
    // The deliberately tiny IDAT cannot reach decoding: sample-layout rejection happens first.
    assert_eq!(
        inspect(&png_with_header(4096, 4096, 16, 6), ImageMime::Png).unwrap_err(),
        ImageIntakeError::ImageLimit
    );
    let mut jpeg = JPEG.to_vec();
    let frame = jpeg
        .windows(2)
        .position(|bytes| bytes == [0xff, 0xc0])
        .unwrap();
    jpeg[frame + 5..frame + 9].copy_from_slice(&[0xff; 4]);
    assert_eq!(
        inspect(&jpeg, ImageMime::Jpeg).unwrap_err(),
        ImageIntakeError::ImageLimit
    );
}

#[test]
fn max_rgba8_output_actually_decodes_at_the_exact_bound() {
    let encoded = [
        png_header(4096, 4096, 8, 6),
        chunk(b"IDAT", &compressed_zeros(4096 * (1 + 4096 * 4))),
        chunk(b"IEND", &[]),
    ]
    .concat();
    assert!((encoded.len() as u64) < MAX_IMAGE_BYTES);
    let verified = inspect(&encoded, ImageMime::Png).unwrap();
    assert_eq!(
        (
            verified.metadata().width_px(),
            verified.metadata().height_px()
        ),
        (4096, 4096)
    );
    assert_eq!(verified.bytes(), encoded);
}

#[test]
fn hostile_chunk_lengths_metadata_decompression_and_invalid_pixels_fail_closed() {
    let length_bomb = [b"\x89PNG\r\n\x1a\n".as_slice(), &[0xff; 4], b"iCCP"].concat();
    assert_eq!(
        inspect(&length_bomb, ImageMime::Png).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
    let jpeg_length_bomb = [0xff, 0xd8, 0xff, 0xe1, 0xff, 0xff];
    assert_eq!(
        inspect(&jpeg_length_bomb, ImageMime::Jpeg).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
    // Bounded encoded bytes expand an ICC profile beyond the pre-constructor PNG budget.
    let mut iccp = b"synthetic\0\0".to_vec();
    iccp.extend_from_slice(&compressed_zeros(MAX_DECODED_IMAGE_BYTES as usize + 1));
    let overloaded = [&PNG[..33], &chunk(b"iCCP", &iccp), &PNG[33..]].concat();
    assert!((overloaded.len() as u64) < MAX_IMAGE_BYTES);
    // Characterize upstream permissiveness: it silently drops the over-budget profile,
    // even though its independent decompressor's output budget is enforced.
    let mut permissive = PngDecoder::with_limits(Cursor::new(&overloaded), limits()).unwrap();
    assert!(permissive.icc_profile().unwrap().is_none());
    permissive.read_image(&mut [0; 18]).unwrap();
    assert_eq!(
        inspect(&overloaded, ImageMime::Png).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
    assert!(inspect(&png_with_header(2, 3, 8, 6), ImageMime::Png).is_err());
    let mut bad_crc = PNG.to_vec();
    bad_crc[29] ^= 1;
    assert!(inspect(&bad_crc, ImageMime::Png).is_err());
}

fn icc_payload() -> Vec<u8> {
    let mut profile = vec![0; 132];
    profile[..4].copy_from_slice(&132_u32.to_be_bytes());
    profile[8..12].copy_from_slice(&[4, 0x30, 0, 0]);
    profile[12..16].copy_from_slice(b"mntr");
    profile[16..20].copy_from_slice(b"RGB ");
    profile[20..24].copy_from_slice(b"XYZ ");
    profile[36..40].copy_from_slice(b"acsp");
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::fast());
    encoder.write_all(&profile).unwrap();
    [b"synthetic\0\0".as_slice(), &encoder.finish().unwrap()].concat()
}

#[test]
fn parsed_private_icc_is_retained_but_malformed_duplicate_and_late_profiles_are_rejected() {
    let iccp = chunk(b"iCCP", &icc_payload());
    let valid = [&PNG[..33], &iccp, &PNG[33..]].concat();
    assert_eq!(inspect(&valid, ImageMime::Png).unwrap().bytes(), valid);
    let mut decoder = PngDecoder::with_limits(Cursor::new(&valid), limits()).unwrap();
    assert_eq!(decoder.icc_profile().unwrap().unwrap().len(), 132);
    let malformed = [
        &PNG[..33],
        &chunk(b"iCCP", b"synthetic\0\0malformed zlib"),
        &PNG[33..],
    ]
    .concat();
    let duplicate = [&PNG[..33], &iccp, &iccp, &PNG[33..]].concat();
    let late = [&PNG[..PNG.len() - 12], &iccp, &PNG[PNG.len() - 12..]].concat();
    for bytes in [malformed, duplicate, late] {
        assert_eq!(
            inspect(&bytes, ImageMime::Png).unwrap_err(),
            ImageIntakeError::MalformedImage
        );
    }
}

fn with_exif(jpeg: &[u8], orientation: u16) -> Vec<u8> {
    let mut exif = b"Exif\0\0II\x2a\0\x08\0\0\0\x01\0\x12\x01\x03\0\x01\0\0\0".to_vec();
    exif.extend_from_slice(&orientation.to_le_bytes());
    exif.extend_from_slice(&[0; 6]);
    let mut segment = vec![0xff, 0xe1];
    segment.extend_from_slice(&((exif.len() + 2) as u16).to_be_bytes());
    segment.extend_from_slice(&exif);
    [&jpeg[..2], &segment, &jpeg[2..]].concat()
}

#[test]
fn every_nonidentity_exif_orientation_and_duplicate_exif_rotation_is_rejected() {
    let identity = with_exif(JPEG, 1);
    assert!(inspect(&identity, ImageMime::Jpeg).is_ok());
    for orientation in 2..=8 {
        assert_eq!(
            inspect(&with_exif(JPEG, orientation), ImageMime::Jpeg).unwrap_err(),
            ImageIntakeError::NonIdentityOrientation
        );
        let hidden = with_exif(&with_exif(JPEG, orientation), 1);
        assert_eq!(
            inspect(&hidden, ImageMime::Jpeg).unwrap_err(),
            ImageIntakeError::NonIdentityOrientation
        );
    }
}

#[tokio::test]
async fn two_admissions_refuse_a_third_and_success_keeps_its_body_lane_until_drop() {
    let lanes = lanes();
    let first = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let second = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        ImageAdmission::from_lanes(lanes.clone()).unwrap_err(),
        ImageIntakeError::AdmissionBusy
    );
    let verified = first.inspect(PNG.to_vec(), ImageMime::Png).await.unwrap();
    assert_eq!(lanes.available_permits(), 0);
    drop(second);
    assert_eq!(lanes.available_permits(), 1);
    drop(verified);
    assert_eq!(lanes.available_permits(), 2);
}

#[tokio::test]
async fn unpolled_inspection_future_owns_the_body_and_releases_its_lane_on_drop() {
    let lanes = lanes();
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let future = admission.inspect(PNG.to_vec(), ImageMime::Png);
    assert_eq!(lanes.available_permits(), 1);
    drop(future);
    assert_eq!(lanes.available_permits(), 2);
}

#[tokio::test]
async fn cancellation_after_blocking_start_retains_admission_until_worker_and_body_exit() {
    let lanes = lanes();
    let first = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let _second = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release_tx, release_rx) = std::sync::mpsc::channel();
    let task = tokio::spawn(first.run_blocking(PNG.to_vec(), move |body| {
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        inspect_body(body, ImageMime::Png)
    }));
    started_rx.await.unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert_eq!(lanes.available_permits(), 0);
    assert_eq!(
        ImageAdmission::from_lanes(lanes.clone()).unwrap_err(),
        ImageIntakeError::AdmissionBusy
    );
    release_tx.send(()).unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        while lanes.available_permits() != 1 {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
}

#[tokio::test]
async fn rejected_bytes_and_panicking_blocking_worker_release_admission() {
    let lanes = lanes();
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        admission
            .inspect(vec![1, 2], ImageMime::Png)
            .await
            .unwrap_err(),
        ImageIntakeError::UnsupportedFormat
    );
    assert_eq!(lanes.available_permits(), 2);
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        admission
            .run_blocking(PNG.to_vec(), |_body| panic!("synthetic decoder panic"))
            .await
            .unwrap_err(),
        ImageIntakeError::InspectionUnavailable
    );
    assert_eq!(lanes.available_permits(), 2);
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(128))]
    #[test]
    fn arbitrary_bounded_bytes_never_panic_and_success_has_checked_original_facts(
        bytes in proptest::collection::vec(any::<u8>(), 0..8192),
        png in any::<bool>(),
    ) {
        let mime = if png { ImageMime::Png } else { ImageMime::Jpeg };
        if let Ok(verified) = inspect(&bytes, mime) {
            prop_assert_eq!(verified.bytes(), bytes.as_slice());
            prop_assert_eq!(verified.metadata().byte_length(), bytes.len() as u64);
            prop_assert_eq!(verified.metadata().content_type(), mime);
            prop_assert!(verified.metadata().width_px() > 0 && verified.metadata().width_px() <= MAX_IMAGE_DIMENSION);
            prop_assert!(verified.metadata().height_px() > 0 && verified.metadata().height_px() <= MAX_IMAGE_DIMENSION);
        }
    }
}

#[test]
fn avif_animation_orientation_and_dimension_limits_fail_closed() {
    let mut avis = AVIF.to_vec();
    avis[8..12].copy_from_slice(b"avis");
    assert_eq!(
        inspect(&avis, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::AnimatedImage
    );

    let moov = [&AVIF[..32], b"\0\0\0\x08moov", &AVIF[32..]].concat();
    assert_eq!(
        inspect(&moov, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::AnimatedImage
    );

    // Duplicate the 21-byte OBU_FRAME inside mdat and update iloc/mdat sizes.
    let mut multi_frame = AVIF.to_vec();
    let frame_obu = AVIF[AVIF.len() - 21..].to_vec();
    multi_frame.extend_from_slice(&frame_obu);
    let new_mdat_size = (41 + frame_obu.len()) as u32;
    multi_frame[281..285].copy_from_slice(&new_mdat_size.to_be_bytes());
    let new_iloc_len = (33 + frame_obu.len()) as u32;
    let iloc_pos = multi_frame.windows(4).position(|w| w == b"iloc").unwrap() - 4;
    multi_frame[iloc_pos + 26..iloc_pos + 30].copy_from_slice(&new_iloc_len.to_be_bytes());
    assert_eq!(
        inspect(&multi_frame, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::AnimatedImage
    );

    // Replace colr in ipco with non-identity irot (9 bytes: size=9, 'irot', angle=1).
    let mut rotated = AVIF.to_vec();
    let ispe_pos = rotated.windows(4).position(|w| w == b"ispe").unwrap() - 4;
    let mut over = rotated.clone();
    over[ispe_pos + 12..ispe_pos + 16].copy_from_slice(&4097_u32.to_be_bytes());
    assert_eq!(
        inspect(&over, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::ImageLimit
    );

    // 4096x4096 at 10-bit (high_bitdepth=1 in av1C + pixi=10) exceeds 64 MiB RGBA16 budget.
    let mut high_bit_limit = rotated.clone();
    high_bit_limit[ispe_pos + 12..ispe_pos + 16].copy_from_slice(&4096_u32.to_be_bytes());
    high_bit_limit[ispe_pos + 16..ispe_pos + 20].copy_from_slice(&4096_u32.to_be_bytes());
    let av1c_pos = high_bit_limit
        .windows(4)
        .position(|w| w == b"av1C")
        .unwrap()
        - 4;
    high_bit_limit[av1c_pos + 10] |= 0x40;
    let pixi_pos = high_bit_limit
        .windows(4)
        .position(|w| w == b"pixi")
        .unwrap()
        - 4;
    high_bit_limit[pixi_pos + 13..pixi_pos + 16].copy_from_slice(&[10, 10, 10]);
    assert_eq!(
        inspect(&high_bit_limit, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::ImageLimit
    );

    // Mutate colr box into an irot box with angle=1 while keeping box size 19 -> MalformedImage,
    // or overwrite the first 9 bytes of colr (and shrink meta/iprp/ipco/iloc) to test irot(1).
    let colr_pos = rotated.windows(4).position(|w| w == b"colr").unwrap() - 4;
    rotated[colr_pos + 4..colr_pos + 8].copy_from_slice(b"imir");
    assert_eq!(
        inspect(&rotated, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::NonIdentityOrientation
    );
}

#[test]
fn deterministic_mutations_reach_real_decoders_without_panicking() {
    for (fixture, mime) in [
        (PNG, ImageMime::Png),
        (PNG16, ImageMime::Png),
        (JPEG, ImageMime::Jpeg),
        (AVIF, ImageMime::Avif),
    ] {
        for index in 0..fixture.len() {
            let mut bytes = fixture.to_vec();
            bytes[index] ^= 0xff;
            let result = inspect(&bytes, mime);
            if let Ok(verified) = result {
                // Permissive decoding is decodability, not complete format conformance.
                assert_eq!(verified.bytes(), bytes);
                assert_eq!(
                    verified.metadata().sha256(),
                    format!("{:x}", Sha256::digest(&bytes))
                );
            }
        }
    }
}
