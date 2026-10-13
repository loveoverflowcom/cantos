//! Command-line arguments. Values are parsed into domain types here, so a malformed identity,
//! label or profile is a usage error (exit 2) before anything runs.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

use crate::{
    application::register::InputRef,
    domain::{
        artifact::RevisionNumber, audio_profile::AudioProfile, identity::ArtifactId,
        label::ProducerLabel,
    },
};

#[derive(Debug, Parser)]
#[command(
    name = "cantos-forge",
    version,
    disable_help_subcommand = true,
    about = "Producer-side Cantos Forge. Runs outside the Cantos API: it keeps local artifact \
             revisions and evidence and prepares local handoff bundles. It never generates \
             audio, calls a provider, uploads or publishes."
)]
pub struct Cli {
    /// Write one JSON envelope to stdout.
    #[arg(long, global = true)]
    pub json: bool,
    #[command(subcommand)]
    pub command: Command,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Report what this build implements. Anything not listed is unsupported.
    Capabilities,
    /// Create or inspect a producer workspace.
    Workspace {
        #[command(subcommand)]
        command: WorkspaceCommand,
    },
    /// Register, list and show artifact revisions.
    Artifact {
        #[command(subcommand)]
        command: ArtifactCommand,
    },
    /// Run a deterministic check on an artifact's newest revision and record the evidence.
    Validate(ValidateArgs),
    /// Prepare local handoff files. Nothing is uploaded.
    Handoff {
        #[command(subcommand)]
        command: HandoffCommand,
    },
}

#[derive(Debug, Subcommand)]
pub enum WorkspaceCommand {
    /// Create a workspace in an empty or missing directory (a no-op if one exists).
    Init {
        #[arg(long)]
        path: PathBuf,
    },
}

#[derive(Debug, Subcommand)]
pub enum ArtifactCommand {
    /// Register a file as the first or a later revision of an artifact.
    Add(AddArgs),
    /// List every artifact with its newest revision.
    List {
        #[arg(long)]
        workspace: PathBuf,
    },
    /// Show one artifact's revisions, input freshness, evidence and evaluation layers.
    Show {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long, value_parser = artifact_id)]
        artifact: ArtifactId,
    },
}

#[derive(Debug, Args)]
pub struct AddArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long, value_enum)]
    pub kind: KindArg,
    /// The file whose exact bytes are registered (at most 64 MiB).
    #[arg(long)]
    pub file: PathBuf,
    /// Who made the bytes: an AI host session or a person. A declared label, not an identity.
    #[arg(long, value_parser = producer)]
    pub producer: ProducerLabel,
    /// The producer's rights claim. There is no default; a claim is never verified rights.
    #[arg(long, value_enum)]
    pub rights: RightsArg,
    /// The licence or permission note that supports `licensed` and `permitted-source`.
    #[arg(long)]
    pub rights_reference: Option<String>,
    /// Add a revision to this existing artifact instead of creating a new one.
    #[arg(long, value_parser = artifact_id)]
    pub artifact: Option<ArtifactId>,
    /// Pin an exact input revision as `art_<id>@<revision>`; its digest comes from history.
    #[arg(long = "input", value_parser = input_ref)]
    pub inputs: Vec<InputRef>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum KindArg {
    ManuscriptOriginal,
    ManuscriptCollected,
    ScriptIr,
    Audio,
    Report,
    InstructionBundle,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum RightsArg {
    Unstated,
    OriginalWork,
    Licensed,
    PermittedSource,
}

#[derive(Debug, Args)]
pub struct ValidateArgs {
    #[arg(long)]
    pub workspace: PathBuf,
    #[arg(long, value_parser = artifact_id)]
    pub artifact: ArtifactId,
    /// Required for audio (opus-mono-20-v1, -24-v1, -32-v1 or -40-v1); not accepted for text.
    #[arg(long, value_parser = profile)]
    pub profile: Option<AudioProfile>,
}

#[derive(Debug, Subcommand)]
pub enum HandoffCommand {
    /// Write a bundle (exact audio bytes and a manifest) for an audio artifact's newest revision.
    Export {
        #[arg(long)]
        workspace: PathBuf,
        #[arg(long, value_parser = artifact_id)]
        artifact: ArtifactId,
        /// The profile whose passing check opens the export; there is no default.
        #[arg(long, value_parser = profile)]
        profile: AudioProfile,
        /// A directory that must not exist yet.
        #[arg(long)]
        out: PathBuf,
    },
}

fn artifact_id(text: &str) -> Result<ArtifactId, String> {
    ArtifactId::parse(text).map_err(|_| {
        format!("`{text}` is not an artifact id (expected `art_` and 32 lowercase hex digits)")
    })
}

fn producer(text: &str) -> Result<ProducerLabel, String> {
    ProducerLabel::parse(text).map_err(|error| {
        format!(
            "invalid producer label ({error:?}; 1 to {} characters, no surrounding space or control characters)",
            ProducerLabel::MAX_CHARS
        )
    })
}

fn profile(text: &str) -> Result<AudioProfile, String> {
    AudioProfile::parse(text).map_err(|_| {
        let known: Vec<&str> = AudioProfile::ALL
            .iter()
            .map(|profile| profile.id())
            .collect();
        format!(
            "`{text}` is not an intake profile (known: {})",
            known.join(", ")
        )
    })
}

fn input_ref(text: &str) -> Result<InputRef, String> {
    let expected = || format!("`{text}` is not `art_<id>@<revision>` with a revision of 1 or more");
    let (artifact, revision) = text.split_once('@').ok_or_else(expected)?;
    let artifact = ArtifactId::parse(artifact).map_err(|_| expected())?;
    let revision: u32 = revision.parse().map_err(|_| expected())?;
    let revision = RevisionNumber::new(revision).map_err(|_| expected())?;
    Ok(InputRef { artifact, revision })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn input_references_need_an_id_an_at_sign_and_a_positive_revision() {
        let id = ArtifactId::from_bits(7).to_string();
        let parsed = input_ref(&format!("{id}@3")).unwrap();
        assert_eq!(parsed.artifact, ArtifactId::from_bits(7));
        assert_eq!(parsed.revision.get(), 3);
        for bad in [
            id.clone(),
            format!("{id}@"),
            format!("{id}@0"),
            format!("{id}@-1"),
            format!("{id}@x"),
            format!("{id}@1@2"),
            "@1".to_owned(),
            format!("evd_{:032x}@1", 7),
        ] {
            assert!(input_ref(&bad).is_err(), "{bad}");
        }
    }

    #[test]
    fn the_profile_error_lists_the_known_profiles() {
        let message = profile("aac-mono-32").unwrap_err();
        assert!(message.contains("opus-mono-24-v1"), "{message}");
    }
}
