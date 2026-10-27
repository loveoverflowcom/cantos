//! Release orchestration and verified artifact input; ordinary gates never publish.
use std::{
    path::{Path, PathBuf},
    process::ExitCode,
};

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{
    command::{CommandSpec, Runner},
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

/// An explicit artifact identity, never an arbitrary dist directory.
#[derive(Debug, Clone, Default, Args, PartialEq, Eq)]
pub struct ArtifactArgs {
    #[arg(long, requires_all = ["release_digest", "release_sha"])]
    pub release_manifest: Option<PathBuf>,
    #[arg(long, requires = "release_manifest")]
    pub release_digest: Option<String>,
    #[arg(long, requires = "release_manifest")]
    pub release_sha: Option<String>,
    /// Same-workflow resolved request. Required in CircleCI; never cross-pipeline promotion.
    #[arg(long, requires = "release_manifest", conflicts_with = "local_artifact")]
    pub release_request: Option<PathBuf>,
    /// Acknowledge independently verified local provenance (refused inside CircleCI).
    #[arg(long, requires = "release_manifest")]
    pub local_artifact: bool,
}

impl ArtifactArgs {
    pub fn append(&self, mut command: CommandSpec) -> CommandSpec {
        if let Some(path) = &self.release_manifest {
            command = command.args(["--release-manifest".into(), path.display().to_string()]);
        }
        for (name, value) in [
            ("--release-digest", &self.release_digest),
            ("--release-sha", &self.release_sha),
        ] {
            if let Some(value) = value {
                command = command.args([name, value]);
            }
        }
        if let Some(path) = &self.release_request {
            command = command.args(["--release-request".into(), path.display().to_string()]);
        }
        command.arg_if(self.local_artifact, "--local-artifact")
    }

    pub fn extract_command(
        &self,
        workspace: &Workspace,
        component: &str,
        origin: &str,
        output: &Path,
    ) -> CommandSpec {
        self.append(CommandSpec::new("python3", workspace.root()).args([
            "scripts/release_pipeline.py".into(),
            "extract".into(),
            "--component".into(),
            component.into(),
            "--api-origin".into(),
            origin.into(),
            "--output".into(),
            output.display().to_string(),
        ]))
    }
}

#[derive(Debug, Args)]
pub struct ReleaseArgs {
    #[command(subcommand)]
    pub action: Action,
}

impl ReleaseArgs {
    /// The `api` component was the legacy VPS archive (image tarball + `infra/vps` bundle).
    pub fn names_legacy_api(&self) -> bool {
        matches!(
            &self.action,
            Action::Prepare { component, .. }
                | Action::Intent { component, .. }
                | Action::Promote { component, .. }
            if component == "api" || component == "all"
        )
    }
}

#[derive(Debug, Subcommand)]
pub enum Action {
    /// Resolve once, verify and build selected components without production credentials.
    Prepare {
        #[arg(long)]
        target: Option<String>,
        #[arg(long)]
        build_only: bool,
        #[arg(long)]
        component: String,
        #[arg(long)]
        api_origin: String,
        #[arg(long)]
        transfer: PathBuf,
    },
    /// Validate the same-workflow request and checkout its immutable release revision.
    Checkout {
        #[arg(long)]
        transfer: PathBuf,
    },
    /// Prepare the per-job intent for workspace persistence before any remote write.
    Intent {
        #[arg(long)]
        transfer: PathBuf,
        #[arg(long)]
        component: String,
    },
    /// Explicit remote write of one verified same-workflow artifact. Never retries.
    Promote {
        #[arg(long)]
        transfer: PathBuf,
        #[arg(long)]
        component: String,
    },
    /// Report every selected component; absent/unknown outcomes never become success.
    Report {
        #[arg(long)]
        transfer: PathBuf,
    },
    /// Bind one verified same-workflow Pages artifact and destination before CI persistence.
    PagesIntent {
        #[arg(long)]
        transfer: PathBuf,
        #[arg(long, value_parser = ["theatre-web", "studio-web"])]
        component: String,
    },
    /// Invoke the existing Pages publisher once using a persisted, exact-job intent.
    PagesPromote {
        #[arg(long)]
        transfer: PathBuf,
        #[arg(long, value_parser = ["theatre-web", "studio-web"])]
        component: String,
    },
    /// Run release contract, packaging, DAG and mocked promotion regressions.
    SelfTest,
    /// Build a static, source-pinned Linux deployment executor on a disposable builder.
    /// Requires the pinned Rust/musl build tools there; never installs anything on a host.
    NativeExecutorBuild {
        #[arg(long)]
        output: PathBuf,
    },
    /// Verify an admitted native OCI pair offline; optionally read its exact GHCR metadata.
    /// This never builds, publishes, pulls image layers or activates a host.
    OciVerify {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        manifest_digest: String,
        #[arg(long)]
        source_sha: String,
        #[arg(long)]
        configuration_digest: String,
        #[arg(long)]
        platform: String,
        /// Operator-reviewed producer admission; hashes alone do not establish trust.
        #[arg(long)]
        admission: PathBuf,
        #[arg(long)]
        admission_digest: String,
        /// Optional operator-declared inventory; an offline plan, never host activation authority.
        #[arg(long, requires = "host_inventory_digest")]
        host_inventory: Option<PathBuf>,
        #[arg(long, requires = "host_inventory")]
        host_inventory_digest: Option<String>,
        /// Same read-only plan; no SSH, registry access, subprocess build or output write.
        #[arg(long)]
        dry_run: bool,
        /// Explicit read-only GHCR manifest/config observation; dry-run stays offline.
        #[arg(long)]
        registry_readback: bool,
        /// Private, existing GHCR authfile; no implicit login or credential helper.
        #[arg(long, requires = "registry_readback")]
        authfile: Option<PathBuf>,
    },
    /// Existing NATIVE_READY rootful host only. Default is offline validation; execution
    /// requires admitted exact pairs and never bootstraps, migrates, or connects over SSH.
    NativeUpgrade {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        request_digest: String,
        #[arg(long)]
        execute: bool,
        #[arg(long)]
        rollback_attempt: Option<String>,
    },
    /// Explicit reviewed existing-layout v23 -> v24 adoption. Offline by default; execution
    /// requires separate pinned operator approval. Never connects to a remote host.
    NativeAdopt {
        #[arg(long)]
        request: PathBuf,
        #[arg(long)]
        request_digest: String,
        #[arg(long, requires_all = ["approval", "approval_digest"])]
        execute: bool,
        #[arg(long)]
        approval: Option<PathBuf>,
        #[arg(long)]
        approval_digest: Option<String>,
        #[arg(long)]
        recover_attempt: Option<String>,
    },
    /// Historical upgrade and pair restore on disposable Linux using synthetic data and exact
    /// old/new images. Separate from the four current-schema OCI qualification records.
    NativeRehearse {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        manifest_digest: String,
        #[arg(long)]
        source_sha: String,
        #[arg(long)]
        configuration_digest: String,
        #[arg(long)]
        platform: String,
        #[arg(long)]
        authfile: Option<PathBuf>,
        #[arg(long)]
        output: PathBuf,
        #[arg(long)]
        baseline: PathBuf,
        #[arg(long)]
        baseline_digest: String,
        /// Explicit bounded transition; older v23 -> v24 behavior remains the default.
        #[arg(long, value_parser = ["v23-v24", "v24-v25", "v24-v26"], default_value = "v23-v24")]
        transition: String,
    },
    /// Build the native Server/Identity image pair from one clean commit; with `--publish`, push
    /// it to GHCR and write an unadmitted candidate. Never activates a host or marks it eligible.
    OciBuild {
        /// Exactly `linux/amd64` or `linux/arm64`, and equal to this host: no emulation.
        #[arg(long)]
        platform: String,
        #[arg(long, value_parser = ["local", "github-actions"])]
        producer: String,
        /// Bounded nonsecret identifier; required for, and only for, the local producer.
        #[arg(long, required_if_eq("producer", "local"))]
        build_id: Option<String>,
        /// Push to GHCR. Requires the private authfile; there is no implicit login.
        #[arg(long, requires = "authfile")]
        publish: bool,
        #[arg(long, requires = "publish")]
        authfile: Option<PathBuf>,
    },
    /// Qualify an exact unadmitted image pair on disposable native Linux, using real containers.
    /// Requires rootful Podman; never publishes, admits images or activates a host.
    OciQualify {
        #[arg(long)]
        manifest: PathBuf,
        #[arg(long)]
        manifest_digest: String,
        #[arg(long)]
        source_sha: String,
        #[arg(long)]
        configuration_digest: String,
        #[arg(long)]
        platform: String,
        #[arg(long)]
        authfile: Option<PathBuf>,
        /// Fresh immutable output directory; the input candidate is never overwritten.
        #[arg(long)]
        output: PathBuf,
    },
}

fn command(workspace: &Workspace, action: &Action) -> CommandSpec {
    let command = CommandSpec::new("python3", workspace.root());
    if let Action::NativeExecutorBuild { output } = action {
        return command.args([
            "scripts/release_native_executor.py".into(),
            "--output".into(),
            output.display().to_string(),
        ]);
    }
    if matches!(action, Action::SelfTest) {
        return command.args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_release*.py",
        ]);
    }
    if let Action::NativeUpgrade {
        request,
        request_digest,
        execute,
        rollback_attempt,
    } = action
    {
        let mut command = command
            .args([
                "scripts/release_native.py".into(),
                "--request".into(),
                request.display().to_string(),
                "--request-digest".into(),
                request_digest.clone(),
            ])
            .arg_if(*execute, "--execute");
        if let Some(attempt) = rollback_attempt {
            command = command.args(["--rollback-attempt", attempt]);
        }
        return command;
    }
    if let Action::NativeAdopt {
        request,
        request_digest,
        execute,
        approval,
        approval_digest,
        recover_attempt,
    } = action
    {
        let mut command = command
            .args([
                "scripts/release_native_migration.py".into(),
                "--request".into(),
                request.display().to_string(),
                "--request-digest".into(),
                request_digest.clone(),
            ])
            .arg_if(*execute, "--execute");
        if let Some(path) = approval {
            command = command.args(["--approval".into(), path.display().to_string()]);
        }
        if let Some(digest) = approval_digest {
            command = command.args(["--approval-digest", digest]);
        }
        if let Some(attempt) = recover_attempt {
            command = command.args(["--recover-attempt", attempt]);
        }
        return command;
    }
    if let Action::OciQualify {
        manifest,
        manifest_digest,
        source_sha,
        configuration_digest,
        platform,
        authfile,
        output,
    }
    | Action::NativeRehearse {
        manifest,
        manifest_digest,
        source_sha,
        configuration_digest,
        platform,
        authfile,
        output,
        ..
    } = action
    {
        let script = match action {
            Action::NativeRehearse { transition, .. } if transition == "v24-v26" => {
                "scripts/release_native_v26_rehearsal.py"
            }
            Action::NativeRehearse { transition, .. } if transition == "v24-v25" => {
                "scripts/release_native_v25_rehearsal.py"
            }
            Action::NativeRehearse { .. } => "scripts/release_native_rehearsal.py",
            _ => "scripts/release_oci_qualify.py",
        };
        let mut command = command.args([
            script.into(),
            "--manifest".into(),
            manifest.display().to_string(),
            "--manifest-digest".into(),
            manifest_digest.clone(),
            "--source-sha".into(),
            source_sha.clone(),
            "--configuration-digest".into(),
            configuration_digest.clone(),
            "--platform".into(),
            platform.clone(),
            "--output".into(),
            output.display().to_string(),
        ]);
        if let Action::NativeRehearse {
            baseline,
            baseline_digest,
            ..
        } = action
        {
            command = command.args([
                "--baseline".into(),
                baseline.display().to_string(),
                "--baseline-digest".into(),
                baseline_digest.clone(),
            ]);
        }
        return match authfile {
            Some(path) => command.args(["--authfile".into(), path.display().to_string()]),
            None => command,
        };
    }
    if let Action::OciVerify {
        manifest,
        manifest_digest,
        source_sha,
        configuration_digest,
        platform,
        admission,
        admission_digest,
        host_inventory,
        host_inventory_digest,
        dry_run,
        registry_readback,
        authfile,
    } = action
    {
        let mut command = command
            .args([
                "scripts/release_oci.py".into(),
                "--manifest".into(),
                manifest.display().to_string(),
                "--manifest-digest".into(),
                manifest_digest.clone(),
                "--source-sha".into(),
                source_sha.clone(),
                "--configuration-digest".into(),
                configuration_digest.clone(),
                "--platform".into(),
                platform.clone(),
                "--admission".into(),
                admission.display().to_string(),
                "--admission-digest".into(),
                admission_digest.clone(),
            ])
            .arg_if(*dry_run, "--dry-run")
            .arg_if(*registry_readback, "--registry-readback");
        if let (Some(path), Some(digest)) = (host_inventory, host_inventory_digest) {
            command = command.args([
                "--host-inventory".into(),
                path.display().to_string(),
                "--host-inventory-digest".into(),
                digest.clone(),
            ]);
        }
        return if let Some(path) = authfile {
            command.args(["--authfile".into(), path.display().to_string()])
        } else {
            command
        };
    }
    if let Action::OciBuild {
        platform,
        producer,
        build_id,
        publish,
        authfile,
    } = action
    {
        let mut command = command.args([
            "scripts/release_oci_build.py",
            "--platform",
            platform,
            "--producer",
            producer,
        ]);
        if let Some(id) = build_id {
            command = command.args(["--build-id", id]);
        }
        command = command.arg_if(*publish, "--publish");
        return match authfile {
            Some(path) => command.args(["--authfile".into(), path.display().to_string()]),
            None => command,
        };
    }
    let mut command = command.args(["scripts/release_pipeline.py"]);
    match action {
        Action::Prepare {
            target,
            build_only,
            component,
            api_origin,
            transfer,
        } => {
            command = command
                .args([
                    "prepare",
                    "--component",
                    component,
                    "--api-origin",
                    api_origin,
                ])
                .arg_if(*build_only, "--build-only");
            if let Some(target) = target {
                command = command.args(["--target", target]);
            }
            command.args(["--transfer".into(), transfer.display().to_string()])
        }
        Action::Checkout { transfer } | Action::Report { transfer } => command.args([
            if matches!(action, Action::Checkout { .. }) {
                "checkout"
            } else {
                "report"
            }
            .into(),
            "--transfer".into(),
            transfer.display().to_string(),
        ]),
        Action::PagesIntent {
            transfer,
            component,
        }
        | Action::PagesPromote {
            transfer,
            component,
        } => command.args([
            if matches!(action, Action::PagesIntent { .. }) {
                "pages-intent"
            } else {
                "pages-promote"
            }
            .into(),
            "--component".into(),
            component.clone(),
            "--transfer".into(),
            transfer.display().to_string(),
        ]),
        Action::Promote {
            transfer,
            component,
        }
        | Action::Intent {
            transfer,
            component,
        } => command.args([
            if matches!(action, Action::Promote { .. }) {
                "promote"
            } else {
                "intent"
            }
            .into(),
            "--component".into(),
            component.clone(),
            "--transfer".into(),
            transfer.display().to_string(),
        ]),
        Action::SelfTest
        | Action::NativeExecutorBuild { .. }
        | Action::OciVerify { .. }
        | Action::OciBuild { .. }
        | Action::OciQualify { .. }
        | Action::NativeAdopt { .. }
        | Action::NativeRehearse { .. }
        | Action::NativeUpgrade { .. } => {
            unreachable!("handled above")
        }
    }
}

pub fn run(
    workspace: &Workspace,
    args: &ReleaseArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let mut commands = vec![command(workspace, &args.action)];
    if matches!(args.action, Action::SelfTest) {
        commands.push(CommandSpec::new("python3", workspace.root()).args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_circleci_cloudflare.py",
        ]));
        commands.push(
            CommandSpec::new("cargo", workspace.root()).args(["fmt", "--all", "--", "--check"]),
        );
        commands.push(CommandSpec::new("cargo", workspace.root()).args([
            "clippy",
            "-p",
            "xtask",
            "--all-targets",
            "--locked",
            "--",
            "-D",
            "warnings",
        ]));
        commands.push(CommandSpec::new("cargo", workspace.root()).args([
            "test",
            "-p",
            "xtask",
            "--locked",
            "release::",
        ]));
        commands.push(
            CommandSpec::new("cargo", workspace.root())
                .args(["test", "-p", "xtask", "--locked", "deploy::"]),
        );
        commands.push(CommandSpec::new("cargo", workspace.root()).args([
            "test",
            "-p",
            "xtask",
            "--locked",
            "retired::",
        ]));
    }
    let needed: &[Tool] = if matches!(
        args.action,
        Action::SelfTest | Action::NativeExecutorBuild { .. }
    ) {
        &[Tool::Python3, Tool::Cargo]
    } else {
        &[Tool::Python3]
    };
    let report = pipeline::execute(
        "release",
        &[Step::new("release", needed, commands)],
        tools,
        runner,
    );
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    #[test]
    fn static_native_executor_build_has_an_explicit_output_and_no_host_execution_flag() {
        let command = super::command(
            &crate::workspace::Workspace::discover().unwrap(),
            &super::Action::NativeExecutorBuild {
                output: "executor-output".into(),
            },
        );
        assert_eq!(command.program, "python3");
        assert_eq!(
            command.args,
            [
                "scripts/release_native_executor.py",
                "--output",
                "executor-output"
            ]
        );
    }

    use super::{command, Action};
    use crate::cli::Cli;
    use crate::workspace::Workspace;
    use clap::Parser;

    #[test]
    fn exact_image_qualification_requires_pins_and_has_no_admission_or_deploy_switch() {
        let args = [
            "xtask",
            "release",
            "oci-qualify",
            "--manifest",
            "candidate/manifest.json",
            "--manifest-digest",
            "digest",
            "--source-sha",
            "source",
            "--configuration-digest",
            "configuration",
            "--platform",
            "linux/amd64",
            "--output",
            "qualified",
        ];
        assert!(Cli::try_parse_from(args).is_ok());
        for forbidden in ["--publish", "--execute", "--admission", "--skip"] {
            assert!(Cli::try_parse_from(args.into_iter().chain([forbidden])).is_err());
        }
        assert!(Cli::try_parse_from(&args[..args.len() - 2]).is_err());
        let action = Action::OciQualify {
            manifest: "candidate/manifest.json".into(),
            manifest_digest: "digest".into(),
            source_sha: "source".into(),
            configuration_digest: "configuration".into(),
            platform: "linux/amd64".into(),
            authfile: None,
            output: "qualified".into(),
        };
        let plan = command(&Workspace::discover().unwrap(), &action);
        assert_eq!(plan.program, "python3");
        assert_eq!(plan.args[0], "scripts/release_oci_qualify.py");
        assert!(!plan.args.iter().any(|arg| arg == "--authfile"));
        assert!(plan.env.is_empty());
    }

    #[test]
    fn native_oci_plan_has_explicit_pins_and_only_calls_the_offline_release_owner() {
        let pins = [
            "xtask",
            "release",
            "oci-verify",
            "--manifest",
            "manifest.json",
            "--manifest-digest",
            "digest",
            "--source-sha",
            "sha",
            "--configuration-digest",
            "config",
            "--platform",
            "linux/amd64",
            "--admission",
            "admission.json",
            "--admission-digest",
            "admission-digest",
        ];
        assert!(Cli::try_parse_from(pins).is_ok());
        assert!(Cli::try_parse_from(&pins[..pins.len() - 2]).is_err());
        assert!(Cli::try_parse_from(pins.into_iter().chain(["--authfile", "auth.json"])).is_err());
        assert!(
            Cli::try_parse_from(pins.into_iter().chain(["--host-inventory", "host.json"])).is_err()
        );
        assert!(Cli::try_parse_from(pins.into_iter().chain([
            "--host-inventory",
            "host.json",
            "--host-inventory-digest",
            "digest"
        ]))
        .is_ok());
        assert!(Cli::try_parse_from(pins.into_iter().chain([
            "--registry-readback",
            "--authfile",
            "auth.json",
            "--dry-run"
        ]))
        .is_ok());
        let mut action = Action::OciVerify {
            manifest: "manifest.json".into(),
            manifest_digest: "digest".into(),
            source_sha: "sha".into(),
            configuration_digest: "config".into(),
            platform: "linux/amd64".into(),
            admission: "admission.json".into(),
            admission_digest: "admission-digest".into(),
            host_inventory: None,
            host_inventory_digest: None,
            dry_run: true,
            registry_readback: false,
            authfile: None,
        };
        let workspace = Workspace::discover().unwrap();
        let plan = command(&workspace, &action);
        assert_eq!(plan.program, "python3");
        assert_eq!(plan.args[0], "scripts/release_oci.py");
        assert_eq!(&plan.args[1..pins.len() - 2], &pins[3..]);
        assert_eq!(plan.args.last().unwrap(), "--dry-run");
        assert!(plan.env.is_empty());
        if let Action::OciVerify {
            registry_readback,
            authfile,
            ..
        } = &mut action
        {
            *registry_readback = true;
            *authfile = Some("auth.json".into());
        }
        let plan = command(&workspace, &action);
        assert_eq!(
            &plan.args[plan.args.len() - 4..],
            &[
                "--dry-run",
                "--registry-readback",
                "--authfile",
                "auth.json"
            ]
        );
    }

    #[test]
    fn native_upgrade_is_explicit_and_does_not_enter_legacy_deploy() {
        let base = [
            "xtask",
            "release",
            "native-upgrade",
            "--request",
            "request.json",
            "--request-digest",
            "digest",
        ];
        assert!(Cli::try_parse_from(base).is_ok());
        assert!(Cli::try_parse_from(&base[..5]).is_err());
        let action = Action::NativeUpgrade {
            request: "request.json".into(),
            request_digest: "digest".into(),
            execute: false,
            rollback_attempt: None,
        };
        let cmd = command(&Workspace::discover().unwrap(), &action);
        assert_eq!(
            cmd.args,
            [
                "scripts/release_native.py",
                "--request",
                "request.json",
                "--request-digest",
                "digest"
            ]
        );
        assert!(!cmd.args.iter().any(|arg| arg == "--execute"));
        assert!(Cli::try_parse_from(base.into_iter().chain([
            "--execute",
            "--rollback-attempt",
            "attempt-abc"
        ]))
        .is_ok());
    }

    #[test]
    fn pages_ci_commands_route_to_the_existing_release_owner_and_refuse_non_web_components() {
        for (verb, action) in [
            (
                "pages-intent",
                Action::PagesIntent {
                    transfer: "t".into(),
                    component: "theatre-web".into(),
                },
            ),
            (
                "pages-promote",
                Action::PagesPromote {
                    transfer: "t".into(),
                    component: "theatre-web".into(),
                },
            ),
        ] {
            assert!(Cli::try_parse_from([
                "xtask",
                "release",
                verb,
                "--transfer",
                "t",
                "--component",
                "theatre-web"
            ])
            .is_ok());
            assert!(Cli::try_parse_from([
                "xtask",
                "release",
                verb,
                "--transfer",
                "t",
                "--component",
                "api"
            ])
            .is_err());
            let plan = command(&Workspace::discover().unwrap(), &action);
            assert_eq!(plan.program, "python3");
            assert_eq!(
                plan.args,
                [
                    "scripts/release_pipeline.py",
                    verb,
                    "--component",
                    "theatre-web",
                    "--transfer",
                    "t"
                ]
            );
            assert!(plan.env.is_empty());
        }
    }

    #[test]
    fn native_oci_build_requires_identity_and_never_publishes_without_a_private_authfile() {
        let base = [
            "xtask",
            "release",
            "oci-build",
            "--platform",
            "linux/amd64",
            "--producer",
            "local",
        ];
        assert!(Cli::try_parse_from(base).is_err(), "local needs a build id");
        let local = || base.into_iter().chain(["--build-id", "operator-1"]);
        assert!(Cli::try_parse_from(local()).is_ok());
        assert!(Cli::try_parse_from(local().chain(["--publish"])).is_err());
        assert!(Cli::try_parse_from(local().chain(["--authfile", "a.json"])).is_err());
        assert!(Cli::try_parse_from(local().chain(["--publish", "--authfile", "a.json"])).is_ok());
        assert!(Cli::try_parse_from([
            "xtask",
            "release",
            "oci-build",
            "--platform",
            "linux/amd64",
            "--producer",
            "other"
        ])
        .is_err());
        let action = Action::OciBuild {
            platform: "linux/amd64".into(),
            producer: "github-actions".into(),
            build_id: None,
            publish: true,
            authfile: Some("a.json".into()),
        };
        let plan = command(&Workspace::discover().unwrap(), &action);
        assert_eq!(plan.program, "python3");
        assert_eq!(plan.args[0], "scripts/release_oci_build.py");
        assert_eq!(
            &plan.args[1..],
            &[
                "--platform",
                "linux/amd64",
                "--producer",
                "github-actions",
                "--publish",
                "--authfile",
                "a.json"
            ]
        );
        assert!(plan.env.is_empty());
    }

    #[test]
    fn artifact_input_requires_exact_manifest_digest_and_source_and_never_accepts_a_dist_bypass() {
        let base = [
            "xtask",
            "deploy",
            "firebase",
            "cantos-theatre",
            "--web",
            "--project",
            "p",
            "--channel",
            "live",
        ];
        for suffix in [
            vec!["--release-manifest", "manifest.json"],
            vec!["--release-sha", "a"],
            vec!["--no-build", "/tmp/dist"],
        ] {
            assert!(Cli::try_parse_from(base.into_iter().chain(suffix)).is_err());
        }
        let suffix = [
            "--release-manifest",
            "manifest.json",
            "--release-sha",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
            "--release-digest",
            "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd",
            "--local-artifact",
        ];
        assert!(Cli::try_parse_from(base.into_iter().chain(suffix)).is_ok());
    }

    #[test]
    fn the_legacy_api_component_is_refused_and_web_components_are_not() {
        use crate::retired::Retirement;
        for verb in ["prepare", "intent", "promote"] {
            let mut line = vec![
                "xtask",
                "release",
                verb,
                "--transfer",
                "t",
                "--component",
                "api",
            ];
            if verb == "prepare" {
                line.extend(["--api-origin", "https://api.example.org"]);
            }
            let cli = Cli::try_parse_from(&line).unwrap();
            assert_eq!(
                cli.command.retirement(),
                Some(Retirement::LegacyApiRelease),
                "{verb}"
            );
            let aggregate: Vec<_> = line
                .iter()
                .map(|part| if *part == "api" { "all" } else { part })
                .collect();
            let cli = Cli::try_parse_from(&aggregate).unwrap();
            assert_eq!(cli.command.retirement(), Some(Retirement::LegacyApiRelease));
            let web: Vec<_> = line
                .iter()
                .map(|part| if *part == "api" { "theatre-web" } else { part })
                .collect();
            let cli = Cli::try_parse_from(&web).unwrap();
            assert_eq!(cli.command.retirement(), None, "{verb} theatre-web");
        }
        let checkout =
            Cli::try_parse_from(["xtask", "release", "checkout", "--transfer", "t"]).unwrap();
        assert_eq!(checkout.command.retirement(), None);
    }

    #[test]
    fn native_adoption_execution_requires_distinct_approval_and_never_enters_legacy_deploy() {
        let base = [
            "xtask",
            "release",
            "native-adopt",
            "--request",
            "request.json",
            "--request-digest",
            "digest",
        ];
        assert!(Cli::try_parse_from(base).is_ok());
        assert!(Cli::try_parse_from(base.into_iter().chain(["--execute"])).is_err());
        assert!(Cli::try_parse_from(base.into_iter().chain([
            "--execute",
            "--approval",
            "approval.json"
        ]))
        .is_err());
        assert!(Cli::try_parse_from(base.into_iter().chain([
            "--execute",
            "--approval",
            "approval.json",
            "--approval-digest",
            "approval-digest",
            "--recover-attempt",
            "attempt-abc",
        ]))
        .is_ok());
        let action = Action::NativeAdopt {
            request: "request.json".into(),
            request_digest: "digest".into(),
            execute: false,
            approval: None,
            approval_digest: None,
            recover_attempt: None,
        };
        let cmd = command(&Workspace::discover().unwrap(), &action);
        assert_eq!(
            cmd.args,
            [
                "scripts/release_native_migration.py",
                "--request",
                "request.json",
                "--request-digest",
                "digest"
            ]
        );
        assert!(!cmd.args.iter().any(|argument| argument == "--execute"));
    }

    #[test]
    fn historical_rehearsal_requires_baseline_pins_and_has_no_activation_switch() {
        let base = [
            "xtask",
            "release",
            "native-rehearse",
            "--manifest",
            "manifest.json",
            "--manifest-digest",
            "digest",
            "--source-sha",
            "source",
            "--configuration-digest",
            "configuration",
            "--platform",
            "linux/amd64",
            "--output",
            "out",
            "--baseline",
            "old.json",
            "--baseline-digest",
            "old-digest",
        ];
        assert!(Cli::try_parse_from(base).is_ok());
        assert!(Cli::try_parse_from(&base[..15]).is_err());
        assert!(Cli::try_parse_from(base.into_iter().chain(["--execute"])).is_err());
        let action = Action::NativeRehearse {
            manifest: "manifest.json".into(),
            manifest_digest: "digest".into(),
            source_sha: "source".into(),
            configuration_digest: "configuration".into(),
            platform: "linux/amd64".into(),
            authfile: None,
            output: "out".into(),
            baseline: "old.json".into(),
            baseline_digest: "old-digest".into(),
            transition: "v23-v24".into(),
        };
        let cmd = command(&Workspace::discover().unwrap(), &action);
        assert_eq!(cmd.args[0], "scripts/release_native_rehearsal.py");
        assert!(cmd
            .args
            .windows(4)
            .any(|values| values == ["--baseline", "old.json", "--baseline-digest", "old-digest"]));
        let modern = Action::NativeRehearse {
            manifest: "manifest.json".into(),
            manifest_digest: "digest".into(),
            source_sha: "source".into(),
            configuration_digest: "configuration".into(),
            platform: "linux/amd64".into(),
            authfile: None,
            output: "out".into(),
            baseline: "old.json".into(),
            baseline_digest: "old-digest".into(),
            transition: "v24-v25".into(),
        };
        assert_eq!(
            command(&Workspace::discover().unwrap(), &modern).args[0],
            "scripts/release_native_v25_rehearsal.py"
        );
        let Action::NativeRehearse {
            manifest,
            manifest_digest,
            source_sha,
            configuration_digest,
            platform,
            authfile,
            output,
            baseline,
            baseline_digest,
            ..
        } = modern
        else {
            unreachable!()
        };
        let latest = Action::NativeRehearse {
            manifest,
            manifest_digest,
            source_sha,
            configuration_digest,
            platform,
            authfile,
            output,
            baseline,
            baseline_digest,
            transition: "v24-v26".into(),
        };
        assert_eq!(
            command(&Workspace::discover().unwrap(), &latest).args[0],
            "scripts/release_native_v26_rehearsal.py"
        );
    }
}
