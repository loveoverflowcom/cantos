//! Optional release-bundle measurement. Source snapshots surround the ordinary Trunk build;
//! static observations never establish deployed headers, transfer bytes or browser timing.

use std::{path::PathBuf, process::ExitCode, time::SystemTime};

use anyhow::{Context, Result};
use clap::{Args, ValueEnum};

use crate::{
    build::{self, Profile, Unit},
    command::{CommandSpec, Runner},
    config::App,
    pipeline::{self, Step},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, ValueEnum)]
pub enum WebApp {
    CantosStudio,
    CantosTheatre,
}

impl WebApp {
    fn app(self) -> App {
        match self {
            Self::CantosStudio => App::Studio,
            Self::CantosTheatre => App::Theatre,
        }
    }
}

#[derive(Debug, Args)]
pub struct WebPerformanceArgs {
    #[arg(value_enum)]
    pub target: WebApp,
    /// Fresh directory beneath artifacts/web-performance; default: <app>-<unique run>.
    #[arg(long)]
    pub output: Option<PathBuf>,
    /// Optional earlier inventory JSON for asset URL/byte and bundle-size differences.
    #[arg(long)]
    pub baseline: Option<PathBuf>,
    /// Print the release-build and inventory plan without executing it.
    #[arg(long)]
    pub dry_run: bool,
}

fn plan(workspace: &Workspace, args: &WebPerformanceArgs, output: &std::path::Path) -> Vec<Step> {
    let app = args.target.app();
    let output = if output.is_absolute() {
        output.to_path_buf()
    } else {
        workspace.root().join(output)
    };
    let script = |phase: &str| {
        CommandSpec::new("python3", workspace.root()).args([
            "scripts/web_performance.py".to_owned(),
            phase.to_owned(),
            "--app".to_owned(),
            app.cli_name().to_owned(),
            "--output".to_owned(),
            output.display().to_string(),
        ])
    };
    let mut inventory = script("inventory");
    if let Some(baseline) = &args.baseline {
        inventory = inventory.args(["--baseline".to_owned(), baseline.display().to_string()]);
    }
    let mut release = build::step(workspace, Unit::Web(app), Profile::Release, false);
    // Command-owned fresh output prevents an inherited/configured dist from making a
    // successful build inventory an older bundle. Target/config remain the app's own.
    release.commands[0] = build::release_bundle(workspace, app, &output.join("dist"));
    vec![
        Step::new(
            "capture Web build provenance",
            &[Tool::Python3, Tool::Git, Tool::Cargo, Tool::Trunk],
            vec![script("prepare")],
        ),
        release,
        Step::new(
            "inventory release Web assets",
            &[Tool::Python3, Tool::Git],
            vec![inventory],
        ),
    ]
}

pub fn run(
    workspace: &Workspace,
    args: &WebPerformanceArgs,
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
                "artifacts/web-performance/{}-{nonce}-{}",
                args.target.app().cli_name(),
                std::process::id()
            ))
        }
    };
    let steps = plan(workspace, args, &output);
    if args.dry_run {
        pipeline::print_plan(&steps, workspace.root());
        return Ok(ExitCode::SUCCESS);
    }
    let mut report = pipeline::execute("web-performance", &steps, tools, runner);
    report.note("inventory/compression estimates only; host headers, browser timing and cache hits are NOT_RUN");
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

    fn args(target: WebApp) -> WebPerformanceArgs {
        WebPerformanceArgs {
            target,
            output: Some(PathBuf::from("artifacts/a b")),
            baseline: Some(PathBuf::from("$(literal).json")),
            dry_run: false,
        }
    }

    #[test]
    fn both_apps_use_the_existing_release_build_between_source_snapshots() {
        let workspace = Workspace::discover().unwrap();
        for target in [WebApp::CantosStudio, WebApp::CantosTheatre] {
            let args = args(target);
            let steps = plan(&workspace, &args, args.output.as_ref().unwrap());
            assert_eq!(steps.len(), 3);
            assert_eq!(steps[0].commands[0].args[1], "prepare");
            let ordinary =
                build::step(&workspace, Unit::Web(target.app()), Profile::Release, false);
            assert!(steps[1].commands[0]
                .args
                .starts_with(&ordinary.commands[0].args));
            assert_eq!(steps[1].commands[0].cwd, ordinary.commands[0].cwd);
            assert_eq!(steps[1].commands[0].env, ordinary.commands[0].env);
            assert!(steps[1].commands[0]
                .args
                .windows(2)
                .any(|pair| pair[0] == "--dist"
                    && pair[1] == workspace.path("artifacts/a b/dist").display().to_string()));
            assert_eq!(steps[1].commands[0].args.last().unwrap(), "index.html");
            assert_eq!(steps[2].commands[0].args[1], "inventory");
            assert!(steps[2].commands[0]
                .args
                .windows(2)
                .any(|pair| pair == ["--baseline", "$(literal).json"]));
        }
    }

    #[test]
    fn missing_tools_or_failed_build_never_inventory_an_old_dist() {
        let workspace = Workspace::discover().unwrap();
        let args = args(WebApp::CantosStudio);
        let steps = plan(&workspace, &args, args.output.as_ref().unwrap());
        let mut runner = RecordingRunner::default();
        let report = pipeline::execute(
            "test",
            &steps,
            &FakeTools::without(&[Tool::Trunk]),
            &mut runner,
        );
        assert!(runner.commands.is_empty());
        assert!(!report.succeeded());
        runner.fail_on = Some("trunk build".into());
        let report = pipeline::execute("test", &steps, &FakeTools::all(), &mut runner);
        assert_eq!(runner.commands.len(), 2);
        assert_eq!(report.outcomes[2].status, Status::NotRun);
    }

    #[test]
    fn cli_only_accepts_the_two_web_apps() {
        assert!(matches!(
            Cli::try_parse_from([
                "cargo xtask",
                "web-performance",
                "cantos-theatre",
                "--dry-run"
            ])
            .unwrap()
            .command,
            Task::WebPerformance(_)
        ));
        assert!(Cli::try_parse_from(["cargo xtask", "web-performance", "cantos-server"]).is_err());
        assert!(Cli::try_parse_from([
            "cargo xtask",
            "web-performance",
            "cantos-studio",
            "--mobile"
        ])
        .is_err());
    }
}
