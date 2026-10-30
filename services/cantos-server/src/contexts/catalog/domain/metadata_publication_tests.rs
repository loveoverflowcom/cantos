use proptest::prelude::*;

use super::*;
use crate::contexts::catalog::domain::{
    ContentUnitKind, NewContentUnit, NewWork, OwnerId, ScriptRef, SourceRef, UnitDetails,
    Visibility, WorkDetails, WorkKind,
};

const WORK: &str = "10000000-0000-4000-8000-00000000000a";
const OTHER: &str = "10000000-0000-4000-8000-00000000000b";
const A: &str = "20000000-0000-4000-8000-000000000001";
const B: &str = "20000000-0000-4000-8000-000000000002";
const C: &str = "20000000-0000-4000-8000-000000000003";

fn digest(seed: u8) -> String {
    format!("{seed:02x}").repeat(32)
}
fn pin(revision: u64, seed: u8) -> Pin {
    Pin::new(Revision::new(revision).unwrap(), &digest(seed)).unwrap()
}
fn unit_id(id: &str) -> ContentUnitId {
    ContentUnitId::parse(id).unwrap()
}
fn entry(id: &str, revision: u64) -> PublishedEntry {
    PublishedEntry {
        unit: unit_id(id),
        pin: pin(revision, revision as u8),
        audio: None,
    }
}
fn work(id: &str, rights: &[&str]) -> Work {
    Work::new(NewWork {
        id: WorkId::parse(id).unwrap(),
        kind: WorkKind::Novel,
        owner_id: OwnerId::parse("alice").unwrap(),
        details: WorkDetails {
            title: "Mùa trăng qua hiên".into(),
            synopsis: "Một câu chuyện Việt Nam.".into(),
            language: "vi-VN".into(),
            visibility: Visibility::Private,
            rights_refs: rights.iter().map(|r| (*r).to_owned()).collect(),
        },
        at: 1_000,
    })
    .unwrap()
}
fn unit(parent: &Work, id: &str) -> ContentUnit {
    ContentUnit::new(
        parent,
        NewContentUnit {
            id: unit_id(id),
            kind: ContentUnitKind::Chapter,
            details: UnitDetails {
                title: "Chương một".into(),
                description: "Mở đầu".into(),
                source: None,
                script: None,
                content_digest: None,
            },
            position: 0,
            at: 1_000,
        },
    )
    .unwrap()
}
fn ids(generation: &Generation) -> Vec<&str> {
    generation
        .entries()
        .iter()
        .map(|entry| entry.unit().as_str())
        .collect()
}
fn publish(work: Pin, entries: Vec<PublishedEntry>) -> PublicationChange {
    PublicationChange::Publish { work, entries }
}
fn state(settled: Settled) -> PublicationState {
    PublicationState::Published(settled.published)
}
fn first(entries: Vec<PublishedEntry>) -> PublicationState {
    state(
        decide(
            &PublicationState::NeverPublished,
            0,
            publish(pin(3, 9), entries),
            10,
        )
        .unwrap(),
    )
}

#[test]
fn first_publish_pins_the_explicit_selection_order_and_sets_the_recency_anchor() {
    let settled = decide(
        &PublicationState::NeverPublished,
        0,
        publish(pin(3, 9), vec![entry(C, 1), entry(A, 2)]),
        10,
    )
    .unwrap();
    assert!(settled.created);
    let published = &settled.published;
    assert_eq!(published.version().get(), 1);
    assert_eq!(published.lifecycle(), Live::Active);
    assert_eq!(published.first_visible_at(), 10);
    assert_eq!(published.generation().number().get(), 1);
    assert_eq!(published.generation().parent(), None);
    assert_eq!(published.generation().work(), &pin(3, 9));
    // Order is the command's, not a Catalog position or UUID order.
    assert_eq!(ids(published.generation()), [C, A]);
}

#[test]
fn compare_and_set_reports_the_current_version_and_never_settles_a_stale_command() {
    let active = first(vec![entry(A, 1)]);
    for expected in [0, 2, u64::MAX] {
        assert_eq!(
            decide(&active, expected, PublicationChange::Withdraw, 20),
            Err(PublicationError::StaleVersion { current: 1 })
        );
    }
    assert_eq!(
        decide(
            &PublicationState::NeverPublished,
            1,
            publish(pin(1, 1), vec![]),
            20
        ),
        Err(PublicationError::StaleVersion { current: 0 })
    );
}

#[test]
fn per_unit_publish_copies_every_other_entry_and_keeps_the_work_pin() {
    let active = first(vec![entry(A, 1), entry(B, 1)]);
    let settled = decide(
        &active,
        1,
        PublicationChange::PublishUnit {
            entry: entry(C, 4),
            position: 2,
        },
        20,
    )
    .unwrap();
    let generation = settled.published.generation();
    assert_eq!(ids(generation), [A, C, B]);
    assert_eq!(generation.work(), &pin(3, 9));
    assert_eq!(generation.entries()[0], entry(A, 1));
    assert_eq!(generation.entries()[2], entry(B, 1));
    assert_eq!(generation.number().get(), 2);
    assert_eq!(generation.parent().map(GenerationNumber::get), Some(1));
    assert_eq!(settled.published.first_visible_at(), 10);
    assert_eq!(settled.published.version().get(), 2);

    // Replacing an existing entry moves it and pins the new revision; the rest is unchanged.
    let replaced = decide(
        &state(settled),
        2,
        PublicationChange::PublishUnit {
            entry: entry(B, 7),
            position: 1,
        },
        30,
    )
    .unwrap();
    assert_eq!(ids(replaced.published.generation()), [B, A, C]);
    assert_eq!(replaced.published.generation().entries()[0], entry(B, 7));
    assert_eq!(replaced.published.generation().entries()[2], entry(C, 4));
}

#[test]
fn per_unit_actions_need_an_active_pointer_and_a_published_unit() {
    let never = PublicationState::NeverPublished;
    let add = || PublicationChange::PublishUnit {
        entry: entry(A, 1),
        position: 1,
    };
    assert_eq!(
        decide(&never, 0, add(), 1),
        Err(PublicationError::NotActive)
    );
    assert_eq!(
        decide(&never, 0, PublicationChange::Withdraw, 1),
        Err(PublicationError::NotActive)
    );
    let active = first(vec![entry(A, 1)]);
    assert_eq!(
        decide(
            &active,
            1,
            PublicationChange::WithdrawUnit { unit: unit_id(B) },
            20
        ),
        Err(PublicationError::UnitNotPublished { unit: unit_id(B) })
    );
    for position in [0, 3, u32::MAX] {
        assert_eq!(
            decide(
                &active,
                1,
                PublicationChange::PublishUnit {
                    entry: entry(B, 1),
                    position
                },
                20
            ),
            Err(PublicationError::InvalidPosition)
        );
    }
    let withdrawn = state(decide(&active, 1, PublicationChange::Withdraw, 20).unwrap());
    assert_eq!(
        decide(&withdrawn, 2, add(), 30),
        Err(PublicationError::NotActive)
    );
    assert_eq!(
        decide(&withdrawn, 2, PublicationChange::Withdraw, 30),
        Err(PublicationError::NotActive)
    );
}

#[test]
fn withdrawing_the_last_unit_keeps_a_visible_work_with_no_units() {
    let active = first(vec![entry(A, 1)]);
    let settled = decide(
        &active,
        1,
        PublicationChange::WithdrawUnit { unit: unit_id(A) },
        20,
    )
    .unwrap();
    assert!(settled.created);
    assert_eq!(settled.published.lifecycle(), Live::Active);
    assert!(settled.published.generation().entries().is_empty());
}

#[test]
fn withdraw_and_republish_keep_history_and_the_first_anchor() {
    let active = first(vec![entry(A, 1), entry(B, 1)]);
    let withdrawn = decide(&active, 1, PublicationChange::Withdraw, 20).unwrap();
    assert!(!withdrawn.created);
    assert_eq!(withdrawn.published.lifecycle(), Live::Withdrawn);
    assert_eq!(withdrawn.published.generation().number().get(), 1);
    assert_eq!(withdrawn.published.version().get(), 2);
    let withdrawn = state(withdrawn);
    assert_eq!(withdrawn.lifecycle(), PublicationLifecycle::Withdrawn);
    // Republishing even the identical selection is a new generation, never a revived pointer.
    let republished = decide(
        &withdrawn,
        2,
        publish(pin(3, 9), vec![entry(A, 1), entry(B, 1)]),
        30,
    )
    .unwrap();
    assert!(republished.created);
    assert_eq!(republished.published.generation().number().get(), 2);
    assert_eq!(republished.published.version().get(), 3);
    assert_eq!(republished.published.first_visible_at(), 10);
}

#[test]
fn an_identical_active_result_is_refused_instead_of_minting_a_version() {
    let active = first(vec![entry(A, 1), entry(B, 1)]);
    assert_eq!(
        decide(
            &active,
            1,
            publish(pin(3, 9), vec![entry(A, 1), entry(B, 1)]),
            20
        ),
        Err(PublicationError::Unchanged)
    );
    assert_eq!(
        decide(
            &active,
            1,
            PublicationChange::PublishUnit {
                entry: entry(B, 1),
                position: 2
            },
            20
        ),
        Err(PublicationError::Unchanged)
    );
    // A reorder is a change.
    let reordered = decide(
        &active,
        1,
        publish(pin(3, 9), vec![entry(B, 1), entry(A, 1)]),
        20,
    )
    .unwrap();
    assert_eq!(ids(reordered.published.generation()), [B, A]);
}

#[test]
fn selections_are_bounded_and_duplicate_free() {
    assert_eq!(
        decide(
            &PublicationState::NeverPublished,
            0,
            publish(pin(1, 1), vec![entry(A, 1), entry(A, 2)]),
            1
        ),
        Err(PublicationError::DuplicateUnit { unit: unit_id(A) })
    );
    let many: Vec<_> = (0..=MAX_PUBLISHED_UNITS)
        .map(|n| PublishedEntry {
            audio: None,
            unit: unit_id(&format!("30000000-0000-4000-8000-{n:012x}")),
            pin: pin(1, 1),
        })
        .collect();
    assert_eq!(
        decide(
            &PublicationState::NeverPublished,
            0,
            publish(pin(1, 1), many.clone()),
            1
        ),
        Err(PublicationError::TooManyUnits)
    );
    let bounded = many[..MAX_PUBLISHED_UNITS].to_vec();
    let full = state(
        decide(
            &PublicationState::NeverPublished,
            0,
            publish(pin(1, 1), bounded),
            1,
        )
        .unwrap(),
    );
    assert_eq!(
        decide(
            &full,
            1,
            PublicationChange::PublishUnit {
                entry: entry(A, 1),
                position: 1
            },
            2
        ),
        Err(PublicationError::TooManyUnits)
    );
    assert_eq!(
        decide(
            &PublicationState::NeverPublished,
            0,
            publish(pin(1, 1), vec![]),
            -1
        ),
        Err(PublicationError::InvalidTimestamp)
    );
}

#[test]
fn admission_refuses_archived_foreign_and_unsubstantiated_revisions() {
    let current = work(WORK, &["rights-1"]);
    assert_eq!(
        admit_work(&current, &current, &digest(1), true),
        Ok(pin(1, 1))
    );
    assert_eq!(
        admit_work(&current, &current, &digest(1), false),
        Err(PublicationError::RightsUnavailable)
    );
    let unsubstantiated = work(WORK, &[]);
    assert_eq!(
        admit_work(&unsubstantiated, &unsubstantiated, &digest(1), true),
        Err(PublicationError::RightsUnavailable)
    );
    let archived = current.archive(1, 2_000).unwrap();
    assert_eq!(
        admit_work(&current, &archived, &digest(1), true),
        Err(PublicationError::WorkArchived)
    );
    assert_eq!(
        admit_work(&current, &current, "not-a-digest", true),
        Err(PublicationError::InvalidDigest)
    );
    assert_eq!(
        admit_work(&work(OTHER, &["r"]), &current, &digest(1), true),
        Err(PublicationError::Corrupt)
    );

    let chapter = unit(&current, A);
    let work_id = WorkId::parse(WORK).unwrap();
    assert_eq!(
        admit_unit(&work_id, &chapter, &chapter, &digest(1)),
        Ok(entry(A, 1))
    );
    let foreign = unit(&work(OTHER, &[]), A);
    assert_eq!(
        admit_unit(&work_id, &foreign, &foreign, &digest(1)),
        Err(PublicationError::WrongWork { unit: unit_id(A) })
    );
    let tombstone = chapter.archive(&current, 1, 2_000).unwrap();
    assert_eq!(
        admit_unit(&work_id, &chapter, &tombstone, &digest(1)),
        Err(PublicationError::UnitArchived { unit: unit_id(A) })
    );
}

#[test]
fn restore_rejects_stored_contradictions() {
    let number = GenerationNumber::new(2).unwrap();
    let duplicated = vec![(unit_id(A), pin(1, 1), None), (unit_id(A), pin(2, 2), None)];
    assert_eq!(
        Generation::restore(number, pin(1, 1), duplicated),
        Err(PublicationError::Corrupt)
    );
    let generation = Generation::restore(number, pin(1, 1), vec![]).unwrap();
    let version = PublicationVersion::new(1).unwrap();
    assert_eq!(
        Published::restore(version, Live::Active, generation.clone(), 0),
        Err(PublicationError::Corrupt)
    );
    let version = PublicationVersion::new(2).unwrap();
    assert_eq!(
        Published::restore(version, Live::Active, generation, -1),
        Err(PublicationError::Corrupt)
    );
    assert_eq!(Live::parse("draft"), Err(PublicationError::Corrupt));
    assert_eq!(
        PublicationVersion::new(0),
        Err(PublicationError::InvalidVersion)
    );
    assert_eq!(
        GenerationNumber::new(i64::MAX as u64 + 1),
        Err(PublicationError::InvalidVersion)
    );
}

#[test]
fn public_visibility_ignores_editorial_visibility_and_hides_archived_units_entirely() {
    assert!(work_visible(Live::Active, true, false));
    assert!(!work_visible(Live::Withdrawn, true, false));
    assert!(!work_visible(Live::Active, false, false));
    assert!(!work_visible(Live::Active, true, true));
    let generation = match first(vec![entry(A, 1), entry(B, 1), entry(C, 1)]) {
        PublicationState::Published(published) => published.generation().clone(),
        PublicationState::NeverPublished => unreachable!(),
    };
    let visible = visible_entries(&generation, |unit| unit.as_str() == B);
    let ranked: Vec<_> = visible
        .iter()
        .map(|visible| {
            (
                visible.ordinal,
                visible.position,
                visible.entry.unit().as_str(),
            )
        })
        .collect();
    assert_eq!(ranked, [(1, 1, A), (2, 3, C)]);
}

#[test]
fn search_text_uses_the_discovery_nfc_lowercase_rule() {
    // Composed and decomposed Vietnamese spellings meet at one NFC lower-case form.
    assert_eq!(public_search_text("MÙA Trăng"), "mùa trăng");
    assert_eq!(public_search_text("A\u{0301}nh"), "ánh");
    assert_eq!(public_search_text("Ánh"), public_search_text("A\u{0301}nh"));
}

fn release(seed: u8) -> AudioRef {
    AudioRef::new(
        &format!("70000000-0000-4000-8000-0000000000{seed:02x}"),
        &format!("rl-r1:sha256:{}", digest(seed)),
    )
    .unwrap()
}
fn exact(seed: u8) -> AudioAssociation {
    AudioAssociation::admit(release(seed), AudioCompatibility::Exact, false).unwrap()
}
fn associate(unit: &str, audio: AudioAssociation) -> PublicationChange {
    PublicationChange::AssociateAudio {
        unit: unit_id(unit),
        audio,
    }
}
fn audio_of(state: &PublicationState, unit: &str) -> Option<AudioAssociation> {
    match state {
        PublicationState::Published(published) => published
            .generation()
            .entry(&unit_id(unit))
            .and_then(|entry| entry.audio().cloned()),
        PublicationState::NeverPublished => None,
    }
}
fn edited_work(base: &Work, change: impl FnOnce(&mut WorkDetails)) -> Work {
    let mut details = base.details().clone();
    change(&mut details);
    base.edit(base.revision(), details, 2_000).unwrap()
}
fn edited_unit(
    parent: &Work,
    base: &ContentUnit,
    change: impl FnOnce(&mut UnitDetails),
) -> ContentUnit {
    let mut details = base.details().clone();
    change(&mut details);
    base.edit(parent, base.revision(), details, 2_000).unwrap()
}

#[test]
fn audio_refs_admit_only_canonical_release_ids_and_release_digests() {
    let digest = format!("rl-r1:sha256:{}", "a".repeat(64));
    assert!(AudioRef::new("70000000-0000-4000-8000-000000000001", &digest).is_ok());
    for (id, digest) in [
        ("70000000-0000-4000-8000-00000000000A", digest.as_str()),
        ("not-a-release", digest.as_str()),
        (
            "70000000-0000-4000-8000-000000000001",
            "a".repeat(64).as_str(),
        ),
        ("70000000-0000-4000-8000-000000000001", "rm-r1:sha256:aaaa"),
        (
            "70000000-0000-4000-8000-000000000001",
            &format!("rl-r1:sha256:{}", "A".repeat(64)),
        ),
    ] {
        assert_eq!(
            AudioRef::new(id, digest),
            Err(PublicationError::InvalidAudioRef)
        );
    }
}

/// The D06 truth table over real Catalog snapshots: which drift keeps audio, which asks the
/// owner, which leaves the entry metadata-only.
#[test]
fn compatibility_matrix_separates_display_only_from_content_language_kind_and_rights() {
    let released_work = work(WORK, &["r1"]);
    let released_unit = unit(&released_work, A);
    let compat =
        |w: &Work, u: &ContentUnit| audio_compatibility(&released_work, &released_unit, w, u);
    // Same Unit revision: exact, also when only the Work's display text or visibility moved.
    assert_eq!(
        compat(&released_work, &released_unit),
        AudioCompatibility::Exact
    );
    let retitled = edited_work(&released_work, |d| {
        d.title = "Mùa trăng mới".into();
        d.synopsis = "Lời giới thiệu khác".into();
        d.visibility = Visibility::Public;
    });
    assert_eq!(compat(&retitled, &released_unit), AudioCompatibility::Exact);
    // Unit display text only: explicit owner association.
    let renamed = edited_unit(&released_work, &released_unit, |d| {
        d.title = "Chương một (bản sửa)".into();
        d.description = "Mô tả mới".into();
    });
    assert_eq!(
        compat(&released_work, &renamed),
        AudioCompatibility::DisplayOnly
    );
    assert_eq!(compat(&retitled, &renamed), AudioCompatibility::DisplayOnly);
    // A new revision with no visible change is still not the exact pin.
    let resaved = edited_unit(&released_work, &released_unit, |_| {});
    assert_eq!(
        compat(&released_work, &resaved),
        AudioCompatibility::DisplayOnly
    );
    // Content, language and rights drift: incompatible, whatever else is equal.
    let content = edited_unit(&released_work, &released_unit, |d| {
        d.content_digest = Some(format!("sir-c1:sha256:{}", "c".repeat(64)));
    });
    let source = edited_unit(&released_work, &released_unit, |d| {
        d.source = Some(SourceRef::new("source-1".into(), "d".repeat(64)).unwrap());
    });
    let script = edited_unit(&released_work, &released_unit, |d| {
        d.script = Some(
            ScriptRef::new(
                "50000000-0000-4000-8000-000000000001".into(),
                2,
                format!("sir-e1:sha256:{}", "e".repeat(64)),
            )
            .unwrap(),
        );
    });
    for drifted in [&content, &source, &script] {
        assert_eq!(
            compat(&released_work, drifted),
            AudioCompatibility::Incompatible
        );
    }
    let english = edited_work(&released_work, |d| d.language = "en".into());
    let more_rights = edited_work(&released_work, |d| d.rights_refs.push("r2".into()));
    let other_rights = edited_work(&released_work, |d| d.rights_refs = vec!["r2".into()]);
    for drifted in [&english, &more_rights, &other_rights] {
        assert_eq!(
            compat(drifted, &released_unit),
            AudioCompatibility::Incompatible
        );
        assert_eq!(compat(drifted, &renamed), AudioCompatibility::Incompatible);
    }
    // Tag case and rights order are not drift.
    let recased = edited_work(&released_work, |d| d.language = "VI-vn".into());
    assert_eq!(compat(&recased, &released_unit), AudioCompatibility::Exact);
    let two = work(WORK, &["r1", "r2"]);
    let swapped = edited_work(&two, |d| d.rights_refs = vec!["r2".into(), "r1".into()]);
    assert_eq!(
        audio_compatibility(&two, &unit(&two, A), &swapped, &unit(&two, A)),
        AudioCompatibility::Exact
    );
    // Another Unit or Work is never compatible.
    assert_eq!(
        compat(&released_work, &unit(&released_work, B)),
        AudioCompatibility::Incompatible
    );
    let other = work(OTHER, &["r1"]);
    assert_eq!(
        compat(&other, &unit(&other, A)),
        AudioCompatibility::Incompatible
    );
}

#[test]
fn association_admission_needs_confirmation_for_display_drift_and_refuses_incompatibility() {
    assert_eq!(exact(1).basis(), AudioBasis::Exact);
    // Confirming an exact pin records it as exact, not as an owner override.
    assert_eq!(
        AudioAssociation::admit(release(1), AudioCompatibility::Exact, true).unwrap(),
        exact(1)
    );
    assert_eq!(
        AudioAssociation::admit(release(1), AudioCompatibility::DisplayOnly, false),
        Err(PublicationError::AudioConfirmationRequired)
    );
    let confirmed =
        AudioAssociation::admit(release(1), AudioCompatibility::DisplayOnly, true).unwrap();
    assert_eq!(confirmed.basis(), AudioBasis::DisplayOnly);
    for confirm in [false, true] {
        assert_eq!(
            AudioAssociation::admit(release(1), AudioCompatibility::Incompatible, confirm),
            Err(PublicationError::AudioIncompatible)
        );
    }
    assert_eq!(AudioBasis::parse("exact"), Ok(AudioBasis::Exact));
    assert_eq!(
        AudioBasis::parse("display_only"),
        Ok(AudioBasis::DisplayOnly)
    );
    assert_eq!(AudioBasis::parse("latest"), Err(PublicationError::Corrupt));
}

#[test]
fn associating_writes_a_new_generation_that_copies_every_other_entry() {
    let active = first(vec![entry(A, 1), entry(B, 2)]);
    let settled = decide(&active, 1, associate(B, exact(7)), 20).unwrap();
    assert!(settled.created);
    let next = state(settled);
    let PublicationState::Published(published) = &next else {
        unreachable!()
    };
    assert_eq!(published.version().get(), 2);
    assert_eq!(published.generation().number().get(), 2);
    assert_eq!(published.first_visible_at(), 10);
    assert_eq!(ids(published.generation()), [A, B]);
    assert_eq!(published.generation().work(), &pin(3, 9));
    assert_eq!(audio_of(&next, A), None);
    assert_eq!(audio_of(&next, B), Some(exact(7)));
    // The same association again mints nothing; a different release does.
    assert_eq!(
        decide(&next, 2, associate(B, exact(7)), 30),
        Err(PublicationError::Unchanged)
    );
    let replaced = state(decide(&next, 2, associate(B, exact(8)), 30).unwrap());
    assert_eq!(audio_of(&replaced, B), Some(exact(8)));
    // Only an ACTIVE generation's published Unit can be associated.
    assert_eq!(
        decide(&next, 2, associate(C, exact(7)), 30),
        Err(PublicationError::UnitNotPublished { unit: unit_id(C) })
    );
    let withdrawn = state(decide(&next, 2, PublicationChange::Withdraw, 30).unwrap());
    assert_eq!(
        decide(&withdrawn, 3, associate(B, exact(7)), 40),
        Err(PublicationError::NotActive)
    );
    assert_eq!(
        decide(
            &PublicationState::NeverPublished,
            0,
            associate(B, exact(7)),
            40
        ),
        Err(PublicationError::NotActive)
    );
    assert_eq!(
        decide(&next, 1, associate(B, exact(8)), 30),
        Err(PublicationError::StaleVersion { current: 2 })
    );
}

#[test]
fn metadata_changes_keep_audio_only_on_the_same_pin_and_never_rebind_implicitly() {
    let display =
        AudioAssociation::admit(release(5), AudioCompatibility::DisplayOnly, true).unwrap();
    let active = first(vec![entry(A, 1), entry(B, 2)]);
    let active = state(decide(&active, 1, associate(A, display.clone()), 20).unwrap());
    let active = state(decide(&active, 2, associate(B, exact(6)), 21).unwrap());
    // Republishing the same pins in a new order keeps both associations.
    let reordered = state(
        decide(
            &active,
            3,
            publish(pin(3, 9), vec![entry(B, 2), entry(A, 1)]),
            30,
        )
        .unwrap(),
    );
    assert_eq!(audio_of(&reordered, A), Some(display.clone()));
    assert_eq!(audio_of(&reordered, B), Some(exact(6)));
    // A new Work pin carries nothing: its language or rights may differ, so the shell re-offers
    // exact associations and the owner re-confirms display-only ones.
    let rewritten = state(
        decide(
            &reordered,
            4,
            publish(pin(4, 8), vec![entry(B, 2), entry(A, 1)]),
            35,
        )
        .unwrap(),
    );
    assert_eq!(audio_of(&rewritten, A), None);
    assert_eq!(audio_of(&rewritten, B), None);
    // A new Unit revision is metadata-only: no association follows a changed pin.
    let edited = state(
        decide(
            &reordered,
            4,
            PublicationChange::PublishUnit {
                entry: entry(A, 3),
                position: 2,
            },
            40,
        )
        .unwrap(),
    );
    assert_eq!(audio_of(&edited, A), None);
    assert_eq!(audio_of(&edited, B), Some(exact(6)));
    // Unless the shell offers an exact association for the new pin.
    let offered = state(
        decide(
            &edited,
            5,
            PublicationChange::PublishUnit {
                entry: entry(A, 4).with_audio(Some(exact(9))),
                position: 2,
            },
            50,
        )
        .unwrap(),
    );
    assert_eq!(audio_of(&offered, A), Some(exact(9)));
    // Withdrawing a Unit copies the others with their audio.
    let without_a = state(
        decide(
            &offered,
            6,
            PublicationChange::WithdrawUnit { unit: unit_id(A) },
            60,
        )
        .unwrap(),
    );
    assert_eq!(audio_of(&without_a, B), Some(exact(6)));
    // A republication after WITHDRAWN revives no earlier association.
    let withdrawn = state(decide(&without_a, 7, PublicationChange::Withdraw, 70).unwrap());
    let republished =
        state(decide(&withdrawn, 8, publish(pin(3, 9), vec![entry(B, 2)]), 80).unwrap());
    assert_eq!(audio_of(&republished, B), None);
}

fn unit_pool() -> Vec<String> {
    (0..8)
        .map(|n| format!("40000000-0000-4000-8000-{n:012x}"))
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig { cases: 256, rng_seed: proptest::test_runner::RngSeed::Fixed(20_261_030), ..ProptestConfig::default() })]
    /// Per-Unit publish against an independent list oracle: the Work pin and every other
    /// entry's pin and relative order survive, and the target lands at the asked position.
    #[test]
    fn per_unit_publish_matches_a_list_oracle(
        initial in proptest::sample::subsequence(unit_pool(), 0..=8),
        target in proptest::sample::select(unit_pool()),
        revision in 1_u64..50,
        position in 1_u32..=9,
    ) {
        let entries: Vec<_> = initial.iter().map(|id| entry(id, 1)).collect();
        let active = first(entries.clone());
        let mut oracle: Vec<_> = entries.into_iter().filter(|e| e.unit().as_str() != target).collect();
        let change = PublicationChange::PublishUnit { entry: entry(&target, revision), position };
        let result = decide(&active, 1, change, 20);
        if position as usize > oracle.len() + 1 {
            prop_assert_eq!(result, Err(PublicationError::InvalidPosition));
        } else {
            oracle.insert(position as usize - 1, entry(&target, revision));
            let previous = match &active { PublicationState::Published(p) => p.generation().entries().to_vec(), _ => unreachable!() };
            if oracle == previous {
                prop_assert_eq!(result, Err(PublicationError::Unchanged));
            } else {
                let settled = result.unwrap();
                prop_assert_eq!(settled.published.generation().entries(), &oracle[..]);
                prop_assert_eq!(settled.published.generation().work(), &pin(3, 9));
            }
        }
    }
}
