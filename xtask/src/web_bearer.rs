//! Explicit loopback browser fixture. No provider, database, deployment or live identity access.
use crate::{
    build::{self, Profile},
    command::{CommandSpec, Runner},
    config::App,
    pipeline::{self, Step},
    report::{Report, Status},
    run_files,
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};
use anyhow::Result;
use std::process::ExitCode;

pub fn steps(workspace: &Workspace) -> Vec<Step> {
    vec![
        Step::new(
            "web-bearer-prerequisites",
            &[Tool::Node],
            vec![CommandSpec::new("node", workspace.root())
                .args(["scripts/test_web_bearer.mjs", "--probe"])],
        ),
        Step::new(
            "web-bearer-studio-build",
            &[Tool::Cargo, Tool::Trunk, Tool::Wasm32Target],
            vec![build::trunk(workspace, App::Studio, "build", Profile::Dev)
                .env("CANTOS_API_ORIGIN", "http://127.0.0.1:18080")],
        ),
        Step::new(
            "web-bearer-theatre-build",
            &[Tool::Cargo, Tool::Trunk, Tool::Wasm32Target],
            vec![build::trunk(workspace, App::Theatre, "build", Profile::Dev)
                .env("CANTOS_API_ORIGIN", "http://127.0.0.1:18080")],
        ),
        Step::new(
            "web-bearer-browser-mock",
            &[Tool::Node],
            vec![CommandSpec::new("node", workspace.root()).args(["scripts/test_web_bearer.mjs"])],
        ),
    ]
}
pub fn execute(
    workspace: &Workspace,
    dry_run: bool,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let steps = steps(workspace);
    if dry_run {
        pipeline::print_plan(&steps, workspace.root());
        return Ok(ExitCode::SUCCESS);
    }
    let executable = std::env::var_os("CANTOS_TEST_CHROMIUM")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome".into());
    if !executable.is_absolute() || !executable.is_file() {
        let mut report = Report::new("verify web-bearer");
        report.record("web-bearer-prerequisites",Status::Unsupported,"installed Chromium required; set an absolute CANTOS_TEST_CHROMIUM; nothing installed or started");
        for step in steps.iter().skip(1) {
            report.record(&step.name, Status::NotRun, "missing browser prerequisite");
        }
        report.print();
        return Ok(report.exit_code());
    }
    let before = run_files::source_identity(workspace.root()).map_err(anyhow::Error::msg)?;
    let mut report = pipeline::execute("verify web-bearer", &steps, tools, runner);
    report.note("Production Web modules, loopback HTTP and mocked identity/API only; real Kanidm, gateway, SQL, Firebase and native media acceptance NOT_RUN. Uses an isolated installed Chromium profile; installs nothing.");
    let after = run_files::source_identity(workspace.root()).map_err(anyhow::Error::msg)?;
    if before != after {
        report.record(
            "web-bearer-source-stability",
            Status::Fail,
            "source changed across build/browser fixture",
        );
    }
    report.print();
    Ok(report.exit_code())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn explicit_mock_lane_cannot_target_a_live_origin_or_enter_ordinary_gates() {
        let workspace = Workspace::discover().unwrap();
        let planned = steps(&workspace);
        assert_eq!(planned.len(), 4);
        for step in &planned {
            for command in &step.commands {
                for (key, value) in &command.env {
                    if key == "CANTOS_API_ORIGIN" {
                        assert_eq!(value, "http://127.0.0.1:18080");
                    }
                }
            }
        }
        assert!(
            crate::gate::verify_suite(&crate::cli::VerifySuite::WebBearer { dry_run: false })
                .is_empty()
        );
        assert!(crate::gate::full()
            .iter()
            .flat_map(|g| g.commands(&workspace))
            .all(|c| !c.args.iter().any(|a| a.contains("test_web_bearer"))));
    }
}
