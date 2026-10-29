use std::collections::BTreeMap;

use proptest::prelude::*;

use super::*;
use crate::contexts::catalog::domain::{NewWork, Visibility, WorkDetails, WorkKind};
use crate::contexts::media::domain::image::{ImageIntent, ImageMetadata, ImageMime, PrepareImage};

const WORK: &str = "00000000-0000-0000-0000-000000000001";
const OPERATION: &str = "00000000-0000-0000-0000-000000000002";
const INTENT: &str = "00000000-0000-0000-0000-000000000003";

fn work() -> Work {
    Work::new(NewWork {
        id: WorkId::parse(WORK).unwrap(),
        kind: WorkKind::Story,
        owner_id: OwnerId::parse("owner").unwrap(),
        details: WorkDetails {
            title: "Ảnh bìa riêng".into(),
            synopsis: String::new(),
            language: "vi".into(),
            visibility: Visibility::Private,
            rights_refs: vec!["rights-a".into()],
        },
        at: 100,
    })
    .unwrap()
}
fn asset() -> ImageAsset {
    let work = work();
    let intent = ImageIntent::create(
        PrepareImage::admit(&dto::CreateImageIntentRequest {
            operation_id: OPERATION.into(),
            id: INTENT.into(),
            expected_sha256: "a".repeat(64),
            expected_byte_length: 67,
            content_type: dto::ImageMime::Avif,
            declarations: dto::ImageDeclarations {
                provenance: "Prepared cover".into(),
                rights_refs: vec!["rights-z".into(), "rights-a".into()],
            },
        })
        .unwrap(),
        work.id(),
        work.owner_id(),
        100,
    )
    .unwrap();
    ImageAsset::create(
        &intent,
        ImageMetadata::new("a".repeat(64), 67, ImageMime::Avif, 2, 3).unwrap(),
        101,
    )
    .unwrap()
}
fn selection(asset: &ImageAsset) -> dto::WorkArtworkSelection {
    dto::WorkArtworkSelection {
        image_asset_id: asset.id().as_str().into(),
        image_asset_digest: asset.digest().into(),
        alt_text: "A quiet cover".into(),
    }
}
fn request(revision: u64, artwork: Option<dto::WorkArtworkSelection>) -> SetWorkArtwork {
    SetWorkArtwork::admit(&dto::SetWorkArtworkRequest {
        operation_id: OPERATION.into(),
        expected_work_revision: revision,
        artwork,
    })
    .unwrap()
}
fn mutation(decision: ArtworkDecision) -> ArtworkMutation {
    match decision {
        ArtworkDecision::Apply(mutation) => *mutation,
        ArtworkDecision::Replay => panic!("expected an applied mutation"),
    }
}

#[test]
fn selection_references_are_checked_without_claiming_existence() {
    let asset = asset();
    let raw = selection(&asset);
    let selected = WorkArtworkSelection::restore(&raw).unwrap();
    assert_eq!(selected.document(), raw);
    assert_eq!(selected.require_asset(&work(), &asset), Ok(()));
    let mut changed = raw.clone();
    changed.image_asset_id = OPERATION.into();
    assert_eq!(
        WorkArtworkSelection::restore(&changed)
            .unwrap()
            .require_asset(&work(), &asset),
        Err(ArtworkError::AssetBindingMismatch)
    );
    changed = raw.clone();
    changed.image_asset_digest = format!("ia-r1:sha256:{}", "a".repeat(64));
    assert_eq!(
        WorkArtworkSelection::restore(&changed)
            .unwrap()
            .require_asset(&work(), &asset),
        Err(ArtworkError::AssetBindingMismatch)
    );
    changed.image_asset_digest = format!("ii-r1:sha256:{}", "a".repeat(64));
    assert_eq!(
        WorkArtworkSelection::restore(&changed),
        Err(ArtworkError::Image(ImageError::InvalidDigest))
    );
}

#[test]
fn alt_text_is_nonblank_single_line_utf8_with_an_exact_byte_bound() {
    let asset = asset();
    let raw = selection(&asset);
    for (text, error) in [
        ("".into(), ArtworkError::BlankAltText),
        (" \t".into(), ArtworkError::BlankAltText),
        ("a".repeat(1025), ArtworkError::AltTextTooLong),
        ("bìa\ntruyện".into(), ArtworkError::AltTextControlCharacter),
        ("bìa\ttruyện".into(), ArtworkError::AltTextControlCharacter),
        (
            "bìa\u{85}truyện".into(),
            ArtworkError::AltTextControlCharacter,
        ),
        (
            "bìa\u{2028}truyện".into(),
            ArtworkError::AltTextControlCharacter,
        ),
        (
            "bìa\u{2029}truyện".into(),
            ArtworkError::AltTextControlCharacter,
        ),
    ] {
        assert_eq!(
            WorkArtworkSelection::restore(&dto::WorkArtworkSelection {
                alt_text: text,
                ..raw.clone()
            }),
            Err(error)
        );
    }
    let text = "é".repeat(512);
    assert_eq!(
        WorkArtworkSelection::restore(&dto::WorkArtworkSelection {
            alt_text: text.clone(),
            ..raw
        })
        .unwrap()
        .alt_text(),
        text
    );
}

#[test]
fn set_clear_and_unrelated_edits_reproduce_exact_history_without_resurrection() {
    let initial = work();
    let asset = asset();
    assert_eq!(
        artwork_at(&initial, &[]).unwrap(),
        dto::WorkArtworkResponse {
            work_id: WORK.into(),
            work_revision: 1,
            artwork: None,
            changed_at_ms: None
        }
    );
    let set = mutation(
        decide_artwork_change(
            &initial,
            &request(1, Some(selection(&asset))),
            Some(&asset),
            None,
            101,
        )
        .unwrap(),
    );
    let edited = set
        .work()
        .edit(2, set.work().details().clone(), 102)
        .unwrap();
    let clear =
        mutation(decide_artwork_change(&edited, &request(3, None), None, None, 103).unwrap());
    let later = clear
        .work()
        .edit(4, clear.work().details().clone(), 104)
        .unwrap();
    let changes = vec![clear.change().clone(), set.change().clone()];
    for snapshot in [&initial, set.work(), &edited, clear.work(), &later] {
        let actual = artwork_at(snapshot, &changes).unwrap();
        assert_eq!(actual.work_revision, snapshot.revision());
        match snapshot.revision() {
            1 => assert_eq!((actual.artwork, actual.changed_at_ms), (None, None)),
            2 | 3 => assert_eq!(
                (actual.artwork, actual.changed_at_ms),
                (Some(selection(&asset)), Some(101))
            ),
            4 | 5 => assert_eq!((actual.artwork, actual.changed_at_ms), (None, Some(103))),
            _ => panic!("unexpected test snapshot"),
        }
    }
    assert_eq!(set.work().details(), initial.details());
    assert_eq!(initial.revision(), 1);
    assert_eq!(asset.admitted_at_ms(), 101);
}

#[test]
fn each_new_explicit_same_value_command_advances_catalog_metadata_once() {
    let initial = work();
    let asset = asset();
    let first = mutation(
        decide_artwork_change(
            &initial,
            &request(1, Some(selection(&asset))),
            Some(&asset),
            None,
            101,
        )
        .unwrap(),
    );
    let second = mutation(
        decide_artwork_change(
            first.work(),
            &request(2, Some(selection(&asset))),
            Some(&asset),
            None,
            102,
        )
        .unwrap(),
    );
    assert_eq!(second.work().revision(), 3);
    assert_eq!(second.response().artwork, first.response().artwork);
    assert_eq!(second.response().changed_at_ms, Some(102));
    let first_clear =
        mutation(decide_artwork_change(second.work(), &request(3, None), None, None, 103).unwrap());
    let second_clear = mutation(
        decide_artwork_change(first_clear.work(), &request(4, None), None, None, 104).unwrap(),
    );
    assert_eq!(second_clear.work().revision(), 5);
    assert_eq!(second_clear.response().artwork, None);
    assert_eq!(second_clear.response().changed_at_ms, Some(104));
}

#[test]
fn set_uses_catalog_touch_errors_and_every_rejection_leaves_all_input_unchanged() {
    let initial = work();
    let asset = asset();
    let original = (initial.clone(), asset.clone());
    assert_eq!(
        decide_artwork_change(&initial, &request(2, None), None, None, 101),
        Err(ArtworkError::Catalog(CatalogError::RevisionConflict {
            current_revision: 1,
            expected_revision: 2
        }))
    );
    assert_eq!(
        decide_artwork_change(&initial, &request(1, None), None, None, 99),
        Err(ArtworkError::Catalog(CatalogError::TimestampRegression))
    );
    assert_eq!(
        decide_artwork_change(&initial, &request(1, None), None, None, -1),
        Err(ArtworkError::Catalog(CatalogError::InvalidTimestamp))
    );
    assert_eq!(
        decide_artwork_change(
            &initial,
            &request(1, Some(selection(&asset))),
            None,
            None,
            101
        ),
        Err(ArtworkError::MissingAsset)
    );
    assert_eq!(
        decide_artwork_change(&initial, &request(1, None), Some(&asset), None, 101),
        Err(ArtworkError::UnexpectedAsset)
    );
    let archived = initial.archive(1, 101).unwrap();
    assert_eq!(
        decide_artwork_change(&archived, &request(2, None), None, None, 102),
        Err(ArtworkError::Catalog(CatalogError::WorkArchived))
    );
    // Use Catalog's fallible restoration, not unchecked serde, for the exhausted snapshot.
    let maximal = Work::from_stored(crate::contexts::catalog::domain::StoredWork {
        id: initial.id().clone(),
        kind: initial.kind(),
        owner_id: initial.owner_id().clone(),
        details: initial.details().clone(),
        lifecycle: initial.lifecycle(),
        revision: i64::MAX as u64,
        created_at: 100,
        updated_at: 100,
    })
    .unwrap();
    assert_eq!(
        decide_artwork_change(&maximal, &request(i64::MAX as u64, None), None, None, 101),
        Err(ArtworkError::Catalog(CatalogError::RevisionLimit))
    );
    assert_eq!((initial, asset), original);
}

#[test]
fn operation_replay_survives_later_edits_and_archive_but_changed_or_cross_kind_payload_conflicts() {
    let initial = work();
    let asset = asset();
    let set = request(1, Some(selection(&asset)));
    let fingerprint = set.fingerprint(initial.id()).unwrap();
    let applied = mutation(decide_artwork_change(&initial, &set, Some(&asset), None, 101).unwrap());
    let archived = applied.work().archive(2, 102).unwrap();
    assert_eq!(
        decide_artwork_change(&archived, &set, None, Some(&fingerprint), i64::MAX),
        Ok(ArtworkDecision::Replay)
    );
    for changed in [request(2, Some(selection(&asset))), request(1, None)] {
        assert_eq!(
            decide_artwork_change(&archived, &changed, None, Some(&fingerprint), 103),
            Err(ArtworkError::Image(ImageError::OperationReused))
        );
    }
    assert_eq!(
        decide_artwork_change(&initial, &set, Some(&asset), Some("image-operation1:sha256:986d6ad04f10fe2cd35fa43b38ad4c28b18025adc883f4b03eb34e7d7e9726c3"), 101),
        Err(ArtworkError::Image(ImageError::OperationReused))
    );
    assert_eq!(applied.response().work_revision, 2);
}

#[test]
fn selection_cannot_cross_work_or_owner_even_with_a_valid_asset_digest() {
    let initial = work();
    let asset = asset();
    // A fully valid foreign receipt is constructed from its own authorized scope facts.
    let foreign_work = Work::from_stored(crate::contexts::catalog::domain::StoredWork {
        id: WorkId::parse(OPERATION).unwrap(),
        kind: initial.kind(),
        owner_id: initial.owner_id().clone(),
        details: initial.details().clone(),
        lifecycle: initial.lifecycle(),
        revision: 1,
        created_at: 100,
        updated_at: 100,
    })
    .unwrap();
    assert_eq!(
        WorkArtworkSelection::restore(&selection(&asset))
            .unwrap()
            .require_asset(&foreign_work, &asset),
        Err(ArtworkError::AssetBindingMismatch)
    );
    let foreign_owner = Work::from_stored(crate::contexts::catalog::domain::StoredWork {
        id: initial.id().clone(),
        kind: initial.kind(),
        owner_id: OwnerId::parse("other-owner").unwrap(),
        details: initial.details().clone(),
        lifecycle: initial.lifecycle(),
        revision: 1,
        created_at: 100,
        updated_at: 100,
    })
    .unwrap();
    assert_eq!(
        WorkArtworkSelection::restore(&selection(&asset))
            .unwrap()
            .require_asset(&foreign_owner, &asset),
        Err(ArtworkError::AssetBindingMismatch)
    );
}

#[test]
fn ordered_change_and_set_clear_operation_preimages_match_independent_python_goldens() {
    let initial = work();
    let asset = asset();
    let set = request(1, Some(selection(&asset)));
    assert_eq!(
        set.fingerprint(initial.id()).unwrap(),
        "image-operation1:sha256:06717cc876a8fa88a91151e0e459eb5adfa06c702f5e1490ee0c8d79cf991a78"
    );
    assert_eq!(
        request(1, None).fingerprint(initial.id()).unwrap(),
        "image-operation1:sha256:54b88cc298fe7c7c1a98ccca69546a2df0cd209b6a47826ccb70b082d1e38374"
    );
    let applied = mutation(decide_artwork_change(&initial, &set, Some(&asset), None, 101).unwrap());
    assert_eq!(
        applied.change().document().digest,
        "wa-r1:sha256:e991dac2780025d540181f6786fc7f6969f3cd2e60efe7e9d4584873151d78ea"
    );
    assert_eq!(
        ArtworkChange::restore(applied.change().document().clone()).unwrap(),
        *applied.change()
    );
}

#[test]
fn stored_history_and_response_mismatches_fail_closed() {
    let initial = work();
    let applied =
        mutation(decide_artwork_change(&initial, &request(1, None), None, None, 101).unwrap());
    let raw = applied.change().document().clone();
    let mut missing = serde_json::to_value(&raw).unwrap();
    missing.as_object_mut().unwrap().remove("artwork");
    assert_eq!(
        serde_json::from_value::<ArtworkChangeDocument>(missing)
            .unwrap_err()
            .to_string(),
        "missing field `artwork`"
    );
    for (changed, expected) in [
        (
            ArtworkChangeDocument {
                recorded_by: "other-owner".into(),
                ..raw.clone()
            },
            ArtworkError::InvalidRecordedBy,
        ),
        (
            ArtworkChangeDocument {
                work_metadata_revision: 0,
                ..raw.clone()
            },
            ArtworkError::Catalog(CatalogError::InvalidRevision),
        ),
        (
            ArtworkChangeDocument {
                recorded_at_ms: u64::MAX,
                ..raw.clone()
            },
            ArtworkError::Image(ImageError::InvalidTimestamp),
        ),
        (
            ArtworkChangeDocument {
                work_metadata_revision: 3,
                ..raw.clone()
            },
            ArtworkError::Image(ImageError::InvalidDigest),
        ),
    ] {
        assert_eq!(ArtworkChange::restore(changed), Err(expected));
    }
    assert_eq!(
        artwork_at(
            applied.work(),
            &[applied.change().clone(), applied.change().clone()]
        ),
        Err(ArtworkError::DuplicateRevision)
    );
    let response = applied.response().clone();
    assert_eq!(
        restore_artwork_response(response.clone(), initial.id(), Revision::new(2).unwrap())
            .unwrap(),
        response
    );
    assert_eq!(
        restore_artwork_response(response.clone(), initial.id(), Revision::new(3).unwrap()),
        Err(ArtworkError::InvalidResponse)
    );
    assert_eq!(
        restore_artwork_response(
            dto::WorkArtworkResponse {
                changed_at_ms: Some(u64::MAX),
                ..response
            },
            initial.id(),
            Revision::new(2).unwrap()
        ),
        Err(ArtworkError::Image(ImageError::InvalidTimestamp))
    );
}

#[test]
fn cross_transport_set_and_explicit_clear_keep_identity_while_omission_is_rejected() {
    use crate::contract::codec::{decode_artwork, encode_artwork, WireFormat};
    let work = work();
    let asset = asset();
    let set = request(1, Some(selection(&asset)));
    let clear = request(1, None);
    assert_ne!(set.fingerprint(work.id()), clear.fingerprint(work.id()));
    for command in [&set, &clear] {
        for (first, second) in [
            (WireFormat::Json, WireFormat::Protobuf),
            (WireFormat::Protobuf, WireFormat::Json),
        ] {
            let bytes = encode_artwork(first, command.document()).unwrap();
            let raw: dto::SetWorkArtworkRequest = decode_artwork(first, &bytes).unwrap();
            let bytes = encode_artwork(second, &raw).unwrap();
            let raw: dto::SetWorkArtworkRequest = decode_artwork(second, &bytes).unwrap();
            let admitted = SetWorkArtwork::admit(&raw).unwrap();
            assert_eq!(admitted, *command);
            assert_eq!(
                admitted.fingerprint(work.id()),
                command.fingerprint(work.id())
            );
            assert_eq!(
                decide_artwork_change(
                    &work,
                    &admitted,
                    None,
                    Some(&command.fingerprint(work.id()).unwrap()),
                    103,
                ),
                Ok(ArtworkDecision::Replay)
            );
        }
    }
    assert_eq!(
        decode_artwork::<dto::SetWorkArtworkRequest>(
            WireFormat::Json,
            format!("{{\"operation_id\":\"{OPERATION}\",\"expected_work_revision\":1}}").as_bytes(),
        ),
        Err(crate::contract::codec::DecodeError::Shape)
    );
    let absent_proto = [0x0a, 0x24];
    let mut absent_proto = [absent_proto.as_slice(), OPERATION.as_bytes(), &[0x10, 0x01]].concat();
    assert_eq!(
        decode_artwork::<dto::SetWorkArtworkRequest>(WireFormat::Protobuf, &absent_proto),
        Err(crate::contract::codec::DecodeError::Shape)
    );
    // Empty, present ClearArtwork is an explicit action, not an absent oneof arm.
    absent_proto.extend([0x22, 0x00]);
    assert_eq!(
        SetWorkArtwork::admit(
            &decode_artwork::<dto::SetWorkArtworkRequest>(WireFormat::Protobuf, &absent_proto)
                .unwrap()
        )
        .unwrap(),
        clear
    );
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 128, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_013), ..ProptestConfig::default() })]
    #[test]
    fn arbitrary_set_edit_clear_retry_traces_match_a_naive_history_and_receipt_model(
        commands in prop::collection::vec((0_u8..6, 0_u8..8), 1..64)
    ) {
        let asset = asset();
        let mut actual_work = work();
        let mut snapshots = vec![actual_work.clone()];
        let mut actual_changes = Vec::new();
        let mut model_events: Vec<(u64, Option<dto::WorkArtworkSelection>, u64)> = Vec::new();
        let mut receipts: BTreeMap<u8,(dto::SetWorkArtworkRequest,String)> = BTreeMap::new();
        let mut model_head = 1_u64;
        for (index,(command,operation)) in commands.iter().copied().enumerate() {
            let now = 101 + index as i64;
            let before_work = actual_work.clone();
            let before_changes = actual_changes.clone();
            if command == 0 {
                actual_work = actual_work.edit(actual_work.revision(),actual_work.details().clone(),now).unwrap();
                model_head += 1;
                snapshots.push(actual_work.clone());
            } else {
                let op_id = format!("00000000-0000-0000-0000-{:012}",u64::from(operation)+10);
                let mut raw = dto::SetWorkArtworkRequest {
                    operation_id: op_id,
                    expected_work_revision: if command == 3 { if model_head == 1 { 2 } else { model_head-1 } } else { model_head },
                    artwork: if command == 2 { None } else { Some(selection(&asset)) },
                };
                if command == 4 {
                    if let Some((prior,_)) = receipts.get(&operation) { raw = prior.clone(); }
                }
                if command == 5 {
                    if let Some((prior,_)) = receipts.get(&operation) {
                        raw = prior.clone();
                        raw.artwork = match raw.artwork { Some(_) => None, None => Some(selection(&asset)) };
                    }
                }
                let checked = SetWorkArtwork::admit(&raw).unwrap();
                let prior_fingerprint = receipts.get(&operation).map(|(_,fp)|fp.as_str());
                let resolved = if raw.artwork.is_some() { Some(&asset) } else { None };
                let observed = decide_artwork_change(&actual_work,&checked,resolved,prior_fingerprint,now);
                // The reference model compares complete explicit requests, not hashes or the
                // implementation's decision table. Its history is a plain append-only list.
                if let Some((prior,_)) = receipts.get(&operation) {
                    if prior == &raw {
                        prop_assert_eq!(observed,Ok(ArtworkDecision::Replay));
                    } else {
                        prop_assert_eq!(observed,Err(ArtworkError::Image(ImageError::OperationReused)));
                    }
                    prop_assert_eq!(&actual_work,&before_work);
                    prop_assert_eq!(&actual_changes,&before_changes);
                } else if raw.expected_work_revision != model_head {
                    prop_assert_eq!(observed,Err(ArtworkError::Catalog(CatalogError::RevisionConflict { current_revision:model_head,expected_revision:raw.expected_work_revision })));
                    prop_assert_eq!(&actual_work,&before_work);
                    prop_assert_eq!(&actual_changes,&before_changes);
                } else {
                    let applied = mutation(observed.unwrap());
                    model_head += 1;
                    model_events.push((model_head,raw.artwork.clone(),now as u64));
                    receipts.insert(operation,(raw,checked.fingerprint(actual_work.id()).unwrap()));
                    actual_work = applied.work().clone();
                    actual_changes.push(applied.change().clone());
                    snapshots.push(actual_work.clone());
                }
            }
            prop_assert_eq!(actual_work.revision(),model_head);
            for snapshot in &snapshots {
                let latest = model_events.iter().rev().find(|(rev,_,_)|*rev<=snapshot.revision());
                let (artwork,changed_at_ms) = latest.map_or((None,None),|(_,artwork,at)|(artwork.clone(),Some(*at)));
                prop_assert_eq!(artwork_at(snapshot,&actual_changes).unwrap(),dto::WorkArtworkResponse {
                    work_id:WORK.into(),work_revision:snapshot.revision(),artwork,changed_at_ms
                });
            }
        }
    }
}

#[test]
fn a_restored_legacy_asset_cannot_be_selected_as_new_artwork() {
    let mut raw = asset().document().clone();
    raw.verified_media.content_type = dto::ImageMime::Png;
    // Independent Python literal oracle retained from the historical fixture.
    raw.digest =
        "ia-r1:sha256:4200f444b5e0006f4d0a04b7388d5b1e91b9ae03b2f38256130f5e8b2dad7f63".into();
    let legacy = ImageAsset::restore(raw).unwrap();
    let initial = work();
    let set = request(1, Some(selection(&legacy)));
    assert_eq!(
        decide_artwork_change(&initial, &set, Some(&legacy), None, 101),
        Err(ArtworkError::Image(
            crate::contexts::media::domain::image::ImageError::UnsupportedMime
        ))
    );
    assert_eq!(initial.revision(), 1);
}
