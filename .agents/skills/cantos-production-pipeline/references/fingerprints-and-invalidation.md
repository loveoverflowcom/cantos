# Fingerprints and invalidation

> **Current scope (ADR 0018):** Speech fingerprints and regeneration are optional **external producer** techniques. Cantos does not maintain a TTS cache, regenerate clips or compute server-side speech jobs. The remaining details below describe historical producer-side algorithm guidance only.


> **Scope.** Compose canonical input fingerprints for dialogue speech, scene mixes, episode
> masters and release renditions; keep the dependency graph dialogue → scene mix → episode master
> → release outputs; plan partial regeneration with a pure `plan_regeneration`; decide cache reuse
> from fingerprint, scope, checksum and availability; handle unpinnable models with an explicit
> refresh; and test fingerprints with golden values, determinism properties and per-field
> mutation. Use when a cache key, an effective synthesis input, a reuse rule, an invalidation
> reason or "why did this line regenerate?" is involved.

> **Implemented subset.** The bounded [clip reuse v1 contract](../../../../services/cantos-server/contracts/historical/clip-reuse-v1.md) implements
> sections 2, 4 and 5 for dialogue speech only, with these deliberate differences from the sketches below: the
> fingerprint is `speech-v1` over `ResolvedSynthesisInput` (no separate voice-profile or model-revision types; the
> exact `model_id` is the pin, and an unpinned model has no fingerprint); reuse also requires the same *producer*
> (a synthetic fixture never satisfies a real engine) and the same voice-rights record; and `plan_regeneration`
> reports scene mix/QC staleness and cost but persists nothing. Section 3 (mix, master and rendition
> fingerprints) and the refresh epoch are still proposals.

The product rules live in [Script IR § TTS cache boundary](../../../../docs/architecture/script-ir.md#tts-cache-boundary)
(the field lists), [pipeline § Cache and partial regeneration](../../../../docs/product/production-pipeline.md#cache-and-partial-regeneration)
and [business rules § Production, caching and costs](../../../../docs/product/business-rules.md#production-caching-and-costs)
(rules 14–16). This reference does not re-list which fields count; it shows how to make "every
effective input, and nothing else" checkable. Every type is a **proposal**.

## 1. Who supplies what

| Input | Supplied by | Notes |
|---|---|---|
| effective spoken content per line (text, language, line delivery, line pronunciation overrides) as canonical bytes | [`cantos-script-ir`](../../cantos-script-ir/SKILL.md) `SpokenContent` | position-independent: excludes dialogue ID, order, scene and neighbors |
| the canonical encoder and digest scheme | [`canonical-digest.md`](../../cantos-script-ir/references/canonical-digest.md) | reuse it; never invent a second canonical encoding |
| voice profile, pronunciation profile, resolved controls, provider, model, synthesis parameters, output profile, adapter version | this skill, from `FrozenProductionInputs` | resolved after [capability checks](provider-adapters.md#2-capabilities-are-data-checked-before-any-paid-call) |
| rights and tenant scope | [`rights-and-provenance.md`](../../cantos-publication/references/rights-and-provenance.md) | checked separately; never part of a key |

If an adapter sends neighboring lines as prosody context, that context is a new effective input:
add it to the fingerprint and accept that moving such a line regenerates it. Script IR's
position-independence guarantee then no longer covers that render; say so in the change.

## 2. The speech fingerprint

```rust
// Illustrative and proposed.
pub struct SpeechFingerprintInput {
    pub content: SpokenContentDigest,          // from cantos-script-ir
    pub language: LanguageTag,
    pub pronunciation: PronunciationProfileRevision,
    pub delivery: ResolvedControls,            // after capability resolution and mappings
    pub voice: VoiceProfileRevision,
    pub provider: ProviderId,
    pub model: ModelIdentity,
    pub params: SynthesisParams,               // fixed-point values only, never f32/f64
    pub output: OutputProfileRevision,
    pub adapter: AdapterVersion,
}

pub fn speech_fingerprint(input: &SpeechFingerprintInput) -> SpeechFingerprint {
    // Exhaustive destructuring, no `..`: a new field fails to compile until it is encoded.
    let SpeechFingerprintInput {
        content,
        language,
        pronunciation,
        delivery,
        voice,
        provider,
        model,
        params,
        output,
        adapter,
    } = input;
    let mut encoder = CanonicalEncoder::new(SPEECH_FINGERPRINT_SCHEME); // "cantos/speech/v1"
    encoder.field("adapter", adapter);
    encoder.field("content", content);
    encoder.field("delivery", delivery);
    encoder.field("language", language);
    encoder.field("model", model);
    encoder.field("output", output);
    encoder.field("params", params);
    encoder.field("pronunciation", pronunciation);
    encoder.field("provider", provider);
    encoder.field("voice", voice);
    SpeechFingerprint(encoder.finish())
}
```

- **Exclusion is type-enforced.** Dialogue ID, position, scene, character display name and script
  revision ID are not fields, so they cannot leak in. A script revision ID alone is never a reason
  to rerender ([Script IR § TTS cache boundary](../../../../docs/architecture/script-ir.md#tts-cache-boundary)).
- **The scheme tag is versioned.** Changing the encoding or the field set is a new scheme; every
  key changes on purpose, in a reviewed change that says so.
- **Store the input, not only the hash.** Persist the canonical input document next to the
  fingerprint so the planner can name *which* field changed and an audit can recompute the key.
- **Fixed-point only.** Intensity is permille (`600`), rates and gains are scaled integers;
  float text forms make equal values hash differently.

### Unpinnable models

```rust
pub enum ModelIdentity {
    Pinned { model: ModelId, revision: ModelRevision },
    Unpinned { model: ModelId, refresh_epoch: RefreshEpoch },
}
```

When the provider cannot pin a model version, record that limitation on the provider descriptor
and fingerprint a `RefreshEpoch` instead. An explicit refresh — a recorded action with actor and
reason — increments the epoch and invalidates exactly the lines using that model. The
provider-reported model version on each attempt observation is provenance, not key material; a
change observed while unpinned raises a drift finding that suggests a refresh. Reuse under an
unpinned model is identity reuse, not a claim that a new call would return the same bytes.

## 3. Mix, master and rendition fingerprints

Downstream fingerprints hash the **checksums of accepted artifacts**, not upstream fingerprints:
under an unpinned model two artifacts can share a speech fingerprint and differ in bytes, and the
mix depends on the bytes.

| Fingerprint | Inputs (in canonical order) |
|---|---|
| scene mix | speech artifact checksums in scene order with placement and deliberate pauses; cue asset revisions and checksums with anchors, gains and fades; ducking plan; mix profile revision; mixer tool identity |
| episode master | scene mix checksums in episode order; transitions; mastering profile revision; tool identity |
| release rendition | master checksum; delivery format; encoder tool identity |

```text
dialogue speech ──▶ scene mix ──▶ episode master ──▶ release renditions
 cue asset ────────▶    ▲              ▲                    ▲
 mix profile ───────────┘   mastering profile ──┘   output profile ──┘
 QC results and approvals attach to each artifact (audio-mix-and-qc.md, cantos-publication)
```

## 4. Plan regeneration as a pure function

```rust
pub fn plan_regeneration(previous: &ProductionGraph, next: &PlannedInputs) -> RegenerationPlan;

pub struct RegenerationPlan {
    pub speech: BTreeMap<DialogueId, Work<SpeechFingerprint>>,
    pub scene_mixes: BTreeMap<SceneId, Work<SceneMixInputs>>,
    pub master: Work<MasterInputs>,
    pub renditions: BTreeMap<DeliveryFormatId, Work<RenditionInputs>>,
}

pub enum Work<I> {
    Reuse { artifact: ArtifactId },
    Produce { inputs: I, reasons: Vec<InvalidationReason> },
}
```

`BTreeMap` keeps output order deterministic. Reasons (`ContentChanged`,
`VoiceProfileChanged { character, from, to }`, `ModelRefreshed`, `OrderChanged`,
`CueChanged { cue }`, `MixProfileChanged`, `ToolChanged`, …) come from comparing stored inputs
field by field, so Studio can say why a line regenerates. The plan says what *could* be reused;
`resolve_reuse` (§ 5) decides whether it *may*.

Expected plans for scene `scene-01` (`Sau buổi diễn`) of the sample episode:

| Change | Speech | Scene mix | Master · renditions |
|---|---|---|---|
| edit `dialogue-02` text | produce `dialogue-02` only | remix `scene-01` | remaster · re-encode |
| swap `dialogue-02` and `dialogue-03` | reuse all | remix `scene-01` (order changed) | remaster · re-encode |
| move `dialogue-03` to another scene | reuse all | remix both scenes | remaster · re-encode |
| new voice profile revision for An | produce An's lines only | remix scenes containing An | remaster · re-encode |
| new pronunciation profile revision | produce every line depending on that revision | remix their scenes | remaster · re-encode |
| replace the `cue-01` ambience asset | reuse all | remix `scene-01` | remaster · re-encode |
| new mix profile revision | reuse all | remix every scene using it | remaster · re-encode |
| delivery format only | reuse all | reuse all | reuse master · re-encode |
| new script revision, no effective change in the scene | reuse all | reuse | reuse |
| explicit refresh of an unpinned model | produce lines using that model | remix their scenes | remaster · re-encode |
| rights on `cue-01` expire | no key changes | blocking finding, not a silent remix | blocked |

The pronunciation row follows business rule 16 as written. A narrower per-line projection of
pronunciation rules would change that product rule; update the document before implementing it.

## 5. Reuse needs four facts, not one

```rust
pub fn resolve_reuse(
    fingerprint: &SpeechFingerprint,
    candidates: &[CacheCandidate],     // shell-resolved: scope, rights verdict, checksum, availability
    requester: &TenantScope,
) -> ReuseDecision;

pub enum ReuseDecision {
    Reuse(ArtifactId),
    Produce(ReuseRefusal),             // NoCandidate · ScopeDenied · ChecksumMismatch · Unavailable
    Block(RightsBlocker),              // rights do not cover this use; producing anew is no fix
}
```

Rights and tenant scope stay out of the key because they can change without the audio changing:
an expired voice licence must block, not trigger a regeneration that would be equally
unlicensed. A private voice asset of one creator is never reusable by an unrelated creator
(`ScopeDenied`), even with an identical fingerprint. Checksum and availability are verified
against the stored object, not assumed from the row.

## 6. Tests that make the field list checkable

**Golden.** A committed input (An, `dialogue-02`, `Ngày mai, mình có diễn tiếp không?`) maps to a
committed hex literal. A change to the literal is a scheme bump, never a refreshed snapshot.

**Every effective field changes the key.** Mutate one field at a time; the destructuring makes
the test stop compiling when a field is added without a mutator.

```rust
#[test]
fn every_effective_field_changes_the_speech_fingerprint() {
    let base = golden_input();
    let SpeechFingerprintInput {
        content: _, language: _, pronunciation: _, delivery: _, voice: _,
        provider: _, model: _, params: _, output: _, adapter: _,
    } = &base;
    let mutators: [(&str, fn(&mut SpeechFingerprintInput)); 10] = [
        ("content", |i| i.content = other_content()),
        ("language", |i| i.language = LanguageTag::en_us()),
        ("pronunciation", |i| i.pronunciation = i.pronunciation.next()),
        ("delivery", |i| i.delivery = other_controls()),
        ("voice", |i| i.voice = i.voice.next()),
        ("provider", |i| i.provider = other_provider()),
        ("model", |i| i.model = refreshed(&i.model)),
        ("params", |i| i.params = slower(&i.params)),
        ("output", |i| i.output = i.output.next()),
        ("adapter", |i| i.adapter = i.adapter.next()),
    ];
    let expected = speech_fingerprint(&base);
    for (field, mutate) in mutators {
        let mut changed = base.clone();
        mutate(&mut changed);
        assert_ne!(speech_fingerprint(&changed), expected, "{field} is not fingerprinted");
    }
}
```

A field omitted from the fingerprint is a defect, and this test is how it is caught. A
mutation-testing run (candidate tool: cargo-mutants) over `speech_fingerprint` and the planner
then checks that deleting any `encoder.field` call is killed
([`mutation-and-formal.md`](../../cantos-engineering/references/mutation-and-formal.md)).

**Determinism and equivalence.** Unchanged inputs give identical bytes across runs started from
scratch; inputs built in a different insertion order give the same key
([`property-and-differential-testing.md`](../../cantos-engineering/references/property-and-differential-testing.md)).

**Planner laws.** `plan_regeneration(g, inputs_of(g))` reuses everything; changing one line's
content produces exactly that line plus its scene and descendants; reorder-only changes produce
no speech; adding an unrelated scene leaves every existing decision unchanged. Compare the
incremental planner with a naive reference that recomputes every fingerprint from scratch and
diffs the maps (`differentially-tested`). Each row of the table in § 4 is also an example test
named like a theorem: `moving_a_dialogue_between_scenes_keeps_its_speech_and_remixes_both`.

## 7. Rule cards

| Rule | Failure mode | Good | Counterexample | Oracle | Status · exception |
|---|---|---|---|---|---|
| Fingerprint every effective input | stale audio reused after a voice or model change | § 2 with exhaustive destructuring | key = `(dialogue_id, script_revision)` | per-field mutation test | proposed · none |
| Fingerprint nothing else | reorder or a new revision ID rerenders paid speech | position-free `SpokenContent` | position or revision ID in the key | reorder-only metamorphic test | proposed · prosody context the adapter really sends |
| Downstream keys hash accepted checksums | a mix reused over different bytes | checksums in scene order | upstream fingerprints only | unpinned-model scenario | proposed · none |
| Reuse needs fingerprint, scope, checksum, availability | another creator's private voice, or a deleted object, reused | `resolve_reuse` | fingerprint lookup alone | one example per `ReuseDecision` variant | proposed · none |
| Rights are checked, never keyed | expired licence triggers regeneration instead of blocking | `ReuseDecision::Block` | `rights_status` inside the key | expiry example | proposed · none |
| Unpinnable models carry an explicit refresh epoch | silent model drift with stale cache | `ModelIdentity::Unpinned { refresh_epoch }` | `model = "latest"` | drift finding test | proposed · none |
| Encoding changes bump the scheme | every key changes silently | `cantos/speech/v2` with a migration note | golden literal "updated" | golden test | proposed · none |
