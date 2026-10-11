//! Package-local UI-system previews. Plans are immutable values; execution reuses the gate
//! pipeline and process-group supervisor. Neither renderer starts an API or a database.

use std::{process::ExitCode, time::Duration};

use anyhow::{bail, Result};
use clap::{Subcommand, ValueEnum};

use crate::{
    command::{CommandSpec, Runner},
    config::{
        loopback, UI_CMP_DIR, UI_LEPTOS_DIR, UI_PACKAGE, UI_PREVIEW_DIST, UI_PREVIEW_MARKER,
        UI_PREVIEW_PORT, WASM_TARGET,
    },
    gate::Gate,
    pipeline::{self, Step},
    probe::Readiness,
    process::{Supervisor, Wait},
    report::{Report, Status},
    run::{self, LaunchPlan, Service},
    tools::{Tool, ToolProbe},
    workspace::Workspace,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Renderer {
    /// Leptos component gallery in packages/ui-system/leptos.
    Leptos,
    /// Compose Multiplatform desktop preview in packages/ui-system/cmp.
    Cmp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Subcommand)]
pub enum PreviewAction {
    /// Compile the package-local preview (Trunk or compileKotlinDesktop).
    Build,
    /// Supervise only the preview process until Ctrl-C or desktop window closure.
    Run {
        /// Open the Leptos preview in a browser once its own title is ready.
        #[arg(long)]
        open: bool,
    },
    /// Leptos package tests/lint/build, or the CMP desktopTest task.
    Verify,
}

pub fn leptos_command(workspace: &Workspace, subcommand: &str) -> CommandSpec {
    CommandSpec::new("trunk", workspace.path(UI_LEPTOS_DIR))
        .args([
            subcommand,
            "preview.html",
            "--locked",
            "--dist",
            UI_PREVIEW_DIST,
        ])
        .env("NO_COLOR", "true")
}

/// Use the installed, reviewed Gradle version. No wrapper or JDK provisioning.
/// A foreground, non-persistent Gradle invocation keeps the desktop task in our process group.
pub fn cmp_command(workspace: &Workspace, task: &str) -> CommandSpec {
    CommandSpec::new("gradle", workspace.root()).args([
        "--project-dir",
        UI_CMP_DIR,
        "--no-daemon",
        "-Dorg.gradle.java.installations.auto-download=false",
        task,
    ])
}

pub fn cmp_tools() -> Vec<Tool> {
    vec![Tool::Gradle, Tool::Java, Tool::CmpGradleProject]
}

pub fn build_steps(workspace: &Workspace, renderer: Renderer) -> Vec<Step> {
    match renderer {
        Renderer::Leptos => vec![Gate::UiPreview.step(workspace)],
        Renderer::Cmp => vec![Step::new(
            "cmp-ui-preview build",
            &cmp_tools(),
            vec![cmp_command(workspace, "compileKotlinDesktop")],
        )],
    }
}

pub fn verify_steps(workspace: &Workspace, renderer: Renderer) -> Vec<Step> {
    match renderer {
        Renderer::Leptos => vec![
            Step::new(
                "ui-preview tests (leptos)",
                &[Tool::Cargo],
                // Native component string tests use SSR. The preview feature enables CSR and
                // belongs only to the WASM lint/build, not these native SSR assertions.
                vec![CommandSpec::new("cargo", workspace.root())
                    .args(["test", "--locked", "-p", UI_PACKAGE])],
            ),
            Step::new(
                "ui-preview lint (leptos)",
                &[Tool::Cargo, Tool::Clippy, Tool::Wasm32Target],
                vec![CommandSpec::new("cargo", workspace.root()).args([
                    "clippy",
                    "-p",
                    UI_PACKAGE,
                    "--all-targets",
                    "--target",
                    WASM_TARGET,
                    "--features",
                    "preview",
                    "--locked",
                    "--",
                    "-D",
                    "warnings",
                ])],
            ),
            Gate::UiPreview.step(workspace),
        ],
        Renderer::Cmp => vec![Gate::CmpUiPreview.step(workspace)],
    }
}

pub fn leptos_launch_plan(workspace: &Workspace, open: bool) -> LaunchPlan {
    let addr = loopback(UI_PREVIEW_PORT);
    LaunchPlan {
        requirements: Vec::new(),
        prepare: Vec::new(),
        services: vec![Service {
            name: "Leptos UI-system preview (trunk serve)".into(),
            command: leptos_command(workspace, "serve").args([
                "--address",
                "127.0.0.1",
                "--port",
                &UI_PREVIEW_PORT.to_string(),
            ]),
            tools: Gate::UiPreview.tools(),
            addr,
            readiness: Readiness::Http {
                path: "/",
                marker: UI_PREVIEW_MARKER,
            },
            timeout: Duration::from_secs(900),
        }],
        open: open.then(|| format!("http://{addr}")),
    }
}

/// Unlike a Web server, a desktop window has no HTTP readiness endpoint. Supervise the actual
/// Gradle task without inventing a renderer-ready, semantics-tested or device-tested claim.
fn launch_cmp(workspace: &Workspace) -> Result<ExitCode> {
    let command = cmp_command(workspace, "run");
    println!(
        "\n==> start CMP UI-system preview: {}",
        command.display(workspace.root())
    );
    let mut supervisor = Supervisor::new()?;
    supervisor.spawn("CMP UI-system preview", &command)?;
    println!(
        "Gradle desktop task started. Window readiness is not asserted. Press Ctrl-C to stop."
    );
    let outcome = supervisor.wait()?;
    supervisor.shutdown();
    match outcome {
        Wait::Interrupted => {
            println!("Stopped the preview processes xtask started.");
            Ok(ExitCode::SUCCESS)
        }
        Wait::Exited { status, .. } if status.success() => {
            println!("CMP preview task exited successfully.");
            Ok(ExitCode::SUCCESS)
        }
        Wait::Exited { name, status } => bail!("{name} exited with {status}"),
    }
}

fn unsupported_run(renderer: Renderer, tools: &dyn ToolProbe) -> Option<ExitCode> {
    let required = match renderer {
        Renderer::Leptos => Gate::UiPreview.tools(),
        Renderer::Cmp => cmp_tools(),
    };
    let missing = tools.missing(&required);
    if missing.is_empty() {
        return None;
    }
    let details: Vec<_> = missing
        .iter()
        .map(|tool| format!("{} ({})", tool.label(), tool.hint()))
        .collect();
    let mut report = Report::new("ui-preview run");
    report.record(
        format!("{renderer:?} preview run"),
        Status::Unsupported,
        format!("missing {}", details.join(", ")),
    );
    report.print();
    Some(report.exit_code())
}

pub fn execute(
    workspace: &Workspace,
    renderer: Renderer,
    action: PreviewAction,
    tools: &dyn ToolProbe,
    runner: &mut dyn Runner,
) -> Result<ExitCode> {
    let steps = match action {
        PreviewAction::Build => build_steps(workspace, renderer),
        PreviewAction::Verify => verify_steps(workspace, renderer),
        PreviewAction::Run { open } => {
            if renderer == Renderer::Cmp && open {
                eprintln!("xtask: error: --open applies only to the Leptos browser preview; CMP opens its desktop window");
                return Ok(ExitCode::from(2));
            }
            if let Some(code) = unsupported_run(renderer, tools) {
                return Ok(code);
            }
            return match renderer {
                Renderer::Leptos => run::launch(
                    workspace,
                    &leptos_launch_plan(workspace, open),
                    tools,
                    runner,
                ),
                Renderer::Cmp => launch_cmp(workspace),
            };
        }
    };
    let report = pipeline::execute("ui-preview", &steps, tools, runner);
    report.print();
    Ok(report.exit_code())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{command::testing::RecordingRunner, tools::testing::FakeTools};

    fn workspace() -> Workspace {
        Workspace::discover().unwrap()
    }

    #[test]
    fn build_and_verify_plans_use_only_the_package_local_preview_targets() {
        let workspace = workspace();
        let lines = |steps: Vec<Step>| -> Vec<String> {
            steps
                .iter()
                .flat_map(|step| &step.commands)
                .map(|command| command.display(workspace.root()))
                .collect()
        };
        assert_eq!(lines(build_steps(&workspace, Renderer::Leptos)), [
            "(in packages/ui-system/leptos) NO_COLOR=true trunk build preview.html --locked --dist ../../../target/ui-foundation-preview"
        ]);
        assert_eq!(lines(build_steps(&workspace, Renderer::Cmp)), [
            "gradle --project-dir packages/ui-system/cmp --no-daemon -Dorg.gradle.java.installations.auto-download=false compileKotlinDesktop"
        ]);
        assert_eq!(lines(verify_steps(&workspace, Renderer::Cmp)), [
            "gradle --project-dir packages/ui-system/cmp --no-daemon -Dorg.gradle.java.installations.auto-download=false desktopTest"
        ]);
        let leptos = lines(verify_steps(&workspace, Renderer::Leptos));
        assert_eq!(leptos[0], "cargo test --locked -p cantos-ui");
        assert_eq!(leptos[1], "cargo clippy -p cantos-ui --all-targets --target wasm32-unknown-unknown --features preview --locked -- -D warnings");
        assert_eq!(leptos.len(), 3);
        assert!(!leptos
            .iter()
            .any(|line| line.contains("cantos-api") || line.contains("postgres")));
    }

    #[test]
    fn native_ui_tests_keep_ssr_while_the_wasm_preview_lint_uses_csr() {
        let steps = verify_steps(&workspace(), Renderer::Leptos);
        let native = &steps[0].commands[0];
        assert_eq!(native.program, "cargo");
        assert_eq!(native.args, ["test", "--locked", "-p", "cantos-ui"]);
        let wasm = &steps[1].commands[0];
        assert!(wasm
            .args
            .windows(2)
            .any(|pair| pair == ["--target", "wasm32-unknown-unknown"]));
        assert!(wasm
            .args
            .windows(2)
            .any(|pair| pair == ["--features", "preview"]));
    }

    #[test]
    fn leptos_run_owns_one_loopback_trunk_process_and_uses_the_gallery_title() {
        let workspace = workspace();
        let plan = leptos_launch_plan(&workspace, true);
        assert!(plan.requirements.is_empty());
        assert!(plan.prepare.is_empty());
        assert_eq!(plan.services.len(), 1);
        let service = &plan.services[0];
        assert_eq!(service.addr, loopback(8083));
        assert_eq!(
            service.readiness,
            Readiness::Http {
                path: "/",
                marker: "<title>Cantos UI foundation preview</title>"
            }
        );
        assert_eq!(service.command.display(workspace.root()), "(in packages/ui-system/leptos) NO_COLOR=true trunk serve preview.html --locked --dist ../../../target/ui-foundation-preview --address 127.0.0.1 --port 8083");
        assert_eq!(plan.open.as_deref(), Some("http://127.0.0.1:8083"));
        assert_eq!(leptos_launch_plan(&workspace, false).open, None);
    }

    #[test]
    fn every_missing_cmp_prerequisite_is_unsupported_before_commands_run() {
        let workspace = workspace();
        for tool in cmp_tools() {
            let tools = FakeTools::without(&[tool]);
            let mut runner = RecordingRunner::default();
            let report = pipeline::execute(
                "test",
                &build_steps(&workspace, Renderer::Cmp),
                &tools,
                &mut runner,
            );
            assert_eq!(report.outcomes[0].status, Status::Unsupported);
            assert!(runner.commands.is_empty());
            for action in [
                PreviewAction::Build,
                PreviewAction::Verify,
                PreviewAction::Run { open: false },
            ] {
                assert_eq!(
                    execute(&workspace, Renderer::Cmp, action, &tools, &mut runner).unwrap(),
                    ExitCode::FAILURE
                );
                assert!(runner.commands.is_empty());
            }
        }
    }

    #[test]
    fn cmp_browser_flag_is_a_usage_error_before_any_build_or_launch() {
        let mut runner = RecordingRunner::default();
        assert_eq!(
            execute(
                &workspace(),
                Renderer::Cmp,
                PreviewAction::Run { open: true },
                &FakeTools::all(),
                &mut runner
            )
            .unwrap(),
            ExitCode::from(2)
        );
        assert!(runner.commands.is_empty());
    }

    #[test]
    fn missing_leptos_tool_does_not_build_or_start_an_api() {
        let mut runner = RecordingRunner::default();
        assert_eq!(
            execute(
                &workspace(),
                Renderer::Leptos,
                PreviewAction::Run { open: false },
                &FakeTools::without(&[Tool::Trunk]),
                &mut runner
            )
            .unwrap(),
            ExitCode::FAILURE
        );
        assert!(runner.commands.is_empty());
    }
}
