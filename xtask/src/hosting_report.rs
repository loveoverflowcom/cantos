//! Offline operator-declared rollout evidence. No publisher, status API or deployment dependency.
use std::{path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::Args;

use crate::{
    command::{CommandSpec, Runner},
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Args)]
pub struct HostingReportArgs {
    /// Local, sanitized rollout record; references are not fetched or verified.
    #[arg(
        long,
        required_unless_present = "self_test",
        conflicts_with = "self_test"
    )]
    pub record: Option<PathBuf>,
    /// Print the local reading plan without probing tools, reading files or writing output.
    #[arg(long, conflicts_with = "self_test")]
    pub dry_run: bool,
    /// Deterministic report regressions; no provider or network access.
    #[arg(long)]
    pub self_test: bool,
}

fn plan(workspace: &Workspace, args: &HostingReportArgs) -> Vec<Step> {
    let mut command = CommandSpec::new("python3", workspace.root());
    if args.self_test {
        command = command.args([
            "-m",
            "unittest",
            "discover",
            "-s",
            "scripts",
            "-p",
            "test_hosting_report.py",
        ]);
    } else if let Some(record) = &args.record {
        command = command.args([
            "scripts/hosting_report.py".to_owned(),
            "--record".to_owned(),
            record.display().to_string(),
        ]);
    }
    vec![Step::new(
        "hosting-report-schema",
        &[Tool::Python3],
        vec![command],
    )]
}

pub fn run(
    workspace: &Workspace,
    args: &HostingReportArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let steps = plan(workspace, args);
    if args.dry_run {
        pipeline::print_plan(&steps, workspace.root());
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = pipeline::execute("hosting-report", &steps, tools, runner);
    report.note("PASS here means a valid local record/test only; all rollout rows are operator declarations, never live verification or permission");
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::{Cli, Task},
        command::testing::RecordingRunner,
        tools::testing::FakeTools,
    };
    use clap::Parser;

    fn args(argv: &[&str]) -> HostingReportArgs {
        let Task::HostingReport(args) = Cli::try_parse_from(argv).unwrap().command else {
            panic!("wrong task")
        };
        args
    }

    #[test]
    fn dry_run_neither_probes_tools_nor_reads_even_a_missing_record() {
        let workspace = Workspace::discover().unwrap();
        let args = args(&[
            "xtask",
            "hosting-report",
            "--record",
            "/missing/record.json",
            "--dry-run",
        ]);
        let mut runner = RecordingRunner::default();
        assert_eq!(
            run(
                &workspace,
                &args,
                &FakeTools::without(&[Tool::Python3]),
                &mut runner
            )
            .unwrap(),
            ExitCode::SUCCESS
        );
        assert!(runner.commands.is_empty());
    }

    #[test]
    fn local_record_is_a_literal_argument_to_one_offline_reader() {
        let workspace = Workspace::discover().unwrap();
        let args = args(&["xtask", "hosting-report", "--record", "$(literal) a.json"]);
        let steps = plan(&workspace, &args);
        assert_eq!(steps.len(), 1);
        assert_eq!(steps[0].commands.len(), 1);
        assert_eq!(steps[0].commands[0].program, "python3");
        assert_eq!(
            steps[0].commands[0].args,
            ["scripts/hosting_report.py", "--record", "$(literal) a.json"]
        );
        assert_eq!(steps[0].commands[0].cwd, workspace.root());
    }

    #[test]
    fn no_implicit_record_or_deployment_arguments() {
        for argv in [
            vec!["xtask", "hosting-report"],
            vec!["xtask", "hosting-report", "--record", "x", "--self-test"],
            vec!["xtask", "hosting-report", "--self-test", "--dry-run"],
            vec!["xtask", "hosting-report", "--record", "x", "--host", "vps"],
        ] {
            assert!(Cli::try_parse_from(argv).is_err());
        }
        let args = args(&["xtask", "hosting-report", "--self-test"]);
        assert_eq!(
            plan(&Workspace::discover().unwrap(), &args)[0].commands[0].args,
            [
                "-m",
                "unittest",
                "discover",
                "-s",
                "scripts",
                "-p",
                "test_hosting_report.py"
            ]
        );
    }
}
