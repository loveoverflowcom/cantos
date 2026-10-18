//! Optional local APK size report. Reads one explicit APK and writes an ignored report; the
//! Python primitive never builds, uploads, signs or installs, and never picks an APK by glob.

use std::{path::PathBuf, process::ExitCode, time::SystemTime};

use anyhow::{Context, Result};
use clap::Args;

use crate::{
    command::{CommandSpec, Runner},
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Args)]
pub struct TheatreSizeArgs {
    /// Exact APK to measure (for example a Gradle output or a staged distribution artifact).
    #[arg(long)]
    pub apk: PathBuf,
    /// Fresh directory beneath artifacts/apk-size; default: a unique run directory.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Earlier report.json from this command for absolute and percent deltas.
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    /// Known source commit of the APK; recorded as `unknown` when omitted, never inferred.
    #[arg(long)]
    pub source_revision: Option<String>,
    /// Print the plan without reading the APK.
    #[arg(long)]
    pub dry_run: bool,
}

fn plan(workspace: &Workspace, args: &TheatreSizeArgs, output: &std::path::Path) -> Vec<Step> {
    let mut command = CommandSpec::new("python3", workspace.root()).args([
        "scripts/apk_size.py".to_owned(),
        "--apk".to_owned(),
        args.apk.display().to_string(),
        "--output".to_owned(),
        output.display().to_string(),
    ]);
    if let Some(baseline) = &args.baseline {
        command = command.args(["--baseline".to_owned(), baseline.display().to_string()]);
    }
    if let Some(revision) = &args.source_revision {
        command = command.args(["--source-revision".to_owned(), revision.clone()]);
    }
    vec![Step::new(
        "measure APK size groups and deltas",
        &[Tool::Python3],
        vec![command],
    )]
}

pub fn run(
    workspace: &Workspace,
    args: &TheatreSizeArgs,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let output = match &args.output {
        Some(output) => output.clone(),
        None => {
            let nonce = SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .context("system clock is before the Unix epoch")?
                .as_nanos();
            workspace.path(&format!(
                "artifacts/apk-size/run-{nonce}-{}",
                std::process::id()
            ))
        }
    };
    let steps = plan(workspace, args, &output);
    if args.dry_run {
        pipeline::print_plan(&steps, workspace.root());
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = pipeline::execute("theatre-size", &steps, tools, runner);
    report.note(
        "APK bytes only; installed footprint, Firebase download and device behavior are NOT_RUN",
    );
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        cli::{Cli, Task},
        command::testing::RecordingRunner,
        report::Status,
        tools::testing::FakeTools,
    };
    use clap::Parser;

    fn args() -> TheatreSizeArgs {
        TheatreSizeArgs {
            apk: PathBuf::from("a b/$(app).apk"),
            output: None,
            baseline: Some(PathBuf::from("base line.json")),
            source_revision: Some("abc123".into()),
            dry_run: false,
        }
    }

    #[test]
    fn plan_passes_literal_paths_and_only_the_supplied_options() {
        let workspace = Workspace::discover().unwrap();
        let steps = plan(
            &workspace,
            &args(),
            std::path::Path::new("artifacts/apk-size/x"),
        );
        assert_eq!(steps.len(), 1);
        let command = &steps[0].commands[0];
        assert_eq!(command.args[0], "scripts/apk_size.py");
        assert!(command
            .args
            .windows(2)
            .any(|pair| pair == ["--apk", "a b/$(app).apk"]));
        assert!(command
            .args
            .windows(2)
            .any(|pair| pair == ["--baseline", "base line.json"]));
        assert!(command
            .args
            .windows(2)
            .any(|pair| pair == ["--source-revision", "abc123"]));
        let bare = TheatreSizeArgs {
            baseline: None,
            source_revision: None,
            ..args()
        };
        let steps = plan(&workspace, &bare, std::path::Path::new("o"));
        let rendered = steps[0].commands[0].args.join(" ");
        assert!(!rendered.contains("--baseline") && !rendered.contains("--source-revision"));
    }

    #[test]
    fn missing_python_runs_nothing_and_a_failing_primitive_is_a_failure() {
        let workspace = Workspace::discover().unwrap();
        let steps = plan(&workspace, &args(), std::path::Path::new("o"));
        let mut runner = RecordingRunner::default();
        let report = pipeline::execute(
            "test",
            &steps,
            &FakeTools::without(&[Tool::Python3]),
            &mut runner,
        );
        assert!(runner.commands.is_empty());
        assert!(!report.succeeded());
        runner.fail_on = Some("apk_size.py".into());
        let report = pipeline::execute("test", &steps, &FakeTools::all(), &mut runner);
        assert_eq!(runner.commands.len(), 1);
        assert!(!report.succeeded());
        assert!(report.outcomes.iter().all(|o| o.status != Status::Pass));
    }

    #[test]
    fn cli_requires_an_explicit_apk() {
        assert!(matches!(
            Cli::try_parse_from(["cargo xtask", "theatre-size", "--apk", "x.apk", "--dry-run"])
                .unwrap()
                .command,
            Task::TheatreSize(_)
        ));
        assert!(Cli::try_parse_from(["cargo xtask", "theatre-size"]).is_err());
    }
}
