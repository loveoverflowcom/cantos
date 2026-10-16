//! Executes one parsed command against a runtime and produces its reply.
//!
//! This layer only translates: arguments into use-case requests, typed use-case results into
//! typed replies. Every durable decision lives in the domain or the application.

use std::path::Path;

use crate::{
    application::{
        export::{prepare_export, ExportError, ExportRequest},
        ports::{BundleError, Runtime, SourceError, StoreError},
        register::{register_artifact, RegisterError, RegisterRequest, Target},
        validate::{validate_artifact, ValidateError, ValidateRequest},
        view::{list_artifacts, show_artifact, ViewError},
    },
    domain::{
        artifact::{ArtifactKind, ArtifactPayload, ManuscriptOrigin, RightsClaim},
        audio_profile::AudioProfile,
        evaluation::CheckOutcome,
        identity::ArtifactId,
        label::ClaimReference,
    },
};

use super::{
    args::{
        AddArgs, ArtifactCommand, Command, HandoffCommand, KindArg, RightsArg, ValidateArgs,
        WorkspaceCommand,
    },
    render,
    reply::{Problem, Reply},
};

pub fn execute<R: Runtime>(command: Command, runtime: &R) -> Reply {
    match command {
        Command::Capabilities => {
            let (data, text) = render::capabilities();
            Reply::Ok {
                command: "capabilities",
                data,
                text,
            }
        }
        Command::Workspace {
            command: WorkspaceCommand::Init { path },
        } => init(runtime, &path),
        Command::Artifact { command } => match command {
            ArtifactCommand::Add(args) => add(runtime, args),
            ArtifactCommand::List { workspace } => list(runtime, &workspace),
            ArtifactCommand::Show {
                workspace,
                artifact,
            } => show(runtime, &workspace, artifact),
        },
        Command::Validate(args) => validate(runtime, args),
        Command::Handoff {
            command:
                HandoffCommand::Export {
                    workspace,
                    artifact,
                    profile,
                    out,
                },
        } => export(runtime, &workspace, artifact, profile, &out),
    }
}

fn init<R: Runtime>(runtime: &R, path: &Path) -> Reply {
    const COMMAND: &str = "workspace init";
    match runtime.initialize(path) {
        Ok(outcome) => {
            let (data, text) = render::initialized(path, outcome);
            Reply::Ok {
                command: COMMAND,
                data,
                text,
            }
        }
        Err(error) => store_failure(COMMAND, path, error),
    }
}

fn add<R: Runtime>(runtime: &R, args: AddArgs) -> Reply {
    const COMMAND: &str = "artifact add";
    let rights = match rights_claim(args.rights, args.rights_reference.as_deref()) {
        Ok(rights) => rights,
        Err(message) => return Reply::Usage { message },
    };
    let store = match runtime.open(&args.workspace) {
        Ok(store) => store,
        Err(error) => return store_failure(COMMAND, &args.workspace, error),
    };
    let bytes = match runtime.read_source(&args.file) {
        Ok(bytes) => bytes,
        Err(error) => return source_failure(COMMAND, &args.file, error),
    };
    let payload = payload_of(args.kind);
    let kind = payload.kind();
    let request = RegisterRequest {
        target: args.artifact.map_or(Target::New, Target::Existing),
        payload,
        producer: args.producer,
        rights,
        inputs: args.inputs,
        bytes,
    };
    match register_artifact(&store, runtime.ids(), request) {
        Ok(outcome) => {
            let (data, text) = render::registered(kind, outcome);
            Reply::Ok {
                command: COMMAND,
                data,
                text,
            }
        }
        Err(error) => register_failure(COMMAND, &args.workspace, error),
    }
}

fn list<R: Runtime>(runtime: &R, workspace: &Path) -> Reply {
    const COMMAND: &str = "artifact list";
    let store = match runtime.open(workspace) {
        Ok(store) => store,
        Err(error) => return store_failure(COMMAND, workspace, error),
    };
    match list_artifacts(&store) {
        Ok(summaries) => {
            let (data, text) = render::listed(&summaries);
            Reply::Ok {
                command: COMMAND,
                data,
                text,
            }
        }
        Err(error) => view_failure(COMMAND, workspace, error),
    }
}

fn show<R: Runtime>(runtime: &R, workspace: &Path, artifact: ArtifactId) -> Reply {
    const COMMAND: &str = "artifact show";
    let store = match runtime.open(workspace) {
        Ok(store) => store,
        Err(error) => return store_failure(COMMAND, workspace, error),
    };
    match show_artifact(&store, artifact) {
        Ok(detail) => {
            let (data, text) = render::shown(artifact, &detail);
            Reply::Ok {
                command: COMMAND,
                data,
                text,
            }
        }
        Err(error) => view_failure(COMMAND, workspace, error),
    }
}

fn validate<R: Runtime>(runtime: &R, args: ValidateArgs) -> Reply {
    const COMMAND: &str = "validate";
    let store = match runtime.open(&args.workspace) {
        Ok(store) => store,
        Err(error) => return store_failure(COMMAND, &args.workspace, error),
    };
    let request = ValidateRequest {
        artifact: args.artifact,
        profile: args.profile,
    };
    match validate_artifact(&store, runtime.ids(), request) {
        Ok(validated) => {
            let (data, text, passed) = render::validated(&validated);
            if passed {
                Reply::Ok {
                    command: COMMAND,
                    data,
                    text,
                }
            } else {
                Reply::CheckFailed {
                    command: COMMAND,
                    data,
                    text,
                }
            }
        }
        Err(error) => validate_failure(COMMAND, &args.workspace, error),
    }
}

fn export<R: Runtime>(
    runtime: &R,
    workspace: &Path,
    artifact: ArtifactId,
    profile: AudioProfile,
    out: &Path,
) -> Reply {
    const COMMAND: &str = "handoff export";
    let store = match runtime.open(workspace) {
        Ok(store) => store,
        Err(error) => return store_failure(COMMAND, workspace, error),
    };
    let bundle = match prepare_export(&store, ExportRequest { artifact, profile }) {
        Ok(bundle) => bundle,
        Err(error) => return export_failure(COMMAND, workspace, artifact, profile, error),
    };
    match runtime.write_bundle(out, &bundle) {
        Ok(receipt) => {
            let (data, text) = render::exported(&bundle.plan, &receipt);
            Reply::Ok {
                command: COMMAND,
                data,
                text,
            }
        }
        Err(BundleError::DestinationExists) => Reply::Refused {
            command: COMMAND,
            problem: Problem::new(
                "bundle_destination_exists",
                format!(
                    "{} already exists; a bundle is never merged or overwritten",
                    out.display()
                ),
            )
            .action("choose a new --out directory"),
        },
        Err(BundleError::Unavailable { operation, detail }) => Reply::Failed {
            command: COMMAND,
            problem: Problem::new("io", format!("could not {operation}: {detail}")),
        },
    }
}

fn payload_of(kind: KindArg) -> ArtifactPayload {
    match kind {
        KindArg::ManuscriptOriginal => ArtifactPayload::Manuscript {
            origin: ManuscriptOrigin::Original,
        },
        KindArg::ManuscriptCollected => ArtifactPayload::Manuscript {
            origin: ManuscriptOrigin::Collected,
        },
        KindArg::ScriptIr => ArtifactPayload::ScriptIr,
        KindArg::Audio => ArtifactPayload::Audio,
        KindArg::Report => ArtifactPayload::Report,
        KindArg::InstructionBundle => ArtifactPayload::InstructionBundle,
    }
}

/// `licensed` and `permitted-source` need the note that supports them; the others take none.
fn rights_claim(rights: RightsArg, reference: Option<&str>) -> Result<RightsClaim, String> {
    let parse_reference = |text: &str| {
        ClaimReference::parse(text).map_err(|error| {
            format!(
                "invalid --rights-reference ({error:?}; 1 to {} characters)",
                ClaimReference::MAX_CHARS
            )
        })
    };
    match (rights, reference) {
        (RightsArg::Unstated, None) => Ok(RightsClaim::Unstated),
        (RightsArg::OriginalWork, None) => Ok(RightsClaim::OriginalWork),
        (RightsArg::Licensed, Some(text)) => Ok(RightsClaim::Licensed {
            reference: parse_reference(text)?,
        }),
        (RightsArg::PermittedSource, Some(text)) => Ok(RightsClaim::PermittedSource {
            reference: parse_reference(text)?,
        }),
        (RightsArg::Licensed | RightsArg::PermittedSource, None) => {
            Err("--rights licensed and permitted-source need --rights-reference".to_owned())
        }
        (RightsArg::Unstated | RightsArg::OriginalWork, Some(_)) => {
            Err("--rights-reference is only accepted with licensed or permitted-source".to_owned())
        }
    }
}

fn workspace_action(workspace: &Path) -> String {
    format!("cantos-forge workspace init --path {}", workspace.display())
}

fn store_failure(command: &'static str, workspace: &Path, error: StoreError) -> Reply {
    match error {
        StoreError::NotInitialized => Reply::Blocked {
            command,
            problem: Problem::new(
                "workspace_missing",
                format!("{} is not a Cantos Forge workspace", workspace.display()),
            )
            .action(workspace_action(workspace)),
        },
        StoreError::NotEmptyDirectory => Reply::Refused {
            command,
            problem: Problem::new(
                "workspace_directory_not_empty",
                format!(
                    "{} holds other files and no workspace; Forge never writes into a directory it did not create",
                    workspace.display()
                ),
            )
            .action("choose an empty or new directory"),
        },
        StoreError::UnsupportedWorkspace { found } => Reply::Failed {
            command,
            problem: Problem::new(
                "workspace_unsupported",
                format!("the workspace uses format `{found}`, which this build does not know"),
            ),
        },
        StoreError::Conflict => Reply::Refused {
            command,
            problem: Problem::new(
                "conflict",
                "another writer created the same record first; nothing was overwritten",
            )
            .action("run the command again"),
        },
        StoreError::Corrupt(corruption) => Reply::Failed {
            command,
            problem: Problem::new(
                "workspace_corrupt",
                "a stored record cannot be trusted; nothing was changed",
            )
            .details(render::corruption_json(&corruption)),
        },
        StoreError::Unavailable { operation, detail } => Reply::Failed {
            command,
            problem: Problem::new("io", format!("could not {operation}: {detail}")),
        },
    }
}

fn source_failure(command: &'static str, file: &Path, error: SourceError) -> Reply {
    match error {
        SourceError::NotARegularFile => Reply::Refused {
            command,
            problem: Problem::new(
                "source_not_a_regular_file",
                format!("{} is not a regular file", file.display()),
            ),
        },
        SourceError::TooLarge { max_bytes } => Reply::Refused {
            command,
            problem: Problem::new(
                "source_too_large",
                format!(
                    "{} is larger than the {max_bytes}-byte limit",
                    file.display()
                ),
            ),
        },
        SourceError::Unavailable { detail } => Reply::Failed {
            command,
            problem: Problem::new("io", format!("could not read {}: {detail}", file.display())),
        },
    }
}

fn register_failure(command: &'static str, workspace: &Path, error: RegisterError) -> Reply {
    let refused = |problem: Problem| Reply::Refused { command, problem };
    match error {
        RegisterError::UnknownArtifact(artifact) => refused(
            Problem::new(
                "unknown_artifact",
                format!("{artifact} is not in this workspace"),
            )
            .action("omit --artifact to create a new artifact"),
        ),
        RegisterError::UnknownInput(input) => refused(Problem::new(
            "unknown_input",
            format!(
                "{}@{} is not a recorded revision",
                input.artifact,
                input.revision.get()
            ),
        )),
        RegisterError::RepeatedInput(error) => refused(Problem::new(
            "repeated_input",
            format!("an artifact can be pinned once per revision ({error:?})"),
        )),
        RegisterError::Refused(error) => refused(Problem::new(
            "revision_refused",
            format!("the registration rules refuse this revision: {error:?}"),
        )),
        RegisterError::Ids(_) => Reply::Failed {
            command,
            problem: Problem::new(
                "ids_unavailable",
                "the operating system's random source failed",
            ),
        },
        RegisterError::Store(error) => store_failure(command, workspace, error),
    }
}

fn validate_failure(command: &'static str, workspace: &Path, error: ValidateError) -> Reply {
    let refused = |problem: Problem| Reply::Refused { command, problem };
    match error {
        ValidateError::UnknownArtifact(artifact) => refused(Problem::new(
            "unknown_artifact",
            format!("{artifact} is not in this workspace"),
        )),
        ValidateError::ProfileRequired => refused(
            Problem::new(
                "profile_required",
                "audio is measured against a profile the producer chooses; there is no default",
            )
            .action("pass --profile opus-mono-20-v1, opus-mono-24-v1, opus-mono-32-v1 or opus-mono-40-v1"),
        ),
        ValidateError::ProfileNotApplicable { kind } => refused(Problem::new(
            "profile_not_applicable",
            format!("--profile only applies to audio, not {}", kind_label(kind)),
        )),
        ValidateError::NoCheckFor { kind } => refused(Problem::new(
            "no_check_for_kind",
            format!(
                "Forge has no deterministic check for {}; its content is validated elsewhere or not at all yet",
                kind_label(kind)
            ),
        )),
        ValidateError::CheckName(error) => Reply::Failed {
            command,
            problem: Problem::new("internal", format!("a check name is not a valid label: {error:?}")),
        },
        ValidateError::Ids(_) => Reply::Failed {
            command,
            problem: Problem::new("ids_unavailable", "the operating system's random source failed"),
        },
        ValidateError::Store(error) => store_failure(command, workspace, error),
    }
}

fn export_failure(
    command: &'static str,
    workspace: &Path,
    artifact: ArtifactId,
    profile: AudioProfile,
    error: ExportError,
) -> Reply {
    match error {
        ExportError::UnknownArtifact(artifact) => Reply::Refused {
            command,
            problem: Problem::new(
                "unknown_artifact",
                format!("{artifact} is not in this workspace"),
            ),
        },
        ExportError::Refused(refusals) => {
            let details: Vec<_> = refusals
                .iter()
                .map(|refusal| render::refusal_json(refusal, workspace, artifact, profile))
                .collect();
            Reply::Refused {
                command,
                problem: Problem::new(
                    "export_refused",
                    format!(
                        "{artifact} cannot be exported yet ({} reason{})",
                        details.len(),
                        if details.len() == 1 { "" } else { "s" }
                    ),
                )
                .details(serde_json::Value::Array(details)),
            }
        }
        ExportError::RemeasureFailed(result) => Reply::Failed {
            command,
            problem: Problem::new(
                "remeasure_failed",
                "the stored bytes passed their recorded check but fail the same check now",
            )
            .details(serde_json::json!({
                "outcome": match result.outcome {
                    CheckOutcome::Passed => "passed",
                    CheckOutcome::Failed => "failed",
                },
                "detail": crate::wire::detail_to_wire(&result.detail),
            })),
        },
        ExportError::CheckName(error) => Reply::Failed {
            command,
            problem: Problem::new(
                "internal",
                format!("a check name is not a valid label: {error:?}"),
            ),
        },
        ExportError::History(error) => Reply::Failed {
            command,
            problem: Problem::new(
                "workspace_corrupt",
                format!("recorded history is inconsistent: {error:?}"),
            ),
        },
        ExportError::Store(error) => store_failure(command, workspace, error),
    }
}

fn view_failure(command: &'static str, workspace: &Path, error: ViewError) -> Reply {
    match error {
        ViewError::UnknownArtifact(artifact) => Reply::Refused {
            command,
            problem: Problem::new(
                "unknown_artifact",
                format!("{artifact} is not in this workspace"),
            ),
        },
        ViewError::History(error) => Reply::Failed {
            command,
            problem: Problem::new(
                "workspace_corrupt",
                format!("recorded history is inconsistent: {error:?}"),
            ),
        },
        ViewError::Store(error) => store_failure(command, workspace, error),
    }
}

fn kind_label(kind: ArtifactKind) -> &'static str {
    render::kind_name(kind)
}
