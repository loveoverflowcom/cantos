use proptest::prelude::*;

use super::*;
use crate::contexts::catalog::domain::{
    ContentUnitKind, NewContentUnit, NewWork, OwnerId, UnitDetails, Visibility, WorkDetails,
    WorkKind,
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
    let duplicated = vec![(unit_id(A), pin(1, 1)), (unit_id(A), pin(2, 2))];
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
