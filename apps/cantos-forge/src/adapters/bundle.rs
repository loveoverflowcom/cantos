//! Writing a local handoff bundle: the exact audio bytes and a manifest of what is known.
//!
//! The manifest states only what Forge can stand behind: producer-declared provenance and
//! rights, producer-side measurements, and the independent evaluation layers as recorded. It
//! says plainly that nothing was submitted to the Cantos API, and it is never a receipt,
//! approval or publication.

use std::{collections::BTreeMap, fs, io::ErrorKind, path::Path};

use serde::Serialize;

use crate::{
    application::ports::{Bundle, BundleError, BundleReceipt, StoreError},
    domain::{digest::Sha256Digest, evaluation::CheckOutcome},
    wire::{
        detail_to_wire, DetailWire, LayersWire, PayloadWire, PinWire, ProvenanceWire, RightsWire,
    },
};

use super::fs::publish_new;

pub const MANIFEST_SCHEMA: &str = "cantos-forge.handoff-bundle/v1";
pub const MANIFEST_FILE: &str = "bundle.json";
pub const AUDIO_FILE: &str = "audio.ogg";

const NOT_SUBMITTED: &str = "Forge has no API handoff in this build; this bundle is local files \
     only. Claims here are producer-declared and producer-measured: not server-inspected, not \
     listened to unless a human_review layer says so, and not editor-approved or published.";

#[derive(Serialize)]
struct Manifest {
    schema: &'static str,
    forge_version: &'static str,
    artifact: String,
    revision: u32,
    payload: PayloadWire,
    audio: AudioFile,
    profile: &'static str,
    /// The profile check run again on these exact bytes while the bundle was prepared.
    measured: Measured,
    provenance: ProvenanceWire,
    rights: RightsWire,
    inputs: Vec<PinWire>,
    evaluation: Evaluation,
    submission: Submission,
}

#[derive(Serialize)]
struct AudioFile {
    file: &'static str,
    sha256: String,
    byte_length: u64,
}

#[derive(Serialize)]
struct Measured {
    check: String,
    outcome: &'static str,
    detail: BTreeMap<String, DetailWire>,
}

#[derive(Serialize)]
struct Evaluation {
    #[serde(flatten)]
    layers: LayersWire,
    /// The evidence record that opened the export gate.
    gate_evidence: String,
}

#[derive(Serialize)]
struct Submission {
    status: &'static str,
    note: &'static str,
}

/// The manifest document for `bundle`, exactly as it is written to disk.
pub fn encode_manifest(bundle: &Bundle) -> Result<Vec<u8>, serde_json::Error> {
    let plan = &bundle.plan;
    let facts = plan.revision.facts();
    let manifest = Manifest {
        schema: MANIFEST_SCHEMA,
        forge_version: env!("CARGO_PKG_VERSION"),
        artifact: plan.revision.artifact().to_string(),
        revision: plan.revision.revision().get(),
        payload: (&facts.payload).into(),
        audio: AudioFile {
            file: AUDIO_FILE,
            sha256: facts.content.to_string(),
            byte_length: bundle.audio.len() as u64,
        },
        profile: plan.profile.id(),
        measured: Measured {
            check: bundle.measured.check.label(),
            outcome: match bundle.measured.outcome {
                CheckOutcome::Passed => "passed",
                CheckOutcome::Failed => "failed",
            },
            detail: detail_to_wire(&bundle.measured.detail),
        },
        provenance: (&facts.provenance).into(),
        rights: (&facts.rights).into(),
        inputs: plan.inputs.iter().copied().map(PinWire::from).collect(),
        evaluation: Evaluation {
            layers: LayersWire::from(&plan.layers),
            gate_evidence: plan.gate_evidence.to_string(),
        },
        submission: Submission {
            status: "not_run",
            note: NOT_SUBMITTED,
        },
    };
    let mut bytes = serde_json::to_vec_pretty(&manifest)?;
    bytes.push(b'\n');
    Ok(bytes)
}

/// Creates `destination` (which must not exist) holding the audio bytes and then the manifest.
/// The manifest is written last, so a directory with a manifest is a complete bundle. A failure
/// removes the directory this call created.
pub fn write(destination: &Path, bundle: &Bundle) -> Result<BundleReceipt, BundleError> {
    let manifest = encode_manifest(bundle).map_err(|error| BundleError::Unavailable {
        operation: "encode the manifest",
        detail: error.to_string(),
    })?;
    if let Some(parent) = destination
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)
            .map_err(|error| unavailable("create the parent directory", &error))?;
    }
    match fs::create_dir(destination) {
        Ok(()) => {}
        Err(error) if error.kind() == ErrorKind::AlreadyExists => {
            return Err(BundleError::DestinationExists)
        }
        Err(error) => return Err(unavailable("create the bundle directory", &error)),
    }
    let written = publish_new(&destination.join(AUDIO_FILE), &bundle.audio)
        .and_then(|()| publish_new(&destination.join(MANIFEST_FILE), &manifest));
    if let Err(error) = written {
        // The directory is ours: this call just created it and nothing else wrote to it.
        let _ = fs::remove_dir_all(destination);
        return Err(match error {
            StoreError::Unavailable { operation, detail } => {
                BundleError::Unavailable { operation, detail }
            }
            other => BundleError::Unavailable {
                operation: "write the bundle",
                detail: format!("{other:?}"),
            },
        });
    }
    Ok(BundleReceipt {
        manifest: destination.join(MANIFEST_FILE).display().to_string(),
        audio: destination.join(AUDIO_FILE).display().to_string(),
        manifest_digest: Sha256Digest::of(&manifest),
    })
}

fn unavailable(operation: &'static str, error: &std::io::Error) -> BundleError {
    BundleError::Unavailable {
        operation,
        detail: error.to_string(),
    }
}
