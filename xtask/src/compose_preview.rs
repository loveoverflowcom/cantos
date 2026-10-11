//! Optional Android Studio host rendering. The Python primitive owns capture and validation;
//! this module owns the command surface and maps its exit status into xtask's vocabulary.

use std::{path::PathBuf, process::ExitCode};

use anyhow::Result;
use clap::Args;

use crate::{
    command::CommandSpec,
    config::App,
    report::{Report, Status},
    run::AppTarget,
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Args)]
pub struct ComposePreviewArgs {
    /// Product whose mobile Gradle project is already open in Android Studio Quail 2+.
    #[arg(value_enum)]
    pub target: AppTarget,
    /// Kotlin file containing the preview, absolute or relative to the repository root.
    #[arg(long)]
    pub file: PathBuf,
    /// Name of the @Preview composable; it must wrap production UI with deterministic state.
    #[arg(long)]
    pub composable: String,
    /// Scenario label only; @Preview annotations configure theme, locale and font scale.
    #[arg(long)]
    pub label: Option<String>,
    /// Select an Android Studio instance when several are running.
    #[arg(long, value_parser = clap::value_parser!(u32).range(1..))]
    pub pid: Option<u32>,
    /// Bounded timeout for each CLI operation.
    #[arg(long, default_value_t = 60, value_parser = clap::value_parser!(u16).range(1..=300))]
    pub timeout_seconds: u16,
    /// Official Android CLI executable; supports an explicit path to avoid the legacy SDK tool.
    #[arg(long, default_value = "android")]
    pub android_cli: String,
    /// Print the render plan without contacting the CLI/IDE or writing artifacts.
    #[arg(long)]
    pub dry_run: bool,
}

pub fn command(workspace: &Workspace, args: &ComposePreviewArgs) -> CommandSpec {
    let app = match args.target {
        AppTarget::Cantos => App::Theatre,
        AppTarget::CantosStudio => App::Studio,
    };
    let mut command = CommandSpec::new("python3", workspace.root()).args([
        "scripts/render_compose_preview.py".to_owned(),
        "--app".to_owned(),
        app.cli_name().to_owned(),
        "--file".to_owned(),
        args.file.display().to_string(),
        "--composable".to_owned(),
        args.composable.clone(),
        "--timeout-seconds".to_owned(),
        args.timeout_seconds.to_string(),
        "--android-cli".to_owned(),
        args.android_cli.clone(),
    ]);
    if let Some(label) = &args.label {
        command = command.args(["--label", label]);
    }
    if let Some(pid) = args.pid {
        command = command.args(["--pid".to_owned(), pid.to_string()]);
    }
    command.arg_if(args.dry_run, "--dry-run")
}

pub fn run(
    workspace: &Workspace,
    args: &ComposePreviewArgs,
    tools: &dyn ToolProbe,
) -> Result<ExitCode> {
    let mut report = Report::new("compose-preview");
    if !tools.available(Tool::Python3) {
        report.record("capture", Status::Unsupported, Tool::Python3.hint());
    } else {
        let spec = command(workspace, args);
        println!("    $ {}", spec.display(workspace.root()));
        match spec.to_command().status() {
            Ok(status) if status.success() && args.dry_run => return Ok(ExitCode::SUCCESS),
            Ok(status) if status.success() => report.record(
                "capture",
                Status::Pass,
                "PNG and semantics captured; inspect separately, no device or interaction claim",
            ),
            Ok(status) if status.code() == Some(3) => report.record(
                "capture",
                Status::Unsupported,
                "see primitive output: mobile Gradle project, official CLI and Quail 2+ IDE required",
            ),
            Ok(status) => {
                report.record("capture", Status::Fail, format!("capture exited with {status}"));
                report.print();
                return Ok(if status.code() == Some(2) {
                    ExitCode::from(2)
                } else {
                    ExitCode::FAILURE
                });
            }
            Err(error) => report.record("capture", Status::Fail, error.to_string()),
        }
    }
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::{Cli, Task};
    use clap::Parser;

    #[test]
    fn forwards_preview_options_without_shell_interpolation() {
        let cli = Cli::try_parse_from([
            "cargo xtask",
            "compose-preview",
            "cantos-studio",
            "--file",
            "apps/cantos-studio/mobile/Screen.kt",
            "--composable",
            "ScreenPreview",
            "--label",
            "dark vi-VN large text",
            "--pid",
            "123",
            "--android-cli",
            "/tools with spaces/android",
            "--timeout-seconds",
            "30",
            "--dry-run",
        ])
        .unwrap();
        let Task::ComposePreview(args) = cli.command else {
            panic!("expected compose-preview");
        };
        let workspace = Workspace::discover().unwrap();
        let spec = command(&workspace, &args);
        assert_eq!(spec.program, "python3");
        assert_eq!(spec.cwd, workspace.root());
        assert_eq!(
            spec.args,
            [
                "scripts/render_compose_preview.py",
                "--app",
                "cantos-studio",
                "--file",
                "apps/cantos-studio/mobile/Screen.kt",
                "--composable",
                "ScreenPreview",
                "--timeout-seconds",
                "30",
                "--android-cli",
                "/tools with spaces/android",
                "--label",
                "dark vi-VN large text",
                "--pid",
                "123",
                "--dry-run",
            ]
        );
    }

    #[test]
    fn rejects_missing_preview_identity_and_unbounded_timeouts() {
        for tail in [
            vec!["cantos"],
            vec!["cantos", "--file", "Screen.kt"],
            vec![
                "cantos",
                "--file",
                "Screen.kt",
                "--composable",
                "Preview",
                "--timeout-seconds",
                "0",
            ],
            vec![
                "cantos",
                "--file",
                "Screen.kt",
                "--composable",
                "Preview",
                "--timeout-seconds",
                "301",
            ],
            vec![
                "cantos",
                "--file",
                "Screen.kt",
                "--composable",
                "Preview",
                "--pid",
                "0",
            ],
        ] {
            let error =
                Cli::try_parse_from(["cargo xtask", "compose-preview"].into_iter().chain(tail))
                    .unwrap_err();
            assert_eq!(error.exit_code(), 2);
        }
    }
}
