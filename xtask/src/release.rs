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
    /// Run release contract, packaging, DAG and mocked promotion regressions.
    SelfTest,
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
}

fn command(workspace: &Workspace, action: &Action) -> CommandSpec {
    let command = CommandSpec::new("python3", workspace.root());
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
    if let Action::OciVerify {
        manifest,
        manifest_digest,
        source_sha,
        configuration_digest,
        platform,
        admission,
        admission_digest,
        dry_run,
        registry_readback,
        authfile,
    } = action
    {
        let command = command
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
        return if let Some(path) = authfile {
            command.args(["--authfile".into(), path.display().to_string()])
        } else {
            command
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
        Action::SelfTest | Action::OciVerify { .. } => unreachable!("handled above"),
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
        commands.push(CommandSpec::new("python3", workspace.root()).args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_vps_*.py",
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
    }
    let needed: &[Tool] = if matches!(args.action, Action::SelfTest) {
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
    use super::{command, Action};
    use crate::cli::Cli;
    use crate::workspace::Workspace;
    use clap::Parser;

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
    fn artifact_input_requires_exact_manifest_digest_and_source_and_never_accepts_a_dist_bypass() {
        let base = [
            "xtask",
            "deploy",
            "vps",
            "cantos-server",
            "--host",
            "root@h",
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
}
