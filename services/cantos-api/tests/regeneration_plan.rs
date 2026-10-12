//! Partial-regeneration planning over the pure `regeneration` and `clip_reuse` modules.
//! Plain tests: no database, no filesystem, no provider. Evidence level: example-tested for the
//! named scenarios, property-tested and differentially-tested (against a naive reference) for
//! the minimal-invalidation laws.
use std::collections::{BTreeMap, BTreeSet};

use cantos_api::contexts::synthesis::{
    fingerprint::{SpeechField, SpeechFingerprint, SpeechInput},
    jobs::ExecutionMode,
    regeneration::{
        plan_regeneration, CacheMiss, EpisodeReason, LineAction, LineChange, PlanGate, PlanInputs,
        ProductionShape, RegenerationPlan, StaleReason, Staleness, COST_BASIS,
    },
    reuse::{
        ArtifactProbe, AudioDigest, ClipCandidate, ClipRefusal, ClipRequest, Refusal,
        RegisteredClip, ReuseDecision, RightsStanding,
    },
};
use cantos_contracts::{
    ProductionBudget, ProductionCurrency, ProductionEstimate, ProductionRate,
    ResolvedPronunciation, ResolvedSynthesisInput, SynthesisPerformance,
};
use proptest::prelude::*;

const OWNER: &str = "alice";

fn budget() -> ProductionBudget {
    ProductionBudget {
        currency: ProductionCurrency::VND,
        limit_minor: 1_000_000,
        rate: Some(ProductionRate {
            reference: "Synthetic fixture, no quoted provider price".into(),
            version: "fixture-v1".into(),
            units_per_charge: 10,
            amount_minor: 1,
        }),
        scope: "private integration checks only".into(),
        territory: "private-planning".into(),
    }
}

fn voice_of(character: &str) -> &'static str {
    match character {
        "an" => "vi-vn-x-south",
        "minh" => "vi-vn-x-central",
        _ => "vi",
    }
}

fn line(id: &str, character: &str, text: &str) -> ResolvedSynthesisInput {
    ResolvedSynthesisInput {
        dialogue_id: id.into(),
        character_id: character.into(),
        text: text.into(),
        language: "vi-VN".into(),
        provider_id: "espeak-ng-local".into(),
        model_id: "espeak-ng-1.52.0".into(),
        voice_id: voice_of(character).into(),
        voice_rights_record_id: "voice-espeak".into(),
        adapter_version: "espeak-ng-local-v1".into(),
        output_contract: "wav-pcm-s16le-mono-22050-v1".into(),
        performance: SynthesisPerformance {
            rate_permille: 1_000,
            pitch_semitones: 0,
            emotion: Some("neutral".into()),
            intensity_permille: Some(0),
        },
        pronunciation: Vec::new(),
    }
}

/// The two-scene sample episode ("Sau buổi diễn", "Bên cửa sổ").
fn episode() -> Vec<(String, Vec<ResolvedSynthesisInput>)> {
    vec![
        (
            "scene-01".into(),
            vec![
                line(
                    "dialogue-01",
                    "narrator",
                    "Ánh đèn cuối cùng còn sáng trên sân khấu.",
                ),
                line("dialogue-02", "an", "Ngày mai, mình có diễn tiếp không?"),
                line("dialogue-03", "minh", "Có chứ. Tôi sẽ chờ cậu ở đây."),
            ],
        ),
        (
            "scene-02".into(),
            vec![line("dialogue-04", "an", "Vọng Đài vẫn sáng… Hẹn gặp lại!")],
        ),
    ]
}

fn shape(scenes: Vec<(String, Vec<ResolvedSynthesisInput>)>) -> ProductionShape {
    ProductionShape::new(scenes, budget()).unwrap()
}

fn fingerprint(input: &ResolvedSynthesisInput) -> SpeechFingerprint {
    SpeechInput::from_resolved(input).unwrap().fingerprint()
}

fn digest_of(fingerprint: &SpeechFingerprint) -> AudioDigest {
    AudioDigest::of_bytes(fingerprint.as_str().as_bytes())
}

fn clip_for(input: &ResolvedSynthesisInput, sequence: i64) -> RegisteredClip {
    let fingerprint = fingerprint(input);
    RegisteredClip {
        id: format!("clip-{sequence}"),
        sequence,
        audio_sha256: digest_of(&fingerprint),
        fingerprint,
        owner_id: OWNER.into(),
        producer: ExecutionMode::LocalEspeak,
        voice_rights_record_id: input.voice_rights_record_id.clone(),
        output_contract: input.output_contract.clone(),
    }
}

type Cache = BTreeMap<SpeechFingerprint, Vec<ClipCandidate>>;
type Spoil = fn(&mut RegisteredClip);

/// Every line of `shape` has one verified, currently readable clip.
fn full_cache(scenes: &[(String, Vec<ResolvedSynthesisInput>)]) -> Cache {
    let mut cache = Cache::new();
    let mut sequence = 0;
    for (_, lines) in scenes {
        for input in lines {
            sequence += 1;
            let clip = clip_for(input, sequence);
            let probe = ArtifactProbe::Present {
                audio_sha256: clip.audio_sha256.clone(),
            };
            cache
                .entry(clip.fingerprint.clone())
                .or_default()
                .push(ClipCandidate { clip, probe });
        }
    }
    cache
}

fn plan(
    previous: Option<&ProductionShape>,
    next: &ProductionShape,
    gate: PlanGate,
    cache: &Cache,
) -> RegenerationPlan {
    plan_regeneration(&PlanInputs {
        previous,
        next,
        gate,
        owner_id: OWNER,
        producer: ExecutionMode::LocalEspeak,
        cache,
    })
}

fn produced(plan: &RegenerationPlan) -> Vec<&str> {
    plan.lines
        .iter()
        .filter(|line| matches!(line.action, LineAction::Produce { .. }))
        .map(|line| line.dialogue_id.as_str())
        .collect()
}

fn reused(plan: &RegenerationPlan) -> Vec<&str> {
    plan.lines
        .iter()
        .filter(|line| matches!(line.action, LineAction::Reuse { .. }))
        .map(|line| line.dialogue_id.as_str())
        .collect()
}

fn stale_scenes(plan: &RegenerationPlan) -> Vec<&str> {
    plan.scenes
        .iter()
        .filter(|scene| scene.mix == Staleness::Stale)
        .map(|scene| scene.scene_id.as_str())
        .collect()
}

fn scene<'a>(
    plan: &'a RegenerationPlan,
    id: &str,
) -> &'a cantos_api::contexts::synthesis::regeneration::SceneImpact {
    plan.scenes
        .iter()
        .find(|scene| scene.scene_id == id)
        .unwrap()
}

// ---- the sample episode, one change at a time ----------------------------------------------

#[test]
fn an_identical_snapshot_with_verified_clips_reuses_everything_and_dispatches_nothing() {
    let scenes = episode();
    let cache = full_cache(&scenes);
    let shape = shape(scenes);
    let plan = plan(Some(&shape), &shape, PlanGate::Open, &cache);
    assert_eq!(reused(&plan).len(), 4);
    assert!(produced(&plan).is_empty());
    assert!(stale_scenes(&plan).is_empty());
    assert!(plan.removed_dialogues.is_empty() && plan.removed_scenes.is_empty());
    assert_eq!(plan.episode.master, Staleness::Unchanged);
    assert_eq!(plan.episode.qc, Staleness::Unchanged);
    assert_eq!(plan.cost.cache_hits, 4);
    assert_eq!(plan.cost.cache_misses, 0);
    assert_eq!(plan.cost.provider_dispatches_expected, 0);
    assert!(plan.lines.iter().all(|line| line.change
        == LineChange::Unchanged {
            placement_changed: false
        }));
}

#[test]
fn editing_one_dialogue_regenerates_only_it_and_remixes_only_its_scene() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    after[0].1[1].text = "Ngày mai, mình có diễn tiếp không nhỉ?".into();
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);

    assert_eq!(produced(&plan), ["dialogue-02"]);
    assert_eq!(reused(&plan), ["dialogue-01", "dialogue-03", "dialogue-04"]);
    let edited = plan
        .lines
        .iter()
        .find(|l| l.dialogue_id == "dialogue-02")
        .unwrap();
    assert_eq!(
        edited.change,
        LineChange::Changed {
            fields: vec![SpeechField::Text]
        }
    );
    assert_eq!(
        edited.action,
        LineAction::Produce {
            cache: CacheMiss::NoCandidate
        }
    );

    assert_eq!(stale_scenes(&plan), ["scene-01"]);
    let stale = scene(&plan, "scene-01");
    assert_eq!(stale.qc, Staleness::Stale);
    assert!(stale.reasons.contains(&StaleReason::SpeechSequenceChanged));
    assert!(stale.reasons.contains(&StaleReason::SpeechRegenerated {
        dialogue_id: "dialogue-02".into()
    }));
    assert_eq!(scene(&plan, "scene-02").qc, Staleness::Unchanged);
    assert_eq!(plan.episode.master, Staleness::Stale);
    assert_eq!(
        plan.episode.reasons,
        [EpisodeReason::SceneStale {
            scene_id: "scene-01".into()
        }]
    );
    assert_eq!((plan.cost.cache_hits, plan.cost.cache_misses), (3, 1));
    assert_eq!(plan.cost.provider_dispatches_expected, 1);
}

#[test]
fn changing_a_characters_voice_regenerates_exactly_that_characters_lines() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    for (_, lines) in &mut after {
        for line in lines.iter_mut().filter(|l| l.character_id == "an") {
            line.voice_id = "vi".into();
        }
    }
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert_eq!(produced(&plan), ["dialogue-02", "dialogue-04"]);
    assert_eq!(reused(&plan), ["dialogue-01", "dialogue-03"]);
    for id in ["dialogue-02", "dialogue-04"] {
        let planned = plan.lines.iter().find(|l| l.dialogue_id == id).unwrap();
        assert_eq!(
            planned.change,
            LineChange::Changed {
                fields: vec![SpeechField::Voice]
            }
        );
    }
    assert_eq!(stale_scenes(&plan), ["scene-01", "scene-02"]);
}

#[test]
fn a_pronunciation_profile_change_regenerates_every_line_that_carries_it() {
    // Business rule 16: a pronunciation change invalidates every line depending on that
    // version; resolved lists are per character, not per occurrence of a surface.
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    for (_, lines) in &mut after {
        for line in lines.iter_mut().filter(|l| l.character_id == "an") {
            line.pronunciation = vec![ResolvedPronunciation {
                surface: "An".into(),
                replacement: "Ân".into(),
            }];
        }
    }
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert_eq!(produced(&plan), ["dialogue-02", "dialogue-04"]);
    assert_eq!(reused(&plan), ["dialogue-01", "dialogue-03"]);
}

#[test]
fn a_synthesis_setting_change_names_the_changed_field_and_regenerates_the_line() {
    for (field, mutate) in [
        (
            SpeechField::Rate,
            (|l: &mut ResolvedSynthesisInput| l.performance.rate_permille = 900)
                as fn(&mut ResolvedSynthesisInput),
        ),
        (SpeechField::Pitch, |l| l.performance.pitch_semitones = 1),
        (SpeechField::Emotion, |l| l.performance.emotion = None),
        (SpeechField::Intensity, |l| {
            l.performance.intensity_permille = Some(5)
        }),
        (SpeechField::Adapter, |l| {
            l.adapter_version = "espeak-ng-local-v2".into()
        }),
        (SpeechField::OutputContract, |l| {
            l.output_contract = "wav-pcm-s16le-mono-24000-v1".into()
        }),
        (SpeechField::Language, |l| l.language = "vi".into()),
    ] {
        let before = episode();
        let cache = full_cache(&before);
        let mut after = episode();
        mutate(&mut after[1].1[0]);
        let (previous, next) = (shape(before), shape(after));
        let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
        assert_eq!(produced(&plan), ["dialogue-04"], "{field:?}");
        let planned = plan
            .lines
            .iter()
            .find(|l| l.dialogue_id == "dialogue-04")
            .unwrap();
        assert_eq!(
            planned.change,
            LineChange::Changed {
                fields: vec![field]
            }
        );
    }
}

#[test]
fn reordering_or_moving_dialogue_keeps_every_clip_and_only_remixes_the_scenes_it_touches() {
    let before = episode();
    let cache = full_cache(&before);

    let mut swapped = episode();
    swapped[0].1.swap(1, 2);
    let (previous, next) = (shape(before.clone()), shape(swapped));
    let plan_swap = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert!(produced(&plan_swap).is_empty());
    assert_eq!(plan_swap.cost.provider_dispatches_expected, 0);
    assert_eq!(stale_scenes(&plan_swap), ["scene-01"]);
    assert_eq!(
        scene(&plan_swap, "scene-01").reasons,
        [StaleReason::SpeechSequenceChanged]
    );

    let mut moved = episode();
    let carried = moved[0].1.remove(2);
    moved[1].1.insert(0, carried);
    let next = shape(moved);
    let plan_move = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert!(produced(&plan_move).is_empty());
    assert_eq!(stale_scenes(&plan_move), ["scene-01", "scene-02"]);
    let relocated = plan_move
        .lines
        .iter()
        .find(|l| l.dialogue_id == "dialogue-03")
        .unwrap();
    assert_eq!(
        relocated.change,
        LineChange::Unchanged {
            placement_changed: true
        }
    );
    assert!(matches!(relocated.action, LineAction::Reuse { .. }));
}

#[test]
fn a_new_revision_with_no_effective_change_in_a_scene_leaves_that_scene_untouched() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    after[1].1[0].text = "Vọng Đài vẫn sáng… Hẹn mai gặp lại!".into();
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert_eq!(stale_scenes(&plan), ["scene-02"]);
    assert_eq!(scene(&plan, "scene-01").mix, Staleness::Unchanged);
    assert_eq!(scene(&plan, "scene-01").qc, Staleness::Unchanged);
}

#[test]
fn added_and_removed_dialogue_and_scenes_are_reported() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    after[0].1.remove(0);
    after.push((
        "scene-03".into(),
        vec![line("dialogue-05", "an", "Chúc ngủ ngon.")],
    ));
    after.remove(1);
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert_eq!(plan.removed_dialogues, ["dialogue-01", "dialogue-04"]);
    assert_eq!(plan.removed_scenes, ["scene-02"]);
    assert_eq!(produced(&plan), ["dialogue-05"]);
    let added = plan
        .lines
        .iter()
        .find(|l| l.dialogue_id == "dialogue-05")
        .unwrap();
    assert_eq!(added.change, LineChange::Added);
    assert_eq!(scene(&plan, "scene-03").reasons[0], StaleReason::SceneAdded);
    assert!(plan.episode.reasons.contains(&EpisodeReason::SceneRemoved {
        scene_id: "scene-02".into()
    }));
}

#[test]
fn scene_order_changes_stale_the_master_without_touching_scene_speech() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    after.swap(0, 1);
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert!(produced(&plan).is_empty());
    assert!(stale_scenes(&plan).is_empty());
    assert_eq!(plan.episode.master, Staleness::Stale);
    assert_eq!(plan.episode.reasons, [EpisodeReason::SceneOrderChanged]);
}

// ---- the cache decides, never the history ---------------------------------------------------

#[test]
fn a_first_production_still_reuses_clips_registered_by_earlier_runs() {
    let scenes = episode();
    let cache = full_cache(&scenes);
    let next = shape(scenes);
    let plan = plan(None, &next, PlanGate::Open, &cache);
    assert_eq!(reused(&plan).len(), 4);
    assert!(plan.lines.iter().all(|l| l.change == LineChange::Added));
    assert_eq!(plan.episode.reasons[0], EpisodeReason::NoPreviousProduction);
    assert!(plan
        .scenes
        .iter()
        .all(|s| s.reasons.contains(&StaleReason::SceneAdded)));
}

#[test]
fn reverting_an_edit_is_a_cache_hit_not_a_new_synthesis() {
    let original = episode();
    let mut edited = episode();
    edited[0].1[1].text = "Ngày mai, mình có diễn tiếp không nhỉ?".into();
    // History holds clips for both wordings.
    let mut cache = full_cache(&original);
    for (fingerprint, candidates) in full_cache(&edited) {
        cache.entry(fingerprint).or_default().extend(candidates);
    }
    let (previous, next) = (shape(edited), shape(original));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert!(produced(&plan).is_empty(), "{:?}", plan.lines);
}

#[test]
fn an_unchanged_line_whose_clip_cannot_be_verified_is_regenerated_with_the_reason() {
    let scenes = episode();
    let previous = shape(scenes.clone());
    let next = shape(scenes.clone());
    let probes = [
        (ArtifactProbe::Missing, Refusal::Unavailable),
        (ArtifactProbe::Invalid, Refusal::ArtifactInvalid),
        (ArtifactProbe::NotProbed, Refusal::Unverified),
        (
            ArtifactProbe::Present {
                audio_sha256: AudioDigest::of_bytes(b"tampered"),
            },
            Refusal::ChecksumMismatch,
        ),
    ];
    for (probe, reason) in probes {
        let mut cache = full_cache(&scenes);
        let target = fingerprint(&scenes[0].1[1]);
        for candidate in cache.get_mut(&target).unwrap() {
            candidate.probe = probe.clone();
        }
        let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
        assert_eq!(produced(&plan), ["dialogue-02"], "{reason:?}");
        let planned = plan
            .lines
            .iter()
            .find(|l| l.dialogue_id == "dialogue-02")
            .unwrap();
        // Nothing changed in the script; the cache is the only reason to synthesize.
        assert_eq!(
            planned.change,
            LineChange::Unchanged {
                placement_changed: false
            }
        );
        assert_eq!(
            planned.action,
            LineAction::Produce {
                cache: CacheMiss::Refused {
                    refusals: vec![ClipRefusal {
                        clip_id: "clip-2".into(),
                        reason
                    }]
                }
            }
        );
        // The previous sequence is intact, but new audio means the mix hashes new bytes.
        assert_eq!(
            scene(&plan, "scene-01").reasons,
            [StaleReason::SpeechRegenerated {
                dialogue_id: "dialogue-02".into()
            }]
        );
        assert_eq!(plan.cost.cache_hits, 3);
    }
}

#[test]
fn clips_of_another_owner_voice_record_producer_or_contract_are_never_hits() {
    let scenes = episode();
    let next = shape(scenes.clone());
    let spoilers: [(&str, Spoil); 4] = [
        ("owner", |c| c.owner_id = "mallory".into()),
        ("voice record", |c| {
            c.voice_rights_record_id = "voice-other".into()
        }),
        ("producer", |c| c.producer = ExecutionMode::SyntheticFake),
        ("contract", |c| {
            c.output_contract = "wav-pcm-s16le-mono-24000-v1".into()
        }),
    ];
    for (what, spoil) in spoilers {
        let mut cache = full_cache(&scenes);
        for candidates in cache.values_mut() {
            for candidate in candidates {
                spoil(&mut candidate.clip);
            }
        }
        let plan = plan(None, &next, PlanGate::Open, &cache);
        assert_eq!(plan.cost.cache_hits, 0, "{what}");
        assert_eq!(plan.cost.cache_misses, 4, "{what}");
        assert!(plan.lines.iter().all(|l| matches!(
            &l.action,
            LineAction::Produce {
                cache: CacheMiss::Refused { .. }
            }
        )));
    }
}

#[test]
fn a_closed_gate_blocks_reuse_and_production_alike_and_expects_no_dispatch() {
    let scenes = episode();
    let cache = full_cache(&scenes);
    let next = shape(scenes);
    let gate = PlanGate::Blocked {
        findings: vec!["rights_expired".into()],
    };
    let plan = plan(None, &next, gate.clone(), &cache);
    assert_eq!(plan.gate, gate);
    assert!(plan.lines.iter().all(|l| l.action == LineAction::Blocked));
    assert_eq!(plan.cost.blocked, 4);
    assert_eq!((plan.cost.cache_hits, plan.cost.cache_misses), (0, 0));
    assert_eq!(plan.cost.provider_dispatches_expected, 0);
    assert!(plan.scenes.iter().all(|s| s
        .reasons
        .iter()
        .any(|r| matches!(r, StaleReason::SpeechBlocked { .. }))));
}

#[test]
fn a_model_without_a_pin_is_produced_with_a_stated_reason_never_reused() {
    let mut scenes = episode();
    scenes[1].1[0].provider_id = "google-cloud-tts".into();
    scenes[1].1[0].model_id = "vi-VN-Chirp3-HD-Aoede".into();
    let cache = full_cache(&episode());
    let next = shape(scenes);
    let plan = plan(None, &next, PlanGate::Open, &cache);
    let planned = plan
        .lines
        .iter()
        .find(|l| l.dialogue_id == "dialogue-04")
        .unwrap();
    assert_eq!(planned.fingerprint, None);
    assert_eq!(
        planned.action,
        LineAction::Produce {
            cache: CacheMiss::NoFingerprint {
                code: "model_not_pinned".into()
            }
        }
    );
    assert_eq!(planned.change, LineChange::Added);
}

#[test]
fn costs_separate_what_is_synthesized_from_what_the_cache_avoids() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    after[0].1[1].text = "x".repeat(25); // 25 scalars at 1 minor unit per 10, rounded up: 3
    let (previous, next) = (shape(before), shape(after));
    let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert_eq!(plan.cost.basis, COST_BASIS);
    let units = |text: &str| text.chars().count() as u64;
    let hit_units: u64 = ["dialogue-01", "dialogue-03", "dialogue-04"]
        .iter()
        .map(|id| {
            let text = &episode()
                .iter()
                .flat_map(|(_, l)| l.clone())
                .find(|l| l.dialogue_id == *id)
                .unwrap()
                .text;
            units(text)
        })
        .sum();
    let expected_hit_minor: u64 = ["dialogue-01", "dialogue-03", "dialogue-04"]
        .iter()
        .map(|id| {
            let text = episode()
                .iter()
                .flat_map(|(_, l)| l.clone())
                .find(|l| l.dialogue_id == *id)
                .unwrap()
                .text;
            units(&text).div_ceil(10)
        })
        .sum();
    match plan.cost.miss_estimate {
        ProductionEstimate::Known {
            amount_minor,
            units,
            currency,
            ..
        } => {
            assert_eq!(
                (amount_minor, units, currency),
                (3, 25, ProductionCurrency::VND)
            );
        }
        other => panic!("{other:?}"),
    }
    match plan.cost.hit_estimate_avoided {
        ProductionEstimate::Known {
            amount_minor,
            units,
            ..
        } => {
            assert_eq!((amount_minor, units), (expected_hit_minor, hit_units));
        }
        other => panic!("{other:?}"),
    }
}

#[test]
fn an_unrecorded_planning_rate_is_unavailable_not_zero() {
    let scenes = episode();
    let mut unpriced = budget();
    unpriced.rate = None;
    let next = ProductionShape::new(scenes, unpriced).unwrap();
    let plan = plan(None, &next, PlanGate::Open, &Cache::new());
    assert_eq!(
        plan.cost.miss_estimate,
        ProductionEstimate::Unavailable {
            reason: "planning_rate_unrecorded".into()
        }
    );
}

#[test]
fn plans_are_deterministic_and_serialize_to_the_documented_shape() {
    let before = episode();
    let cache = full_cache(&before);
    let mut after = episode();
    after[0].1[1].text = "Ngày mai, mình có diễn tiếp không nhỉ?".into();
    let (previous, next) = (shape(before), shape(after));
    let first = plan(Some(&previous), &next, PlanGate::Open, &cache);
    assert_eq!(first, plan(Some(&previous), &next, PlanGate::Open, &cache));
    let json = serde_json::to_value(&first).unwrap();
    assert_eq!(json["version"], "regeneration-plan-v1");
    assert_eq!(json["gate"]["status"], "open");
    assert_eq!(json["cost"]["basis"], "planning_estimate_not_a_charge");
    assert_eq!(json["lines"][1]["change"]["kind"], "changed");
    assert_eq!(json["lines"][1]["change"]["fields"][0], "text");
    assert_eq!(json["lines"][1]["action"]["cache"]["kind"], "no_candidate");
    assert_eq!(json["lines"][0]["action"]["kind"], "reuse");
}

#[test]
fn duplicate_scene_or_dialogue_identifiers_cannot_form_a_shape() {
    use cantos_api::contexts::synthesis::regeneration::ShapeError;
    let mut duplicate_scene = episode();
    duplicate_scene[1].0 = "scene-01".into();
    assert_eq!(
        ProductionShape::new(duplicate_scene, budget()).unwrap_err(),
        ShapeError::DuplicateScene
    );
    let mut duplicate_line = episode();
    duplicate_line[1].1[0].dialogue_id = "dialogue-01".into();
    assert_eq!(
        ProductionShape::new(duplicate_line, budget()).unwrap_err(),
        ShapeError::DuplicateDialogue
    );
}

// ---- laws over generated edits --------------------------------------------------------------

/// A naive reference: recompute every fingerprint from scratch and compare with the cache.
fn reference_produce_set(
    next: &[(String, Vec<ResolvedSynthesisInput>)],
    cached: &BTreeSet<SpeechFingerprint>,
) -> BTreeSet<String> {
    next.iter()
        .flat_map(|(_, lines)| lines.iter())
        .filter(|l| !cached.contains(&fingerprint(l)))
        .map(|l| l.dialogue_id.clone())
        .collect()
}

#[derive(Debug, Clone)]
enum Edit {
    Text {
        scene: usize,
        line: usize,
        suffix: u8,
    },
    Voice {
        scene: usize,
        line: usize,
    },
    Swap {
        scene: usize,
    },
    Move {
        from: usize,
    },
    Remove {
        scene: usize,
        line: usize,
    },
}

fn apply(edits: &[Edit], scenes: &mut [(String, Vec<ResolvedSynthesisInput>)]) {
    for edit in edits {
        match *edit {
            Edit::Text {
                scene,
                line,
                suffix,
            } => {
                let scene = scene % scenes.len();
                let index = line % scenes[scene].1.len().max(1);
                if let Some(target) = scenes[scene].1.get_mut(index) {
                    target.text.push((b'a' + suffix % 26) as char);
                }
            }
            Edit::Voice { scene, line } => {
                let scene = scene % scenes.len();
                let index = line % scenes[scene].1.len().max(1);
                if let Some(target) = scenes[scene].1.get_mut(index) {
                    target.voice_id.push('x');
                }
            }
            Edit::Swap { scene } => {
                let scene = scene % scenes.len();
                if scenes[scene].1.len() >= 2 {
                    scenes[scene].1.swap(0, 1);
                }
            }
            Edit::Move { from } => {
                let from = from % scenes.len();
                let to = (from + 1) % scenes.len();
                if from != to && !scenes[from].1.is_empty() {
                    let moved = scenes[from].1.remove(0);
                    scenes[to].1.push(moved);
                }
            }
            Edit::Remove { scene, line } => {
                let scene = scene % scenes.len();
                if scenes[scene].1.len() > 1 {
                    let index = line % scenes[scene].1.len();
                    scenes[scene].1.remove(index);
                }
            }
        }
    }
}

fn edit() -> impl Strategy<Value = Edit> {
    prop_oneof![
        (0..2usize, 0..4usize, any::<u8>()).prop_map(|(scene, line, suffix)| Edit::Text {
            scene,
            line,
            suffix
        }),
        (0..2usize, 0..4usize).prop_map(|(scene, line)| Edit::Voice { scene, line }),
        (0..2usize).prop_map(|scene| Edit::Swap { scene }),
        (0..2usize).prop_map(|from| Edit::Move { from }),
        (0..2usize, 0..4usize).prop_map(|(scene, line)| Edit::Remove { scene, line }),
    ]
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(512))]

    #[test]
    fn only_lines_without_a_verified_clip_are_ever_synthesized(
        edits in proptest::collection::vec(edit(), 0..6),
    ) {
        let before = episode();
        let cache = full_cache(&before);
        let cached: BTreeSet<_> = cache.keys().cloned().collect();
        let mut after = episode();
        apply(&edits, &mut after);
        let (previous, next) = (shape(before), shape(after.clone()));
        let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);

        let expected = reference_produce_set(&after, &cached);
        let actual: BTreeSet<String> = produced(&plan).into_iter().map(str::to_owned).collect();
        prop_assert_eq!(actual, expected);
        prop_assert_eq!(
            plan.cost.cache_hits + plan.cost.cache_misses + plan.cost.blocked,
            plan.lines.len() as u64
        );
        prop_assert_eq!(plan.cost.provider_dispatches_expected, plan.cost.cache_misses);
    }

    #[test]
    fn a_scene_is_remixed_only_when_its_speech_or_audio_changes(
        edits in proptest::collection::vec(edit(), 0..6),
    ) {
        let before = episode();
        let cache = full_cache(&before);
        let mut after = episode();
        apply(&edits, &mut after);
        let (previous, next) = (shape(before.clone()), shape(after.clone()));
        let plan = plan(Some(&previous), &next, PlanGate::Open, &cache);
        for (scene_id, lines) in &after {
            let fingerprints: Vec<_> = lines.iter().map(fingerprint).collect();
            let before_prints: Option<Vec<_>> = before
                .iter()
                .find(|(id, _)| id == scene_id)
                .map(|(_, l)| l.iter().map(fingerprint).collect());
            let untouched = before_prints.as_ref() == Some(&fingerprints);
            let impact = scene(&plan, scene_id);
            prop_assert_eq!(
                impact.mix == Staleness::Unchanged,
                untouched,
                "scene {} with edits {:?}", scene_id, edits
            );
            prop_assert_eq!(impact.qc, impact.mix);
        }
    }

    #[test]
    fn planning_an_unchanged_snapshot_is_a_total_reuse_whatever_the_cache_order(
        rotate in 0usize..4,
    ) {
        let scenes = episode();
        let mut cache = full_cache(&scenes);
        // Duplicate every clip under a later sequence in a rotated order: the oldest still wins.
        for candidates in cache.values_mut() {
            let mut extra = candidates[0].clone();
            extra.clip.id = format!("{}-dup", extra.clip.id);
            extra.clip.sequence += 100;
            candidates.insert(0, extra);
            let by = rotate % candidates.len();
            candidates.rotate_left(by);
        }
        let shape = shape(scenes);
        let plan = plan(Some(&shape), &shape, PlanGate::Open, &cache);
        prop_assert_eq!(reused(&plan).len(), 4);
        for line in &plan.lines {
            if let LineAction::Reuse { clip_id, .. } = &line.action {
                prop_assert!(!clip_id.ends_with("-dup"), "{}", clip_id);
            }
        }
    }
}

#[test]
fn resolve_reuse_is_the_single_source_of_hit_semantics_for_the_plan() {
    // The plan's per-line action must equal a direct `resolve_reuse` over the same facts.
    let scenes = episode();
    let cache = full_cache(&scenes);
    let next = shape(scenes.clone());
    let plan = plan(None, &next, PlanGate::Open, &cache);
    for planned in &plan.lines {
        let input = scenes
            .iter()
            .flat_map(|(_, l)| l)
            .find(|l| l.dialogue_id == planned.dialogue_id)
            .unwrap();
        let fingerprint = fingerprint(input);
        let decision = cantos_api::contexts::synthesis::reuse::resolve_reuse(
            &ClipRequest {
                fingerprint: fingerprint.clone(),
                owner_id: OWNER.into(),
                producer: ExecutionMode::LocalEspeak,
                voice_rights_record_id: input.voice_rights_record_id.clone(),
                output_contract: input.output_contract.clone(),
            },
            RightsStanding::Current,
            &cache[&fingerprint],
        );
        let ReuseDecision::Reuse {
            clip_id,
            audio_sha256,
        } = decision
        else {
            panic!("expected a hit for {}", planned.dialogue_id)
        };
        assert_eq!(
            planned.action,
            LineAction::Reuse {
                clip_id,
                audio_sha256
            }
        );
    }
}
