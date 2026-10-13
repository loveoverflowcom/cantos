//! Result views: the JSON `data` of each command and its plain-text form.
//!
//! The JSON is the contract; text is a convenience for people and carries no extra facts.

use std::path::Path;

use serde_json::{json, Value};

use crate::{
    application::{
        ports::{BundleReceipt, Corruption, InitOutcome},
        register::{Placement, Registered},
        validate::{Validated, Write},
        view::{ArtifactDetail, ArtifactSummary},
    },
    domain::{
        artifact::ArtifactKind,
        audio_profile::AudioProfile,
        checks::DetailValue,
        evaluation::CheckOutcome,
        handoff::{ExportPlan, ExportRefusal},
        identity::ArtifactId,
        lineage::{Freshness, PinViolation},
    },
    wire::{detail_to_wire, EvidenceWire, LayerWire, LayersWire, PinWire, RevisionWire},
};

/// Every implemented command, as typed on the command line.
pub const COMMANDS: &[&str] = &[
    "capabilities",
    "workspace init",
    "artifact add",
    "artifact list",
    "artifact show",
    "validate",
    "handoff export",
];

const ARTIFACT_KINDS: &[&str] = &[
    "manuscript-original",
    "manuscript-collected",
    "script-ir",
    "audio",
    "report",
    "instruction-bundle",
];
const CHECKS: &[&str] = &["ogg-opus-profile-v1", "utf8-text-v1"];

pub fn capabilities() -> (Value, String) {
    let profiles: Vec<&str> = AudioProfile::ALL
        .iter()
        .map(|profile| profile.id())
        .collect();
    let data = json!({
        "forge_version": env!("CARGO_PKG_VERSION"),
        "commands": COMMANDS,
        "workflows": { "host_assisted": "local", "provider_managed": "unsupported" },
        "providers": [],
        "handoff": "unsupported",
        "local_bundle_export": "audio",
        "artifact_kinds": ARTIFACT_KINDS,
        "audio_profiles": profiles,
        "checks": CHECKS,
    });
    let text = format!(
        "cantos-forge {}\ncommands: {}\nhost-assisted workflow: local (register, validate, export a local bundle)\n\
         provider-managed workflow: unsupported\nproviders: none\nhandoff to the Cantos API: unsupported\n\
         audio profiles: {}\nchecks: {}\n",
        env!("CARGO_PKG_VERSION"),
        COMMANDS.join(", "),
        profiles.join(", "),
        CHECKS.join(", "),
    );
    (data, text)
}

pub fn kind_name(kind: ArtifactKind) -> &'static str {
    match kind {
        ArtifactKind::Manuscript => "manuscript",
        ArtifactKind::ScriptIr => "script_ir",
        ArtifactKind::Audio => "audio",
        ArtifactKind::Report => "report",
        ArtifactKind::InstructionBundle => "instruction_bundle",
    }
}

pub fn initialized(path: &Path, outcome: InitOutcome) -> (Value, String) {
    let created = outcome == InitOutcome::Created;
    let data = json!({ "workspace": path.display().to_string(), "created": created });
    let text = if created {
        format!("created workspace {}\n", path.display())
    } else {
        format!(
            "workspace {} already exists; nothing changed\n",
            path.display()
        )
    };
    (data, text)
}

pub fn registered(kind: ArtifactKind, outcome: Registered) -> (Value, String) {
    let (result, placement) = match outcome {
        Registered::Created(placement) => ("created", placement),
        Registered::AlreadyCurrent(placement) => ("already_current", placement),
    };
    let Placement {
        artifact,
        revision,
        digest,
    } = placement;
    let data = json!({
        "result": result,
        "artifact": artifact.to_string(),
        "kind": kind_name(kind),
        "revision": revision.get(),
        "digest": digest.to_string(),
    });
    let text = match result {
        "created" => format!(
            "registered {artifact} revision {} ({digest})\n",
            revision.get()
        ),
        _ => format!(
            "{artifact} revision {} already has exactly these bytes; nothing changed\n",
            revision.get()
        ),
    };
    (data, text)
}

pub fn listed(summaries: &[ArtifactSummary]) -> (Value, String) {
    let artifacts: Vec<Value> = summaries
        .iter()
        .map(|summary| {
            json!({
                "artifact": summary.artifact.to_string(),
                "kind": kind_name(summary.kind),
                "head_revision": summary.head.get(),
                "digest": summary.digest.to_string(),
            })
        })
        .collect();
    let mut text = String::new();
    for summary in summaries {
        text.push_str(&format!(
            "{} {} r{} {}\n",
            summary.artifact,
            kind_name(summary.kind),
            summary.head.get(),
            summary.digest
        ));
    }
    if text.is_empty() {
        text.push_str("no artifacts\n");
    }
    (json!({ "artifacts": artifacts }), text)
}

fn violation_json(violation: &PinViolation) -> Value {
    match violation {
        PinViolation::UnknownArtifact { pin } => {
            json!({ "code": "unknown_artifact", "pin": PinWire::from(*pin) })
        }
        PinViolation::BeyondHead { pin, head } => {
            json!({ "code": "beyond_head", "pin": PinWire::from(*pin), "head": head.get() })
        }
        PinViolation::DigestConflict { pin, recorded } => json!({
            "code": "digest_conflict",
            "pin": PinWire::from(*pin),
            "recorded": recorded.to_string(),
        }),
    }
}

fn violations_json(violations: &[PinViolation]) -> Value {
    json!({
        "status": "integrity_violation",
        "violations": violations.iter().map(violation_json).collect::<Vec<_>>(),
    })
}

pub fn shown(artifact: ArtifactId, detail: &ArtifactDetail) -> (Value, String) {
    let head = detail.revisions.last();
    let kind = head.map(|revision| kind_name(revision.facts().payload.kind()));
    let head_number = head.map(|revision| revision.revision().get());
    let inputs = match &detail.inputs {
        Ok(Freshness::Current) => json!({ "status": "current" }),
        Ok(Freshness::Stale(superseded)) => json!({
            "status": "stale",
            "superseded": superseded.iter().map(|input| json!({
                "pin": PinWire::from(input.pin),
                "head": input.head.get(),
            })).collect::<Vec<_>>(),
        }),
        Err(violations) => violations_json(violations),
    };
    let evaluation = match &detail.evaluation {
        Ok(summary) => serde_json::to_value(LayersWire::from(summary)).unwrap_or(Value::Null),
        Err(violations) => violations_json(violations),
    };
    let revisions: Vec<RevisionWire> = detail
        .revisions
        .iter()
        .map(|revision| {
            RevisionWire::from_facts(revision.artifact(), revision.revision(), revision.facts())
        })
        .collect();
    let evidence: Vec<EvidenceWire> = detail
        .evidence
        .iter()
        .map(|record| EvidenceWire::from_record(&record.evidence, &record.detail))
        .collect();
    let data = json!({
        "artifact": artifact.to_string(),
        "kind": kind,
        "head_revision": head_number,
        "revisions": revisions,
        "inputs": inputs,
        "evaluation": evaluation,
        "evidence": evidence,
    });
    let mut text = format!(
        "{artifact} {} r{}\n",
        kind.unwrap_or("unknown"),
        head_number.unwrap_or(0)
    );
    for revision in &detail.revisions {
        text.push_str(&format!(
            "  r{} {}\n",
            revision.revision().get(),
            revision.facts().content
        ));
    }
    text.push_str(&format!(
        "inputs: {}\n",
        inputs["status"].as_str().unwrap_or("?")
    ));
    match &detail.evaluation {
        Ok(summary) => {
            let layers = LayersWire::from(summary);
            text.push_str(&format!(
                "deterministic: {}\nai review: {}\nhuman review: {}\n",
                layer_text(&layers.deterministic),
                layer_text(&layers.ai_review),
                layer_text(&layers.human_review),
            ));
        }
        Err(_) => text.push_str("evaluation: integrity violation (see --json)\n"),
    }
    (data, text)
}

fn layer_text(layer: &LayerWire) -> String {
    match layer {
        LayerWire::NotRun => "not run".to_owned(),
        LayerWire::Stale => "stale (evidence is about another revision)".to_owned(),
        LayerWire::Current { result } => format!("current: {result}"),
    }
}

pub fn validated(validated: &Validated) -> (Value, String, bool) {
    let passed = validated.result.outcome == CheckOutcome::Passed;
    let outcome = if passed { "passed" } else { "failed" };
    let result = match validated.write {
        Write::Recorded => "recorded",
        Write::AlreadyRecorded => "already_recorded",
    };
    let data = json!({
        "result": result,
        "artifact": validated.subject.artifact.to_string(),
        "revision": validated.subject.revision.get(),
        "digest": validated.subject.digest.to_string(),
        "check": validated.result.check.label(),
        "outcome": outcome,
        "evidence": validated.evidence.to_string(),
        "detail": detail_to_wire(&validated.result.detail),
    });
    let mut text = format!(
        "{}: {outcome} ({result}) for {} r{} as {}\n",
        validated.result.check.label(),
        validated.subject.artifact,
        validated.subject.revision.get(),
        validated.evidence,
    );
    for (key, value) in validated.result.detail.iter() {
        let shown = match value {
            DetailValue::Count(count) => count.to_string(),
            DetailValue::Text(text) => text.clone(),
            DetailValue::Codes(codes) if codes.is_empty() => "none".to_owned(),
            DetailValue::Codes(codes) => codes.join(", "),
        };
        text.push_str(&format!("  {key}: {shown}\n"));
    }
    (data, text, passed)
}

pub fn exported(plan: &ExportPlan, receipt: &BundleReceipt) -> (Value, String) {
    let data = json!({
        "artifact": plan.revision.artifact().to_string(),
        "revision": plan.revision.revision().get(),
        "profile": plan.profile.id(),
        "bundle": {
            "manifest": receipt.manifest,
            "audio": receipt.audio,
            "manifest_sha256": receipt.manifest_digest.to_string(),
        },
        "submission": "not_run",
    });
    let text = format!(
        "wrote local bundle for {} r{} ({})\n  manifest: {}\n  audio: {}\nsubmission: not run (Forge has no API handoff)\n",
        plan.revision.artifact(),
        plan.revision.revision().get(),
        plan.profile,
        receipt.manifest,
        receipt.audio,
    );
    (data, text)
}

/// One export refusal with the command that resolves it, when there is one.
pub fn refusal_json(
    refusal: &ExportRefusal,
    workspace: &Path,
    artifact: ArtifactId,
    profile: AudioProfile,
) -> Value {
    let workspace = workspace.display();
    let validate = format!(
        "cantos-forge validate --workspace {workspace} --artifact {artifact} --profile {profile}"
    );
    match refusal {
        ExportRefusal::NotAudio { kind } => json!({
            "code": "not_audio", "kind": kind_name(*kind),
            "note": "only audio can be exported in this build",
        }),
        ExportRefusal::SupersededRevision { head } => json!({
            "code": "superseded_revision", "head": head,
        }),
        ExportRefusal::RightsUnstated => json!({
            "code": "rights_unstated",
            "action": "register a new revision with an explicit --rights claim",
        }),
        ExportRefusal::InputsStale(superseded) => json!({
            "code": "inputs_stale",
            "superseded": superseded.iter().map(|input| json!({
                "pin": PinWire::from(input.pin), "head": input.head.get(),
            })).collect::<Vec<_>>(),
            "action": "produce and register audio from the current inputs",
        }),
        ExportRefusal::HistoryViolations(violations) => json!({
            "code": "history_violations",
            "violations": violations.iter().map(violation_json).collect::<Vec<_>>(),
        }),
        ExportRefusal::CheckNotRun => json!({ "code": "check_not_run", "action": validate }),
        ExportRefusal::CheckStale => json!({ "code": "check_stale", "action": validate }),
        ExportRefusal::CheckFailed => json!({
            "code": "check_failed",
            "note": "the recorded check for this revision and profile failed; fix the audio or pick another profile",
        }),
    }
}

pub fn corruption_json(corruption: &Corruption) -> Value {
    match corruption {
        Corruption::MalformedRecord { record } => {
            json!({ "kind": "malformed_record", "record": record })
        }
        Corruption::NotARegularFile { record } => {
            json!({ "kind": "not_a_regular_file", "record": record })
        }
        Corruption::UnexpectedEntry { record } => {
            json!({ "kind": "unexpected_entry", "record": record })
        }
        Corruption::BlobDigestMismatch { digest } => {
            json!({ "kind": "blob_digest_mismatch", "digest": digest.to_string() })
        }
        Corruption::MissingBlob { digest } => {
            json!({ "kind": "missing_blob", "digest": digest.to_string() })
        }
        Corruption::RevisionNumbering { artifact } => {
            json!({ "kind": "revision_numbering", "artifact": artifact.to_string() })
        }
        Corruption::HistoryRejected { artifact, error } => json!({
            "kind": "history_rejected", "artifact": artifact.to_string(), "reason": format!("{error:?}"),
        }),
        Corruption::TooLarge { record } => json!({ "kind": "too_large", "record": record }),
    }
}
