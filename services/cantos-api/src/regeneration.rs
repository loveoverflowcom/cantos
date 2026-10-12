//! Pure partial-regeneration plan: which dialogue to synthesize, which verified clip to keep,
//! which scene mixes and QC results go stale, and what the synthesis would cost.
//!
//! The plan compares two frozen production snapshots and consults verified clips through
//! [`resolve_reuse`]. It performs no I/O and starts no job. It is a forecast: the durable
//! worker still re-checks authority, the lease and a fresh object probe before it reuses a
//! clip, and a cache hit is never recorded as a provider dispatch.
//!
//! Mixing, QC and release records do not exist yet, so the stale marks name what *would*
//! need remixing or re-review; nothing is persisted for them.

use std::collections::{BTreeMap, BTreeSet, HashMap};

use cantos_contracts::{
    FrozenProductionDocument, ProductionBudget, ProductionEligibility, ProductionEstimate,
    ResolvedSynthesisInput,
};
use serde::Serialize;

use crate::{
    clip_reuse::{
        resolve_reuse, AudioDigest, ClipCandidate, ClipRefusal, ClipRequest, ReuseDecision,
        RightsStanding,
    },
    jobs::ExecutionMode,
    production::estimate_dialogues,
    script_ir::read_canonical_script,
    speech_fingerprint::{FingerprintError, SpeechField, SpeechFingerprint, SpeechInput},
};

pub const PLAN_VERSION: &str = "regeneration-plan-v1";
/// The estimate uses the recorded planning rate. It is not a reservation, usage or a charge.
pub const COST_BASIS: &str = "planning_estimate_not_a_charge";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShapeError {
    ScriptUnreadable,
    DialogueUnresolved,
    ResolutionUnplaced,
    DuplicateScene,
    DuplicateDialogue,
}

impl std::fmt::Display for ShapeError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::ScriptUnreadable => "frozen script revision is not a canonical export",
            Self::DialogueUnresolved => "a scripted dialogue has no resolved synthesis input",
            Self::ResolutionUnplaced => "a resolved synthesis input is in no scene",
            Self::DuplicateScene => "scene identifiers must be unique",
            Self::DuplicateDialogue => "dialogue identifiers must be unique",
        })
    }
}

impl std::error::Error for ShapeError {}

/// A request's identity and exact effective input, or the reason it has none.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpeechIdentity {
    pub input: SpeechInput,
    pub fingerprint: SpeechFingerprint,
}

#[derive(Debug, Clone)]
pub struct ShapeLine {
    dialogue_id: String,
    resolved: ResolvedSynthesisInput,
    speech: Result<SpeechIdentity, FingerprintError>,
}

impl ShapeLine {
    fn new(resolved: ResolvedSynthesisInput) -> Self {
        let speech = SpeechInput::from_resolved(&resolved).map(|input| SpeechIdentity {
            fingerprint: input.fingerprint(),
            input,
        });
        Self {
            dialogue_id: resolved.dialogue_id.clone(),
            resolved,
            speech,
        }
    }

    fn fingerprint(&self) -> Option<&SpeechFingerprint> {
        self.speech.as_ref().ok().map(|speech| &speech.fingerprint)
    }
}

/// One snapshot's production layout: scenes in order, each with its dialogue in order.
#[derive(Debug, Clone)]
pub struct ProductionShape {
    scenes: Vec<(String, Vec<ShapeLine>)>,
    budget: ProductionBudget,
}

impl ProductionShape {
    pub fn new(
        scenes: Vec<(String, Vec<ResolvedSynthesisInput>)>,
        budget: ProductionBudget,
    ) -> Result<Self, ShapeError> {
        let mut scene_ids = BTreeSet::new();
        let mut dialogue_ids = BTreeSet::new();
        for (scene_id, lines) in &scenes {
            if !scene_ids.insert(scene_id.clone()) {
                return Err(ShapeError::DuplicateScene);
            }
            for line in lines {
                if !dialogue_ids.insert(line.dialogue_id.clone()) {
                    return Err(ShapeError::DuplicateDialogue);
                }
            }
        }
        Ok(Self {
            scenes: scenes
                .into_iter()
                .map(|(scene_id, lines)| {
                    (scene_id, lines.into_iter().map(ShapeLine::new).collect())
                })
                .collect(),
            budget,
        })
    }

    /// Scene membership comes from the frozen script revision; synthesis inputs from the frozen
    /// resolution. Every resolved line must sit in exactly one scene.
    pub fn from_document(document: &FrozenProductionDocument) -> Result<Self, ShapeError> {
        let script = read_canonical_script(document.revision.script_json.as_bytes())
            .map_err(|_| ShapeError::ScriptUnreadable)?;
        let mut by_dialogue: HashMap<&str, &ResolvedSynthesisInput> = HashMap::new();
        for line in &document.resolved {
            if by_dialogue
                .insert(line.dialogue_id.as_str(), line)
                .is_some()
            {
                return Err(ShapeError::DuplicateDialogue);
            }
        }
        let mut placed = 0_usize;
        let mut scenes = Vec::new();
        for scene in script.spoken_scenes() {
            let mut lines = Vec::new();
            for dialogue_id in scene.dialogue_ids {
                let resolved = by_dialogue
                    .get(dialogue_id)
                    .ok_or(ShapeError::DialogueUnresolved)?;
                lines.push((*resolved).clone());
                placed += 1;
            }
            scenes.push((scene.scene_id.to_owned(), lines));
        }
        if placed != document.resolved.len() {
            return Err(ShapeError::ResolutionUnplaced);
        }
        Self::new(scenes, document.settings.settings.budget.clone())
    }

    /// Every fingerprint the shell needs registry rows for.
    pub fn fingerprints(&self) -> BTreeSet<SpeechFingerprint> {
        self.lines()
            .filter_map(|(_, line)| line.fingerprint().cloned())
            .collect()
    }

    fn lines(&self) -> impl Iterator<Item = (&str, &ShapeLine)> {
        self.scenes
            .iter()
            .flat_map(|(scene, lines)| lines.iter().map(move |line| (scene.as_str(), line)))
    }
}

/// Whether the requester may currently act on the next snapshot at all.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum PlanGate {
    Open,
    Blocked { findings: Vec<String> },
}

impl PlanGate {
    /// Mirrors the durable worker's own authority test: a current approval over eligible inputs.
    /// A reuse hit never widens it; rights, budget and approval gates stay as they are.
    pub fn from_eligibility(eligibility: &ProductionEligibility) -> Self {
        if eligibility.inputs_eligible && eligibility.approval_current {
            return Self::Open;
        }
        let mut findings: Vec<String> = eligibility
            .findings
            .iter()
            .filter_map(|finding| serde_json::to_value(finding.code).ok())
            .filter_map(|code| code.as_str().map(str::to_owned))
            .collect();
        if !eligibility.approval_current {
            findings.push("approval_not_current".into());
        }
        findings.sort();
        findings.dedup();
        Self::Blocked { findings }
    }

    fn standing(&self) -> RightsStanding {
        match self {
            Self::Open => RightsStanding::Current,
            Self::Blocked { .. } => RightsStanding::Lapsed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LineChange {
    /// Not in the previous snapshot (or there is none).
    Added,
    /// Same synthesis request. `placement_changed` is true when only its scene differs; a reorder
    /// inside one scene is reported by that scene's `speech_sequence_changed` instead.
    Unchanged {
        placement_changed: bool,
    },
    Changed {
        fields: Vec<SpeechField>,
    },
    /// One side has no fingerprint, so no field comparison exists.
    Incomparable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum CacheMiss {
    NoCandidate,
    Refused { refusals: Vec<ClipRefusal> },
    NoFingerprint { code: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum LineAction {
    Reuse {
        clip_id: String,
        audio_sha256: AudioDigest,
    },
    Produce {
        cache: CacheMiss,
    },
    /// The gate is closed: neither reuse nor new synthesis is permitted.
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct LinePlan {
    pub scene_id: String,
    pub dialogue_id: String,
    pub fingerprint: Option<SpeechFingerprint>,
    pub change: LineChange,
    pub action: LineAction,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Staleness {
    Unchanged,
    Stale,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum StaleReason {
    SceneAdded,
    /// The ordered speech of the scene differs: a line added, removed, moved, reordered or changed.
    SpeechSequenceChanged,
    /// The speech is unchanged but new audio will be synthesized, and a mix hashes audio bytes.
    SpeechRegenerated {
        dialogue_id: String,
    },
    SpeechBlocked {
        dialogue_id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SceneImpact {
    pub scene_id: String,
    pub mix: Staleness,
    /// A QC result names the exact artifact it judged, so it goes stale with its mix.
    pub qc: Staleness,
    pub reasons: Vec<StaleReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum EpisodeReason {
    NoPreviousProduction,
    SceneOrderChanged,
    SceneRemoved { scene_id: String },
    SceneStale { scene_id: String },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct EpisodeImpact {
    pub master: Staleness,
    pub qc: Staleness,
    pub reasons: Vec<EpisodeReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CostForecast {
    pub basis: &'static str,
    pub cache_hits: u64,
    pub cache_misses: u64,
    pub blocked: u64,
    /// Provider sends the plan expects. Cache hits contribute none.
    pub provider_dispatches_expected: u64,
    pub miss_estimate: ProductionEstimate,
    pub hit_estimate_avoided: ProductionEstimate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RegenerationPlan {
    pub version: &'static str,
    pub gate: PlanGate,
    pub lines: Vec<LinePlan>,
    pub removed_dialogues: Vec<String>,
    pub scenes: Vec<SceneImpact>,
    pub removed_scenes: Vec<String>,
    pub episode: EpisodeImpact,
    pub cost: CostForecast,
}

pub struct PlanInputs<'a> {
    pub previous: Option<&'a ProductionShape>,
    pub next: &'a ProductionShape,
    pub gate: PlanGate,
    pub owner_id: &'a str,
    pub producer: ExecutionMode,
    /// Registered clips per fingerprint, each with the shell's fresh probe of its object.
    pub cache: &'a BTreeMap<SpeechFingerprint, Vec<ClipCandidate>>,
}

fn line_change(
    previous: Option<(&str, &ShapeLine)>,
    scene_id: &str,
    next: &ShapeLine,
) -> LineChange {
    let Some((previous_scene, previous)) = previous else {
        return LineChange::Added;
    };
    match (&previous.speech, &next.speech) {
        (Ok(before), Ok(after)) => {
            let fields = before.input.differing_fields(&after.input);
            if fields.is_empty() {
                LineChange::Unchanged {
                    placement_changed: previous_scene != scene_id,
                }
            } else {
                LineChange::Changed { fields }
            }
        }
        _ => LineChange::Incomparable,
    }
}

fn line_action(inputs: &PlanInputs<'_>, line: &ShapeLine) -> LineAction {
    let identity = match &line.speech {
        Ok(identity) => identity,
        Err(error) => {
            return match inputs.gate {
                PlanGate::Open => LineAction::Produce {
                    cache: CacheMiss::NoFingerprint { code: error.code() },
                },
                PlanGate::Blocked { .. } => LineAction::Blocked,
            }
        }
    };
    let request = ClipRequest {
        fingerprint: identity.fingerprint.clone(),
        owner_id: inputs.owner_id.to_owned(),
        producer: inputs.producer,
        voice_rights_record_id: line.resolved.voice_rights_record_id.clone(),
        output_contract: line.resolved.output_contract.clone(),
    };
    let candidates = inputs
        .cache
        .get(&identity.fingerprint)
        .map_or(&[][..], Vec::as_slice);
    match resolve_reuse(&request, inputs.gate.standing(), candidates) {
        ReuseDecision::Reuse {
            clip_id,
            audio_sha256,
        } => LineAction::Reuse {
            clip_id,
            audio_sha256,
        },
        ReuseDecision::Produce { refusals } if refusals.is_empty() => LineAction::Produce {
            cache: CacheMiss::NoCandidate,
        },
        ReuseDecision::Produce { refusals } => LineAction::Produce {
            cache: CacheMiss::Refused { refusals },
        },
        ReuseDecision::Blocked => LineAction::Blocked,
    }
}

fn estimate_of(lines: &[ResolvedSynthesisInput], budget: &ProductionBudget) -> ProductionEstimate {
    estimate_dialogues(lines, budget)
}

pub fn plan_regeneration(inputs: &PlanInputs<'_>) -> RegenerationPlan {
    let previous_lines: HashMap<&str, (&str, &ShapeLine)> = inputs
        .previous
        .map(|shape| {
            shape
                .lines()
                .map(|(scene, line)| (line.dialogue_id.as_str(), (scene, line)))
                .collect()
        })
        .unwrap_or_default();

    let mut lines = Vec::new();
    let mut hits = Vec::new();
    let mut misses = Vec::new();
    let mut blocked = 0_u64;
    for (scene_id, line) in inputs.next.lines() {
        let action = line_action(inputs, line);
        match &action {
            LineAction::Reuse { .. } => hits.push(line.resolved.clone()),
            LineAction::Produce { .. } => misses.push(line.resolved.clone()),
            LineAction::Blocked => blocked += 1,
        }
        lines.push(LinePlan {
            scene_id: scene_id.to_owned(),
            dialogue_id: line.dialogue_id.clone(),
            fingerprint: line.fingerprint().cloned(),
            change: line_change(
                previous_lines.get(line.dialogue_id.as_str()).copied(),
                scene_id,
                line,
            ),
            action,
        });
    }

    let next_dialogues: BTreeSet<&str> = inputs
        .next
        .lines()
        .map(|(_, line)| line.dialogue_id.as_str())
        .collect();
    let mut removed_dialogues: Vec<String> = previous_lines
        .keys()
        .filter(|id| !next_dialogues.contains(*id))
        .map(|id| (*id).to_owned())
        .collect();
    removed_dialogues.sort();

    let (scenes, removed_scenes, episode) = scene_impacts(inputs, &lines);
    let cost = CostForecast {
        basis: COST_BASIS,
        cache_hits: hits.len() as u64,
        cache_misses: misses.len() as u64,
        blocked,
        provider_dispatches_expected: misses.len() as u64,
        miss_estimate: estimate_of(&misses, &inputs.next.budget),
        hit_estimate_avoided: estimate_of(&hits, &inputs.next.budget),
    };
    RegenerationPlan {
        version: PLAN_VERSION,
        gate: inputs.gate.clone(),
        lines,
        removed_dialogues,
        scenes,
        removed_scenes,
        episode,
        cost,
    }
}

fn scene_impacts(
    inputs: &PlanInputs<'_>,
    lines: &[LinePlan],
) -> (Vec<SceneImpact>, Vec<String>, EpisodeImpact) {
    let sequence =
        |shape: &ProductionShape, scene: &str| -> Option<Vec<Option<SpeechFingerprint>>> {
            shape
                .scenes
                .iter()
                .find(|(id, _)| id == scene)
                .map(|(_, scene_lines)| {
                    scene_lines
                        .iter()
                        .map(|l| l.fingerprint().cloned())
                        .collect()
                })
        };
    let mut scenes = Vec::new();
    for (scene_id, scene_lines) in &inputs.next.scenes {
        let mut reasons = Vec::new();
        match inputs.previous {
            None => reasons.push(StaleReason::SceneAdded),
            Some(previous) => match sequence(previous, scene_id) {
                None => reasons.push(StaleReason::SceneAdded),
                Some(before) => {
                    let after: Vec<_> = scene_lines
                        .iter()
                        .map(|l| l.fingerprint().cloned())
                        .collect();
                    // A line without a fingerprint can never be proven unchanged.
                    if before != after || after.iter().any(Option::is_none) {
                        reasons.push(StaleReason::SpeechSequenceChanged);
                    }
                }
            },
        }
        for plan in lines.iter().filter(|plan| &plan.scene_id == scene_id) {
            match plan.action {
                LineAction::Reuse { .. } => {}
                LineAction::Produce { .. } => reasons.push(StaleReason::SpeechRegenerated {
                    dialogue_id: plan.dialogue_id.clone(),
                }),
                LineAction::Blocked => reasons.push(StaleReason::SpeechBlocked {
                    dialogue_id: plan.dialogue_id.clone(),
                }),
            }
        }
        let stale = if reasons.is_empty() {
            Staleness::Unchanged
        } else {
            Staleness::Stale
        };
        scenes.push(SceneImpact {
            scene_id: scene_id.clone(),
            mix: stale,
            qc: stale,
            reasons,
        });
    }

    let next_scene_ids: Vec<&str> = inputs
        .next
        .scenes
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    let mut removed_scenes = Vec::new();
    let mut episode_reasons = Vec::new();
    match inputs.previous {
        None => episode_reasons.push(EpisodeReason::NoPreviousProduction),
        Some(previous) => {
            let previous_ids: Vec<&str> =
                previous.scenes.iter().map(|(id, _)| id.as_str()).collect();
            for id in &previous_ids {
                if !next_scene_ids.contains(id) {
                    removed_scenes.push((*id).to_owned());
                    episode_reasons.push(EpisodeReason::SceneRemoved {
                        scene_id: (*id).to_owned(),
                    });
                }
            }
            let kept_before: Vec<_> = previous_ids
                .iter()
                .filter(|id| next_scene_ids.contains(id))
                .collect();
            let kept_after: Vec<_> = next_scene_ids
                .iter()
                .filter(|id| previous_ids.contains(id))
                .collect();
            if kept_before != kept_after {
                episode_reasons.push(EpisodeReason::SceneOrderChanged);
            }
        }
    }
    for scene in scenes.iter().filter(|scene| scene.mix == Staleness::Stale) {
        episode_reasons.push(EpisodeReason::SceneStale {
            scene_id: scene.scene_id.clone(),
        });
    }
    let stale = if episode_reasons.is_empty() {
        Staleness::Unchanged
    } else {
        Staleness::Stale
    };
    (
        scenes,
        removed_scenes,
        EpisodeImpact {
            master: stale,
            qc: stale,
            reasons: episode_reasons,
        },
    )
}
