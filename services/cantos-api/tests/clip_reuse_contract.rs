//! Clip-reuse v1 contract: the published JSON Schema, a literal plan fixture whose fingerprints
//! come from the independent oracle vectors, and the planner that must reproduce that fixture.
//! Plain tests: no database, no filesystem, no provider.
use std::collections::BTreeMap;

use cantos_api::{
    clip_reuse::{ArtifactProbe, AudioDigest, ClipCandidate, RegisteredClip},
    jobs::ExecutionMode,
    postgres::durable::{ObjectProbes, PlanReport},
    regeneration::{plan_regeneration, PlanGate, PlanInputs, ProductionShape},
    speech_fingerprint::{SpeechFingerprint, SpeechInput},
};
use cantos_contracts::{
    ProductionBudget, ProductionCurrency, ProductionRate, ResolvedSynthesisInput,
};
use jsonschema::Validator;
use serde_json::{json, Value};

const SCHEMA: &str = include_str!("../../../packages/contracts/schema/clip-reuse/v1.schema.json");
const PLAN: &str =
    include_str!("../../../packages/contracts/fixtures/clip-reuse/v1/plan-edit-one-line.json");
const VECTORS: &str =
    include_str!("../../../packages/contracts/fixtures/clip-reuse/v1/speech-vectors.json");

// Independent literals: also written by hand in the fixture. A mismatch fails the comparison.
const CLIP_1: &str = "b6d609598c8ac7f2c529911fde1b24d5be8033afc130369de74e260d35aa3c5c";
const CLIP_3: &str = "a6255c9c88c7e9c5b6daa48ee3be687663b22489e942ee65c86281623f0da1c8";

fn schema() -> Value {
    serde_json::from_str(SCHEMA).unwrap()
}

fn validator() -> Validator {
    jsonschema::validator_for(&schema()).unwrap()
}

fn vector(id: &str) -> ResolvedSynthesisInput {
    let file: Value = serde_json::from_str(VECTORS).unwrap();
    let entry = file["vectors"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap_or_else(|| panic!("vector {id}"));
    serde_json::from_value(entry["input"].clone()).unwrap()
}

fn with_dialogue(id: &str, dialogue: &str) -> ResolvedSynthesisInput {
    let mut input = vector(id);
    input.dialogue_id = dialogue.into();
    input
}

fn budget(rate: bool) -> ProductionBudget {
    ProductionBudget {
        currency: ProductionCurrency::VND,
        limit_minor: 1_000_000,
        rate: rate.then(|| ProductionRate {
            reference: "Synthetic fixture, no quoted provider price".into(),
            version: "fixture-v1".into(),
            units_per_charge: 10,
            amount_minor: 1,
        }),
        scope: "private integration checks only".into(),
        territory: "private-planning".into(),
    }
}

fn clip(id: &str, sequence: i64, input: &ResolvedSynthesisInput, digest: &str) -> ClipCandidate {
    let audio_sha256: AudioDigest = digest.parse().unwrap();
    ClipCandidate {
        clip: RegisteredClip {
            id: id.into(),
            sequence,
            fingerprint: SpeechInput::from_resolved(input).unwrap().fingerprint(),
            owner_id: "alice".into(),
            producer: ExecutionMode::LocalEspeak,
            voice_rights_record_id: input.voice_rights_record_id.clone(),
            output_contract: input.output_contract.clone(),
            audio_sha256: audio_sha256.clone(),
        },
        probe: ArtifactProbe::Present { audio_sha256 },
    }
}

type Cache = BTreeMap<SpeechFingerprint, Vec<ClipCandidate>>;
type Mutation = fn(&mut Value);
type Spoil = fn(&mut ClipCandidate);

fn cache_of(candidates: Vec<(ClipCandidate, &ResolvedSynthesisInput)>) -> Cache {
    let mut cache = Cache::new();
    for (candidate, input) in candidates {
        cache
            .entry(SpeechInput::from_resolved(input).unwrap().fingerprint())
            .or_default()
            .push(candidate);
    }
    cache
}

fn report(plan: cantos_api::regeneration::RegenerationPlan) -> Value {
    serde_json::to_value(PlanReport {
        script_id: "11111111-1111-4111-8111-111111111111".into(),
        previous_snapshot_id: Some("22222222-2222-4222-8222-222222222222".into()),
        next_snapshot_id: "33333333-3333-4333-8333-333333333333".into(),
        execution_mode: ExecutionMode::LocalEspeak,
        object_probes: ObjectProbes::Performed,
        plan,
    })
    .unwrap()
}

#[test]
fn the_published_schema_is_valid_and_accepts_the_committed_plan() {
    assert!(jsonschema::draft202012::meta::is_valid(&schema()));
    let fixture: Value = serde_json::from_str(PLAN).unwrap();
    validator().validate(&fixture).unwrap();
}

#[test]
fn the_planner_reproduces_the_hand_written_fixture_exactly() {
    // One edited line: dialogue-02 changes voice and wording; everything else is unchanged.
    let previous = ProductionShape::new(
        vec![
            (
                "scene-01".into(),
                vec![
                    with_dialogue("base", "dialogue-01"),
                    with_dialogue("voice_changed", "dialogue-02"),
                ],
            ),
            (
                "scene-02".into(),
                vec![with_dialogue("language_changed", "dialogue-03")],
            ),
        ],
        budget(true),
    )
    .unwrap();
    let next = ProductionShape::new(
        vec![
            (
                "scene-01".into(),
                vec![
                    with_dialogue("base", "dialogue-01"),
                    with_dialogue("text_changed", "dialogue-02"),
                ],
            ),
            (
                "scene-02".into(),
                vec![with_dialogue("language_changed", "dialogue-03")],
            ),
        ],
        budget(true),
    )
    .unwrap();
    let (base, voice, language) = (
        vector("base"),
        vector("voice_changed"),
        vector("language_changed"),
    );
    let cache = cache_of(vec![
        (clip("clip-0001", 1, &base, CLIP_1), &base),
        (
            clip(
                "clip-0002",
                2,
                &voice,
                "576cdc2f3e57628f8b9785c736a1022e02d1ac20493af8297a3183ec464356a8",
            ),
            &voice,
        ),
        (clip("clip-0003", 3, &language, CLIP_3), &language),
    ]);
    let plan = plan_regeneration(&PlanInputs {
        previous: Some(&previous),
        next: &next,
        gate: PlanGate::Open,
        owner_id: "alice",
        producer: ExecutionMode::LocalEspeak,
        cache: &cache,
    });
    let fixture: Value = serde_json::from_str(PLAN).unwrap();
    assert_eq!(report(plan), fixture);
}

#[test]
fn fixture_fingerprints_are_the_oracle_vectors_not_values_the_rust_code_produced() {
    let vectors: Value = serde_json::from_str(VECTORS).unwrap();
    let oracle = |id: &str| {
        vectors["vectors"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == id)
            .unwrap()["expected"]["fingerprint"]
            .clone()
    };
    let fixture: Value = serde_json::from_str(PLAN).unwrap();
    let lines = fixture["plan"]["lines"].as_array().unwrap();
    assert_eq!(lines[0]["fingerprint"], oracle("base"));
    assert_eq!(lines[1]["fingerprint"], oracle("text_changed"));
    assert_eq!(lines[2]["fingerprint"], oracle("language_changed"));
}

#[test]
fn the_schema_rejects_fabricated_or_malformed_reports() {
    let validator = validator();
    let fixture: Value = serde_json::from_str(PLAN).unwrap();
    let mutations: [(&str, Mutation); 14] = [
        ("unknown report field", |v| v["approved"] = json!(true)),
        ("unknown plan field", |v| v["plan"]["publish"] = json!(true)),
        ("unknown line field", |v| {
            v["plan"]["lines"][0]["cached"] = json!(true)
        }),
        ("wrong version", |v| {
            v["plan"]["version"] = json!("regeneration-plan-v2")
        }),
        ("bad fingerprint scheme", |v| {
            v["plan"]["lines"][0]["fingerprint"] =
                json!("durable-input-v1:sha256:".to_owned() + &"a".repeat(64))
        }),
        ("uppercase digest", |v| {
            v["plan"]["lines"][0]["action"]["audio_sha256"] = json!("A".repeat(64))
        }),
        ("unknown action kind", |v| {
            v["plan"]["lines"][0]["action"] = json!({"kind": "dispatch"})
        }),
        ("reuse without a clip", |v| {
            v["plan"]["lines"][0]["action"] = json!({"kind": "reuse"})
        }),
        ("empty changed fields", |v| {
            v["plan"]["lines"][1]["change"] = json!({"kind": "changed", "fields": []})
        }),
        ("unknown field name", |v| {
            v["plan"]["lines"][1]["change"]["fields"] = json!(["dialogue_id"])
        }),
        ("cost basis not an estimate", |v| {
            v["plan"]["cost"]["basis"] = json!("charge")
        }),
        ("negative cache hits", |v| {
            v["plan"]["cost"]["cache_hits"] = json!(-1)
        }),
        ("unknown execution mode", |v| {
            v["execution_mode"] = json!("google")
        }),
        ("missing object probes", |v| {
            v.as_object_mut().unwrap().remove("object_probes");
        }),
    ];
    for (name, mutate) in mutations {
        let mut changed = fixture.clone();
        mutate(&mut changed);
        assert!(!validator.is_valid(&changed), "{name} must be rejected");
    }
}

#[test]
fn every_plan_shape_the_planner_can_emit_validates_against_the_schema() {
    let validator = validator();
    let base = vector("base");
    let other = vector("text_changed");
    let unpinned = {
        let file: Value = serde_json::from_str(VECTORS).unwrap();
        let entry = file["rejected"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["id"] == "google_voice_unpinned")
            .unwrap();
        let mut input: ResolvedSynthesisInput =
            serde_json::from_value(entry["input"].clone()).unwrap();
        input.dialogue_id = "dialogue-03".into();
        input
    };
    let shape = |scenes: Vec<(&str, Vec<ResolvedSynthesisInput>)>, rate: bool| {
        ProductionShape::new(
            scenes
                .into_iter()
                .map(|(id, lines)| (id.to_owned(), lines))
                .collect(),
            budget(rate),
        )
        .unwrap()
    };
    let scenes = || {
        vec![
            ("scene-01", vec![with_dialogue("base", "dialogue-01")]),
            (
                "scene-02",
                vec![with_dialogue("text_changed", "dialogue-02")],
            ),
        ]
    };
    let digest = CLIP_1;
    let spoilers: [(&str, Spoil); 8] = [
        ("fingerprint", |c| {
            c.clip.fingerprint = SpeechInput::from_resolved(&vector("rate_changed"))
                .unwrap()
                .fingerprint()
        }),
        ("owner", |c| c.clip.owner_id = "mallory".into()),
        ("voice record", |c| {
            c.clip.voice_rights_record_id = "other".into()
        }),
        ("producer", |c| {
            c.clip.producer = ExecutionMode::SyntheticFake
        }),
        ("contract", |c| {
            c.clip.output_contract = "other-contract".into()
        }),
        ("not probed", |c| c.probe = ArtifactProbe::NotProbed),
        ("missing", |c| c.probe = ArtifactProbe::Missing),
        ("invalid", |c| c.probe = ArtifactProbe::Invalid),
    ];
    let mut reports = Vec::new();
    for (name, spoil) in spoilers {
        let mut candidate = clip("clip-x", 1, &base, digest);
        spoil(&mut candidate);
        let cache = cache_of(vec![(candidate, &base)]);
        let next = shape(scenes(), true);
        reports.push((
            name,
            plan_regeneration(&PlanInputs {
                previous: None,
                next: &next,
                gate: PlanGate::Open,
                owner_id: "alice",
                producer: ExecutionMode::LocalEspeak,
                cache: &cache,
            }),
        ));
    }
    let mut mismatch = clip("clip-y", 1, &base, digest);
    mismatch.probe = ArtifactProbe::Present {
        audio_sha256: AudioDigest::of_bytes(b"tampered"),
    };
    let tampered_cache = cache_of(vec![(mismatch, &base)]);
    let empty = Cache::new();
    let previous = shape(
        vec![
            (
                "scene-01",
                vec![with_dialogue("base", "dialogue-01"), other.clone()],
            ),
            (
                "scene-09",
                vec![with_dialogue("language_changed", "dialogue-09")],
            ),
        ],
        true,
    );
    let next = shape(scenes(), true);
    let unpriced = shape(scenes(), false);
    let with_unpinned = shape(vec![("scene-01", vec![unpinned])], true);
    let swapped = shape(
        vec![
            (
                "scene-02",
                vec![with_dialogue("text_changed", "dialogue-02")],
            ),
            ("scene-01", vec![with_dialogue("base", "dialogue-01")]),
        ],
        true,
    );
    let run = |previous: Option<&ProductionShape>,
               next: &ProductionShape,
               gate: PlanGate,
               cache: &Cache| {
        plan_regeneration(&PlanInputs {
            previous,
            next,
            gate,
            owner_id: "alice",
            producer: ExecutionMode::LocalEspeak,
            cache,
        })
    };
    reports.push((
        "tampered digest",
        run(None, &next, PlanGate::Open, &tampered_cache),
    ));
    reports.push((
        "removed scene and dialogue",
        run(Some(&previous), &next, PlanGate::Open, &empty),
    ));
    reports.push((
        "blocked gate",
        run(
            None,
            &next,
            PlanGate::Blocked {
                findings: vec!["rights_expired".into(), "approval_not_current".into()],
            },
            &empty,
        ),
    ));
    reports.push(("unpriced", run(None, &unpriced, PlanGate::Open, &empty)));
    reports.push((
        "unpinned model",
        run(None, &with_unpinned, PlanGate::Open, &empty),
    ));
    reports.push((
        "scene order",
        run(Some(&next), &swapped, PlanGate::Open, &empty),
    ));
    for (name, plan) in reports {
        let value = report(plan);
        let errors: Vec<String> = validator
            .iter_errors(&value)
            .map(|error| error.to_string())
            .collect();
        assert!(errors.is_empty(), "{name}: {errors:?}\n{value}");
    }
}
