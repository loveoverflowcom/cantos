use super::super::domain::image::MAX_IMAGE_DIMENSION;
use super::*;
use proptest::prelude::*;
use std::{sync::Arc, time::Duration};
const AVIF: &[u8] = include_bytes!("fixtures/ffmpeg-yuv420p-2x3.avif");
const CONVERTED: [&[u8]; 3] = [
    include_bytes!("fixtures/python-rgb-2x3.avif"),
    include_bytes!("fixtures/python-rgba16-2x3.avif"),
    include_bytes!("fixtures/pillow-rgb-2x3.avif"),
];
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
fn independent_and_converted_avif_fixtures_decode_pixels_and_preserve_original_identity() {
    let records: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/conversions.json")).unwrap();
    for (bytes, record) in CONVERTED.into_iter().zip(records.as_array().unwrap()) {
        let verified = inspect(bytes, ImageMime::Avif).unwrap();
        assert_eq!(verified.bytes(), bytes);
        assert_eq!(
            verified.metadata().sha256(),
            record["sha256"].as_str().unwrap()
        );
        assert_eq!(verified.metadata().byte_length(), bytes.len() as u64);
        assert_eq!(
            (
                verified.metadata().width_px(),
                verified.metadata().height_px()
            ),
            (2, 3)
        );
    }
    assert_eq!(
        inspect(AVIF, ImageMime::Avif).unwrap().metadata().sha256(),
        "7c2306b2967d74b97c08809e1aa73b4923ab830a71cdecdf7acf2f0ba6873257"
    );
}
#[test]
fn legacy_actual_bytes_are_rejection_vectors_never_positive_admission() {
    let vectors: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/legacy-rejection.json")).unwrap();
    for (name, value) in vectors.as_object().unwrap() {
        let hex = value.as_str().unwrap();
        let bytes: Vec<u8> = (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).unwrap())
            .collect();
        let mime = if name.ends_with("jpg") {
            ImageMime::Jpeg
        } else {
            ImageMime::Png
        };
        assert_eq!(
            inspect(&bytes, mime).unwrap_err(),
            ImageIntakeError::UnsupportedFormat
        );
        assert_eq!(
            inspect(&bytes, ImageMime::Avif).unwrap_err(),
            ImageIntakeError::FormatMismatch
        );
    }
    for mime in [ImageMime::Png, ImageMime::Jpeg] {
        assert_eq!(
            inspect(AVIF, mime).unwrap_err(),
            ImageIntakeError::FormatMismatch
        );
    }
}
#[test]
fn empty_oversized_truncated_and_appended_avif_fail_closed() {
    assert_eq!(
        inspect(&[], ImageMime::Avif).unwrap_err(),
        ImageIntakeError::EmptyImage
    );
    assert_eq!(
        inspect(&vec![0; MAX_IMAGE_BYTES as usize + 1], ImageMime::Avif).unwrap_err(),
        ImageIntakeError::ImageTooLarge
    );
    for fixture in [AVIF, CONVERTED[0], CONVERTED[1], CONVERTED[2]] {
        for length in 1..fixture.len() {
            assert!(
                inspect(&fixture[..length], ImageMime::Avif).is_err(),
                "accepted truncated {length}"
            );
        }
        for suffix in [b"untrusted".as_slice(), fixture] {
            assert_eq!(
                inspect(&[fixture, suffix].concat(), ImageMime::Avif).unwrap_err(),
                ImageIntakeError::MalformedImage
            );
        }
    }
}
#[test]
fn corrupt_entropy_tiles_are_rejected_even_with_valid_container_headers() {
    let mut bytes = AVIF.to_vec();
    let end = bytes.len();
    bytes[end - 8..].fill(0xff);
    assert_eq!(
        inspect(&bytes, ImageMime::Avif).unwrap_err(),
        ImageIntakeError::MalformedImage
    );
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
    let verified = first.inspect(AVIF.to_vec(), ImageMime::Avif).await.unwrap();
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
    let future = admission.inspect(AVIF.to_vec(), ImageMime::Avif);
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
    let task = tokio::spawn(first.run_blocking(AVIF.to_vec(), move |body| {
        started_tx.send(()).unwrap();
        release_rx.recv_timeout(Duration::from_secs(5)).unwrap();
        inspect_body(body, ImageMime::Avif)
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
            .inspect(vec![1, 2], ImageMime::Avif)
            .await
            .unwrap_err(),
        ImageIntakeError::UnsupportedFormat
    );
    assert_eq!(lanes.available_permits(), 2);
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    assert_eq!(
        admission
            .run_blocking(AVIF.to_vec(), |_body| panic!("synthetic decoder panic"))
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
    ) {
        let mime = ImageMime::Avif;
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
    for (fixture, mime) in [(AVIF, ImageMime::Avif)] {
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

const ALPHA: &[u8] = include_bytes!("fixtures/pillow-alpha-64x48.avif");
const GRID: &[u8] = include_bytes!("fixtures/grid-2x2-120x100.avif");
const GRID_ALPHA: &[u8] = include_bytes!("fixtures/grid-alpha-2x2-120x100.avif");
const ALPHA_GRID_FIXTURES: [(&[u8], (u32, u32)); 3] = [
    (ALPHA, (64, 48)),
    (GRID, (120, 100)),
    (GRID_ALPHA, (120, 100)),
];

/// Replaces the `occurrence`th (0-based) match of `needle` with the same-length `with`,
/// so every box size and `iloc` offset of the fixture stays valid.
fn patched(fixture: &[u8], needle: &[u8], occurrence: usize, with: &[u8]) -> Vec<u8> {
    assert_eq!(needle.len(), with.len());
    let at = fixture
        .windows(needle.len())
        .enumerate()
        .filter(|(_, window)| *window == needle)
        .nth(occurrence)
        .map(|(index, _)| index)
        .expect("fixture contains the patched bytes");
    let mut bytes = fixture.to_vec();
    bytes[at..at + needle.len()].copy_from_slice(with);
    bytes
}

fn rejection(bytes: &[u8]) -> ImageIntakeError {
    inspect(bytes, ImageMime::Avif).expect_err("profile violation must be refused")
}

#[test]
fn alpha_and_grid_fixtures_are_admitted_after_every_plane_decodes() {
    for (bytes, (width, height)) in ALPHA_GRID_FIXTURES {
        let verified = inspect(bytes, ImageMime::Avif).unwrap();
        assert_eq!(verified.bytes(), bytes);
        assert_eq!(
            (
                verified.metadata().width_px(),
                verified.metadata().height_px()
            ),
            (width, height)
        );
        assert_eq!(
            verified.metadata().sha256(),
            format!("{:x}", Sha256::digest(bytes))
        );
    }
}

#[test]
fn truncated_appended_and_metadata_mutated_alpha_grid_inputs_fail_closed() {
    for (fixture, _) in ALPHA_GRID_FIXTURES {
        for length in 1..fixture.len() {
            assert!(
                inspect(&fixture[..length], ImageMime::Avif).is_err(),
                "accepted truncated {length}"
            );
        }
        assert_eq!(
            rejection(&[fixture, b"untrusted"].concat()),
            ImageIntakeError::MalformedImage
        );
        // Flip each container byte: a mutation either stays a still image or is refused;
        // none panics, and an admitted body is still the exact input.
        let mdat = fixture
            .windows(4)
            .position(|window| window == b"mdat")
            .unwrap();
        for index in 0..mdat {
            let mut bytes = fixture.to_vec();
            bytes[index] ^= 0xff;
            if let Ok(verified) = inspect(&bytes, ImageMime::Avif) {
                assert_eq!(verified.bytes(), bytes);
            }
        }
    }
}

#[test]
fn corrupt_alpha_or_tile_entropy_is_rejected_even_when_the_container_is_valid() {
    for (fixture, _) in [(ALPHA, ()), (GRID, ()), (GRID_ALPHA, ())] {
        // The final extent of every fixture is a grid descriptor or the alpha tile payload.
        let mut bytes = fixture.to_vec();
        let end = bytes.len();
        bytes[end - 8..].fill(0xff);
        assert!(inspect(&bytes, ImageMime::Avif).is_err());
    }
    // Corrupt only the alpha payload of the single-item file: 12 bytes before the file end.
    let mut single = ALPHA.to_vec();
    let end = single.len();
    single[end - 12..].fill(0xff);
    assert_eq!(rejection(&single), ImageIntakeError::MalformedImage);
}

#[test]
fn only_the_mpeg_alpha_auxiliary_link_is_admitted() {
    for (fixture, _) in [(ALPHA, ()), (GRID_ALPHA, ())] {
        // Premultiplied, thumbnail and any other reference type needs its own profile.
        for other in [b"prem", b"thmb", b"cdsc", b"base"] {
            assert_eq!(
                rejection(&patched(fixture, b"auxl", 0, other)),
                ImageIntakeError::UnsupportedFormat
            );
        }
        // A different auxiliary type (depth, HEVC alpha URN) is not the alpha profile.
        let not_alpha = patched(fixture, b"auxiliary:alpha", 0, b"auxiliary:alphb");
        assert_eq!(rejection(&not_alpha), ImageIntakeError::UnsupportedFormat);
    }
    // An alpha-typed item with no `auxl` link is an unaccounted extra picture.
    let unlinked = patched(ALPHA, b"auxl", 0, b"dimg");
    assert!(matches!(
        rejection(&unlinked),
        ImageIntakeError::MalformedImage | ImageIntakeError::UnsupportedFormat
    ));
}

#[test]
fn alpha_plane_must_be_a_monochrome_image_of_the_same_size() {
    // Alpha tile 5 reuses the color `av1C` (not monochrome) while still holding a 1-channel picture.
    let color_config = patched(GRID_ALPHA, &[0, 5, 3, 1, 6, 7], 0, &[0, 5, 3, 1, 6, 3]);
    assert_eq!(rejection(&color_config), ImageIntakeError::MalformedImage);
    // The alpha grid claims the 64x64 tile `ispe` instead of the color output size.
    let size = patched(GRID_ALPHA, &[0, 10, 2, 5, 8], 0, &[0, 10, 2, 1, 8]);
    assert_eq!(rejection(&size), ImageIntakeError::MalformedImage);
}

#[test]
fn grid_layout_violations_fail_closed() {
    // The final descriptor of GRID is `version flags rows-1 columns-1 width height`.
    let descriptor = |rows: u8, columns: u8, flags: u8, width: u16| {
        let mut bytes = vec![0, flags, rows, columns];
        bytes.extend_from_slice(&width.to_be_bytes());
        bytes.extend_from_slice(&100_u16.to_be_bytes());
        bytes
    };
    let with_descriptor = |bytes: Vec<u8>| {
        let mut file = GRID.to_vec();
        let end = file.len();
        file[end - 8..].copy_from_slice(&bytes);
        file
    };
    // 3 x 2 grid but 4 referenced tiles; unknown version/flags; zero and oversized output.
    assert_eq!(
        rejection(&with_descriptor(descriptor(2, 1, 0, 120))),
        ImageIntakeError::MalformedImage
    );
    assert_eq!(
        rejection(&with_descriptor(descriptor(1, 1, 2, 120))),
        ImageIntakeError::MalformedImage
    );
    assert_eq!(
        rejection(&with_descriptor(descriptor(1, 1, 0, 0))),
        ImageIntakeError::MalformedImage
    );
    assert_eq!(
        rejection(&with_descriptor(descriptor(1, 1, 0, 4097))),
        ImageIntakeError::ImageLimit
    );
    // More tiles than the profile bound (9 x 8 = 72 > 64) before any tile is decoded.
    assert_eq!(
        rejection(&with_descriptor(descriptor(8, 7, 0, 120))),
        ImageIntakeError::ImageLimit
    );
    let mut version = descriptor(1, 1, 0, 120);
    version[0] = 1;
    assert_eq!(
        rejection(&with_descriptor(version)),
        ImageIntakeError::MalformedImage
    );
    // Output wider than the tiles cover (2 x 64 = 128 < 200).
    let wider = patched(
        &with_descriptor(descriptor(1, 1, 0, 200)),
        &[0, 0, 0, 120, 0, 0, 0, 100],
        0,
        &[0, 0, 0, 200, 0, 0, 0, 100],
    );
    assert_eq!(rejection(&wider), ImageIntakeError::MalformedImage);
    // A whole surplus tile column: output 60 wide needs one column of 64, not two.
    let narrower = patched(
        &with_descriptor(descriptor(1, 1, 0, 60)),
        &[0, 0, 0, 120, 0, 0, 0, 100],
        0,
        &[0, 0, 0, 60, 0, 0, 0, 100],
    );
    assert_eq!(rejection(&narrower), ImageIntakeError::MalformedImage);
    // The grid item's `ispe` must equal the descriptor output.
    let ispe = patched(
        GRID,
        &[0, 0, 0, 120, 0, 0, 0, 100],
        0,
        &[0, 0, 0, 120, 0, 0, 0, 99],
    );
    assert_eq!(rejection(&ispe), ImageIntakeError::MalformedImage);
}

#[test]
fn grid_tile_graph_violations_fail_closed() {
    // dimg 5 -> (1, 2, 3, 3): a repeated tile (and an unreferenced one).
    let repeated = patched(
        GRID,
        &[0, 1, 0, 2, 0, 3, 0, 4],
        0,
        &[0, 1, 0, 2, 0, 3, 0, 3],
    );
    assert_eq!(rejection(&repeated), ImageIntakeError::UnsupportedFormat);
    // A tile referencing the grid itself (nested grid) is not decodable here.
    let nested = patched(
        GRID,
        &[0, 1, 0, 2, 0, 3, 0, 4],
        0,
        &[0, 1, 0, 2, 0, 3, 0, 5],
    );
    assert_eq!(rejection(&nested), ImageIntakeError::UnsupportedFormat);
    // A tile with different dimensions than its siblings.
    let sized = patched(GRID, &[0, 4, 3, 1, 2, 3], 0, &[0, 4, 3, 5, 2, 3]);
    assert_eq!(rejection(&sized), ImageIntakeError::MalformedImage);
    // An unsupported extra picture kind among the items (here a tile item turned into a tone map).
    let extra = patched(
        GRID,
        &[0, 4, 0, 0, b'a', b'v', b'0', b'1'],
        0,
        &[0, 4, 0, 0, b't', b'm', b'a', b'p'],
    );
    assert_eq!(rejection(&extra), ImageIntakeError::UnsupportedFormat);
    // The grid item typed as a plain `av01` while still owning `dimg` tiles.
    let typed = patched(
        GRID,
        &[0, 5, 0, 0, b'g', b'r', b'i', b'd'],
        0,
        &[0, 5, 0, 0, b'a', b'v', b'0', b'1'],
    );
    assert_eq!(rejection(&typed), ImageIntakeError::MalformedImage);
}

#[tokio::test]
async fn grid_admission_keeps_its_lane_until_the_verified_body_drops() {
    let lanes = lanes();
    let admission = ImageAdmission::from_lanes(lanes.clone()).unwrap();
    let verified = admission
        .inspect(GRID_ALPHA.to_vec(), ImageMime::Avif)
        .await
        .unwrap();
    assert_eq!(lanes.available_permits(), 1);
    drop(verified);
    assert_eq!(lanes.available_permits(), 2);
}
