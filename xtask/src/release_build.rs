//! CI packaging is an explicit build lane, independent of production deployment credentials.
use std::process::ExitCode;

use anyhow::Result;
use clap::{Args, ValueEnum};

use crate::{
    command::{CommandSpec, Runner},
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum ReleaseTarget {
    Api,
    TheatreWeb,
    StudioWeb,
    Android,
}

impl ReleaseTarget {
    fn name(self) -> &'static str {
        match self {
            Self::Api => "api",
            Self::TheatreWeb => "theatre-web",
            Self::StudioWeb => "studio-web",
            Self::Android => "android",
        }
    }
}

#[derive(Debug, Args)]
pub struct ReleaseBuildArgs {
    #[arg(
        value_enum,
        required_unless_present = "self_test",
        conflicts_with = "self_test"
    )]
    pub target: Option<ReleaseTarget>,
    /// Print the packaging plan without building or writing artifacts.
    #[arg(long, conflicts_with = "self_test")]
    pub dry_run: bool,
    /// Run deterministic packaging regression tests; no build tools or cloud access.
    #[arg(long)]
    pub self_test: bool,
    /// Public exact HTTPS API origin for Web builds; required via flag or CANTOS_API_ORIGIN.
    #[arg(long, conflicts_with = "self_test")]
    pub api_origin: Option<String>,
}

fn command(workspace: &Workspace, args: &ReleaseBuildArgs) -> CommandSpec {
    let spec = CommandSpec::new("python3", workspace.root());
    if args.self_test {
        return spec.args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_*release*.py",
        ]);
    }
    let mut spec = spec.args(["scripts/release_build.py"]);
    if let Some(target) = args.target {
        spec = spec.args([target.name()]);
    }
    if let Some(origin) = &args.api_origin {
        spec = spec.args(["--api-origin", origin]);
    }
    spec.arg_if(args.dry_run, "--dry-run")
}

pub fn run(
    workspace: &Workspace,
    args: &ReleaseBuildArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let mut commands = vec![command(workspace, args)];
    if args.self_test {
        commands.push(CommandSpec::new("python3", workspace.root()).args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_theatre_android_host.py",
        ]));
        commands.push(
            CommandSpec::new("cargo", workspace.root()).args(["test", "-p", "xtask", "--locked"]),
        );
    }
    let required = if args.self_test {
        vec![Tool::Python3, Tool::Cargo]
    } else {
        vec![Tool::Python3]
    };
    let report = pipeline::execute(
        "release-build",
        &[Step::new("release-artifacts", &required, commands)],
        tools,
        runner,
    );
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Task};
    use clap::Parser;

    #[test]
    fn release_lane_requires_an_explicit_target_and_has_no_deployment_arguments() {
        assert!(Cli::try_parse_from(["xtask", "release-build"]).is_err());
        for target in ["api", "theatre-web", "studio-web", "android"] {
            let cli = Cli::try_parse_from(["xtask", "release-build", target, "--dry-run"]).unwrap();
            let Task::ReleaseBuild(args) = cli.command else {
                panic!("wrong task")
            };
            assert_eq!(
                command(&Workspace::discover().unwrap(), &args).args,
                ["scripts/release_build.py", target, "--dry-run"]
            );
        }
        assert!(Cli::try_parse_from(["xtask", "release-build", "android", "--self-test"]).is_err());
        assert!(Cli::try_parse_from(["xtask", "release-build", "api", "--host", "vps"]).is_err());
    }
}
