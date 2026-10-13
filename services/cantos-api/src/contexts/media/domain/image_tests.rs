use proptest::prelude::*;

use super::*;

const WORK: &str = "00000000-0000-0000-0000-000000000001";
const OPERATION: &str = "00000000-0000-0000-0000-000000000002";
const INTENT: &str = "00000000-0000-0000-0000-000000000003";

fn request() -> dto::CreateImageIntentRequest {
    dto::CreateImageIntentRequest {
        operation_id: OPERATION.into(),
        id: INTENT.into(),
        expected_sha256: "a".repeat(64),
        expected_byte_length: 67,
        content_type: dto::ImageMime::Png,
        declarations: dto::ImageDeclarations {
            provenance: "Prepared cover".into(),
            rights_refs: vec!["rights-z".into(), "rights-a".into()],
        },
    }
}
fn intent() -> ImageIntent {
    ImageIntent::create(
        PrepareImage::admit(&request()).unwrap(),
        &WorkId::parse(WORK).unwrap(),
        &OwnerId::parse("owner").unwrap(),
        100,
    )
    .unwrap()
}
fn metadata() -> ImageMetadata {
    ImageMetadata::new("a".repeat(64), 67, ImageMime::Png, 2, 3).unwrap()
}

#[test]
fn numeric_boundaries_have_exact_outcomes_and_layout_sizes() {
    for (length, expected) in [
        (0, Err(ImageError::EmptyImage)),
        (1, Ok(1)),
        (MAX_IMAGE_BYTES, Ok(1)),
        (MAX_IMAGE_BYTES + 1, Err(ImageError::ImageTooLarge)),
        (u64::MAX, Err(ImageError::ImageTooLarge)),
    ] {
        assert_eq!(
            check_image_bounds(length, 1, 1, 1).map(ImageBounds::decoded_byte_length),
            expected
        );
    }
    for dimension in [0, 4097, u32::MAX] {
        assert_eq!(
            check_image_bounds(1, dimension, 1, 1),
            Err(ImageError::InvalidDimensions)
        );
        assert_eq!(
            check_image_bounds(1, 1, dimension, 1),
            Err(ImageError::InvalidDimensions)
        );
    }
    for layout in [1, 2, 3, 4, 6, 8] {
        let bounds = check_image_bounds(1, 1, 4096, layout).unwrap();
        assert_eq!(bounds.pixel_count(), 4096);
        assert_eq!(bounds.decoded_byte_length(), 4096 * usize::from(layout));
    }
    assert_eq!(
        check_image_bounds(MAX_IMAGE_BYTES, 4096, 4096, 4)
            .unwrap()
            .decoded_byte_length(),
        MAX_DECODED_IMAGE_BYTES as usize
    );
    for layout in [6, 8] {
        assert_eq!(
            check_image_bounds(1, 4096, 4096, layout),
            Err(ImageError::DecodedImageTooLarge)
        );
    }
    for layout in 0..=u8::MAX {
        if ![1, 2, 3, 4, 6, 8].contains(&layout) {
            assert_eq!(
                check_image_bounds(1, 1, 1, layout),
                Err(ImageError::UnsupportedLayout)
            );
        }
    }
    assert_eq!(
        check_image_bounds(1, 2048, 4096, 8)
            .unwrap()
            .decoded_byte_length(),
        MAX_DECODED_IMAGE_BYTES as usize
    );
    assert_eq!(
        check_image_bounds(1, 2049, 4096, 8),
        Err(ImageError::DecodedImageTooLarge)
    );
}

#[test]
fn closed_profiles_and_canonical_ids_reject_alias_spellings() {
    for invalid in [
        "",
        "image/jpg",
        "IMAGE/PNG",
        "image/png; charset=utf-8",
        "image/svg+xml",
    ] {
        assert_eq!(ImageMime::parse(invalid), Err(ImageError::UnsupportedMime));
    }
    assert_eq!(ImageMime::parse("image/png"), Ok(ImageMime::Png));
    assert_eq!(ImageMime::parse("image/jpeg"), Ok(ImageMime::Jpeg));
    assert_eq!(
        ImageProfile::parse("cover-image-v2"),
        Err(ImageError::UnsupportedProfile)
    );
    for invalid in [
        "",
        "00000000000000000000000000000003",
        "{00000000-0000-0000-0000-000000000003}",
        "00000000-0000-0000-0000-00000000000A",
    ] {
        assert_eq!(
            ImageIntentId::parse(invalid),
            Err(ImageError::InvalidIntentId)
        );
        assert_eq!(
            ImageAssetId::parse(invalid),
            Err(ImageError::InvalidAssetId)
        );
        assert_eq!(
            ImageOperationId::parse(invalid),
            Err(ImageError::InvalidOperationId)
        );
    }
}

#[test]
fn expected_metadata_accepts_only_canonical_hash_and_bounded_encoded_size() {
    for hash in ["".into(), "A".repeat(64), "a".repeat(63), "g".repeat(64)] {
        assert_eq!(
            ExpectedImage::new(hash, 1, ImageMime::Png),
            Err(ImageError::InvalidHash)
        );
    }
    assert_eq!(
        ExpectedImage::new("a".repeat(64), 0, ImageMime::Png),
        Err(ImageError::EmptyImage)
    );
    assert_eq!(
        ImageMetadata::new("a".repeat(64), 1, ImageMime::Png, 0, 1),
        Err(ImageError::InvalidDimensions)
    );
    let expected = ExpectedImage::new("a".repeat(64), 67, ImageMime::Png).unwrap();
    assert_eq!(expected.matches(&metadata()), Ok(()));
    assert_eq!(
        expected.matches(&ImageMetadata::new("b".repeat(64), 67, ImageMime::Png, 2, 3).unwrap()),
        Err(ImageError::ExpectedHashMismatch)
    );
    assert_eq!(
        expected.matches(&ImageMetadata::new("a".repeat(64), 68, ImageMime::Png, 2, 3).unwrap()),
        Err(ImageError::ExpectedLengthMismatch)
    );
    assert_eq!(
        expected.matches(&ImageMetadata::new("a".repeat(64), 67, ImageMime::Jpeg, 2, 3).unwrap()),
        Err(ImageError::ExpectedMimeMismatch)
    );
}

#[test]
fn declarations_bound_utf8_and_controls_without_normalizing_owner_text() {
    for provenance in ["", " \n\t"] {
        assert_eq!(
            ImageDeclarations::new(provenance.into(), vec!["rights-a".into()]),
            Err(ImageError::BlankProvenance)
        );
    }
    let valid = "é".repeat(2048);
    assert_eq!(
        ImageDeclarations::new(valid.clone(), vec!["rights-a".into()])
            .unwrap()
            .provenance(),
        valid
    );
    assert_eq!(
        ImageDeclarations::new(format!("{valid}a"), vec!["rights-a".into()]),
        Err(ImageError::ProvenanceTooLong)
    );
    for control in ['\0', '\r', '\u{7f}', '\u{85}'] {
        assert_eq!(
            ImageDeclarations::new(format!("cover{control}"), vec!["rights-a".into()]),
            Err(ImageError::ControlCharacter)
        );
    }
    let text = "Ảnh bìa\nTác giả\tCantos";
    assert_eq!(
        ImageDeclarations::new(text.into(), vec!["rights-a".into()])
            .unwrap()
            .provenance(),
        text
    );
}

#[test]
fn rights_are_a_nonempty_distinct_bounded_existing_token_set() {
    assert_eq!(
        ImageDeclarations::new("cover".into(), vec![]),
        Err(ImageError::MissingRightsRefs)
    );
    for invalid in ["", "RIGHTS", "rights a", "é", &"a".repeat(65)] {
        assert_eq!(
            ImageDeclarations::new("cover".into(), vec![invalid.into()]),
            Err(ImageError::InvalidRightsRef)
        );
    }
    assert_eq!(
        ImageDeclarations::new("cover".into(), vec!["a".into(), "a".into()]),
        Err(ImageError::DuplicateRightsRef)
    );
    assert_eq!(
        ImageDeclarations::new("cover".into(), (0..65).map(|n| format!("r-{n}")).collect()),
        Err(ImageError::TooManyRightsRefs)
    );
    let admitted = ImageDeclarations::new(
        "cover".into(),
        (0..64).rev().map(|n| format!("r-{n:02}")).collect(),
    )
    .unwrap();
    assert_eq!(admitted.rights_refs().first().unwrap(), "r-00");
    assert_eq!(admitted.rights_refs().last().unwrap(), "r-63");
}

#[test]
fn new_rights_order_is_canonical_but_restore_never_repairs_it() {
    let first = PrepareImage::admit(&request()).unwrap();
    let mut reordered = request();
    reordered.declarations.rights_refs.reverse();
    let second = PrepareImage::admit(&reordered).unwrap();
    assert_eq!(first, second);
    assert_eq!(
        first.fingerprint(&WorkId::parse(WORK).unwrap()),
        second.fingerprint(&WorkId::parse(WORK).unwrap())
    );
    let mut stored = intent().document().clone();
    stored.request.declarations.rights_refs.reverse();
    assert_eq!(
        ImageIntent::restore(stored),
        Err(ImageError::NoncanonicalRightsRefs)
    );
}

#[test]
fn intent_and_asset_digests_match_independent_ordered_python_literals() {
    // Python 3 hashlib, literal ordered JSON per plan §7, not the production encoder.
    let intent = intent();
    assert_eq!(
        intent.document().digest,
        "ii-r1:sha256:c978993c7c06e67a8f1a818c80c2d22c2b9b6fb2f058eaa9879773b05b87a59a"
    );
    assert_eq!(
        intent.request().fingerprint(intent.work_id()).unwrap(),
        "image-operation1:sha256:a89ca0283d4c21f633a1f55b283a36158f94e36d87510f70bfda781b02ad1de9"
    );
    let asset = ImageAsset::create(&intent, metadata(), 101).unwrap();
    assert_eq!(
        asset.digest(),
        "ia-r1:sha256:4200f444b5e0006f4d0a04b7388d5b1e91b9ae03b2f38256130f5e8b2dad7f63"
    );
    assert_eq!(
        ImageIntent::restore(intent.document().clone()).unwrap(),
        intent
    );
    assert_eq!(
        ImageAsset::restore(asset.document().clone()).unwrap(),
        asset
    );
}

#[test]
fn intent_window_rejects_machine_edges_and_expires_at_equality() {
    for (now, expected) in [
        (-1, ImageError::InvalidTimestamp),
        (
            i64::MAX - IMAGE_INTENT_LIFETIME_MS + 1,
            ImageError::InvalidExpiry,
        ),
        (i64::MAX, ImageError::InvalidExpiry),
    ] {
        assert_eq!(
            ImageIntent::create(
                PrepareImage::admit(&request()).unwrap(),
                &WorkId::parse(WORK).unwrap(),
                &OwnerId::parse("owner").unwrap(),
                now
            ),
            Err(expected)
        );
    }
    let intent = intent();
    assert_eq!(
        intent.require_pending(-1),
        Err(ImageError::InvalidTimestamp)
    );
    assert_eq!(
        intent.require_pending(99),
        Err(ImageError::IntentNotYetCreated)
    );
    assert_eq!(intent.require_pending(100), Ok(()));
    assert_eq!(intent.require_pending(1_800_099), Ok(()));
    assert_eq!(
        intent.require_pending(1_800_100),
        Err(ImageError::IntentExpired)
    );
    assert_eq!(intent.state(None, 100), Ok(dto::ImageIntentState::Pending));
    assert_eq!(
        intent.state(None, 1_800_100),
        Ok(dto::ImageIntentState::Expired)
    );
    let maximal = ImageIntent::create(
        PrepareImage::admit(&request()).unwrap(),
        &WorkId::parse(WORK).unwrap(),
        &OwnerId::parse("owner").unwrap(),
        i64::MAX - IMAGE_INTENT_LIFETIME_MS,
    )
    .unwrap();
    assert_eq!(maximal.expires_at_ms(), i64::MAX);
}

#[test]
fn stored_intents_refuse_forged_ids_times_schemes_and_business_fields() {
    let original = intent().document().clone();
    let cases: Vec<(dto::ImageIntentDocument, ImageError)> = vec![
        (
            dto::ImageIntentDocument {
                schema_version: "2".into(),
                ..original.clone()
            },
            ImageError::UnsupportedSchema,
        ),
        (
            dto::ImageIntentDocument {
                id: OPERATION.into(),
                ..original.clone()
            },
            ImageError::BindingMismatch,
        ),
        (
            dto::ImageIntentDocument {
                operation_id: INTENT.into(),
                ..original.clone()
            },
            ImageError::BindingMismatch,
        ),
        (
            dto::ImageIntentDocument {
                work_id: "bad".into(),
                ..original.clone()
            },
            ImageError::InvalidWorkId,
        ),
        (
            dto::ImageIntentDocument {
                owner_id: "OWNER".into(),
                ..original.clone()
            },
            ImageError::InvalidOwnerId,
        ),
        (
            dto::ImageIntentDocument {
                profile_id: "cover-image-v2".into(),
                ..original.clone()
            },
            ImageError::UnsupportedProfile,
        ),
        (
            dto::ImageIntentDocument {
                created_at_ms: u64::MAX,
                ..original.clone()
            },
            ImageError::InvalidTimestamp,
        ),
        (
            dto::ImageIntentDocument {
                expires_at_ms: 101,
                ..original.clone()
            },
            ImageError::InvalidExpiry,
        ),
        (
            dto::ImageIntentDocument {
                digest: format!("ia-r1:sha256:{}", "a".repeat(64)),
                ..original.clone()
            },
            ImageError::InvalidDigest,
        ),
        (
            dto::ImageIntentDocument {
                work_id: OPERATION.into(),
                ..original.clone()
            },
            ImageError::InvalidDigest,
        ),
    ];
    for (stored, expected) in cases {
        assert_eq!(ImageIntent::restore(stored), Err(expected));
    }
    assert_eq!(original, intent().document().clone());
}

#[test]
fn admission_replays_exact_receipt_after_expiry_and_refuses_metadata_changes() {
    let intent = intent();
    let original = ImageAsset::create(&intent, metadata(), 101).unwrap();
    assert_eq!(
        decide_image_admission(&intent, &metadata(), None, 100),
        Ok(ImageAdmissionDecision::Create)
    );
    assert_eq!(
        decide_image_admission(&intent, &metadata(), None, intent.expires_at_ms()),
        Err(ImageError::IntentExpired)
    );
    assert_eq!(
        decide_image_admission(&intent, &metadata(), Some(&original), i64::MAX),
        Ok(ImageAdmissionDecision::Replay)
    );
    assert_eq!(
        intent.state(Some(&original), i64::MAX),
        Ok(dto::ImageIntentState::Admitted)
    );
    assert_eq!(
        decide_image_admission(
            &intent,
            &ImageMetadata::new("a".repeat(64), 67, ImageMime::Png, 3, 2).unwrap(),
            Some(&original),
            102,
        ),
        Err(ImageError::BindingMismatch)
    );
    assert_eq!(original.admitted_at_ms(), 101);
    for now in [-1, 99, 100] {
        assert_eq!(
            decide_image_admission(&intent, &metadata(), Some(&original), now),
            Err(ImageError::InvalidTimestamp)
        );
        assert_eq!(
            intent.state(Some(&original), now),
            Err(ImageError::InvalidTimestamp)
        );
    }
}

#[test]
fn assets_restore_exact_receipts_and_refuse_foreign_or_invalid_intent_binding() {
    let intent = intent();
    let original = ImageAsset::create(&intent, metadata(), 101).unwrap();
    let raw = original.document().clone();
    assert_eq!(
        ImageAsset::restore(dto::ImageAssetDocument {
            id: OPERATION.into(),
            ..raw.clone()
        }),
        Err(ImageError::BindingMismatch)
    );
    assert_eq!(
        ImageAsset::restore(dto::ImageAssetDocument {
            admitted_at_ms: u64::MAX,
            ..raw.clone()
        }),
        Err(ImageError::InvalidTimestamp)
    );
    assert_eq!(
        ImageAsset::restore(dto::ImageAssetDocument {
            owner_id: "other-owner".into(),
            ..raw.clone()
        }),
        Err(ImageError::InvalidDigest)
    );
    let foreign = ImageIntent::create(
        PrepareImage::admit(&request()).unwrap(),
        &WorkId::parse(OPERATION).unwrap(),
        &OwnerId::parse("owner").unwrap(),
        100,
    )
    .unwrap();
    let foreign_asset = ImageAsset::create(&foreign, metadata(), 101).unwrap();
    assert_eq!(
        foreign_asset.require_intent(&intent),
        Err(ImageError::BindingMismatch)
    );
    assert_eq!(
        ImageAsset::create(&intent, metadata(), 99),
        Err(ImageError::IntentNotYetCreated)
    );
    assert_eq!(
        ImageAsset::create(&intent, metadata(), intent.expires_at_ms()),
        Err(ImageError::IntentExpired)
    );
    assert_eq!(original.document(), &raw);
}

#[test]
fn prepare_receipt_replay_is_distinct_from_duplicate_known_id() {
    let intent = intent();
    let request = intent.request();
    let fingerprint = request.fingerprint(intent.work_id()).unwrap();
    assert_eq!(
        decide_image_prepare(request, intent.work_id(), Some(&fingerprint), Some(&intent)),
        Ok(ImagePrepareDecision::Replay)
    );
    assert_eq!(
        decide_image_prepare(request, intent.work_id(), None, Some(&intent)),
        Err(ImageError::DuplicateIntent)
    );
    assert_eq!(
        decide_image_prepare(
            request,
            intent.work_id(),
            Some("set-artwork-fingerprint"),
            None
        ),
        Err(ImageError::OperationReused)
    );
    assert_eq!(
        decide_image_prepare(request, intent.work_id(), None, None),
        Ok(ImagePrepareDecision::Create)
    );
}

#[test]
fn json_and_protobuf_normalize_to_the_same_complete_prepare_operation() {
    use dto::wire::{decode_artwork, encode_artwork, WireFormat};
    let work = WorkId::parse(WORK).unwrap();
    let original = PrepareImage::admit(&request()).unwrap();
    let fingerprint = original.fingerprint(&work).unwrap();
    for (first, second) in [
        (WireFormat::Json, WireFormat::Protobuf),
        (WireFormat::Protobuf, WireFormat::Json),
    ] {
        let bytes = encode_artwork(first, &request()).unwrap();
        let decoded: dto::CreateImageIntentRequest = decode_artwork(first, &bytes).unwrap();
        let bytes = encode_artwork(second, &decoded).unwrap();
        let mut decoded: dto::CreateImageIntentRequest = decode_artwork(second, &bytes).unwrap();
        decoded.declarations.rights_refs.reverse();
        let admitted = PrepareImage::admit(&decoded).unwrap();
        assert_eq!(admitted, original);
        assert_eq!(admitted.fingerprint(&work).unwrap(), fingerprint);
        assert_eq!(
            decide_image_prepare(&admitted, &work, Some(&fingerprint), None),
            Ok(ImagePrepareDecision::Replay)
        );
    }
}

fn independent_numeric_oracle(
    length: u64,
    width: u32,
    height: u32,
    layout: u8,
) -> Result<(u128, u128), ImageError> {
    if length == 0 {
        Err(ImageError::EmptyImage)
    } else if length > 8_388_608 {
        Err(ImageError::ImageTooLarge)
    } else if width == 0 || height == 0 || width > 4096 || height > 4096 {
        Err(ImageError::InvalidDimensions)
    } else if ![1, 2, 3, 4, 6, 8].contains(&layout) {
        Err(ImageError::UnsupportedLayout)
    } else {
        let pixels = u128::from(width) * u128::from(height);
        let bytes = pixels * u128::from(layout);
        if bytes > 67_108_864 {
            Err(ImageError::DecodedImageTooLarge)
        } else if bytes > usize::MAX as u128 {
            Err(ImageError::OutputSizeOverflow)
        } else {
            Ok((pixels, bytes))
        }
    }
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_013), ..ProptestConfig::default() })]
    #[test]
    fn full_width_raw_bounds_agree_with_independent_u128_specification(
        length in any::<u64>(), width in any::<u32>(), height in any::<u32>(), layout in any::<u8>()
    ) {
        prop_assert_eq!(
            check_image_bounds(length,width,height,layout).map(|b| (u128::from(b.pixel_count()), b.decoded_byte_length() as u128)),
            independent_numeric_oracle(length,width,height,layout)
        );
    }
    #[test]
    fn admitted_and_near_boundary_bounds_agree_with_independent_u128_specification(
        length in 0_u64..=8_388_609, width in 0_u32..=4097, height in 0_u32..=4097,
        layout in prop::sample::select(vec![0_u8,1,2,3,4,5,6,7,8,255])
    ) {
        prop_assert_eq!(
            check_image_bounds(length,width,height,layout).map(|b| (u128::from(b.pixel_count()), b.decoded_byte_length() as u128)),
            independent_numeric_oracle(length,width,height,layout)
        );
    }
    #[test]
    fn each_complete_prepare_payload_field_and_path_affects_operation_identity(which in 0_u8..8) {
        let original = PrepareImage::admit(&request()).unwrap();
        let work = WorkId::parse(WORK).unwrap();
        let mut changed = request();
        let mut path = work.clone();
        match which {
            0 => changed.id = OPERATION.into(),
            1 => changed.operation_id = INTENT.into(),
            2 => changed.expected_sha256 = "b".repeat(64),
            3 => changed.expected_byte_length = 68,
            4 => changed.content_type = dto::ImageMime::Jpeg,
            5 => changed.declarations.provenance.push('é'),
            6 => changed.declarations.rights_refs.push("rights-b".into()),
            _ => path = WorkId::parse(OPERATION).unwrap(),
        }
        let changed = PrepareImage::admit(&changed).unwrap();
        prop_assert_ne!(original.fingerprint(&work).unwrap(), changed.fingerprint(&path).unwrap());
    }
    #[test]
    fn full_width_intent_clocks_match_independent_i128_creation_and_half_open_window(
        created in any::<i64>(), now in any::<i64>()
    ) {
        let admitted = ImageIntent::create(
            PrepareImage::admit(&request()).unwrap(),
            &WorkId::parse(WORK).unwrap(),
            &OwnerId::parse("owner").unwrap(),
            created,
        );
        let expiry = i128::from(created) + 1_800_000;
        if created < 0 {
            prop_assert_eq!(admitted,Err(ImageError::InvalidTimestamp));
        } else if expiry > i128::from(i64::MAX) {
            prop_assert_eq!(admitted,Err(ImageError::InvalidExpiry));
        } else {
            let intent = admitted.unwrap();
            prop_assert_eq!(i128::from(intent.expires_at_ms()),expiry);
            let expected = if now < 0 {
                Err(ImageError::InvalidTimestamp)
            } else if now < created {
                Err(ImageError::IntentNotYetCreated)
            } else if i128::from(now) >= expiry {
                Err(ImageError::IntentExpired)
            } else { Ok(()) };
            prop_assert_eq!(intent.require_pending(now),expected);
        }
    }
}
