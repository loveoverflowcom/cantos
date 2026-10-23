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
        Action::SelfTest | Action::OciVerify { .. } | Action::OciBuild { .. } => {
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
}
