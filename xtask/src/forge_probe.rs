//! Explicit local packaging/qualification only; no Firebase invocation or remote write.

use std::{path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::{Args, Subcommand};

use crate::{
    command::CommandSpec,
    process::{Supervisor, Wait},
    report::{Report, Status},
    workspace::Workspace,
};

#[derive(Debug, Args)]
pub struct ForgeProbeArgs {
    #[command(subcommand)]
    action: Action,
}

#[derive(Debug, Subcommand)]
enum Action {
    /// Build the locked Linux worker and stage a fresh isolated Functions package.
    Package {
        #[arg(long)]
        project: String,
        /// Operator-selected region; no ambient project or region fallback.
        #[arg(long)]
        region: String,
        /// New directory below artifacts/forge-probe; never overwrites.
        #[arg(long)]
        output: PathBuf,
        /// Validate arguments and print the plan; no subprocess or artifact effects.
        #[arg(long)]
        dry_run: bool,
    },
    /// Check bundle hashes, exact config, executable mode and static Linux ELF shape.
    Inspect {
        #[arg(long)]
        bundle: PathBuf,
    },
    /// Linux x86_64/Node 22 only: install locked SDK without scripts, run actual HTTP/worker tests.
    Qualify {
        #[arg(long)]
        bundle: PathBuf,
    },
    /// Print the exact scoped Firebase primitive and prerequisites; never invokes Firebase.
    DeployPlan {
        #[arg(long)]
        bundle: PathBuf,
    },
    /// Offline deterministic package-policy and corrupt-binary regressions.
    SelfTest,
}

fn command(workspace: &Workspace, args: &ForgeProbeArgs) -> CommandSpec {
    let spec = CommandSpec::new("python3", workspace.root());
    match &args.action {
        Action::SelfTest => spec.args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_forge_probe.py",
        ]),
        Action::Package {
            project,
            region,
            output,
            dry_run,
        } => spec
            .args([
                "scripts/forge_probe.py".to_owned(),
                "package".into(),
                "--project".into(),
                project.clone(),
                "--region".into(),
                region.clone(),
                "--output".into(),
                output.display().to_string(),
            ])
            .arg_if(*dry_run, "--dry-run"),
        Action::Inspect { bundle } | Action::Qualify { bundle } | Action::DeployPlan { bundle } => {
            spec.args([
                "scripts/forge_probe.py".to_owned(),
                match args.action {
                    Action::Inspect { .. } => "inspect",
                    Action::Qualify { .. } => "qualify",
                    _ => "deploy-plan",
                }
                .into(),
                "--bundle".into(),
                bundle.display().to_string(),
            ])
        }
    }
}

pub fn run(workspace: &Workspace, args: &ForgeProbeArgs) -> Result<ExitCode> {
    let spec = command(workspace, args);
    println!("    $ {}", spec.display(workspace.root()));
    let mut supervisor = Supervisor::new()?;
    supervisor.spawn("forge-probe", &spec)?;
    let result = supervisor.wait()?;
    supervisor.shutdown();
    let status = match result {
        Wait::Exited { status, .. } if status.success() => Status::Pass,
        Wait::Exited { status, .. } if status.code() == Some(3) => Status::Unsupported,
        _ => Status::Fail,
    };
    let mut report = Report::new("forge-probe");
    report.record(
        "local probe tooling",
        status,
        "see output; cloud acceptance is NOT_RUN",
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
    fn package_paths_are_literal_and_the_default_never_deploys() {
        let cli = Cli::try_parse_from([
            "cargo xtask",
            "forge-probe",
            "package",
            "--project",
            "cantos-a348b",
            "--region",
            "asia-southeast1",
            "--output",
            "artifacts/forge-probe/a b",
            "--dry-run",
        ])
        .unwrap();
        let Task::ForgeProbe(args) = cli.command else {
            panic!("wrong task")
        };
        let spec = command(&Workspace::discover().unwrap(), &args);
        assert_eq!(spec.program, "python3");
        assert_eq!(spec.args.last().unwrap(), "--dry-run");
        assert!(spec.args.contains(&"artifacts/forge-probe/a b".to_owned()));
        assert!(!spec.args.iter().any(|arg| arg == "deploy"));
        assert!(Cli::try_parse_from(["cargo xtask", "forge-probe", "deploy"]).is_err());
        assert!(Cli::try_parse_from(["cargo xtask", "forge-probe", "package"]).is_err());
    }
}
